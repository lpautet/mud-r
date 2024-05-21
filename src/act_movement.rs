/* ************************************************************************
*   File: act.movement.rs                               Part of CircleMUD *
*  Usage: movement commands, door handling, & sleep/rest/etc state        *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::borrow::Borrow;
use std::rc::Rc;

use crate::act_informative::look_at_room;
use crate::act_item::find_eq_pos;
use crate::config::{NOPERSON, OK, TUNNEL_SIZE};
use crate::constants::{DEX_APP_SKILL, DIRS, MOVEMENT_LOSS, REV_DIR};
use crate::db::DB;
use crate::handler::{fname, isname, FIND_CHAR_ROOM, FIND_OBJ_INV, FIND_OBJ_ROOM};
use crate::house::house_can_enter;
use crate::interpreter::{
    one_argument, search_block, special, two_arguments, SCMD_CLOSE, SCMD_LOCK, SCMD_OPEN,
    SCMD_PICK, SCMD_UNLOCK,
};
use crate::spells::SKILL_PICK_LOCK;
use crate::structs::{
    CharData, ObjData, ObjVnum, RoomDirectionData, RoomRnum, AFF_CHARM, AFF_GROUP, AFF_SLEEP,
    AFF_SNEAK, AFF_WATERWALK, CONT_CLOSEABLE, CONT_CLOSED, CONT_LOCKED, CONT_PICKPROOF, EX_CLOSED,
    EX_ISDOOR, EX_LOCKED, EX_PICKPROOF, ITEM_BOAT, ITEM_CONTAINER, LVL_GOD, LVL_GRGOD, LVL_IMMORT,
    NOTHING, NOWHERE, NUM_OF_DIRS, NUM_WEARS, POS_FIGHTING, POS_RESTING, POS_SITTING, POS_SLEEPING,
    POS_STANDING, ROOM_ATRIUM, ROOM_DEATH, ROOM_GODROOM, ROOM_INDOORS, ROOM_TUNNEL,
    SECT_WATER_NOSWIM, WEAR_HOLD,
};
use crate::util::{add_follower, circle_follow, log_death_trap, num_pc_in_room, rand_number};
use crate::{an, is_set, send_to_char, Game, TO_CHAR, TO_ROOM, TO_SLEEP, TO_VICT};

/* simple function to determine if char can walk on water */
fn has_boat(ch: &Rc<CharData>) -> bool {
    if ch.get_level() > LVL_IMMORT as u8 {
        return true;
    }

    if ch.aff_flagged(AFF_WATERWALK) {
        return true;
    }

    /* non-wearable boats in inventory will do it */

    for obj in ch.carrying.borrow().iter() {
        if obj.get_obj_type() == ITEM_BOAT && (find_eq_pos(ch, obj, "") < 0) {
            return true;
        }
    }

    /* and any boat you're wearing will do it too */

    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some() && ch.get_eq(i).as_ref().unwrap().get_obj_type() == ITEM_BOAT {
            return true;
        }
    }

    false
}

/* do_simple_move assumes
 *    1. That there is no master and no followers.
 *    2. That the direction exists.
 *
 *   Returns :
 *   1 : If succes.
 *   0 : If fail
 */
pub fn perform_move(
    game: &mut Game,
    ch: &Rc<CharData>,
    dir: i32,
    need_specials_check: bool,
) -> bool {
    if dir < 0 || dir >= NUM_OF_DIRS as i32 || ch.fighting().is_some() {
        return false;
    } else if game.db.exit(ch, dir as usize).is_none()
        || game
            .db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .to_room
            .get()
            == NOWHERE
    {
        send_to_char(ch, "Alas, you cannot go that way...\r\n");
    } else if game
        .db
        .exit(ch, dir as usize)
        .as_ref()
        .unwrap()
        .exit_flagged(EX_CLOSED)
    {
        if !game
            .db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .keyword
            .is_empty()
        {
            send_to_char(
                ch,
                format!(
                    "The {} seems to be closed.\r\n",
                    fname(
                        game.db
                            .exit(ch, dir as usize)
                            .as_ref()
                            .unwrap()
                            .keyword
                            .as_str()
                    )
                )
                .as_str(),
            );
        } else {
            send_to_char(ch, "It seems to be closed.\r\n");
        }
    } else {
        if ch.followers.borrow().is_empty() {
            return do_simple_move(game, ch, dir, need_specials_check);
        }

        let was_in = ch.in_room();
        if !do_simple_move(game, ch, dir, need_specials_check) {
            return false;
        }

        for k in ch.followers.borrow().iter() {
            if k.follower.in_room() == was_in && k.follower.get_pos() >= POS_STANDING {
                game.db.act(
                    "You follow $N.\r\n",
                    false,
                    Some(&k.follower),
                    None,
                    Some(ch),
                    TO_CHAR,
                );
                perform_move(game, &k.follower, dir, true);
            }
        }
        return true;
    }
    return false;
}

pub fn do_simple_move(
    game: &mut Game,
    ch: &Rc<CharData>,
    dir: i32,
    need_specials_check: bool,
) -> bool {
    let was_in;
    let need_movement;

    /*
     * Check for special routines (North is 1 in command list, but 0 here) Note
     * -- only check if following; this avoids 'double spec-proc' bug
     */
    if need_specials_check && special(game, ch, dir + 1, "") {
        return false;
    }

    let db = &game.db;

    /* charmed? */
    if ch.aff_flagged(AFF_CHARM)
        && ch.master.borrow().is_some()
        && ch.in_room() == ch.master.borrow().as_ref().unwrap().in_room()
    {
        send_to_char(ch, "The thought of leaving your master makes you weep.\r\n");
        db.act(
            "$n bursts into tears.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        return false;
    }

    /* if this room or the one we're going to needs a boat, check for one */
    if (db.sect(ch.in_room()) == SECT_WATER_NOSWIM)
        || (db.sect(db.exit(ch, dir as usize).as_ref().unwrap().to_room.get()) == SECT_WATER_NOSWIM)
    {
        if !has_boat(ch) {
            send_to_char(ch, "You need a boat to go there.\r\n");
            return false;
        }
    }

    /* move points needed is avg. move loss for src and destination sect type */
    need_movement = (MOVEMENT_LOSS[db.sect(ch.in_room()) as usize]
        + MOVEMENT_LOSS
            [db.sect(db.exit(ch, dir as usize).as_ref().unwrap().to_room.get()) as usize])
        / 2;

    if ch.get_move() < need_movement as i16 && !ch.is_npc() {
        if need_specials_check && ch.master.borrow().is_some() {
            send_to_char(ch, "You are too exhausted to follow.\r\n");
        } else {
            send_to_char(ch, "You are too exhausted.\r\n");
        }

        return false;
    }

    if db.room_flagged(ch.in_room(), ROOM_ATRIUM) {
        if !house_can_enter(
            db,
            ch,
            db.get_room_vnum(db.exit(ch, dir as usize).as_ref().unwrap().to_room.get()),
        ) {
            send_to_char(ch, "That's private property -- no trespassing!\r\n");
            return false;
        }
    }
    if db.room_flagged(
        db.exit(ch, dir as usize).as_ref().unwrap().to_room.get(),
        ROOM_TUNNEL,
    ) && num_pc_in_room(
        db.world.borrow()[db.exit(ch, dir as usize).as_ref().unwrap().to_room.get() as usize]
            .borrow(),
    ) >= TUNNEL_SIZE
    {
        if TUNNEL_SIZE > 1 {
            send_to_char(ch, "There isn't enough room for you to go there!\r\n");
        } else {
            send_to_char(
                ch,
                "There isn't enough room there for more than one person!\r\n",
            );
        }
        return false;
    }
    /* Mortals and low level gods cannot enter greater god rooms. */
    if db.room_flagged(
        db.exit(ch, dir as usize).as_ref().unwrap().to_room.get(),
        ROOM_GODROOM,
    ) && ch.get_level() < LVL_GRGOD as u8
    {
        send_to_char(ch, "You aren't godly enough to use that room!\r\n");
        return false;
    }

    /* Now we know we're allow to go into the room. */
    if ch.get_level() < LVL_IMMORT as u8 && !ch.is_npc() {
        ch.incr_move(-need_movement as i16);
    }

    if !ch.aff_flagged(AFF_SNEAK) {
        let buf2 = format!("$n leaves {}.", DIRS[dir as usize]);
        db.act(buf2.as_str(), true, Some(ch), None, None, TO_ROOM);
    }
    was_in = ch.in_room();
    db.char_from_room(ch);
    db.char_to_room(
        ch,
        db.world.borrow()[was_in as usize].dir_option[dir as usize]
            .as_ref()
            .unwrap()
            .to_room
            .get(),
    );

    if !ch.aff_flagged(AFF_SNEAK) {
        db.act("$n has arrived.", true, Some(ch), None, None, TO_ROOM);
    }

    if ch.desc.borrow().is_some() {
        look_at_room(db, ch, false);
    }

    if db.room_flagged(ch.in_room(), ROOM_DEATH) && ch.get_level() < LVL_IMMORT as u8 {
        log_death_trap(game, ch);
        db.death_cry(ch);
        db.extract_char(ch);
        return false;
    }
    return true;
}

pub fn do_move(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, subcmd: i32) {
    /*
     * This is basically a mapping of cmd numbers to perform_move indices.
     * It cannot be done in perform_move because perform_move is called
     * by other functions which do not require the remapping.
     */
    perform_move(game, ch, subcmd - 1, false);
}

fn find_door(db: &DB, ch: &Rc<CharData>, type_: &str, dir: &str, cmdname: &str) -> Option<i32> {
    let dooro;

    if !dir.is_empty() {
        /* a direction was specified */
        if {
            dooro = search_block(dir, &DIRS, false);
            dooro.is_none()
        } {
            /* Partial Match */
            send_to_char(ch, "That's not a direction.\r\n");
            return None;
        }
        let door = dooro.unwrap();
        if db.exit(ch, door).is_some() {
            /* Braces added according to indent. -gg */
            if !db.exit(ch, door).as_ref().unwrap().keyword.is_empty() {
                if isname(
                    type_,
                    &db.exit(ch, door)
                        .as_ref()
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .keyword,
                ) {
                    return Some(door as i32);
                } else {
                    send_to_char(ch, format!("I see no {} there.\r\n", type_).as_str());
                    return None;
                }
            } else {
                return Some(door as i32);
            }
        } else {
            send_to_char(
                ch,
                format!(
                    "I really don't see how you can {} anything there.\r\n",
                    cmdname
                )
                .as_str(),
            );
            return None;
        }
    } else {
        /* try to locate the keyword */
        if type_.is_empty() {
            send_to_char(
                ch,
                format!("What is it you want to {}?\r\n", cmdname).as_str(),
            );
            return None;
        }
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some() {
                if !db.exit(ch, door).as_ref().unwrap().keyword.is_empty() {
                    if isname(type_, &db.exit(ch, door).as_ref().unwrap().keyword) {
                        return Some(door as i32);
                    }
                }
            }
        }

        send_to_char(
            ch,
            format!(
                "There doesn't seem to be {} {} here.\r\n",
                an!(type_),
                type_
            )
            .as_str(),
        );
        return None;
    }
}

fn has_key(db: &DB, ch: &Rc<CharData>, key: ObjVnum) -> bool {
    for o in ch.carrying.borrow().iter() {
        if db.get_obj_vnum(o) == key {
            return true;
        }
    }

    if ch.get_eq(WEAR_HOLD as i8).borrow().is_some() {
        if db.get_obj_vnum(ch.get_eq(WEAR_HOLD as i8).borrow().as_ref().unwrap()) == key {
            return true;
        }
    }
    false
}

const NEED_OPEN: i32 = 1 << 0;
const NEED_CLOSED: i32 = 1 << 1;
const NEED_UNLOCKED: i32 = 1 << 2;
const NEED_LOCKED: i32 = 1 << 3;

const CMD_DOOR: [&str; 5] = ["open", "close", "unlock", "lock", "pick"];

const FLAGS_DOOR: [i32; 5] = [
    NEED_CLOSED | NEED_UNLOCKED,
    NEED_OPEN,
    NEED_CLOSED | NEED_LOCKED,
    NEED_CLOSED | NEED_UNLOCKED,
    NEED_CLOSED | NEED_LOCKED,
];

fn exitn(db: &DB, room: RoomRnum, door: usize) -> Rc<RoomDirectionData> {
    db.world.borrow()[room as usize].dir_option[door]
        .as_ref()
        .unwrap()
        .clone()
}

fn open_door(db: &DB, room: RoomRnum, obj: Option<&Rc<ObjData>>, door: Option<usize>) {
    if obj.is_some() {
        obj.as_ref().unwrap().remove_objval_bit(1, CONT_CLOSED);
    } else {
        exitn(db, room, door.unwrap())
            .exit_info
            .set(exitn(db, room, door.unwrap()).exit_info.get() & !EX_CLOSED);
    }
}

fn close_door(db: &DB, room: RoomRnum, obj: Option<&Rc<ObjData>>, door: Option<usize>) {
    if obj.is_some() {
        obj.as_ref().unwrap().set_objval_bit(1, CONT_CLOSED);
    } else {
        exitn(db, room, door.unwrap())
            .exit_info
            .set(exitn(db, room, door.unwrap()).exit_info.get() | EX_CLOSED);
    }
}

fn lock_door(db: &DB, room: RoomRnum, obj: Option<&Rc<ObjData>>, door: Option<usize>) {
    if obj.is_some() {
        obj.as_ref().unwrap().set_objval_bit(1, CONT_LOCKED);
    } else {
        exitn(db, room, door.unwrap())
            .exit_info
            .set(exitn(db, room, door.unwrap()).exit_info.get() | EX_LOCKED);
    }
}

fn unlock_door(db: &DB, room: RoomRnum, obj: Option<&Rc<ObjData>>, door: Option<usize>) {
    if obj.is_some() {
        obj.as_ref().unwrap().remove_objval_bit(1, CONT_LOCKED);
    } else {
        exitn(db, room, door.unwrap())
            .exit_info
            .set(exitn(db, room, door.unwrap()).exit_info.get() & !EX_LOCKED);
    }
}

fn togle_lock(db: &DB, room: RoomRnum, obj: Option<&Rc<ObjData>>, door: Option<usize>) {
    if obj.is_some() {
        obj.as_ref()
            .unwrap()
            .set_obj_val(1, obj.as_ref().unwrap().get_obj_val(1) ^ CONT_LOCKED);
    } else {
        exitn(db, room, door.unwrap())
            .exit_info
            .set(exitn(db, room, door.unwrap()).exit_info.get() ^ EX_LOCKED);
    }
}

fn do_doorcmd(
    db: &DB,
    ch: &Rc<CharData>,
    obj: Option<&Rc<ObjData>>,
    door: Option<usize>,
    scmd: i32,
) {
    let mut buf;

    let mut other_room = NOWHERE;

    let mut back = None;
    let w = db.world.borrow();

    buf = format!("$n {}s ", CMD_DOOR[scmd as usize]);
    if obj.is_none() && {
        other_room = db.exit(ch, door.unwrap()).as_ref().unwrap().to_room.get();
        other_room != NOWHERE
    } {
        if {
            back = w[other_room as usize].dir_option[REV_DIR[door.unwrap()] as usize].as_ref();
            back.is_some()
        } {
            if back.unwrap().to_room != ch.in_room {
                back = None;
            }
        }
    }

    match scmd {
        SCMD_OPEN => {
            open_door(db, ch.in_room(), obj, door);
            if back.is_some() {
                open_door(
                    db,
                    other_room,
                    obj,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            send_to_char(ch, OK);
        }
        SCMD_CLOSE => {
            close_door(db, ch.in_room(), obj, door);
            if back.is_some() {
                close_door(
                    db,
                    other_room,
                    obj,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            send_to_char(ch, OK);
        }
        SCMD_LOCK => {
            lock_door(db, ch.in_room(), obj, door);
            if back.is_some() {
                lock_door(
                    db,
                    other_room,
                    obj,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            send_to_char(ch, OK);
        }
        SCMD_UNLOCK => {
            unlock_door(db, ch.in_room(), obj, door);
            if back.is_some() {
                unlock_door(
                    db,
                    other_room,
                    obj,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            send_to_char(ch, OK);
        }

        SCMD_PICK => {
            togle_lock(db, ch.in_room(), obj, door);
            if (&back).is_some() {
                togle_lock(
                    db,
                    other_room,
                    obj,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            send_to_char(ch, "The lock quickly yields to your skills.\r\n");
            buf = "$n skillfully picks the lock on ".to_string();
        }
        _ => {}
    }

    /* Notify the room. */
    buf.push_str(
        format!(
            "{}{}.",
            if obj.is_some() { "" } else { "the " },
            if obj.is_some() {
                "$p"
            } else {
                if !db
                    .exit(ch, door.unwrap())
                    .as_ref()
                    .unwrap()
                    .keyword
                    .is_empty()
                {
                    "$F"
                } else {
                    "door"
                }
            }
        )
        .as_str(),
    );
    if obj.is_none() || obj.as_ref().unwrap().in_room() != NOWHERE {
        let x = db.exit(ch, door.unwrap());
        db.act(
            &buf,
            false,
            Some(ch),
            obj,
            if obj.is_some() {
                None
            } else {
                Some(&x.as_ref().unwrap().keyword)
            },
            TO_ROOM,
        );
    }

    /* Notify the other room */
    if back.is_some() && (scmd == SCMD_OPEN || scmd == SCMD_CLOSE) {
        let x = fname(&back.as_ref().unwrap().keyword);
        db.send_to_room(
            db.exit(ch, door.unwrap()).as_ref().unwrap().to_room.get(),
            format!(
                "The {} is {}{} from the other side.",
                if !back.as_ref().unwrap().keyword.is_empty() {
                    x.as_ref()
                } else {
                    "door"
                },
                CMD_DOOR[scmd as usize],
                if scmd == SCMD_CLOSE { "d" } else { "ed" }
            )
            .as_str(),
        );
    }
}

fn ok_pick(ch: &Rc<CharData>, keynum: ObjVnum, pickproof: bool, scmd: i32) -> bool {
    if scmd != SCMD_PICK {
        return true;
    }

    let percent = rand_number(1, 101);
    let skill_lvl =
        ch.get_skill(SKILL_PICK_LOCK) as i16 + DEX_APP_SKILL[ch.get_dex() as usize].p_locks;

    if keynum == NOTHING {
        send_to_char(ch, "Odd - you can't seem to find a keyhole.\r\n");
    } else if pickproof {
        send_to_char(ch, "It resists your attempts to pick it.\r\n");
    } else if percent > skill_lvl as u32 {
        send_to_char(ch, "You failed to pick the lock.\r\n");
    } else {
        return true;
    }
    return false;
}

fn door_is_openable(
    db: &DB,
    ch: &Rc<CharData>,
    obj: Option<&Rc<ObjData>>,
    door: Option<usize>,
) -> bool {
    if obj.is_some() {
        obj.as_ref().unwrap().get_obj_type() == ITEM_CONTAINER
            && obj.as_ref().unwrap().objval_flagged(CONT_CLOSEABLE)
    } else {
        db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(EX_ISDOOR)
    }
}

fn door_is_open(
    db: &DB,
    ch: &Rc<CharData>,
    obj: Option<&Rc<ObjData>>,
    door: Option<usize>,
) -> bool {
    if obj.is_some() {
        !obj.as_ref().unwrap().objval_flagged(CONT_CLOSED)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(EX_CLOSED)
    }
}

fn door_is_unlocked(
    db: &DB,
    ch: &Rc<CharData>,
    obj: Option<&Rc<ObjData>>,
    door: Option<usize>,
) -> bool {
    if obj.is_some() {
        !obj.as_ref().unwrap().objval_flagged(CONT_LOCKED)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(EX_LOCKED)
    }
}

fn door_is_pickproof(
    db: &DB,
    ch: &Rc<CharData>,
    obj: Option<&Rc<ObjData>>,
    door: Option<usize>,
) -> bool {
    if obj.is_some() {
        !obj.as_ref().unwrap().objval_flagged(CONT_PICKPROOF)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(EX_PICKPROOF)
    }
}

fn door_is_closed(
    db: &DB,
    ch: &Rc<CharData>,
    obj: Option<&Rc<ObjData>>,
    door: Option<usize>,
) -> bool {
    !door_is_open(db, ch, obj, door)
}

fn door_is_locked(
    db: &DB,
    ch: &Rc<CharData>,
    obj: Option<&Rc<ObjData>>,
    door: Option<usize>,
) -> bool {
    !door_is_unlocked(db, ch, obj, door)
}

fn door_key(db: &DB, ch: &Rc<CharData>, obj: Option<&Rc<ObjData>>, door: Option<usize>) -> ObjVnum {
    if obj.is_some() {
        obj.as_ref().unwrap().get_obj_val(2) as ObjVnum
    } else {
        db.exit(ch, door.unwrap()).as_ref().unwrap().key
    }
}

pub fn do_gen_door(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let mut dooro: Option<usize> = None;
    let argument = argument.trim_start();
    let db = &game.db;
    if argument.is_empty() {
        send_to_char(
            ch,
            format!(
                "{}{} what?\r\n",
                CMD_DOOR[subcmd as usize][0..0].to_lowercase(),
                &CMD_DOOR[subcmd as usize][1..]
            )
            .as_str(),
        );
        return;
    }
    let mut type_ = String::new();
    let mut dir = String::new();
    let mut victim = None;
    let mut obj = None;
    two_arguments(argument, &mut type_, &mut dir);
    if !db.generic_find(
        &type_,
        (FIND_OBJ_INV | FIND_OBJ_ROOM) as i64,
        ch,
        &mut victim,
        &mut obj,
    ) != 0
    {
        let dooroi = find_door(db, ch, &type_, &dir, CMD_DOOR[subcmd as usize]);
        dooro = if dooroi.is_some() {
            Some(dooroi.unwrap() as usize)
        } else {
            None
        };
    }

    if obj.is_some() || dooro.is_some() {
        let keynum = door_key(db, ch, obj.as_ref(), dooro);
        if !door_is_openable(db, ch, obj.as_ref(), dooro) {
            db.act(
                "You can't $F that!",
                false,
                Some(ch),
                None,
                Some(&CMD_DOOR[subcmd as usize]),
                TO_CHAR,
            );
        } else if !door_is_open(db, ch, obj.as_ref(), dooro)
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_OPEN)
        {
            send_to_char(ch, "But it's already closed!\r\n");
        } else if !door_is_closed(db, ch, obj.as_ref(), dooro)
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_CLOSED)
        {
            send_to_char(ch, "But it's currently open!\r\n");
        } else if !(door_is_locked(db, ch, obj.as_ref(), dooro))
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_LOCKED)
        {
            send_to_char(ch, "Oh.. it wasn't locked, after all..\r\n");
        } else if !(door_is_unlocked(db, ch, obj.as_ref(), dooro))
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_UNLOCKED)
        {
            send_to_char(ch, "It seems to be locked.\r\n");
        } else if !has_key(db, ch, keynum)
            && (ch.get_level() < LVL_GOD as u8)
            && ((subcmd == SCMD_LOCK) || (subcmd == SCMD_UNLOCK))
        {
            send_to_char(ch, "You don't seem to have the proper key.\r\n");
        } else if ok_pick(
            ch,
            keynum,
            door_is_pickproof(db, ch, obj.as_ref(), dooro),
            subcmd,
        ) {
            do_doorcmd(db, ch, obj.as_ref(), dooro, subcmd);
        }
    }
    return;
}

pub fn do_enter(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let db = &game.db;
    one_argument(argument, &mut buf);

    if !buf.is_empty() {
        /* an argument was supplied, search for door keyword */
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some() {
                if !db.exit(ch, door).as_ref().unwrap().keyword.is_empty() {
                    if db.exit(ch, door).as_ref().unwrap().keyword == buf {
                        perform_move(game, ch, door as i32, true);
                        return;
                    }
                }
            }
        }
        send_to_char(ch, format!("There is no {} here.\r\n", buf).as_str());
    } else if db.room_flagged(ch.in_room(), ROOM_INDOORS) {
        send_to_char(ch, "You are already indoors.\r\n");
    } else {
        /* try to locate an entrance */
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some() {
                if db.exit(ch, door).as_ref().unwrap().to_room.get() != NOWHERE {
                    if !db.exit(ch, door).as_ref().unwrap().exit_flagged(EX_CLOSED)
                        && db.room_flagged(
                            db.exit(ch, door).as_ref().unwrap().to_room.get(),
                            ROOM_INDOORS,
                        )
                    {
                        perform_move(game, ch, door as i32, true);
                        return;
                    }
                }
            }
        }
        send_to_char(ch, "You can't seem to find anything to enter.\r\n");
    }
}

pub fn do_leave(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    if db.outside(ch) {
        send_to_char(ch, "You are outside.. where do you want to go?\r\n");
    } else {
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some() {
                if db.exit(ch, door).as_ref().unwrap().to_room.get() != NOWHERE {
                    if !db.exit(ch, door).as_ref().unwrap().exit_flagged(EX_CLOSED)
                        && !db.room_flagged(
                            db.exit(ch, door).as_ref().unwrap().to_room.get(),
                            ROOM_INDOORS,
                        )
                    {
                        perform_move(game, ch, door as i32, true);
                        return;
                    }
                }
            }
        }
        send_to_char(ch, "I see no obvious exits to the outside.\r\n");
    }
}

pub fn do_stand(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    match ch.get_pos() {
        POS_STANDING => {
            send_to_char(ch, "You are already standing.\r\n");
        }
        POS_SITTING => {
            send_to_char(ch, "You stand up.\r\n");
            db.act(
                "$n clambers to $s feet.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            /* Will be sitting after a successful bash and may still be fighting. */
            ch.set_pos(if ch.fighting().is_some() {
                POS_FIGHTING
            } else {
                POS_STANDING
            });
        }
        POS_RESTING => {
            send_to_char(ch, "You stop resting, and stand up.\r\n");
            db.act(
                "$n stops resting, and clambers on $s feet.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            ch.set_pos(POS_STANDING);
        }
        POS_SLEEPING => {
            send_to_char(ch, "You have to wake up first!\r\n");
        }
        POS_FIGHTING => {
            send_to_char(ch, "Do you not consider fighting as standing?\r\n");
        }
        _ => {
            send_to_char(
                ch,
                "You stop floating around, and put your feet on the ground.\r\n",
            );
            db.act(
                "$n stops floating around, and puts $s feet on the ground.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            ch.set_pos(POS_STANDING);
        }
    }
}

pub fn do_sit(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    match ch.get_pos() {
        POS_STANDING => {
            send_to_char(ch, "You sit down.\r\n");
            db.act("$n sits down.", false, Some(ch), None, None, TO_ROOM);
            ch.set_pos(POS_SITTING);
        }
        POS_SITTING => {
            send_to_char(ch, "You're sitting already.\r\n");
        }
        POS_RESTING => {
            send_to_char(ch, "You stop resting, and sit up.\r\n");
            db.act("$n stops resting.", true, Some(ch), None, None, TO_ROOM);
            ch.set_pos(POS_SITTING);
        }
        POS_SLEEPING => {
            send_to_char(ch, "You have to wake up first.\r\n");
        }
        POS_FIGHTING => {
            send_to_char(ch, "Sit down while fighting? Are you MAD?\r\n");
        }
        _ => {
            send_to_char(ch, "You stop floating around, and sit down.\r\n");
            db.act(
                "$n stops floating around, and sits down.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            ch.set_pos(POS_SITTING);
        }
    }
}

pub fn do_rest(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    match ch.get_pos() {
        POS_STANDING => {
            send_to_char(ch, "You sit down and rest your tired bones.\r\n");
            db.act(
                "$n sits down and rests.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            ch.set_pos(POS_RESTING);
        }
        POS_SITTING => {
            send_to_char(ch, "You rest your tired bones.\r\n");
            db.act("$n rests.", true, Some(ch), None, None, TO_ROOM);
            ch.set_pos(POS_RESTING);
        }
        POS_RESTING => {
            send_to_char(ch, "You are already resting.\r\n");
        }
        POS_SLEEPING => {
            send_to_char(ch, "You have to wake up first.\r\n");
        }
        POS_FIGHTING => {
            send_to_char(ch, "Rest while fighting?  Are you MAD?\r\n");
        }
        _ => {
            send_to_char(
                ch,
                "You stop floating around, and stop to rest your tired bones.\r\n",
            );
            db.act(
                "$n stops floating around, and rests.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            ch.set_pos(POS_SITTING);
        }
    }
}

pub fn do_sleep(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    match ch.get_pos() {
        POS_STANDING | POS_SITTING | POS_RESTING => {
            send_to_char(ch, "You go to sleep.\r\n");
            db.act(
                "$n lies down and falls asleep.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            ch.set_pos(POS_SLEEPING);
        }
        POS_SLEEPING => {
            send_to_char(ch, "You are already sound asleep.\r\n");
        }
        POS_FIGHTING => {
            send_to_char(ch, "Sleep while fighting?  Are you MAD?\r\n");
        }
        _ => {
            send_to_char(ch, "You stop floating around, and lie down to sleep.\r\n");
            db.act(
                "$n stops floating around, and lie down to sleep.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            ch.set_pos(POS_SLEEPING);
        }
    }
}

pub fn do_wake(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut arg = String::new();
    let vict;
    let mut self_ = false;

    one_argument(argument, &mut arg);
    if !arg.is_empty() {
        if ch.get_pos() == POS_SLEEPING {
            send_to_char(ch, "Maybe you should wake yourself up first.\r\n");
        } else if {
            vict = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
            vict.is_none()
        } {
            send_to_char(ch, NOPERSON);
        } else if Rc::ptr_eq(vict.as_ref().unwrap(), ch) {
            self_ = true;
        } else if vict.as_ref().unwrap().awake() {
            db.act(
                "$E is already awake.",
                false,
                Some(ch),
                None,
                Some(vict.as_ref().unwrap()),
                TO_CHAR,
            );
        } else if vict.as_ref().unwrap().aff_flagged(AFF_SLEEP) {
            db.act(
                "You can't wake $M up!",
                false,
                Some(ch),
                None,
                Some(vict.as_ref().unwrap()),
                TO_CHAR,
            );
        } else if vict.as_ref().unwrap().get_pos() < POS_SLEEPING {
            db.act(
                "$E's in pretty bad shape!",
                false,
                Some(ch),
                None,
                Some(vict.as_ref().unwrap()),
                TO_CHAR,
            );
        } else {
            db.act(
                "You wake $M up.",
                false,
                Some(ch),
                None,
                Some(vict.as_ref().unwrap()),
                TO_CHAR,
            );
            db.act(
                "You are awakened by $n.",
                false,
                Some(ch),
                None,
                Some(vict.as_ref().unwrap()),
                TO_VICT | TO_SLEEP,
            );
            vict.as_ref().unwrap().set_pos(POS_SITTING);
        }
        if !self_ {
            return;
        }
    }
    if ch.aff_flagged(AFF_SLEEP) {
        send_to_char(ch, "You can't wake up!\r\n");
    } else if ch.get_pos() > POS_SLEEPING {
        send_to_char(ch, "You are already awake...\r\n");
    } else {
        send_to_char(ch, "You awaken, and sit up.\r\n");
        db.act("$n awakens.", true, Some(ch), None, None, TO_ROOM);
        ch.set_pos(POS_SITTING);
    }
}

pub fn do_follow(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let leader;
    if !buf.is_empty() {
        if {
            leader = db.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
            leader.is_none()
        } {
            send_to_char(ch, NOPERSON);
            return;
        }
    } else {
        send_to_char(ch, "Whom do you wish to follow?\r\n");
        return;
    }

    if ch.master.borrow().is_some()
        && Rc::ptr_eq(
            ch.master.borrow().as_ref().unwrap(),
            leader.as_ref().unwrap(),
        )
    {
        db.act(
            "You are already following $M.",
            false,
            Some(ch),
            None,
            Some(leader.as_ref().unwrap()),
            TO_CHAR,
        );
        return;
    }
    if ch.aff_flagged(AFF_CHARM) && (ch.master.borrow().is_some()) {
        db.act(
            "But you only feel like following $N!",
            false,
            Some(ch),
            None,
            Some(ch.master.borrow().as_ref().unwrap()),
            TO_CHAR,
        );
    } else {
        /* Not Charmed follow person */
        if Rc::ptr_eq(leader.as_ref().unwrap(), ch) {
            if ch.master.borrow().is_none() {
                send_to_char(ch, "You are already following yourself.\r\n");
                return;
            }
            db.stop_follower(ch);
        } else {
            if circle_follow(ch, leader.as_ref()) {
                send_to_char(ch, "Sorry, but following in loops is not allowed.\r\n");
                return;
            }
            if ch.master.borrow().is_some() {
                db.stop_follower(ch);
            }
            ch.remove_aff_flags(AFF_GROUP);

            add_follower(db, ch, leader.as_ref().unwrap());
        }
    }
}
