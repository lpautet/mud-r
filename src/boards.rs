/* ************************************************************************
*   File: boards.c                                      Part of CircleMUD *
*  Usage: handling of multiple bulletin boards                            *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
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
use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::ptr::addr_of_mut;
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
    rnum: ObjRnum,
    /* rnum of this board */
}

// #define BOARD_VNUM(i) (BOARD_INFO[i].vnum)
// #define READ_LVL(i) (BOARD_INFO[i].read_lvl)
// #define WRITE_LVL(i) (BOARD_INFO[i].write_lvl)
// #define REMOVE_LVL(i) (BOARD_INFO[i].remove_lvl)
// #define FILENAME(i) (BOARD_INFO[i].filename)
// #define BOARD_RNUM(i) (BOARD_INFO[i].rnum)
//
// #define NEW_MSG_INDEX(i) (msg_index[i][num_of_msgs[i]])
// #define MSG_HEADING(i, j) (msg_index[i][j].heading)
// #define MSG_SLOTNUM(i, j) (msg_index[i][j].slot_num)
// #define MSG_LEVEL(i, j) (msg_index[i][j].level)

/* Board appearance order. */
const NEWEST_AT_TOP: bool = false;

/*
format:	vnum, read lvl, write lvl, remove lvl, filename, 0 at end
Be sure to also change NUM_OF_BOARDS in board.h
*/
const BOARD_INFO: [BoardInfoType; NUM_OF_BOARDS] = [
    BoardInfoType {
        vnum: 3099,
        read_lvl: 0,
        write_lvl: 0,
        remove_lvl: LVL_GOD,
        filename: "./etc/board.mort",
        rnum: 0,
    },
    BoardInfoType {
        vnum: 3098,
        read_lvl: LVL_IMMORT,
        write_lvl: LVL_IMMORT,
        remove_lvl: LVL_GRGOD,
        filename: "./etc/board.immort",
        rnum: 0,
    },
    BoardInfoType {
        vnum: 3097,
        read_lvl: LVL_IMMORT,
        write_lvl: LVL_FREEZE as i16,
        remove_lvl: LVL_IMPL,
        filename: "./etc/board.freeze",
        rnum: 0,
    },
    BoardInfoType {
        vnum: 3096,
        read_lvl: 0,
        write_lvl: 0,
        remove_lvl: LVL_IMMORT,
        filename: "./etc/board.social",
        rnum: 0,
    },
];

pub struct BoardSystem {
    loaded: bool,
    msg_storage: [Option<Rc<RefCell<String>>>; INDEX_SIZE],
    msg_storage_taken: [bool; INDEX_SIZE],
    num_of_msgs: [usize; NUM_OF_BOARDS],
    acmd_read: usize,
    acmd_look: usize,
    acmd_examine: usize,
    acmd_write: usize,
    acmd_remove: usize,
    msg_index: [[BoardMsginfo; MAX_BOARD_MESSAGES]; NUM_OF_BOARDS],
}

impl BoardSystem {
    pub(crate) fn new() -> BoardSystem {
        let z = {
            let mut u: MaybeUninit<BoardSystem> = MaybeUninit::uninit();
            let ptr = u.as_mut_ptr();
            unsafe {
                addr_of_mut!((*ptr).loaded).write(false);
                addr_of_mut!((*ptr).acmd_read).write(0);
                addr_of_mut!((*ptr).acmd_look).write(0);
                addr_of_mut!((*ptr).acmd_examine).write(0);
                addr_of_mut!((*ptr).acmd_write).write(0);
                addr_of_mut!((*ptr).acmd_remove).write(0);
            }
            for i in 0..INDEX_SIZE {
                unsafe {
                    addr_of_mut!((*ptr).msg_storage[i]).write(None);
                    addr_of_mut!((*ptr).msg_storage_taken[i]).write(false);
                }
            }
            for i in 0..NUM_OF_BOARDS {
                unsafe {
                    addr_of_mut!((*ptr).num_of_msgs[i]).write(0);
                }
                for j in 0..MAX_BOARD_MESSAGES {
                    unsafe {
                        addr_of_mut!((*ptr).msg_index[i][j]).write(BoardMsginfo {
                            slot_num: None,
                            heading: None,
                            level: 0,
                            heading_len: 0,
                            message_len: 0,
                        })
                    }
                }
            }
            unsafe { u.assume_init() }
        };
        z
    }
}

fn find_slot(b: &mut BoardSystem) -> Option<usize> {
    for i in 0..INDEX_SIZE {
        if !b.msg_storage_taken[i] {
            b.msg_storage_taken[i] = true;
            Some(i);
        }
    }
    None
}

/* search the room ch is standing in to find which board he's looking at */
fn find_board(db: &DB, ch: &Rc<CharData>) -> Option<usize> {
    for obj in db.world.borrow()[ch.in_room() as usize]
        .contents
        .borrow()
        .iter()
    {
        for i in 0..NUM_OF_BOARDS {
            if BOARD_INFO[i].rnum == obj.get_obj_rnum() {
                return Some(i);
            }
        }
    }

    if ch.get_level() >= LVL_IMMORT as u8 {
        for obj in ch.carrying.borrow().iter() {
            for i in 0..NUM_OF_BOARDS {
                if BOARD_INFO[i].rnum == obj.get_obj_rnum() {
                    return Some(i);
                }
            }
        }
    }

    None
}

fn init_boards(db: &DB) {
    let mut fatal_error = 0;
    let mut b: &mut BoardSystem = &mut db.boards.borrow_mut();
    for i in 0..INDEX_SIZE {
        b.msg_storage[i] = None;
        b.msg_storage_taken[i] = false;
    }
    let mut board_rnum;
    for i in 0..NUM_OF_BOARDS {
        if {
            board_rnum = db.real_object(BOARD_INFO[i].vnum);
            board_rnum == NOTHING
        } {
            error!(
                "SYSERR: Fatal board error: board vnum {} does not exist!",
                BOARD_INFO[i].vnum
            );
            fatal_error = 1;
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

#[allow(unused_variables)]
pub fn gen_board(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    let cmd = cmd as usize;
    let db = &game.db;
    let board = me.downcast_ref::<Rc<ObjData>>().unwrap();
    let b: &mut BoardSystem = &mut db.boards.borrow_mut();
    if !b.loaded {
        init_boards(db);
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
        board_type = find_board(db, ch);
        board_type.is_none()
    } {
        error!("SYSERR:  degenerate board!  (what the hell...)");
        return false;
    }
    let board_type = board_type.unwrap();

    return if cmd == b.acmd_write {
        board_write_message(db, board_type, ch, argument)
    } else if cmd == b.acmd_look || cmd == b.acmd_examine {
        board_show_board(db, board_type, ch, argument, board)
    } else if cmd == b.acmd_read {
        board_display_msg(game, board_type, ch, argument, board)
    } else if cmd == b.acmd_remove {
        board_remove_msg(game, board_type, ch, argument)
    } else {
        false
    };
}

fn board_write_message(db: &DB, board_type: usize, ch: &Rc<CharData>, arg: &str) -> bool {
    let b: &mut BoardSystem = &mut db.boards.borrow_mut();
    if ch.get_level() < BOARD_INFO[board_type].write_lvl as u8 {
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
        .as_ref()
        .unwrap()
        .clone(),
        MAX_MESSAGE_LENGTH,
        board_type as i64 + BOARD_MAGIC,
    );

    b.num_of_msgs[board_type] += 1;
    return true;
}

fn board_show_board(
    db: &DB,
    board_type: usize,
    ch: &Rc<CharData>,
    arg: &str,
    board: &Rc<ObjData>,
) -> bool {
    let b: &mut BoardSystem = &mut db.boards.borrow_mut();
    if ch.desc.borrow().is_none() {
        return false;
    }
    let mut tmp = String::new();
    one_argument(arg, &mut tmp);

    if tmp.is_empty() || !isname(&tmp, board.name.borrow().as_str()) {
        return false;
    }

    if ch.get_level() < BOARD_INFO[board_type].read_lvl as u8 {
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
            for i in (0..b.num_of_msgs[board_type] - 1).rev() {
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
        } else {
            for i in 0..b.num_of_msgs[board_type] - 1 {
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
        page_string(ch.desc.borrow().as_ref(), &buf, true);
    }
    return true;
}

fn board_display_msg(
    game: &Game,
    board_type: usize,
    ch: &Rc<CharData>,
    arg: &str,
    board: &Rc<ObjData>,
) -> bool {
    let db = &game.db;
    let b: &mut BoardSystem = &mut db.boards.borrow_mut();
    let mut number = String::new();
    one_argument(arg, &mut number);
    if number.is_empty() {
        return false;
    }
    if isname(&number, &board.name.borrow()) {
        /* so "read board" works */
        return board_show_board(db, board_type, ch, arg, board);
    }
    if !is_number(&number) {
        /* read 2.mail, look 2.sword */
        return false;
    }
    let msg = number.parse::<i32>().unwrap();
    if msg == 0 {
        return false;
    }

    if ch.get_level() < BOARD_INFO[board_type].read_lvl as u8 {
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

    if b.msg_storage[msg_slot_num].is_none() {
        send_to_char(ch, "That message seems to be empty.\r\n");
        return true;
    }
    let buffer = format!(
        "Message {} : {}\r\n\r\n{}\r\n",
        msg,
        b.msg_index[board_type][ind].heading.as_ref().unwrap(),
        RefCell::borrow(b.msg_storage[msg_slot_num].as_ref().unwrap())
    );

    page_string(ch.desc.borrow().as_ref(), &buffer, true);

    true
}

fn board_remove_msg(game: &Game, board_type: usize, ch: &Rc<CharData>, arg: &str) -> bool {
    let db = &game.db;
    let b: &mut BoardSystem = &mut db.boards.borrow_mut();
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
    if ch.get_level() < BOARD_INFO[board_type].remove_lvl as u8
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
    for d in game.descriptor_list.borrow().iter() {
        if d.state() == ConPlaying
            && d.str.borrow().is_some()
            && Rc::ptr_eq(
                d.str.borrow().as_ref().unwrap(),
                &b.msg_storage[slot_num].as_ref().unwrap(),
            )
        {
            send_to_char(
                ch,
                "At least wait until the author is finished before removing it!\r\n",
            );
            return true;
        }
    }
    if !b.msg_storage[slot_num].is_none() {
        b.msg_storage[slot_num] = None;
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
    let filename = BOARD_INFO[board_type].filename;

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
            b.msg_index[board_type][i].heading_len = tmp1.as_ref().unwrap().len();
        } else {
            b.msg_index[board_type][i].heading_len = 0;
        }

        let msg_slotnum = b.msg_index[board_type][i].slot_num;
        let tmp2 = &b.msg_storage[msg_slotnum.unwrap()];

        if tmp2.is_some() {
            b.msg_index[board_type][i].message_len = RefCell::borrow(tmp2.as_ref().unwrap()).len();
        } else {
            b.msg_index[board_type][i].message_len = 0;
        }

        unsafe {
            let msginfo_slice = slice::from_raw_parts(
                &mut b.num_of_msgs[board_type] as *mut _ as *mut u8,
                mem::size_of::<BoardMsginfo>(),
            );
            fl.write_all(msginfo_slice)
                .expect("Error while number of messages in board");
        }

        if !tmp1.is_some() {
            fl.write_all(tmp1.as_ref().unwrap().as_bytes())
                .expect("writing board message heading");
        }

        if tmp2.is_some() {
            fl.write_all(RefCell::borrow(tmp2.as_ref().unwrap()).as_bytes())
                .expect("writing board message");
        }
    }
}

fn board_load_board(b: &mut BoardSystem, board_type: usize) {
    let fl = OpenOptions::new()
        .read(true)
        .open(BOARD_INFO[board_type].filename);

    if fl.is_err() {
        let err = fl.err().unwrap();
        error!("SYSERR: Error reading board {}", err);
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
        unsafe {
            let config_slice = slice::from_raw_parts_mut(
                &mut b.msg_index[board_type][i] as *mut _ as *mut u8,
                mem::size_of::<BoardMsginfo>(),
            );
            // `read_exact()` comes from `Read` impl for `&[u8]`
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
        b.msg_index[board_type][i].heading = Some(Rc::from(parse_c_string(tmp1).as_str()));
        let sn;
        if {
            sn = find_slot(b);
            sn.is_some()
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
            b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()] =
                Some(Rc::new(RefCell::new(parse_c_string(tmp2.as_slice()))));
        } else {
            b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()] = None;
        }
    }
}

// /* When shutting down, clear all boards. */
// void Board_clear_all(void)
// {
// int i;
//
// for (i = 0; i < NUM_OF_BOARDS; i++)
// Board_clear_board(i);
// }

/* Clear the in-memory structures. */
fn board_clear_board(b: &mut BoardSystem, board_type: usize) {
    for i in 0..MAX_BOARD_MESSAGES {
        if !b.msg_index[board_type][i].heading.is_none() {
            b.msg_index[board_type][i].heading = None;
        }
        if !b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()].is_none() {
            b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()] = None;
        }

        b.msg_storage_taken[b.msg_index[board_type][i].slot_num.unwrap()] = false;
        // memset((char *)&(msg_index[board_type][i]),0,sizeof(struct BoardMsginfo));
        b.msg_index[board_type][i].slot_num = None;
    }
    b.num_of_msgs[board_type] = 0;
}

/* Destroy the on-disk and in-memory board. */
fn board_reset_board(b: &mut BoardSystem, board_type: usize) {
    board_clear_board(b, board_type);
    fs::remove_file(BOARD_INFO[board_type].filename).expect("Removing board file");
}
