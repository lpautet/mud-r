/* ************************************************************************
*   File: act.wizard.c                                  Part of CircleMUD *
*  Usage: Player-level god commands and other goodies                     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use std::borrow::Borrow;
use std::cmp::{max, min};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::rc::Rc;
use std::{mem, slice};

use crate::act_informative::look_at_room;
use crate::class::{
    do_start, level_exp, parse_class, roll_real_abils, CLASS_ABBREVS, PC_CLASS_TYPES,
};
use crate::config::{LOAD_INTO_INVENTORY, NOPERSON, OK};
use crate::constants::{
    ACTION_BITS, AFFECTED_BITS, APPLY_TYPES, CONNECTED_TYPES, CONTAINER_BITS, DEX_APP, DIRS,
    DRINKS, EXIT_BITS, EXTRA_BITS, GENDERS, INT_APP, ITEM_TYPES, NPC_CLASS_TYPES, PLAYER_BITS,
    POSITION_TYPES, PREFERENCE_BITS, ROOM_BITS, SECTOR_TYPES, WEAR_BITS, WIS_APP,
};
use crate::db::{
    clear_char, parse_c_string, store_to_char, DB, FASTBOOT_FILE, KILLSCRIPT_FILE, PAUSE_FILE, REAL,
};
use crate::depot::{DepotId, HasId};
use crate::fight::{update_pos, ATTACK_HIT_TEXT};
use crate::handler::{get_number, FIND_CHAR_ROOM, FIND_CHAR_WORLD};
use crate::house::{hcontrol_list_houses, house_can_enter};
use crate::interpreter::{
    command_interpreter, delete_doubledollar, half_chop, is_abbrev, is_number, one_argument,
    search_block, two_arguments, SCMD_DATE, SCMD_EMOTE, SCMD_FREEZE, SCMD_NOTITLE, SCMD_PARDON,
    SCMD_POOFIN, SCMD_POOFOUT, SCMD_REROLL, SCMD_SHUTDOWN, SCMD_SQUELCH, SCMD_THAW, SCMD_UNAFFECT,
};
use crate::limits::{gain_exp_regardless, hit_gain, mana_gain, move_gain, set_title};
use crate::modify::page_string;
use crate::objsave::crash_listrent;
use crate::screen::{C_NRM, KCYN, KGRN, KNRM, KNUL, KYEL};
use crate::shops::show_shops;
use crate::spell_parser::skill_name;
use crate::structs::ConState::{ConClose, ConDisconnect, ConPlaying};
use crate::structs::{
    CharData, CharFileU, RoomRnum, RoomVnum, ZoneRnum, AFF_HIDE, AFF_INVISIBLE, CLASS_UNDEFINED,
    DRUNK, FULL, ITEM_ARMOR, ITEM_CONTAINER, ITEM_DRINKCON, ITEM_FOOD, ITEM_FOUNTAIN, ITEM_KEY,
    ITEM_LIGHT, ITEM_MONEY, ITEM_NOTE, ITEM_POTION, ITEM_SCROLL, ITEM_STAFF, ITEM_TRAP, ITEM_WAND,
    ITEM_WEAPON, LVL_FREEZE, LVL_GOD, LVL_GRGOD, LVL_IMMORT, LVL_IMPL, MAX_OBJ_AFFECT, MAX_SKILLS,
    NOBODY, NOTHING, NOWHERE, NUM_OF_DIRS, NUM_WEARS, PLR_DELETED, PLR_FROZEN, PLR_INVSTART,
    PLR_KILLER, PLR_LOADROOM, PLR_MAILING, PLR_NODELETE, PLR_NOSHOUT, PLR_NOTITLE, PLR_NOWIZLIST,
    PLR_SITEOK, PLR_THIEF, PLR_WRITING, PRF_BRIEF, PRF_COLOR_1, PRF_COLOR_2, PRF_HOLYLIGHT,
    PRF_LOG1, PRF_LOG2, PRF_NOHASSLE, PRF_NOREPEAT, PRF_NOWIZ, PRF_QUEST, PRF_ROOMFLAGS,
    PRF_SUMMONABLE, ROOM_DEATH, ROOM_GODROOM, ROOM_HOUSE, ROOM_PRIVATE, THIRST,
};
use crate::util::{
    age, clone_vec2, ctime, hmhr, sprintbit, sprinttype, time_now, touch, BRF, NRM,
    SECS_PER_MUD_YEAR,
};
use crate::VictimRef;
use crate::{
    _clrlevel, clr, onoff, yesno, Game, CCCYN, CCGRN, CCNRM, CCYEL, TO_CHAR, TO_NOTVICT, TO_ROOM,
    TO_VICT,
};
use chrono::{TimeZone, Utc};
use hmac::Hmac;
use log::{error, info};
use sha2::Sha256;

pub fn do_echo(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let argument = argument.trim_start();

    if argument.is_empty() {
        game.send_to_char(db,chid, "Yes.. but what?\r\n");
    } else {
        let buf;
        if subcmd == SCMD_EMOTE {
            buf = format!("$n {}", argument);
        } else {
            buf = argument.to_string();
        }

        game.act(db,&buf, false, Some(chid), None, None, TO_ROOM);
        let ch = db.ch(chid);
        if ch.prf_flagged(PRF_NOREPEAT) {
            game.send_to_char(db,chid, OK);
        } else {
            game.act(db,&buf, false, Some(chid), None, None, TO_CHAR);
        }
    }
}

pub fn do_send(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut argument = argument.to_string();
    let mut arg = String::new();
    let mut buf = String::new();
    let vict_id;

    half_chop(&mut argument, &mut arg, &mut buf);

    if argument.is_empty() {
        game.send_to_char(db,chid, "Send what to who?\r\n");
        return;
    }
    if {
        vict_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_WORLD);
        vict_id.is_none()
    } {
        game.send_to_char(db,chid, NOPERSON);
        return;
    }
    game.send_to_char(db,vict_id.unwrap(), format!("{}\r\n", buf).as_str());
    let ch = db.ch(chid);
    if ch.prf_flagged(PRF_NOREPEAT) {
        game.send_to_char(db,chid, "Sent.\r\n");
    } else {
        game.send_to_char(db,
            chid,
            format!(
                "You send '{}' to {}.\r\n",
                buf,
                db.ch(vict_id.unwrap()).get_name()
            )
            .as_str(),
        );
    }
}

/* take a string, and return an rnum.. used for goto, at, etc.  -je 4/6/93 */
fn find_target_room(game: &mut Game, db: &DB, chid: DepotId, rawroomstr: &str) -> RoomRnum {
    let ch = db.ch(chid);

    let mut location = NOWHERE;
    let mut roomstr = String::new();
    one_argument(rawroomstr, &mut roomstr);

    if roomstr.is_empty() {
        game.send_to_char(db,chid, "You must supply a room number or name.\r\n");
        return NOWHERE;
    }

    if roomstr.chars().next().unwrap().is_digit(10) && !roomstr.contains('.') {
        if {
            location = db.real_room(roomstr.parse::<i16>().unwrap());
            location == NOWHERE
        } {
            game.send_to_char(db,chid, "No room exists with that number.\r\n");
            return NOWHERE;
        }
    } else {
        let target_mob_id;
        let target_obj_id;
        let mut mobobjstr = roomstr;

        let mut num = get_number(&mut mobobjstr);
        if {
            target_mob_id =
                game.get_char_vis(db,chid, &mut mobobjstr, Some(&mut num), FIND_CHAR_WORLD);
            target_mob_id.is_some()
        } {
            if {
                location = db.ch(target_mob_id.unwrap()).in_room();
                location == NOWHERE
            } {
                game.send_to_char(db,chid, "That character is currently lost.\r\n");
                return NOWHERE;
            }
        } else if {
            target_obj_id = game.get_obj_vis(db,ch, &mut mobobjstr, Some(&mut num));
            target_obj_id.is_some()
        } {
            if db.obj(target_obj_id.unwrap()).in_room() != NOWHERE {
                location = db.obj(target_obj_id.unwrap()).in_room();
            } else if 
                db
                .obj(target_obj_id.unwrap())
                .carried_by
                .borrow()
                .is_some()
                && 
                    db
                    .ch(db.obj(target_obj_id.unwrap()).carried_by.unwrap())
                    .in_room()
                    != NOWHERE
            {
                location = 
                    db
                    .ch(db.obj(target_obj_id.unwrap()).carried_by.unwrap())
                    .in_room();
            } else if 
                db
                .obj(target_obj_id.unwrap())
                .worn_by
                .borrow()
                .is_some()
                && 
                    db
                    .ch(db.obj(target_obj_id.unwrap()).worn_by.unwrap())
                    .in_room()
                    != NOWHERE
            {
                location = 
                    db
                    .ch(db.obj(target_obj_id.unwrap()).worn_by.unwrap())
                    .in_room();
            }

            if location == NOWHERE {
                game.send_to_char(db,chid, "That object is currently not in a room.\r\n");
                return NOWHERE;
            }
        }

        if location == NOWHERE {
            game.send_to_char(db,chid, "Nothing exists by that name.\r\n");
            return NOWHERE;
        }
    }

    /* a location has been found -- if you're >= GRGOD, no restrictions. */
    if ch.get_level() >= LVL_GRGOD as u8 {
        return location;
    }

    if db.room_flagged(location, ROOM_GODROOM) {
        game.send_to_char(db,chid, "You are not godly enough to use that room!\r\n");
    } else if db.room_flagged(location, ROOM_PRIVATE)
        && db.world[location as usize].peoples.len() > 1
    {
        game.send_to_char(db,
            chid,
            "There's a private conversation going on in that room.\r\n",
        );
    } else if db.room_flagged(location, ROOM_HOUSE)
        && !house_can_enter(&db, ch, db.get_room_vnum(location))
    {
        game.send_to_char(db,chid, "That's private property -- no trespassing!\r\n");
    } else {
        return location;
    }

    return NOWHERE;
}

pub fn do_at(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut argument = argument.to_string();
    let mut buf = String::new();
    let mut command = String::new();

    half_chop(&mut argument, &mut buf, &mut command);
    if buf.is_empty() {
        game.send_to_char(db,chid, "You must supply a room number or a name.\r\n");
        return;
    }

    if command.is_empty() {
        game.send_to_char(db,chid, "What do you want to do there?\r\n");
        return;
    }
    let location;
    if {
        location = find_target_room(game, db,chid, &buf);
        location == NOWHERE
    } {
        return;
    }

    /* a location has been found. */
    let ch = db.ch(chid);
    let original_loc = ch.in_room();
    db.char_from_room(chid);
    db.char_to_room(chid, location);
    command_interpreter(game, db, chid, &command);

    /* check if the char is still there */
    let ch = db.ch(chid);
    if ch.in_room() == location {
        db.char_from_room(chid);
        db.char_to_room(chid, original_loc);
    }
}

pub fn do_goto(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let location;

    if {
        location = find_target_room(game, db,chid, argument);
        location == NOWHERE
    } {
        return;
    }
    let ch = db.ch(chid);
    let x = ch.poofout();
    let buf = format!(
        "$n {}",
        if !x.is_empty() {
            x.as_ref()
        } else {
            "disappears in a puff of smoke."
        }
    );
    game.act(db,&buf, true, Some(chid), None, None, TO_ROOM);

    db.char_from_room(chid);
    db.char_to_room(chid, location);
    let ch = db.ch(chid);
    let x = ch.poofin();
    let buf = format!(
        "$n {}",
        if !x.is_empty() {
            x.as_ref()
        } else {
            "appears with an ear-splitting bang."
        }
    );
    game.act(db,&buf, true, Some(chid), None, None, TO_ROOM);

    look_at_room(game, db,chid, false);
}

pub fn do_trans(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let victim_id;
    if buf.is_empty() {
        game.send_to_char(db,chid, "Whom do you wish to transfer?\r\n");
    } else if "all" != buf {
        if {
            victim_id = game.get_char_vis(db,chid, &mut buf, None, FIND_CHAR_WORLD);
            victim_id.is_none()
        } {
            game.send_to_char(db,chid, NOPERSON);
        } else if victim_id.unwrap() == chid {
            game.send_to_char(db,chid, "That doesn't make much sense, does it?\r\n");
        } else {
            let victim_id = victim_id.unwrap();
            let victim = db.ch(victim_id);
            if (ch.get_level() < victim.get_level()) && !victim.is_npc() {
                game.send_to_char(db,chid, "Go transfer someone your own size.\r\n");
                return;
            }
            game.act(db,
                "$n disappears in a mushroom cloud.",
                false,
                Some(victim_id),
                None,
                None,
                TO_ROOM,
            );
            db.char_from_room(victim_id);
            let ch = db.ch(chid);
            db.char_to_room(victim_id, ch.in_room());
            game.act(db,
                "$n arrives from a puff of smoke.",
                false,
                Some(victim_id),
                None,
                None,
                TO_ROOM,
            );
            game.act(db,
                "$n has transferred you!",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(victim_id)),
                TO_VICT,
            );
            look_at_room(game, db,victim_id, false);
        }
    } else {
        /* Trans All */
        if ch.get_level() < LVL_GRGOD as u8 {
            game.send_to_char(db,chid, "I think not.\r\n");
            return;
        }

        let list = game.descriptor_list.ids();
        for i in list {
            if game.descriptor_list.get(i).state() == ConPlaying
                && game.descriptor_list.get(i).character.is_some()
                && game.descriptor_list.get(i).character.unwrap() != chid
            {
                let ic = game.descriptor_list.get(i).character;
                let victim_id = ic.unwrap();
                let victim = db.ch(victim_id);
                let ch = db.ch(chid);
                if victim.get_level() >= ch.get_level() {
                    continue;
                }
                game.act(db,
                    "$n disappears in a mushroom cloud.",
                    false,
                    Some(victim.id()),
                    None,
                    None,
                    TO_ROOM,
                );
                db.char_from_room(victim_id);
                let ch = db.ch(chid);
                db.char_to_room(victim_id, ch.in_room());
                game.act(db,
                    "$n arrives from a puff of smoke.",
                    false,
                    Some(victim_id),
                    None,
                    None,
                    TO_ROOM,
                );
                game.act(db,
                    "$n has transferred you!",
                    false,
                    Some(chid),
                    None,
                    Some(VictimRef::Char(victim_id)),
                    TO_VICT,
                );
                look_at_room(game, db,victim_id, false);
            }
        }
    }
    game.send_to_char(db,chid, OK);
}

pub fn do_teleport(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);
    let victim_id;
    let victim;
    let target;
    if buf.is_empty() {
        game.send_to_char(db,chid, "Whom do you wish to teleport?\r\n");
    } else if {
        victim_id = game.get_char_vis(db,chid, &mut buf, None, FIND_CHAR_WORLD);
        victim = victim_id.map(|v| db.ch(v));
        victim_id.is_none()
    } {
        game.send_to_char(db,chid, NOPERSON);
    } else if victim_id.unwrap() == chid {
        game.send_to_char(db,chid, "Use 'goto' to teleport yourself.\r\n");
    } else if victim.as_ref().unwrap().get_level() >= ch.get_level() {
        game.send_to_char(db,chid, "Maybe you shouldn't do that.\r\n");
    } else if buf2.is_empty() {
        game.send_to_char(db,chid, "Where do you wish to send this person?\r\n");
    } else if {
        target = find_target_room(game, db,chid, &buf2);
        target != NOWHERE
    } {
        game.send_to_char(db,chid, OK);
        game.act(db,
            "$n disappears in a puff of smoke.",
            false,
            Some(victim_id.unwrap()),
            None,
            None,
            TO_ROOM,
        );
        db.char_from_room(victim_id.unwrap());
        db.char_to_room(victim_id.unwrap(), target);
        game.act(db,
            "$n arrives from a puff of smoke.",
            false,
            Some(victim_id.unwrap()),
            None,
            None,
            TO_ROOM,
        );
        game.act(db,
            "$n has teleported you!",
            false,
            Some(chid),
            None,
            Some(VictimRef::Char(victim_id.unwrap())),
            TO_VICT,
        );
        look_at_room(game, db,victim_id.unwrap(), false);
    }
}

pub fn do_vnum(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !is_abbrev(&buf, "mob") && !is_abbrev(&buf, "obj") {
        game.send_to_char(db,chid, "Usage: vnum { obj | mob } <name>\r\n");
        return;
    }
    if is_abbrev(&buf, "mob") {
        if game.vnum_mobile(db,&buf2, chid) == 0 {
            game.send_to_char(db,chid, "No mobiles by that name.\r\n");
        }
    }

    if is_abbrev(&buf, "obj") {
        if game.vnum_object(db,&buf2, chid) == 0 {
            game.send_to_char(db,chid, "No objects by that name.\r\n");
        }
    }
}

fn do_stat_room(game: &mut Game, db: &mut DB, chid: DepotId) {
    let ch = db.ch(chid);

    let rm_name = db.world[ch.in_room() as usize].name.clone();
    let rm_zone = db.world[ch.in_room() as usize].zone;
    let rm_sector_type = db.world[ch.in_room() as usize].sector_type;
    let rm_number = db.world[ch.in_room() as usize].number;
    let rm_func_is_none = db.world[ch.in_room() as usize].func.is_none();
    let rm_description = db.world[ch.in_room() as usize].description.clone();
    let rm_ex_descriptions_len = db.world[ch.in_room() as usize].ex_descriptions.len();
    let rm_peoples = clone_vec2(&db.world[ch.in_room() as usize].peoples);
    let rm_contents = clone_vec2(&db.world[ch.in_room() as usize].contents);
    let rm_room_flags = db.world[ch.in_room() as usize].room_flags;
    let rm_dir_option = db.world[ch.in_room() as usize].dir_option.clone();

    game.send_to_char(db,
        chid,
        format!(
            "Room name: {}{}{}\r\n",
            CCCYN!(ch, C_NRM),
            rm_name,
            CCNRM!(ch, C_NRM)
        )
        .as_str(),
    );
    let mut buf2 = String::new();
    sprinttype(rm_sector_type, &SECTOR_TYPES, &mut buf2);
    let ch = db.ch(chid);
    game.send_to_char(db,
        chid,
        format!(
            "Zone: [{:3}], VNum: [{}{:5}{}], RNum: [{:5}], Type: {}\r\n",
            db.zone_table[rm_zone as usize].number,
            CCGRN!(ch, C_NRM),
            rm_number,
            CCNRM!(ch, C_NRM),
            ch.in_room(),
            buf2
        )
        .as_str(),
    );

    sprintbit(rm_room_flags as i64, &ROOM_BITS, &mut buf2);
    game.send_to_char(db,
        chid,
        format!(
            "SpecProc: {}, Flags: {}\r\n",
            if rm_func_is_none { "None" } else { "Exists" },
            buf2
        )
        .as_str(),
    );

    game.send_to_char(db,
        chid,
        format!(
            "Description:\r\n{}",
            if !rm_description.is_empty() {
                &rm_description
            } else {
                "  None.\r\n"
            }
        )
        .as_str(),
    );

    if rm_ex_descriptions_len != 0 {
        let ch = db.ch(chid);
        game.send_to_char(db,chid, format!("Extra descs:{}", CCCYN!(ch, C_NRM)).as_str());
        for idx in 0..rm_ex_descriptions_len {
            let ch = db.ch(chid);
            let desc_keyword = db.world[ch.in_room() as usize].ex_descriptions[idx]
                .keyword
                .clone();

            game.send_to_char(db,chid, format!(" {}", desc_keyword).as_str());
            let ch = db.ch(chid);
            game.send_to_char(db,chid, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());
        }
    }

    if rm_peoples.len() != 0 {
        let ch = db.ch(chid);
        game.send_to_char(db,
            chid,
            format!("Chars present:{}", CCYEL!(ch, C_NRM)).as_str(),
        );
        let mut column = 14; /* ^^^ strlen ^^^ */
        let mut found = 0;
        for (i, k_id) in rm_peoples.iter().enumerate() {
            let k = db.ch(*k_id);
            let ch = db.ch(chid);
            if !game.can_see(db,ch, k) {
                continue;
            }

            column += game.send_to_char(db,
                chid,
                format!(
                    "{} {}({})",
                    if found != 0 { "," } else { "" },
                    k.get_name(),
                    if !k.is_npc() {
                        "PC"
                    } else {
                        if !db.is_mob(k) {
                            "NPC"
                        } else {
                            "MOB"
                        }
                    }
                )
                .as_str(),
            );
            found += 1;
            if column >= 62 {
                game.send_to_char(db,
                    chid,
                    format!("{}\r\n", if i == rm_peoples.len() - 1 { "," } else { "" }).as_str(),
                );
                found = 0;
                column = 0;
            }
        }
        let ch = db.ch(chid);
        game.send_to_char(db,chid, CCNRM!(ch, C_NRM));
    }
    if !rm_contents.is_empty() {
        let ch = db.ch(chid);
        game.send_to_char(db,chid, format!("Contents:{}", CCGRN!(ch, C_NRM)).as_str());
        let mut column = 9; /* ^^^ strlen ^^^ */
        let mut found = 0;
        for (i, oid) in rm_contents.iter().enumerate() {
            let ch = db.ch(chid);
            if !game.can_see_obj(db,ch, db.obj(*oid)) {
                continue;
            }

            column += game.send_to_char(db,
                chid,
                format!(
                    "{} {}",
                    if found != 0 { "," } else { "" },
                    db.obj(*oid).short_description
                )
                .as_str(),
            );
            found += 1;
            if column >= 62 {
                game.send_to_char(db,
                    chid,
                    format!("{}\r\n", if i == rm_contents.len() - 1 { "," } else { "" }).as_str(),
                );
                found = 0;
                column = 0;
            }
        }
        let ch = db.ch(chid);
        game.send_to_char(db,chid, format!("{}", CCNRM!(ch, C_NRM)).as_str());
    }

    for i in 0..NUM_OF_DIRS {
        if rm_dir_option[i].is_none() {
            continue;
        }
        let buf1;
        if rm_dir_option[i].as_ref().unwrap().to_room == NOWHERE {
            let ch = db.ch(chid);
            buf1 = format!(" {}NONE{}", CCCYN!(ch, C_NRM), CCNRM!(ch, C_NRM));
        } else {
            let ch = db.ch(chid);
            buf1 = format!(
                "{}{:5}{}",
                CCCYN!(ch, C_NRM),
                db
                    .get_room_vnum(rm_dir_option[i].as_ref().unwrap().to_room),
                CCNRM!(ch, C_NRM)
            );
        }
        let mut buf2 = String::new();
        sprintbit(
            rm_dir_option[i].as_ref().unwrap().exit_info as i64,
            &EXIT_BITS,
            &mut buf2,
        );
        let ch = db.ch(chid);
        format!(
            "Exit {}{:5}{}:  To: [{}], Key: [{:5}], Keywrd: {}, Type: {}\r\n{}",
            CCCYN!(ch, C_NRM),
            DIRS[i],
            CCNRM!(ch, C_NRM),
            buf1,
            rm_dir_option[i].as_ref().unwrap().key,
            if !rm_dir_option[i].as_ref().unwrap().keyword.is_empty() {
                &rm_dir_option[i].as_ref().unwrap().keyword
            } else {
                "None"
            },
            buf2,
            if !rm_dir_option[i]
                .as_ref()
                .unwrap()
                .general_description
                .is_empty()
            {
                &rm_dir_option[i].as_ref().unwrap().general_description
            } else {
                "  No exit description.\r\n"
            }
        );
        let msg = format!(
            "Exit {}{:5}{}:  To: [{}], Key: [{:5}], Keywrd: {}, Type: {}\r\n{}",
            CCCYN!(ch, C_NRM),
            DIRS[i],
            CCNRM!(ch, C_NRM),
            buf1,
            rm_dir_option[i].as_ref().unwrap().key,
            if !rm_dir_option[i].as_ref().unwrap().keyword.is_empty() {
                &rm_dir_option[i].as_ref().unwrap().keyword
            } else {
                "None"
            },
            buf2,
            if !rm_dir_option[i]
                .as_ref()
                .unwrap()
                .general_description
                .is_empty()
            {
                &rm_dir_option[i].as_ref().unwrap().general_description
            } else {
                "  No exit description.\r\n"
            }
        );
        game.send_to_char(db,chid, msg.as_str());
    }
}

fn do_stat_object(game: &mut Game, db: &mut DB, chid: DepotId, oid: DepotId) {
    let ch = db.ch(chid);

    let vnum = db.get_obj_vnum(db.obj(oid));
    game.send_to_char(db,
        chid,
        format!(
            "Name: '{}{}{}', Aliases: {}\r\n",
            CCYEL!(ch, C_NRM),
            if !db.obj(oid).short_description.is_empty() {
                &db.obj(oid).short_description
            } else {
                "<None>"
            },
            CCNRM!(ch, C_NRM),
            db.obj(oid).name
        )
        .as_str(),
    );
    let mut buf = String::new();
    sprinttype(
        db.obj(oid).get_obj_type() as i32,
        &ITEM_TYPES,
        &mut buf,
    );
    let ch = db.ch(chid);
    game.send_to_char(db,
        chid,
        format!(
            "VNum: [{}{:5}{}], RNum: [{:5}], Type: {}, SpecProc: {}\r\n",
            CCGRN!(ch, C_NRM),
            vnum,
            CCNRM!(ch, C_NRM),
            db.obj(oid).get_obj_rnum(),
            buf,
            if db.get_obj_spec(oid).is_some() {
                "Exists"
            } else {
                "none"
            }
        )
        .as_str(),
    );

    if !db.obj(oid).ex_descriptions.is_empty() {
        let ch = db.ch(chid);
        game.send_to_char(db,chid, format!("Extra descs:{}", CCCYN!(ch, C_NRM)).as_str());

        for desc in db.obj(oid).ex_descriptions.clone().iter() {
            game.send_to_char(db,chid, format!(" {}", desc.keyword).as_str());
            let ch = db.ch(chid);
            game.send_to_char(db,chid, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());
        }
    }
    buf.clear();
    sprintbit(db.obj(oid).get_obj_wear() as i64, &WEAR_BITS, &mut buf);
    game.send_to_char(db,chid, format!("Can be worn on: {}\r\n", buf).as_str());
    buf.clear();
    sprintbit(db.obj(oid).get_obj_affect(), &AFFECTED_BITS, &mut buf);
    game.send_to_char(db,chid, format!("Set char bits : {}\r\n", buf).as_str());
    buf.clear();
    sprintbit(
        db.obj(oid).get_obj_extra() as i64,
        &EXTRA_BITS,
        &mut buf,
    );
    game.send_to_char(db,chid, format!("Extra flags   : {}\r\n", buf).as_str());

    game.send_to_char(db,
        chid,
        format!(
            "Weight: {}, Value: {}, Cost/day: {}, Timer: {}\r\n",
            db.obj(oid).get_obj_weight(),
            db.obj(oid).get_obj_cost(),
            db.obj(oid).get_obj_rent(),
            db.obj(oid).get_obj_timer()
        )
        .as_str(),
    );
    game.send_to_char(db,
        chid,
        format!(
            "In room: {} ({}), ",
            db.get_room_vnum(db.obj(oid).in_room()),
            if db.obj(oid).in_room() == NOWHERE {
                "Nowhere"
            } else {
                db.world[db.obj(oid).in_room() as usize]
                    .name
                    .as_str()
            }
        )
        .as_str(),
    );

    /*
     * NOTE: In order to make it this far, we must already be able to see the
     *       character holding the object. Therefore, we do not need CAN_SEE().
     */
    let jio = db.obj(oid).in_obj.borrow();
    game.send_to_char(db,
        chid,
        format!(
            "In object: {}, ",
            if db.obj(oid).in_obj.borrow().is_some() {
                db.obj(jio.unwrap()).short_description.as_ref()
            } else {
                "None"
            }
        )
        .as_str(),
    );
    game.send_to_char(db,
        chid,
        format!(
            "Carried by: {}, ",
            if db.obj(oid).carried_by.is_some() {
                db.ch(db.obj(oid).carried_by.unwrap()).get_name().as_ref()
            } else {
                "Nobody"
            }
        )
        .as_str(),
    );
    game.send_to_char(db,
        chid,
        format!(
            "Worn by: {}\r\n",
            if db.obj(oid).worn_by.is_some() {
                db.ch(db.obj(oid).worn_by.unwrap()).get_name().as_ref()
            } else {
                "Nobody"
            }
        )
        .as_str(),
    );

    match db.obj(oid).get_obj_type() {
        ITEM_LIGHT => {
            if db.obj(oid).get_obj_val(2) == -1 {
                game.send_to_char(db,chid, "Hours left: Infinite\r\n");
            } else {
                game.send_to_char(db,
                    chid,
                    format!("Hours left: [{}]\r\n", db.obj(oid).get_obj_val(2)).as_str(),
                );
            }
        }
        ITEM_SCROLL | ITEM_POTION => {
            game.send_to_char(db,
                chid,
                format!(
                    "Spells: (Level {}) {}, {}, {}\r\n",
                    db.obj(oid).get_obj_val(0),
                    skill_name(&db, db.obj(oid).get_obj_val(1)),
                    skill_name(&db, db.obj(oid).get_obj_val(2)),
                    skill_name(&db, db.obj(oid).get_obj_val(3))
                )
                .as_str(),
            );
        }
        ITEM_WAND | ITEM_STAFF => {
            game.send_to_char(db,
                chid,
                format!(
                    "Spell: {} at level {}, {} (of {}) charges remaining\r\n",
                    skill_name(&db, db.obj(oid).get_obj_val(3)),
                    db.obj(oid).get_obj_val(0),
                    db.obj(oid).get_obj_val(2),
                    db.obj(oid).get_obj_val(1)
                )
                .as_str(),
            );
        }
        ITEM_WEAPON => {
            game.send_to_char(db,
                chid,
                format!(
                    "Todam: {}d{}, Message type: {}\r\n",
                    db.obj(oid).get_obj_val(1),
                    db.obj(oid).get_obj_val(2),
                    db.obj(oid).get_obj_val(3)
                )
                .as_str(),
            );
        }
        ITEM_ARMOR => {
            game.send_to_char(db,
                chid,
                format!("AC-apply: [{}]\r\n", db.obj(oid).get_obj_val(0)).as_str(),
            );
        }
        ITEM_TRAP => {
            game.send_to_char(db,
                chid,
                format!(
                    "Spell: {}, - Hitpoints: {}\r\n",
                    db.obj(oid).get_obj_val(0),
                    db.obj(oid).get_obj_val(1)
                )
                .as_str(),
            );
        }
        ITEM_CONTAINER => {
            buf.clear();
            sprintbit(
                db.obj(oid).get_obj_val(1) as i64,
                &CONTAINER_BITS,
                &mut buf,
            );
            game.send_to_char(db,
                chid,
                format!(
                    "Weight capacity: {}, Lock Type: {}, Key Num: {}, Corpse: {}\r\n",
                    db.obj(oid).get_obj_val(0),
                    buf,
                    db.obj(oid).get_obj_val(2),
                    yesno!(db.obj(oid).get_obj_val(3) != 0)
                )
                .as_str(),
            );
        }
        ITEM_DRINKCON | ITEM_FOUNTAIN => {
            buf.clear();
            sprinttype(db.obj(oid).get_obj_val(2), &DRINKS, &mut buf);
            game.send_to_char(db,
                chid,
                format!(
                    "Capacity: {}, Contains: {}, Poisoned: {}, Liquid: {}\r\n",
                    db.obj(oid).get_obj_val(0),
                    db.obj(oid).get_obj_val(1),
                    yesno!(db.obj(oid).get_obj_val(3) != 0),
                    buf
                )
                .as_str(),
            );
        }
        ITEM_NOTE => {
            game.send_to_char(db,
                chid,
                format!("Tongue: {}\r\n", db.obj(oid).get_obj_val(0)).as_str(),
            );
        }
        ITEM_KEY => { /* Nothing */ }
        ITEM_FOOD => {
            game.send_to_char(db,
                chid,
                format!(
                    "Makes full: {}, Poisoned: {}\r\n",
                    db.obj(oid).get_obj_val(0),
                    yesno!(db.obj(oid).get_obj_val(3) != 0)
                )
                .as_str(),
            );
        }
        ITEM_MONEY => {
            game.send_to_char(db,
                chid,
                format!("Coins: {}\r\n", db.obj(oid).get_obj_val(0)).as_str(),
            );
        }
        _ => {
            game.send_to_char(db,
                chid,
                format!(
                    "Values 0-3: [{}] [{}] [{}] [{}]\r\n",
                    db.obj(oid).get_obj_val(0),
                    db.obj(oid).get_obj_val(1),
                    db.obj(oid).get_obj_val(2),
                    db.obj(oid).get_obj_val(3)
                )
                .as_str(),
            );
        }
    }

    /*
     * I deleted the "equipment status" code from here because it seemed
     * more or less useless and just takes up valuable screen space.
     */

    if !db.obj(oid).contains.is_empty() {
        let ch = db.ch(chid);
        game.send_to_char(db,chid, format!("\r\nContents:{}", CCGRN!(ch, C_NRM)).as_str());
        let mut column = 9; /* ^^^ strlen ^^^ */
        let mut found = 0;

        for (i2, j2) in db.obj(oid).contains.clone().iter().enumerate() {
            let messg = format!(
                "{} {}",
                if found != 0 { "," } else { "" },
                db.obj(*j2).short_description
            );
            column += game.send_to_char(db,chid, messg.as_str());
            if column >= 62 {
                let messg = format!(
                    "{}\r\n",
                    if i2 < db.obj(oid).contains.len() - 1 {
                        ","
                    } else {
                        ""
                    }
                );
                game.send_to_char(db,chid, messg.as_str());
                found = 0;
                column = 0;
            }
        }
        let ch = db.ch(chid);
        game.send_to_char(db,chid, CCNRM!(ch, C_NRM));
    }

    let mut found = 0;
    game.send_to_char(db,chid, "Affections:");

    for i in 0..MAX_OBJ_AFFECT as usize {
        if db.obj(oid).affected[i].modifier != 0 {
            buf.clear();
            sprinttype(
                db.obj(oid).affected[i].location as i32,
                &APPLY_TYPES,
                &mut buf,
            );
            game.send_to_char(db,
                chid,
                format!(
                    "{} {} to {}",
                    if found != 0 { "," } else { "" },
                    db.obj(oid).affected[i].modifier,
                    buf
                )
                .as_str(),
            );
            found += 1;
        }
        if found == 0 {
            game.send_to_char(db,chid, " None");
        }
        game.send_to_char(db,chid, "\r\n");
    }
}

fn do_stat_character(game: &mut Game, db: &mut DB, chid: DepotId, k_id: DepotId) {
    let k = db.ch(k_id);

    let mut buf = String::new();
    sprinttype(k.get_sex() as i32, &GENDERS, &mut buf);
    game.send_to_char(db,
        chid,
        format!(
            "{} {} '{}'  IDNum: [{:5}], In room [{:5}]\r\n",
            buf,
            if !k.is_npc() {
                "PC"
            } else {
                if !db.is_mob(k) {
                    "NPC"
                } else {
                    "MOB"
                }
            },
            k.get_name(),
            k.get_idnum(),
            db.get_room_vnum(k.in_room())
        )
        .as_str(),
    );
    let k = db.ch(k_id);
    if db.is_mob(k) {
        game.send_to_char(db,
            chid,
            format!(
                "Alias: {}, VNum: [{:5}], RNum: [{:5}]\r\n",
                k.player.name,
                db.get_mob_vnum(k),
                k.get_mob_rnum()
            )
            .as_str(),
        );
    }
    let k = db.ch(k_id);
    let mut title = &Rc::from("<None>");
    let mut long_descr = &Rc::from("<None>");
    {
        let player = &k.player;
        if player.title.is_some() {
            title = player.title.as_ref().unwrap();
        }
        if !player.long_descr.is_empty() {
            long_descr = &player.long_descr;
        }
    }
    let messg_title = format!("Title: {}\r\n", title);
    let messg_descr = format!("L-Des: {}", long_descr);

    game.send_to_char(db,chid, messg_title.as_str());
    game.send_to_char(db,chid, messg_descr.as_str());
    let k = db.ch(k_id);
    buf.clear();
    sprinttype(
        k.player.chclass as i32,
        if k.is_npc() {
            &NPC_CLASS_TYPES
        } else {
            &PC_CLASS_TYPES
        },
        &mut buf,
    );
    let ch = db.ch(chid);
    let k = db.ch(k_id);
    game.send_to_char(db,
        chid,
        format!(
            "{}Class: {}, Lev: [{}{:2}{}], XP: [{}{:7}{}], Align: [{:4}]\r\n",
            if k.is_npc() { "Monster " } else { "" },
            buf,
            CCYEL!(ch, C_NRM),
            k.get_level(),
            CCNRM!(ch, C_NRM),
            CCYEL!(ch, C_NRM),
            k.get_exp(),
            CCNRM!(ch, C_NRM),
            k.get_alignment()
        )
        .as_str(),
    );
    let k = db.ch(k_id);

    if !k.is_npc() {
        let buf1 = ctime(k.player.time.birth);
        let buf2 = ctime(k.player.time.logon);
        game.send_to_char(db,
            chid,
            format!(
                "Created: [{}], Last Logon: [{}], Played [{}h {}m], Age [{}]\r\n",
                buf1,
                buf2,
                k.player.time.played / 3600,
                ((k.player.time.played % 3600) / 60),
                age(k).year
            )
            .as_str(),
        );
        let k = db.ch(k_id);
        game.send_to_char(db,
            chid,
            format!(
                "Hometown: [{}], Speaks: [{}/{}/{}], (STL[{}]/per[{}]/NSTL[{}])\r\n",
                k.player.hometown,
                k.get_talk_mut(0),
                k.get_talk_mut(1),
                k.get_talk_mut(2),
                k.get_practices(),
                INT_APP[k.get_int() as usize].learn,
                WIS_APP[k.get_wis() as usize].bonus
            )
            .as_str(),
        );
    }
    let ch = db.ch(chid);
    let k = db.ch(k_id);
    game.send_to_char(db,
        chid,
        format!(
            "Str: [{}{}/{}{}]  Int: [{}{}{}]  Wis: [{}{}{}]  \
Dex: [{}{}{}]  Con: [{}{}{}]  Cha: [{}{}{}]\r\n",
            CCCYN!(ch, C_NRM),
            k.get_str(),
            k.get_add(),
            CCNRM!(ch, C_NRM),
            CCCYN!(ch, C_NRM),
            k.get_int(),
            CCNRM!(ch, C_NRM),
            CCCYN!(ch, C_NRM),
            k.get_wis(),
            CCNRM!(ch, C_NRM),
            CCCYN!(ch, C_NRM),
            k.get_dex(),
            CCNRM!(ch, C_NRM),
            CCCYN!(ch, C_NRM),
            k.get_con(),
            CCNRM!(ch, C_NRM),
            CCCYN!(ch, C_NRM),
            k.get_cha(),
            CCNRM!(ch, C_NRM)
        )
        .as_str(),
    );
    let k = db.ch(k_id);
    let ch = db.ch(chid);
    game.send_to_char(db,
        chid,
        format!(
            "Hit p.:[{}{}/{}+{}{}]  Mana p.:[{}{}/{}+{}{}]  Move p.:[{}{}/{}+{}{}]\r\n",
            CCGRN!(ch, C_NRM),
            k.get_hit(),
            k.get_max_hit(),
            hit_gain(k),
            CCNRM!(ch, C_NRM),
            CCGRN!(ch, C_NRM),
            k.get_mana(),
            k.get_max_mana(),
            mana_gain(k),
            CCNRM!(ch, C_NRM),
            CCGRN!(ch, C_NRM),
            k.get_move(),
            k.get_max_move(),
            move_gain(k),
            CCNRM!(ch, C_NRM)
        )
        .as_str(),
    );
    let k = db.ch(k_id);
    game.send_to_char(db,
        chid,
        format!(
            "Coins: [{:9}], Bank: [{:9}] (Total: {})\r\n",
            k.get_gold(),
            k.get_bank_gold(),
            k.get_gold() + k.get_bank_gold()
        )
        .as_str(),
    );
    let k = db.ch(k_id);
    game.send_to_char(db,
        chid,
        format!(
            "AC: [{}{}/10], Hitroll: [{:2}], Damroll: [{:2}], Saving throws: [{}/{}/{}/{}/{}]\r\n",
            k.get_ac(),
            DEX_APP[k.get_dex() as usize].defensive,
            k.points.hitroll,
            k.points.damroll,
            k.get_save(0),
            k.get_save(1),
            k.get_save(2),
            k.get_save(3),
            k.get_save(4)
        )
        .as_str(),
    );
    let k = db.ch(k_id);
    buf.clear();
    sprinttype(k.get_pos() as i32, &POSITION_TYPES, &mut buf);
    game.send_to_char(db,
        chid,
        format!(
            "Pos: {}, Fighting: {}",
            buf,
            if k.fighting_id().is_some() {
                db.ch(k.fighting_id().unwrap()).get_name().as_ref()
            } else {
                "Nobody"
            }
        )
        .as_str(),
    );
    let k = db.ch(k_id);
    if k.is_npc() {
        game.send_to_char(db,
            chid,
            format!(
                ", Attack type: {}",
                &ATTACK_HIT_TEXT[k.mob_specials.attack_type as usize].singular
            )
            .as_str(),
        );
    }
    let k = db.ch(k_id);
    if k.desc.is_some() {
        buf.clear();
        sprinttype(
            game.descriptor_list.get(k.desc.unwrap()).state() as i32,
            &CONNECTED_TYPES,
            &mut buf,
        );
        game.send_to_char(db,chid, format!(", Connected: {}", buf).as_str());
    }
    let k = db.ch(k_id);
    if k.is_npc() {
        buf.clear();
        sprinttype(k.mob_specials.default_pos as i32, &POSITION_TYPES, &mut buf);
        game.send_to_char(db,chid, format!(", Default position: {}\r\n", buf).as_str());
        buf.clear();
        let k = db.ch(k_id);
        sprintbit(k.mob_flags(), &ACTION_BITS, &mut buf);
        let ch = db.ch(chid);
        game.send_to_char(db,
            chid,
            format!(
                "NPC flags: {}{}{}\r\n",
                CCCYN!(ch, C_NRM),
                buf,
                CCNRM!(ch, C_NRM)
            )
            .as_str(),
        );
    } else {
        game.send_to_char(db,
            chid,
            format!(
                ", Idle Timer (in tics) [{}]\r\n",
                k.char_specials.timer
            )
            .as_str(),
        );
        buf.clear();
        let k = db.ch(k_id);
        sprintbit(k.plr_flags(), &PLAYER_BITS, &mut buf);
        let ch = db.ch(chid);
        game.send_to_char(db,
            chid,
            format!("PLR: {}{}{}\r\n", CCCYN!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
        );
        buf.clear();
        let k = db.ch(k_id);
        sprintbit(k.prf_flags(), &PREFERENCE_BITS, &mut buf);
        let ch = db.ch(chid);
        game.send_to_char(db,
            chid,
            format!("PRF: {}{}{}\r\n", CCGRN!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
        );
    }
    let k = db.ch(k_id);
    if db.is_mob(k) {
        game.send_to_char(db,
            chid,
            format!(
                "Mob Spec-Proc: {}, NPC Bare Hand Dam: {}d{}\r\n",
                if db.mob_index[k.get_mob_rnum() as usize].func.is_some() {
                    "Exists"
                } else {
                    "None"
                },
                k.mob_specials.damnodice,
                k.mob_specials.damsizedice
            )
            .as_str(),
        );
    }
    let k = db.ch(k_id);
    game.send_to_char(db,
        chid,
        format!(
            "Carried: weight: {}, items: {}; Items in: inventory: {}, ",
            k.is_carrying_w(),
            k.is_carrying_n(),
            k.carrying.len()
        )
        .as_str(),
    );
    let k = db.ch(k_id);
    let mut i2 = 0;
    for i in 0..NUM_WEARS {
        if k.get_eq(i).is_some() {
            i2 += 1;
        }
    }

    game.send_to_char(db,chid, format!("eq: {}\r\n", i2).as_str());
    let k = db.ch(k_id);
    if !k.is_npc() {
        game.send_to_char(db,
            chid,
            format!(
                "Hunger: {}, Thirst: {}, Drunk: {}\r\n",
                k.get_cond(FULL),
                k.get_cond(THIRST),
                k.get_cond(DRUNK)
            )
            .as_str(),
        );
    }
    let k = db.ch(k_id);
    let mut column = game.send_to_char(db,
        chid,
        format!(
            "Master is: {}, Followers are:",
            if k.master.is_some() {
                db.ch(k.master.unwrap()).get_name().as_ref()
            } else {
                "<none>"
            }
        )
        .as_str(),
    );
    let k = db.ch(k_id);
    if k.followers.is_empty() {
        game.send_to_char(db,chid, " <none>\r\n");
    } else {
        let mut found = 0;
        let list = k.followers.clone();
        for (i, fol) in list.iter().enumerate() {
            let ch = db.ch(chid);
            column += game.send_to_char(db,
                chid,
                format!(
                    "{} {}",
                    if found != 0 { "," } else { "" },
                    game.pers(db,db.ch(fol.follower), ch)
                )
                .as_str(),
            );
            let k = db.ch(k_id);
            found += 1;
            if column >= 62 {
                let msg = format!(
                    "{}\r\n",
                    if i < k.followers.len() - 1 {
                        ","
                    } else {
                        ""
                    }
                );
                game.send_to_char(db,chid, msg.as_str());
                found = 0;
                column = 0;
            }
        }
        if column != 0 {
            game.send_to_char(db,chid, "\r\n");
        }
    }

    /* Showing the bitvector */
    buf.clear();
    let k = db.ch(k_id);
    sprintbit(k.aff_flags(), &AFFECTED_BITS, &mut buf);
    let ch = db.ch(chid);
    game.send_to_char(db,
        chid,
        format!("AFF: {}{}{}\r\n", CCYEL!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
    );

    /* Routine to show what spells a char is affected by */
    let k = db.ch(k_id);
    if k.affected.len() != 0 {
        let list = k.affected.clone();
        for aff in list {
            let ch = db.ch(chid);
            game.send_to_char(db,
                chid,
                format!(
                    "SPL: ({:3}hr) {}{:21}{} ",
                    aff.duration + 1,
                    CCCYN!(ch, C_NRM),
                    skill_name(&db, aff._type as i32),
                    CCNRM!(ch, C_NRM)
                )
                .as_str(),
            );

            if aff.modifier != 0 {
                game.send_to_char(db,
                    chid,
                    format!(
                        "{} to {}",
                        aff.modifier, &APPLY_TYPES[aff.location as usize]
                    )
                    .as_str(),
                );
            }

            if aff.bitvector != 0 {
                if aff.modifier != 0 {
                    game.send_to_char(db,chid, ", ");
                }
                buf.clear();
                sprintbit(aff.bitvector, &AFFECTED_BITS, &mut buf);
                game.send_to_char(db,chid, format!("sets {}", buf).as_str());
            }
            game.send_to_char(db,chid, "\r\n");
        }
    }
}

pub fn do_stat(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut buf1 = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut buf1, &mut buf2);

    if buf1.is_empty() {
        game.send_to_char(db,chid, "Stats on who or what?\r\n");
        return;
    } else if is_abbrev(&buf1, "room") {
        do_stat_room(game,db, chid);
    } else if is_abbrev(&buf1, "mob") {
        if buf2.is_empty() {
            game.send_to_char(db,chid, "Stats on which mobile?\r\n");
        } else {
            let victim_id;
            if {
                victim_id = game.get_char_vis(db,chid, &mut buf2, None, FIND_CHAR_WORLD);
                victim_id.is_some()
            } {
                do_stat_character(game, db,chid, victim_id.unwrap());
            } else {
                game.send_to_char(db,chid, "No such mobile around.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "player") {
        if buf2.is_empty() {
            game.send_to_char(db,chid, "Stats on which player?\r\n");
        } else {
            let victim_id;
            if {
                victim_id = game.get_player_vis(db,chid, &mut buf2, None, FIND_CHAR_WORLD);
                victim_id.is_some()
            } {
                do_stat_character(game,db, chid, victim_id.unwrap());
            } else {
                game.send_to_char(db,chid, "No such player around.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "file") {
        let victim_id;
        if buf2.is_empty() {
            game.send_to_char(db,chid, "Stats on which player?\r\n");
        } else if {
            victim_id = game.get_player_vis(db,chid, &mut buf2, None, FIND_CHAR_WORLD);
            victim_id.is_some()
        } {
            do_stat_character(game, db,chid, victim_id.unwrap());
        } else {
            let mut victim = CharData::default();
            let mut tmp_store = CharFileU::new();
            clear_char(&mut victim);
            if db.load_char(&buf2, &mut tmp_store).is_some() {
                store_to_char(&tmp_store, &mut victim);
                victim.player.time.logon = tmp_store.last_logon;
                let victim = Rc::new(victim);
                db.char_to_room(victim_id.unwrap(), 0);
                let ch = db.ch(chid);
                if victim.get_level() > ch.get_level() {
                    game.send_to_char(db,chid, "Sorry, you can't do that.\r\n");
                } else {
                    do_stat_character(game, db, chid, victim_id.unwrap());
                }
                game.extract_char_final(db,victim.id());
            } else {
                game.send_to_char(db,chid, "There is no such player.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "object") {
        if buf2.is_empty() {
            game.send_to_char(db,chid, "Stats on which object?\r\n");
        } else {
            let oid;
            if {
                oid = game.get_obj_vis(db,ch, &mut buf2, None);
                oid.is_some()
            } {
                do_stat_object(game, db, chid, oid.unwrap());
            } else {
                game.send_to_char(db,chid, "No such object around.\r\n");
            }
        }
    } else {
        let mut name = buf1;
        let mut number = get_number(&mut name);
        let mut oid;
        let mut victim_id;
        if {
            oid = game.get_obj_in_equip_vis(db,ch, &name, Some(&mut number), &ch.equipment);
            oid.is_some()
        } {
            do_stat_object(game, db, chid, oid.unwrap());
        } else if {
            oid = game.get_obj_in_list_vis(db,ch, &name, Some(&mut number), &ch.carrying);
            oid.is_some()
        } {
            do_stat_object(game,db,  chid, oid.unwrap());
        } else if {
            victim_id = game.get_char_vis(db,chid, &mut name, Some(&mut number), FIND_CHAR_ROOM);
            victim_id.is_some()
        } {
            do_stat_character(game, db, chid, victim_id.unwrap());
        } else if {
            oid = game.get_obj_in_list_vis2(db,
                ch,
                &mut name,
                Some(&mut number),
                &db.world[ch.in_room() as usize].contents,
            );
            oid.is_some()
        } {
            do_stat_object(game, db, chid, oid.unwrap());
        } else if {
            victim_id = game.get_char_vis(db,chid, &mut name, Some(&mut number), FIND_CHAR_WORLD);
            victim_id.is_some()
        } {
            do_stat_character(game, db, chid, victim_id.unwrap());
        } else if {
            oid = game.get_obj_vis(db,ch, &mut name, Some(&mut number));
            oid.is_some()
        } {
            do_stat_object(game, db, chid, oid.unwrap());
        } else {
            game.send_to_char(db,chid, "Nothing around by that name.\r\n");
        }
    }
}

pub fn do_shutdown(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();
    if subcmd != SCMD_SHUTDOWN {
        game.send_to_char(db,chid, "If you want to shut something down, say so!\r\n");
        return;
    }
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        info!("(GC) Shutdown by {}.", ch.get_name());
        game.send_to_all("Shutting down.\r\n");
        game.circle_shutdown = true;
    } else if arg == "reboot" {
        info!("(GC) Reboot by {}.", ch.get_name());
        game.send_to_all("Rebooting.. come back in a minute or two.\r\n");
        touch(Path::new(FASTBOOT_FILE)).unwrap();
        game.circle_shutdown = true;
        game.circle_reboot = true;
    } else if arg == "die" {
        info!("(GC) Shutdown by {}.", ch.get_name());
        game.send_to_all("Shutting down for maintenance.\r\n");
        touch(Path::new(KILLSCRIPT_FILE)).unwrap();
        game.circle_shutdown = true;
    } else if arg == "pause" {
        info!("(GC) Shutdown by {}.", ch.get_name());
        game.send_to_all("Shutting down for maintenance.\r\n");
        touch(Path::new(PAUSE_FILE)).unwrap();
        game.circle_shutdown = true;
    } else {
        game.send_to_char(db,chid, "Unknown shutdown option.\r\n");
    }
}

pub fn snoop_check(game: &mut Game, db: &DB, chid: DepotId) {
    let ch = db.ch(chid);
    /*  This short routine is to ensure that characters that happen
     *  to be snooping (or snooped) and get advanced/demoted will
     *  not be snooping/snooped someone of a higher/lower level (and
     *  thus, not entitled to be snooping.
     */
    if ch.desc.is_none() {
        return;
    }
    let d_id = ch.desc.unwrap();
    if game.desc(d_id).snooping.is_some()
        && 
            db
            .ch(game
                .desc(game.desc(d_id).snooping.unwrap())
                .character
                .unwrap())
            .get_level()
            >= ch.get_level()
    {
        game.desc_mut(game.desc(d_id).snooping.unwrap()).snoop_by = None;
        game.desc_mut(d_id).snooping = None;
    }
    let ch = db.ch(chid);
    if game.desc(d_id).snoop_by.is_some()
        && ch.get_level()
            >=
                db
                .ch(game
                    .desc(game.desc(d_id).snoop_by.unwrap())
                    .character
                    .unwrap())
                .get_level()
    {
        game.desc_mut(game.desc(d_id).snoop_by.unwrap()).snooping = None;
        game.desc_mut(d_id).snoop_by = None;
    }
}

fn stop_snooping(game: &mut Game, db: &mut DB, chid: DepotId) {
    let ch = db.ch(chid);

    if game.desc(ch.desc.unwrap()).snooping.is_none() {
        game.send_to_char(db,chid, "You aren't snooping anyone.\r\n");
    } else {
        game.send_to_char(db,chid, "You stop snooping.\r\n");
        let ch = db.ch(chid);
        let desc_id = game.desc(ch.desc.unwrap()).snooping.unwrap();
        game.desc_mut(desc_id).snoop_by = None;
        let ch = db.ch(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).snooping = None;
    }
}

pub fn do_snoop(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();

    if ch.desc.is_none() {
        return;
    }

    one_argument(argument, &mut arg);
    let victim;
    let victim_id;
    let voriginal_id;
    let tch_id;
    let tch;
    if arg.is_empty() {
        stop_snooping(game,db, chid);
    } else if {
        victim_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_WORLD);
        victim = victim_id.map(|v| db.ch(v));
        victim.is_none()
    } {
        game.send_to_char(db,chid, "No such person around.\r\n");
    } else if victim.as_ref().unwrap().desc.is_none() {
        game.send_to_char(db,chid, "There's no link.. nothing to snoop.\r\n");
    } else if victim_id.unwrap() == chid {
        stop_snooping(game, db, chid);
    } else if game
        .desc(victim.as_ref().unwrap().desc.unwrap())
        .snoop_by
        .is_some()
    {
        game.send_to_char(db,chid, "Busy already. \r\n");
    } else if game
        .desc(victim.as_ref().unwrap().desc.unwrap())
        .snooping
        .unwrap()
        == ch.desc.unwrap()
    {
        game.send_to_char(db,chid, "Don't be stupid.\r\n");
    } else {
        if game
            .desc(victim.as_ref().unwrap().desc.unwrap())
            .original
            .is_some()
        {
            voriginal_id = game
                .desc(victim.as_ref().unwrap().desc.unwrap())
                .original;
            tch_id = voriginal_id;
            tch = tch_id.map(|v| db.ch(v));
        } else {
            tch_id = victim_id;
            tch = tch_id.map(|v| db.ch(v));
        }
        if tch.as_ref().unwrap().get_level() >= ch.get_level() {
            game.send_to_char(db,chid, "You can't.\r\n");
            return;
        }
        game.send_to_char(db,chid, OK);
        let ch = db.ch(chid);
        if game.desc(ch.desc.unwrap()).snooping.is_some() {
            let desc_id = ch.desc.unwrap();
            let snooping_desc_id = game.desc(desc_id).snooping.unwrap();
            game.desc_mut(snooping_desc_id).snoop_by = None;
        }
        let ch = db.ch(chid);
        let desc_id = ch.desc.unwrap();
        let victim = victim_id.map(|v| db.ch(v));
        let new_snooping = Some(victim.as_ref().unwrap().desc.unwrap()).clone();
        game.desc_mut(desc_id).snooping = new_snooping;
    }
}

pub fn do_switch(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let victim;
    let victim_id;
    if game.desc(ch.desc.unwrap()).original.is_some() {
        game.send_to_char(db,chid, "You're already switched.\r\n");
    } else if arg.is_empty() {
        game.send_to_char(db,chid, "Switch with who?\r\n");
    } else if {
        victim_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_WORLD);
        victim = victim_id.map(|v| db.ch(v));
        victim.is_none()
    } {
        game.send_to_char(db,chid, "No such character.\r\n");
    } else if chid == victim_id.unwrap() {
        game.send_to_char(db,chid, "Hee hee... we are jolly funny today, eh?\r\n");
    } else if victim.as_ref().unwrap().desc.is_some() {
        game.send_to_char(db,chid, "You can't do that, the body is already in use!\r\n");
    } else if ch.get_level() < LVL_IMPL as u8 && !victim.as_ref().unwrap().is_npc() {
        game.send_to_char(db,chid, "You aren't holy enough to use a mortal's body.\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8
        && 
            db
            .room_flagged(victim.as_ref().unwrap().in_room(), ROOM_GODROOM)
    {
        game.send_to_char(db,chid, "You are not godly enough to use that room!\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8
        && 
            db
            .room_flagged(victim.as_ref().unwrap().in_room(), ROOM_HOUSE)
        && !house_can_enter(
            &db,
            ch,
            db.get_room_vnum(victim.as_ref().unwrap().in_room()),
        )
    {
        game.send_to_char(db,chid, "That's private property -- no trespassing!\r\n");
    } else {
        game.send_to_char(db,chid, OK);
        let ch = db.ch(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).character = victim_id;
        let ch = db.ch(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).original = Some(chid);
        let ch = db.ch(chid);
        let val = ch.desc.clone();
        let mut victim = victim_id.map(|v| db.ch_mut(v));
        victim.as_mut().unwrap().desc = val;
        let ch = db.ch_mut(chid);
        ch.desc = None;
    }
}

pub fn do_return(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    if ch.desc.is_some() && game.desc(ch.desc.unwrap()).original.is_some() {
        game.send_to_char(db,chid, "You return to your original body.\r\n");
        let ch = db.ch(chid);
        /*
         * If someone switched into your original body, disconnect them.
         *   - JE 2/22/95
         *
         * Zmey: here we put someone switched in our body to disconnect state
         * but we must also None his pointer to our character, otherwise
         * close_socket() will damage our character's pointer to our descriptor
         * (which is assigned below in this function). 12/17/99
         */
        if 
            db
            .ch(game.desc(ch.desc.unwrap()).original.unwrap())
            .desc
            .borrow()
            .is_some()
        {
            let dorig_id =
                db
                .ch(game.desc(ch.desc.unwrap()).original.unwrap())
                .desc
                .unwrap();
            game.desc_mut(dorig_id).character = None;
            game.desc_mut(dorig_id).set_state(ConDisconnect);
        }

        /* Now our descriptor points to our original body. */
        let ch = db.ch(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).character = game.desc(desc_id).original.clone();
        let ch = db.ch(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).original = None;

        /* And our body's pointer to descriptor now points to our descriptor. */
        let ch = db.ch(chid);
        
            db
            .ch_mut(game.desc(ch.desc.unwrap()).character.unwrap())
            .desc
          = ch.desc.clone();
          let ch = db.ch_mut(chid);
        ch.desc = None;
    }
}

pub fn do_load(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !buf2.chars().next().unwrap().is_digit(10) {
        game.send_to_char(db,chid, "Usage: load { obj | mob } <number>\r\n");
        return;
    }
    if !is_number(&buf2) {
        game.send_to_char(db,chid, "That is not a number.\r\n");
        return;
    }

    if is_abbrev(&buf, "mob") {
        let r_num;

        if {
            r_num = db.real_mobile(buf2.parse::<i16>().unwrap());
            r_num == NOBODY
        } {
            game.send_to_char(db,chid, "There is no monster with that number.\r\n");
            return;
        }
        let mob_id = db.read_mobile(r_num, REAL).unwrap();
        let ch = db.ch(chid);
        db.char_to_room(mob_id, ch.in_room());

        game.act(db,
            "$n makes a quaint, magical gesture with one hand.",
            true,
            Some(chid),
            None,
            None,
            TO_ROOM,
        );
        game.act(db,
            "$n has created $N!",
            false,
            Some(chid),
            None,
            Some(VictimRef::Char(mob_id)),
            TO_ROOM,
        );
        game.act(db,
            "You create $N.",
            false,
            Some(chid),
            None,
            Some(VictimRef::Char(mob_id)),
            TO_CHAR,
        );
    } else if is_abbrev(&buf, "obj") {
        let r_num;

        if {
            r_num = db.real_object(buf2.parse::<i16>().unwrap());
            r_num == NOTHING
        } {
            game.send_to_char(db,chid, "There is no object with that number.\r\n");
            return;
        }
        let oid = db.read_object(r_num, REAL).unwrap();
        if LOAD_INTO_INVENTORY {
            db.obj_to_char(oid, chid);
        } else {
            let ch = db.ch(chid);
            db.obj_to_room(oid, ch.in_room());
        }
        game.act(db,
            "$n makes a strange magical gesture.",
            true,
            Some(chid),
            None,
            None,
            TO_ROOM,
        );
        game.act(db,
            "$n has created $p!",
            false,
            Some(chid),
            Some(oid),
            None,
            TO_ROOM,
        );
        game.act(db,
            "You create $p.",
            false,
            Some(chid),
            Some(oid),
            None,
            TO_CHAR,
        );
    } else {
        game.send_to_char(db,chid, "That'll have to be either 'obj' or 'mob'.\r\n");
    }
}

pub fn do_vstat(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !buf2.chars().next().unwrap().is_digit(10) {
        game.send_to_char(db,chid, "Usage: vstat { obj | mob } <number>\r\n");
        return;
    }
    if !is_number(&buf2) {
        game.send_to_char(db,chid, "That's not a valid number.\r\n");
        return;
    }

    if is_abbrev(&buf, "mob") {
        let r_num;

        if {
            r_num = db.real_mobile(buf2.parse::<i16>().unwrap());
            r_num == NOBODY
        } {
            game.send_to_char(db,chid, "There is no monster with that number.\r\n");
            return;
        }
        let mob_id = db.read_mobile(r_num, REAL);
        db.char_to_room(mob_id.unwrap(), 0);
        do_stat_character(game, db,chid, mob_id.unwrap());
        db.extract_char(mob_id.unwrap());
    } else if is_abbrev(&buf, "obj") {
        let r_num;

        if {
            r_num = db.real_object(buf2.parse::<i16>().unwrap());
            r_num == NOTHING
        } {
            game.send_to_char(db,chid, "There is no object with that number.\r\n");
            return;
        }
        let oid = db.read_object(r_num, REAL);
        do_stat_object(game, db, chid, oid.unwrap());
        game.extract_obj(db,oid.unwrap());
    } else {
        game.send_to_char(db,chid, "That'll have to be either 'obj' or 'mob'.\r\n");
    }
}

pub fn do_purge(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    /* clean a room of all mobiles and objects */
    let mut buf = String::new();
    one_argument(argument, &mut buf);
    let vict_id;
    let oid;
    /* argument supplied. destroy single object or char */
    if !buf.is_empty() {
        if {
            vict_id = game.get_char_vis(db,chid, &mut buf, None, FIND_CHAR_ROOM);
            vict_id.is_some()
        } {
            if !db.ch(vict_id.unwrap()).is_npc()
                && ch.get_level() <= db.ch(vict_id.unwrap()).get_level()
            {
                game.send_to_char(db,chid, "Fuuuuuuuuu!\r\n");
                return;
            }
            game.act(db,
                "$n disintegrates $N.",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(vict_id.unwrap())),
                TO_NOTVICT,
            );
            let vict_id = vict_id.unwrap();
            let vict = db.ch(vict_id);
            if !vict.is_npc() {
                let ch = db.ch(chid);
                game.mudlog(db,
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} has purged {}.", ch.get_name(), vict.get_name()).as_str(),
                );
                let vict = db.ch(vict_id);
                if vict.desc.is_some() {
                    let desc_id = vict.desc.unwrap();
                    game.desc_mut(desc_id).set_state(ConClose);
                    let vict = db.ch(vict_id);
                    let desc_id = vict.desc.unwrap();
                    game.desc_mut(desc_id).character = None;
                    let vict = db.ch_mut(vict_id);
                    vict.desc = None;
                }
            }
            db.extract_char(vict_id);
        } else if {
            oid = game.get_obj_in_list_vis2(db,
                ch,
                &mut buf,
                None,
                &db.world[ch.in_room() as usize].contents,
            );
            oid.is_some()
        } {
            let oid = oid.unwrap();
            game.act(db,
                "$n destroys $p.",
                false,
                Some(chid),
                Some(oid),
                None,
                TO_ROOM,
            );
            game.extract_obj(db, oid);
        } else {
            game.send_to_char(db,chid, "Nothing here by that name.\r\n");
            return;
        }

        game.send_to_char(db,chid, OK);
    } else {
        /* no argument. clean out the room */

        game.act(db,
            "$n gestures... You are surrounded by scorching flames!",
            false,
            Some(chid),
            None,
            None,
            TO_ROOM,
        );
        let ch = db.ch(chid);
        game.send_to_room(db, ch.in_room(), "The world seems a little cleaner.\r\n");
        let ch = db.ch(chid);
        let list = clone_vec2(&db.world[ch.in_room() as usize].peoples);
        for vict_id in list {
            let vict = db.ch(vict_id);
            if !vict.is_npc() {
                continue;
            }

            /* Dump inventory. */
            while { let vict = db.ch(vict_id); vict.carrying.len() > 0 } {
                let vict = db.ch(vict_id);
                let oid = vict.carrying[0];
                game.extract_obj(db,oid);
            }

            /* Dump equipment. */
            for i in 0..NUM_WEARS {
                let vict = db.ch(vict_id);
                if vict.get_eq(i).is_some() {
                    let oid = vict.get_eq(i).unwrap();
                    game.extract_obj(db,oid)
                }
            }

            /* Dump character. */
            db.extract_char(vict_id);
        }

        /* Clear the ground. */
        let ch = db.ch(chid);
        let ch_in_room = ch.in_room();
        loop {
            if db.world[ch_in_room as usize].contents.len() <= 0 {
                break;
            }
            let oid = db.world[ch_in_room as usize].contents[0];
            game.extract_obj(db,oid);
        }
    }
}

const LOGTYPES: [&str; 5] = ["off", "brief", "normal", "complete", "\n"];

pub fn do_syslog(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        game.send_to_char(db,
            chid,
            format!(
                "Your syslog is currently {}.\r\n",
                LOGTYPES[if ch.prf_flagged(PRF_LOG1) { 1 } else { 0 }
                    + if ch.prf_flagged(PRF_LOG2) { 2 } else { 0 }]
            )
            .as_str(),
        );
        return;
    }
    let tp;
    if {
        tp = search_block(&arg, &LOGTYPES, false);
        tp.is_none()
    } {
        game.send_to_char(db,
            chid,
            "Usage: syslog { Off | Brief | Normal | Complete }\r\n",
        );
        return;
    }
    let tp = tp.unwrap();
    let ch = db.ch_mut(chid);
    ch.remove_prf_flags_bits(PRF_LOG1 | PRF_LOG2);
    ch.set_prf_flags_bits(PRF_LOG1 * (tp & 1) as i64 | PRF_LOG2 * (tp & 2) as i64 >> 1);

    game.send_to_char(db,
        chid,
        format!("Your syslog is now {}.\r\n", &LOGTYPES[tp]).as_str(),
    );
}

pub fn do_advance(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut name = String::new();
    let mut level = String::new();
    let victim;
    two_arguments(argument, &mut name, &mut level);

    if name.len() != 0 {
        if {
            victim = game.get_char_vis(db,chid, &mut name, None, FIND_CHAR_WORLD);
            victim.is_none()
        } {
            game.send_to_char(db,chid, "That player is not here.\r\n");
            return;
        }
    } else {
        game.send_to_char(db,chid, "Advance who?\r\n");
        return;
    }
    let victim_id = victim.unwrap();
    let victim = db.ch(victim_id);

    if ch.get_level() <= victim.get_level() {
        game.send_to_char(db,chid, "Maybe that's not such a great idea.\r\n");
        return;
    }
    if victim.is_npc() {
        game.send_to_char(db,chid, "NO!  Not on NPC's.\r\n");
        return;
    }
    let r = level.parse::<u8>();
    let mut newlevel = 255;
    if r.is_err() || {
        newlevel = r.unwrap();
        newlevel == 0
    } {
        game.send_to_char(db,chid, "That's not a level!\r\n");
        return;
    }

    if newlevel > LVL_IMPL as u8 {
        game.send_to_char(db,
            chid,
            format!("{} is the highest possible level.\r\n", LVL_IMPL).as_str(),
        );
        return;
    }
    if newlevel > ch.get_level() {
        game.send_to_char(db,chid, "Yeah, right.\r\n");
        return;
    }
    if newlevel == victim.get_level() {
        game.send_to_char(db,chid, "They are already at that level.\r\n");
        return;
    }
    let oldlevel = victim.get_level();
    if newlevel < oldlevel {
        do_start(game, db,victim_id);
        let victim = db.ch_mut(victim_id);
        victim.set_level(newlevel);
        let victim = db.ch(victim_id);
        game.send_to_char(db,
            victim.id(),
            "You are momentarily enveloped by darkness!\r\nYou feel somewhat diminished.\r\n",
        );
    } else {
        game.act(db,
            "$n makes some strange gestures.\r\n\
A strange feeling comes upon you,\r\n\
Like a giant hand, light comes down\r\n\
from above, grabbing your body, that\r\n\
begins to pulse with colored lights\r\n\
from inside.\r\n\r\n\
Your head seems to be filled with demons\r\n\
from another plane as your body dissolves\r\n\
to the elements of time and space itself.\r\n\
Suddenly a silent explosion of light\r\n\
snaps you back to reality.\r\n\r\n\
You feel slightly different.",
            false,
            Some(chid),
            None,
            Some(VictimRef::Char(victim.id())),
            TO_VICT,
        );
    }

    game.send_to_char(db,chid, OK);
    let ch = db.ch(chid);
    if newlevel < oldlevel {
        let victim = db.ch(victim_id);
        info!(
            "(GC) {} demoted {} from level {} to {}.",
            ch.get_name(),
            victim.get_name(),
            oldlevel,
            newlevel
        );
    } else {
        let victim = db.ch(victim_id);
        info!(
            "(GC) {} has advanced {} to level {} (from {})",
            ch.get_name(),
            victim.get_name(),
            newlevel,
            oldlevel
        );
    }
    if oldlevel >= LVL_IMMORT as u8 && newlevel < LVL_IMMORT as u8 {
        /* If they are no longer an immortal, let's remove some of the
         * nice immortal only flags, shall we?
         */
        let victim = db.ch_mut(victim_id);
        victim.remove_prf_flags_bits(PRF_LOG1 | PRF_LOG2);
        victim.remove_prf_flags_bits(PRF_NOHASSLE | PRF_HOLYLIGHT);

        // TODO run_autowiz();
    }
    let victim = db.ch(victim_id);
    gain_exp_regardless(
        game,db,
        victim_id,
        level_exp(victim.get_class(), newlevel as i16) - victim.get_exp(),
    );
    game.save_char(db, victim_id);
}

pub fn do_restore(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let vict_id;
    if buf.is_empty() {
        game.send_to_char(db,chid, "Whom do you wish to restore?\r\n");
    } else if {
        vict_id = game.get_char_vis(db,chid, &mut buf, None, FIND_CHAR_WORLD);
        vict_id.is_none()
    } {
        game.send_to_char(db,chid, NOPERSON);
    } else if !db.ch(vict_id.unwrap()).is_npc()
        && chid != vict_id.unwrap()
        && db.ch(vict_id.unwrap()).get_level() >= ch.get_level()
    {
        game.send_to_char(db,chid, "They don't need your help.\r\n");
    } else {
        let vict_id = vict_id.unwrap();
        let vict = db.ch_mut(vict_id);
        vict.set_hit(vict.get_max_hit());
        vict.set_mana(vict.get_max_mana());
        vict.set_move(vict.get_move());
        let ch = db.ch(chid);
        let vict = db.ch(vict_id);
        if !vict.is_npc() && ch.get_level() >= LVL_GRGOD as u8 {
            if vict.get_level() >= LVL_IMMORT as u8 {
                for i in 1..MAX_SKILLS + 1 {
                    let vict = db.ch_mut(vict_id);
                    vict.set_skill(i as i32, 100);
                }
            }

        let vict = db.ch(vict_id);
            if vict.get_level() >= LVL_GRGOD as u8 {
                let vict = db.ch_mut(vict_id);
                vict.real_abils.str_add = 100;
                vict.real_abils.intel = 25;
                vict.real_abils.wis = 25;
                vict.real_abils.dex = 25;
                vict.real_abils.str = 25;
                vict.real_abils.con = 25;
                vict.real_abils.cha = 25;
            }
        }
        let vict = db.ch_mut(vict_id);
        update_pos(vict);
        db.affect_total(vict_id);
        game.send_to_char(db,chid, OK);
        game.act(db,
            "You have been fully healed by $N!",
            false,
            Some(vict_id),
            None,
            Some(VictimRef::Char(chid)),
            TO_CHAR,
        );
    }
}

pub fn perform_immort_vis(game: &mut Game, db: &mut DB, chid: DepotId) {
    let ch = db.ch(chid);
    if ch.get_invis_lev() == 0 && !ch.aff_flagged(AFF_HIDE | AFF_INVISIBLE) {
        game.send_to_char(db,chid, "You are already fully visible.\r\n");
        return;
    }
    let ch = db.ch_mut(chid);

    ch.set_invis_lev(0);

    game.appear(db,chid);
    game.send_to_char(db,chid, "You are now fully visible.\r\n");
}

fn perform_immort_invis(game: &mut Game, db: &mut DB, chid: DepotId, level: i32) {
    let ch = db.ch(chid);

    let list = clone_vec2(&db.world[ch.in_room() as usize].peoples);
    for tch_id in list {
        let tch = db.ch(tch_id);
        if tch_id == chid {
            continue;
        }
        let ch = db.ch(chid);
        if tch.get_level() >= ch.get_invis_lev() as u8 && tch.get_level() < level as u8 {
            game.act(db,
                "You blink and suddenly realize that $n is gone.",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(tch_id)),
                TO_VICT,
            );
        }
        let tch = db.ch(tch_id);
        let ch = db.ch(chid);
        if tch.get_level() < ch.get_invis_lev() as u8 && tch.get_level() >= level as u8 {
            game.act(db,
                "You suddenly realize that $n is standing beside you.",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(tch_id)),
                TO_VICT,
            );
        }
    }
    let ch = db.ch_mut(chid);
    ch.set_invis_lev(level as i16);
    game.send_to_char(db,
        chid,
        format!("Your invisibility level is {}.\r\n", level).as_str(),
    );
}

pub fn do_invis(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();

    if ch.is_npc() {
        game.send_to_char(db,chid, "You can't do that!\r\n");
        return;
    }

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        if ch.get_invis_lev() > 0 {
            perform_immort_vis(game, db, chid);
        } else {
            perform_immort_invis(game, db, chid, ch.get_level() as i32);
        }
    } else {
        let level = arg.parse::<i32>();
        let level = if level.is_err() { 0 } else { level.unwrap() };
        if level > ch.get_level() as i32 {
            game.send_to_char(db,chid, "You can't go invisible above your own level.\r\n");
        } else if level < 1 {
            perform_immort_vis(game, db, chid);
        } else {
            perform_immort_invis(game, db, chid, level);
        }
    }
}

pub fn do_gecho(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if argument.is_empty() {
        game.send_to_char(db,chid, "That must be a mistake...\r\n");
    } else {
        for pt_id in game.descriptor_list.ids() {
            if game.desc(pt_id).state() == ConPlaying
                && game.desc(pt_id).character.is_some()
                && game.desc(pt_id).character.unwrap() != chid
            {
                let chid = game.desc(pt_id).character.unwrap();
                game.send_to_char(db,chid, format!("{}\r\n", argument).as_str());
            }
        }
        let ch = db.ch(chid);
        if ch.prf_flagged(PRF_NOREPEAT) {
            game.send_to_char(db,chid, OK);
        } else {
            game.send_to_char(db,chid, format!("{}\r\n", argument).as_str());
        }
    }
}

pub fn do_poofset(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = db.ch_mut(chid);
    {
        let msg;

        let cps = &mut ch.player_specials;
        match subcmd {
            SCMD_POOFIN => {
                msg = &mut cps.poofin;
            }
            SCMD_POOFOUT => {
                msg = &mut cps.poofout;
            }
            _ => {
                return;
            }
        }

        let argument = argument.trim_start();

        *msg = Rc::from(argument);
    }
    game.send_to_char(db,chid, OK);
}

pub fn do_dc(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let num_to_dc = arg.parse::<u32>();
    if num_to_dc.is_err() {
        game.send_to_char(db,chid, "Usage: DC <user number> (type USERS for a list)\r\n");
        return;
    }
    let num_to_dc = num_to_dc.unwrap();
    let mut d_id = None;
    {
        for cd_id in game.descriptor_list.ids() {
            if game.desc(cd_id).desc_num == num_to_dc as usize {
                d_id = Some(cd_id);
            }
        }
    }

    if d_id.is_none() {
        game.send_to_char(db,chid, "No such connection.\r\n");
        return;
    }
    let d_id = d_id.unwrap();
    if game.desc(d_id).character.is_some()
        && db.ch(game.desc(d_id).character.unwrap()).get_level() >= ch.get_level()
    {
        if !game.can_see(db,ch, db.ch(game.desc(d_id).character.unwrap())) {
            game.send_to_char(db,chid, "No such connection.\r\n");
        } else {
            game.send_to_char(db,chid, "Umm.. maybe that's not such a good idea...\r\n");
        }
        return;
    }

    /* We used to just close the socket here using close_socket(), but
     * various people pointed out this could cause a crash if you're
     * closing the person below you on the descriptor list.  Just setting
     * to CON_CLOSE leaves things in a massively inconsistent state so I
     * had to add this new flag to the descriptor. -je
     *
     * It is a much more logical extension for a CON_DISCONNECT to be used
     * for in-game socket closes and CON_CLOSE for out of game closings.
     * This will retain the stability of the close_me hack while being
     * neater in appearance. -gg 12/1/97
     *
     * For those unlucky souls who actually manage to get disconnected
     * by two different immortals in the same 1/10th of a second, we have
     * the below 'if' check. -gg 12/17/99
     */
    if game.desc(d_id).state() == ConDisconnect || game.desc(d_id).state() == ConClose {
        game.send_to_char(db,chid, "They're already being disconnected.\r\n");
    } else {
        /*
         * Remember that we can disconnect people not in the game and
         * that rather confuses the code when it expected there to be
         * a character context.
         */
        if game.desc(d_id).state() == ConPlaying {
            game.desc_mut(d_id).set_state(ConDisconnect);
        } else {
            game.desc_mut(d_id).set_state(ConClose);
        }
        game.send_to_char(db,
            chid,
            format!("Connection #{} closed.\r\n", num_to_dc).as_str(),
        );
        let ch = db.ch(chid);
        info!("(GC) Connection closed by {}.", ch.get_name());
    }
}

pub fn do_wizlock(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();
    let value;
    one_argument(argument, &mut arg);
    let when;
    if !arg.is_empty() {
        value = arg.parse::<i32>();
        let value = if value.is_err() { -1 } else { value.unwrap() };
        if value < 0 || value > ch.get_level() as i32 {
            game.send_to_char(db,chid, "Invalid wizlock value.\r\n");
            return;
        }
        db.circle_restrict = value as u8;
        when = "now";
    } else {
        when = "currently";
    }

    match db.circle_restrict {
        0 => {
            game.send_to_char(db,
                chid,
                format!("The game is {} completely open.\r\n", when).as_str(),
            );
        }
        1 => {
            game.send_to_char(db,
                chid,
                format!("The game is {} closed to new players.\r\n", when).as_str(),
            );
        }
        _ => {
            game.send_to_char(db,
                chid,
                format!(
                    "Only level {} and above may enter the game {}.\r\n",
                    db.circle_restrict, when
                )
                .as_str(),
            );
        }
    }
}

pub fn do_date(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, subcmd: i32) {
    let mytime;
    if subcmd == SCMD_DATE {
        mytime = time_now();
    } else {
        mytime = db.boot_time as u64;
    }

    let date_time = Utc.timestamp_millis_opt(mytime as i64 * 1000).unwrap();
    let tmstr = date_time.to_rfc2822();

    if subcmd == SCMD_DATE {
        game.send_to_char(db,
            chid,
            format!("Current machine time: {}\r\n", tmstr).as_str(),
        );
    } else {
        let mytime = time_now() - db.boot_time as u64;
        let d = mytime / 86400;
        let h = (mytime / 3600) % 24;
        let m = (mytime / 60) % 60;

        game.send_to_char(db,
            chid,
            format!(
                "Up since {}: {} day{}, {}:{:2}\r\n",
                tmstr,
                d,
                if d == 1 { "" } else { "s" },
                h,
                m
            )
            .as_str(),
        );
    }
}

pub fn do_last(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        game.send_to_char(db,chid, "For whom do you wish to search?\r\n");
        return;
    }
    let mut chdata = CharFileU::new();
    if db.load_char(&arg, &mut chdata).is_none() {
        game.send_to_char(db,chid, "There is no such player.\r\n");
        return;
    }
    let ch = db.ch(chid);
    if chdata.level > ch.get_level() && ch.get_level() < LVL_IMPL as u8 {
        game.send_to_char(db,chid, "You are not sufficiently godly for that!\r\n");
        return;
    }
    let id = chdata.char_specials_saved.idnum;
    game.send_to_char(db,
        chid,
        format!(
            "[{:5}] [{:2} {}] {:12} : {:-18} : {:20}\r\n",
            id,
            chdata.level,
            CLASS_ABBREVS[chdata.chclass as usize],
            parse_c_string(&chdata.name.clone()),
            parse_c_string(&chdata.host.clone()),
            ctime(chdata.last_logon)
        )
        .as_str(),
    );
}

pub fn do_force(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut argument = argument.to_string();
    let mut arg = String::new();
    let mut to_force = String::new();

    half_chop(&mut argument, &mut arg, &mut to_force);

    let buf1 = format!("$n has forced you to '{}'.", to_force);
    let vict_id;
    if arg.is_empty() || to_force.is_empty() {
        game.send_to_char(db,chid, "Whom do you wish to force do what?\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8 || "all" != arg && "room" != arg {
        if {
            vict_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_WORLD);
            vict_id.is_none()
        } {
            game.send_to_char(db,chid, NOPERSON);
        } else if !db.ch(vict_id.unwrap()).is_npc()
            && ch.get_level() <= db.ch(vict_id.unwrap()).get_level()
        {
            game.send_to_char(db,chid, "No, no, no!\r\n");
        } else {
            let vict_id = vict_id.unwrap();
            game.send_to_char(db,chid, OK);
            game.act(db,
                &buf1,
                true,
                Some(chid),
                None,
                Some(VictimRef::Char(vict_id)),
                TO_VICT,
            );
            let ch = db.ch(chid);
            let vict = db.ch(vict_id);
            game.mudlog(db,
                NRM,
                max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "(GC) {} forced {} to {}",
                    ch.get_name(),
                    vict.get_name(),
                    to_force
                )
                .as_str(),
            );
            command_interpreter(game, db, vict_id, &to_force);
        }
    } else if arg == "room" {
        game.send_to_char(db,chid, OK);
        let ch = db.ch(chid);
        game.mudlog(db,
            NRM,
            max(LVL_GOD as i32, ch.get_invis_lev() as i32),
            true,
            format!(
                "(GC) {} forced room {} to {}",
                ch.get_name(),
                db.get_room_vnum(ch.in_room()),
                to_force
            )
            .as_str(),
        );
        let ch = db.ch(chid);
        let peoples_in_room = clone_vec2(&db.world[ch.in_room() as usize].peoples);
        for vict_id in peoples_in_room.clone() {
            let vict = db.ch(vict_id);
            let ch = db.ch(chid);
            if !vict.is_npc() && vict.get_level() >= ch.get_level() {
                continue;
            }
            game.act(db,
                &buf1,
                true,
                Some(chid),
                None,
                Some(VictimRef::Char(vict_id)),
                TO_VICT,
            );
            command_interpreter(game,db, vict_id, &to_force);
        }
    } else {
        /* force all */
        game.send_to_char(db,chid, OK);
        let ch = db.ch(chid);
        game.mudlog(db,
            NRM,
            max(LVL_GOD as i32, ch.get_invis_lev() as i32),
            true,
            format!("(GC) {} forced all to {}", ch.get_name(), to_force).as_str(),
        );
        for i in game.descriptor_list.ids() {
            let mut vict_id = None;
            let ch = db.ch(chid);
            if game.desc(i).state() != ConPlaying
                || {
                    vict_id = game.desc(i).character;
                    vict_id.is_none()
                }
                || !db.ch(vict_id.unwrap()).is_npc()
                    && db.ch(vict_id.unwrap()).get_level() >= ch.get_level()
            {
                continue;
            }
            game.act(db,
                &buf1,
                true,
                Some(chid),
                None,
                Some(VictimRef::Char(vict_id.unwrap())),
                TO_VICT,
            );
            command_interpreter(game, db, vict_id.unwrap(), &to_force);
        }
    }
}

pub fn do_wiznet(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut emote = false;
    let mut level = LVL_IMMORT;
    let mut buf1 = String::new();
    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if argument.is_empty() {
        game.send_to_char(db,chid, "Usage: wiznet <text> | #<level> <text> | *<emotetext> |\r\n        wiznet @<level> *<emotetext> | wiz @\r\n");
        return;
    }
    match argument.chars().next().unwrap() {
        '*' | '#' => {
            if argument.remove(0) == '*' {
                emote = true;
            }
            one_argument(&argument, &mut buf1);
            if is_number(&buf1) {
                let mut arg_left = argument.clone();
                half_chop(&mut arg_left, &mut buf1, &mut argument);
                level = max(buf1.parse::<i16>().unwrap(), LVL_IMMORT);
                if level > ch.get_level() as i16 {
                    game.send_to_char(db,chid, "You can't wizline above your own level.\r\n");
                    return;
                }
            } else if emote {
            }
        }

        '@' => {
            game.send_to_char(db,chid, "God channel status:\r\n");
            for d_id in game.descriptor_list.ids() {
                if game.desc(d_id).state() != ConPlaying
                    || 
                        db
                        .ch(game.desc(d_id).character.unwrap())
                        .get_level()
                        < LVL_IMMORT as u8
                {
                    continue;
                }
                let ch = db.ch(chid);
                if !game.can_see(db,ch, db.ch(game.desc(d_id).character.unwrap())) {
                    continue;
                }
                let dco = game.desc(d_id).character;
                let dc_id = dco.unwrap();
                let dc = db.ch(dc_id);
                game.send_to_char(db,
                    chid,
                    format!(
                        "  {:20}{}{}{}\r\n",
                        dc.get_name(),
                        if dc.plr_flagged(PLR_WRITING) {
                            " (Writing)"
                        } else {
                            ""
                        },
                        if dc.plr_flagged(PLR_MAILING) {
                            " (Writing mail)"
                        } else {
                            ""
                        },
                        if dc.prf_flagged(PRF_NOWIZ) {
                            " (Offline)"
                        } else {
                            ""
                        }
                    )
                    .as_str(),
                );
            }
            return;
        }

        '\\' => {
            argument.remove(0);
        }

        _ => {}
    }

    if ch.prf_flagged(PRF_NOWIZ) {
        game.send_to_char(db,chid, "You are offline!\r\n");
        return;
    }
    let argument = argument.trim_start();

    if argument.is_empty() {
        game.send_to_char(db,chid, "Don't bother the gods like that!\r\n");
        return;
    }
    let buf2;
    if level > LVL_IMMORT {
        buf1 = format!(
            "{}: <{}> {}{}\r\n",
            ch.get_name(),
            level,
            if emote { "<--- " } else { "" },
            argument
        );
        buf2 = format!(
            "Someone: <{}> {}{}\r\n",
            level,
            if emote { "<--- " } else { "" },
            argument
        );
    } else {
        buf1 = format!(
            "{}: {}{}\r\n",
            ch.get_name(),
            if emote { "<--- " } else { "" },
            argument
        );
        buf2 = format!(
            "Someone: {}{}\r\n",
            if emote { "<--- " } else { "" },
            argument
        );
    }
    for d_id in game.descriptor_list.ids() {
        if { let ch = db.ch(chid); game.desc(d_id).state() == ConPlaying
            && db.ch(game.desc(d_id).character.unwrap()).get_level() >= level as u8
            && !
                db
                .ch(game.desc(d_id).character.unwrap())
                .prf_flagged(PRF_NOWIZ)
            && !
                db
                .ch(game.desc(d_id).character.unwrap())
                .plr_flagged(PLR_WRITING | PLR_MAILING)
            && d_id == ch.desc.unwrap()
            || !
                db
                .ch(game.desc(d_id).character.unwrap())
                .prf_flagged(PRF_NOREPEAT)
        } {
            let chid = game.desc(d_id).character.as_ref().unwrap().clone();
            game.send_to_char(db,
                chid,
                CCCYN!(db.ch(game.desc(d_id).character.unwrap()), C_NRM),
            );
            let dc_id = game.desc(d_id).character.unwrap();
            let dc = db.ch(dc_id);
            let ch = db.ch(chid);
            if game.can_see(db,dc, ch) {
                game.send_to_char(db,dc_id, &buf1);
            } else {
                game.send_to_char(db,dc_id, &buf2);
            }
            let dc = db.ch(dc_id);
            game.send_to_char(db,dc_id, CCNRM!(dc, C_NRM));
        }
    }
    let ch = db.ch(chid);
    if ch.prf_flagged(PRF_NOREPEAT) {
        game.send_to_char(db,chid, OK);
    }
}

pub fn do_zreset(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        game.send_to_char(db,chid, "You must specify a zone.\r\n");
        return;
    }
    let zone_count = db.zone_table.len();
    let mut i = zone_count;
    if arg.starts_with('*') {
        for i in 0..zone_count {
            game.reset_zone(db, i);
        }
        game.send_to_char(db,chid, "Reset world.\r\n");
        let ch = db.ch(chid);
        game.mudlog(db,
            NRM,
            max(LVL_GRGOD as i32, ch.get_invis_lev() as i32),
            true,
            format!("(GC) {} reset entire world.", ch.get_name()).as_str(),
        );
        return;
    } else if arg.starts_with('.') {
        i = db.world[ch.in_room() as usize].zone as usize;
    } else {
        let j = arg.parse::<i32>();
        if j.is_err() {
            return;
        };
        let j = j.unwrap();
        for ii in 0..db.zone_table.len() {
            if db.zone_table[ii].number == j as i16 {
                i = ii;
                break;
            }
        }
    }
    if i < db.zone_table.len() {
        game.reset_zone(db, i as usize);
        game.send_to_char(db,
            chid,
            format!(
                "Reset zone {} (#{}): {}.\r\n",
                i, db.zone_table[i].number, db.zone_table[i].name
            )
            .as_str(),
        );
        let ch = db.ch(chid);
        game.mudlog(db,
            NRM,
            max(LVL_GRGOD as i32, ch.get_invis_lev() as i32),
            true,
            format!(
                "(GC) {} reset zone {} ({})",
                ch.get_name(),
                i,
                db.zone_table[i].name
            )
            .as_str(),
        );
    } else {
        game.send_to_char(db,chid, "Invalid zone number.\r\n");
    }
}

/*
 *  General fn for wizcommands of the sort: cmd <player>
 */
pub fn do_wizutil(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();
    one_argument(argument, &mut arg);
    let vict_id;
    if arg.is_empty() {
        game.send_to_char(db,chid, "Yes, but for whom?!?\r\n");
    } else if {
        vict_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_WORLD);
        vict_id.is_none()
    } {
        game.send_to_char(db,chid, "There is no such player.\r\n");
    } else if db.ch(vict_id.unwrap()).is_npc() {
        game.send_to_char(db,chid, "You can't do that to a mob!\r\n");
    } else if db.ch(vict_id.unwrap()).get_level() > ch.get_level() {
        game.send_to_char(db,chid, "Hmmm...you'd better not.\r\n");
    } else {
        let vict_id = vict_id.unwrap();
        let vict = db.ch(vict_id);
        match subcmd {
            SCMD_REROLL => {
                game.send_to_char(db,chid, "Rerolled...\r\n");
                let vict = db.ch_mut(vict_id);
                roll_real_abils(vict);
                let ch = db.ch(chid);
                let vict = db.ch(vict_id);
                info!("(GC) {} has rerolled {}.", ch.get_name(), vict.get_name());
                game.send_to_char(db,
                    chid,
                    format!(
                        "New stats: Str {}/{}, Int {}, Wis {}, Dex {}, Con {}, Cha {}\r\n",
                        vict.get_str(),
                        vict.get_add(),
                        vict.get_int(),
                        vict.get_wis(),
                        vict.get_dex(),
                        vict.get_con(),
                        vict.get_cha()
                    )
                    .as_str(),
                );
            }
            SCMD_PARDON => {
                if !vict.plr_flagged(PLR_THIEF | PLR_KILLER) {
                    game.send_to_char(db,chid, "Your victim is not flagged.\r\n");
                    return;
                }
                let vict = db.ch_mut(vict_id);
                vict.remove_plr_flag(PLR_THIEF | PLR_KILLER);
                game.send_to_char(db,chid, "Pardoned.\r\n");
                game.send_to_char(db,vict_id, "You have been pardoned by the Gods!\r\n");
                let ch = db.ch(chid);
                let vict = db.ch(vict_id);
                game.mudlog(db,
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} pardoned by {}", vict.get_name(), ch.get_name()).as_str(),
                );
            }
            SCMD_NOTITLE => {
                let vict = db.ch_mut(vict_id);
                let result = vict.plr_tog_chk(PLR_NOTITLE);
                let ch = db.ch(chid);
                let vict = db.ch(vict_id);
                game.mudlog(db,
                    NRM,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!(
                        "(GC) Notitle {} for {} by {}.",
                        onoff!(result != 0),
                        vict.get_name(),
                        ch.get_name()
                    )
                    .as_str(),
                );
                let vict = db.ch(vict_id);
                let ch = db.ch(chid);
                game.send_to_char(db,
                    chid,
                    format!(
                        "(GC) Notitle {} for {} by {}.\r\n",
                        onoff!(result != 0),
                        vict.get_name(),
                        ch.get_name()
                    )
                    .as_str(),
                );
            }
            SCMD_SQUELCH => {
                let vict = db.ch_mut(vict_id);
                let result = vict.plr_tog_chk(PLR_NOSHOUT);
                let ch = db.ch(chid);
                let vict = db.ch(vict_id);
                game.mudlog(db,
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!(
                        "(GC) Squelch {} for {} by {}.",
                        onoff!(result != 0),
                        vict.get_name(),
                        ch.get_name()
                    )
                    .as_str(),
                );
                let vict = db.ch(vict_id);
                let ch = db.ch(chid);
                game.send_to_char(db,
                    chid,
                    format!(
                        "(GC) Squelch {} for {} by {}.\r\n",
                        onoff!(result != 0),
                        vict.get_name(),
                        ch.get_name()
                    )
                    .as_str(),
                );
            }
            SCMD_FREEZE => {
                if chid == vict_id {
                    game.send_to_char(db,chid, "Oh, yeah, THAT'S real smart...\r\n");
                    return;
                }
                if vict.plr_flagged(PLR_FROZEN) {
                    game.send_to_char(db,chid, "Your victim is already pretty cold.\r\n");
                    return;
                }
                let vict = db.ch_mut(vict_id);
                vict.set_plr_flag_bit(PLR_FROZEN);
                let ch = db.ch(chid);
                let val = ch.get_level();
                let vict = db.ch_mut(vict_id);
                vict.set_freeze_lev(val as i8);
                game.send_to_char(db,vict_id, "A bitter wind suddenly rises and drains every erg of heat from your body!\r\nYou feel frozen!\r\n");
                game.send_to_char(db,chid, "Frozen.\r\n");
                game.act(db,
                    "A sudden cold wind conjured from nowhere freezes $n!",
                    false,
                    Some(vict_id),
                    None,
                    None,
                    TO_ROOM,
                );
                let vict = db.ch(vict_id);
                let ch = db.ch(chid);
                game.mudlog(db,
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} frozen by {}.", vict.get_name(), ch.get_name()).as_str(),
                );
            }
            SCMD_THAW => {
                if !vict.plr_flagged(PLR_FROZEN) {
                    game.send_to_char(db,
                        chid,
                        "Sorry, your victim is not morbidly encased in ice at the moment.\r\n",
                    );
                    return;
                }
                if vict.get_freeze_lev() > ch.get_level() as i8 {
                    game.send_to_char(db,
                        chid,
                        format!(
                            "Sorry, a level {} God froze {}... you can't unfreeze {}.\r\n",
                            vict.get_freeze_lev(),
                            vict.get_name(),
                            hmhr(vict)
                        )
                        .as_str(),
                    );
                    return;
                }
                game.mudlog(db,
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} un-frozen by {}.", vict.get_name(), ch.get_name()).as_str(),
                );
                let vict = db.ch_mut(vict_id);
                vict.remove_plr_flag(PLR_FROZEN);
                game.send_to_char(db,vict_id, "A fireball suddenly explodes in front of you, melting the ice!\r\nYou feel thawed.\r\n");
                game.send_to_char(db,chid, "Thawed.\r\n");
                game.act(db,
                    "A sudden fireball conjured from nowhere thaws $n!",
                    false,
                    Some(vict_id),
                    None,
                    None,
                    TO_ROOM,
                );
            }
            SCMD_UNAFFECT => {
                if vict.affected.len() != 0 {
                    while { let vict = db.ch(vict_id); vict.affected.len() != 0 } {
                        let af = db.ch(vict_id).affected[0];
                        db.affect_remove(vict_id, af);
                    }
                    game.send_to_char(db,
                        vict_id,
                        "There is a brief flash of light!\r\nYou feel slightly different.\r\n",
                    );
                    game.send_to_char(db,chid, "All spells removed.\r\n");
                } else {
                    game.send_to_char(db,chid, "Your victim does not have any affections!\r\n");
                    return;
                }
            }
            _ => {
                error!("SYSERR: Unknown subcmd {} passed to do_wizutil ", subcmd);
            }
        }
        game.save_char(db, vict_id);
    }
}

/* single zone printing fn used by "show zone" so it's not repeated in the
code 3 times ... -je, 4/6/93 */

fn print_zone_to_buf(db: &DB, buf: &mut String, zone: ZoneRnum) {
    let zone = &db.zone_table[zone as usize];
    buf.push_str(
        format!(
            "{:3} {:30} Age: {:3}; Reset: {:3} ({:1}); Range: {:5}-{:5}\r\n",
            zone.number, zone.name, zone.age, zone.lifespan, zone.reset_mode, zone.bot, zone.top
        )
        .as_str(),
    );
}

pub fn do_show(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut self_ = false;

    struct ShowStruct {
        cmd: &'static str,
        level: i16,
    }

    const FIELDS: [ShowStruct; 12] = [
        ShowStruct {
            cmd: "nothing",
            level: 0,
        }, /* 0 */
        ShowStruct {
            cmd: "zones",
            level: LVL_IMMORT,
        }, /* 1 */
        ShowStruct {
            cmd: "player",
            level: LVL_GOD,
        },
        ShowStruct {
            cmd: "rent",
            level: LVL_GOD,
        },
        ShowStruct {
            cmd: "stats",
            level: LVL_IMMORT,
        },
        ShowStruct {
            cmd: "errors",
            level: LVL_IMPL,
        }, /* 5 */
        ShowStruct {
            cmd: "death",
            level: LVL_GOD,
        },
        ShowStruct {
            cmd: "godrooms",
            level: LVL_GOD,
        },
        ShowStruct {
            cmd: "shops",
            level: LVL_IMMORT,
        },
        ShowStruct {
            cmd: "houses",
            level: LVL_GOD,
        },
        ShowStruct {
            cmd: "snoop",
            level: LVL_GRGOD,
        }, /* 10 */
        ShowStruct {
            cmd: "\n",
            level: 0,
        },
    ];

    let argument = argument.trim_start();

    if argument.is_empty() {
        game.send_to_char(db,chid, "Show options:\r\n");
        let mut j = 0;
        for i in 1..FIELDS.len() - 1 {
            let ch = db.ch(chid);
            if FIELDS[i].level <= ch.get_level() as i16 {
                game.send_to_char(db,
                    chid,
                    format!(
                        "{:15}{}",
                        FIELDS[i].cmd,
                        if {
                            j += 1;
                            j % 5 == 0
                        } {
                            "\r\n"
                        } else {
                            ""
                        }
                    )
                    .as_str(),
                );
            }
        }
        game.send_to_char(db,chid, "\r\n");
        return;
    }

    let mut field = String::new();
    let mut value = String::new();
    two_arguments(argument, &mut field, &mut value);

    let l = FIELDS.iter().position(|f| f.cmd == field);
    let l = if l.is_some() { l.unwrap() } else { 0 };

    if ch.get_level() < FIELDS[l].level as u8 {
        game.send_to_char(db,chid, "You are not godly enough for that!\r\n");
        return;
    }
    if value == "." {
        self_ = true;
    }
    let mut buf = String::new();

    match l {
        /* show zone */
        1 => {
            /* tightened up by JE 4/6/93 */
            if self_ {
                print_zone_to_buf(
                    &db,
                    &mut buf,
                    db.world[ch.in_room() as usize].zone,
                );
            } else if !value.is_empty() && is_number(&value) {
                let value = value.parse::<i32>().unwrap();
                let zrn = 
                    db
                    .zone_table
                    .iter()
                    .position(|z| z.number == value as i16);
                if zrn.is_some() {
                    print_zone_to_buf(&db, &mut buf, zrn.unwrap() as ZoneRnum);
                } else {
                    game.send_to_char(db,chid, "That is not a valid zone.\r\n");
                    return;
                }
            } else {
                for i in 0..db.zone_table.len() {
                    print_zone_to_buf(&db, &mut buf, i as ZoneRnum);
                }
            }
            let desc_id = ch.desc.unwrap();
            page_string(game, db, desc_id, &buf, true);
        }

        /* show player */
        2 => {
            if value.is_empty() {
                game.send_to_char(db,chid, "A name would help.\r\n");
                return;
            }

            let mut vbuf = CharFileU::new();
            if db.load_char(&value, &mut vbuf).is_none() {
                game.send_to_char(db,chid, "There is no such player.\r\n");
                return;
            }

            game.send_to_char(db,
                chid,
                format!(
                    "Player: {:12} ({}) [{:2} {}]\r\n",
                    parse_c_string(&vbuf.name),
                    GENDERS[vbuf.sex as usize],
                    vbuf.level,
                    CLASS_ABBREVS[vbuf.chclass as usize]
                )
                .as_str(),
            );
            let g = vbuf.points.gold;
            let bg = vbuf.points.bank_gold;
            let exp = vbuf.points.exp;
            let ali = vbuf.char_specials_saved.alignment;
            let stl = vbuf.player_specials_saved.spells_to_learn;
            game.send_to_char(db,
                chid,
                format!(
                    "Au:{:8}  Bal:{:8}  Exp:{:8}  Align: {:5}  Lessons: {:3}\r\n",
                    g, bg, exp, ali, stl
                )
                .as_str(),
            );
            /* ctime() uses static buffer: do not combine. */
            game.send_to_char(db,chid, format!("Started: {}  ", ctime(vbuf.birth)).as_str());
            game.send_to_char(db,
                chid,
                format!(
                    "Last: {:20}  Played: {:3}h {:2}m\r\n",
                    ctime(vbuf.last_logon),
                    vbuf.played / 3600,
                    vbuf.played / 60 % 60
                )
                .as_str(),
            );
        }

        /* show rent */
        3 => {
            if value.is_empty() {
                game.send_to_char(db,chid, "A name would help.\r\n");
                return;
            }
            crash_listrent(game, db, chid, &value);
        }

        /* show stats */
        4 => {
            let mut i = 0;
            let mut j = 0;
            let mut k = 0;
            let mut con = 0;
            for vict in db.character_list.iter() {
                if vict.is_npc() {
                    j += 1;
                } else if game.can_see(db,ch, vict) {
                    i += 1;
                    if vict.desc.is_some() {
                        con += 1;
                    }
                }
            }
            for _ in db.object_list.iter() {
                k += 1;
            }
            game.send_to_char(db,
                chid,
                format!(
                    "Current stats:\r\n\
                               {:5} players in game  {:5} connected\r\n\
                               {:5} registered\r\n\
                               {:5} mobiles          {:5} prototypes\r\n\
                               {:5} objects          {:5} prototypes\r\n\
                               {:5} rooms            {:5} zones\r\n",
                    i,
                    con,
                    db.player_table.len(),
                    j,
                    db.mob_protos.len(),
                    k,
                    db.obj_proto.len(),
                    db.world.len(),
                    db.zone_table.len()
                )
                .as_str(),
            );
        }

        /* show errors */
        5 => {
            let mut buf = "Errant Rooms\r\n------------\r\n".to_string();
            let mut k = 0;
            for i in 0..db.world.len() {
                for j in 0..NUM_OF_DIRS {
                    if db.world[i].dir_option[j].is_some()
                        && db.world[i].dir_option[j].as_ref().unwrap().to_room == 0
                    {
                        k += 1;

                        buf.push_str(
                            format!(
                                "{:2}: [{:5}] {}\r\n",
                                k,
                                db.get_room_vnum(i as RoomVnum),
                                db.world[i].name
                            )
                            .as_str(),
                        )
                    }
                }
            }
            let desc_id = ch.desc.unwrap();
            page_string(game, db, desc_id, &buf, true);
        }

        /* show death */
        6 => {
            let mut buf = "Death Traps\r\n-----------\r\n".to_string();
            let mut j = 0;
            for i in 0..db.world.len() {
                if db.room_flagged(i as RoomRnum, ROOM_DEATH) {
                    j += 1;
                    buf.push_str(
                        format!(
                            "{:2}: [{:5}] {}\r\n",
                            j,
                            db.get_room_vnum(i as RoomVnum),
                            db.world[i].name
                        )
                        .as_str(),
                    );
                }
            }
            let desc_id = ch.desc.unwrap();
            page_string(game, db, desc_id, &buf, true);
        }

        /* show godrooms */
        7 => {
            let mut buf = "Godrooms\r\n--------------------------\r\n".to_string();
            let mut j = 0;
            for i in 0..db.world.len() {
                if db.room_flagged(i as RoomRnum, ROOM_GODROOM) {
                    j += 1;
                    buf.push_str(
                        format!(
                            "{:2}: [{:5}] {}\r\n",
                            j,
                            db.get_room_vnum(i as RoomVnum),
                            db.world[i].name
                        )
                        .as_str(),
                    );
                }
            }
            let desc_id = ch.desc.unwrap();
            page_string(game, db, desc_id, &buf, true);
        }

        /* show shops */
        8 => {
            show_shops(game, db, chid, &value);
        }

        /* show houses */
        9 => {
            hcontrol_list_houses(game, db, chid);
        }

        /* show snoop */
        10 => {
            let mut i = 0;
            game.send_to_char(db,
                chid,
                "People currently snooping:\r\n--------------------------\r\n",
            );
            for d_id in game.descriptor_list.ids() {
                if game.desc(d_id).snooping.borrow().is_none()
                    || game.desc(d_id).character.is_none()
                {
                    continue;
                }
                let dco = game.desc(d_id).character;
                let dc_id = dco.unwrap();
                let dc = db.ch(dc_id);
                let ch = db.ch(chid);
                if game.desc(d_id).state() != ConPlaying || ch.get_level() < dc.get_level() {
                    continue;
                }
                if !game.can_see(db,ch, dc) || dc.in_room() == NOWHERE {
                    continue;
                }
                i += 1;
                game.send_to_char(db,
                    chid,
                    format!(
                        "{:10} - snooped by {}.\r\n",
                        db
                            .ch(game
                                .desc(game.desc(d_id).snooping.unwrap())
                                .character
                                .unwrap())
                            .get_name(),
                        dc.get_name()
                    )
                    .as_str(),
                );
            }
            if i == 0 {
                game.send_to_char(db,chid, "No one is currently snooping.\r\n");
            }
        }

        /* show what? */
        _ => {
            game.send_to_char(db,chid, "Sorry, I don't understand that.\r\n");
        }
    }
}

/***************** The do_set function ***********************************/

const PC: u8 = 1;
const NPC: u8 = 2;
const BOTH: u8 = 3;

const MISC: u8 = 0;
const BINARY: u8 = 1;
const NUMBER: u8 = 2;

macro_rules! range {
    ($value:expr, $low:expr, $high:expr) => {
        max($low as i32, min($high as i32, $value))
    };
}

/* The set options available */
struct SetStruct {
    cmd: &'static str,
    level: i16,
    pcnpc: u8,
    type_: u8,
}

const SET_FIELDS: [SetStruct; 52] = [
    SetStruct {
        cmd: "brief",
        level: LVL_GOD,
        pcnpc: PC,
        type_: BINARY,
    }, /* 0 */
    SetStruct {
        cmd: "invstart",
        level: LVL_GOD,
        pcnpc: PC,
        type_: BINARY,
    }, /* 1 */
    SetStruct {
        cmd: "title",
        level: LVL_GOD,
        pcnpc: PC,
        type_: MISC,
    },
    SetStruct {
        cmd: "nosummon",
        level: LVL_GRGOD,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "maxhit",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "maxmana",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    }, /* 5 */
    SetStruct {
        cmd: "maxmove",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "hit",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "mana",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "move",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "align",
        level: LVL_GOD,
        pcnpc: BOTH,
        type_: NUMBER,
    }, /* 10 */
    SetStruct {
        cmd: "str",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "stradd",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "int",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "wis",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "dex",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    }, /* 15 */
    SetStruct {
        cmd: "con",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "cha",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "ac",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "gold",
        level: LVL_GOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "bank",
        level: LVL_GOD,
        pcnpc: PC,
        type_: NUMBER,
    }, /* 20 */
    SetStruct {
        cmd: "exp",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "hitroll",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "damroll",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "invis",
        level: LVL_IMPL,
        pcnpc: PC,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "nohassle",
        level: LVL_GRGOD,
        pcnpc: PC,
        type_: BINARY,
    }, /* 25 */
    SetStruct {
        cmd: "frozen",
        level: LVL_FREEZE as i16,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "practices",
        level: LVL_GRGOD,
        pcnpc: PC,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "lessons",
        level: LVL_GRGOD,
        pcnpc: PC,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "drunk",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: MISC,
    },
    SetStruct {
        cmd: "hunger",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: MISC,
    }, /* 30 */
    SetStruct {
        cmd: "thirst",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: MISC,
    },
    SetStruct {
        cmd: "killer",
        level: LVL_GOD,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "thief",
        level: LVL_GOD,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "level",
        level: LVL_IMPL,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "room",
        level: LVL_IMPL,
        pcnpc: BOTH,
        type_: NUMBER,
    }, /* 35 */
    SetStruct {
        cmd: "roomflag",
        level: LVL_GRGOD,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "siteok",
        level: LVL_GRGOD,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "deleted",
        level: LVL_IMPL,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "class",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: MISC,
    },
    SetStruct {
        cmd: "nowizlist",
        level: LVL_GOD,
        pcnpc: PC,
        type_: BINARY,
    }, /* 40 */
    SetStruct {
        cmd: "quest",
        level: LVL_GOD,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "loadroom",
        level: LVL_GRGOD,
        pcnpc: PC,
        type_: MISC,
    },
    SetStruct {
        cmd: "color",
        level: LVL_GOD,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "idnum",
        level: LVL_IMPL,
        pcnpc: PC,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "passwd",
        level: LVL_IMPL,
        pcnpc: PC,
        type_: MISC,
    }, /* 45 */
    SetStruct {
        cmd: "nodelete",
        level: LVL_GOD,
        pcnpc: PC,
        type_: BINARY,
    },
    SetStruct {
        cmd: "sex",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: MISC,
    },
    SetStruct {
        cmd: "age",
        level: LVL_GRGOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "height",
        level: LVL_GOD,
        pcnpc: BOTH,
        type_: NUMBER,
    },
    SetStruct {
        cmd: "weight",
        level: LVL_GOD,
        pcnpc: BOTH,
        type_: NUMBER,
    }, /* 50 */
    SetStruct {
        cmd: "\n",
        level: 0,
        pcnpc: BOTH,
        type_: MISC,
    },
];

fn perform_set(game: &mut Game, db: &mut DB, chid: DepotId, vict_id: DepotId, mode: i32, val_arg: &str) -> bool {
    let ch = db.ch(chid);
    let vict = db.ch(vict_id);

    let mut on = false;
    let mut off = false;
    let mut value = 0;
    let mode = mode as usize;

    /* Check to make sure all the levels are correct */
    if ch.get_level() != LVL_IMPL as u8 {
        if !vict.is_npc() && ch.get_level() <= vict.get_level() && vict_id != chid {
            game.send_to_char(db,chid, "Maybe that's not such a great idea...\r\n");
            return false;
        }
    }
    if ch.get_level() < SET_FIELDS[mode].level as u8 {
        game.send_to_char(db,chid, "You are not godly enough for that!\r\n");
        return false;
    }

    /* Make sure the PC/NPC is correct */
    if vict.is_npc() && SET_FIELDS[mode].pcnpc & NPC == 0 {
        game.send_to_char(db,chid, "You can't do that to a beast!\r\n");
        return false;
    } else if !vict.is_npc() && SET_FIELDS[mode].pcnpc & PC == 0 {
        game.send_to_char(db,chid, "That can only be done to a beast!\r\n");
        return false;
    }

    /* Find the value of the argument */
    if SET_FIELDS[mode].type_ == BINARY {
        if val_arg == "on" || val_arg == "yes" {
            on = true;
        } else if val_arg == "off" || val_arg == "no" {
            off = true;
        }
        if !on || off {
            game.send_to_char(db,chid, "Value must be 'on' or 'off'.\r\n");
            return false;
        }
        game.send_to_char(db,
            chid,
            format!(
                "{} {} for {}.\r\n",
                SET_FIELDS[mode].cmd,
                onoff!(on),
                vict.get_name()
            )
            .as_str(),
        );
    } else if SET_FIELDS[mode].type_ == NUMBER {
        let r = val_arg.parse::<i32>();
        value = if r.is_ok() { r.unwrap() } else { 0 };
        game.send_to_char(db,
            chid,
            format!(
                "{}'s {} set to {}.\r\n",
                vict.get_name(),
                SET_FIELDS[mode].cmd,
                value
            )
            .as_str(),
        );
    } else {
        game.send_to_char(db,chid, OK);
    }
    let rnum;
    let vict = db.ch_mut(vict_id);
    match mode {
        0 => {
            if on {
                vict.set_prf_flags_bits(PRF_BRIEF)
            } else {
                vict.remove_prf_flags_bits(PRF_BRIEF)
            }
        }
        1 => {
            if on {
                vict.set_plr_flag_bit(PLR_INVSTART)
            } else {
                vict.remove_plr_flag(PLR_INVSTART)
            }
        }
        2 => {
            set_title(vict, Some(val_arg.to_string()));
            let messg = format!(
                "{}'s title is now: {}\r\n",
                vict.get_name(),
                vict.get_title()
            );
            game.send_to_char(db,
                chid,
                
                messg.as_str(),
            );
        }
        3 => {
            if on {
                vict.set_prf_flags_bits(PRF_SUMMONABLE)
            } else {
                vict.remove_prf_flags_bits(PRF_SUMMONABLE)
            }
            let messg = format!("Nosummon {} for {}.\r\n", onoff!(!on), vict.get_name());
            game.send_to_char(db,
                chid,
                messg.as_str(),
            );
        }
        4 => {
            vict.points.max_hit = range!(value, 1, 5000) as i16;
            db.affect_total(vict_id);
        }
        5 => {
            vict.points.max_mana = range!(value, 1, 5000) as i16;
            db.affect_total(vict_id);
        }
        6 => {
            vict.points.max_move = range!(value, 1, 5000) as i16;
            db.affect_total(vict_id);
        }
        7 => {
            vict.points.hit = range!(value, -9, vict.points.max_hit) as i16;
            db.affect_total(vict_id);
        }
        8 => {
            vict.points.mana = range!(value, 0, vict.points.max_mana) as i16;
            db.affect_total(vict_id);
        }
        9 => {
            vict.points.movem = range!(value, 0, vict.points.max_move) as i16;
            db.affect_total(vict_id);
        }
        10 => {
            vict.set_alignment(range!(value, -1000, 1000));
            db.affect_total(vict_id);
        }
        11 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.str = value as i8;
            vict.real_abils.str_add = 0;
            db.affect_total(vict_id);
        }
        12 => {
            vict.real_abils.str_add = range!(value, 0, 100) as i8;
            if value > 0 {
                vict.real_abils.str = 18;
            }
            db.affect_total(vict_id);
        }
        13 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.intel = value as i8;
            db.affect_total(vict_id);
        }
        14 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.wis = value as i8;
            db.affect_total(vict_id);
        }
        15 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.dex = value as i8;
            db.affect_total(vict_id);
        }
        16 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.con = value as i8;
            db.affect_total(vict_id);
        }
        17 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.cha = value as i8;
            db.affect_total(vict_id);
        }
        18 => {
            vict.points.armor = range!(value, -100, 100) as i16;
            db.affect_total(vict_id);
        }
        19 => {
            vict.set_gold(range!(value, 0, 100000000));
        }
        20 => {
            vict.set_bank_gold(range!(value, 0, 100000000));
        }
        21 => {
            vict.points.exp = range!(value, 0, 50000000);
        }
        22 => {
            vict.points.hitroll = range!(value, -20, 20) as i8;
            db.affect_total(vict_id);
        }
        23 => {
            vict.points.damroll = range!(value, -20, 20) as i8;
            db.affect_total(vict_id);
        }
        24 => {
            let ch = db.ch(chid);
            if ch.get_level() < LVL_IMPL as u8 && chid != vict_id {
                game.send_to_char(db,chid, "You aren't godly enough for that!\r\n");
                return false;
            }
            let vict = db.ch_mut(vict_id);
            vict.set_invis_lev(range!(value, 0, vict.get_level()) as i16);
        }
        25 => {
            let ch = db.ch(chid);
            if ch.get_level() < LVL_IMPL as u8 && chid != vict_id {
                game.send_to_char(db,chid, "You aren't godly enough for that!\r\n");
                return false;
            }
            let vict = db.ch_mut(vict_id);
            if on {
                vict.set_prf_flags_bits(PRF_NOHASSLE)
            } else {
                vict.remove_prf_flags_bits(PRF_NOHASSLE)
            }
        }
        26 => {
            if chid == vict_id && on {
                game.send_to_char(db,chid, "Better not -- could be a long winter!\r\n");
                return false;
            }
            if on {
                vict.set_plr_flag_bit(PLR_FROZEN)
            } else {
                vict.remove_plr_flag(PLR_FROZEN)
            }
        }
        27 | 28 => {
            vict.set_practices(range!(value, 0, 100));
        }
        29 | 30 | 31 => {
            if val_arg == "off" {
                vict.set_cond((mode - 29) as i32, -1); /* warning: magic number here */
                let vict = db.ch(vict_id);
                game.send_to_char(db,
                    chid,
                    format!(
                        "{}'s {} now off.\r\n",
                        vict.get_name(),
                        SET_FIELDS[mode].cmd
                    )
                    .as_str(),
                );
            } else if is_number(val_arg) {
                value = val_arg.parse::<i32>().unwrap();
                value = range!(value, 0, 24);
                vict.set_cond((mode - 29) as i32, value as i16); /* and here too */
                let vict = db.ch(vict_id);
                game.send_to_char(db,
                    chid,
                    format!(
                        "{}'s {} set to {}.\r\n",
                        vict.get_name(),
                        SET_FIELDS[mode].cmd,
                        value
                    )
                    .as_str(),
                );
            } else {
                game.send_to_char(db,chid, "Must be 'off' or a value from 0 to 24.\r\n");
                return false;
            }
        }
        32 => {
            if on {
                vict.set_plr_flag_bit(PLR_KILLER)
            } else {
                vict.remove_plr_flag(PLR_KILLER)
            }
        }
        33 => {
            if on {
                vict.set_plr_flag_bit(PLR_THIEF)
            } else {
                vict.remove_plr_flag(PLR_THIEF)
            }
        }
        34 => {
            let ch = db.ch(chid);
            if value > ch.get_level() as i32 || value > LVL_IMPL as i32 {
                game.send_to_char(db,chid, "You can't do that.\r\n");
                return false;
            }
            value = range!(value, 0, LVL_IMPL);
            db.ch_mut(vict_id).player.level = value as u8;
        }
        35 => {
            if {
                rnum = db.real_room(value as RoomRnum);
                rnum == NOWHERE
            } {
                game.send_to_char(db,chid, "No room exists with that number.\r\n");
                return false;
            }
            let vict = db.ch(vict_id);
            if vict.in_room() != NOWHERE {
                /* Another Eric Green special. */
                db.char_from_room(vict_id);
            }
            db.char_to_room(vict_id, rnum);
        }
        36 => {
            if on {
                vict.set_plr_flag_bit(PRF_ROOMFLAGS)
            } else {
                vict.remove_plr_flag(PRF_ROOMFLAGS)
            }
        }
        37 => {
            if on {
                vict.set_plr_flag_bit(PLR_SITEOK)
            } else {
                vict.remove_plr_flag(PLR_SITEOK)
            }
        }
        38 => {
            if on {
                vict.set_plr_flag_bit(PLR_DELETED)
            } else {
                vict.remove_plr_flag(PLR_DELETED)
            }
        }
        39 => {
            let i;
            if {
                i = parse_class(val_arg.chars().next().unwrap());
                i == CLASS_UNDEFINED
            } {
                game.send_to_char(db,chid, "That is not a class.\r\n");
                return false;
            }
            vict.set_class(i);
        }
        40 => {
            if on {
                vict.set_plr_flag_bit(PLR_NOWIZLIST)
            } else {
                vict.remove_plr_flag(PLR_NOWIZLIST)
            }
        }
        41 => {
            if on {
                vict.set_prf_flags_bits(PRF_QUEST)
            } else {
                vict.remove_prf_flags_bits(PRF_QUEST)
            }
        }
        42 => {
            if val_arg == "off" {
                vict.remove_prf_flags_bits(PLR_LOADROOM);
            } else if is_number(val_arg) {
                let rvnum = val_arg.parse::<i32>().unwrap() as RoomRnum;
                if db.real_room(rvnum) != NOWHERE {
                    let vict = db.ch_mut(vict_id);
                    vict.set_plr_flag_bit(PLR_LOADROOM);
                    vict.set_loadroom(rvnum);
                    let vict = db.ch(vict_id);
                    game.send_to_char(db,
                        chid,
                        format!(
                            "{} will enter at room #{}.",
                            vict.get_name(),
                            vict.get_loadroom()
                        )
                        .as_str(),
                    );
                } else {
                    game.send_to_char(db,chid, "That room does not exist!\r\n");
                    return false;
                }
            } else {
                game.send_to_char(db,chid, "Must be 'off' or a room's virtual number.\r\n");
                return false;
            }
        }
        43 => {
            if on {
                vict.set_prf_flags_bits(PRF_COLOR_1 | PRF_COLOR_2)
            } else {
                vict.remove_prf_flags_bits(PRF_COLOR_1 | PRF_COLOR_2)
            }
        }
        44 => {
            let ch = db.ch(chid);
            let vict = db.ch(vict_id);
            if ch.get_idnum() != 1 || !vict.is_npc() {
                return false;
            }
            let vict = db.ch_mut(vict_id);
            vict.set_idnum(value as i64);
        }
        45 => {
            let ch = db.ch(chid);
            if ch.get_idnum() > 1 {
                game.send_to_char(db,chid, "Please don't use this command, yet.\r\n");
                return false;
            }
            let vict = db.ch(vict_id);
            if vict.get_level() >= LVL_GRGOD as u8 {
                game.send_to_char(db,chid, "You cannot change that.\r\n");
                return false;
            }
            let mut passwd2 = [0 as u8; 16];
            let salt = vict.get_name();
            pbkdf2::pbkdf2::<Hmac<Sha256>>(val_arg.as_bytes(), salt.as_bytes(), 4, &mut passwd2)
                .expect("Error while encrypting password");
            let vict = db.ch_mut(vict_id);
            vict.set_passwd(passwd2);
            game.send_to_char(db,
                chid,
                format!("Password changed to '{}'.\r\n", val_arg).as_str(),
            );
        }
        46 => {
            if on {
                vict.set_plr_flag_bit(PLR_NODELETE)
            } else {
                vict.remove_plr_flag(PLR_NODELETE)
            }
        }
        47 => {
            let i;
            if {
                i = search_block(val_arg, &GENDERS, false);
                i.is_none()
            } {
                game.send_to_char(db,chid, "Must be 'male', 'female', or 'neutral'.\r\n");
                return false;
            }
            vict.set_sex(i.unwrap() as u8);
        }
        48 => {
            /* set age */
            if value < 2 || value > 200 {
                /* Arbitrary limits. */
                game.send_to_char(db,chid, "Ages 2 to 200 accepted.\r\n");
                return false;
            }
            /*
             * NOTE: May not display the exact age specified due to the integer
             * division used elsewhere in the code.  Seems to only happen for
             * some values below the starting age (17) anyway. -gg 5/27/98
             */
            db.ch_mut(vict_id).player.time.birth =
                time_now() - ((value as u64 - 17) * SECS_PER_MUD_YEAR);
        }

        49 => {
            /* Blame/Thank Rick Glover. :) */
            vict.set_height(value as u8);
            db.affect_total(vict_id);
        }

        50 => {
            vict.set_weight(value as u8);
            db.affect_total(vict_id);
        }

        _ => {
            game.send_to_char(db,chid, "Can't set that!\r\n");
            return false;
        }
    }

    true
}

pub fn do_set(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut player_i = None;
    let mut is_file = false;
    let mut is_player = false;
    let mut argument = argument.to_string();
    let mut name = String::new();
    let mut buf = String::new();
    let mut field = String::new();
    let mut tmp_store = CharFileU::new();

    half_chop(&mut argument, &mut name, &mut buf);

    if name == "file" {
        is_file = true;
        let mut buf2 = buf.clone();
        half_chop(&mut buf2, &mut name, &mut buf);
        buf = buf2;
    } else if name == "player" {
        is_player = true;
        let mut buf2 = buf.clone();
        half_chop(&mut buf2, &mut name, &mut buf);
        buf = buf2;
    } else if name == "mob" {
        let mut buf2 = buf.clone();
        half_chop(&mut buf2, &mut name, &mut buf);
        buf = buf2;
    }
    let mut buf2 = buf.clone();
    half_chop(&mut buf2, &mut field, &mut buf);
    buf = buf2;

    if name.is_empty() || field.is_empty() {
        game.send_to_char(db,chid, "Usage: set <victim> <field> <value>\r\n");
        return;
    }
    let mut vict_id = None;
    /* find the target */
    if !is_file {
        if is_player {
            if {
                vict_id = game.get_player_vis(db,chid, &mut name, None, FIND_CHAR_WORLD);
                vict_id.is_none()
            } {
                game.send_to_char(db,chid, "There is no such player.\r\n");
                return;
            }
        } else {
            /* is_mob */
            if {
                vict_id = game.get_char_vis(db,chid, &mut name, None, FIND_CHAR_WORLD);
                vict_id.is_none()
            } {
                game.send_to_char(db,chid, "There is no such creature.\r\n");
                return;
            }
        }
    } else if is_file {
        /* try to load the player off disk */
        let mut cbuf = CharData::default();
        clear_char(&mut cbuf);
        if {
            player_i = db.load_char(&name, &mut tmp_store);
            player_i.is_some()
        } {
            store_to_char(&tmp_store, &mut cbuf);
            let ch = db.ch(chid);
            if cbuf.get_level() >= ch.get_level() {
                game.send_to_char(db,chid, "Sorry, you can't do that.\r\n");
                return;
            }
            vict_id = Some(db.character_list.push(cbuf));
        } else {
            game.send_to_char(db,chid, "There is no such player.\r\n");
            return;
        }
    }

    /* find the command in the list */
    let mode = SET_FIELDS.iter().position(|e| e.cmd.starts_with(&field));
    let mode = if mode.is_some() {
        mode.unwrap()
    } else {
        SET_FIELDS.len() - 1
    };

    /* perform the set */
    let retval = perform_set(game, db, chid, vict_id.unwrap(), mode as i32, &buf);

    /* save the character if a change was made */
    if retval {
        if !is_file && !db.ch(vict_id.unwrap()).is_npc() {
            game.save_char(db, vict_id.unwrap());
        }
        if is_file {
            game.char_to_store(db, vict_id.unwrap(), &mut tmp_store);

            unsafe {
                let player_slice = slice::from_raw_parts(
                    &mut tmp_store as *mut _ as *mut u8,
                    mem::size_of::<CharFileU>(),
                );
                db
                    .player_fl
                    .as_mut()
                    .unwrap()
                    .write_all_at(
                        player_slice,
                        (player_i.unwrap() * mem::size_of::<CharFileU>()) as u64,
                    )
                    .expect("Error while writing player record to file");
            }

            game.send_to_char(db,chid, "Saved in file.\r\n");
        }
    }
    if is_file {
        db.character_list.take(vict_id.unwrap());
    }
}
