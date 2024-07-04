/* ************************************************************************
*   File: act.movement.rs                               Part of CircleMUD *
*  Usage: movement commands, door handling, & sleep/rest/etc state        *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use crate::depot::DepotId;
use crate::VictimRef;
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
use crate::{an, is_set, Game, TO_CHAR, TO_ROOM, TO_SLEEP, TO_VICT};

/* simple function to determine if char can walk on water */
fn has_boat(game: &mut Game, db: &DB, chid: DepotId) -> bool {
    let ch = db.ch(chid);
    if ch.get_level() > LVL_IMMORT as u8 {
        return true;
    }

    if ch.aff_flagged(AFF_WATERWALK) {
        return true;
    }

    /* non-wearable boats in inventory will do it */

    let list = ch.carrying.clone();
    for oid in list.iter() {
        if db.obj(*oid).get_obj_type() == ITEM_BOAT && (find_eq_pos(game, db, chid, *oid, "") < 0)
        {
            return true;
        }
    }

    /* and any boat you're wearing will do it too */
    let ch = db.ch(chid);
    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some() && db.obj(ch.get_eq(i).unwrap()).get_obj_type() == ITEM_BOAT
        {
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
pub fn perform_move(game: &mut Game, db: &mut DB, chid: DepotId, dir: i32, need_specials_check: bool) -> bool {
    let ch = db.ch(chid);
    if dir < 0 || dir >= NUM_OF_DIRS as i32 || ch.fighting_id().is_some() {
        return false;
    } else if db.exit(ch, dir as usize).is_none()
        || db.exit(ch, dir as usize).as_ref().unwrap().to_room == NOWHERE
    {
        game.send_to_char(ch, "Alas, you cannot go that way...\r\n");
    } else if 
        db
        .exit(ch, dir as usize)
        .as_ref()
        .unwrap()
        .exit_flagged(EX_CLOSED)
    {
        if !
            db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .keyword
            .is_empty()
        {
            game.send_to_char(ch,
                format!(
                    "The {} seems to be closed.\r\n",
                    fname(
                        db
                            .exit(ch, dir as usize)
                            .as_ref()
                            .unwrap()
                            .keyword
                            .as_ref()
                    )
                )
                .as_str(),
            );
        } else {
            game.send_to_char(ch, "It seems to be closed.\r\n");
        }
    } else {
        if ch.followers.is_empty() {
            return do_simple_move(game, db, chid, dir, need_specials_check);
        }

        let was_in = ch.in_room();
        if !do_simple_move(game,db, chid, dir, need_specials_check) {
            return false;
        }

        let ch = db.ch(chid);
        let list = ch.followers.clone();
        for k in list.iter() {
            let follower = db.ch(k.follower);
            if follower.in_room() == was_in && follower.get_pos() >= POS_STANDING {
                let ch = db.ch(chid);
                game.act(db,
                    "You follow $N.\r\n",
                    false,
                    Some(follower),
                    None,
                    Some(VictimRef::Char(ch)),
                    TO_CHAR,
                );
                perform_move(game, db,k.follower, dir, true);
            }
        }
        return true;
    }
    return false;
}

pub fn do_simple_move(game: &mut Game, db: &mut DB, chid: DepotId, dir: i32, need_specials_check: bool) -> bool {
    let was_in;
    let need_movement;

    /*
     * Check for special routines (North is 1 in command list, but 0 here) Note
     * -- only check if following; this avoids 'double spec-proc' bug
     */
    if need_specials_check && special(game, db, chid, dir + 1, "") {
        return false;
    }

    /* charmed? */
    let ch = db.ch(chid);
    if ch.aff_flagged(AFF_CHARM)
        && ch.master.is_some()
        && ch.in_room() == db.ch(ch.master.unwrap()).in_room()
    {
        game.send_to_char(ch,
            "The thought of leaving your master makes you weep.\r\n",
        );
        game.act(db,
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
        || (
            db
            .sect(db.exit(ch, dir as usize).as_ref().unwrap().to_room)
            == SECT_WATER_NOSWIM)
    {
        if !has_boat(game, db,chid) {
            game.send_to_char(ch, "You need a boat to go there.\r\n");
            return false;
        }
    }

    /* move points needed is avg. move loss for src and destination sect type */
    let ch = db.ch(chid);
    need_movement = (MOVEMENT_LOSS[db.sect(ch.in_room()) as usize]
        + MOVEMENT_LOSS[
            db
            .sect(db.exit(ch, dir as usize).as_ref().unwrap().to_room)
            as usize])
        / 2;

    if ch.get_move() < need_movement as i16 && !ch.is_npc() {
        if need_specials_check && ch.master.is_some() {
            game.send_to_char(ch, "You are too exhausted to follow.\r\n");
        } else {
            game.send_to_char(ch, "You are too exhausted.\r\n");
        }

        return false;
    }

    if db.room_flagged(ch.in_room(), ROOM_ATRIUM) {
        if !house_can_enter(
            db,
            ch,
            db
                .get_room_vnum(db.exit(ch, dir as usize).as_ref().unwrap().to_room),
        ) {
            game.send_to_char(ch, "That's private property -- no trespassing!\r\n");
            return false;
        }
    }
    if db.room_flagged(
        db.exit(ch, dir as usize).as_ref().unwrap().to_room,
        ROOM_TUNNEL,
    ) && num_pc_in_room(
        db.world[db.exit(ch, dir as usize).as_ref().unwrap().to_room as usize].borrow(),
    ) >= TUNNEL_SIZE
    {
        if TUNNEL_SIZE > 1 {
            game.send_to_char(ch, "There isn't enough room for you to go there!\r\n");
        } else {
            game.send_to_char(ch,
                "There isn't enough room there for more than one person!\r\n",
            );
        }
        return false;
    }
    /* Mortals and low level gods cannot enter greater god rooms. */
    if db.room_flagged(
        db.exit(ch, dir as usize).as_ref().unwrap().to_room,
        ROOM_GODROOM,
    ) && ch.get_level() < LVL_GRGOD as u8
    {
        game.send_to_char(ch, "You aren't godly enough to use that room!\r\n");
        return false;
    }

    /* Now we know we're allow to go into the room. */
    if ch.get_level() < LVL_IMMORT as u8 && !ch.is_npc() {
        let ch = db.ch_mut(chid);
        ch.incr_move(-need_movement as i16);
    }
    let ch = db.ch(chid);
    if !ch.aff_flagged(AFF_SNEAK) {
        let buf2 = format!("$n leaves {}.", DIRS[dir as usize]);
        game.act(db,buf2.as_str(), true, Some(ch), None, None, TO_ROOM);
    }
    let ch = db.ch(chid);
    was_in = ch.in_room();
    db.char_from_room(chid);
    let room_dir = db.world[was_in as usize].dir_option[dir as usize]
        .as_ref()
        .unwrap()
        .to_room;
    db.char_to_room(chid, room_dir);

    let ch = db.ch(chid);
    if !ch.aff_flagged(AFF_SNEAK) {
        game.act(db,"$n has arrived.", true, Some(ch), None, None, TO_ROOM);
    }

    let ch = db.ch(chid);
    if ch.desc.borrow().is_some() {
        look_at_room(game, db, chid, false);
    }

    let ch = db.ch(chid);
    if db.room_flagged(ch.in_room(), ROOM_DEATH) && ch.get_level() < LVL_IMMORT as u8 {
        log_death_trap(game, db, chid);
        game.death_cry(db, chid);
        db.extract_char(chid);
        return false;
    }
    return true;
}

pub fn do_move(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, subcmd: i32) {
    /*
     * This is basically a mapping of cmd numbers to perform_move indices.
     * It cannot be done in perform_move because perform_move is called
     * by other functions which do not require the remapping.
     */
    perform_move(game, db,chid, subcmd - 1, false);
}

fn find_door(game: &mut Game, db:  &DB, chid: DepotId, type_: &str, dir: &str, cmdname: &str) -> Option<i32> {
    let ch = db.ch(chid);
    let dooro;

    if !dir.is_empty() {
        /* a direction was specified */
        if {
            dooro = search_block(dir, &DIRS, false);
            dooro.is_none()
        } {
            /* Partial Match */
            game.send_to_char(ch, "That's not a direction.\r\n");
            return None;
        }
        let door = dooro.unwrap();
        if db.exit(ch, door).is_some() {
            /* Braces added according to indent. -gg */
            if !db.exit(ch, door).as_ref().unwrap().keyword.is_empty() {
                if isname(
                    type_,
                    &
                        db
                        .exit(ch, door)
                        .as_ref()
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .keyword,
                ) {
                    return Some(door as i32);
                } else {
                    game.send_to_char(ch, format!("I see no {} there.\r\n", type_).as_str());
                    return None;
                }
            } else {
                return Some(door as i32);
            }
        } else {
            game.send_to_char(ch,
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
            game.send_to_char(ch,
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

        game.send_to_char(ch,
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

fn has_key(db: &DB, ch: &CharData, key: ObjVnum) -> bool {
    for o in ch.carrying.iter() {
        if db.get_obj_vnum(db.obj(*o)) == key {
            return true;
        }
    }

    if ch.get_eq(WEAR_HOLD as i8).is_some() {
        if db.get_obj_vnum(db.obj(ch.get_eq(WEAR_HOLD as i8).unwrap())) == key {
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

fn open_door(db: &mut DB, room: RoomRnum, oid: Option<DepotId>, door: Option<usize>) {
    if oid.is_some() {
        db.obj_mut(oid.unwrap()).remove_objval_bit(1, CONT_CLOSED);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .exit_info &= !EX_CLOSED;
    }
}

fn close_door(db: &mut DB, room: RoomRnum, oid: Option<DepotId>, door: Option<usize>) {
    if oid.is_some() {
        db.obj_mut(oid.unwrap()).set_objval_bit(1, CONT_CLOSED);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .exit_info |= EX_CLOSED;
    }
}

fn lock_door(db: &mut DB, room: RoomRnum, oid: Option<DepotId>, door: Option<usize>) {
    if oid.is_some() {
        db.obj_mut(oid.unwrap()).set_objval_bit(1, CONT_LOCKED);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .exit_info |= EX_LOCKED;
    }
}

fn unlock_door(db: &mut DB, room: RoomRnum, oid: Option<DepotId>, door: Option<usize>) {
    if oid.is_some() {
        db.obj_mut(oid.unwrap()).remove_objval_bit(1, CONT_LOCKED);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .exit_info &= !EX_LOCKED;
    }
}

fn togle_lock(db: &mut DB, room: RoomRnum, oid: Option<DepotId>, door: Option<usize>) {
    if oid.is_some() {
        let v = db.obj(oid.unwrap()).get_obj_val(1) ^ CONT_LOCKED;
        db.obj_mut(oid.unwrap()).set_obj_val(1, v);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .exit_info ^= EX_LOCKED;
    }
}

fn do_doorcmd(
    game: &mut Game, db: &mut DB,
    chid: DepotId,
    oid: Option<DepotId>,
    door: Option<usize>,
    scmd: i32,
) {
    let ch = db.ch(chid);
    let mut buf;

    let mut other_room = NOWHERE;

    let mut back_to_room: Option<i16> = None;
    let mut back_keyword = None;

    buf = format!("$n {}s ", CMD_DOOR[scmd as usize]);
    if oid.is_none() && {
        other_room = db.exit(ch, door.unwrap()).as_ref().unwrap().to_room;
        other_room != NOWHERE
    } {
        if {
            back_to_room = db.world[other_room as usize].dir_option
                [REV_DIR[door.unwrap()] as usize]
                .as_ref()
                .map(|e| e.to_room);
            back_to_room.is_some()
        } {
            if back_to_room.unwrap() != ch.in_room {
                back_to_room = None;
            }
            back_keyword = db.world[other_room as usize].dir_option
                [REV_DIR[door.unwrap()] as usize]
                .as_ref()
                .map(|e: &RoomDirectionData| e.keyword.clone());
        }
    }

    match scmd {
        SCMD_OPEN => {
            let ch_in_room = ch.in_room();
            open_door( db, ch_in_room, oid, door);
            if back_to_room.is_some() {
                open_door(
                     db,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            let ch = db.ch(chid);
            game.send_to_char(ch, OK);
        }
        SCMD_CLOSE => {
            let ch_in_room = ch.in_room();
            close_door( db, ch_in_room,oid, door);
            if back_to_room.is_some() {
                close_door(
                     db,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            let ch = db.ch(chid);
            game.send_to_char(ch, OK);
        }
        SCMD_LOCK => {
            let ch_in_room = ch.in_room();
            lock_door(db, ch_in_room,oid, door);
            if back_to_room.is_some() {
                lock_door(
                     db,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            let ch = db.ch(chid);
            game.send_to_char(ch, OK);
        }
        SCMD_UNLOCK => {
            let ch_in_room = ch.in_room();
            unlock_door(db, ch_in_room,oid, door);
            if back_to_room.is_some() {
                unlock_door(
                    db,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            let ch = db.ch(chid);
            game.send_to_char(ch, OK);
        }

        SCMD_PICK => {
            let ch_in_room = ch.in_room();
            togle_lock( db, ch_in_room,oid, door);
            if back_to_room.is_some() {
                togle_lock(
                     db,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap() as usize] as usize),
                );
            }
            let ch = db.ch(chid);
            game.send_to_char(ch, "The lock quickly yields to your skills.\r\n");
            buf = "$n skillfully picks the lock on ".to_string();
        }
        _ => {}
    }

    /* Notify the room. */
    buf.push_str(
        format!(
            "{}{}.",
            if oid.is_some() { "" } else { "the " },
            if oid.is_some() {
                "$p"
            } else {
                let ch = db.ch(chid);
                if !
                    db
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
    let ch = db.ch(chid);

    if oid.is_none() || db.obj(oid.unwrap()).in_room() != NOWHERE {
        let vict_obj = if oid.is_some() {
            None
        } else {
            let ch = db.ch(chid);
            Some(VictimRef::Str(
                db.exit(ch, door.unwrap()).unwrap().keyword.clone(),
            ))
        };
        game.act(db,
            &buf,
            false,
            Some(ch),
            if oid.is_none() {
                None
            } else {
                Some(db.obj(oid.unwrap()))
            },
            vict_obj,
            TO_ROOM,
        );
    }

    /* Notify the other room */
    if back_to_room.is_some() && (scmd == SCMD_OPEN || scmd == SCMD_CLOSE) {
        let x = fname(back_keyword.as_ref().unwrap());
        let ch = db.ch(chid);
        game.send_to_room(db,
            db.exit(ch, door.unwrap()).as_ref().unwrap().to_room,
            format!(
                "The {} is {}{} from the other side.",
                if !back_keyword.as_ref().unwrap().is_empty() {
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

fn ok_pick(game: &mut Game, db: &mut DB, chid: DepotId, keynum: ObjVnum, pickproof: bool, scmd: i32) -> bool {
    let ch = db.ch(chid);
    if scmd != SCMD_PICK {
        return true;
    }

    let percent = rand_number(1, 101);
    let skill_lvl =
        ch.get_skill(SKILL_PICK_LOCK) as i16 + DEX_APP_SKILL[ch.get_dex() as usize].p_locks;

    if keynum == NOTHING {
        game.send_to_char(ch, "Odd - you can't seem to find a keyhole.\r\n");
    } else if pickproof {
        game.send_to_char(ch, "It resists your attempts to pick it.\r\n");
    } else if percent > skill_lvl as u32 {
        game.send_to_char(ch, "You failed to pick the lock.\r\n");
    } else {
        return true;
    }
    return false;
}

fn door_is_openable(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
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

fn door_is_open(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    if obj.is_some() {
        !obj.as_ref().unwrap().objval_flagged(CONT_CLOSED)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(EX_CLOSED)
    }
}

fn door_is_unlocked(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    if obj.is_some() {
        !obj.as_ref().unwrap().objval_flagged(CONT_LOCKED)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(EX_LOCKED)
    }
}

fn door_is_pickproof(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    if obj.is_some() {
        !obj.as_ref().unwrap().objval_flagged(CONT_PICKPROOF)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(EX_PICKPROOF)
    }
}

fn door_is_closed(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    !door_is_open(db, ch, obj, door)
}

fn door_is_locked(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    !door_is_unlocked(db, ch, obj, door)
}

fn door_key(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> ObjVnum {
    if obj.is_some() {
        obj.as_ref().unwrap().get_obj_val(2) as ObjVnum
    } else {
        db.exit(ch, door.unwrap()).as_ref().unwrap().key
    }
}

pub fn do_gen_door(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = db.ch(chid);
    let mut dooro: Option<usize> = None;
    let argument = argument.trim_start();
    if argument.is_empty() {
        game.send_to_char(ch,
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
    let mut oid = None;
    two_arguments(argument, &mut type_, &mut dir);
    if !game.generic_find(db,
        &type_,
        (FIND_OBJ_INV | FIND_OBJ_ROOM) as i64,
        chid,
        &mut victim,
        &mut oid,
    ) != 0
    {
        let dooroi = find_door(game,db, chid, &type_, &dir, CMD_DOOR[subcmd as usize]);
        dooro = if dooroi.is_some() {
            Some(dooroi.unwrap() as usize)
        } else {
            None
        };
    }

    if oid.is_some() || dooro.is_some() {
        let ch = db.ch(chid);
        let keynum = door_key(&db, ch, oid.map(|o| db.obj(o)), dooro);
        if !door_is_openable(&db, ch, oid.map(|o| db.obj(o)), dooro) {
            game.act(db,
                "You can't $F that!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Str(Rc::from(CMD_DOOR[subcmd as usize]))),
                TO_CHAR,
            );
        } else if !door_is_open(&db, ch, oid.map(|o| db.obj(o)), dooro)
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_OPEN)
        {
            game.send_to_char(ch, "But it's already closed!\r\n");
        } else if !door_is_closed(&db, ch, oid.map(|o| db.obj(o)), dooro)
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_CLOSED)
        {
            game.send_to_char(ch, "But it's currently open!\r\n");
        } else if !(door_is_locked(&db, ch, oid.map(|o| db.obj(o)), dooro))
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_LOCKED)
        {
            game.send_to_char(ch, "Oh.. it wasn't locked, after all..\r\n");
        } else if !(door_is_unlocked(&db, ch, oid.map(|o| db.obj(o)), dooro))
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_UNLOCKED)
        {
            game.send_to_char(ch, "It seems to be locked.\r\n");
        } else if !has_key(&db, ch, keynum)
            && (ch.get_level() < LVL_GOD as u8)
            && ((subcmd == SCMD_LOCK) || (subcmd == SCMD_UNLOCK))
        {
            game.send_to_char(ch, "You don't seem to have the proper key.\r\n");
        } else if {
            let pickproof = door_is_pickproof(&db, ch, oid.map(|o| db.obj(o)), dooro);
            ok_pick(game, db,chid, keynum, pickproof, subcmd)
        } {
            do_doorcmd(game, db, chid, oid, dooro, subcmd);
        }
    }
    return;
}

pub fn do_enter(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut buf = String::new();
    one_argument(argument, &mut buf);

    if !buf.is_empty() {
        /* an argument was supplied, search for door keyword */
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some() {
                if !db.exit(ch, door).as_ref().unwrap().keyword.is_empty() {
                    if db.exit(ch, door).as_ref().unwrap().keyword.as_ref() == buf {
                        perform_move(game,db, chid, door as i32, true);
                        return;
                    }
                }
            }
        }
        game.send_to_char(ch, format!("There is no {} here.\r\n", buf).as_str());
    } else if db.room_flagged(ch.in_room(), ROOM_INDOORS) {
        game.send_to_char(ch, "You are already indoors.\r\n");
    } else {
        /* try to locate an entrance */
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some() {
                if db.exit(ch, door).as_ref().unwrap().to_room != NOWHERE {
                    if !db.exit(ch, door).as_ref().unwrap().exit_flagged(EX_CLOSED)
                        && db
                            .room_flagged(db.exit(ch, door).as_ref().unwrap().to_room, ROOM_INDOORS)
                    {
                        perform_move(game,db, chid, door as i32, true);
                        return;
                    }
                }
            }
        }
        game.send_to_char(ch, "You can't seem to find anything to enter.\r\n");
    }
}

pub fn do_leave(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    if db.outside(ch) {
        game.send_to_char(ch, "You are outside.. where do you want to go?\r\n");
    } else {
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some() {
                if db.exit(ch, door).as_ref().unwrap().to_room != NOWHERE {
                    if !db.exit(ch, door).as_ref().unwrap().exit_flagged(EX_CLOSED)
                        && !db
                            .room_flagged(db.exit(ch, door).as_ref().unwrap().to_room, ROOM_INDOORS)
                    {
                        perform_move(game, db, chid, door as i32, true);
                        return;
                    }
                }
            }
        }
        game.send_to_char(ch, "I see no obvious exits to the outside.\r\n");
    }
}

pub fn do_stand(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    match ch.get_pos() {
        POS_STANDING => {
            game.send_to_char(ch, "You are already standing.\r\n");
        }
        POS_SITTING => {
            game.send_to_char(ch, "You stand up.\r\n");
            game.act(db,
                "$n clambers to $s feet.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            /* Will be sitting after a successful bash and may still be fighting. */
            ch.set_pos(if ch.fighting_id().is_some() {
                POS_FIGHTING
            } else {
                POS_STANDING
            });
        }
        POS_RESTING => {
            game.send_to_char(ch, "You stop resting, and stand up.\r\n");
            game.act(db,
                "$n stops resting, and clambers on $s feet.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_STANDING);
        }
        POS_SLEEPING => {
            game.send_to_char(ch, "You have to wake up first!\r\n");
        }
        POS_FIGHTING => {
            game.send_to_char(ch, "Do you not consider fighting as standing?\r\n");
        }
        _ => {
            game.send_to_char(ch,
                "You stop floating around, and put your feet on the ground.\r\n",
            );
            game.act(db,
                "$n stops floating around, and puts $s feet on the ground.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_STANDING);
        }
    }
}

pub fn do_sit(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    match ch.get_pos() {
        POS_STANDING => {
            game.send_to_char(ch, "You sit down.\r\n");
            game.act(db,"$n sits down.", false, Some(ch), None, None, TO_ROOM);
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_SITTING);
        }
        POS_SITTING => {
            game.send_to_char(ch, "You're sitting already.\r\n");
        }
        POS_RESTING => {
            game.send_to_char(ch, "You stop resting, and sit up.\r\n");
            game.act(db,
                "$n stops resting.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_SITTING);
        }
        POS_SLEEPING => {
            game.send_to_char(ch, "You have to wake up first.\r\n");
        }
        POS_FIGHTING => {
            game.send_to_char(ch, "Sit down while fighting? Are you MAD?\r\n");
        }
        _ => {
            game.send_to_char(ch, "You stop floating around, and sit down.\r\n");
            game.act(db,
                "$n stops floating around, and sits down.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_SITTING);
        }
    }
}

pub fn do_rest(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    match ch.get_pos() {
        POS_STANDING => {
            game.send_to_char(ch, "You sit down and rest your tired bones.\r\n");
            game.act(db,
                "$n sits down and rests.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_RESTING);
        }
        POS_SITTING => {
            game.send_to_char(ch, "You rest your tired bones.\r\n");
            game.act(db,"$n rests.", true, Some(ch), None, None, TO_ROOM);
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_RESTING);
        }
        POS_RESTING => {
            game.send_to_char(ch, "You are already resting.\r\n");
        }
        POS_SLEEPING => {
            game.send_to_char(ch, "You have to wake up first.\r\n");
        }
        POS_FIGHTING => {
            game.send_to_char(ch, "Rest while fighting?  Are you MAD?\r\n");
        }
        _ => {
            game.send_to_char(ch,
                "You stop floating around, and stop to rest your tired bones.\r\n",
            );
            game.act(db,
                "$n stops floating around, and rests.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_SITTING);
        }
    }
}

pub fn do_sleep(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    match ch.get_pos() {
        POS_STANDING | POS_SITTING | POS_RESTING => {
            game.send_to_char(ch, "You go to sleep.\r\n");
            game.act(db,
                "$n lies down and falls asleep.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_SLEEPING);
        }
        POS_SLEEPING => {
            game.send_to_char(ch, "You are already sound asleep.\r\n");
        }
        POS_FIGHTING => {
            game.send_to_char(ch, "Sleep while fighting?  Are you MAD?\r\n");
        }
        _ => {
            game.send_to_char(ch,
                "You stop floating around, and lie down to sleep.\r\n",
            );
            game.act(db,
                "$n stops floating around, and lie down to sleep.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_SLEEPING);
        }
    }
}

pub fn do_wake(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut arg = String::new();
    let vict_id;
    let mut self_ = false;

    one_argument(argument, &mut arg);
    if !arg.is_empty() {
        if ch.get_pos() == POS_SLEEPING {
            game.send_to_char(ch, "Maybe you should wake yourself up first.\r\n");
        } else if {
            vict_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_ROOM);
            vict_id.is_none()
        } {
            game.send_to_char(ch, NOPERSON);
        } else if vict_id.unwrap() == chid {
            self_ = true;
        } else if db.ch(vict_id.unwrap()).awake() {
            let vict = db.ch(vict_id.unwrap());
            game.act(db,
                "$E is already awake.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
        } else if db.ch(vict_id.unwrap()).aff_flagged(AFF_SLEEP) {
            let vict = db.ch(vict_id.unwrap());
            game.act(db,
                "You can't wake $M up!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
        } else if db.ch(vict_id.unwrap()).get_pos() < POS_SLEEPING {
            let vict = db.ch(vict_id.unwrap());
            game.act(db,
                "$E's in pretty bad shape!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
        } else {
            let vict = db.ch(vict_id.unwrap());
            game.act(db,
                "You wake $M up.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
            game.act(db,
                "You are awakened by $n.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT | TO_SLEEP,
            );
            db.ch_mut(vict_id.unwrap()).set_pos(POS_SITTING);
        }
        if !self_ {
            return;
        }
    }
    let ch = db.ch(chid);
    if ch.aff_flagged(AFF_SLEEP) {
        game.send_to_char(ch, "You can't wake up!\r\n");
    } else if ch.get_pos() > POS_SLEEPING {
        game.send_to_char(ch, "You are already awake...\r\n");
    } else {
        game.send_to_char(ch, "You awaken, and sit up.\r\n");
        game.act(db,"$n awakens.", true, Some(ch), None, None, TO_ROOM);
        let ch = db.ch_mut(chid);
        ch.set_pos(POS_SITTING);
    }
}

pub fn do_follow(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let leader_id;
    if !buf.is_empty() {
        if {
            leader_id = game.get_char_vis(db,chid, &mut buf, None, FIND_CHAR_ROOM);
            leader_id.is_none()
        } {
            game.send_to_char(ch, NOPERSON);
            return;
        }
    } else {
        game.send_to_char(ch, "Whom do you wish to follow?\r\n");
        return;
    }

    if ch.master.is_some() && ch.master.unwrap() == leader_id.unwrap() {
        let leader = db.ch(leader_id.unwrap());
        game.act(db,
            "You are already following $M.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(leader)),
            TO_CHAR,
        );
        return;
    }
    if ch.aff_flagged(AFF_CHARM) && (ch.master.is_some()) {
        let master_id = ch.master.unwrap();
        let master = db.ch(master_id);
        game.act(db,
            "But you only feel like following $N!",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(master)),
            TO_CHAR,
        );
    } else {
        /* Not Charmed follow person */
        if leader_id.unwrap() == chid {
            if ch.master.is_none() {
                game.send_to_char(ch, "You are already following yourself.\r\n");
                return;
            }
            game.stop_follower(db, chid);
        } else {
            if circle_follow(&db, chid, leader_id) {
                game.send_to_char(ch, "Sorry, but following in loops is not allowed.\r\n");
                return;
            }
            if ch.master.is_some() {
                game.stop_follower(db, chid);
            }
            let ch = db.ch_mut(chid);
            ch.remove_aff_flags(AFF_GROUP);

            add_follower(game, db, chid, leader_id.unwrap());
        }
    }
}
