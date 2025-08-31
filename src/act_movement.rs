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

use crate::depot::{Depot, DepotId, HasId};
use crate::fight::death_cry;
use crate::{act, send_to_char, send_to_room, DescriptorData, TextData, VictimRef};
use std::borrow::Borrow;

use crate::act_informative::look_at_room;
use crate::act_item::find_eq_pos;
use crate::config::{NOPERSON, OK, TUNNEL_SIZE};
use crate::constants::{DEX_APP_SKILL, DIRS, MOVEMENT_LOSS, REV_DIR};
use crate::db::DB;
use crate::handler::{fname, generic_find, get_char_vis, isname, FindFlags};
use crate::house::house_can_enter;
use crate::interpreter::{
    one_argument, search_block, special, two_arguments, SCMD_CLOSE, SCMD_LOCK, SCMD_OPEN,
    SCMD_PICK, SCMD_UNLOCK,
};
use crate::spells::SKILL_PICK_LOCK;
use crate::structs::{
    AffectFlags, CharData, ExitFlags, ItemType, ObjData, ObjVnum, Position, RoomDirectionData,
    RoomFlags, RoomRnum, SectorType, CONT_CLOSEABLE, CONT_CLOSED, CONT_LOCKED, CONT_PICKPROOF,
    LVL_GOD, LVL_GRGOD, LVL_IMMORT, NOTHING, NOWHERE, NUM_OF_DIRS, NUM_WEARS, WEAR_HOLD,
};
use crate::util::{
    add_follower, circle_follow, log_death_trap, num_pc_in_room, rand_number, stop_follower,
};
use crate::{an, is_set, Game, TO_CHAR, TO_ROOM, TO_SLEEP, TO_VICT};

/* simple function to determine if char can walk on water */
fn has_boat(descs: &mut Depot<DescriptorData>, objs: &Depot<ObjData>, ch: &CharData) -> bool {
    if ch.get_level() > LVL_IMMORT as u8 {
        return true;
    }

    if ch.aff_flagged(AffectFlags::WATERWALK) {
        return true;
    }

    /* non-wearable boats in inventory will do it */

    for &oid in &ch.carrying {
        let obj = objs.get(oid);
        if obj.get_obj_type() == ItemType::Boat && (find_eq_pos(descs, ch, obj, "") < 0) {
            return true;
        }
    }

    /* and any boat you're wearing will do it too */
    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some()
            && objs.get(ch.get_eq(i).unwrap()).get_obj_type() == ItemType::Boat
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
#[allow(clippy::too_many_arguments)]
pub fn perform_move(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    dir: i32,
    need_specials_check: bool,
) -> bool {
    let ch = chars.get(chid);
    if dir < 0 || dir >= NUM_OF_DIRS as i32 || ch.fighting_id().is_some() {
        return false;
    } else if db.exit(ch, dir as usize).is_none()
        || db.exit(ch, dir as usize).as_ref().unwrap().to_room == NOWHERE
    {
        send_to_char(
            &mut game.descriptors,
            ch,
            "Alas, you cannot go that way...\r\n",
        );
    } else if db
        .exit(ch, dir as usize)
        .as_ref()
        .unwrap()
        .exit_flagged(ExitFlags::CLOSED)
    {
        if !db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .keyword
            .is_empty()
        {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!(
                    "The {} seems to be closed.\r\n",
                    fname(db.exit(ch, dir as usize).as_ref().unwrap().keyword.as_ref())
                )
                .as_str(),
            );
        } else {
            send_to_char(&mut game.descriptors, ch, "It seems to be closed.\r\n");
        }
    } else {
        if ch.followers.is_empty() {
            return do_simple_move(game, db, chars, texts, objs, chid, dir, need_specials_check);
        }

        let was_in = ch.in_room();
        if !do_simple_move(game, db, chars, texts, objs, chid, dir, need_specials_check) {
            return false;
        }

        let ch = chars.get(chid);
        for f in ch.followers.clone() {
            let follower = chars.get(f.follower);
            if follower.in_room() == was_in && follower.get_pos() >= Position::Standing {
                let ch = chars.get(chid);
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "You follow $N.\r\n",
                    false,
                    Some(follower),
                    None,
                    Some(VictimRef::Char(ch)),
                    TO_CHAR,
                );
                perform_move(game, db, chars, texts, objs, f.follower, dir, true);
            }
        }
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn do_simple_move(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    dir: i32,
    need_specials_check: bool,
) -> bool {
    /*
     * Check for special routines (North is 1 in command list, but 0 here) Note
     * -- only check if following; this avoids 'double spec-proc' bug
     */
    if need_specials_check && special(game, db, chars, texts, objs, chid, dir + 1, "") {
        return false;
    }

    /* charmed? */
    let ch = chars.get(chid);
    if ch.aff_flagged(AffectFlags::CHARM)
        && ch.master.is_some()
        && ch.in_room() == chars.get(ch.master.unwrap()).in_room()
    {
        send_to_char(
            &mut game.descriptors,
            ch,
            "The thought of leaving your master makes you weep.\r\n",
        );
        act(
            &mut game.descriptors,
            chars,
            db,
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
    if ((db.sect(ch.in_room()) == SectorType::WaterNoSwim)
        || (db.sect(db.exit(ch, dir as usize).as_ref().unwrap().to_room)
            == SectorType::WaterNoSwim))
        && !has_boat(&mut game.descriptors, objs, ch)
    {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You need a boat to go there.\r\n",
        );
        return false;
    }

    /* move points needed is avg. move loss for src and destination sect type */
    let ch = chars.get(chid);
    let need_movement = (MOVEMENT_LOSS[db.sect(ch.in_room()) as usize]
        + MOVEMENT_LOSS[db.sect(db.exit(ch, dir as usize).as_ref().unwrap().to_room) as usize])
        / 2;

    if ch.get_move() < need_movement as i16 && !ch.is_npc() {
        if need_specials_check && ch.master.is_some() {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You are too exhausted to follow.\r\n",
            );
        } else {
            send_to_char(&mut game.descriptors, ch, "You are too exhausted.\r\n");
        }

        return false;
    }

    if db.room_flagged(ch.in_room(), RoomFlags::ATRIUM)
        && !house_can_enter(
            db,
            ch,
            db.get_room_vnum(db.exit(ch, dir as usize).as_ref().unwrap().to_room),
        )
    {
        send_to_char(
            &mut game.descriptors,
            ch,
            "That's private property -- no trespassing!\r\n",
        );
        return false;
    }
    if db.room_flagged(
        db.exit(ch, dir as usize).as_ref().unwrap().to_room,
        RoomFlags::TUNNEL,
    ) && num_pc_in_room(
        db.world[db.exit(ch, dir as usize).as_ref().unwrap().to_room as usize].borrow(),
    ) >= TUNNEL_SIZE
    {
        if TUNNEL_SIZE > 1 {
            send_to_char(
                &mut game.descriptors,
                ch,
                "There isn't enough room for you to go there!\r\n",
            );
        } else {
            send_to_char(
                &mut game.descriptors,
                ch,
                "There isn't enough room there for more than one person!\r\n",
            );
        }
        return false;
    }
    /* Mortals and low level gods cannot enter greater god rooms. */
    if db.room_flagged(
        db.exit(ch, dir as usize).as_ref().unwrap().to_room,
        RoomFlags::GODROOM,
    ) && ch.get_level() < LVL_GRGOD as u8
    {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You aren't godly enough to use that room!\r\n",
        );
        return false;
    }

    /* Now we know we're allow to go into the room. */
    if ch.get_level() < LVL_IMMORT as u8 && !ch.is_npc() {
        let ch = chars.get_mut(chid);
        ch.incr_move(-need_movement as i16);
    }
    let ch = chars.get(chid);
    if !ch.aff_flagged(AffectFlags::SNEAK) {
        let buf2 = format!("$n leaves {}.", DIRS[dir as usize]);
        act(
            &mut game.descriptors,
            chars,
            db,
            buf2.as_str(),
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
    }
    let ch = chars.get(chid);
    let was_in = ch.in_room();
    let ch = chars.get_mut(chid);
    db.char_from_room(objs, ch);
    let room_dir = db.world[was_in as usize].dir_option[dir as usize]
        .as_ref()
        .unwrap()
        .to_room;
    db.char_to_room(chars, objs, chid, room_dir);

    let ch = chars.get(chid);
    if !ch.aff_flagged(AffectFlags::SNEAK) {
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n has arrived.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
    }

    let ch = chars.get(chid);
    if ch.desc.borrow().is_some() {
        look_at_room(&mut game.descriptors, db, chars, texts, objs, ch, false);
    }

    let ch = chars.get(chid);
    if db.room_flagged(ch.in_room(), RoomFlags::DEATH) && ch.get_level() < LVL_IMMORT as u8 {
        log_death_trap(game, chars, db, chid);
        death_cry(&mut game.descriptors, chars, db, ch);
        db.extract_char(chars, chid);
        return false;
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub fn do_move(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    /*
     * This is basically a mapping of cmd numbers to perform_move indices.
     * It cannot be done in perform_move because perform_move is called
     * by other functions which do not require the remapping.
     */
    perform_move(game, db, chars, texts, objs, chid, subcmd - 1, false);
}

fn find_door(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    ch: &CharData,
    type_: &str,
    dir: &str,
    cmdname: &str,
) -> Option<i32> {
    let dooro;

    if !dir.is_empty() {
        /* a direction was specified */
        let res = {
            dooro = search_block(dir, &DIRS, false);
            dooro.is_none()
        };
        if res {
            /* Partial Match */
            send_to_char(descs, ch, "That's not a direction.\r\n");
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
                    Some(door as i32)
                } else {
                    send_to_char(descs, ch, format!("I see no {} there.\r\n", type_).as_str());
                    None
                }
            } else {
                Some(door as i32)
            }
        } else {
            send_to_char(
                descs,
                ch,
                format!(
                    "I really don't see how you can {} anything there.\r\n",
                    cmdname
                )
                .as_str(),
            );
            None
        }
    } else {
        /* try to locate the keyword */
        if type_.is_empty() {
            send_to_char(
                descs,
                ch,
                format!("What is it you want to {}?\r\n", cmdname).as_str(),
            );
            return None;
        }
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some()
                && !db.exit(ch, door).as_ref().unwrap().keyword.is_empty()
                && isname(type_, &db.exit(ch, door).as_ref().unwrap().keyword)
            {
                return Some(door as i32);
            }
        }

        send_to_char(
            descs,
            ch,
            format!(
                "There doesn't seem to be {} {} here.\r\n",
                an!(type_),
                type_
            )
            .as_str(),
        );
        None
    }
}

fn has_key(db: &DB, objs: &Depot<ObjData>, ch: &CharData, key: ObjVnum) -> bool {
    for o in ch.carrying.iter() {
        if db.get_obj_vnum(objs.get(*o)) == key {
            return true;
        }
    }

    if ch.get_eq(WEAR_HOLD).is_some()
        && db.get_obj_vnum(objs.get(ch.get_eq(WEAR_HOLD).unwrap())) == key
    {
        return true;
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

fn open_door(
    db: &mut DB,
    objs: &mut Depot<ObjData>,
    room: RoomRnum,
    oid: Option<DepotId>,
    door: Option<usize>,
) {
    if let Some(oid) = oid {
        objs.get_mut(oid).remove_objval_bit(1, CONT_CLOSED);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .remove_exit_info_bit(ExitFlags::CLOSED);
    }
}

fn close_door(
    db: &mut DB,
    objs: &mut Depot<ObjData>,
    room: RoomRnum,
    oid: Option<DepotId>,
    door: Option<usize>,
) {
    if let Some(oid) = oid {
        objs.get_mut(oid).set_objval_bit(1, CONT_CLOSED);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .set_exit_info_bit(ExitFlags::CLOSED);
    }
}

fn lock_door(
    db: &mut DB,
    objs: &mut Depot<ObjData>,
    room: RoomRnum,
    oid: Option<DepotId>,
    door: Option<usize>,
) {
    if let Some(oid) = oid {
        objs.get_mut(oid).set_objval_bit(1, CONT_LOCKED);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .set_exit_info_bit(ExitFlags::LOCKED);
    }
}

fn unlock_door(
    db: &mut DB,
    objs: &mut Depot<ObjData>,
    room: RoomRnum,
    oid: Option<DepotId>,
    door: Option<usize>,
) {
    if let Some(oid) = oid {
        objs.get_mut(oid).remove_objval_bit(1, CONT_LOCKED);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .remove_exit_info_bit(ExitFlags::LOCKED);
    }
}

fn togle_lock(
    db: &mut DB,
    objs: &mut Depot<ObjData>,
    room: RoomRnum,
    oid: Option<DepotId>,
    door: Option<usize>,
) {
    if let Some(oid) = oid {
        let v = objs.get(oid).get_obj_val(1) ^ CONT_LOCKED;
        objs.get_mut(oid).set_obj_val(1, v);
    } else {
        db.world[room as usize].dir_option[door.unwrap()]
            .as_mut()
            .unwrap()
            .exit_info
            .toggle(ExitFlags::LOCKED);
    }
}

#[allow(clippy::too_many_arguments)]
fn do_doorcmd(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    oid: Option<DepotId>,
    door: Option<usize>,
    scmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf;

    let mut other_room = NOWHERE;

    let mut back_to_room = None;
    let mut back_keyword = None;

    buf = format!("$n {}s ", CMD_DOOR[scmd as usize]);
    if oid.is_none()
        && {
            other_room = db.exit(ch, door.unwrap()).as_ref().unwrap().to_room;
            other_room != NOWHERE
        }
        && {
            back_to_room = db.world[other_room as usize].dir_option
                [REV_DIR[door.unwrap()] as usize]
                .as_ref()
                .map(|e| e.to_room);
            back_to_room.is_some()
        }
    {
        if back_to_room.unwrap() != ch.in_room {
            back_to_room = None;
        }
        back_keyword = db.world[other_room as usize].dir_option[REV_DIR[door.unwrap()] as usize]
            .as_ref()
            .map(|e: &RoomDirectionData| e.keyword.clone());
    }

    match scmd {
        SCMD_OPEN => {
            let ch_in_room = ch.in_room();
            open_door(db, objs, ch_in_room, oid, door);
            if back_to_room.is_some() {
                open_door(
                    db,
                    objs,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap()] as usize),
                );
            }
            let ch = chars.get(chid);
            send_to_char(descs, ch, OK);
        }
        SCMD_CLOSE => {
            let ch_in_room = ch.in_room();
            close_door(db, objs, ch_in_room, oid, door);
            if back_to_room.is_some() {
                close_door(
                    db,
                    objs,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap()] as usize),
                );
            }
            let ch = chars.get(chid);
            send_to_char(descs, ch, OK);
        }
        SCMD_LOCK => {
            let ch_in_room = ch.in_room();
            lock_door(db, objs, ch_in_room, oid, door);
            if back_to_room.is_some() {
                lock_door(
                    db,
                    objs,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap()] as usize),
                );
            }
            let ch = chars.get(chid);
            send_to_char(descs, ch, OK);
        }
        SCMD_UNLOCK => {
            let ch_in_room = ch.in_room();
            unlock_door(db, objs, ch_in_room, oid, door);
            if back_to_room.is_some() {
                unlock_door(
                    db,
                    objs,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap()] as usize),
                );
            }
            let ch = chars.get(chid);
            send_to_char(descs, ch, OK);
        }

        SCMD_PICK => {
            let ch_in_room = ch.in_room();
            togle_lock(db, objs, ch_in_room, oid, door);
            if back_to_room.is_some() {
                togle_lock(
                    db,
                    objs,
                    other_room,
                    oid,
                    Some(REV_DIR[door.unwrap()] as usize),
                );
            }
            let ch = chars.get(chid);
            send_to_char(descs, ch, "The lock quickly yields to your skills.\r\n");
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
                let ch = chars.get(chid);
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
    let ch = chars.get(chid);

    if oid.is_none() || objs.get(oid.unwrap()).in_room() != NOWHERE {
        let vict_obj = if oid.is_some() {
            None
        } else {
            let ch = chars.get(chid);
            Some(VictimRef::Str(
                db.exit(ch, door.unwrap()).unwrap().keyword.as_ref(),
            ))
        };
        act(
            descs,
            chars,
            db,
            &buf,
            false,
            Some(ch),
            if let Some(oid) = oid {
                Some(objs.get(oid))
            } else {
                None
            },
            vict_obj,
            TO_ROOM,
        );
    }

    /* Notify the other room */
    if back_to_room.is_some() && (scmd == SCMD_OPEN || scmd == SCMD_CLOSE) {
        let x = fname(back_keyword.as_ref().unwrap());
        let ch = chars.get(chid);
        send_to_room(
            descs,
            chars,
            db,
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

fn ok_pick(
    descs: &mut Depot<DescriptorData>,
    chars: &mut Depot<CharData>,
    chid: DepotId,
    keynum: ObjVnum,
    pickproof: bool,
    scmd: i32,
) -> bool {
    let ch = chars.get(chid);
    if scmd != SCMD_PICK {
        return true;
    }

    let percent = rand_number(1, 101);
    let skill_lvl =
        ch.get_skill(SKILL_PICK_LOCK) as i16 + DEX_APP_SKILL[ch.get_dex() as usize].p_locks;

    if keynum == NOTHING {
        send_to_char(descs, ch, "Odd - you can't seem to find a keyhole.\r\n");
    } else if pickproof {
        send_to_char(descs, ch, "It resists your attempts to pick it.\r\n");
    } else if percent > skill_lvl as u32 {
        send_to_char(descs, ch, "You failed to pick the lock.\r\n");
    } else {
        return true;
    }
    false
}

fn door_is_openable(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    if let Some(obj) = obj {
        obj.get_obj_type() == ItemType::Container && obj.objval_flagged(CONT_CLOSEABLE)
    } else {
        db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(ExitFlags::ISDOOR)
    }
}

fn door_is_open(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    if let Some(obj) = obj {
        !obj.objval_flagged(CONT_CLOSED)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(ExitFlags::CLOSED)
    }
}

fn door_is_unlocked(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    if let Some(obj) = obj {
        !obj.objval_flagged(CONT_LOCKED)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(ExitFlags::LOCKED)
    }
}

fn door_is_pickproof(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    if let Some(obj) = obj {
        !obj.objval_flagged(CONT_PICKPROOF)
    } else {
        !db.exit(ch, door.unwrap())
            .as_ref()
            .unwrap()
            .exit_flagged(ExitFlags::PICKPROOF)
    }
}

fn door_is_closed(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    !door_is_open(db, ch, obj, door)
}

fn door_is_locked(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> bool {
    !door_is_unlocked(db, ch, obj, door)
}

fn door_key(db: &DB, ch: &CharData, obj: Option<&ObjData>, door: Option<usize>) -> ObjVnum {
    if let Some(obj) = obj {
        obj.get_obj_val(2) as ObjVnum
    } else {
        db.exit(ch, door.unwrap()).as_ref().unwrap().key
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_gen_door(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut dooro: Option<usize> = None;
    let argument = argument.trim_start();
    if argument.is_empty() {
        send_to_char(
            &mut game.descriptors,
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
    if generic_find(
        &game.descriptors,
        chars,
        db,
        objs,
        &type_,
        FindFlags::OBJ_INV | FindFlags::OBJ_ROOM,
        ch,
        &mut victim,
        &mut obj,
    )
    .is_empty()
    {
        let dooroi = find_door(
            &mut game.descriptors,
            db,
            ch,
            &type_,
            &dir,
            CMD_DOOR[subcmd as usize],
        );
        dooro = dooroi.map(|dooroi| dooroi as usize);
    }

    if obj.is_some() || dooro.is_some() {
        let obj_id = obj.map(|o| o.id());
        let ch = chars.get(chid);
        let keynum = door_key(db, ch, obj, dooro);
        #[allow(clippy::blocks_in_conditions)]
        if !door_is_openable(db, ch, obj, dooro) {
            act(
                &mut game.descriptors,
                chars,
                db,
                "You can't $F that!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Str(CMD_DOOR[subcmd as usize])),
                TO_CHAR,
            );
        } else if !door_is_open(db, ch, obj, dooro)
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_OPEN)
        {
            send_to_char(&mut game.descriptors, ch, "But it's already closed!\r\n");
        } else if !door_is_closed(db, ch, obj, dooro)
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_CLOSED)
        {
            send_to_char(&mut game.descriptors, ch, "But it's currently open!\r\n");
        } else if !(door_is_locked(db, ch, obj, dooro))
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_LOCKED)
        {
            send_to_char(
                &mut game.descriptors,
                ch,
                "Oh.. it wasn't locked, after all..\r\n",
            );
        } else if !(door_is_unlocked(db, ch, obj, dooro))
            && is_set!(FLAGS_DOOR[subcmd as usize], NEED_UNLOCKED)
        {
            send_to_char(&mut game.descriptors, ch, "It seems to be locked.\r\n");
        } else if !has_key(db, objs, ch, keynum)
            && (ch.get_level() < LVL_GOD as u8)
            && ((subcmd == SCMD_LOCK) || (subcmd == SCMD_UNLOCK))
        {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You don't seem to have the proper key.\r\n",
            );
        } else if {
            let pickproof = door_is_pickproof(db, ch, obj, dooro);
            ok_pick(
                &mut game.descriptors,
                chars,
                chid,
                keynum,
                pickproof,
                subcmd,
            )
        } {
            do_doorcmd(
                &mut game.descriptors,
                db,
                chars,
                texts,
                objs,
                chid,
                obj_id,
                dooro,
                subcmd,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_enter(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf = String::new();
    one_argument(argument, &mut buf);

    if !buf.is_empty() {
        /* an argument was supplied, search for door keyword */
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some()
                && !db.exit(ch, door).as_ref().unwrap().keyword.is_empty()
                && db.exit(ch, door).as_ref().unwrap().keyword.as_ref() == buf
            {
                perform_move(game, db, chars, texts, objs, chid, door as i32, true);
                return;
            }
        }
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("There is no {} here.\r\n", buf).as_str(),
        );
    } else if db.room_flagged(ch.in_room(), RoomFlags::INDOORS) {
        send_to_char(&mut game.descriptors, ch, "You are already indoors.\r\n");
    } else {
        /* try to locate an entrance */
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some()
                && db.exit(ch, door).as_ref().unwrap().to_room != NOWHERE
                && !db
                    .exit(ch, door)
                    .as_ref()
                    .unwrap()
                    .exit_flagged(ExitFlags::CLOSED)
                && db.room_flagged(
                    db.exit(ch, door).as_ref().unwrap().to_room,
                    RoomFlags::INDOORS,
                )
            {
                perform_move(game, db, chars, texts, objs, chid, door as i32, true);
                return;
            }
        }
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can't seem to find anything to enter.\r\n",
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_leave(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if db.outside(ch) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are outside.. where do you want to go?\r\n",
        );
    } else {
        for door in 0..NUM_OF_DIRS {
            if db.exit(ch, door).is_some()
                && db.exit(ch, door).as_ref().unwrap().to_room != NOWHERE
                && !db
                    .exit(ch, door)
                    .as_ref()
                    .unwrap()
                    .exit_flagged(ExitFlags::CLOSED)
                && !db.room_flagged(
                    db.exit(ch, door).as_ref().unwrap().to_room,
                    RoomFlags::INDOORS,
                )
            {
                perform_move(game, db, chars, texts, objs, chid, door as i32, true);
                return;
            }
        }
        send_to_char(
            &mut game.descriptors,
            ch,
            "I see no obvious exits to the outside.\r\n",
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_stand(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    match ch.get_pos() {
        Position::Standing => {
            send_to_char(&mut game.descriptors, ch, "You are already standing.\r\n");
        }
        Position::Sitting => {
            send_to_char(&mut game.descriptors, ch, "You stand up.\r\n");
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n clambers to $s feet.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            /* Will be sitting after a successful bash and may still be fighting. */
            ch.set_pos(if ch.fighting_id().is_some() {
                Position::Fighting
            } else {
                Position::Standing
            });
        }
        Position::Resting => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You stop resting, and stand up.\r\n",
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n stops resting, and clambers on $s feet.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Standing);
        }
        Position::Sleeping => {
            send_to_char(&mut game.descriptors, ch, "You have to wake up first!\r\n");
        }
        Position::Fighting => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "Do you not consider fighting as standing?\r\n",
            );
        }
        _ => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You stop floating around, and put your feet on the ground.\r\n",
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n stops floating around, and puts $s feet on the ground.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Standing);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_sit(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    match ch.get_pos() {
        Position::Standing => {
            send_to_char(&mut game.descriptors, ch, "You sit down.\r\n");
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n sits down.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Sitting);
        }
        Position::Sitting => {
            send_to_char(&mut game.descriptors, ch, "You're sitting already.\r\n");
        }
        Position::Resting => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You stop resting, and sit up.\r\n",
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n stops resting.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Sitting);
        }
        Position::Sleeping => {
            send_to_char(&mut game.descriptors, ch, "You have to wake up first.\r\n");
        }
        Position::Fighting => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "Sit down while fighting? Are you MAD?\r\n",
            );
        }
        _ => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You stop floating around, and sit down.\r\n",
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n stops floating around, and sits down.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Sitting);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_rest(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    match ch.get_pos() {
        Position::Standing => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You sit down and rest your tired bones.\r\n",
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n sits down and rests.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Resting);
        }
        Position::Sitting => {
            send_to_char(&mut game.descriptors, ch, "You rest your tired bones.\r\n");
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n rests.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Resting);
        }
        Position::Resting => {
            send_to_char(&mut game.descriptors, ch, "You are already resting.\r\n");
        }
        Position::Sleeping => {
            send_to_char(&mut game.descriptors, ch, "You have to wake up first.\r\n");
        }
        Position::Fighting => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "Rest while fighting?  Are you MAD?\r\n",
            );
        }
        _ => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You stop floating around, and stop to rest your tired bones.\r\n",
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n stops floating around, and rests.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Sitting);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_sleep(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    match ch.get_pos() {
        Position::Standing | Position::Sitting | Position::Resting => {
            send_to_char(&mut game.descriptors, ch, "You go to sleep.\r\n");
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n lies down and falls asleep.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Sleeping);
        }
        Position::Sleeping => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You are already sound asleep.\r\n",
            );
        }
        Position::Fighting => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "Sleep while fighting?  Are you MAD?\r\n",
            );
        }
        _ => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You stop floating around, and lie down to sleep.\r\n",
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n stops floating around, and lie down to sleep.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Sleeping);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_wake(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();
    let vict;
    let mut self_ = false;

    one_argument(argument, &mut arg);
    if !arg.is_empty() {
        #[allow(clippy::blocks_in_conditions)]
        if ch.get_pos() == Position::Sleeping {
            send_to_char(
                &mut game.descriptors,
                ch,
                "Maybe you should wake yourself up first.\r\n",
            );
        } else if {
            vict = get_char_vis(
                &game.descriptors,
                chars,
                db,
                ch,
                &mut arg,
                None,
                FindFlags::CHAR_ROOM,
            );
            vict.is_none()
        } {
            send_to_char(&mut game.descriptors, ch, NOPERSON);
        } else if vict.unwrap().id() == chid {
            self_ = true;
        } else if vict.unwrap().awake() {
            let vict = vict.unwrap();
            act(
                &mut game.descriptors,
                chars,
                db,
                "$E is already awake.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
        } else if vict.unwrap().aff_flagged(AffectFlags::SLEEP) {
            let vict = vict.unwrap();
            act(
                &mut game.descriptors,
                chars,
                db,
                "You can't wake $M up!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
        } else if vict.unwrap().get_pos() < Position::Sleeping {
            let vict = vict.unwrap();
            act(
                &mut game.descriptors,
                chars,
                db,
                "$E's in pretty bad shape!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
        } else {
            let vict = vict.unwrap();
            act(
                &mut game.descriptors,
                chars,
                db,
                "You wake $M up.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "You are awakened by $n.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT | TO_SLEEP,
            );
            chars.get_mut(vict.id()).set_pos(Position::Sitting);
        }
        if !self_ {
            return;
        }
    }
    let ch = chars.get(chid);
    if ch.aff_flagged(AffectFlags::SLEEP) {
        send_to_char(&mut game.descriptors, ch, "You can't wake up!\r\n");
    } else if ch.get_pos() > Position::Sleeping {
        send_to_char(&mut game.descriptors, ch, "You are already awake...\r\n");
    } else {
        send_to_char(&mut game.descriptors, ch, "You awaken, and sit up.\r\n");
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n awakens.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let ch = chars.get_mut(chid);
        ch.set_pos(Position::Sitting);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_follow(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let leader;
    if !buf.is_empty() {
        let res = {
            leader = get_char_vis(
                &game.descriptors,
                chars,
                db,
                ch,
                &mut buf,
                None,
                FindFlags::CHAR_ROOM,
            );
            leader.is_none()
        };
        if res {
            send_to_char(&mut game.descriptors, ch, NOPERSON);
            return;
        }
    } else {
        send_to_char(&mut game.descriptors, ch, "Whom do you wish to follow?\r\n");
        return;
    }

    if ch.master.is_some() && ch.master.unwrap() == leader.unwrap().id() {
        let leader = leader.unwrap();
        act(
            &mut game.descriptors,
            chars,
            db,
            "You are already following $M.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(leader)),
            TO_CHAR,
        );
        return;
    }
    if ch.aff_flagged(AffectFlags::CHARM) && (ch.master.is_some()) {
        let master_id = ch.master.unwrap();
        let master = chars.get(master_id);
        act(
            &mut game.descriptors,
            chars,
            db,
            "But you only feel like following $N!",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(master)),
            TO_CHAR,
        );
    } else {
        /* Not Charmed follow person */
        if leader.unwrap().id() == chid {
            if ch.master.is_none() {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "You are already following yourself.\r\n",
                );
                return;
            }
            stop_follower(&mut game.descriptors, chars, db, objs, chid);
        } else {
            if circle_follow(chars, ch, leader) {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "Sorry, but following in loops is not allowed.\r\n",
                );
                return;
            }
            let leader_id = leader.unwrap().id();
            if ch.master.is_some() {
                stop_follower(&mut game.descriptors, chars, db, objs, chid);
            }
            let ch = chars.get_mut(chid);
            ch.remove_aff_flags(AffectFlags::GROUP);

            add_follower(&mut game.descriptors, chars, db, chid, leader_id);
        }
    }
}
