/* ************************************************************************
*   File: act.wizard.c                                  Part of CircleMUD *
*  Usage: Player-level god commands and other goodies                     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::borrow::Borrow;
use std::cmp::{max, min};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::rc::Rc;
use std::{mem, slice};

use chrono::{TimeZone, Utc};
use hmac::Hmac;
use log::{error, info};
use sha2::Sha256;

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
use crate::fight::{update_pos, ATTACK_HIT_TEXT};
use crate::handler::{affect_remove, affect_total, get_number, FIND_CHAR_ROOM, FIND_CHAR_WORLD};
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
    CharData, CharFileU, ObjData, RoomRnum, RoomVnum, ZoneRnum, AFF_HIDE, AFF_INVISIBLE,
    CLASS_UNDEFINED, DRUNK, FULL, ITEM_ARMOR, ITEM_CONTAINER, ITEM_DRINKCON, ITEM_FOOD,
    ITEM_FOUNTAIN, ITEM_KEY, ITEM_LIGHT, ITEM_MONEY, ITEM_NOTE, ITEM_POTION, ITEM_SCROLL,
    ITEM_STAFF, ITEM_TRAP, ITEM_WAND, ITEM_WEAPON, LVL_FREEZE, LVL_GOD, LVL_GRGOD, LVL_IMMORT,
    LVL_IMPL, MAX_OBJ_AFFECT, MAX_SKILLS, NOBODY, NOTHING, NOWHERE, NUM_OF_DIRS, NUM_WEARS,
    PLR_DELETED, PLR_FROZEN, PLR_INVSTART, PLR_KILLER, PLR_LOADROOM, PLR_MAILING, PLR_NODELETE,
    PLR_NOSHOUT, PLR_NOTITLE, PLR_NOWIZLIST, PLR_SITEOK, PLR_THIEF, PLR_WRITING, PRF_BRIEF,
    PRF_COLOR_1, PRF_COLOR_2, PRF_HOLYLIGHT, PRF_LOG1, PRF_LOG2, PRF_NOHASSLE, PRF_NOREPEAT,
    PRF_NOWIZ, PRF_QUEST, PRF_ROOMFLAGS, PRF_SUMMONABLE, ROOM_DEATH, ROOM_GODROOM, ROOM_HOUSE,
    ROOM_PRIVATE, THIRST,
};
use crate::util::{
    age, clone_vec, clone_vec2, ctime, hmhr, sprintbit, sprinttype, time_now, touch, BRF, NRM, SECS_PER_MUD_YEAR
};
use crate::{
    _clrlevel, clr, onoff, send_to_char, yesno, Game, CCCYN, CCGRN, CCNRM, CCYEL, TO_CHAR,
    TO_NOTVICT, TO_ROOM, TO_VICT,
};

pub fn do_echo(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let db = &game.db;
    let argument = argument.trim_start();

    if argument.is_empty() {
        send_to_char(ch, "Yes.. but what?\r\n");
    } else {
        let buf;
        if subcmd == SCMD_EMOTE {
            buf = format!("$n {}", argument);
        } else {
            buf = argument.to_string();
        }

        db.act(&buf, false, Some(ch), None, None, TO_ROOM);

        if ch.prf_flagged(PRF_NOREPEAT) {
            send_to_char(ch, OK);
        } else {
            db.act(&buf, false, Some(ch), None, None, TO_CHAR);
        }
    }
}

pub fn do_send(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut argument = argument.to_string();
    let mut arg = String::new();
    let mut buf = String::new();
    let vict;

    half_chop(&mut argument, &mut arg, &mut buf);

    if argument.is_empty() {
        send_to_char(ch, "Send what to who?\r\n");
        return;
    }
    if {
        vict = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(ch, NOPERSON);
        return;
    }
    send_to_char(vict.as_ref().unwrap(), format!("{}\r\n", buf).as_str());
    if ch.prf_flagged(PRF_NOREPEAT) {
        send_to_char(ch, "Sent.\r\n");
    } else {
        send_to_char(
            ch,
            format!(
                "You send '{}' to {}.\r\n",
                buf,
                vict.as_ref().unwrap().get_name()
            )
            .as_str(),
        );
    }
}

/* take a string, and return an rnum.. used for goto, at, etc.  -je 4/6/93 */
fn find_target_room(db: &DB, ch: &Rc<CharData>, rawroomstr: &str) -> RoomRnum {
    let mut location = NOWHERE;
    let mut roomstr = String::new();
    one_argument(rawroomstr, &mut roomstr);

    if roomstr.is_empty() {
        send_to_char(ch, "You must supply a room number or name.\r\n");
        return NOWHERE;
    }

    if roomstr.chars().next().unwrap().is_digit(10) && !roomstr.contains('.') {
        if {
            location = db.real_room(roomstr.parse::<i16>().unwrap());
            location == NOWHERE
        } {
            send_to_char(ch, "No room exists with that number.\r\n");
            return NOWHERE;
        }
    } else {
        let target_mob;
        let target_obj;
        let mut mobobjstr = roomstr;

        let mut num = get_number(&mut mobobjstr);
        if {
            target_mob = db.get_char_vis(ch, &mut mobobjstr, Some(&mut num), FIND_CHAR_WORLD);
            target_mob.is_some()
        } {
            if {
                location = target_mob.as_ref().unwrap().in_room();
                location == NOWHERE
            } {
                send_to_char(ch, "That character is currently lost.\r\n");
                return NOWHERE;
            }
        } else if {
            target_obj = db.get_obj_vis(ch, &mut mobobjstr, Some(&mut num));
            target_obj.is_some()
        } {
            if target_obj.as_ref().unwrap().in_room() != NOWHERE {
                location = target_obj.as_ref().unwrap().in_room();
            } else if target_obj.as_ref().unwrap().carried_by.borrow().is_some()
                && target_obj
                    .as_ref()
                    .unwrap()
                    .carried_by
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .in_room()
                    != NOWHERE
            {
                location = target_obj
                    .as_ref()
                    .unwrap()
                    .carried_by
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .in_room();
            } else if target_obj.as_ref().unwrap().worn_by.borrow().is_some()
                && target_obj
                    .as_ref()
                    .unwrap()
                    .worn_by
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .in_room()
                    != NOWHERE
            {
                location = target_obj
                    .as_ref()
                    .unwrap()
                    .worn_by
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .in_room();
            }

            if location == NOWHERE {
                send_to_char(ch, "That object is currently not in a room.\r\n");
                return NOWHERE;
            }
        }

        if location == NOWHERE {
            send_to_char(ch, "Nothing exists by that name.\r\n");
            return NOWHERE;
        }
    }

    /* a location has been found -- if you're >= GRGOD, no restrictions. */
    if ch.get_level() >= LVL_GRGOD as u8 {
        return location;
    }

    if db.room_flagged(location, ROOM_GODROOM) {
        send_to_char(ch, "You are not godly enough to use that room!\r\n");
    } else if db.room_flagged(location, ROOM_PRIVATE)
        && db.world[location as usize].peoples.borrow().len() > 1
    {
        send_to_char(
            ch,
            "There's a private conversation going on in that room.\r\n",
        );
    } else if db.room_flagged(location, ROOM_HOUSE)
        && !house_can_enter(db, ch, db.get_room_vnum(location))
    {
        send_to_char(ch, "That's private property -- no trespassing!\r\n");
    } else {
        return location;
    }

    return NOWHERE;
}

pub fn do_at(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut argument = argument.to_string();
    let mut buf = String::new();
    let mut command = String::new();

    half_chop(&mut argument, &mut buf, &mut command);
    if buf.is_empty() {
        send_to_char(ch, "You must supply a room number or a name.\r\n");
        return;
    }

    if command.is_empty() {
        send_to_char(ch, "What do you want to do there?\r\n");
        return;
    }
    let location;
    if {
        location = find_target_room(&game.db, ch, &buf);
        location == NOWHERE
    } {
        return;
    }

    /* a location has been found. */
    let original_loc = ch.in_room();
    game.db.char_from_room(ch);
    game.db.char_to_room(ch, location);
    command_interpreter(game, ch, &command);

    /* check if the char is still there */
    if ch.in_room() == location {
        game.db.char_from_room(ch);
        game.db.char_to_room(ch, original_loc);
    }
}

pub fn do_goto(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let location;

    if {
        location = find_target_room(&game.db, ch, argument);
        location == NOWHERE
    } {
        return;
    }

    let x = ch.poofout();
    let buf = format!(
        "$n {}",
        if !x.is_empty() {
            x.as_ref()
        } else {
            "disappears in a puff of smoke."
        }
    );
    game.db.act(&buf, true, Some(ch), None, None, TO_ROOM);

    game.db.char_from_room(ch);
    game.db.char_to_room(ch, location);

    let x = ch.poofin();
    let buf = format!(
        "$n {}",
        if !x.is_empty() {
            x.as_ref()
        } else {
            "appears with an ear-splitting bang."
        }
    );
    game.db.act(&buf, true, Some(ch), None, None, TO_ROOM);

    look_at_room(game, ch, false);
}

pub fn do_trans(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let victim;
    if buf.is_empty() {
        send_to_char(ch, "Whom do you wish to transfer?\r\n");
    } else if "all" != buf {
        if {
            victim = game.db.get_char_vis(ch, &mut buf, None, FIND_CHAR_WORLD);
            victim.is_none()
        } {
            send_to_char(ch, NOPERSON);
        } else if Rc::ptr_eq(victim.as_ref().unwrap(), ch) {
            send_to_char(ch, "That doesn't make much sense, does it?\r\n");
        } else {
            let victim = victim.as_ref().unwrap();
            if (ch.get_level() < victim.get_level()) && !victim.is_npc() {
                send_to_char(ch, "Go transfer someone your own size.\r\n");
                return;
            }
            game.db.act(
                "$n disappears in a mushroom cloud.",
                false,
                Some(victim),
                None,
                None,
                TO_ROOM,
            );
            game.db.char_from_room(victim);
            game.db.char_to_room(victim, ch.in_room());
            game.db.act(
                "$n arrives from a puff of smoke.",
                false,
                Some(victim),
                None,
                None,
                TO_ROOM,
            );
            game.db.act(
                "$n has transferred you!",
                false,
                Some(ch),
                None,
                Some(victim),
                TO_VICT,
            );
            look_at_room(game, victim, false);
        }
    } else {
        /* Trans All */
        if ch.get_level() < LVL_GRGOD as u8 {
            send_to_char(ch, "I think not.\r\n");
            return;
        }

        let list = clone_vec2(&game.descriptor_list);
        for i in list {
            if i.state() == ConPlaying
                && i.character.borrow().is_some()
                && !Rc::ptr_eq(i.character.borrow().as_ref().unwrap(), ch)
            {
                let ic = i.character.borrow();
                let victim = ic.as_ref().unwrap();
                if victim.get_level() >= ch.get_level() {
                    continue;
                }
                game.db.act(
                    "$n disappears in a mushroom cloud.",
                    false,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                game.db.char_from_room(victim);
                game.db.char_to_room(victim, ch.in_room());
                game.db.act(
                    "$n arrives from a puff of smoke.",
                    false,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                game.db.act(
                    "$n has transferred you!",
                    false,
                    Some(ch),
                    None,
                    Some(victim),
                    TO_VICT,
                );
                look_at_room(game, victim, false);
            }
        }
    }
    send_to_char(ch, OK);
}

pub fn do_teleport(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);
    let victim;
    let target;
    if buf.is_empty() {
        send_to_char(ch, "Whom do you wish to teleport?\r\n");
    } else if {
        victim = game.db.get_char_vis(ch, &mut buf, None, FIND_CHAR_WORLD);
        victim.is_none()
    } {
        send_to_char(ch, NOPERSON);
    } else if Rc::ptr_eq(victim.as_ref().unwrap(), ch) {
        send_to_char(ch, "Use 'goto' to teleport yourself.\r\n");
    } else if victim.as_ref().unwrap().get_level() >= ch.get_level() {
        send_to_char(ch, "Maybe you shouldn't do that.\r\n");
    } else if buf2.is_empty() {
        send_to_char(ch, "Where do you wish to send this person?\r\n");
    } else if {
        target = find_target_room(&game.db, ch, &buf2);
        target != NOWHERE
    } {
        let victim = victim.as_ref().unwrap();
        send_to_char(ch, OK);
        game.db.act(
            "$n disappears in a puff of smoke.",
            false,
            Some(victim),
            None,
            None,
            TO_ROOM,
        );
        game.db.char_from_room(victim);
        game.db.char_to_room(victim, target);
        game.db.act(
            "$n arrives from a puff of smoke.",
            false,
            Some(victim),
            None,
            None,
            TO_ROOM,
        );
        game.db.act(
            "$n has teleported you!",
            false,
            Some(ch),
            None,
            Some(victim),
            TO_VICT,
        );
        look_at_room(game, victim, false);
    }
}

pub fn do_vnum(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut buf = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !is_abbrev(&buf, "mob") && !is_abbrev(&buf, "obj") {
        send_to_char(ch, "Usage: vnum { obj | mob } <name>\r\n");
        return;
    }
    if is_abbrev(&buf, "mob") {
        if db.vnum_mobile(&buf2, ch) == 0 {
            send_to_char(ch, "No mobiles by that name.\r\n");
        }
    }

    if is_abbrev(&buf, "obj") {
        if db.vnum_object(&buf2, ch) == 0 {
            send_to_char(ch, "No objects by that name.\r\n");
        }
    }
}

fn do_stat_room(db: &DB, ch: &Rc<CharData>) {
    let rm = &db.world[ch.in_room() as usize];

    send_to_char(
        ch,
        format!(
            "Room name: {}{}{}\r\n",
            CCCYN!(ch, C_NRM),
            rm.name,
            CCNRM!(ch, C_NRM)
        )
        .as_str(),
    );
    let mut buf2 = String::new();
    sprinttype(rm.sector_type, &SECTOR_TYPES, &mut buf2);
    send_to_char(
        ch,
        format!(
            "Zone: [{:3}], VNum: [{}{:5}{}], RNum: [{:5}], Type: {}\r\n",
            db.zone_table.borrow()[rm.zone as usize].number,
            CCGRN!(ch, C_NRM),
            rm.number,
            CCNRM!(ch, C_NRM),
            ch.in_room(),
            buf2
        )
        .as_str(),
    );

    sprintbit(rm.room_flags.get() as i64, &ROOM_BITS, &mut buf2);
    send_to_char(
        ch,
        format!(
            "SpecProc: {}, Flags: {}\r\n",
            if rm.func.borrow().is_none() {
                "None"
            } else {
                "Exists"
            },
            buf2
        )
        .as_str(),
    );

    send_to_char(
        ch,
        format!(
            "Description:\r\n{}",
            if !rm.description.is_empty() {
                &rm.description
            } else {
                "  None.\r\n"
            }
        )
        .as_str(),
    );

    if !rm.ex_descriptions.is_empty() {
        send_to_char(ch, format!("Extra descs:{}", CCCYN!(ch, C_NRM)).as_str());
        for desc in rm.ex_descriptions.iter() {
            send_to_char(ch, format!(" {}", desc.keyword).as_str());
            send_to_char(ch, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());
        }

        send_to_char(ch, format!("Chars present:{}", CCYEL!(ch, C_NRM)).as_str());
        let mut column = 14; /* ^^^ strlen ^^^ */
        let mut found = 0;
        for (i, k) in rm.peoples.borrow().iter().enumerate() {
            if !db.can_see(ch, k) {
                continue;
            }

            column += send_to_char(
                ch,
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
                send_to_char(
                    ch,
                    format!(
                        "{}\r\n",
                        if i == rm.peoples.borrow().len() - 1 {
                            ","
                        } else {
                            ""
                        }
                    )
                    .as_str(),
                );
                found = 0;
                column = 0;
            }
        }
        send_to_char(ch, CCNRM!(ch, C_NRM));
    }
    if !rm.contents.borrow().is_empty() {
        send_to_char(ch, format!("Contents:{}", CCGRN!(ch, C_NRM)).as_str());
        let mut column = 9; /* ^^^ strlen ^^^ */
        let mut found = 0;
        for (i, j) in rm.contents.borrow().iter().enumerate() {
            if !db.can_see_obj(ch, j) {
                continue;
            }

            column += send_to_char(
                ch,
                format!(
                    "{} {}",
                    if found != 0 { "," } else { "" },
                    j.short_description
                )
                .as_str(),
            );
            found += 1;
            if column >= 62 {
                send_to_char(
                    ch,
                    format!(
                        "{}\r\n",
                        if i == rm.contents.borrow().len() - 1 {
                            ","
                        } else {
                            ""
                        }
                    )
                    .as_str(),
                );
                found = 0;
                column = 0;
            }
        }
        send_to_char(ch, format!("{}", CCNRM!(ch, C_NRM)).as_str());
    }

    for i in 0..NUM_OF_DIRS {
        if rm.dir_option[i].is_none() {
            continue;
        }
        let buf1;
        if rm.dir_option[i].as_ref().unwrap().to_room.get() == NOWHERE {
            buf1 = format!(" {}NONE{}", CCCYN!(ch, C_NRM), CCNRM!(ch, C_NRM));
        } else {
            buf1 = format!(
                "{}{:5}{}",
                CCCYN!(ch, C_NRM),
                db.get_room_vnum(rm.dir_option[i].as_ref().unwrap().to_room.get()),
                CCNRM!(ch, C_NRM)
            );
        }
        let mut buf2 = String::new();
        sprintbit(
            rm.dir_option[i].as_ref().unwrap().exit_info.get() as i64,
            &EXIT_BITS,
            &mut buf2,
        );

        send_to_char(
            ch,
            format!(
                "Exit {}{:5}{}:  To: [{}], Key: [{:5}], Keywrd: {}, Type: {}\r\n{}",
                CCCYN!(ch, C_NRM),
                DIRS[i],
                CCNRM!(ch, C_NRM),
                buf1,
                rm.dir_option[i].as_ref().unwrap().key,
                if !rm.dir_option[i].as_ref().unwrap().keyword.is_empty() {
                    &rm.dir_option[i].as_ref().unwrap().keyword
                } else {
                    "None"
                },
                buf2,
                if !rm.dir_option[i]
                    .as_ref()
                    .unwrap()
                    .general_description
                    .is_empty()
                {
                    &rm.dir_option[i].as_ref().unwrap().general_description
                } else {
                    "  No exit description.\r\n"
                }
            )
            .as_str(),
        );
    }
}

fn do_stat_object(db: &DB, ch: &Rc<CharData>, j: &Rc<ObjData>) {
    let vnum = db.get_obj_vnum(j);
    send_to_char(
        ch,
        format!(
            "Name: '{}{}{}', Aliases: {}\r\n",
            CCYEL!(ch, C_NRM),
            if !j.short_description.is_empty() {
                &j.short_description
            } else {
                "<None>"
            },
            CCNRM!(ch, C_NRM),
            j.name.borrow()
        )
        .as_str(),
    );
    let mut buf = String::new();
    sprinttype(j.get_obj_type() as i32, &ITEM_TYPES, &mut buf);
    send_to_char(
        ch,
        format!(
            "VNum: [{}{:5}{}], RNum: [{:5}], Type: {}, SpecProc: {}\r\n",
            CCGRN!(ch, C_NRM),
            vnum,
            CCNRM!(ch, C_NRM),
            j.get_obj_rnum(),
            buf,
            if db.get_obj_spec(j).is_some() {
                "Exists"
            } else {
                "none"
            }
        )
        .as_str(),
    );

    if !j.ex_descriptions.is_empty() {
        send_to_char(ch, format!("Extra descs:{}", CCCYN!(ch, C_NRM)).as_str());

        for desc in j.ex_descriptions.iter() {
            send_to_char(ch, format!(" {}", desc.keyword).as_str());
            send_to_char(ch, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());
        }
    }
    buf.clear();
    sprintbit(j.get_obj_wear() as i64, &WEAR_BITS, &mut buf);
    send_to_char(ch, format!("Can be worn on: {}\r\n", buf).as_str());
    buf.clear();
    sprintbit(j.get_obj_affect(), &AFFECTED_BITS, &mut buf);
    send_to_char(ch, format!("Set char bits : {}\r\n", buf).as_str());
    buf.clear();
    sprintbit(j.get_obj_extra() as i64, &EXTRA_BITS, &mut buf);
    send_to_char(ch, format!("Extra flags   : {}\r\n", buf).as_str());

    send_to_char(
        ch,
        format!(
            "Weight: {}, Value: {}, Cost/day: {}, Timer: {}\r\n",
            j.get_obj_weight(),
            j.get_obj_cost(),
            j.get_obj_rent(),
            j.get_obj_timer()
        )
        .as_str(),
    );
    send_to_char(
        ch,
        format!(
            "In room: {} ({}), ",
            db.get_room_vnum(j.in_room()),
            if j.in_room() == NOWHERE {
                "Nowhere"
            } else {
                db.world[j.in_room() as usize].name.as_str()
            }
        )
        .as_str(),
    );

    /*
     * NOTE: In order to make it this far, we must already be able to see the
     *       character holding the object. Therefore, we do not need CAN_SEE().
     */
    let jio = j.in_obj.borrow();
    send_to_char(
        ch,
        format!(
            "In object: {}, ",
            if j.in_obj.borrow().is_some() {
                &jio.as_ref().unwrap().short_description
            } else {
                "None"
            }
        )
        .as_str(),
    );
    send_to_char(
        ch,
        format!(
            "Carried by: {}, ",
            if j.carried_by.borrow().is_some() {
                j.carried_by.borrow().as_ref().unwrap().get_name()
            } else {
                Rc::from("Nobody")
            }
        )
        .as_str(),
    );
    send_to_char(
        ch,
        format!(
            "Worn by: {}\r\n",
            if j.worn_by.borrow().is_some() {
                j.worn_by.borrow().as_ref().unwrap().get_name()
            } else {
                Rc::from("Nobody")
            }
        )
        .as_str(),
    );

    match j.get_obj_type() {
        ITEM_LIGHT => {
            if j.get_obj_val(2) == -1 {
                send_to_char(ch, "Hours left: Infinite\r\n");
            } else {
                send_to_char(
                    ch,
                    format!("Hours left: [{}]\r\n", j.get_obj_val(2)).as_str(),
                );
            }
        }
        ITEM_SCROLL | ITEM_POTION => {
            send_to_char(
                ch,
                format!(
                    "Spells: (Level {}) {}, {}, {}\r\n",
                    j.get_obj_val(0),
                    skill_name(db, j.get_obj_val(1)),
                    skill_name(db, j.get_obj_val(2)),
                    skill_name(db, j.get_obj_val(3))
                )
                .as_str(),
            );
        }
        ITEM_WAND | ITEM_STAFF => {
            send_to_char(
                ch,
                format!(
                    "Spell: {} at level {}, {} (of {}) charges remaining\r\n",
                    skill_name(db, j.get_obj_val(3)),
                    j.get_obj_val(0),
                    j.get_obj_val(2),
                    j.get_obj_val(1)
                )
                .as_str(),
            );
        }
        ITEM_WEAPON => {
            send_to_char(
                ch,
                format!(
                    "Todam: {}d{}, Message type: {}\r\n",
                    j.get_obj_val(1),
                    j.get_obj_val(2),
                    j.get_obj_val(3)
                )
                .as_str(),
            );
        }
        ITEM_ARMOR => {
            send_to_char(ch, format!("AC-apply: [{}]\r\n", j.get_obj_val(0)).as_str());
        }
        ITEM_TRAP => {
            send_to_char(
                ch,
                format!(
                    "Spell: {}, - Hitpoints: {}\r\n",
                    j.get_obj_val(0),
                    j.get_obj_val(1)
                )
                .as_str(),
            );
        }
        ITEM_CONTAINER => {
            buf.clear();
            sprintbit(j.get_obj_val(1) as i64, &CONTAINER_BITS, &mut buf);
            send_to_char(
                ch,
                format!(
                    "Weight capacity: {}, Lock Type: {}, Key Num: {}, Corpse: {}\r\n",
                    j.get_obj_val(0),
                    buf,
                    j.get_obj_val(2),
                    yesno!(j.get_obj_val(3) != 0)
                )
                .as_str(),
            );
        }
        ITEM_DRINKCON | ITEM_FOUNTAIN => {
            buf.clear();
            sprinttype(j.get_obj_val(2), &DRINKS, &mut buf);
            send_to_char(
                ch,
                format!(
                    "Capacity: {}, Contains: {}, Poisoned: {}, Liquid: {}\r\n",
                    j.get_obj_val(0),
                    j.get_obj_val(1),
                    yesno!(j.get_obj_val(3) != 0),
                    buf
                )
                .as_str(),
            );
        }
        ITEM_NOTE => {
            send_to_char(ch, format!("Tongue: {}\r\n", j.get_obj_val(0)).as_str());
        }
        ITEM_KEY => { /* Nothing */ }
        ITEM_FOOD => {
            send_to_char(
                ch,
                format!(
                    "Makes full: {}, Poisoned: {}\r\n",
                    j.get_obj_val(0),
                    yesno!(j.get_obj_val(3) != 0)
                )
                .as_str(),
            );
        }
        ITEM_MONEY => {
            send_to_char(ch, format!("Coins: {}\r\n", j.get_obj_val(0)).as_str());
        }
        _ => {
            send_to_char(
                ch,
                format!(
                    "Values 0-3: [{}] [{}] [{}] [{}]\r\n",
                    j.get_obj_val(0),
                    j.get_obj_val(1),
                    j.get_obj_val(2),
                    j.get_obj_val(3)
                )
                .as_str(),
            );
        }
    }

    /*
     * I deleted the "equipment status" code from here because it seemed
     * more or less useless and just takes up valuable screen space.
     */

    if !j.contains.borrow().is_empty() {
        send_to_char(ch, format!("\r\nContents:{}", CCGRN!(ch, C_NRM)).as_str());
        let mut column = 9; /* ^^^ strlen ^^^ */
        let mut found = 0;

        for (i2, j2) in j.contains.borrow().iter().enumerate() {
            column += send_to_char(
                ch,
                format!(
                    "{} {}",
                    if found != 0 { "," } else { "" },
                    j2.short_description
                )
                .as_str(),
            );
            if column >= 62 {
                send_to_char(
                    ch,
                    format!(
                        "{}\r\n",
                        if i2 < j.contains.borrow().len() - 1 {
                            ","
                        } else {
                            ""
                        }
                    )
                    .as_str(),
                );
                found = 0;
                column = 0;
            }
        }
        send_to_char(ch, CCNRM!(ch, C_NRM));
    }

    let mut found = 0;
    send_to_char(ch, "Affections:");

    for i in 0..MAX_OBJ_AFFECT as usize {
        if j.affected[i].get().modifier != 0 {
            buf.clear();
            sprinttype(j.affected[i].get().location as i32, &APPLY_TYPES, &mut buf);
            send_to_char(
                ch,
                format!(
                    "{} {} to {}",
                    if found != 0 { "," } else { "" },
                    j.affected[i].get().modifier,
                    buf
                )
                .as_str(),
            );
            found += 1;
        }
        if found == 0 {
            send_to_char(ch, " None");
        }
        send_to_char(ch, "\r\n");
    }
}

fn do_stat_character(db: &DB, ch: &Rc<CharData>, k: &Rc<CharData>) {
    let mut buf = String::new();
    sprinttype(k.get_sex() as i32, &GENDERS, &mut buf);
    send_to_char(
        ch,
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

    if db.is_mob(k) {
        send_to_char(
            ch,
            format!(
                "Alias: {}, VNum: [{:5}], RNum: [{:5}]\r\n",
                k.player.borrow().name,
                db.get_mob_vnum(k),
                k.get_mob_rnum()
            )
            .as_str(),
        );
    }
    let kp = k.player.borrow();
    send_to_char(
        ch,
        format!(
            "Title: {}\r\n",
            if k.player.borrow().title.is_some() {
                kp.title.as_ref().unwrap()
            } else {
                "<None>"
            }
        )
        .as_str(),
    );

    send_to_char(
        ch,
        format!(
            "L-Des: {}",
            if !k.player.borrow().long_descr.is_empty() {
                &kp.long_descr
            } else {
                "<None>\r\n"
            }
        )
        .as_str(),
    );
    buf.clear();
    sprinttype(
        k.player.borrow().chclass as i32,
        if k.is_npc() {
            &NPC_CLASS_TYPES
        } else {
            &PC_CLASS_TYPES
        },
        &mut buf,
    );
    send_to_char(
        ch,
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

    if !k.is_npc() {
        let buf1 = ctime(k.player.borrow().time.birth);
        let buf2 = ctime(k.player.borrow().time.logon);
        send_to_char(
            ch,
            format!(
                "Created: [{}], Last Logon: [{}], Played [{}h {}m], Age [{}]\r\n",
                buf1,
                buf2,
                k.player.borrow().time.played / 3600,
                ((k.player.borrow().time.played % 3600) / 60),
                age(k).year
            )
            .as_str(),
        );

        send_to_char(
            ch,
            format!(
                "Hometown: [{}], Speaks: [{}/{}/{}], (STL[{}]/per[{}]/NSTL[{}])\r\n",
                k.player.borrow().hometown,
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
    send_to_char(
        ch,
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

    send_to_char(
        ch,
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

    send_to_char(
        ch,
        format!(
            "Coins: [{:9}], Bank: [{:9}] (Total: {})\r\n",
            k.get_gold(),
            k.get_bank_gold(),
            k.get_gold() + k.get_bank_gold()
        )
        .as_str(),
    );

    send_to_char(
        ch,
        format!(
            "AC: [{}{}/10], Hitroll: [{:2}], Damroll: [{:2}], Saving throws: [{}/{}/{}/{}/{}]\r\n",
            k.get_ac(),
            DEX_APP[k.get_dex() as usize].defensive,
            k.points.borrow().hitroll,
            k.points.borrow().damroll,
            k.get_save(0),
            k.get_save(1),
            k.get_save(2),
            k.get_save(3),
            k.get_save(4)
        )
        .as_str(),
    );
    buf.clear();
    sprinttype(k.get_pos() as i32, &POSITION_TYPES, &mut buf);
    send_to_char(
        ch,
        format!(
            "Pos: {}, Fighting: {}",
            buf,
            if k.fighting().is_some() {
                k.fighting().as_ref().unwrap().get_name()
            } else {
                Rc::from("Nobody")
            }
        )
        .as_str(),
    );

    if k.is_npc() {
        send_to_char(
            ch,
            format!(
                ", Attack type: {}",
                &ATTACK_HIT_TEXT[k.mob_specials.attack_type as usize].singular
            )
            .as_str(),
        );
    }

    if k.desc.borrow().is_some() {
        buf.clear();
        sprinttype(
            k.desc.borrow().as_ref().unwrap().state() as i32,
            &CONNECTED_TYPES,
            &mut buf,
        );
        send_to_char(ch, format!(", Connected: {}", buf).as_str());
    }

    if k.is_npc() {
        buf.clear();
        sprinttype(k.mob_specials.default_pos as i32, &POSITION_TYPES, &mut buf);
        send_to_char(ch, format!(", Default position: {}\r\n", buf).as_str());
        buf.clear();
        sprintbit(k.mob_flags(), &ACTION_BITS, &mut buf);
        send_to_char(
            ch,
            format!(
                "NPC flags: {}{}{}\r\n",
                CCCYN!(ch, C_NRM),
                buf,
                CCNRM!(ch, C_NRM)
            )
            .as_str(),
        );
    } else {
        send_to_char(
            ch,
            format!(
                ", Idle Timer (in tics) [{}]\r\n",
                k.char_specials.borrow().timer.get()
            )
            .as_str(),
        );
        buf.clear();
        sprintbit(k.plr_flags(), &PLAYER_BITS, &mut buf);
        send_to_char(
            ch,
            format!("PLR: {}{}{}\r\n", CCCYN!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
        );
        buf.clear();
        sprintbit(k.prf_flags(), &PREFERENCE_BITS, &mut buf);
        send_to_char(
            ch,
            format!("PRF: {}{}{}\r\n", CCGRN!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
        );
    }

    if db.is_mob(k) {
        send_to_char(
            ch,
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

    send_to_char(
        ch,
        format!(
            "Carried: weight: {}, items: {}; Items in: inventory: {}, ",
            k.is_carrying_w(),
            k.is_carrying_n(),
            k.carrying.borrow().len()
        )
        .as_str(),
    );

    let mut i2 = 0;
    for i in 0..NUM_WEARS {
        if k.get_eq(i).is_some() {
            i2 += 1;
        }
    }

    send_to_char(ch, format!("eq: {}\r\n", i2).as_str());

    if !k.is_npc() {
        send_to_char(
            ch,
            format!(
                "Hunger: {}, Thirst: {}, Drunk: {}\r\n",
                k.get_cond(FULL),
                k.get_cond(THIRST),
                k.get_cond(DRUNK)
            )
            .as_str(),
        );
    }

    let mut column = send_to_char(
        ch,
        format!(
            "Master is: {}, Followers are:",
            if k.master.borrow().is_some() {
                k.master.borrow().as_ref().unwrap().get_name()
            } else {
                Rc::from("<none>")
            }
        )
        .as_str(),
    );
    if k.followers.borrow().is_empty() {
        send_to_char(ch, " <none>\r\n");
    } else {
        let mut found = 0;
        for (i, fol) in k.followers.borrow().iter().enumerate() {
            column += send_to_char(
                ch,
                format!(
                    "{} {}",
                    if found != 0 { "," } else { "" },
                    db.pers(fol.follower.borrow(), ch)
                )
                .as_str(),
            );
            found += 1;
            if column >= 62 {
                send_to_char(
                    ch,
                    format!(
                        "{}\r\n",
                        if i < k.followers.borrow().len() - 1 {
                            ","
                        } else {
                            ""
                        }
                    )
                    .as_str(),
                );
                found = 0;
                column = 0;
            }
        }
        if column != 0 {
            send_to_char(ch, "\r\n");
        }
    }

    /* Showing the bitvector */
    buf.clear();
    sprintbit(k.aff_flags(), &AFFECTED_BITS, &mut buf);
    send_to_char(
        ch,
        format!("AFF: {}{}{}\r\n", CCYEL!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
    );

    /* Routine to show what spells a char is affected by */
    if k.affected.borrow().len() != 0 {
        for aff in k.affected.borrow().iter() {
            send_to_char(
                ch,
                format!(
                    "SPL: ({:3}hr) {}{:21}{} ",
                    aff.duration + 1,
                    CCCYN!(ch, C_NRM),
                    skill_name(db, aff._type as i32),
                    CCNRM!(ch, C_NRM)
                )
                .as_str(),
            );

            if aff.modifier != 0 {
                send_to_char(
                    ch,
                    format!(
                        "{} to {}",
                        aff.modifier, &APPLY_TYPES[aff.location as usize]
                    )
                    .as_str(),
                );
            }

            if aff.bitvector != 0 {
                if aff.modifier != 0 {
                    send_to_char(ch, ", ");
                }
                buf.clear();
                sprintbit(aff.bitvector, &AFFECTED_BITS, &mut buf);
                send_to_char(ch, format!("sets {}", buf).as_str());
            }
            send_to_char(ch, "\r\n");
        }
    }
}

pub fn do_stat(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf1 = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut buf1, &mut buf2);

    if buf1.is_empty() {
        send_to_char(ch, "Stats on who or what?\r\n");
        return;
    } else if is_abbrev(&buf1, "room") {
        do_stat_room(&game.db, ch);
    } else if is_abbrev(&buf1, "mob") {
        if buf2.is_empty() {
            send_to_char(ch, "Stats on which mobile?\r\n");
        } else {
            let victim;
            if {
                victim = game.db.get_char_vis(ch, &mut buf2, None, FIND_CHAR_WORLD);
                victim.is_some()
            } {
                do_stat_character(&game.db, ch, victim.as_ref().unwrap());
            } else {
                send_to_char(ch, "No such mobile around.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "player") {
        if buf2.is_empty() {
            send_to_char(ch, "Stats on which player?\r\n");
        } else {
            let victim;
            if {
                victim = game.db.get_player_vis(ch, &mut buf2, None, FIND_CHAR_WORLD);
                victim.is_some()
            } {
                do_stat_character(&game.db, ch, victim.as_ref().unwrap());
            } else {
                send_to_char(ch, "No such player around.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "file") {
        let victim;
        if buf2.is_empty() {
            send_to_char(ch, "Stats on which player?\r\n");
        } else if {
            victim = game.db.get_player_vis(ch, &mut buf2, None, FIND_CHAR_WORLD);
            victim.is_some()
        } {
            do_stat_character(&game.db, ch, victim.as_ref().unwrap());
        } else {
            let mut victim = CharData::new();
            let mut tmp_store = CharFileU::new();
            clear_char(&mut victim);
            if game.db.load_char(&buf2, &mut tmp_store).is_some() {
                store_to_char(&tmp_store, &mut victim);
                victim.player.borrow_mut().time.logon = tmp_store.last_logon;
                let victim = Rc::new(victim);
                game.db.char_to_room(&victim, 0);
                if victim.get_level() > ch.get_level() {
                    send_to_char(ch, "Sorry, you can't do that.\r\n");
                } else {
                    do_stat_character(&game.db, ch, &victim);
                }
                game.extract_char_final(&victim);
            } else {
                send_to_char(ch, "There is no such player.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "object") {
        if buf2.is_empty() {
            send_to_char(ch, "Stats on which object?\r\n");
        } else {
            let object;
            if {
                object = game.db.get_obj_vis(ch, &mut buf2, None);
                object.is_some()
            } {
                do_stat_object(&game.db, ch, object.as_ref().unwrap());
            } else {
                send_to_char(ch, "No such object around.\r\n");
            }
        }
    } else {
        let mut name = buf1;
        let mut number = get_number(&mut name);
        let mut object;
        let mut victim;
        if {
            object = game
                .db
                .get_obj_in_equip_vis(ch, &name, Some(&mut number), &ch.equipment);
            object.is_some()
        } {
            do_stat_object(&game.db, ch, object.as_ref().unwrap());
        } else if {
            object =
                game.db
                    .get_obj_in_list_vis(ch, &name, Some(&mut number), ch.carrying.borrow());
            object.is_some()
        } {
            do_stat_object(&game.db, ch, object.as_ref().unwrap());
        } else if {
            victim = game
                .db
                .get_char_vis(ch, &mut name, Some(&mut number), FIND_CHAR_ROOM);
            victim.is_some()
        } {
            do_stat_character(&game.db, ch, victim.as_ref().unwrap());
        } else if {
            object = game.db.get_obj_in_list_vis(
                ch,
                &mut name,
                Some(&mut number),
                game.db.world[ch.in_room() as usize]
                    .contents
                    .borrow(),
            );
            object.is_some()
        } {
            do_stat_object(&game.db, ch, object.as_ref().unwrap());
        } else if {
            victim = game
                .db
                .get_char_vis(ch, &mut name, Some(&mut number), FIND_CHAR_WORLD);
            victim.is_some()
        } {
            do_stat_character(&game.db, ch, victim.as_ref().unwrap());
        } else if {
            object = game.db.get_obj_vis(ch, &mut name, Some(&mut number));
            object.is_some()
        } {
            do_stat_object(&game.db, ch, object.as_ref().unwrap());
        } else {
            send_to_char(ch, "Nothing around by that name.\r\n");
        }
    }
}

pub fn do_shutdown(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let mut arg = String::new();
    if subcmd != SCMD_SHUTDOWN {
        send_to_char(ch, "If you want to shut something down, say so!\r\n");
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
        send_to_char(ch, "Unknown shutdown option.\r\n");
    }
}

pub fn snoop_check(ch: &CharData) {
    /*  This short routine is to ensure that characters that happen
     *  to be snooping (or snooped) and get advanced/demoted will
     *  not be snooping/snooped someone of a higher/lower level (and
     *  thus, not entitled to be snooping.
     */
    if ch.desc.borrow().is_none() {
        return;
    }
    let desco = ch.desc.borrow();
    let desc = desco.as_ref().unwrap();
    if desc.snooping.borrow().is_some()
        && desc
            .snooping
            .borrow()
            .as_ref()
            .unwrap()
            .character
            .borrow()
            .as_ref()
            .unwrap()
            .get_level()
            >= ch.get_level()
    {
        *desc
            .snooping
            .borrow()
            .as_ref()
            .unwrap()
            .snoop_by
            .borrow_mut() = None;
        *desc.snooping.borrow_mut() = None;
    }

    if desc.snoop_by.borrow().is_some()
        && ch.get_level()
            >= desc
                .snoop_by
                .borrow()
                .as_ref()
                .unwrap()
                .character
                .borrow()
                .as_ref()
                .unwrap()
                .get_level()
    {
        *desc
            .snoop_by
            .borrow()
            .as_ref()
            .unwrap()
            .snooping
            .borrow_mut() = None;
        *desc.snoop_by.borrow_mut() = None;
    }
}

fn stop_snooping(ch: &Rc<CharData>) {
    if ch
        .desc
        .borrow()
        .as_ref()
        .unwrap()
        .snooping
        .borrow()
        .is_none()
    {
        send_to_char(ch, "You aren't snooping anyone.\r\n");
    } else {
        send_to_char(ch, "You stop snooping.\r\n");
        *ch.desc
            .borrow()
            .as_ref()
            .unwrap()
            .snooping
            .borrow()
            .as_ref()
            .unwrap()
            .snoop_by
            .borrow_mut() = None;
        *ch.desc.borrow().as_ref().unwrap().snooping.borrow_mut() = None;
    }
}

pub fn do_snoop(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut arg = String::new();

    if ch.desc.borrow().is_none() {
        return;
    }

    one_argument(argument, &mut arg);
    let victim;
    let vdesco;
    let voriginal;
    let tch;
    if arg.is_empty() {
        stop_snooping(ch);
    } else if {
        victim = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_WORLD);
        victim.is_none()
    } {
        send_to_char(ch, "No such person around.\r\n");
    } else if victim.as_ref().unwrap().desc.borrow().is_none() {
        send_to_char(ch, "There's no link.. nothing to snoop.\r\n");
    } else if Rc::ptr_eq(victim.as_ref().unwrap(), ch) {
        stop_snooping(ch);
    } else if victim
        .as_ref()
        .unwrap()
        .desc
        .borrow()
        .as_ref()
        .unwrap()
        .snoop_by
        .borrow()
        .is_some()
    {
        send_to_char(ch, "Busy already. \r\n");
    } else if Rc::ptr_eq(
        victim
            .as_ref()
            .unwrap()
            .desc
            .borrow()
            .as_ref()
            .unwrap()
            .snooping
            .borrow()
            .as_ref()
            .unwrap(),
        ch.desc.borrow().as_ref().unwrap(),
    ) {
        send_to_char(ch, "Don't be stupid.\r\n");
    } else {
        if victim
            .as_ref()
            .unwrap()
            .desc
            .borrow()
            .as_ref()
            .unwrap()
            .original
            .borrow()
            .is_some()
        {
            vdesco = victim.as_ref().unwrap().desc.borrow();
            voriginal = vdesco.as_ref().unwrap().original.borrow();
            tch = voriginal.as_ref();
        } else {
            tch = victim.as_ref();
        }
        if tch.as_ref().unwrap().get_level() >= ch.get_level() {
            send_to_char(ch, "You can't.\r\n");
            return;
        }
        send_to_char(ch, OK);

        if ch
            .desc
            .borrow()
            .as_ref()
            .unwrap()
            .snooping
            .borrow()
            .is_some()
        {
            *ch.desc
                .borrow()
                .as_ref()
                .unwrap()
                .snooping
                .borrow()
                .as_ref()
                .unwrap()
                .snoop_by
                .borrow_mut() = None;
        }
        *ch.desc.borrow().as_ref().unwrap().snooping.borrow_mut() =
            victim.as_ref().unwrap().desc.borrow().clone();
    }
}

pub fn do_switch(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let victim;
    if ch
        .desc
        .borrow()
        .as_ref()
        .unwrap()
        .original
        .borrow()
        .is_some()
    {
        send_to_char(ch, "You're already switched.\r\n");
    } else if arg.is_empty() {
        send_to_char(ch, "Switch with who?\r\n");
    } else if {
        victim = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_WORLD);
        victim.is_none()
    } {
        send_to_char(ch, "No such character.\r\n");
    } else if Rc::ptr_eq(ch, victim.as_ref().unwrap()) {
        send_to_char(ch, "Hee hee... we are jolly funny today, eh?\r\n");
    } else if victim.as_ref().unwrap().desc.borrow().is_some() {
        send_to_char(ch, "You can't do that, the body is already in use!\r\n");
    } else if ch.get_level() < LVL_IMPL as u8 && !victim.as_ref().unwrap().is_npc() {
        send_to_char(ch, "You aren't holy enough to use a mortal's body.\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8
        && db.room_flagged(victim.as_ref().unwrap().in_room(), ROOM_GODROOM)
    {
        send_to_char(ch, "You are not godly enough to use that room!\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8
        && db.room_flagged(victim.as_ref().unwrap().in_room(), ROOM_HOUSE)
        && !house_can_enter(db, ch, db.get_room_vnum(victim.as_ref().unwrap().in_room()))
    {
        send_to_char(ch, "That's private property -- no trespassing!\r\n");
    } else {
        send_to_char(ch, OK);
        *ch.desc.borrow().as_ref().unwrap().character.borrow_mut() = victim.clone();
        *ch.desc.borrow().as_ref().unwrap().original.borrow_mut() = Some(ch.clone());

        *victim.as_ref().unwrap().desc.borrow_mut() = ch.desc.borrow().clone();
        *ch.desc.borrow_mut() = None;
    }
}

pub fn do_return(_game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.desc.borrow().is_some()
        && ch
            .desc
            .borrow()
            .as_ref()
            .unwrap()
            .original
            .borrow()
            .is_some()
    {
        send_to_char(ch, "You return to your original body.\r\n");

        /*
         * If someone switched into your original body, disconnect them.
         *   - JE 2/22/95
         *
         * Zmey: here we put someone switched in our body to disconnect state
         * but we must also None his pointer to our character, otherwise
         * close_socket() will damage our character's pointer to our descriptor
         * (which is assigned below in this function). 12/17/99
         */
        if ch
            .desc
            .borrow()
            .as_ref()
            .unwrap()
            .original
            .borrow()
            .as_ref()
            .unwrap()
            .desc
            .borrow()
            .is_some()
        {
            *ch.desc
                .borrow()
                .as_ref()
                .unwrap()
                .original
                .borrow()
                .as_ref()
                .unwrap()
                .desc
                .borrow()
                .as_ref()
                .unwrap()
                .character
                .borrow_mut() = None;
            ch.desc
                .borrow()
                .as_ref()
                .unwrap()
                .original
                .borrow()
                .as_ref()
                .unwrap()
                .desc
                .borrow()
                .as_ref()
                .unwrap()
                .set_state(ConDisconnect);
        }

        /* Now our descriptor points to our original body. */
        *ch.desc.borrow().as_ref().unwrap().character.borrow_mut() =
            ch.desc.borrow().as_ref().unwrap().original.borrow().clone();
        *ch.desc.borrow().as_ref().unwrap().original.borrow_mut() = None;

        /* And our body's pointer to descriptor now points to our descriptor. */
        *ch.desc
            .borrow()
            .as_ref()
            .unwrap()
            .character
            .borrow()
            .as_ref()
            .unwrap()
            .desc
            .borrow_mut() = ch.desc.borrow().clone();
        *ch.desc.borrow_mut() = None;
    }
}

pub fn do_load(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !buf2.chars().next().unwrap().is_digit(10) {
        send_to_char(ch, "Usage: load { obj | mob } <number>\r\n");
        return;
    }
    if !is_number(&buf2) {
        send_to_char(ch, "That is not a number.\r\n");
        return;
    }

    if is_abbrev(&buf, "mob") {
        let r_num;

        if {
            r_num = game.db.real_mobile(buf2.parse::<i16>().unwrap());
            r_num == NOBODY
        } {
            send_to_char(ch, "There is no monster with that number.\r\n");
            return;
        }
        let mob = game.db.read_mobile(r_num, REAL);
        game.db.char_to_room(mob.as_ref().unwrap(), ch.in_room());

        game.db.act(
            "$n makes a quaint, magical gesture with one hand.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        game.db.act(
            "$n has created $N!",
            false,
            Some(ch),
            None,
            Some(mob.as_ref().unwrap()),
            TO_ROOM,
        );
        game.db.act(
            "You create $N.",
            false,
            Some(ch),
            None,
            Some(mob.as_ref().unwrap()),
            TO_CHAR,
        );
    } else if is_abbrev(&buf, "obj") {
        let r_num;

        if {
            r_num = game.db.real_object(buf2.parse::<i16>().unwrap());
            r_num == NOTHING
        } {
            send_to_char(ch, "There is no object with that number.\r\n");
            return;
        }
        let obj = game.db.read_object(r_num, REAL).unwrap();
        if LOAD_INTO_INVENTORY {
            DB::obj_to_char(&obj, ch);
        } else {
            game.db.obj_to_room(&obj, ch.in_room());
        }
        game.db.act(
            "$n makes a strange magical gesture.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        game.db.act(
            "$n has created $p!",
            false,
            Some(ch),
            Some(obj.as_ref()),
            None,
            TO_ROOM,
        );
        game.db.act(
            "You create $p.",
            false,
            Some(ch),
            Some(obj.as_ref()),
            None,
            TO_CHAR,
        );
    } else {
        send_to_char(ch, "That'll have to be either 'obj' or 'mob'.\r\n");
    }
}

pub fn do_vstat(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !buf2.chars().next().unwrap().is_digit(10) {
        send_to_char(ch, "Usage: vstat { obj | mob } <number>\r\n");
        return;
    }
    if !is_number(&buf2) {
        send_to_char(ch, "That's not a valid number.\r\n");
        return;
    }

    if is_abbrev(&buf, "mob") {
        let r_num;

        if {
            r_num = game.db.real_mobile(buf2.parse::<i16>().unwrap());
            r_num == NOBODY
        } {
            send_to_char(ch, "There is no monster with that number.\r\n");
            return;
        }
        let mob = game.db.read_mobile(r_num, REAL);
        game.db.char_to_room(mob.as_ref().unwrap(), 0);
        do_stat_character(&game.db, ch, mob.as_ref().unwrap());
        game.db.extract_char(mob.as_ref().unwrap());
    } else if is_abbrev(&buf, "obj") {
        let r_num;

        if {
            r_num = game.db.real_object(buf2.parse::<i16>().unwrap());
            r_num == NOTHING
        } {
            send_to_char(ch, "There is no object with that number.\r\n");
            return;
        }
        let obj = game.db.read_object(r_num, REAL);
        do_stat_object(&game.db, ch, obj.as_ref().unwrap());
        game.db.extract_obj(obj.as_ref().unwrap());
    } else {
        send_to_char(ch, "That'll have to be either 'obj' or 'mob'.\r\n");
    }
}

pub fn do_purge(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    /* clean a room of all mobiles and objects */
    let mut buf = String::new();
    one_argument(argument, &mut buf);
    let vict;
    let obj;
    /* argument supplied. destroy single object or char */
    if !buf.is_empty() {
        if {
            vict = game.db.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
            vict.is_some()
        } {
            if !vict.as_ref().unwrap().is_npc()
                && ch.get_level() <= vict.as_ref().unwrap().get_level()
            {
                send_to_char(ch, "Fuuuuuuuuu!\r\n");
                return;
            }
            game.db.act(
                "$n disintegrates $N.",
                false,
                Some(ch),
                None,
                Some(vict.as_ref().unwrap()),
                TO_NOTVICT,
            );
            let vict = vict.as_ref().unwrap();
            if !vict.is_npc() {
                game.mudlog(
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} has purged {}.", ch.get_name(), vict.get_name()).as_str(),
                );
                if vict.desc.borrow().is_some() {
                    vict.desc.borrow().as_ref().unwrap().set_state(ConClose);
                    *vict.desc.borrow().as_ref().unwrap().character.borrow_mut() = None;
                    *vict.desc.borrow_mut() = None;
                }
            }
            game.db.extract_char(vict);
        } else if {
            obj = game.db.get_obj_in_list_vis(
                ch,
                &mut buf,
                None,
                game.db.world[ch.in_room() as usize].contents.borrow(),
            );
            obj.is_some()
        } {
            let obj = obj.unwrap();
            game.db.act(
                "$n destroys $p.",
                false,
                Some(ch),
                Some(obj.as_ref()),
                None,
                TO_ROOM,
            );
            game.db.extract_obj(&obj);
        } else {
            send_to_char(ch, "Nothing here by that name.\r\n");
            return;
        }

        send_to_char(ch, OK);
    } else {
        /* no argument. clean out the room */

        game.db.act(
            "$n gestures... You are surrounded by scorching flames!",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        game.db.send_to_room(ch.in_room(), "The world seems a little cleaner.\r\n");

        let room = game.db.world[ch.in_room() as usize].clone();
        for vict in 
            room.peoples
            .borrow()
            .iter()
        {
            if !vict.is_npc() {
                continue;
            }

            /* Dump inventory. */
            while vict.carrying.borrow().len() > 0 {
                game.db.extract_obj(&vict.carrying.borrow()[0].clone());
            }

            /* Dump equipment. */
            for i in 0..NUM_WEARS {
                if vict.get_eq(i).is_some() {
                    game.db.extract_obj(vict.get_eq(i).as_ref().unwrap())
                }
            }

            /* Dump character. */
            game.db.extract_char(vict);
        }

        /* Clear the ground. */
        while game.db.world[ch.in_room() as usize]
            .contents
            .borrow()
            .len()
            > 0
        {
            let o = game.db.world[ch.in_room() as usize].contents.borrow()[0].clone();
            game.db.extract_obj(&o);
        }
    }
}

const LOGTYPES: [&str; 5] = ["off", "brief", "normal", "complete", "\n"];

pub fn do_syslog(_game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        send_to_char(
            ch,
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
        send_to_char(ch, "Usage: syslog { Off | Brief | Normal | Complete }\r\n");
        return;
    }
    let tp = tp.unwrap();
    ch.remove_prf_flags_bits(PRF_LOG1 | PRF_LOG2);
    ch.set_prf_flags_bits(PRF_LOG1 * (tp & 1) as i64 | PRF_LOG2 * (tp & 2) as i64 >> 1);

    send_to_char(
        ch,
        format!("Your syslog is now {}.\r\n", &LOGTYPES[tp]).as_str(),
    );
}

pub fn do_advance(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut name = String::new();
    let mut level = String::new();
    let victim;
    let db = &game.db;
    two_arguments(argument, &mut name, &mut level);

    if name.len() != 0 {
        if {
            victim = db.get_char_vis(ch, &mut name, None, FIND_CHAR_WORLD);
            victim.is_none()
        } {
            send_to_char(ch, "That player is not here.\r\n");
            return;
        }
    } else {
        send_to_char(ch, "Advance who?\r\n");
        return;
    }
    let victim = victim.unwrap();

    if ch.get_level() <= victim.get_level() {
        send_to_char(ch, "Maybe that's not such a great idea.\r\n");
        return;
    }
    if victim.is_npc() {
        send_to_char(ch, "NO!  Not on NPC's.\r\n");
        return;
    }
    let r = level.parse::<u8>();
    let mut newlevel = 255;
    if r.is_err() || {
        newlevel = r.unwrap();
        newlevel == 0
    } {
        send_to_char(ch, "That's not a level!\r\n");
        return;
    }

    if newlevel > LVL_IMPL as u8 {
        send_to_char(
            ch,
            format!("{} is the highest possible level.\r\n", LVL_IMPL).as_str(),
        );
        return;
    }
    if newlevel > ch.get_level() {
        send_to_char(ch, "Yeah, right.\r\n");
        return;
    }
    if newlevel == victim.get_level() {
        send_to_char(ch, "They are already at that level.\r\n");
        return;
    }
    let oldlevel = victim.get_level();
    if newlevel < oldlevel {
        do_start(game, &victim);
        victim.set_level(newlevel);

        send_to_char(
            &victim,
            "You are momentarily enveloped by darkness!\r\nYou feel somewhat diminished.\r\n",
        );
    } else {
        db.act(
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
            Some(ch),
            None,
            Some(&victim),
            TO_VICT,
        );
    }

    send_to_char(ch, OK);

    if newlevel < oldlevel {
        info!(
            "(GC) {} demoted {} from level {} to {}.",
            ch.get_name(),
            victim.get_name(),
            oldlevel,
            newlevel
        );
    } else {
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
        victim.remove_prf_flags_bits(PRF_LOG1 | PRF_LOG2);
        victim.remove_prf_flags_bits(PRF_NOHASSLE | PRF_HOLYLIGHT);

        // TODO run_autowiz();
    }

    gain_exp_regardless(
        game,
        &victim,
        level_exp(victim.get_class(), newlevel as i16) - victim.get_exp(),
    );
    db.save_char(&victim);
}

pub fn do_restore(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let vict;
    if buf.is_empty() {
        send_to_char(ch, "Whom do you wish to restore?\r\n");
    } else if {
        vict = db.get_char_vis(ch, &mut buf, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(ch, NOPERSON);
    } else if !vict.as_ref().unwrap().is_npc()
        && !Rc::ptr_eq(ch, vict.as_ref().unwrap())
        && vict.as_ref().unwrap().get_level() >= ch.get_level()
    {
        send_to_char(ch, "They don't need your help.\r\n");
    } else {
        let vict = vict.as_ref().unwrap();
        vict.set_hit(vict.get_max_hit());
        vict.set_mana(vict.get_max_mana());
        vict.set_move(vict.get_move());

        if !vict.is_npc() && ch.get_level() >= LVL_GRGOD as u8 {
            if vict.get_level() >= LVL_IMMORT as u8 {
                for i in 1..MAX_SKILLS + 1 {
                    vict.set_skill(i as i32, 100);
                }
            }

            if vict.get_level() >= LVL_GRGOD as u8 {
                vict.real_abils.borrow_mut().str_add = 100;
                vict.real_abils.borrow_mut().intel = 25;
                vict.real_abils.borrow_mut().wis = 25;
                vict.real_abils.borrow_mut().dex = 25;
                vict.real_abils.borrow_mut().str = 25;
                vict.real_abils.borrow_mut().con = 25;
                vict.real_abils.borrow_mut().cha = 25;
            }
        }
        update_pos(vict);
        affect_total(vict);
        send_to_char(ch, OK);
        db.act(
            "You have been fully healed by $N!",
            false,
            Some(vict),
            None,
            Some(ch),
            TO_CHAR,
        );
    }
}

pub fn perform_immort_vis(db: &DB, ch: &Rc<CharData>) {
    if ch.get_invis_lev() == 0 && !ch.aff_flagged(AFF_HIDE | AFF_INVISIBLE) {
        send_to_char(ch, "You are already fully visible.\r\n");
        return;
    }

    ch.set_invis_lev(0);

    db.appear(ch);
    send_to_char(ch, "You are now fully visible.\r\n");
}

fn perform_immort_invis(db: &DB, ch: &Rc<CharData>, level: i32) {
    for tch in db.world[ch.in_room() as usize]
        .peoples
        .borrow()
        .iter()
    {
        if Rc::ptr_eq(tch, ch) {
            continue;
        }
        if tch.get_level() >= ch.get_invis_lev() as u8 && tch.get_level() < level as u8 {
            db.act(
                "You blink and suddenly realize that $n is gone.",
                false,
                Some(ch),
                None,
                Some(tch),
                TO_VICT,
            );
        }
        if tch.get_level() < ch.get_invis_lev() as u8 && tch.get_level() >= level as u8 {
            db.act(
                "You suddenly realize that $n is standing beside you.",
                false,
                Some(ch),
                None,
                Some(tch),
                TO_VICT,
            );
        }
    }

    ch.set_invis_lev(level as i16);
    send_to_char(
        ch,
        format!("Your invisibility level is {}.\r\n", level).as_str(),
    );
}

pub fn do_invis(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut arg = String::new();

    if ch.is_npc() {
        send_to_char(ch, "You can't do that!\r\n");
        return;
    }

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        if ch.get_invis_lev() > 0 {
            perform_immort_vis(db, ch);
        } else {
            perform_immort_invis(db, ch, ch.get_level() as i32);
        }
    } else {
        let level = arg.parse::<i32>();
        let level = if level.is_err() { 0 } else { level.unwrap() };
        if level > ch.get_level() as i32 {
            send_to_char(ch, "You can't go invisible above your own level.\r\n");
        } else if level < 1 {
            perform_immort_vis(db, ch);
        } else {
            perform_immort_invis(db, ch, level);
        }
    }
}

pub fn do_gecho(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if argument.is_empty() {
        send_to_char(ch, "That must be a mistake...\r\n");
    } else {
        for pt in game.descriptor_list.iter() {
            if pt.state() == ConPlaying
                && pt.character.borrow().is_some()
                && !Rc::ptr_eq(pt.character.borrow().as_ref().unwrap(), ch)
            {
                send_to_char(
                    pt.character.borrow().as_ref().unwrap(),
                    format!("{}\r\n", argument).as_str(),
                );
            }
        }
        if ch.prf_flagged(PRF_NOREPEAT) {
            send_to_char(ch, OK);
        } else {
            send_to_char(ch, format!("{}\r\n", argument).as_str());
        }
    }
}

pub fn do_poofset(_game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let msg;
    let mut cps = ch.player_specials.borrow_mut();
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

    send_to_char(ch, OK);
}

pub fn do_dc(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let num_to_dc = arg.parse::<u32>();
    if num_to_dc.is_err() {
        send_to_char(ch, "Usage: DC <user number> (type USERS for a list)\r\n");
        return;
    }
    let num_to_dc = num_to_dc.unwrap();
    let mut d = None;
    {
        for cd in game.descriptor_list.iter() {
            if cd.desc_num.get() == num_to_dc as usize {
                d = Some(cd.clone());
            }
        }
    }

    if d.is_none() {
        send_to_char(ch, "No such connection.\r\n");
        return;
    }
    let d = d.as_ref().unwrap();
    if d.character.borrow().is_some()
        && d.character.borrow().as_ref().unwrap().get_level() >= ch.get_level()
    {
        if !db.can_see(ch, d.character.borrow().as_ref().unwrap()) {
            send_to_char(ch, "No such connection.\r\n");
        } else {
            send_to_char(ch, "Umm.. maybe that's not such a good idea...\r\n");
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
    if d.state() == ConDisconnect || d.state() == ConClose {
        send_to_char(ch, "They're already being disconnected.\r\n");
    } else {
        /*
         * Remember that we can disconnect people not in the game and
         * that rather confuses the code when it expected there to be
         * a character context.
         */
        if d.state() == ConPlaying {
            d.set_state(ConDisconnect);
        } else {
            d.set_state(ConClose);
        }
        send_to_char(
            ch,
            format!("Connection #{} closed.\r\n", num_to_dc).as_str(),
        );
        info!("(GC) Connection closed by {}.", ch.get_name());
    }
}

pub fn do_wizlock(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    let value;
    one_argument(argument, &mut arg);
    let when;
    if !arg.is_empty() {
        value = arg.parse::<i32>();
        let value = if value.is_err() { -1 } else { value.unwrap() };
        if value < 0 || value > ch.get_level() as i32 {
            send_to_char(ch, "Invalid wizlock value.\r\n");
            return;
        }
        game.db.circle_restrict.set(value as u8);
        when = "now";
    } else {
        when = "currently";
    }

    match game.db.circle_restrict.get() {
        0 => {
            send_to_char(
                ch,
                format!("The game is {} completely open.\r\n", when).as_str(),
            );
        }
        1 => {
            send_to_char(
                ch,
                format!("The game is {} closed to new players.\r\n", when).as_str(),
            );
        }
        _ => {
            send_to_char(
                ch,
                format!(
                    "Only level {} and above may enter the game {}.\r\n",
                    game.db.circle_restrict.get(),
                    when
                )
                .as_str(),
            );
        }
    }
}

pub fn do_date(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, subcmd: i32) {
    let mytime;
    if subcmd == SCMD_DATE {
        mytime = time_now();
    } else {
        mytime = game.db.boot_time.get() as u64;
    }

    let date_time = Utc.timestamp_millis_opt(mytime as i64 * 1000).unwrap();
    let tmstr = date_time.to_rfc2822();

    if subcmd == SCMD_DATE {
        send_to_char(ch, format!("Current machine time: {}\r\n", tmstr).as_str());
    } else {
        let mytime = time_now() - game.db.boot_time.get() as u64;
        let d = mytime / 86400;
        let h = (mytime / 3600) % 24;
        let m = (mytime / 60) % 60;

        send_to_char(
            ch,
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

pub fn do_last(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        send_to_char(ch, "For whom do you wish to search?\r\n");
        return;
    }
    let mut chdata = CharFileU::new();
    if db.load_char(&arg, &mut chdata).is_none() {
        send_to_char(ch, "There is no such player.\r\n");
        return;
    }
    if chdata.level > ch.get_level() && ch.get_level() < LVL_IMPL as u8 {
        send_to_char(ch, "You are not sufficiently godly for that!\r\n");
        return;
    }
    let id = chdata.char_specials_saved.idnum;
    send_to_char(
        ch,
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

pub fn do_force(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut argument = argument.to_string();
    let mut arg = String::new();
    let mut to_force = String::new();

    half_chop(&mut argument, &mut arg, &mut to_force);

    let buf1 = format!("$n has forced you to '{}'.", to_force);
    let vict;
    if arg.is_empty() || to_force.is_empty() {
        send_to_char(ch, "Whom do you wish to force do what?\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8 || "all" != arg && "room" != arg {
        if {
            vict = game.db.get_char_vis(ch, &mut arg, None, FIND_CHAR_WORLD);
            vict.is_none()
        } {
            send_to_char(ch, NOPERSON);
        } else if !vict.as_ref().unwrap().is_npc()
            && ch.get_level() <= vict.as_ref().unwrap().get_level()
        {
            send_to_char(ch, "No, no, no!\r\n");
        } else {
            let vict = vict.as_ref().unwrap();
            send_to_char(ch, OK);
            game.db
                .act(&buf1, true, Some(ch), None, Some(vict.as_ref()), TO_VICT);
            game.mudlog(
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
            command_interpreter(game, vict, &to_force);
        }
    } else if arg == "room" {
        send_to_char(ch, OK);
        game.mudlog(
            NRM,
            max(LVL_GOD as i32, ch.get_invis_lev() as i32),
            true,
            format!(
                "(GC) {} forced room {} to {}",
                ch.get_name(),
                game.db.get_room_vnum(ch.in_room()),
                to_force
            )
            .as_str(),
        );
        let peoples_in_room = clone_vec(&game.db.world[ch.in_room() as usize].peoples);
        for vict in peoples_in_room.iter() {
            if !vict.is_npc() && vict.get_level() >= ch.get_level() {
                continue;
            }
            game.db
                .act(&buf1, true, Some(ch), None, Some(vict), TO_VICT);
            command_interpreter(game, vict, &to_force);
        }
    } else {
        /* force all */
        send_to_char(ch, OK);
        game.mudlog(
            NRM,
            max(LVL_GOD as i32, ch.get_invis_lev() as i32),
            true,
            format!("(GC) {} forced all to {}", ch.get_name(), to_force).as_str(),
        );
        let descriptors = clone_vec2(&game.descriptor_list);
        for i in descriptors.iter() {
            let mut vict = None;
            let ic = i.character.borrow();
            if i.state() != ConPlaying
                || {
                    vict = ic.as_ref().clone();
                    vict.is_none()
                }
                || !vict.as_ref().unwrap().is_npc()
                    && vict.as_ref().unwrap().get_level() >= ch.get_level()
            {
                continue;
            }
            game.db.act(
                &buf1,
                true,
                Some(ch),
                None,
                Some(&vict.unwrap().clone()),
                TO_VICT,
            );
            command_interpreter(game, vict.as_ref().unwrap(), &to_force);
        }
    }
}

pub fn do_wiznet(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut emote = false;
    let mut level = LVL_IMMORT;
    let mut buf1 = String::new();
    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if argument.is_empty() {
        send_to_char(ch, "Usage: wiznet <text> | #<level> <text> | *<emotetext> |\r\n        wiznet @<level> *<emotetext> | wiz @\r\n");
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
                    send_to_char(ch, "You can't wizline above your own level.\r\n");
                    return;
                }
            } else if emote {
            }
        }

        '@' => {
            send_to_char(ch, "God channel status:\r\n");
            for d in game.descriptor_list.iter() {
                if d.state() != ConPlaying
                    || d.character.borrow().as_ref().unwrap().get_level() < LVL_IMMORT as u8
                {
                    continue;
                }
                if !db.can_see(ch, d.character.borrow().as_ref().unwrap()) {
                    continue;
                }
                let dco = d.character.borrow();
                let dc = dco.as_ref().unwrap();
                send_to_char(
                    ch,
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
        send_to_char(ch, "You are offline!\r\n");
        return;
    }
    let argument = argument.trim_start();

    if argument.is_empty() {
        send_to_char(ch, "Don't bother the gods like that!\r\n");
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
    for d in game.descriptor_list.iter() {
        let db = &game.db;
        if d.state() == ConPlaying
            && d.character.borrow().as_ref().unwrap().get_level() >= level as u8
            && !d
                .character
                .borrow()
                .as_ref()
                .unwrap()
                .prf_flagged(PRF_NOWIZ)
            && !d
                .character
                .borrow()
                .as_ref()
                .unwrap()
                .plr_flagged(PLR_WRITING | PLR_MAILING)
            && !Rc::ptr_eq(d, ch.desc.borrow().as_ref().unwrap())
            || !d
                .character
                .borrow()
                .as_ref()
                .unwrap()
                .prf_flagged(PRF_NOREPEAT)
        {
            send_to_char(
                d.character.borrow().as_ref().unwrap(),
                CCCYN!(d.character.borrow().as_ref().unwrap(), C_NRM),
            );
            let dco = d.character.borrow();
            let dc = dco.as_ref().unwrap();
            if db.can_see(dc, ch) {
                send_to_char(dc, &buf1);
            } else {
                send_to_char(dc, &buf2);
            }
            send_to_char(dc, CCNRM!(dc, C_NRM));
        }
    }

    if ch.prf_flagged(PRF_NOREPEAT) {
        send_to_char(ch, OK);
    }
}

pub fn do_zreset(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        send_to_char(ch, "You must specify a zone.\r\n");
        return;
    }
    let zone_count = game.db.zone_table.borrow().len();
    let mut i = zone_count;
    if arg.starts_with('*') {
        for i in 0..zone_count {
            game.reset_zone( i);
        }
        send_to_char(ch, "Reset world.\r\n");
        game.mudlog(
            NRM,
            max(LVL_GRGOD as i32, ch.get_invis_lev() as i32),
            true,
            format!("(GC) {} reset entire world.", ch.get_name()).as_str(),
        );
        return;
    } else if arg.starts_with('.') {
        i = game.db.world[ch.in_room() as usize].zone as usize;
    } else {
        let j = arg.parse::<i32>();
        if j.is_err() {
            return;
        };
        let j = j.unwrap();
        for ii in 0..game.db.zone_table.borrow().len() {
            if game.db.zone_table.borrow()[ii].number == j as i16 {
                i = ii;
                break;
            }
        }
    }
    if i < game.db.zone_table.borrow().len() {
        game.reset_zone( i as usize);
        send_to_char(
            ch,
            format!(
                "Reset zone {} (#{}): {}.\r\n",
                i,
                game.db.zone_table.borrow()[i].number,
                game.db.zone_table.borrow()[i].name
            )
            .as_str(),
        );
        game.mudlog(
            NRM,
            max(LVL_GRGOD as i32, ch.get_invis_lev() as i32),
            true,
            format!(
                "(GC) {} reset zone {} ({})",
                ch.get_name(),
                i,
                game.db.zone_table.borrow()[i].name
            )
            .as_str(),
        );
    } else {
        send_to_char(ch, "Invalid zone number.\r\n");
    }
}

/*
 *  General fn for wizcommands of the sort: cmd <player>
 */
pub fn do_wizutil(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let db = &game.db;
    let mut arg = String::new();
    one_argument(argument, &mut arg);
    let vict;
    if arg.is_empty() {
        send_to_char(ch, "Yes, but for whom?!?\r\n");
    } else if {
        vict = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(ch, "There is no such player.\r\n");
    } else if vict.as_ref().unwrap().is_npc() {
        send_to_char(ch, "You can't do that to a mob!\r\n");
    } else if vict.as_ref().unwrap().get_level() > ch.get_level() {
        send_to_char(ch, "Hmmm...you'd better not.\r\n");
    } else {
        let vict = vict.as_ref().unwrap();
        match subcmd {
            SCMD_REROLL => {
                send_to_char(ch, "Rerolled...\r\n");
                roll_real_abils(vict);
                info!("(GC) {} has rerolled {}.", ch.get_name(), vict.get_name());
                send_to_char(
                    ch,
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
                    send_to_char(ch, "Your victim is not flagged.\r\n");
                    return;
                }
                vict.remove_plr_flag(PLR_THIEF | PLR_KILLER);
                send_to_char(ch, "Pardoned.\r\n");
                send_to_char(vict, "You have been pardoned by the Gods!\r\n");
                game.mudlog(
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} pardoned by {}", vict.get_name(), ch.get_name()).as_str(),
                );
            }
            SCMD_NOTITLE => {
                let result = vict.plr_tog_chk(PLR_NOTITLE);
                game.mudlog(
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
                send_to_char(
                    ch,
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
                let result = vict.plr_tog_chk(PLR_NOSHOUT);
                game.mudlog(
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
                send_to_char(
                    ch,
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
                if Rc::ptr_eq(ch, vict) {
                    send_to_char(ch, "Oh, yeah, THAT'S real smart...\r\n");
                    return;
                }
                if vict.plr_flagged(PLR_FROZEN) {
                    send_to_char(ch, "Your victim is already pretty cold.\r\n");
                    return;
                }
                vict.set_plr_flag_bit(PLR_FROZEN);
                vict.set_freeze_lev(ch.get_level() as i8);
                send_to_char(vict, "A bitter wind suddenly rises and drains every erg of heat from your body!\r\nYou feel frozen!\r\n");
                send_to_char(ch, "Frozen.\r\n");
                db.act(
                    "A sudden cold wind conjured from nowhere freezes $n!",
                    false,
                    Some(vict),
                    None,
                    None,
                    TO_ROOM,
                );
                game.mudlog(
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} frozen by {}.", vict.get_name(), ch.get_name()).as_str(),
                );
            }
            SCMD_THAW => {
                if !vict.plr_flagged(PLR_FROZEN) {
                    send_to_char(
                        ch,
                        "Sorry, your victim is not morbidly encased in ice at the moment.\r\n",
                    );
                    return;
                }
                if vict.get_freeze_lev() > ch.get_level() as i8 {
                    send_to_char(
                        ch,
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
                game.mudlog(
                    BRF,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} un-frozen by {}.", vict.get_name(), ch.get_name()).as_str(),
                );
                vict.remove_plr_flag(PLR_FROZEN);
                send_to_char(vict, "A fireball suddenly explodes in front of you, melting the ice!\r\nYou feel thawed.\r\n");
                send_to_char(ch, "Thawed.\r\n");
                db.act(
                    "A sudden fireball conjured from nowhere thaws $n!",
                    false,
                    Some(vict),
                    None,
                    None,
                    TO_ROOM,
                );
            }
            SCMD_UNAFFECT => {
                if vict.affected.borrow().len() != 0 {
                    while vict.affected.borrow().len() != 0 {
                        affect_remove(vict, &vict.affected.borrow()[0]);
                    }
                    send_to_char(
                        vict,
                        "There is a brief flash of light!\r\nYou feel slightly different.\r\n",
                    );
                    send_to_char(ch, "All spells removed.\r\n");
                } else {
                    send_to_char(ch, "Your victim does not have any affections!\r\n");
                    return;
                }
            }
            _ => {
                error!("SYSERR: Unknown subcmd {} passed to do_wizutil ", subcmd);
            }
        }
        db.save_char(vict);
    }
}

/* single zone printing fn used by "show zone" so it's not repeated in the
code 3 times ... -je, 4/6/93 */

fn print_zone_to_buf(db: &DB, buf: &mut String, zone: ZoneRnum) {
    let zt = db.zone_table.borrow();
    let zone = &zt[zone as usize];
    buf.push_str(
        format!(
            "{:3} {:30} Age: {:3}; Reset: {:3} ({:1}); Range: {:5}-{:5}\r\n",
            zone.number,
            zone.name,
            zone.age.get(),
            zone.lifespan,
            zone.reset_mode,
            zone.bot,
            zone.top
        )
        .as_str(),
    );
}

pub fn do_show(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
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
        send_to_char(ch, "Show options:\r\n");
        let mut j = 0;
        for i in 1..FIELDS.len() - 1 {
            if FIELDS[i].level <= ch.get_level() as i16 {
                send_to_char(
                    ch,
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
        send_to_char(ch, "\r\n");
        return;
    }

    let mut field = String::new();
    let mut value = String::new();
    two_arguments(argument, &mut field, &mut value);

    let l = FIELDS.iter().position(|f| f.cmd == field);
    let l = if l.is_some() { l.unwrap() } else { 0 };

    if ch.get_level() < FIELDS[l].level as u8 {
        send_to_char(ch, "You are not godly enough for that!\r\n");
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
                print_zone_to_buf(&game.db, &mut buf, game.db.world[ch.in_room() as usize].zone);
            } else if !value.is_empty() && is_number(&value) {
                let value = value.parse::<i32>().unwrap();
                let zrn = game.db
                    .zone_table
                    .borrow()
                    .iter()
                    .position(|z| z.number == value as i16);
                if zrn.is_some() {
                    print_zone_to_buf(&game.db, &mut buf, zrn.unwrap() as ZoneRnum);
                } else {
                    send_to_char(ch, "That is not a valid zone.\r\n");
                    return;
                }
            } else {
                for i in 0..game.db.zone_table.borrow().len() {
                    print_zone_to_buf(&game.db, &mut buf, i as ZoneRnum);
                }
            }
            page_string(ch.desc.borrow().as_ref().unwrap(), &buf, true);
        }

        /* show player */
        2 => {
            if value.is_empty() {
                send_to_char(ch, "A name would help.\r\n");
                return;
            }

            let mut vbuf = CharFileU::new();
            if game.db.load_char(&value, &mut vbuf).is_none() {
                send_to_char(ch, "There is no such player.\r\n");
                return;
            }

            send_to_char(
                ch,
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
            send_to_char(
                ch,
                format!(
                    "Au:{:8}  Bal:{:8}  Exp:{:8}  Align: {:5}  Lessons: {:3}\r\n",
                    g, bg, exp, ali, stl
                )
                .as_str(),
            );
            /* ctime() uses static buffer: do not combine. */
            send_to_char(ch, format!("Started: {}  ", ctime(vbuf.birth)).as_str());
            send_to_char(
                ch,
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
                send_to_char(ch, "A name would help.\r\n");
                return;
            }
            crash_listrent(&mut game.db, ch, &value);
        }

        /* show stats */
        4 => {
            let mut i = 0;
            let mut j = 0;
            let mut k = 0;
            let mut con = 0;
            for vict in game.db.character_list.iter() {
                if vict.is_npc() {
                    j += 1;
                } else if game.db.can_see(ch, vict) {
                    i += 1;
                    if vict.desc.borrow().is_some() {
                        con += 1;
                    }
                }
            }
            for _ in game.db.object_list.iter() {
                k += 1;
            }
            send_to_char(
                ch,
                format!(
                    "Current stats:\r\n\
                               {:5} players in game  {:5} connected\r\n\
                               {:5} registered\r\n\
                               {:5} mobiles          {:5} prototypes\r\n\
                               {:5} objects          {:5} prototypes\r\n\
                               {:5} rooms            {:5} zones\r\n",
                    i,
                    con,
                    game.db.player_table.borrow().len(),
                    j,
                    game.db.mob_protos.len(),
                    k,
                    game.db.obj_proto.len(),
                    game.db.world.len(),
                    game.db.zone_table.borrow().len()
                )
                .as_str(),
            );
        }

        /* show errors */
        5 => {
            let mut buf = "Errant Rooms\r\n------------\r\n".to_string();
            let mut k = 0;
            for i in 0..game.db.world.len() {
                for j in 0..NUM_OF_DIRS {
                    if game.db.world[i].dir_option[j].is_some()
                        && game.db.world[i].dir_option[j]
                            .as_ref()
                            .unwrap()
                            .to_room
                            .get()
                            == 0
                    {
                        k += 1;

                        buf.push_str(
                            format!(
                                "{:2}: [{:5}] {}\r\n",
                                k,
                                game.db.get_room_vnum(i as RoomVnum),
                                game.db.world[i].name
                            )
                            .as_str(),
                        )
                    }
                }
            }
            page_string(ch.desc.borrow().as_ref().unwrap(), &buf, true);
        }

        /* show death */
        6 => {
            let mut buf = "Death Traps\r\n-----------\r\n".to_string();
            let mut j = 0;
            for i in 0..game.db.world.len() {
                if game.db.room_flagged(i as RoomRnum, ROOM_DEATH) {
                    j += 1;
                    buf.push_str(
                        format!(
                            "{:2}: [{:5}] {}\r\n",
                            j,
                            game.db.get_room_vnum(i as RoomVnum),
                            game.db.world[i].name
                        )
                        .as_str(),
                    );
                }
            }
            page_string(ch.desc.borrow().as_ref().unwrap(), &buf, true);
        }

        /* show godrooms */
        7 => {
            let mut buf = "Godrooms\r\n--------------------------\r\n".to_string();
            let mut j = 0;
            for i in 0..game.db.world.len() {
                if game.db.room_flagged(i as RoomRnum, ROOM_GODROOM) {
                    j += 1;
                    buf.push_str(
                        format!(
                            "{:2}: [{:5}] {}\r\n",
                            j,
                            game.db.get_room_vnum(i as RoomVnum),
                            game.db.world[i].name
                        )
                        .as_str(),
                    );
                }
            }
            page_string(ch.desc.borrow().as_ref().unwrap(), &buf, true);
        }

        /* show shops */
        8 => {
            show_shops(&game.db, ch, &value);
        }

        /* show houses */
        9 => {
            hcontrol_list_houses(&game.db, ch);
        }

        /* show snoop */
        10 => {
            let mut i = 0;
            send_to_char(
                ch,
                "People currently snooping:\r\n--------------------------\r\n",
            );
            for d in game.descriptor_list.iter() {
                if d.snooping.borrow().is_none() || d.character.borrow().is_none() {
                    continue;
                }
                let dco = d.character.borrow();
                let dc = dco.as_ref().unwrap();
                if d.state() != ConPlaying || ch.get_level() < dc.get_level() {
                    continue;
                }
                if !game.db.can_see(ch, dc) || dc.in_room() == NOWHERE {
                    continue;
                }
                i += 1;
                send_to_char(
                    ch,
                    format!(
                        "{:10} - snooped by {}.\r\n",
                        d.snooping
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .character
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .get_name(),
                        dc.get_name()
                    )
                    .as_str(),
                );
            }
            if i == 0 {
                send_to_char(ch, "No one is currently snooping.\r\n");
            }
        }

        /* show what? */
        _ => {
            send_to_char(ch, "Sorry, I don't understand that.\r\n");
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

fn perform_set(db: &mut DB, ch: &Rc<CharData>, vict: &Rc<CharData>, mode: i32, val_arg: &str) -> bool {
    let mut on = false;
    let mut off = false;
    let mut value = 0;
    let mode = mode as usize;

    /* Check to make sure all the levels are correct */
    if ch.get_level() != LVL_IMPL as u8 {
        if !vict.is_npc() && ch.get_level() <= vict.get_level() && !Rc::ptr_eq(vict, ch) {
            send_to_char(ch, "Maybe that's not such a great idea...\r\n");
            return false;
        }
    }
    if ch.get_level() < SET_FIELDS[mode].level as u8 {
        send_to_char(ch, "You are not godly enough for that!\r\n");
        return false;
    }

    /* Make sure the PC/NPC is correct */
    if vict.is_npc() && SET_FIELDS[mode].pcnpc & NPC == 0 {
        send_to_char(ch, "You can't do that to a beast!\r\n");
        return false;
    } else if !vict.is_npc() && SET_FIELDS[mode].pcnpc & PC == 0 {
        send_to_char(ch, "That can only be done to a beast!\r\n");
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
            send_to_char(ch, "Value must be 'on' or 'off'.\r\n");
            return false;
        }
        send_to_char(
            ch,
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
        send_to_char(
            ch,
            format!(
                "{}'s {} set to {}.\r\n",
                vict.get_name(),
                SET_FIELDS[mode].cmd,
                value
            )
            .as_str(),
        );
    } else {
        send_to_char(ch, OK);
    }
    let rnum;
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
            send_to_char(
                ch,
                format!(
                    "{}'s title is now: {}\r\n",
                    vict.get_name(),
                    vict.get_title()
                )
                .as_str(),
            );
        }
        3 => {
            if on {
                vict.set_prf_flags_bits(PRF_SUMMONABLE)
            } else {
                vict.remove_prf_flags_bits(PRF_SUMMONABLE)
            }
            send_to_char(
                ch,
                format!("Nosummon {} for {}.\r\n", onoff!(!on), vict.get_name()).as_str(),
            );
        }
        4 => {
            vict.points.borrow_mut().max_hit = range!(value, 1, 5000) as i16;
            affect_total(vict);
        }
        5 => {
            vict.points.borrow_mut().max_mana = range!(value, 1, 5000) as i16;
            affect_total(vict);
        }
        6 => {
            vict.points.borrow_mut().max_move = range!(value, 1, 5000) as i16;
            affect_total(vict);
        }
        7 => {
            vict.points.borrow_mut().hit = range!(value, -9, vict.points.borrow().max_hit) as i16;
            affect_total(vict);
        }
        8 => {
            vict.points.borrow_mut().mana = range!(value, 0, vict.points.borrow().max_mana) as i16;
            affect_total(vict);
        }
        9 => {
            vict.points.borrow_mut().movem = range!(value, 0, vict.points.borrow().max_move) as i16;
            affect_total(vict);
        }
        10 => {
            vict.set_alignment(range!(value, -1000, 1000));
            affect_total(vict);
        }
        11 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.borrow_mut().str = value as i8;
            vict.real_abils.borrow_mut().str_add = 0;
            affect_total(vict);
        }
        12 => {
            vict.real_abils.borrow_mut().str_add = range!(value, 0, 100) as i8;
            if value > 0 {
                vict.real_abils.borrow_mut().str = 18;
            }
            affect_total(vict);
        }
        13 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.borrow_mut().intel = value as i8;
            affect_total(vict);
        }
        14 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.borrow_mut().wis = value as i8;
            affect_total(vict);
        }
        15 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.borrow_mut().dex = value as i8;
            affect_total(vict);
        }
        16 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.borrow_mut().con = value as i8;
            affect_total(vict);
        }
        17 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.borrow_mut().cha = value as i8;
            affect_total(vict);
        }
        18 => {
            vict.points.borrow_mut().armor = range!(value, -100, 100) as i16;
            affect_total(vict);
        }
        19 => {
            vict.set_gold(range!(value, 0, 100000000));
        }
        20 => {
            vict.set_bank_gold(range!(value, 0, 100000000));
        }
        21 => {
            vict.points.borrow_mut().exp = range!(value, 0, 50000000);
        }
        22 => {
            vict.points.borrow_mut().hitroll = range!(value, -20, 20) as i8;
            affect_total(vict);
        }
        23 => {
            vict.points.borrow_mut().damroll = range!(value, -20, 20) as i8;
            affect_total(vict);
        }
        24 => {
            if ch.get_level() < LVL_IMPL as u8 && !Rc::ptr_eq(ch, vict) {
                send_to_char(ch, "You aren't godly enough for that!\r\n");
                return false;
            }
            vict.set_invis_lev(range!(value, 0, vict.get_level()) as i16);
        }
        25 => {
            if ch.get_level() < LVL_IMPL as u8 && !Rc::ptr_eq(ch, vict) {
                send_to_char(ch, "You aren't godly enough for that!\r\n");
                return false;
            }
            if on {
                vict.set_prf_flags_bits(PRF_NOHASSLE)
            } else {
                vict.remove_prf_flags_bits(PRF_NOHASSLE)
            }
        }
        26 => {
            if Rc::ptr_eq(ch, vict) && on {
                send_to_char(ch, "Better not -- could be a long winter!\r\n");
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
                send_to_char(
                    ch,
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
                send_to_char(
                    ch,
                    format!(
                        "{}'s {} set to {}.\r\n",
                        vict.get_name(),
                        SET_FIELDS[mode].cmd,
                        value
                    )
                    .as_str(),
                );
            } else {
                send_to_char(ch, "Must be 'off' or a value from 0 to 24.\r\n");
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
            if value > ch.get_level() as i32 || value > LVL_IMPL as i32 {
                send_to_char(ch, "You can't do that.\r\n");
                return false;
            }
            value = range!(value, 0, LVL_IMPL);
            vict.player.borrow_mut().level = value as u8;
        }
        35 => {
            if {
                rnum = db.real_room(value as RoomRnum);
                rnum == NOWHERE
            } {
                send_to_char(ch, "No room exists with that number.\r\n");
                return false;
            }
            if vict.in_room() != NOWHERE {
                /* Another Eric Green special. */
                db.char_from_room(vict);
            }
            db.char_to_room(vict, rnum);
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
                send_to_char(ch, "That is not a class.\r\n");
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
                    vict.set_plr_flag_bit(PLR_LOADROOM);
                    vict.set_loadroom(rvnum);
                    send_to_char(
                        ch,
                        format!(
                            "{} will enter at room #{}.",
                            vict.get_name(),
                            vict.get_loadroom()
                        )
                        .as_str(),
                    );
                } else {
                    send_to_char(ch, "That room does not exist!\r\n");
                    return false;
                }
            } else {
                send_to_char(ch, "Must be 'off' or a room's virtual number.\r\n");
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
            if ch.get_idnum() != 1 || !vict.is_npc() {
                return false;
            }
            vict.set_idnum(value as i64);
        }
        45 => {
            if ch.get_idnum() > 1 {
                send_to_char(ch, "Please don't use this command, yet.\r\n");
                return false;
            }
            if vict.get_level() >= LVL_GRGOD as u8 {
                send_to_char(ch, "You cannot change that.\r\n");
                return false;
            }
            let mut passwd2 = [0 as u8; 16];
            let salt = vict.get_name();
            pbkdf2::pbkdf2::<Hmac<Sha256>>(val_arg.as_bytes(), salt.as_bytes(), 4, &mut passwd2)
                .expect("Error while encrypting password");
            vict.set_passwd(passwd2);
            send_to_char(
                ch,
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
                send_to_char(ch, "Must be 'male', 'female', or 'neutral'.\r\n");
                return false;
            }
            vict.set_sex(i.unwrap() as u8);
        }
        48 => {
            /* set age */
            if value < 2 || value > 200 {
                /* Arbitrary limits. */
                send_to_char(ch, "Ages 2 to 200 accepted.\r\n");
                return false;
            }
            /*
             * NOTE: May not display the exact age specified due to the integer
             * division used elsewhere in the code.  Seems to only happen for
             * some values below the starting age (17) anyway. -gg 5/27/98
             */
            vict.player.borrow_mut().time.birth =
                time_now() - ((value as u64 - 17) * SECS_PER_MUD_YEAR);
        }

        49 => {
            /* Blame/Thank Rick Glover. :) */
            vict.set_height(value as u8);
            affect_total(vict);
        }

        50 => {
            vict.set_weight(value as u8);
            affect_total(vict);
        }

        _ => {
            send_to_char(ch, "Can't set that!\r\n");
            return false;
        }
    }

    true
}

pub fn do_set(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
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
        send_to_char(ch, "Usage: set <victim> <field> <value>\r\n");
        return;
    }
    let mut vict = None;
    /* find the target */
    if !is_file {
        if is_player {
            if {
                vict = game.db.get_player_vis(ch, &mut name, None, FIND_CHAR_WORLD);
                vict.is_none()
            } {
                send_to_char(ch, "There is no such player.\r\n");
                return;
            }
        } else {
            /* is_mob */
            if {
                vict = game.db.get_char_vis(ch, &mut name, None, FIND_CHAR_WORLD);
                vict.is_none()
            } {
                send_to_char(ch, "There is no such creature.\r\n");
                return;
            }
        }
    } else if is_file {
        /* try to load the player off disk */
        let mut cbuf = CharData::new();
        clear_char(&mut cbuf);
        if {
            player_i = game.db.load_char(&name, &mut tmp_store);
            player_i.is_some()
        } {
            store_to_char(&tmp_store, &mut cbuf);
            if cbuf.get_level() >= ch.get_level() {
                send_to_char(ch, "Sorry, you can't do that.\r\n");
                return;
            }
            vict = Some(Rc::new(cbuf));
        } else {
            send_to_char(ch, "There is no such player.\r\n");
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
    let retval = perform_set(&mut game.db, ch, vict.as_ref().unwrap(), mode as i32, &buf);

    /* save the character if a change was made */
    if retval {
        if !is_file && !vict.as_ref().unwrap().is_npc() {
            game.db.save_char(vict.as_ref().unwrap());
        }
        if is_file {
            game.db.char_to_store(vict.as_ref().unwrap(), &mut tmp_store);

            unsafe {
                let player_slice = slice::from_raw_parts(
                    &mut tmp_store as *mut _ as *mut u8,
                    mem::size_of::<CharFileU>(),
                );
                game.db.player_fl
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .write_all_at(
                        player_slice,
                        (player_i.unwrap() * mem::size_of::<CharFileU>()) as u64,
                    )
                    .expect("Error while writing player record to file");
            }

            send_to_char(ch, "Saved in file.\r\n");
        }
    }
}
