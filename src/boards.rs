/* ************************************************************************
*   File: boards.rs                                     Part of CircleMUD *
*  Usage: handling of multiple bulletin boards                            *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

/* FEATURES & INSTALLATION INSTRUCTIONS ***********************************

This board code has many improvements over the infamously buggy standard
Diku board code.  Features include:

- Arbitrary number of boards handled by one set of generalized routines.
  Adding a new board is as easy as adding another entry to an array.
- Safe removal of messages while other messages are being written.
- Does not allow messages to be removed by someone of a level less than
  the poster's level.


TO ADD A NEW BOARD, simply follow our easy 4-step program:

1 - Create a new board object in the object files

2 - Increase the NUM_OF_BOARDS constant in boards.h

3 - Add a new line to the BOARD_INFO array below.  The fields, in order, are:

    Board's virtual number.
    Min level one must be to look at this board or read messages on it.
    Min level one must be to post a message to the board.
    Min level one must be to remove other people's messages from this
        board (but you can always remove your own message).
    Filename of this board, in quotes.
    Last field must always be 0.

4 - In spec_assign.c, find the section which assigns the special procedure
    gen_board to the other bulletin boards, and add your new one in a
    similar fashion.

*/

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::rc::Rc;
use std::{fs, mem, process, slice};

use log::error;

use crate::db::{parse_c_string, DB};
use crate::handler::isname;
use crate::interpreter::{delete_doubledollar, find_command, is_number, one_argument};
use crate::modify::{page_string, string_write};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    CharData, ObjData, ObjRnum, ObjVnum, LVL_FREEZE, LVL_GOD, LVL_GRGOD, LVL_IMMORT, LVL_IMPL,
    NOTHING,
};
use crate::util::{ctime, time_now};
use crate::{send_to_char, Game, TO_ROOM};

const NUM_OF_BOARDS: usize = 4; /* change if needed! */
const MAX_BOARD_MESSAGES: usize = 60; /* arbitrary -- change if needed */
const MAX_MESSAGE_LENGTH: usize = 4096; /* arbitrary -- change if needed */

const INDEX_SIZE: usize = ((NUM_OF_BOARDS * MAX_BOARD_MESSAGES) + 5) as usize;

pub const BOARD_MAGIC: i64 = 1048575; /* arbitrary number - see modify.c */

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct BoardMsgInfoRecord {
    slot_num: usize,
    level: i32,
    heading_len: usize,
    message_len: usize,
}

struct BoardMsginfo {
    slot_num: Option<usize>,
    /* pos of message in "master index" */
    heading: Option<Rc<str>>,
    /* pointer to message's heading */
    level: i32,
    /* level of poster */
    heading_len: usize,
    /* size of header (for file write) */
    message_len: usize,
    /* size of message text (for file write) */
}

struct BoardInfoType {
    vnum: ObjVnum,
    /* vnum of this board */
    read_lvl: i16,
    /* min level to read messages on this board */
    write_lvl: i16,
    /* min level to write messages on this board */
    remove_lvl: i16,
    /* min level to remove messages from this board */
    filename: &'static str,
    /* file to save this board to */
    rnum: Cell<ObjRnum>,
    /* rnum of this board */
}

/* Board appearance order. */
const NEWEST_AT_TOP: bool = false;

pub struct BoardSystem {
    loaded: bool,
    msg_storage: [Rc<RefCell<String>>; INDEX_SIZE],
    msg_storage_taken: [bool; INDEX_SIZE],
    num_of_msgs: [usize; NUM_OF_BOARDS],
    acmd_read: usize,
    acmd_look: usize,
    acmd_examine: usize,
    acmd_write: usize,
    acmd_remove: usize,
    msg_index: [[BoardMsginfo; MAX_BOARD_MESSAGES]; NUM_OF_BOARDS],
    boardinfo: [BoardInfoType; NUM_OF_BOARDS],
}

impl BoardSystem {
    pub(crate) fn new() -> BoardSystem {
        BoardSystem {
            loaded: false,
            msg_storage: [(); INDEX_SIZE].map(|_| Rc::new(RefCell::new(String::new()))),
            msg_storage_taken: [false; INDEX_SIZE],
            num_of_msgs: [0; NUM_OF_BOARDS],
            acmd_read: 0,
            acmd_look: 0,
            acmd_examine: 0,
            acmd_write: 0,
            acmd_remove: 0,
            msg_index: [(); NUM_OF_BOARDS].map(|_e| {
                [(); MAX_BOARD_MESSAGES].map(|_e2| BoardMsginfo {
                    slot_num: None,
                    heading: None,
                    level: 0,
                    heading_len: 0,
                    message_len: 0,
                })
            }),
            boardinfo: [
                BoardInfoType {
                    vnum: 3099,
                    read_lvl: 0,
                    write_lvl: 0,
                    remove_lvl: LVL_GOD,
                    filename: "./etc/board.mort",
                    rnum: Cell::new(0),
                },
                BoardInfoType {
                    vnum: 3098,
                    read_lvl: LVL_IMMORT,
                    write_lvl: LVL_IMMORT,
                    remove_lvl: LVL_GRGOD,
                    filename: "./etc/board.immort",
                    rnum: Cell::new(0),
                },
                BoardInfoType {
                    vnum: 3097,
                    read_lvl: LVL_IMMORT,
                    write_lvl: LVL_FREEZE as i16,
                    remove_lvl: LVL_IMPL,
                    filename: "./etc/board.freeze",
                    rnum: Cell::new(0),
                },
                BoardInfoType {
                    vnum: 3096,
                    read_lvl: 0,
                    write_lvl: 0,
                    remove_lvl: LVL_IMMORT,
                    filename: "./etc/board.social",
                    rnum: Cell::new(0),
                },
            ],
        }
    }
}

fn find_slot(b: &mut BoardSystem) -> Option<usize> {
    for i in 0..INDEX_SIZE {
        if !b.msg_storage_taken[i] {
            b.msg_storage_taken[i] = true;
            return Some(i);
        }
    }
    None
}

/* search the room ch is standing in to find which board he's looking at */
fn find_board(b: &mut BoardSystem, db: &DB, ch: &Rc<CharData>) -> Option<usize> {
    for obj in db.world[ch.in_room() as usize]
        .contents
        .iter()
    {
        for i in 0..NUM_OF_BOARDS {
            if b.boardinfo[i].rnum.get() == obj.get_obj_rnum() {
                return Some(i);
            }
        }
    }

    if ch.get_level() >= LVL_IMMORT as u8 {
        for obj in ch.carrying.borrow().iter() {
            for i in 0..NUM_OF_BOARDS {
                if b.boardinfo[i].rnum.get() == obj.get_obj_rnum() {
                    return Some(i);
                }
            }
        }
    }

    None
}

fn init_boards(b: &mut BoardSystem, db: &DB) {
    let mut fatal_error = 0;
    for i in 0..INDEX_SIZE {
        *RefCell::borrow_mut(&b.msg_storage[i]) = String::new();
        b.msg_storage_taken[i] = false;
    }
    for i in 0..NUM_OF_BOARDS {
        let rnum;
        if {
            rnum = db.real_object(b.boardinfo[i].vnum);
            rnum == NOTHING
        } {
            error!(
                "SYSERR: Fatal board error: board vnum {} does not exist!",
                b.boardinfo[i].vnum
            );
            fatal_error = 1;
        } else {
            b.boardinfo[i].rnum.set(rnum);
        }
        b.num_of_msgs[i] = 0;
        for j in 0..MAX_BOARD_MESSAGES {
            b.msg_index[i][j].slot_num = None;
        }
        board_load_board(b, i);
    }

    b.acmd_read = find_command("read").unwrap();
    b.acmd_write = find_command("write").unwrap();
    b.acmd_remove = find_command("remove").unwrap();
    b.acmd_look = find_command("look").unwrap();
    b.acmd_examine = find_command("examine").unwrap();

    if fatal_error != 0 {
        process::exit(1);
    }
}

pub fn gen_board(
    game: &mut Game,
    ch: &Rc<CharData>,
    me: &dyn Any,
    cmd: i32,
    argument: &str,
) -> bool {
    let cmd = cmd as usize;
    let db = &game.db;
    let board = me.downcast_ref::<Rc<ObjData>>().unwrap();
    let b: &mut BoardSystem = &mut db.boards.borrow_mut();
    if !b.loaded {
        init_boards(b, db);
        b.loaded = true;
    }
    if ch.desc.borrow().is_none() {
        return false;
    }

    if cmd != b.acmd_write
        && cmd != b.acmd_look
        && cmd != b.acmd_examine
        && cmd != b.acmd_read
        && cmd != b.acmd_remove
    {
        return false;
    }

    let board_type;
    if {
        board_type = find_board(b, db, ch);
        board_type.is_none()
    } {
        error!("SYSERR:  degenerate board!  (what the hell...)");
        return false;
    }
    let board_type = board_type.unwrap();

    return if cmd == b.acmd_write {
        board_write_message(b, db, board_type, ch, argument)
    } else if cmd == b.acmd_look || cmd == b.acmd_examine {
        board_show_board(b, db, board_type, ch, argument, board)
    } else if cmd == b.acmd_read {
        board_display_msg(b, game, board_type, ch, argument, board)
    } else if cmd == b.acmd_remove {
        board_remove_msg(b, game, board_type, ch, argument)
    } else {
        false
    };
}

fn board_write_message(
    b: &mut BoardSystem,
    db: &DB,
    board_type: usize,
    ch: &Rc<CharData>,
    arg: &str,
) -> bool {
    if ch.get_level() < b.boardinfo[board_type].write_lvl as u8 {
        send_to_char(ch, "You are not holy enough to write on this board.\r\n");
        return true;
    }
    if b.num_of_msgs[board_type] >= MAX_BOARD_MESSAGES {
        send_to_char(ch, "The board is full.\r\n");
        return true;
    }
    let slot;
    if {
        slot = find_slot(b);
        slot.is_none()
    } {
        send_to_char(ch, "The board is malfunctioning - sorry.\r\n");
        error!("SYSERR: Board: failed to find empty slot on write.");
        return false;
    }

    b.msg_index[board_type][b.num_of_msgs[board_type]].slot_num = slot;
    /* skip blanks */
    let mut arg = arg.trim_start().to_string();
    delete_doubledollar(&mut arg);

    /* JE 27 Oct 95 - Truncate headline at 80 chars if it's longer than that */
    arg.truncate(80);

    if arg.is_empty() {
        send_to_char(ch, "We must have a headline!\r\n");
        return true;
    }
    let ct = time_now();
    let tmstr = ctime(ct);

    let buf2 = format!("({})", ch.get_name());
    let buf = format!("{:10} {:12} :: {}", tmstr, buf2, arg);
    b.msg_index[board_type][b.num_of_msgs[board_type]].heading = Some(Rc::from(buf.as_str()));
    b.msg_index[board_type][b.num_of_msgs[board_type]].level = ch.get_level() as i32;

    send_to_char(
        ch,
        "Write your message.  Terminate with a @ on a new line.\r\n\r\n",
    );
    db.act(
        "$n starts to write a message.",
        true,
        Some(ch),
        None,
        None,
        TO_ROOM,
    );

    string_write(
        ch.desc.borrow().as_ref().unwrap(),
        b.msg_storage[b.msg_index[board_type][b.num_of_msgs[board_type]]
            .slot_num
            .unwrap()]
        .clone(),
        MAX_MESSAGE_LENGTH,
        board_type as i64 + BOARD_MAGIC,
    );

    b.num_of_msgs[board_type] += 1;
    return true;
}

fn board_show_board(
    b: &mut BoardSystem,
    db: &DB,
    board_type: usize,
    ch: &Rc<CharData>,
    arg: &str,
    board: &Rc<ObjData>,
) -> bool {
    if ch.desc.borrow().is_none() {
        return false;
    }
    let mut tmp = String::new();
    one_argument(arg, &mut tmp);

    if tmp.is_empty() || !isname(&tmp, board.name.borrow().as_str()) {
        return false;
    }

    if ch.get_level() < b.boardinfo[board_type].read_lvl as u8 {
        send_to_char(ch, "You try but fail to understand the holy words.\r\n");
        return true;
    }
    db.act("$n studies the board.", true, Some(ch), None, None, TO_ROOM);

    if b.num_of_msgs[board_type] == 0 {
        send_to_char(ch, "This is a bulletin board.  Usage: READ/REMOVE <messg #>, WRITE <header>.\r\nThe board is empty.\r\n");
    } else {
        let mut buf = format!(
            "This is a bulletin board.  Usage: READ/REMOVE <messg #>, WRITE <header>.\r\n\
You will need to look at the board to save your message.\r\n\
There are {} messages on the board.\r\n",
            b.num_of_msgs[board_type]
        );
        if NEWEST_AT_TOP {
            for i in (0..b.num_of_msgs[board_type]).rev() {
                if b.msg_index[board_type][i].heading.clone().is_none() {
                    error!("SYSERR: Board {} is fubar'd.", board_type);
                    send_to_char(ch, "Sorry, the board isn't working.\r\n");
                    return true;
                }

                buf.push_str(
                    format!(
                        "{:2} : {}\r\n",
                        i + 1,
                        b.msg_index[board_type][i].heading.as_ref().unwrap()
                    )
                    .as_str(),
                );
            }
        } else {
            for i in 0..b.num_of_msgs[board_type] {
                if b.msg_index[board_type][i].heading.is_none() {
                    error!("SYSERR: Board {} is fubar'd.", board_type);
                    send_to_char(ch, "Sorry, the board isn't working.\r\n");
                    return true;
                }

                buf.push_str(
                    format!(
                        "{:2} : {}\r\n",
                        i + 1,
                        b.msg_index[board_type][i].heading.as_ref().unwrap()
                    )
                    .as_str(),
                );
            }
        }
        page_string(ch.desc.borrow().as_ref().unwrap(), &buf, true);
    }
    return true;
}

fn board_display_msg(
    b: &mut BoardSystem,
    game: &Game,
    board_type: usize,
    ch: &Rc<CharData>,
    arg: &str,
    board: &Rc<ObjData>,
) -> bool {
    let db = &game.db;
    let mut number = String::new();
    one_argument(arg, &mut number);
    if number.is_empty() {
        return false;
    }
    if isname(&number, &board.name.borrow()) {
        /* so "read board" works */
        return board_show_board(b, db, board_type, ch, arg, board);
    }
    if !is_number(&number) {
        /* read 2.mail, look 2.sword */
        return false;
    }
    let msg = number.parse::<i32>().unwrap();
    if msg == 0 {
        return false;
    }

    if ch.get_level() < b.boardinfo[board_type].read_lvl as u8 {
        send_to_char(ch, "You try but fail to understand the holy words.\r\n");
        return true;
    }
    if b.num_of_msgs[board_type] == 0 {
        send_to_char(ch, "The board is empty!\r\n");
        return true;
    }
    if msg < 1 || msg > b.num_of_msgs[board_type] as i32 {
        send_to_char(ch, "That message exists only in your imagination.\r\n");
        return true;
    }
    let ind;
    if NEWEST_AT_TOP {
        ind = b.num_of_msgs[board_type] - msg as usize;
    } else {
        ind = msg as usize - 1;
    }
    let msg_slot_numo = b.msg_index[board_type][ind].slot_num;
    let mut msg_slot_num = 0;
    if msg_slot_numo.is_none() || {
        msg_slot_num = msg_slot_numo.unwrap();
        msg_slot_num >= INDEX_SIZE
    } {
        send_to_char(ch, "Sorry, the board is not working.\r\n");
        error!(
            "SYSERR: Board is screwed up. (Room #{})",
            db.get_room_vnum(ch.in_room())
        );
        return true;
    }

    if b.msg_index[board_type][ind].heading.is_none() {
        send_to_char(ch, "That message appears to be screwed up.\r\n");
        return true;
    }

    if RefCell::borrow(&b.msg_storage[msg_slot_num]).is_empty() {
        send_to_char(ch, "That message seems to be empty.\r\n");
        return true;
    }
    let buffer = format!(
        "Message {} : {}\r\n\r\n{}\r\n",
        msg,
        b.msg_index[board_type][ind].heading.as_ref().unwrap(),
        RefCell::borrow(&b.msg_storage[msg_slot_num])
    );

    page_string(ch.desc.borrow().as_ref().unwrap(), &buffer, true);

    true
}

fn board_remove_msg(
    b: &mut BoardSystem,
    game: &Game,
    board_type: usize,
    ch: &Rc<CharData>,
    arg: &str,
) -> bool {
    let db = &game.db;
    let mut number = String::new();
    one_argument(arg, &mut number);

    if number.is_empty() || !is_number(&number) {
        return false;
    }
    let msg = number.parse::<i32>().unwrap();
    if msg == 0 {
        return false;
    }

    if b.num_of_msgs[board_type] == 0 {
        send_to_char(ch, "The board is empty!\r\n");
        return true;
    }
    if msg < 1 || msg as usize > b.num_of_msgs[board_type] {
        send_to_char(ch, "That message exists only in your imagination.\r\n");
        return true;
    }
    let ind;
    if NEWEST_AT_TOP {
        ind = b.num_of_msgs[board_type] - msg as usize;
    } else {
        ind = msg as usize - 1;
    }

    if b.msg_index[board_type][ind].heading.is_none() {
        send_to_char(ch, "That message appears to be screwed up.\r\n");
        return true;
    }
    let buf = format!("({})", ch.get_name());
    if ch.get_level() < b.boardinfo[board_type].remove_lvl as u8
        && !b.msg_index[board_type][ind]
            .heading
            .as_ref()
            .unwrap()
            .contains(&buf)
    {
        send_to_char(
            ch,
            "You are not holy enough to remove other people's messages.\r\n",
        );
        return true;
    }
    if ch.get_level() < b.msg_index[board_type][ind].level as u8 {
        send_to_char(ch, "You can't remove a message holier than yourself.\r\n");
        return true;
    }
    let slot_numo = b.msg_index[board_type][ind].slot_num;
    let mut slot_num = 0;
    if slot_numo.is_none() || {
        slot_num = slot_numo.unwrap();
        slot_num >= INDEX_SIZE
    } {
        send_to_char(ch, "That message is majorly screwed up.\r\n");
        error!(
            "SYSERR: The board is seriously screwed up. (Room #{})",
            db.get_room_vnum(ch.in_room())
        );
        return true;
    }
    for d in game.descriptor_list.iter() {
        if d.state() == ConPlaying
            && d.str.borrow().is_some()
            && Rc::ptr_eq(d.str.borrow().as_ref().unwrap(), &b.msg_storage[slot_num])
        {
            send_to_char(
                ch,
                "At least wait until the author is finished before removing it!\r\n",
            );
            return true;
        }
    }
    if !RefCell::borrow(&b.msg_storage[slot_num]).is_empty() {
        *RefCell::borrow_mut(&b.msg_storage[slot_num]) = String::new();
    }
    b.msg_storage_taken[slot_num] = false;
    if !b.msg_index[board_type][ind].heading.is_none() {
        b.msg_index[board_type][ind].heading = None;
    }

    for i in ind..b.num_of_msgs[board_type] - 1 {
        b.msg_index[board_type][i].heading = b.msg_index[board_type][i + 1].heading.clone();
        b.msg_index[board_type][i].slot_num = b.msg_index[board_type][i + 1].slot_num;
        b.msg_index[board_type][i].level = b.msg_index[board_type][i + 1].level;
    }
    b.msg_index[board_type][b.num_of_msgs[board_type] - 1].heading = None;
    b.msg_index[board_type][b.num_of_msgs[board_type] - 1].slot_num = None;
    b.msg_index[board_type][b.num_of_msgs[board_type] - 1].level = 0;
    b.num_of_msgs[board_type] -= 1;

    send_to_char(ch, "Message removed.\r\n");
    let buf = format!("$n just removed message {}.", msg);
    db.act(&buf, false, Some(ch), None, None, TO_ROOM);
    board_save_board(b, board_type);

    return true;
}

pub fn board_save_board(b: &mut BoardSystem, board_type: usize) {
    let filename = b.boardinfo[board_type].filename;

    if b.num_of_msgs[board_type] == 0 {
        fs::remove_file(filename).expect("removing board file");
        return;
    }
    let fl = OpenOptions::new().write(true).create(true).open(filename);

    if fl.is_err() {
        let err = fl.err().unwrap();
        error!("SYSERR: Error writing board {}", err);
        return;
    }
    let mut fl = fl.unwrap();
    unsafe {
        let num_slice = slice::from_raw_parts(
            &mut b.num_of_msgs[board_type] as *mut _ as *mut u8,
            mem::size_of::<usize>(),
        );
        fl.write_all(num_slice)
            .expect("Error while number of messages in board");
    }

    for i in 0..b.num_of_msgs[board_type] {
        let tmp1 = b.msg_index[board_type][i].heading.as_ref();
        if tmp1.is_some() {
            b.msg_index[board_type][i].heading_len = tmp1.as_ref().unwrap().as_bytes().len();
        } else {
            b.msg_index[board_type][i].heading_len = 0;
        }

        let msg_slotnum = b.msg_index[board_type][i].slot_num.unwrap();
        let tmp2 = &b.msg_storage[msg_slotnum];

        if !RefCell::borrow(tmp2).is_empty() {
            b.msg_index[board_type][i].message_len = RefCell::borrow(tmp2).as_bytes().len();
        } else {
            b.msg_index[board_type][i].message_len = 0;
        }

        let mut record = BoardMsgInfoRecord {
            slot_num: msg_slotnum,
            level: b.msg_index[board_type][i].level,
            heading_len: b.msg_index[board_type][i].heading_len,
            message_len: b.msg_index[board_type][i].message_len,
        };

        unsafe {
            let msginfo_slice = slice::from_raw_parts(
                &mut record as *mut _ as *mut u8,
                mem::size_of::<BoardMsgInfoRecord>(),
            );
            fl.write_all(msginfo_slice)
                .expect("Error while number of messages in board");
        }

        if b.msg_index[board_type][i].heading_len != 0 {
            fl.write_all(
                b.msg_index[board_type][i]
                    .heading
                    .as_ref()
                    .unwrap()
                    .as_bytes(),
            )
            .expect("writing board message heading");
        }

        if !RefCell::borrow(tmp2).is_empty() {
            fl.write_all(RefCell::borrow(tmp2).as_bytes())
                .expect("writing board message");
        }
    }
}

fn board_load_board(b: &mut BoardSystem, board_type: usize) {
    let fl = OpenOptions::new()
        .read(true)
        .open(b.boardinfo[board_type].filename);

    if fl.is_err() {
        let err = fl.err().unwrap();
        error!(
            "SYSERR: Error reading board ({}): {}",
            b.boardinfo[board_type].filename, err
        );
        return;
    }
    let mut fl = fl.unwrap();

    unsafe {
        let config_slice = slice::from_raw_parts_mut(
            &mut b.num_of_msgs[board_type] as *mut _ as *mut u8,
            mem::size_of::<usize>(),
        );
        // `read_exact()` comes from `Read` impl for `&[u8]`
        let r = fl.read_exact(config_slice);
        if r.is_err() {
            let r = r.err().unwrap();
            error!("[SYSERR] Error while board file {} {r}", board_type);
            board_reset_board(b, board_type);
            return;
        }
    }

    if b.num_of_msgs[board_type] < 1 || b.num_of_msgs[board_type] > MAX_BOARD_MESSAGES {
        error!("SYSERR: Board file {} corrupt.  Resetting.", board_type);
        board_reset_board(b, board_type);
        return;
    }

    for i in 0..b.num_of_msgs[board_type] {
        let mut record = BoardMsgInfoRecord {
            slot_num: 0,
            level: 0,
            heading_len: 0,
            message_len: 0,
        };
        unsafe {
            let config_slice = slice::from_raw_parts_mut(
                &mut record as *mut _ as *mut u8,
                mem::size_of::<BoardMsgInfoRecord>(),
            );
            let r = fl.read_exact(config_slice);
            if r.is_err() {
                let r = r.err().unwrap();
                error!(
                    "[SYSERR] Error while board file record, Resetting. {} {r}",
                    board_type
                );
                board_reset_board(b, board_type);
                return;
            }
        }

        b.msg_index[board_type][i].slot_num = Some(record.slot_num);
        b.msg_index[board_type][i].level = record.level;
        b.msg_index[board_type][i].heading_len = record.heading_len;
        b.msg_index[board_type][i].message_len = record.message_len;

        let len1;
        if {
            len1 = b.msg_index[board_type][i].heading_len;
            len1 <= 0
        } {
            error!("SYSERR: Board file {} corrupt!  Resetting.", board_type);
            board_reset_board(b, board_type);
            return;
        }
        let mut tmp1 = vec![0 as u8; len1];
        let tmp1 = tmp1.as_mut_slice();
        fl.read_exact(tmp1)
            .expect("Error reading board file message");
        let heading: Option<Rc<str>> = Some(Rc::from(parse_c_string(tmp1).as_str()));
        b.msg_index[board_type][i].heading = heading;
        let sn;
        if {
            sn = find_slot(b);
            sn.is_none()
        } {
            error!(
                "SYSERR: Out of slots booting board {}!  Resetting...",
                board_type
            );
            board_reset_board(b, board_type);
            return;
        }
        b.msg_index[board_type][i].slot_num = sn;
        let len2;
        if {
            len2 = b.msg_index[board_type][i].message_len;
            len2 > 0
        } {
            let mut tmp2 = vec![0 as u8; len2];
            fl.read_exact(tmp2.as_mut_slice())
                .expect("Error reading board file message string");
            *RefCell::borrow_mut(&b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()]) =
                parse_c_string(tmp2.as_slice());
        } else {
            *RefCell::borrow_mut(&b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()]) =
                String::new();
        }
    }
}

/* When shutting down, clear all boards. */
pub fn board_clear_all(b: &mut BoardSystem) {
    for i in 0..NUM_OF_BOARDS {
        board_clear_board(b, i);
    }
}

/* Clear the in-memory structures. */
fn board_clear_board(b: &mut BoardSystem, board_type: usize) {
    for i in 0..MAX_BOARD_MESSAGES {
        if !b.msg_index[board_type][i].heading.is_none() {
            b.msg_index[board_type][i].heading = None;
        }
        if b.msg_index[board_type][i].slot_num.is_some()
            && !RefCell::borrow(&b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()])
                .is_empty()
        {
            *RefCell::borrow_mut(&b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()]) =
                String::new();
                b.msg_storage_taken[b.msg_index[board_type][i].slot_num.unwrap()] = false;
        }

        b.msg_index[board_type][i].slot_num = None;
    }
    b.num_of_msgs[board_type] = 0;
}

/* Destroy the on-disk and in-memory board. */
fn board_reset_board(b: &mut BoardSystem, board_type: usize) {
    board_clear_board(b, board_type);
    fs::remove_file(b.boardinfo[board_type].filename).expect("Removing board file");
}
