/* ************************************************************************
*   File: boards.rs                                     Part of CircleMUD *
*  Usage: handling of multiple bulletin boards                            *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
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

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::rc::Rc;
use std::{fs, mem, process, slice};

use log::error;

use crate::db::{parse_c_string, DB};
use crate::depot::{Depot, DepotId};
use crate::handler::isname;
use crate::interpreter::{delete_doubledollar, find_command, is_number, one_argument};
use crate::modify::page_string;
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    MeRef, ObjRnum, ObjVnum, LVL_FREEZE, LVL_GOD, LVL_GRGOD, LVL_IMMORT, LVL_IMPL,
    NOTHING,
};
use crate::util::{ctime, time_now};
use crate::{ act, send_to_char, CharData, Game, ObjData, TextData, TO_ROOM};

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
    rnum: ObjRnum,
    /* rnum of this board */
}

/* Board appearance order. */
const NEWEST_AT_TOP: bool = false;

pub struct BoardSystem {
    loaded: bool,
    msg_storage: [DepotId; INDEX_SIZE],
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
    pub(crate) fn new(texts: &mut  Depot<TextData>,) -> BoardSystem {
        BoardSystem {
            loaded: false,
            msg_storage: [(); INDEX_SIZE].map(|_| texts.add_text("".into())),
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
                    rnum:0,
                },
                BoardInfoType {
                    vnum: 3096,
                    read_lvl: 0,
                    write_lvl: 0,
                    remove_lvl: LVL_IMMORT,
                    filename: "./etc/board.social",
                    rnum: 0,
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
fn find_board(chars: &Depot<CharData>, db: &DB,objs: &Depot<ObjData>,  chid: DepotId) -> Option<usize> {
    let ch = chars.get(chid);

    for oid in db.world[ch.in_room() as usize]
        .contents
        .iter()
    {
        for i in 0..NUM_OF_BOARDS {
            if db.boards.boardinfo[i].rnum == objs.get(*oid).get_obj_rnum() {
                return Some(i);
            }
        }
    }

    if ch.get_level() >= LVL_IMMORT as u8 {
        for oid in ch.carrying.iter() {
            for i in 0..NUM_OF_BOARDS {
                if db.boards.boardinfo[i].rnum == objs.get(*oid).get_obj_rnum() {
                    return Some(i);
                }
            }
        }
    }

    None
}

fn init_boards(db: &mut DB, texts: &mut  Depot<TextData>,) {
    let mut fatal_error = 0;
    for i in 0..INDEX_SIZE {
        texts.get_mut(db.boards.msg_storage[i]).text.clear();
        db.boards.msg_storage_taken[i] = false;
    }
    for i in 0..NUM_OF_BOARDS {
        let rnum;
        if {
            rnum = db.real_object(db.boards.boardinfo[i].vnum);
            rnum == NOTHING
        } {
            error!(
                "SYSERR: Fatal board error: board vnum {} does not exist!",
                db.boards.boardinfo[i].vnum
            );
            fatal_error = 1;
        } else {
            db.boards.boardinfo[i].rnum = rnum;
        }
        db.boards.num_of_msgs[i] = 0;
        for j in 0..MAX_BOARD_MESSAGES {
            db.boards.msg_index[i][j].slot_num = None;
        }
        board_load_board(&mut db.boards, texts, i);
    }

    db.boards.acmd_read = find_command("read").unwrap();
    db.boards.acmd_write = find_command("write").unwrap();
    db.boards.acmd_remove = find_command("remove").unwrap();
    db.boards.acmd_look = find_command("look").unwrap();
    db.boards.acmd_examine = find_command("examine").unwrap();

    if fatal_error != 0 {
        process::exit(1);
    }
}

pub fn gen_board(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    me: MeRef,
    cmd: i32,
    argument: &str,
) -> bool {
    let cmd = cmd as usize;
    let board;
    match me {
        MeRef::Obj(me_obj) => {board = me_obj},
        _ => panic!("Unexpected MeRef type in receptionist"),
    }
    if !db.boards.loaded {
        init_boards( db, texts);
        db.boards.loaded = true;
    }
    let ch = chars.get(chid);
    if ch.desc.is_none() {
        return false;
    }

    if cmd != db.boards.acmd_write
        && cmd != db.boards.acmd_look
        && cmd != db.boards.acmd_examine
        && cmd != db.boards.acmd_read
        && cmd != db.boards.acmd_remove
    {
        return false;
    }

    let board_type;
    if {
        board_type = find_board(chars, &db,objs, chid);
        board_type.is_none()
    } {
        error!("SYSERR:  degenerate board!  (what the hell...)");
        return false;
    }
    let board_type = board_type.unwrap();

    return if cmd == db.boards.acmd_write {
        board_write_message(game, chars, db,board_type, chid, argument)
    } else if cmd == db.boards.acmd_look || cmd == db.boards.acmd_examine {
        board_show_board(game, chars, db,objs,board_type, chid, argument, board)
    } else if cmd == db.boards.acmd_read {
        board_display_msg(game, chars, db, texts, objs,board_type, chid, argument, board)
    } else if cmd == db.boards.acmd_remove {
        board_remove_msg( game, chars, db, texts, board_type, chid, argument)
    } else {
        false
    };
}

fn board_write_message(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,
    board_type: usize,
    chid: DepotId,
    arg: &str,
) -> bool {
    let ch = chars.get(chid);

    if ch.get_level() < db.boards.boardinfo[board_type].write_lvl as u8 {
        send_to_char(&mut game.descriptors, ch, "You are not holy enough to write on this board.\r\n");
        return true;
    }
    if db.boards.num_of_msgs[board_type] >= MAX_BOARD_MESSAGES {
        send_to_char(&mut game.descriptors, ch, "The board is full.\r\n");
        return true;
    }
    let slot;
    if {
        slot = find_slot(&mut db.boards);
        slot.is_none()
    } {
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, "The board is malfunctioning - sorry.\r\n");
        error!("SYSERR: Board: failed to find empty slot on write.");
        return false;
    }

    db.boards.msg_index[board_type][db.boards.num_of_msgs[board_type]].slot_num = slot;
    /* skip blanks */
    let mut arg = arg.trim_start().to_string();
    delete_doubledollar(&mut arg);

    /* JE 27 Oct 95 - Truncate headline at 80 chars if it's longer than that */
    arg.truncate(80);

    if arg.is_empty() {
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, "We must have a headline!\r\n");
        return true;
    }
    let ct = time_now();
    let tmstr = ctime(ct);
    let ch = chars.get(chid);
    let buf2 = format!("({})", ch.get_name());
    let buf = format!("{:10} {:12} :: {}", tmstr, buf2, arg);
    db.boards.msg_index[board_type][db.boards.num_of_msgs[board_type]].heading = Some(Rc::from(buf.as_str()));
    let ch = chars.get(chid);
    db.boards.msg_index[board_type][db.boards.num_of_msgs[board_type]].level = ch.get_level() as i32;
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch,
        "Write your message.  Terminate with a @ on a new line.\r\n\r\n",
    );
    act(&mut game.descriptors, chars, db,
        "$n starts to write a message.",
        true,
        Some(ch),
        None,
        None,
        TO_ROOM,
    );
    let ch = chars.get(chid);
    let desc_id = ch.desc.unwrap();
    let desc = game.desc_mut(desc_id);
    desc.string_write(
        chars, 
        db.boards.msg_storage[db.boards.msg_index[board_type][db.boards.num_of_msgs[board_type]]
            .slot_num
            .unwrap()],
        MAX_MESSAGE_LENGTH,
        board_type as i64 + BOARD_MAGIC,
    );

    db.boards.num_of_msgs[board_type] += 1;
    return true;
}

fn board_show_board(
    game: &mut Game, chars: &Depot<CharData>, db: &mut DB,objs: & Depot<ObjData>, 
    board_type: usize,
    chid: DepotId,
    arg: &str,
    board_id: DepotId,
) -> bool {
    let ch = chars.get(chid);

    if ch.desc.is_none() {
        return false;
    }
    let mut tmp = String::new();
    one_argument(arg, &mut tmp);

    if tmp.is_empty() || !isname(&tmp, objs.get(board_id).name.as_ref()) {
        return false;
    }

    if ch.get_level() < db.boards.boardinfo[board_type].read_lvl as u8 {
        send_to_char(&mut game.descriptors, ch, "You try but fail to understand the holy words.\r\n");
        return true;
    }
    act(&mut game.descriptors, chars, db,"$n studies the board.", true, Some(ch), None, None, TO_ROOM);

    if db.boards.num_of_msgs[board_type] == 0 {
        send_to_char(&mut game.descriptors, ch, "This is a bulletin board.  Usage: READ/REMOVE <messg #>, WRITE <header>.\r\nThe board is empty.\r\n");
    } else {
        let mut buf = format!(
            "This is a bulletin board.  Usage: READ/REMOVE <messg #>, WRITE <header>.\r\n\
You will need to look at the board to save your message.\r\n\
There are {} messages on the board.\r\n",
db.boards.num_of_msgs[board_type]
        );
        if NEWEST_AT_TOP {
            for i in (0..db.boards.num_of_msgs[board_type]).rev() {
                if db.boards.msg_index[board_type][i].heading.clone().is_none() {
                    error!("SYSERR: Board {} is fubar'd.", board_type);
                    send_to_char(&mut game.descriptors, ch, "Sorry, the board isn't working.\r\n");
                    return true;
                }

                buf.push_str(
                    format!(
                        "{:2} : {}\r\n",
                        i + 1,
                        db.boards.msg_index[board_type][i].heading.as_ref().unwrap()
                    )
                    .as_str(),
                );
            }
        } else {
            for i in 0..db.boards.num_of_msgs[board_type] {
                if db.boards.msg_index[board_type][i].heading.is_none() {
                    error!("SYSERR: Board {} is fubar'd.", board_type);
                    send_to_char(&mut game.descriptors, ch, "Sorry, the board isn't working.\r\n");
                    return true;
                }

                buf.push_str(
                    format!(
                        "{:2} : {}\r\n",
                        i + 1,
                        db.boards.msg_index[board_type][i].heading.as_ref().unwrap()
                    )
                    .as_str(),
                );
            }
        }
        let ch = chars.get(chid);
        let d_id = ch.desc.unwrap();
        page_string(&mut game.descriptors, chars,  d_id, &buf, true);
    }
    return true;
}

fn board_display_msg(
    game: &mut Game, chars: &Depot<CharData>, db: &mut DB, texts: &Depot<TextData>,objs: & Depot<ObjData>, 
    board_type: usize,
    chid: DepotId,
    arg: &str,
    board_id: DepotId,
) -> bool {
    let ch = chars.get(chid);

    let mut number = String::new();
    one_argument(arg, &mut number);
    if number.is_empty() {
        return false;
    }
    if isname(&number, &objs.get(board_id).name) {
        /* so "read board" works */
        return board_show_board(game,  chars, db,objs,board_type, chid, arg, board_id);
    }
    if !is_number(&number) {
        /* read 2.mail, look 2.sword */
        return false;
    }
    let msg = number.parse::<i32>().unwrap();
    if msg == 0 {
        return false;
    }

    if ch.get_level() < db.boards.boardinfo[board_type].read_lvl as u8 {
        send_to_char(&mut game.descriptors, ch, "You try but fail to understand the holy words.\r\n");
        return true;
    }
    if db.boards.num_of_msgs[board_type] == 0 {
        send_to_char(&mut game.descriptors, ch, "The board is empty!\r\n");
        return true;
    }
    if msg < 1 || msg > db.boards.num_of_msgs[board_type] as i32 {
        send_to_char(&mut game.descriptors, ch, "That message exists only in your imagination.\r\n");
        return true;
    }
    let ind;
    if NEWEST_AT_TOP {
        ind = db.boards.num_of_msgs[board_type] - msg as usize;
    } else {
        ind = msg as usize - 1;
    }
    let msg_slot_numo = db.boards.msg_index[board_type][ind].slot_num;
    let mut msg_slot_num = 0;
    if msg_slot_numo.is_none() || {
        msg_slot_num = msg_slot_numo.unwrap();
        msg_slot_num >= INDEX_SIZE
    } {
        send_to_char(&mut game.descriptors, ch, "Sorry, the board is not working.\r\n");
        let ch = chars.get(chid);
        error!(
            "SYSERR: Board is screwed up. (Room #{})",
            db.get_room_vnum(ch.in_room())
        );
        return true;
    }

    if db.boards.msg_index[board_type][ind].heading.is_none() {
        send_to_char(&mut game.descriptors, ch, "That message appears to be screwed up.\r\n");
        return true;
    }

    let text = &texts.get(db.boards.msg_storage[msg_slot_num]).text;
    if text.is_empty() {
        send_to_char(&mut game.descriptors, ch, "That message seems to be empty.\r\n");
        return true;
    }
    let buffer = format!(
        "Message {} : {}\r\n\r\n{}\r\n",
        msg,
        db.boards.msg_index[board_type][ind].heading.as_ref().unwrap(),
        text
    );

    let d_id = ch.desc.unwrap();
    page_string(&mut game.descriptors, chars,  d_id , &buffer, true);

    true
}

fn board_remove_msg(
    game: &mut Game, chars: &Depot<CharData>, db: &mut DB,  texts: &mut Depot<TextData>,
    board_type: usize,
    chid: DepotId,
    arg: &str,
) -> bool {
    let ch = chars.get(chid);
    let mut number = String::new();
    one_argument(arg, &mut number);

    if number.is_empty() || !is_number(&number) {
        return false;
    }
    let msg = number.parse::<i32>().unwrap();
    if msg == 0 {
        return false;
    }

    if db.boards.num_of_msgs[board_type] == 0 {
        send_to_char(&mut game.descriptors, ch, "The board is empty!\r\n");
        return true;
    }
    if msg < 1 || msg as usize > db.boards.num_of_msgs[board_type] {
        send_to_char(&mut game.descriptors, ch, "That message exists only in your imagination.\r\n");
        return true;
    }
    let ind;
    if NEWEST_AT_TOP {
        ind = db.boards.num_of_msgs[board_type] - msg as usize;
    } else {
        ind = msg as usize - 1;
    }

    if db.boards.msg_index[board_type][ind].heading.is_none() {
        send_to_char(&mut game.descriptors, ch, "That message appears to be screwed up.\r\n");
        return true;
    }
    let buf = format!("({})", ch.get_name());
    if ch.get_level() < db.boards.boardinfo[board_type].remove_lvl as u8
        && !db.boards.msg_index[board_type][ind]
            .heading
            .as_ref()
            .unwrap()
            .contains(&buf)
    {
        send_to_char(&mut game.descriptors, ch,
            "You are not holy enough to remove other people's messages.\r\n",
        );
        return true;
    }
    if ch.get_level() < db.boards.msg_index[board_type][ind].level as u8 {
        send_to_char(&mut game.descriptors, ch, "You can't remove a message holier than yourself.\r\n");
        return true;
    }
    let slot_numo = db.boards.msg_index[board_type][ind].slot_num;
    let mut slot_num = 0;
    if slot_numo.is_none() || {
        slot_num = slot_numo.unwrap();
        slot_num >= INDEX_SIZE
    } {
        send_to_char(&mut game.descriptors, ch, "That message is majorly screwed up.\r\n");
        let ch = chars.get(chid);
        error!(
            "SYSERR: The board is seriously screwed up. (Room #{})",
            db.get_room_vnum(ch.in_room())
        );
        return true;
    }
    for d_id in game.descriptor_list.clone() {
        let d = game.desc(d_id);
        if d.state() == ConPlaying
            && d.str.is_some()
            && d.str.unwrap() == db.boards.msg_storage[slot_num]
        {
            send_to_char(&mut game.descriptors, ch,
                "At least wait until the author is finished before removing it!\r\n",
            );
            return true;
        }
    }
    let text = &mut (texts.get_mut(db.boards.msg_storage[slot_num]).text);
    if !text.is_empty() {
        text.clear();
    }
    db.boards.msg_storage_taken[slot_num] = false;
    if !db.boards.msg_index[board_type][ind].heading.is_none() {
        db.boards.msg_index[board_type][ind].heading = None;
    }

    for i in ind..db.boards.num_of_msgs[board_type] - 1 {
        db.boards.msg_index[board_type][i].heading = db.boards.msg_index[board_type][i + 1].heading.clone();
        db.boards.msg_index[board_type][i].slot_num = db.boards.msg_index[board_type][i + 1].slot_num;
        db.boards.msg_index[board_type][i].level = db.boards.msg_index[board_type][i + 1].level;
    }
    db.boards.msg_index[board_type][db.boards.num_of_msgs[board_type] - 1].heading = None;
    db.boards.msg_index[board_type][db.boards.num_of_msgs[board_type] - 1].slot_num = None;
    db.boards.msg_index[board_type][db.boards.num_of_msgs[board_type] - 1].level = 0;
    db.boards.num_of_msgs[board_type] -= 1;
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch, "Message removed.\r\n");
    let buf = format!("$n just removed message {}.", msg);
    act(&mut game.descriptors, chars, db,&buf, false, Some(ch), None, None, TO_ROOM);
    board_save_board(&mut db.boards, texts, board_type);

    return true;
}

pub fn board_save_board(b: &mut BoardSystem, texts: &Depot<TextData>, board_type: usize) {
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
        let tmp2 = b.msg_storage[msg_slotnum];
        let text = &texts.get(tmp2).text;

        if !text.is_empty() {
            b.msg_index[board_type][i].message_len = text.as_bytes().len();
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

        if !text.is_empty() {
            fl.write_all(text.as_bytes())
                .expect("writing board message");
        }
    }
}

fn board_load_board(b: &mut BoardSystem,texts: &mut Depot<TextData>, board_type: usize) {
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
        // `read_exact(db,)` comes from `Read` impl for `&[u8]`
        let r = fl.read_exact(config_slice);
        if r.is_err() {
            let r = r.err().unwrap();
            error!("[SYSERR] Error while board file {} {r}", board_type);
            board_reset_board(b, texts, board_type);
            return;
        }
    }

    if b.num_of_msgs[board_type] < 1 || b.num_of_msgs[board_type] > MAX_BOARD_MESSAGES {
        error!("SYSERR: Board file {} corrupt.  Resetting.", board_type);
        board_reset_board(b, texts, board_type);
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
                board_reset_board(b, texts, board_type);
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
            board_reset_board(b, texts, board_type);
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
            board_reset_board(b, texts, board_type);
            return;
        }
        b.msg_index[board_type][i].slot_num = sn;
        let len2;
        let text = &mut texts.get_mut(b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()]).text;
        if {
            len2 = b.msg_index[board_type][i].message_len;
            len2 > 0
        } {
            let mut tmp2 = vec![0 as u8; len2];
            fl.read_exact(tmp2.as_mut_slice())
                .expect("Error reading board file message string");
            *text=parse_c_string(tmp2.as_slice());
        } else {
            text.clear();
        }
    }
}

/* When shutting down, clear all boards. */
pub fn board_clear_all(b: &mut BoardSystem,texts: &mut Depot<TextData>, ) {
    for i in 0..NUM_OF_BOARDS {
        board_clear_board(b, texts, i);
    }
}

/* Clear the in-memory structures. */
fn board_clear_board(b: &mut BoardSystem,texts: &mut Depot<TextData>,  board_type: usize) {
    for i in 0..MAX_BOARD_MESSAGES {
        if !b.msg_index[board_type][i].heading.is_none() {
            b.msg_index[board_type][i].heading = None;
        }
        if b.msg_index[board_type][i].slot_num.is_some()
            && !texts.get(b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()]).text
                .is_empty()
        {
            texts.get_mut(b.msg_storage[b.msg_index[board_type][i].slot_num.unwrap()]).text.clear();
                b.msg_storage_taken[b.msg_index[board_type][i].slot_num.unwrap()] = false;
        }

        b.msg_index[board_type][i].slot_num = None;
    }
    b.num_of_msgs[board_type] = 0;
}

/* Destroy the on-disk and in-memory board. */
fn board_reset_board(b: &mut BoardSystem, texts: &mut Depot<TextData>, board_type: usize) {
    board_clear_board(b, texts, board_type);
    fs::remove_file(b.boardinfo[board_type].filename).expect("Removing board file");
}
