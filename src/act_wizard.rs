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
    clear_char, parse_c_string, store_to_char, DB, FASTBOOT_FILE, KILLSCRIPT_FILE, PAUSE_FILE, LoadType,
};
use crate::depot::{Depot, DepotId, HasId};
use crate::fight::{appear, update_pos, ATTACK_HIT_TEXT};
use crate::handler::{affect_remove, affect_total, get_char_vis, get_number, get_obj_in_equip_vis, get_obj_in_list_vis, get_obj_in_list_vis2, get_obj_vis, get_player_vis, obj_to_char, FindFlags};
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
    AffectFlags, CharData, CharFileU, ItemType, PrefFlags, RoomFlags, RoomRnum, RoomVnum, ZoneRnum, Class, DRUNK, FULL, LVL_FREEZE, LVL_GOD, LVL_GRGOD, LVL_IMMORT, LVL_IMPL, MAX_OBJ_AFFECT, MAX_SKILLS, NOBODY, NOTHING, NOWHERE, NUM_OF_DIRS, NUM_WEARS, PLR_DELETED, PLR_FROZEN, PLR_INVSTART, PLR_KILLER, PLR_LOADROOM, PLR_MAILING, PLR_NODELETE, PLR_NOSHOUT, PLR_NOTITLE, PLR_NOWIZLIST, PLR_SITEOK, PLR_THIEF, PLR_WRITING, THIRST
};
use crate::util::{
    age, can_see, can_see_obj, ctime, hmhr, pers, sprintbit, sprinttype, time_now, touch, DisplayMode, SECS_PER_MUD_YEAR
};
use crate::{act, char_to_store, save_char, send_to_char, send_to_room, vnum_mobile, vnum_object, DescriptorData, ObjData, TextData, VictimRef};
use crate::{
    _clrlevel, clr, onoff, yesno, Game, CCCYN, CCGRN, CCNRM, CCYEL, TO_CHAR, TO_NOTVICT, TO_ROOM,
    TO_VICT,
};
use chrono::{TimeZone, Utc};
use hmac::Hmac;
use log::{error, info};
use sha2::Sha256;

#[allow(clippy::too_many_arguments)]
pub fn do_echo(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let argument = argument.trim_start();

    if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Yes.. but what?\r\n");
    } else {
        let buf=
        if subcmd == SCMD_EMOTE {
            format!("$n {}", argument)
        } else {
            argument.to_string()
        };

        act(&mut game.descriptors, chars, db, &buf, false, Some(ch), None, None, TO_ROOM);
        let ch = chars.get(chid);
        if ch.prf_flagged(PrefFlags::NOREPEAT) {
            send_to_char(&mut game.descriptors, ch, OK);
        } else {
            act(&mut game.descriptors, chars, db, &buf, false, Some(ch), None, None, TO_CHAR);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_send(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut argument = argument.to_string();
    let mut arg = String::new();
    let mut buf = String::new();
    let vict;

    half_chop(&mut argument, &mut arg, &mut buf);

    if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Send what to who?\r\n");
        return;
    }
    let res = {
        vict = get_char_vis(&game.descriptors, chars,db, ch, &mut arg, None, FindFlags::CHAR_WORLD);
        vict.is_none()
    }; 
    if res {
        send_to_char(&mut game.descriptors, ch, NOPERSON);
        return;
    }
    let vict = vict.unwrap();
    send_to_char(&mut game.descriptors, vict, format!("{}\r\n", buf).as_str());
    let ch = chars.get(chid);
    if ch.prf_flagged(PrefFlags::NOREPEAT) {
        send_to_char(&mut game.descriptors, ch, "Sent.\r\n");
    } else {
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "You send '{}' to {}.\r\n",
                buf,
                vict.get_name()
            )
            .as_str(),
        );
    }
}

/* take a string, and return an rnum.. used for goto, at, etc.  -je 4/6/93 */
fn find_target_room(descs: &mut Depot<DescriptorData>, db: &DB,chars: &Depot<CharData>, objs: & Depot<ObjData>, ch: &CharData, rawroomstr: &str) -> RoomRnum {

    let mut location = NOWHERE;
    let mut roomstr = String::new();
    one_argument(rawroomstr, &mut roomstr);

    if roomstr.is_empty() {
        send_to_char(descs, ch, "You must supply a room number or name.\r\n");
        return NOWHERE;
    }

    if roomstr.chars().next().unwrap().is_ascii_digit() && !roomstr.contains('.') {
        let res = {
            location = db.real_room(roomstr.parse::<i16>().unwrap());
            location == NOWHERE
        }; 
        if res {
            send_to_char(descs, ch, "No room exists with that number.\r\n");
            return NOWHERE;
        }
    } else {
        let target_mob;
        let target_obj;
        let mut mobobjstr = roomstr;

        let mut num = get_number(&mut mobobjstr);
        let res = {
            target_mob =
                get_char_vis(descs, chars,db, ch, &mut mobobjstr, Some(&mut num), FindFlags::CHAR_WORLD);
            target_mob.is_some()
        }; 
        #[allow(clippy::blocks_in_conditions)]
        if res {
            let res = {
                location = target_mob.unwrap().in_room();
                location == NOWHERE
            }; 
            if res {
                send_to_char(descs, ch, "That character is currently lost.\r\n");
                return NOWHERE;
            }
        } else if {
            target_obj = get_obj_vis(descs, chars,db, objs,ch, &mobobjstr, Some(&mut num));
            target_obj.is_some()
        } {
            if target_obj.unwrap().in_room() != NOWHERE {
                location = target_obj.unwrap().in_room();
            } else if target_obj.unwrap().carried_by.borrow().is_some()
                && chars.get(target_obj.unwrap().carried_by.unwrap())
                    .in_room()
                    != NOWHERE
            {
                location = chars.get(target_obj.unwrap().carried_by.unwrap())
                    .in_room();
            } else if target_obj.unwrap().worn_by.borrow().is_some()
                && chars.get(target_obj.unwrap().worn_by.unwrap())
                    .in_room()
                    != NOWHERE
            {
                location = chars.get(target_obj.unwrap().worn_by.unwrap())
                    .in_room();
            }

            if location == NOWHERE {
                send_to_char(descs, ch, "That object is currently not in a room.\r\n");
                return NOWHERE;
            }
        }

        if location == NOWHERE {
            send_to_char(descs, ch, "Nothing exists by that name.\r\n");
            return NOWHERE;
        }
    }

    /* a location has been found -- if you're >= GRGOD, no restrictions. */
    if ch.get_level() >= LVL_GRGOD as u8 {
        return location;
    }

    if db.room_flagged(location, RoomFlags::GODROOM) {
        send_to_char(descs, ch, "You are not godly enough to use that room!\r\n");
    } else if db.room_flagged(location, RoomFlags::PRIVATE)
        && db.world[location as usize].peoples.len() > 1
    {
        send_to_char(descs, 
            ch,
            "There's a private conversation going on in that room.\r\n",
        );
    } else if db.room_flagged(location, RoomFlags::HOUSE)
        && !house_can_enter(db, ch, db.get_room_vnum(location))
    {
        send_to_char(descs, ch, "That's private property -- no trespassing!\r\n");
    } else {
        return location;
    }

    NOWHERE
}

#[allow(clippy::too_many_arguments)]
pub fn do_at(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut argument = argument.to_string();
    let mut buf = String::new();
    let mut command = String::new();

    half_chop(&mut argument, &mut buf, &mut command);
    if buf.is_empty() {
        send_to_char(&mut game.descriptors, ch, "You must supply a room number or a name.\r\n");
        return;
    }

    if command.is_empty() {
        send_to_char(&mut game.descriptors, ch, "What do you want to do there?\r\n");
        return;
    }
    let location;
    let res = {
        location = find_target_room(&mut game.descriptors, db,chars, objs,ch, &buf);
        location == NOWHERE
    }; 
    if res {
        return;
    }

    /* a location has been found. */
    let ch = chars.get_mut(chid);
    let original_loc = ch.in_room();
    db.char_from_room( objs,ch);
    db.char_to_room(chars, objs,chid, location);
    command_interpreter(game, db, chars, texts,objs,chid, &command);

    /* check if the char is still there */
    let ch = chars.get_mut(chid);
    if ch.in_room() == location {
        db.char_from_room(objs, ch);
        db.char_to_room(chars, objs,chid, original_loc);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_goto(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let location;
    let ch = chars.get(chid);

    let res = {
        location = find_target_room(&mut game.descriptors, db,chars, objs,ch, argument);
        location == NOWHERE
    }; 
    if res {
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
    act(&mut game.descriptors, chars, db, &buf, true, Some(ch), None, None, TO_ROOM);
    let ch = chars.get_mut(chid);
    db.char_from_room( objs,ch);
    db.char_to_room(chars, objs,chid, location);
    let ch = chars.get(chid);
    let x = ch.poofin();
    let buf = format!(
        "$n {}",
        if !x.is_empty() {
            x.as_ref()
        } else {
            "appears with an ear-splitting bang."
        }
    );
    act(&mut game.descriptors, chars, db, &buf, true, Some(ch), None, None, TO_ROOM);

    look_at_room(&mut game.descriptors, db,chars, texts,objs,ch, false);
}

#[allow(clippy::too_many_arguments)]
pub fn do_trans(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let victim;
    if buf.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Whom do you wish to transfer?\r\n");
    } else if "all" != buf {
        let res = {
            victim = get_char_vis(&game.descriptors, chars,db, ch, &mut buf, None, FindFlags::CHAR_WORLD);
            victim.is_none()
        }; 
        if res {
            send_to_char(&mut game.descriptors, ch, NOPERSON);
        } else if victim.unwrap().id() == chid {
            send_to_char(&mut game.descriptors, ch, "That doesn't make much sense, does it?\r\n");
        } else {
            let victim = victim.unwrap();
            if (ch.get_level() < victim.get_level()) && !victim.is_npc() {
                send_to_char(&mut game.descriptors, ch, "Go transfer someone your own size.\r\n");
                return;
            }
            act(&mut game.descriptors, chars, 
                db,
                "$n disappears in a mushroom cloud.",
                false,
                Some(victim),
                None,
                None,
                TO_ROOM,
            );
            let victim_id = victim.id();
            let victim = chars.get_mut(victim_id);
            db.char_from_room(objs, victim);
            let ch = chars.get(chid);
            db.char_to_room(chars, objs, victim_id, ch.in_room());
            let victim = chars.get(victim_id);
            act(&mut game.descriptors, chars, 
                db,
                "$n arrives from a puff of smoke.",
                false,
                Some(victim),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get(chid);
            act(&mut game.descriptors, chars, 
                db,
                "$n has transferred you!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(victim)),
                TO_VICT,
            );
            look_at_room(&mut game.descriptors, db,chars, texts, objs, chars.get(victim_id), false);
        }
    } else {
        /* Trans All */
        if ch.get_level() < LVL_GRGOD as u8 {
            send_to_char(&mut game.descriptors, ch, "I think not.\r\n");
            return;
        }

        let list = game.descriptor_list.clone();
        for i_id in list {
            let i = game.desc(i_id);
            if i.state() == ConPlaying
                && i.character.is_some()
                && i.character.unwrap() != chid
            {
                let ic = i.character;
                let victim_id = ic.unwrap();
                let victim = chars.get(victim_id);
                let ch = chars.get(chid);
                if victim.get_level() >= ch.get_level() {
                    continue;
                }
                act(&mut game.descriptors, chars, 
                    db,
                    "$n disappears in a mushroom cloud.",
                    false,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                let victim = chars.get_mut(victim_id);
                db.char_from_room(objs, victim);
                let ch = chars.get(chid);
                db.char_to_room(chars, objs, victim_id, ch.in_room());
                let victim = chars.get(victim_id);
                act(&mut game.descriptors, chars, 
                    db,
                    "$n arrives from a puff of smoke.",
                    false,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                let ch = chars.get(chid);
                act(&mut game.descriptors, chars, 
                    db,
                    "$n has transferred you!",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(victim)),
                    TO_VICT,
                );
                look_at_room(&mut game.descriptors, db,chars, texts, objs, victim, false);
            }
        }
    }
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch, OK);
}

#[allow(clippy::too_many_arguments)]
pub fn do_teleport(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);
    let victim;
    let target;
    #[allow(clippy::blocks_in_conditions)]
    if buf.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Whom do you wish to teleport?\r\n");
    } else if {
        victim = get_char_vis(&game.descriptors, chars,db, ch, &mut buf, None, FindFlags::CHAR_WORLD);
        victim.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, NOPERSON);
    } else if victim.unwrap().id() == chid {
        send_to_char(&mut game.descriptors, ch, "Use 'goto' to teleport yourself.\r\n");
    } else if victim.as_ref().unwrap().get_level() >= ch.get_level() {
        send_to_char(&mut game.descriptors, ch, "Maybe you shouldn't do that.\r\n");
    } else if buf2.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Where do you wish to send this person?\r\n");
    } else if {
        target = find_target_room(&mut game.descriptors, db,chars,objs, ch, &buf2);
        target != NOWHERE
    } {
        let victim = victim.unwrap();
        send_to_char(&mut game.descriptors, ch, OK);
        act(&mut game.descriptors, chars, 
            db,
            "$n disappears in a puff of smoke.",
            false,
            Some(victim),
            None,
            None,
            TO_ROOM,
        );
        let victim_id = victim.id();
        let victim = chars.get_mut(victim_id);
        db.char_from_room( objs,victim);
        db.char_to_room(chars, objs,victim_id, target);
        let victim = chars.get(victim_id);
        act(&mut game.descriptors, chars, 
            db,
            "$n arrives from a puff of smoke.",
            false,
            Some(victim),
            None,
            None,
            TO_ROOM,
        );
        let ch = chars.get(chid);
        let victim = chars.get(victim_id);
        act(&mut game.descriptors, chars, 
            db,
            "$n has teleported you!",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(victim)),
            TO_VICT,
        );
        look_at_room(&mut game.descriptors, db,chars, texts,objs, victim, false);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_vnum(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut  Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !is_abbrev(&buf, "mob") && !is_abbrev(&buf, "obj") {
        send_to_char(&mut game.descriptors, ch, "Usage: vnum { obj | mob } <name>\r\n");
        return;
    }
    if is_abbrev(&buf, "mob")
        && vnum_mobile(&mut game.descriptors, db,chars, &buf2, chid) == 0 {
            send_to_char(&mut game.descriptors, ch, "No mobiles by that name.\r\n");
        }

    if is_abbrev(&buf, "obj")
        && vnum_object(&mut game.descriptors, db, chars,&buf2, chid) == 0 {
            send_to_char(&mut game.descriptors, ch, "No objects by that name.\r\n");
        }
}

fn do_stat_room(descs: &mut Depot<DescriptorData>, db: &mut DB,chars: &mut Depot<CharData>,objs: & Depot<ObjData>, chid: DepotId) {
    let ch = chars.get(chid);

    let rm_name = db.world[ch.in_room() as usize].name.as_str();
    let rm_zone = db.world[ch.in_room() as usize].zone;
    let rm_sector_type = db.world[ch.in_room() as usize].sector_type;
    let rm_number = db.world[ch.in_room() as usize].number;
    let rm_func_is_none = db.world[ch.in_room() as usize].func.is_none();
    let rm_description = db.world[ch.in_room() as usize].description.as_str();
    let rm_ex_descriptions_len = db.world[ch.in_room() as usize].ex_descriptions.len();
    let rm_peoples = &db.world[ch.in_room() as usize].peoples;
    let rm_contents = &db.world[ch.in_room() as usize].contents;
    let rm_room_flags = db.world[ch.in_room() as usize].room_flags;
    let rm_dir_option = &db.world[ch.in_room() as usize].dir_option;

    send_to_char(descs, 
        ch,
        format!(
            "Room name: {}{}{}\r\n",
            CCCYN!(ch, C_NRM),
            rm_name,
            CCNRM!(ch, C_NRM)
        )
        .as_str(),
    );
    let mut buf2 = String::new();
    sprinttype(rm_sector_type as i32, &SECTOR_TYPES, &mut buf2);
    let ch = chars.get(chid);
    send_to_char(descs, 
        ch,
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

    sprintbit(rm_room_flags.bits(), &ROOM_BITS, &mut buf2);
    send_to_char(descs, 
        ch,
        format!(
            "SpecProc: {}, Flags: {}\r\n",
            if rm_func_is_none { "None" } else { "Exists" },
            buf2
        )
        .as_str(),
    );

    send_to_char(descs, 
        ch,
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
        let ch = chars.get(chid);
        send_to_char(descs, ch, format!("Extra descs:{}", CCCYN!(ch, C_NRM)).as_str());
        for idx in 0..rm_ex_descriptions_len {
            let desc_keyword = &db.world[ch.in_room() as usize].ex_descriptions[idx]
                .keyword;
            send_to_char(descs, ch, format!(" {}", desc_keyword).as_str());
            send_to_char(descs, ch, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());
        }
    }

    if !rm_peoples.is_empty() {
        let ch = chars.get(chid);
        send_to_char(descs, ch, format!("Chars present:{}", CCYEL!(ch, C_NRM)).as_str());
        let mut column = 14; /* ^^^ strlen ^^^ */
        let mut found = 0;
        for (i, k_id) in rm_peoples.iter().enumerate() {
            let k = chars.get(*k_id);
            let ch = chars.get(chid);
            if !can_see(descs, chars, db, ch, k) {
                continue;
            }

            column += send_to_char(descs, 
                ch,
                format!(
                    "{} {}({})",
                    if found != 0 { "," } else { "" },
                    k.get_name(),
                    if !k.is_npc() {
                        "PC"
                    } else if !db.is_mob(k) {
                        "NPC"
                    } else {
                        "MOB"
                    }
                )
                .as_str(),
            );
            found += 1;
            if column >= 62 {
                send_to_char(descs, 
                    ch,
                    format!("{}\r\n", if i == rm_peoples.len() - 1 { "," } else { "" }).as_str(),
                );
                found = 0;
                column = 0;
            }
        }
        let ch = chars.get(chid);
        send_to_char(descs, ch, CCNRM!(ch, C_NRM));
    }
    if !rm_contents.is_empty() {
        let ch = chars.get(chid);
        send_to_char(descs, ch, format!("Contents:{}", CCGRN!(ch, C_NRM)).as_str());
        let mut column = 9; /* ^^^ strlen ^^^ */
        let mut found = 0;
        for (i, oid) in rm_contents.iter().enumerate() {
            let ch = chars.get(chid);
            if !can_see_obj(descs, chars, db, ch, objs.get(*oid)) {
                continue;
            }

            column += send_to_char(descs, 
                ch,
                format!(
                    "{} {}",
                    if found != 0 { "," } else { "" },
                    objs.get(*oid).short_description
                )
                .as_str(),
            );
            found += 1;
            if column >= 62 {
                send_to_char(descs, 
                    ch,
                    format!("{}\r\n", if i == rm_contents.len() - 1 { "," } else { "" }).as_str(),
                );
                found = 0;
                column = 0;
            }
        }
        let ch = chars.get(chid);
        send_to_char(descs, ch, CCNRM!(ch, C_NRM));
    }

    for i in 0..NUM_OF_DIRS {
        if rm_dir_option[i].is_none() {
            continue;
        }
        let buf1=
        if rm_dir_option[i].as_ref().unwrap().to_room == NOWHERE {
            let ch = chars.get(chid);
            format!(" {}NONE{}", CCCYN!(ch, C_NRM), CCNRM!(ch, C_NRM))
        } else {
            let ch = chars.get(chid);
            format!(
                "{}{:5}{}",
                CCCYN!(ch, C_NRM),
                db.get_room_vnum(rm_dir_option[i].as_ref().unwrap().to_room),
                CCNRM!(ch, C_NRM)
            )
        };
        let mut buf2 = String::new();
        sprintbit(
            rm_dir_option[i].as_ref().unwrap().exit_info.bits().into(),
            &EXIT_BITS,
            &mut buf2,
        );
        let ch = chars.get(chid);
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
        send_to_char(descs, ch, msg.as_str());
    }
}

fn do_stat_object(descs: &mut Depot<DescriptorData>, db: &DB,chars: &Depot<CharData>,objs: & Depot<ObjData>, ch: &CharData, obj: &ObjData) {

    let vnum = db.get_obj_vnum(obj);
    send_to_char(descs, 
        ch,
        format!(
            "Name: '{}{}{}', Aliases: {}\r\n",
            CCYEL!(ch, C_NRM),
            if !obj.short_description.is_empty() {
                &obj.short_description
            } else {
                "<None>"
            },
            CCNRM!(ch, C_NRM),
            obj.name
        )
        .as_str(),
    );
    let mut buf = String::new();
    sprinttype(obj.get_obj_type() as i32, &ITEM_TYPES, &mut buf);
    send_to_char(descs, 
        ch,
        format!(
            "VNum: [{}{:5}{}], RNum: [{:5}], Type: {}, SpecProc: {}\r\n",
            CCGRN!(ch, C_NRM),
            vnum,
            CCNRM!(ch, C_NRM),
            obj.get_obj_rnum(),
            buf,
            if db.get_obj_spec(obj).is_some() {
                "Exists"
            } else {
                "none"
            }
        )
        .as_str(),
    );

    if !obj.ex_descriptions.is_empty() {
        send_to_char(descs, ch, format!("Extra descs:{}", CCCYN!(ch, C_NRM)).as_str());

        for desc in obj.ex_descriptions.iter() {
            send_to_char(descs, ch, format!(" {}", desc.keyword).as_str());
            send_to_char(descs, ch, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());
        }
    }
    buf.clear();
    sprintbit(obj.get_obj_wear().bits().into(), &WEAR_BITS, &mut buf);
    send_to_char(descs, ch, format!("Can be worn on: {}\r\n", buf).as_str());
    buf.clear();
    sprintbit(obj.get_obj_affect().bits(), &AFFECTED_BITS, &mut buf);
    send_to_char(descs, ch, format!("Set char bits : {}\r\n", buf).as_str());
    buf.clear();
    sprintbit(obj.get_obj_extra().bits() as i64, &EXTRA_BITS, &mut buf);
    send_to_char(descs, ch, format!("Extra flags   : {}\r\n", buf).as_str());

    send_to_char(descs, 
        ch,
        format!(
            "Weight: {}, Value: {}, Cost/day: {}, Timer: {}\r\n",
            obj.get_obj_weight(),
            obj.get_obj_cost(),
            obj.get_obj_rent(),
            obj.get_obj_timer()
        )
        .as_str(),
    );
    send_to_char(descs, 
        ch,
        format!(
            "In room: {} ({}), ",
            db.get_room_vnum(obj.in_room()),
            if obj.in_room() == NOWHERE {
                "Nowhere"
            } else {
                db.world[obj.in_room() as usize].name.as_str()
            }
        )
        .as_str(),
    );

    /*
     * NOTE: In order to make it this far, we must already be able to see the
     *       character holding the object. Therefore, we do not need CAN_SEE().
     */
    let jio = obj.in_obj.borrow();
    send_to_char(descs, 
        ch,
        format!(
            "In object: {}, ",
            if obj.in_obj.borrow().is_some() {
                objs.get(jio.unwrap()).short_description.as_ref()
            } else {
                "None"
            }
        )
        .as_str(),
    );
    send_to_char(descs, 
        ch,
        format!(
            "Carried by: {}, ",
            if obj.carried_by.is_some() {
                chars.get(obj.carried_by.unwrap()).get_name().as_ref()
            } else {
                "Nobody"
            }
        )
        .as_str(),
    );
    send_to_char(descs, 
        ch,
        format!(
            "Worn by: {}\r\n",
            if obj.worn_by.is_some() {
                chars.get(obj.worn_by.unwrap()).get_name().as_ref()
            } else {
                "Nobody"
            }
        )
        .as_str(),
    );

    match obj.get_obj_type() {
        ItemType::Light => {
            if obj.get_obj_val(2) == -1 {
                send_to_char(descs, ch, "Hours left: Infinite\r\n");
            } else {
                send_to_char(descs, 
                    ch,
                    format!("Hours left: [{}]\r\n", obj.get_obj_val(2)).as_str(),
                );
            }
        }
        ItemType::Scroll | ItemType::Potion => {
            send_to_char(descs, 
                ch,
                format!(
                    "Spells: (Level {}) {}, {}, {}\r\n",
                    obj.get_obj_val(0),
                    skill_name(db, obj.get_obj_val(1)),
                    skill_name(db, obj.get_obj_val(2)),
                    skill_name(db, obj.get_obj_val(3))
                )
                .as_str(),
            );
        }
        ItemType::Wand | ItemType::Staff => {
            send_to_char(descs, 
                ch,
                format!(
                    "Spell: {} at level {}, {} (of {}) charges remaining\r\n",
                    skill_name(db, obj.get_obj_val(3)),
                    obj.get_obj_val(0),
                    obj.get_obj_val(2),
                    obj.get_obj_val(1)
                )
                .as_str(),
            );
        }
        ItemType::Weapon => {
            send_to_char(descs, 
                ch,
                format!(
                    "Todam: {}d{}, Message type: {}\r\n",
                    obj.get_obj_val(1),
                    obj.get_obj_val(2),
                    obj.get_obj_val(3)
                )
                .as_str(),
            );
        }
        ItemType::Armor => {
            send_to_char(descs, 
                ch,
                format!("AC-apply: [{}]\r\n", obj.get_obj_val(0)).as_str(),
            );
        }
        ItemType::Trap => {
            send_to_char(descs, 
                ch,
                format!(
                    "Spell: {}, - Hitpoints: {}\r\n",
                    obj.get_obj_val(0),
                    obj.get_obj_val(1)
                )
                .as_str(),
            );
        }
        ItemType::Container => {
            buf.clear();
            sprintbit(obj.get_obj_val(1) as i64, &CONTAINER_BITS, &mut buf);
            send_to_char(descs, 
                ch,
                format!(
                    "Weight capacity: {}, Lock Type: {}, Key Num: {}, Corpse: {}\r\n",
                    obj.get_obj_val(0),
                    buf,
                    obj.get_obj_val(2),
                    yesno!(obj.get_obj_val(3) != 0)
                )
                .as_str(),
            );
        }
        ItemType::Drinkcon | ItemType::Fountain => {
            buf.clear();
            sprinttype(obj.get_obj_val(2), &DRINKS, &mut buf);
            send_to_char(descs, 
                ch,
                format!(
                    "Capacity: {}, Contains: {}, Poisoned: {}, Liquid: {}\r\n",
                    obj.get_obj_val(0),
                    obj.get_obj_val(1),
                    yesno!(obj.get_obj_val(3) != 0),
                    buf
                )
                .as_str(),
            );
        }
        ItemType::Note => {
            send_to_char(descs, 
                ch,
                format!("Tongue: {}\r\n", obj.get_obj_val(0)).as_str(),
            );
        }
        ItemType::Key => { /* Nothing */ }
        ItemType::Food => {
            send_to_char(descs, 
                ch,
                format!(
                    "Makes full: {}, Poisoned: {}\r\n",
                    obj.get_obj_val(0),
                    yesno!(obj.get_obj_val(3) != 0)
                )
                .as_str(),
            );
        }
        ItemType::Money => {
            send_to_char(descs, 
                ch,
                format!("Coins: {}\r\n", obj.get_obj_val(0)).as_str(),
            );
        }
        _ => {
            send_to_char(descs, 
                ch,
                format!(
                    "Values 0-3: [{}] [{}] [{}] [{}]\r\n",
                    obj.get_obj_val(0),
                    obj.get_obj_val(1),
                    obj.get_obj_val(2),
                    obj.get_obj_val(3)
                )
                .as_str(),
            );
        }
    }

    /*
     * I deleted the "equipment status" code from here because it seemed
     * more or less useless and just takes up valuable screen space.
     */

    if !obj.contains.is_empty() {
        send_to_char(descs, ch, format!("\r\nContents:{}", CCGRN!(ch, C_NRM)).as_str());
        let mut column = 9; /* ^^^ strlen ^^^ */
        let mut found = 0;

        for (i2, j2) in obj.contains.iter().enumerate() {
            let messg = format!(
                "{} {}",
                if found != 0 { "," } else { "" },
                objs.get(*j2).short_description
            );
            column += send_to_char(descs, ch, messg.as_str());
            if column >= 62 {
                let messg = format!(
                    "{}\r\n",
                    if i2 < obj.contains.len() - 1 {
                        ","
                    } else {
                        ""
                    }
                );
                send_to_char(descs, ch, messg.as_str());
                found = 0;
                column = 0;
            }
        }
        send_to_char(descs, ch, CCNRM!(ch, C_NRM));
    }

    let mut found = 0;
    send_to_char(descs, ch, "Affections:");

    for i in 0..MAX_OBJ_AFFECT as usize {
        if obj.affected[i].modifier != 0 {
            buf.clear();
            sprinttype(
                obj.affected[i].location as i32,
                &APPLY_TYPES,
                &mut buf,
            );
            send_to_char(descs, 
                ch,
                format!(
                    "{} {} to {}",
                    if found != 0 { "," } else { "" },
                    obj.affected[i].modifier,
                    buf
                )
                .as_str(),
            );
            found += 1;
        }
        if found == 0 {
            send_to_char(descs, ch, " None");
        }
        send_to_char(descs, ch, "\r\n");
    }
}

fn do_stat_character(descs: &mut Depot<DescriptorData>, db: &DB,chars: &Depot<CharData>, ch: &CharData, k: &CharData) {

    let mut buf = String::new();
    sprinttype(k.get_sex() as i32, &GENDERS, &mut buf);
    send_to_char(descs, 
        ch,
        format!(
            "{} {} '{}'  IDNum: [{:5}], In room [{:5}]\r\n",
            buf,
            if !k.is_npc() {
                "PC"
            } else if !db.is_mob(k) {
                "NPC"
            } else {
                "MOB"
            },
            k.get_name(),
            k.get_idnum(),
            db.get_room_vnum(k.in_room())
        )
        .as_str(),
    );
    if db.is_mob(k) {
        send_to_char(descs, 
            ch,
            format!(
                "Alias: {}, VNum: [{:5}], RNum: [{:5}]\r\n",
                k.player.name,
                db.get_mob_vnum(k),
                k.get_mob_rnum()
            )
            .as_str(),
        );
    }
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

    send_to_char(descs, ch, messg_title.as_str());
    send_to_char(descs, ch, messg_descr.as_str());
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
    send_to_char(descs, 
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
        let buf1 = ctime(k.player.time.birth);
        let buf2 = ctime(k.player.time.logon);
        send_to_char(descs, 
            ch,
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
        send_to_char(descs, 
            ch,
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
    send_to_char(descs, 
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
    send_to_char(descs, 
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
    send_to_char(descs, 
        ch,
        format!(
            "Coins: [{:9}], Bank: [{:9}] (Total: {})\r\n",
            k.get_gold(),
            k.get_bank_gold(),
            k.get_gold() + k.get_bank_gold()
        )
        .as_str(),
    );
    send_to_char(descs, 
        ch,
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
    buf.clear();
    sprinttype(k.get_pos() as i32, &POSITION_TYPES, &mut buf);
    send_to_char(descs, 
        ch,
        format!(
            "Pos: {}, Fighting: {}",
            buf,
            if k.fighting_id().is_some() {
                chars.get(k.fighting_id().unwrap()).get_name().as_ref()
            } else {
                "Nobody"
            }
        )
        .as_str(),
    );
    if k.is_npc() {
        send_to_char(descs, 
            ch,
            format!(
                ", Attack type: {}",
                &ATTACK_HIT_TEXT[k.mob_specials.attack_type as usize].singular
            )
            .as_str(),
        );
    }
    if k.desc.is_some() {
        buf.clear();
        sprinttype(
            descs.get(k.desc.unwrap()).state() as i32,
            &CONNECTED_TYPES,
            &mut buf,
        );
        send_to_char(descs, ch, format!(", Connected: {}", buf).as_str());
    }
    if k.is_npc() {
        buf.clear();
        sprinttype(k.mob_specials.default_pos as i32, &POSITION_TYPES, &mut buf);
        send_to_char(descs, ch, format!(", Default position: {}\r\n", buf).as_str());
        buf.clear();
        sprintbit(k.mob_flags(), &ACTION_BITS, &mut buf);
        send_to_char(descs, 
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
        send_to_char(descs, 
            ch,
            format!(", Idle Timer (in tics) [{}]\r\n", k.char_specials.timer).as_str(),
        );
        buf.clear();
        sprintbit(k.plr_flags(), &PLAYER_BITS, &mut buf);
        send_to_char(descs, 
            ch,
            format!("PLR: {}{}{}\r\n", CCCYN!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
        );
        buf.clear();
        sprintbit(k.prf_flags().bits(), &PREFERENCE_BITS, &mut buf);
        send_to_char(descs, 
            ch,
            format!("PRF: {}{}{}\r\n", CCGRN!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
        );
    }
    if db.is_mob(k) {
        send_to_char(descs, 
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
    send_to_char(descs, 
        ch,
        format!(
            "Carried: weight: {}, items: {}; Items in: inventory: {}, ",
            k.is_carrying_w(),
            k.is_carrying_n(),
            k.carrying.len()
        )
        .as_str(),
    );
    let mut i2 = 0;
    for i in 0..NUM_WEARS {
        if k.get_eq(i).is_some() {
            i2 += 1;
        }
    }

    send_to_char(descs, ch, format!("eq: {}\r\n", i2).as_str());
    if !k.is_npc() {
        send_to_char(descs, 
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
    let mut column = send_to_char(descs, 
        ch,
        format!(
            "Master is: {}, Followers are:",
            if k.master.is_some() {
                chars.get(k.master.unwrap()).get_name().as_ref()
            } else {
                "<none>"
            }
        )
        .as_str(),
    );
    if k.followers.is_empty() {
        send_to_char(descs, ch, " <none>\r\n");
    } else {
        let mut found = 0;
        for (i, fol) in k.followers.iter().enumerate() {
            let msg = format!(
                "{} {}",
                if found != 0 { "," } else { "" },
                pers(descs, chars, db, chars.get(fol.follower), ch)
            );
            column += send_to_char(descs, 
                ch,
                msg.as_str(),
            );
            found += 1;
            if column >= 62 {
                let msg = format!("{}\r\n", if i < k.followers.len() - 1 { "," } else { "" });
                send_to_char(descs, ch, msg.as_str());
                found = 0;
                column = 0;
            }
        }
        if column != 0 {
            send_to_char(descs, ch, "\r\n");
        }
    }

    /* Showing the bitvector */
    buf.clear();
    sprintbit(k.aff_flags().bits(), &AFFECTED_BITS, &mut buf);
    send_to_char(descs, 
        ch,
        format!("AFF: {}{}{}\r\n", CCYEL!(ch, C_NRM), buf, CCNRM!(ch, C_NRM)).as_str(),
    );

    /* Routine to show what spells a char is affected by */
    if !k.affected.is_empty() {
        for aff in &k.affected {
            send_to_char(descs, 
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
                send_to_char(descs, 
                    ch,
                    format!(
                        "{} to {}",
                        aff.modifier, &APPLY_TYPES[aff.location as usize]
                    )
                    .as_str(),
                );
            }
            let bitvector = aff.bitvector;
            if !bitvector.is_empty() {
                if aff.modifier != 0 {
                    send_to_char(descs, ch, ", ");
                }
                buf.clear();
                sprintbit(bitvector.bits(), &AFFECTED_BITS, &mut buf);
                send_to_char(descs, ch, format!("sets {}", buf).as_str());
            }
            send_to_char(descs, ch, "\r\n");
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_stat(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut buf1 = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut buf1, &mut buf2);

    if buf1.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Stats on who or what?\r\n");
    } else if is_abbrev(&buf1, "room") {
        do_stat_room(&mut game.descriptors,db,chars,   objs, chid);
    } else if is_abbrev(&buf1, "mob") {
        if buf2.is_empty() {
            send_to_char(&mut game.descriptors, ch, "Stats on which mobile?\r\n");
        } else {
            let victim;
            let res = {
                victim = get_char_vis(&game.descriptors, chars,db, ch, &mut buf2, None, FindFlags::CHAR_WORLD);
                victim.is_some()
            }; 
            if res {
                do_stat_character(&mut game.descriptors, db,chars,  ch, victim.unwrap());
            } else {
                send_to_char(&mut game.descriptors, ch, "No such mobile around.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "player") {
        if buf2.is_empty() {
            send_to_char(&mut game.descriptors, ch, "Stats on which player?\r\n");
        } else {
            let victim;
            let res = {
                victim = get_player_vis(&game.descriptors, chars,db, ch, &mut buf2, None, FindFlags::CHAR_WORLD);
                victim.is_some()
            }; 
            if res {
                do_stat_character(&mut game.descriptors, db,chars,  ch, victim.unwrap());
            } else {
                send_to_char(&mut game.descriptors, ch, "No such player around.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "file") {
        let victim;
        #[allow(clippy::blocks_in_conditions)]
        if buf2.is_empty() {
            send_to_char(&mut game.descriptors, ch, "Stats on which player?\r\n");
        } else if {
            victim = get_player_vis(&game.descriptors, chars,db, ch, &mut buf2, None, FindFlags::CHAR_WORLD);
            victim.is_some()
        } {
            do_stat_character(&mut game.descriptors, db,chars,  ch, victim.unwrap());
        } else {
            let mut loaded_victim = CharData::default();
            let mut tmp_store = CharFileU::default();
            clear_char(&mut loaded_victim);
            if db.load_char(&buf2, &mut tmp_store).is_some() {
                store_to_char(texts, &tmp_store, &mut loaded_victim);
                loaded_victim.player.time.logon = tmp_store.last_logon;
                let loaded_victim_id = chars.push(loaded_victim);
                db.char_to_room(chars, objs,loaded_victim_id, 0);
                let ch = chars.get(chid);
                let loaded_victim = chars.get(loaded_victim_id);
                if loaded_victim.get_level() > ch.get_level() {
                    send_to_char(&mut game.descriptors, ch, "Sorry, you can't do that.\r\n");
                } else {
                    do_stat_character(&mut game.descriptors, db,chars,  ch, loaded_victim);
                }
                game.extract_char_final(chars, db, texts, objs,loaded_victim_id);
            } else {
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, ch, "There is no such player.\r\n");
            }
        }
    } else if is_abbrev(&buf1, "object") {
        if buf2.is_empty() {
            send_to_char(&mut game.descriptors, ch, "Stats on which object?\r\n");
        } else {
            let obj;
            let res = {
                obj = get_obj_vis(&game.descriptors, chars,db, objs,ch, &buf2, None);
                obj.is_some()
            }; 
            if res {
                do_stat_object(&mut game.descriptors, db,chars,  objs,ch, obj.unwrap());
            } else {
                send_to_char(&mut game.descriptors, ch, "No such object around.\r\n");
            }
        }
    } else {
        let mut name = buf1;
        let mut number = get_number(&mut name);
        let mut obj;
        let mut victim;
        #[allow(clippy::blocks_in_conditions)]
        #[allow(clippy::if_same_then_else)]
        if {
            obj = get_obj_in_equip_vis(&game.descriptors, chars,db, objs,ch, &name, Some(&mut number), &ch.equipment);
            obj.is_some()
        } {
            do_stat_object(&mut game.descriptors, db,chars,objs,  ch, obj.unwrap());
        } else if {
            obj = get_obj_in_list_vis(&game.descriptors, chars,db, objs,ch, &name, Some(&mut number), &ch.carrying);
            obj.is_some()
        } {
            do_stat_object(&mut game.descriptors, db,chars, objs, ch, obj.unwrap());
        } else if {
            victim = get_char_vis(&game.descriptors, chars,db, ch, &mut name, Some(&mut number), FindFlags::CHAR_ROOM);
            victim.is_some()
        } {
            do_stat_character(&mut game.descriptors, db,chars,  ch, victim.unwrap());
        } else if {
            obj = get_obj_in_list_vis2(&game.descriptors, chars,
                db,objs,
                ch,
                &name,
                Some(&mut number),
                &db.world[ch.in_room() as usize].contents,
            );
            obj.is_some()
        } {
            do_stat_object(&mut game.descriptors, db,chars, objs, ch, obj.unwrap());
        } else if {
            victim = get_char_vis(&game.descriptors, chars,db, ch, &mut name, Some(&mut number), FindFlags::CHAR_WORLD);
            victim.is_some()
        } {
            do_stat_character(&mut game.descriptors, db,chars,  ch, victim.unwrap());
        } else if {
            obj = get_obj_vis(&game.descriptors, chars,db,objs, ch, &name, Some(&mut number));
            obj.is_some()
        } {
            do_stat_object(&mut game.descriptors, db,chars,objs,  ch, obj.unwrap());
        } else {
            send_to_char(&mut game.descriptors, ch, "Nothing around by that name.\r\n");
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_shutdown(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut  Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();
    if subcmd != SCMD_SHUTDOWN {
        send_to_char(&mut game.descriptors, ch, "If you want to shut something down, say so!\r\n");
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
        send_to_char(&mut game.descriptors, ch, "Unknown shutdown option.\r\n");
    }
}

pub fn snoop_check(game: &mut Game, chars: &Depot<CharData>, chid: DepotId) {
    let ch = chars.get(chid);
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
        && chars.get(game
                .desc(game.desc(d_id).snooping.unwrap())
                .character
                .unwrap())
            .get_level()
            >= ch.get_level()
    {
        game.desc_mut(game.desc(d_id).snooping.unwrap()).snoop_by = None;
        game.desc_mut(d_id).snooping = None;
    }
    let ch = chars.get(chid);
    if game.desc(d_id).snoop_by.is_some()
        && ch.get_level()
            >= chars.get(game
                    .desc(game.desc(d_id).snoop_by.unwrap())
                    .character
                    .unwrap())
                .get_level()
    {
        game.desc_mut(game.desc(d_id).snoop_by.unwrap()).snooping = None;
        game.desc_mut(d_id).snoop_by = None;
    }
}

fn stop_snooping(game: &mut Game,chars: &mut Depot<CharData>, chid: DepotId) {
    let ch = chars.get(chid);

    if game.desc(ch.desc.unwrap()).snooping.is_none() {
        send_to_char(&mut game.descriptors, ch, "You aren't snooping anyone.\r\n");
    } else {
        send_to_char(&mut game.descriptors, ch, "You stop snooping.\r\n");
        let ch = chars.get(chid);
        let desc_id = game.desc(ch.desc.unwrap()).snooping.unwrap();
        game.desc_mut(desc_id).snoop_by = None;
        let ch = chars.get(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).snooping = None;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_snoop(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();

    if ch.desc.is_none() {
        return;
    }

    one_argument(argument, &mut arg);
    let victim;
    let voriginal_id;
    let tch;
    #[allow(clippy::blocks_in_conditions)]
    if arg.is_empty() {
        stop_snooping(game, chars, chid);
    } else if {
        victim = get_char_vis(&game.descriptors, chars,db, ch, &mut arg, None, FindFlags::CHAR_WORLD);
        victim.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, "No such person around.\r\n");
    } else if victim.as_ref().unwrap().desc.is_none() {
        send_to_char(&mut game.descriptors, ch, "There's no link.. nothing to snoop.\r\n");
    } else if victim.unwrap().id() == chid {
        stop_snooping(game, chars, chid);
    } else if game
        .desc(victim.as_ref().unwrap().desc.unwrap())
        .snoop_by
        .is_some()
    {
        send_to_char(&mut game.descriptors, ch, "Busy already. \r\n");
    } else if game
        .desc(victim.as_ref().unwrap().desc.unwrap())
        .snooping
        .unwrap()
        == ch.desc.unwrap()
    {
        send_to_char(&mut game.descriptors, ch, "Don't be stupid.\r\n");
    } else {
        if game
            .desc(victim.as_ref().unwrap().desc.unwrap())
            .original
            .is_some()
        {
            voriginal_id = game.desc(victim.as_ref().unwrap().desc.unwrap()).original;
            tch = voriginal_id.map(|v| chars.get(v));
        } else {
            tch = victim;
        }
        if tch.as_ref().unwrap().get_level() >= ch.get_level() {
            send_to_char(&mut game.descriptors, ch, "You can't.\r\n");
            return;
        }
        send_to_char(&mut game.descriptors, ch, OK);
        if game.desc(ch.desc.unwrap()).snooping.is_some() {
            let desc_id = ch.desc.unwrap();
            let snooping_desc_id = game.desc(desc_id).snooping.unwrap();
            game.desc_mut(snooping_desc_id).snoop_by = None;
        }
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).snooping = victim.as_ref().unwrap().desc;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_switch(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let victim;
    #[allow(clippy::blocks_in_conditions)]
    if game.desc(ch.desc.unwrap()).original.is_some() {
        send_to_char(&mut game.descriptors, ch, "You're already switched.\r\n");
    } else if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Switch with who?\r\n");
    } else if {
        victim = get_char_vis(&game.descriptors, chars,db, ch, &mut arg, None, FindFlags::CHAR_WORLD);
        victim.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, "No such character.\r\n");
    } else if chid == victim.unwrap().id() {
        send_to_char(&mut game.descriptors, ch, "Hee hee... we are jolly funny today, eh?\r\n");
    } else if victim.as_ref().unwrap().desc.is_some() {
        send_to_char(&mut game.descriptors, ch, "You can't do that, the body is already in use!\r\n");
    } else if ch.get_level() < LVL_IMPL as u8 && !victim.as_ref().unwrap().is_npc() {
        send_to_char(&mut game.descriptors, ch, "You aren't holy enough to use a mortal's body.\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8
        && db.room_flagged(victim.as_ref().unwrap().in_room(), RoomFlags::GODROOM)
    {
        send_to_char(&mut game.descriptors, ch, "You are not godly enough to use that room!\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8
        && db.room_flagged(victim.as_ref().unwrap().in_room(), RoomFlags::HOUSE)
        && !house_can_enter(
            db,
            ch,
            db.get_room_vnum(victim.as_ref().unwrap().in_room()),
        )
    {
        send_to_char(&mut game.descriptors, ch, "That's private property -- no trespassing!\r\n");
    } else {
        send_to_char(&mut game.descriptors, ch, OK);
        let ch = chars.get(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).character = victim.map(|c| c.id());
        let ch = chars.get(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).original = Some(chid);
        let ch = chars.get(chid);
        let val = ch.desc;
        let victim_id = victim.as_ref().unwrap().id();
        let victim =chars.get_mut(victim_id);
        victim.desc = val;
        let ch = chars.get_mut(chid);
        ch.desc = None;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_return(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut  Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    if ch.desc.is_some() && game.desc(ch.desc.unwrap()).original.is_some() {
        send_to_char(&mut game.descriptors, ch, "You return to your original body.\r\n");
        let ch = chars.get(chid);
        /*
         * If someone switched into your original body, disconnect them.
         *   - JE 2/22/95
         *
         * Zmey: here we put someone switched in our body to disconnect state
         * but we must also None his pointer to our character, otherwise
         * close_socket() will damage our character's pointer to our descriptor
         * (which is assigned below in this function). 12/17/99
         */
        if chars.get(game.desc(ch.desc.unwrap()).original.unwrap())
            .desc
            .borrow()
            .is_some()
        {
            let dorig_id = chars.get(game.desc(ch.desc.unwrap()).original.unwrap())
                .desc
                .unwrap();
            game.desc_mut(dorig_id).character = None;
            game.desc_mut(dorig_id).set_state(ConDisconnect);
        }

        /* Now our descriptor points to our original body. */
        let ch = chars.get(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).character = game.desc(desc_id).original;
        let ch = chars.get(chid);
        let desc_id = ch.desc.unwrap();
        game.desc_mut(desc_id).original = None;

        /* And our body's pointer to descriptor now points to our descriptor. */
        let ch = chars.get(chid);

        chars.get_mut(game.desc(ch.desc.unwrap()).character.unwrap())
            .desc = ch.desc;
        let ch = chars.get_mut(chid);
        ch.desc = None;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_load(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !buf2.chars().next().unwrap().is_ascii_digit() {
        send_to_char(&mut game.descriptors, ch, "Usage: load { obj | mob } <number>\r\n");
        return;
    }
    if !is_number(&buf2) {
        send_to_char(&mut game.descriptors, ch, "That is not a number.\r\n");
        return;
    }

    if is_abbrev(&buf, "mob") {
        let r_num;

        let res = {
            r_num = db.real_mobile(buf2.parse::<i16>().unwrap());
            r_num == NOBODY
        }; 
        if res {
            send_to_char(&mut game.descriptors, ch, "There is no monster with that number.\r\n");
            return;
        }
        let mob_id = db.read_mobile(chars,r_num, LoadType::Real).unwrap();
        let ch = chars.get(chid);
        db.char_to_room(chars, objs,mob_id, ch.in_room());
        let mob = chars.get(mob_id);
        let ch = chars.get(chid);
        act(&mut game.descriptors, chars, 
            db,
            "$n makes a quaint, magical gesture with one hand.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        act(&mut game.descriptors, chars, 
            db,
            "$n has created $N!",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(mob)),
            TO_ROOM,
        );
        act(&mut game.descriptors, chars, 
            db,
            "You create $N.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(mob)),
            TO_CHAR,
        );
    } else if is_abbrev(&buf, "obj") {
        let r_num;

        let res = {
            r_num = db.real_object(buf2.parse::<i16>().unwrap());
            r_num == NOTHING
        }; 
        if res {
            send_to_char(&mut game.descriptors, ch, "There is no object with that number.\r\n");
            return;
        }
        let oid = db.read_object(objs,r_num, LoadType::Real).unwrap();
        let obj = objs.get_mut(oid);
        let ch = chars.get_mut(chid);
        if LOAD_INTO_INVENTORY {
            obj_to_char(obj, ch);
        } else {
            let ch = chars.get(chid);
            db.obj_to_room(obj, ch.in_room());
        }
        let ch = chars.get(chid);
        act(&mut game.descriptors, chars, 
            db,
            "$n makes a strange magical gesture.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        act(&mut game.descriptors, chars, 
            db,
            "$n has created $p!",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_ROOM,
        );
        act(&mut game.descriptors, chars, 
            db,
            "You create $p.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
    } else {
        send_to_char(&mut game.descriptors, ch, "That'll have to be either 'obj' or 'mob'.\r\n");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_vstat(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf = String::new();
    let mut buf2 = String::new();

    two_arguments(argument, &mut buf, &mut buf2);

    if buf.is_empty() || buf2.is_empty() || !buf2.chars().next().unwrap().is_ascii_digit() {
        send_to_char(&mut game.descriptors, ch, "Usage: vstat { obj | mob } <number>\r\n");
        return;
    }
    if !is_number(&buf2) {
        send_to_char(&mut game.descriptors, ch, "That's not a valid number.\r\n");
        return;
    }

    if is_abbrev(&buf, "mob") {
        let r_num;

        let res = {
            r_num = db.real_mobile(buf2.parse::<i16>().unwrap());
            r_num == NOBODY
        }; 
        if res {
            send_to_char(&mut game.descriptors, ch, "There is no monster with that number.\r\n");
            return;
        }
        let mob_id = db.read_mobile(chars,r_num, LoadType::Real);
        db.char_to_room(chars, objs,mob_id.unwrap(), 0);
        let mob = chars.get(mob_id.unwrap());
        let ch = chars.get(chid);
        do_stat_character(&mut game.descriptors, db,chars, ch, mob);
        db.extract_char(chars, mob_id.unwrap());
    } else if is_abbrev(&buf, "obj") {
        let r_num;

        let res = {
            r_num = db.real_object(buf2.parse::<i16>().unwrap());
            r_num == NOTHING
        }; 
        if res {
            send_to_char(&mut game.descriptors, ch, "There is no object with that number.\r\n");
            return;
        }
        let oid = db.read_object(objs,r_num, LoadType::Real);
        let obj = objs.get(oid.unwrap());
        let ch = chars.get(chid);
        do_stat_object(&mut game.descriptors, db,chars, objs,ch, obj);
        db.extract_obj( chars, objs,oid.unwrap());
    } else {
        send_to_char(&mut game.descriptors, ch, "That'll have to be either 'obj' or 'mob'.\r\n");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_purge(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    /* clean a room of all mobiles and objects */
    let mut buf = String::new();
    one_argument(argument, &mut buf);
    let vict;
    let obj;
    /* argument supplied. destroy single object or char */
    if !buf.is_empty() {
        let res = {
            vict = get_char_vis(&game.descriptors, chars,db, ch, &mut buf, None, FindFlags::CHAR_ROOM);
            vict.is_some()
        }; 
        #[allow(clippy::blocks_in_conditions)]
        if res {
            if !vict.unwrap().is_npc()
                && ch.get_level() <= vict.unwrap().get_level()
            {
                send_to_char(&mut game.descriptors, ch, "Fuuuuuuuuu!\r\n");
                return;
            }
            let vict = vict.unwrap();
            act(&mut game.descriptors, chars, 
                db,
                "$n disintegrates $N.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_NOTVICT,
            );
            let vict_id = vict.id();
            if !vict.is_npc() {
                let ch = chars.get(chid);
                game.mudlog(chars,
                    DisplayMode::Brief,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} has purged {}.", ch.get_name(), vict.get_name()).as_str(),
                );
                if vict.desc.is_some() {
                    let desc_id = vict.desc.unwrap();
                    game.desc_mut(desc_id).set_state(ConClose);
                    let desc_id = vict.desc.unwrap();
                    game.desc_mut(desc_id).character = None;
                    let vict = chars.get_mut(vict_id);
                    vict.desc = None;
                }
            }
            db.extract_char(chars, vict_id);
        } else if {
            obj = get_obj_in_list_vis2(&game.descriptors, chars,
                db,objs,
                ch,
                &buf,
                None,
                &db.world[ch.in_room() as usize].contents,
            );
            obj.is_some()
        } {
            let obj = obj.unwrap();
            let oid = obj.id();
            act(&mut game.descriptors, chars, 
                db,
                "$n destroys $p.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_ROOM,
            );
            db.extract_obj( chars, objs,oid);
        } else {
            send_to_char(&mut game.descriptors, ch, "Nothing here by that name.\r\n");
            return;
        }
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, OK);
    } else {
        /* no argument. clean out the room */

        act(&mut game.descriptors, chars, 
            db,
            "$n gestures... You are surrounded by scorching flames!",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let ch = chars.get(chid);
        send_to_room(&mut game.descriptors, chars, db, ch.in_room(), "The world seems a little cleaner.\r\n");
        let ch = chars.get(chid);
        for vict_id in db.world[ch.in_room() as usize].peoples.clone() {
            let vict = chars.get(vict_id);
            if !vict.is_npc() {
                continue;
            }

            /* Dump inventory. */
            while {
                let vict = chars.get(vict_id);
                !vict.carrying.is_empty()
            } {
                let vict = chars.get(vict_id);
                let oid = vict.carrying[0];
                db.extract_obj( chars, objs,oid);
            }

            /* Dump equipment. */
            for i in 0..NUM_WEARS {
                let vict = chars.get(vict_id);
                if vict.get_eq(i).is_some() {
                    let oid = vict.get_eq(i).unwrap();
                    db.extract_obj( chars, objs,oid)
                }
            }

            /* Dump character. */
            db.extract_char(chars, vict_id);
        }

        /* Clear the ground. */
        let ch = chars.get(chid);
        let ch_in_room = ch.in_room();
        loop {
            if db.world[ch_in_room as usize].contents.is_empty() {
                break;
            }
            let oid = db.world[ch_in_room as usize].contents[0];
            db.extract_obj( chars, objs, oid);
        }
    }
}

const LOGTYPES: [&str; 5] = ["off", "brief", "normal", "complete", "\n"];

#[allow(clippy::too_many_arguments)]
pub fn do_syslog(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "Your syslog is currently {}.\r\n",
                LOGTYPES[if ch.prf_flagged(PrefFlags::LOG1) { 1 } else { 0 }
                    + if ch.prf_flagged(PrefFlags::LOG2) { 2 } else { 0 }]
            )
            .as_str(),
        );
        return;
    }
    let tp;
    let res = {
        tp = search_block(&arg, &LOGTYPES, false);
        tp.is_none()
    }; 
    if res {
        send_to_char(&mut game.descriptors, ch, "Usage: syslog { Off | Brief | Normal | Complete }\r\n");
        return;
    }
    let tp = tp.unwrap();
    let ch = chars.get_mut(chid);
    ch.remove_prf_flags_bits(PrefFlags::LOG1 | PrefFlags::LOG2);
    if (tp & 1) != 0 {
        ch.set_prf_flags_bits(PrefFlags::LOG1);
    }
    if (tp & 2) != 0 {
        ch.set_prf_flags_bits(PrefFlags::LOG2);
    }

    send_to_char(&mut game.descriptors, 
        ch,
        format!("Your syslog is now {}.\r\n", &LOGTYPES[tp]).as_str(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn do_advance(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut name = String::new();
    let mut level = String::new();
    let victim;
    two_arguments(argument, &mut name, &mut level);

    if !name.is_empty() {
        let res = {
            victim = get_char_vis(&game.descriptors, chars,db, ch, &mut name, None, FindFlags::CHAR_WORLD);
            victim.is_none()
        }; 
        if res {
            send_to_char(&mut game.descriptors, ch, "That player is not here.\r\n");
            return;
        }
    } else {
        send_to_char(&mut game.descriptors, ch, "Advance who?\r\n");
        return;
    }
    let victim =victim.unwrap();
    let victim_id = victim.id();

    if ch.get_level() <= victim.get_level() {
        send_to_char(&mut game.descriptors, ch, "Maybe that's not such a great idea.\r\n");
        return;
    }
    if victim.is_npc() {
        send_to_char(&mut game.descriptors, ch, "NO!  Not on NPC's.\r\n");
        return;
    }
    let r = level.parse::<u8>();
    let mut newlevel = 255;
    if r.is_err() || {
        newlevel = r.unwrap();
        newlevel == 0
    } {
        send_to_char(&mut game.descriptors, ch, "That's not a level!\r\n");
        return;
    }

    if newlevel > LVL_IMPL as u8 {
        send_to_char(&mut game.descriptors, 
            ch,
            format!("{} is the highest possible level.\r\n", LVL_IMPL).as_str(),
        );
        return;
    }
    if newlevel > ch.get_level() {
        send_to_char(&mut game.descriptors, ch, "Yeah, right.\r\n");
        return;
    }
    if newlevel == victim.get_level() {
        send_to_char(&mut game.descriptors, ch, "They are already at that level.\r\n");
        return;
    }
    let oldlevel = victim.get_level();
    if newlevel < oldlevel {
        do_start(game, chars, db,texts, objs, victim_id);
        let victim = chars.get_mut(victim_id);
        victim.set_level(newlevel);
        let victim = chars.get(victim_id);
        send_to_char(&mut game.descriptors, 
            victim,
            "You are momentarily enveloped by darkness!\r\nYou feel somewhat diminished.\r\n",
        );
    } else {
        let victim = chars.get(victim_id);
        act(&mut game.descriptors, chars, 
            db,
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
            Some(VictimRef::Char(victim)),
            TO_VICT,
        );
    }
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch, OK);
    let ch = chars.get(chid);
    if newlevel < oldlevel {
        let victim = chars.get(victim_id);
        info!(
            "(GC) {} demoted {} from level {} to {}.",
            ch.get_name(),
            victim.get_name(),
            oldlevel,
            newlevel
        );
    } else {
        let victim = chars.get(victim_id);
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
        let victim = chars.get_mut(victim_id);
        victim.remove_prf_flags_bits(PrefFlags::LOG1 | PrefFlags::LOG2);
        victim.remove_prf_flags_bits(PrefFlags::NOHASSLE | PrefFlags::HOLYLIGHT);

        // TODO run_autowiz();
    }
    let victim = chars.get(victim_id);
    gain_exp_regardless(
        game,chars,
        db,
        victim_id,
        level_exp(victim.get_class(), newlevel as i16) - victim.get_exp(),texts,objs,
    );
    save_char(&mut game.descriptors, db,chars, texts,objs,victim_id);
}

#[allow(clippy::too_many_arguments)]
pub fn do_restore(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let vict;
    #[allow(clippy::blocks_in_conditions)]
    if buf.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Whom do you wish to restore?\r\n");
    } else if {
        vict = get_char_vis(&game.descriptors, chars,db, ch, &mut buf, None, FindFlags::CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, NOPERSON);
    } else if !vict.unwrap().is_npc()
        && chid != vict.unwrap().id()
        && vict.unwrap().get_level() >= ch.get_level()
    {
        send_to_char(&mut game.descriptors, ch, "They don't need your help.\r\n");
    } else {
        let vict_id = vict.unwrap().id();
        let vict = chars.get_mut(vict_id);
        vict.set_hit(vict.get_max_hit());
        vict.set_mana(vict.get_max_mana());
        vict.set_move(vict.get_move());
        let ch = chars.get(chid);
        let vict = chars.get(vict_id);
        if !vict.is_npc() && ch.get_level() >= LVL_GRGOD as u8 {
            if vict.get_level() >= LVL_IMMORT as u8 {
                for i in 1..MAX_SKILLS + 1 {
                    let vict = chars.get_mut(vict_id);
                    vict.set_skill(i as i32, 100);
                }
            }

            let vict = chars.get(vict_id);
            if vict.get_level() >= LVL_GRGOD as u8 {
                let vict = chars.get_mut(vict_id);
                vict.real_abils.str_add = 100;
                vict.real_abils.intel = 25;
                vict.real_abils.wis = 25;
                vict.real_abils.dex = 25;
                vict.real_abils.str = 25;
                vict.real_abils.con = 25;
                vict.real_abils.cha = 25;
            }
        }
        let vict = chars.get_mut(vict_id);
        update_pos(vict);
        affect_total( objs,vict);
        let vict = chars.get(vict_id);
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, OK);
        act(&mut game.descriptors, chars, 
            db,
            "You have been fully healed by $N!",
            false,
            Some(vict),
            None,
            Some(VictimRef::Char(ch)),
            TO_CHAR,
        );
    }
}

pub fn perform_immort_vis(descs: &mut Depot<DescriptorData>, db: &mut DB,chars: &mut Depot<CharData>,objs: &mut Depot<ObjData>,  chid: DepotId) {
    let ch = chars.get(chid);
    if ch.get_invis_lev() == 0 && !ch.aff_flagged(AffectFlags::HIDE | AffectFlags::INVISIBLE) {
        send_to_char(descs, ch, "You are already fully visible.\r\n");
        return;
    }
    let ch = chars.get_mut(chid);

    ch.set_invis_lev(0);

    appear(descs, chars, db,objs, chid);
    let ch = chars.get(chid);
    send_to_char(descs, ch, "You are now fully visible.\r\n");
}

fn perform_immort_invis(descs: &mut Depot<DescriptorData>, db: &mut DB,chars: &mut Depot<CharData>, chid: DepotId, level: i32) {
    let ch = chars.get(chid);

    for &tch_id in &db.world[ch.in_room() as usize].peoples {
        let tch = chars.get(tch_id);
        if tch_id == chid {
            continue;
        }
        let ch = chars.get(chid);
        if tch.get_level() >= ch.get_invis_lev() as u8 && tch.get_level() < level as u8 {
            act(descs, chars, 
                db,
                "You blink and suddenly realize that $n is gone.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(tch)),
                TO_VICT,
            );
        }
        let tch = chars.get(tch_id);
        let ch = chars.get(chid);
        if tch.get_level() < ch.get_invis_lev() as u8 && tch.get_level() >= level as u8 {
            act(descs, chars, 
                db,
                "You suddenly realize that $n is standing beside you.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(tch)),
                TO_VICT,
            );
        }
    }
    let ch = chars.get_mut(chid);
    ch.set_invis_lev(level as i16);
    send_to_char(descs, 
        ch,
        format!("Your invisibility level is {}.\r\n", level).as_str(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn do_invis(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();

    if ch.is_npc() {
        send_to_char(&mut game.descriptors, ch, "You can't do that!\r\n");
        return;
    }

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        if ch.get_invis_lev() > 0 {
            perform_immort_vis(&mut game.descriptors, db,chars,objs, chid);
        } else {
            perform_immort_invis(&mut game.descriptors, db,chars, chid, ch.get_level() as i32);
        }
    } else {
        let level = arg.parse::<i32>();
        let level = level.unwrap_or_default();
        if level > ch.get_level() as i32 {
            send_to_char(&mut game.descriptors, ch, "You can't go invisible above your own level.\r\n");
        } else if level < 1 {
            perform_immort_vis(&mut game.descriptors, db,chars,objs, chid);
        } else {
            perform_immort_invis(&mut game.descriptors, db,chars, chid, level);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_gecho(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "That must be a mistake...\r\n");
    } else {
        for pt_id in game.descriptor_list.clone() {
            let pt = game.desc(pt_id);
            if pt.state() == ConPlaying
                && pt.character.is_some()
                && pt.character.unwrap() != chid
            {
                let chid = pt.character.unwrap();
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, ch, format!("{}\r\n", argument).as_str());
            }
        }
        let ch = chars.get(chid);
        if ch.prf_flagged(PrefFlags::NOREPEAT) {
            send_to_char(&mut game.descriptors, ch, OK);
        } else {
            send_to_char(&mut game.descriptors, ch, format!("{}\r\n", argument).as_str());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_poofset(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get_mut(chid);
    {

        let cps = &mut ch.player_specials;
        let msg=
        match subcmd {
            SCMD_POOFIN => {
                &mut cps.poofin
            }
            SCMD_POOFOUT => {
                &mut cps.poofout
            }
            _ => {
                return;
            }
        };

        let argument = argument.trim_start();

        *msg = Rc::from(argument);
    }
    send_to_char(&mut game.descriptors, ch, OK);
}

#[allow(clippy::too_many_arguments)]
pub fn do_dc(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let num_to_dc = arg.parse::<u32>();
    if num_to_dc.is_err() {
        send_to_char(&mut game.descriptors, ch, "Usage: DC <user number> (type USERS for a list)\r\n");
        return;
    }
    let num_to_dc = num_to_dc.unwrap();
    let mut d_id = None;
    {
        for &cd_id in &game.descriptor_list {
            if game.desc(cd_id).desc_num == num_to_dc as usize {
                d_id = Some(cd_id);
            }
        }
    }

    if d_id.is_none() {
        send_to_char(&mut game.descriptors, ch, "No such connection.\r\n");
        return;
    }
    let d_id = d_id.unwrap();
    if game.desc(d_id).character.is_some()
        && chars.get(game.desc(d_id).character.unwrap()).get_level() >= ch.get_level()
    {
        if !can_see(&game.descriptors, chars, db, ch, chars.get(game.desc(d_id).character.unwrap())) {
            send_to_char(&mut game.descriptors, ch, "No such connection.\r\n");
        } else {
            send_to_char(&mut game.descriptors, ch, "Umm.. maybe that's not such a good idea...\r\n");
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
        send_to_char(&mut game.descriptors, ch, "They're already being disconnected.\r\n");
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
        send_to_char(&mut game.descriptors, 
            ch,
            format!("Connection #{} closed.\r\n", num_to_dc).as_str(),
        );
        let ch = chars.get(chid);
        info!("(GC) Connection closed by {}.", ch.get_name());
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_wizlock(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();
    let value;
    one_argument(argument, &mut arg);
    let when;
    if !arg.is_empty() {
        value = arg.parse::<i32>();
        let value = value.unwrap_or(-1);
        if value < 0 || value > ch.get_level() as i32 {
            send_to_char(&mut game.descriptors, ch, "Invalid wizlock value.\r\n");
            return;
        }
        db.circle_restrict = value as u8;
        when = "now";
    } else {
        when = "currently";
    }
    let ch = chars.get(chid);
    match db.circle_restrict {
        0 => {
            send_to_char(&mut game.descriptors, 
                ch,
                format!("The game is {} completely open.\r\n", when).as_str(),
            );
        }
        1 => {
            send_to_char(&mut game.descriptors, 
                ch,
                format!("The game is {} closed to new players.\r\n", when).as_str(),
            );
        }
        _ => {
            send_to_char(&mut game.descriptors, 
                ch,
                format!(
                    "Only level {} and above may enter the game {}.\r\n",
                    db.circle_restrict, when
                )
                .as_str(),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_date(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let mytime=
    if subcmd == SCMD_DATE {
        time_now()
    } else {
        db.boot_time
    };

    let date_time = Utc.timestamp_millis_opt(mytime as i64 * 1000).unwrap();
    let tmstr = date_time.to_rfc2822();

    if subcmd == SCMD_DATE {
        send_to_char(&mut game.descriptors, ch, format!("Current machine time: {}\r\n", tmstr).as_str());
    } else {
        let mytime = time_now() - db.boot_time;
        let d = mytime / 86400;
        let h = (mytime / 3600) % 24;
        let m = (mytime / 60) % 60;

        send_to_char(&mut game.descriptors, 
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

#[allow(clippy::too_many_arguments)]
pub fn do_last(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let mut arg = String::new();
    let ch = chars.get(chid);

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "For whom do you wish to search?\r\n");
        return;
    }
    let mut chdata = CharFileU::default();
    if db.load_char(&arg, &mut chdata).is_none() {
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, "There is no such player.\r\n");
        return;
    }
    let ch = chars.get(chid);
    if chdata.level > ch.get_level() && ch.get_level() < LVL_IMPL as u8 {
        send_to_char(&mut game.descriptors, ch, "You are not sufficiently godly for that!\r\n");
        return;
    }
    let id = chdata.char_specials_saved.idnum;
    send_to_char(&mut game.descriptors, 
        ch,
        format!(
            "[{:5}] [{:2} {}] {:12} : {:-18} : {:20}\r\n",
            id,
            chdata.level,
            CLASS_ABBREVS[chdata.chclass as usize],
            parse_c_string(&chdata.name),
            parse_c_string(&chdata.host),
            ctime(chdata.last_logon)
        )
        .as_str(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn do_force(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut argument = argument.to_string();
    let mut arg = String::new();
    let mut to_force = String::new();

    half_chop(&mut argument, &mut arg, &mut to_force);

    let buf1 = format!("$n has forced you to '{}'.", to_force);
    let vict;
    if arg.is_empty() || to_force.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Whom do you wish to force do what?\r\n");
    } else if ch.get_level() < LVL_GRGOD as u8 || "all" != arg && "room" != arg {
        let res = {
            vict = get_char_vis(&game.descriptors, chars,db, ch, &mut arg, None, FindFlags::CHAR_WORLD);
            vict.is_none()
        }; 
        if res {
            send_to_char(&mut game.descriptors, ch, NOPERSON);
        } else if !vict.unwrap().is_npc()
            && ch.get_level() <= vict.unwrap().get_level()
        {
            send_to_char(&mut game.descriptors, ch, "No, no, no!\r\n");
        } else {
            let vict = vict.unwrap();
            send_to_char(&mut game.descriptors, ch, OK);
            act(&mut game.descriptors, chars, 
                db,
                &buf1,
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT,
            );
            let ch = chars.get(chid);
            game.mudlog(chars,
                DisplayMode::Normal,
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
            command_interpreter(game, db, chars, texts, objs, vict.id(), &to_force);
        }
    } else if arg == "room" {
        send_to_char(&mut game.descriptors, ch, OK);
        let ch = chars.get(chid);
        game.mudlog(chars,
            DisplayMode::Normal,
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
        let ch = chars.get(chid);
        for vict_id in db.world[ch.in_room() as usize].peoples.clone() {
            let vict = chars.get(vict_id);
            let ch = chars.get(chid);
            if !vict.is_npc() && vict.get_level() >= ch.get_level() {
                continue;
            }
            act(&mut game.descriptors, chars, 
                db,
                &buf1,
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT,
            );
            command_interpreter(game, db,chars, texts, objs, vict_id, &to_force);
        }
    } else {
        /* force all */
        send_to_char(&mut game.descriptors, ch, OK);
        let ch = chars.get(chid);
        game.mudlog(chars,
            DisplayMode::Normal,
            max(LVL_GOD as i32, ch.get_invis_lev() as i32),
            true,
            format!("(GC) {} forced all to {}", ch.get_name(), to_force).as_str(),
        );
        for i_id in game.descriptor_list.clone() {
            let mut vict_id = None;
            let ch = chars.get(chid);
            let i = game.desc(i_id);
            if i.state() != ConPlaying
                || {
                    vict_id = i.character;
                    vict_id.is_none()
                }
                || !chars.get(vict_id.unwrap()).is_npc()
                    && chars.get(vict_id.unwrap()).get_level() >= ch.get_level()
            {
                continue;
            }
            let vict = chars.get(vict_id.unwrap());
            act(&mut game.descriptors, chars, 
                db,
                &buf1,
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT,
            );
            command_interpreter(game, db, chars, texts, objs, vict_id.unwrap(), &to_force);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_wiznet(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut emote = false;
    let mut level = LVL_IMMORT;
    let mut buf1 = String::new();
    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Usage: wiznet <text> | #<level> <text> | *<emotetext> |\r\n        wiznet @<level> *<emotetext> | wiz @\r\n");
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
                    send_to_char(&mut game.descriptors, ch, "You can't wizline above your own level.\r\n");
                    return;
                }
            } else if emote {
            }
        }

        '@' => {
            send_to_char(&mut game.descriptors, ch, "God channel status:\r\n");
            for d_id in game.descriptor_list.clone() {
                let d = game.desc(d_id);
                if d.state() != ConPlaying
                    || chars.get(d.character.unwrap()).get_level() < LVL_IMMORT as u8
                {
                    continue;
                }
                let ch = chars.get(chid);
                if !can_see(&game.descriptors, chars, db, ch, chars.get(d.character.unwrap())) {
                    continue;
                }
                let dco = d.character;
                let dc_id = dco.unwrap();
                let dc = chars.get(dc_id);
                send_to_char(&mut game.descriptors, 
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
                        if dc.prf_flagged(PrefFlags::NOWIZ) {
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

    if ch.prf_flagged(PrefFlags::NOWIZ) {
        send_to_char(&mut game.descriptors, ch, "You are offline!\r\n");
        return;
    }
    let argument = argument.trim_start();

    if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Don't bother the gods like that!\r\n");
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
    for d_id in game.descriptor_list.clone() {
        let res = {
            let ch = chars.get(chid);
            let d = game.desc(d_id);
            d.state() == ConPlaying
                && chars.get(d.character.unwrap()).get_level() >= level as u8
                && !chars.get(d.character.unwrap())
                    .prf_flagged(PrefFlags::NOWIZ)
                && !chars.get(d.character.unwrap())
                    .plr_flagged(PLR_WRITING | PLR_MAILING)
                && d_id == ch.desc.unwrap()
                || !chars.get(d.character.unwrap())
                    .prf_flagged(PrefFlags::NOREPEAT)
        }; 
        if res {
            let d = game.desc(d_id);
            let chid = d.character.unwrap();
            let dc_id = d.character.unwrap();
            let msg = CCCYN!(chars.get(d.character.unwrap()), C_NRM);
            send_to_char(&mut game.descriptors, ch, msg );
            let dc = chars.get(dc_id);
            let ch = chars.get(chid);
            if can_see(&game.descriptors, chars, db, dc, ch) {
                send_to_char(&mut game.descriptors, dc, &buf1);
            } else {
                send_to_char(&mut game.descriptors, dc, &buf2);
            }
            send_to_char(&mut game.descriptors, dc, CCNRM!(dc, C_NRM));
        }
    }
    let ch = chars.get(chid);
    if ch.prf_flagged(PrefFlags::NOREPEAT) {
        send_to_char(&mut game.descriptors, ch, OK);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_zreset(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "You must specify a zone.\r\n");
        return;
    }
    let zone_count = db.zone_table.len();
    let mut i = zone_count;
    if arg.starts_with('*') {
        for i in 0..zone_count {
            game.reset_zone(db,chars, objs,i);
        }
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, "Reset world.\r\n");
        game.mudlog(chars,
            DisplayMode::Normal,
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
        game.reset_zone(db,chars,objs, i);
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "Reset zone {} (#{}): {}.\r\n",
                i, db.zone_table[i].number, db.zone_table[i].name
            )
            .as_str(),
        );
        game.mudlog(chars,
            DisplayMode::Normal,
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
        send_to_char(&mut game.descriptors, ch, "Invalid zone number.\r\n");
    }
}

/*
 *  General fn for wizcommands of the sort: cmd <player>
 */
#[allow(clippy::too_many_arguments)]
pub fn do_wizutil(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);

    let mut arg = String::new();
    one_argument(argument, &mut arg);
    let vict;
    #[allow(clippy::blocks_in_conditions)]
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Yes, but for whom?!?\r\n");
    } else if {
        vict = get_char_vis(&game.descriptors, chars,db, ch, &mut arg, None, FindFlags::CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, "There is no such player.\r\n");
    } else if vict.unwrap().is_npc() {
        send_to_char(&mut game.descriptors, ch, "You can't do that to a mob!\r\n");
    } else if vict.unwrap().get_level() > ch.get_level() {
        send_to_char(&mut game.descriptors, ch, "Hmmm...you'd better not.\r\n");
    } else {
        let vict = vict.unwrap();
        let vict_id = vict.id();
        match subcmd {
            SCMD_REROLL => {
                send_to_char(&mut game.descriptors, ch, "Rerolled...\r\n");
                let vict = chars.get_mut(vict_id);
                roll_real_abils(vict);
                let ch = chars.get(chid);
                let vict = chars.get(vict_id);
                info!("(GC) {} has rerolled {}.", ch.get_name(), vict.get_name());
                send_to_char(&mut game.descriptors, 
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
                    send_to_char(&mut game.descriptors, ch, "Your victim is not flagged.\r\n");
                    return;
                }
                let vict = chars.get_mut(vict_id);
                vict.remove_plr_flag(PLR_THIEF | PLR_KILLER);
                send_to_char(&mut game.descriptors, vict, "You have been pardoned by the Gods!\r\n");
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, ch, "Pardoned.\r\n");
                let ch = chars.get(chid);
                let vict = chars.get(vict_id);
                game.mudlog(chars,
                    DisplayMode::Brief,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} pardoned by {}", vict.get_name(), ch.get_name()).as_str(),
                );
            }
            SCMD_NOTITLE => {
                let vict = chars.get_mut(vict_id);
                let result = vict.plr_tog_chk(PLR_NOTITLE);
                let ch = chars.get(chid);
                let vict = chars.get(vict_id);
                game.mudlog(chars,
                    DisplayMode::Normal,
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
                let vict = chars.get(vict_id);
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, 
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
                let vict = chars.get_mut(vict_id);
                let result = vict.plr_tog_chk(PLR_NOSHOUT);
                let ch = chars.get(chid);
                let vict = chars.get(vict_id);
                game.mudlog(chars,
                    DisplayMode::Brief,
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
                let vict = chars.get(vict_id);
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, 
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
                if chid == vict_id {
                    send_to_char(&mut game.descriptors, ch, "Oh, yeah, THAT'S real smart...\r\n");
                    return;
                }
                if vict.plr_flagged(PLR_FROZEN) {
                    send_to_char(&mut game.descriptors, ch, "Your victim is already pretty cold.\r\n");
                    return;
                }
                let vict = chars.get_mut(vict_id);
                vict.set_plr_flag_bit(PLR_FROZEN);
                let ch = chars.get(chid);
                let val = ch.get_level();
                let vict = chars.get_mut(vict_id);
                vict.set_freeze_lev(val as i8);
                let vict = chars.get(vict_id);
                send_to_char(&mut game.descriptors, vict, "A bitter wind suddenly rises and drains every erg of heat from your body!\r\nYou feel frozen!\r\n");
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, ch, "Frozen.\r\n");
                act(&mut game.descriptors, chars, 
                    db,
                    "A sudden cold wind conjured from nowhere freezes $n!",
                    false,
                    Some(vict),
                    None,
                    None,
                    TO_ROOM,
                );
                let vict = chars.get(vict_id);
                let ch = chars.get(chid);
                game.mudlog(chars,
                    DisplayMode::Brief,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} frozen by {}.", vict.get_name(), ch.get_name()).as_str(),
                );
            }
            SCMD_THAW => {
                if !vict.plr_flagged(PLR_FROZEN) {
                    send_to_char(&mut game.descriptors, 
                        ch,
                        "Sorry, your victim is not morbidly encased in ice at the moment.\r\n",
                    );
                    return;
                }
                if vict.get_freeze_lev() > ch.get_level() as i8 {
                    send_to_char(&mut game.descriptors, 
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
                game.mudlog(chars,
                    DisplayMode::Brief,
                    max(LVL_GOD as i32, ch.get_invis_lev() as i32),
                    true,
                    format!("(GC) {} un-frozen by {}.", vict.get_name(), ch.get_name()).as_str(),
                );
                let vict = chars.get_mut(vict_id);
                vict.remove_plr_flag(PLR_FROZEN);
                let vict = chars.get(vict_id);
                send_to_char(&mut game.descriptors, vict, "A fireball suddenly explodes in front of you, melting the ice!\r\nYou feel thawed.\r\n");
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, ch, "Thawed.\r\n");
                act(&mut game.descriptors, chars, 
                    db,
                    "A sudden fireball conjured from nowhere thaws $n!",
                    false,
                    Some(vict),
                    None,
                    None,
                    TO_ROOM,
                );
            }
            SCMD_UNAFFECT => {
                let vict = chars.get_mut(vict_id);
                if !vict.affected.is_empty() {
                    while {
                        !vict.affected.is_empty()
                    } {
                        let af = vict.affected[0];
                        affect_remove( objs,vict, af);
                    }
                    let ch = chars.get(chid);
                    let vict = chars.get(vict_id);
                    send_to_char(&mut game.descriptors, 
                        vict,
                        "There is a brief flash of light!\r\nYou feel slightly different.\r\n",
                    );
                    send_to_char(&mut game.descriptors, ch, "All spells removed.\r\n");
                } else {
                    let ch = chars.get(chid);
                    send_to_char(&mut game.descriptors, ch, "Your victim does not have any affections!\r\n");
                    return;
                }
            }
            _ => {
                error!("SYSERR: Unknown subcmd {} passed to do_wizutil ", subcmd);
            }
        }
        save_char(&mut game.descriptors, db,chars, texts,objs,vict_id);
    }
}

/* single zone printing fn used by "show zone" so it's not repeated in the
code 3 times ... -je, 4/6/93 */

fn print_zone_to_buf(db: &DB,buf: &mut String, zone: ZoneRnum) {
    let zone = &db.zone_table[zone as usize];
    buf.push_str(
        format!(
            "{:3} {:30} Age: {:3}; Reset: {:3} ({:1}); Range: {:5}-{:5}\r\n",
            zone.number, zone.name, zone.age, zone.lifespan, zone.reset_mode, zone.bot, zone.top
        )
        .as_str(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn do_show(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);

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
        send_to_char(&mut game.descriptors, ch, "Show options:\r\n");
        let mut j = 0;
        for field in FIELDS.iter() {
            let ch = chars.get(chid);
            if field.level <= ch.get_level() as i16 {
                #[allow(clippy::blocks_in_conditions)]
                send_to_char(&mut game.descriptors, 
                    ch,
                    format!(
                        "{:15}{}",
                        field.cmd,
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
        send_to_char(&mut game.descriptors, ch, "\r\n");
        return;
    }

    let mut field = String::new();
    let mut value = String::new();
    two_arguments(argument, &mut field, &mut value);

    let l = FIELDS.iter().position(|f| f.cmd == field);
    let l = l.unwrap_or_default();

    if ch.get_level() < FIELDS[l].level as u8 {
        send_to_char(&mut game.descriptors, ch, "You are not godly enough for that!\r\n");
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
                print_zone_to_buf(db, &mut buf, db.world[ch.in_room() as usize].zone);
            } else if !value.is_empty() && is_number(&value) {
                let value = value.parse::<i32>().unwrap();
                let zrn = db.zone_table.iter().position(|z| z.number == value as i16);
                if let Some(zrn) = zrn  {
                    print_zone_to_buf(db, &mut buf, zrn as ZoneRnum);
                } else {
                    send_to_char(&mut game.descriptors, ch, "That is not a valid zone.\r\n");
                    return;
                }
            } else {
                for i in 0..db.zone_table.len() {
                    print_zone_to_buf(db, &mut buf, i as ZoneRnum);
                }
            }
            let desc_id = ch.desc.unwrap();
            page_string(&mut game.descriptors, chars,  desc_id, &buf, true);
        }

        /* show player */
        2 => {
            if value.is_empty() {
                send_to_char(&mut game.descriptors, ch, "A name would help.\r\n");
                return;
            }

            let mut vbuf = CharFileU::default();
            if db.load_char(&value, &mut vbuf).is_none() {
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, ch, "There is no such player.\r\n");
                return;
            }
            let ch = chars.get(chid);
            send_to_char(&mut game.descriptors, 
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
            send_to_char(&mut game.descriptors, 
                ch,
                format!(
                    "Au:{:8}  Bal:{:8}  Exp:{:8}  Align: {:5}  Lessons: {:3}\r\n",
                    g, bg, exp, ali, stl
                )
                .as_str(),
            );
            /* ctime() uses static buffer: do not combine. */
            send_to_char(&mut game.descriptors, ch, format!("Started: {}  ", ctime(vbuf.birth)).as_str());
            send_to_char(&mut game.descriptors, 
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
                send_to_char(&mut game.descriptors, ch, "A name would help.\r\n");
                return;
            }
            crash_listrent(game, chars, db,objs, chid, &value);
        }

        /* show stats */
        4 => {
            let mut i = 0;
            let mut j = 0;
            let mut k = 0;
            let mut con = 0;
            for &vict_id in &db.character_list {
                let vict = chars.get(vict_id);
                if vict.is_npc() {
                    j += 1;
                } else if can_see(&game.descriptors, chars, db, ch, vict) {
                    i += 1;
                    if vict.desc.is_some() {
                        con += 1;
                    }
                }
            }
            for _ in db.object_list.iter() {
                k += 1;
            }
            send_to_char(&mut game.descriptors, 
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
            page_string(&mut game.descriptors, chars,  desc_id, &buf, true);
        }

        /* show death */
        6 => {
            let mut buf = "Death Traps\r\n-----------\r\n".to_string();
            let mut j = 0;
            for i in 0..db.world.len() {
                if db.room_flagged(i as RoomRnum, RoomFlags::DEATH) {
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
            page_string(&mut game.descriptors, chars,  desc_id, &buf, true);
        }

        /* show godrooms */
        7 => {
            let mut buf = "Godrooms\r\n--------------------------\r\n".to_string();
            let mut j = 0;
            for i in 0..db.world.len() {
                if db.room_flagged(i as RoomRnum, RoomFlags::GODROOM) {
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
            page_string(&mut game.descriptors, chars,  desc_id, &buf, true);
        }

        /* show shops */
        8 => {
            show_shops(game, chars, db, chid, &value);
        }

        /* show houses */
        9 => {
            hcontrol_list_houses(&mut game.descriptors, chars, db, chid);
        }

        /* show snoop */
        10 => {
            let mut i = 0;
            send_to_char(&mut game.descriptors, 
                ch,
                "People currently snooping:\r\n--------------------------\r\n",
            );
            for d_id in game.descriptor_list.clone() {
                let d = game.desc(d_id);
                if d.snooping.borrow().is_none()
                    || d.character.is_none()
                {
                    continue;
                }
                let dco = d.character;
                let dc_id = dco.unwrap();
                let dc = chars.get(dc_id);
                let ch = chars.get(chid);
                if d.state() != ConPlaying || ch.get_level() < dc.get_level() {
                    continue;
                }
                if !can_see(&game.descriptors, chars, db, ch, dc) || dc.in_room() == NOWHERE {
                    continue;
                }
                i += 1;
                let msg = format!(
                    "{:10} - snooped by {}.\r\n",
                    chars.get(game.desc(d.snooping.unwrap())
                        .character
                        .unwrap())
                        .get_name(),
                    dc.get_name()
                );
                send_to_char(&mut game.descriptors, 
                    ch,
                    msg.as_str(),
                );
            }
            if i == 0 {
                send_to_char(&mut game.descriptors, ch, "No one is currently snooping.\r\n");
            }
        }

        /* show what? */
        _ => {
            send_to_char(&mut game.descriptors, ch, "Sorry, I don't understand that.\r\n");
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

#[allow(clippy::too_many_arguments)]
fn perform_set(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,chars: &mut Depot<CharData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    vict_id: DepotId,
    mode: i32,
    val_arg: &str,
) -> bool {
    let ch = chars.get(chid);
    let vict = chars.get(vict_id);

    let mut on = false;
    let mut off = false;
    let mut value = 0;
    let mode = mode as usize;

    /* Check to make sure all the levels are correct */
    if ch.get_level() != LVL_IMPL as u8
        && !vict.is_npc() && ch.get_level() <= vict.get_level() && vict_id != chid {
            send_to_char(descs, ch, "Maybe that's not such a great idea...\r\n");
            return false;
        }
    if ch.get_level() < SET_FIELDS[mode].level as u8 {
        send_to_char(descs, ch, "You are not godly enough for that!\r\n");
        return false;
    }

    /* Make sure the PC/NPC is correct */
    if vict.is_npc() && SET_FIELDS[mode].pcnpc & NPC == 0 {
        send_to_char(descs, ch, "You can't do that to a beast!\r\n");
        return false;
    } else if !vict.is_npc() && SET_FIELDS[mode].pcnpc & PC == 0 {
        send_to_char(descs, ch, "That can only be done to a beast!\r\n");
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
            send_to_char(descs, ch, "Value must be 'on' or 'off'.\r\n");
            return false;
        }
        send_to_char(descs, 
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
        let r = val_arg.trim().parse::<i32>();
        value = r.unwrap_or_default();
        send_to_char(descs, 
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
        send_to_char(descs, ch, OK);
    }
    let rnum;
    let vict = chars.get_mut(vict_id);
    match mode {
        0 => {
            if on {
                vict.set_prf_flags_bits(PrefFlags::BRIEF)
            } else {
                vict.remove_prf_flags_bits(PrefFlags::BRIEF)
            }
        }
        1 => {
            if on {
                vict.set_plr_flag_bit(  PLR_INVSTART)
            } else {
                vict.remove_plr_flag(PLR_INVSTART)
            }
        }
        2 => {
            set_title(vict, Some(val_arg));
            let messg = format!(
                "{}'s title is now: {}\r\n",
                vict.get_name(),
                vict.get_title()
            );
            let ch = chars.get(chid);
            send_to_char(descs, ch, messg.as_str());
        }
        3 => {
            if on {
                vict.set_prf_flags_bits(PrefFlags::SUMMONABLE)
            } else {
                vict.remove_prf_flags_bits(PrefFlags::SUMMONABLE)
            }
            let messg = format!("Nosummon {} for {}.\r\n", onoff!(!on), vict.get_name());
            let ch = chars.get(chid);
            send_to_char(descs, ch, messg.as_str());
        }
        4 => {
            vict.points.max_hit = range!(value, 1, 5000) as i16;
            affect_total(objs,vict);
        }
        5 => {
            vict.points.max_mana = range!(value, 1, 5000) as i16;
            affect_total(objs,vict);
        }
        6 => {
            vict.points.max_move = range!(value, 1, 5000) as i16;
            affect_total(objs,vict);
        }
        7 => {
            vict.points.hit = range!(value, -9, vict.points.max_hit) as i16;
            affect_total(objs,vict);
        }
        8 => {
            vict.points.mana = range!(value, 0, vict.points.max_mana) as i16;
            affect_total(objs,vict);
        }
        9 => {
            vict.points.movem = range!(value, 0, vict.points.max_move) as i16;
            affect_total(objs,vict);
        }
        10 => {
            vict.set_alignment(range!(value, -1000, 1000));
            affect_total(objs,vict);
        }
        11 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.str = value as i8;
            vict.real_abils.str_add = 0;
            affect_total(objs,vict);
        }
        12 => {
            vict.real_abils.str_add = range!(value, 0, 100) as i8;
            if value > 0 {
                vict.real_abils.str = 18;
            }
            affect_total(objs,vict);
        }
        13 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.intel = value as i8;
            affect_total(objs,vict);
        }
        14 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.wis = value as i8;
            affect_total(objs,vict);
        }
        15 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.dex = value as i8;
            affect_total(objs,vict);
        }
        16 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.con = value as i8;
            affect_total(objs,vict);
        }
        17 => {
            if vict.is_npc() || vict.get_level() >= LVL_GRGOD as u8 {
                value = range!(value, 3, 25);
            } else {
                value = range!(value, 3, 18);
            }
            vict.real_abils.cha = value as i8;
            affect_total(objs,vict);
        }
        18 => {
            vict.points.armor = range!(value, -100, 100) as i16;
            affect_total(objs,vict);
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
            affect_total(objs,vict);
        }
        23 => {
            vict.points.damroll = range!(value, -20, 20) as i8;
            affect_total(objs,vict);
        }
        24 => {
            let ch = chars.get(chid);
            if ch.get_level() < LVL_IMPL as u8 && chid != vict_id {
                send_to_char(descs, ch, "You aren't godly enough for that!\r\n");
                return false;
            }
            let vict = chars.get_mut(vict_id);
            vict.set_invis_lev(range!(value, 0, vict.get_level()) as i16);
        }
        25 => {
            let ch = chars.get(chid);
            if ch.get_level() < LVL_IMPL as u8 && chid != vict_id {
                send_to_char(descs, ch, "You aren't godly enough for that!\r\n");
                return false;
            }
            let vict = chars.get_mut(vict_id);
            if on {
                vict.set_prf_flags_bits(PrefFlags::NOHASSLE)
            } else {
                vict.remove_prf_flags_bits(PrefFlags::NOHASSLE)
            }
        }
        26 => {
            if chid == vict_id && on {
                let ch = chars.get(chid);
                send_to_char(descs, ch, "Better not -- could be a long winter!\r\n");
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
        29..=31 => {
            if val_arg == "off" {
                vict.set_cond(mode - 29, -1); /* warning: magic number here */
                let vict = chars.get(vict_id);
                let ch = chars.get(chid);
                send_to_char(descs, 
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
                vict.set_cond(mode - 29, value as i16); /* and here too */
                let vict = chars.get(vict_id);
                let ch = chars.get(chid);
                send_to_char(descs, 
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
                let ch = chars.get(chid);
                send_to_char(descs, ch, "Must be 'off' or a value from 0 to 24.\r\n");
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
            let ch = chars.get(chid);
            if value > ch.get_level() as i32 || value > LVL_IMPL as i32 {
                send_to_char(descs, ch, "You can't do that.\r\n");
                return false;
            }
            value = range!(value, 0, LVL_IMPL);
            chars.get_mut(vict_id).player.level = value as u8;
        }
        35 => {
            let res = {
                rnum = db.real_room(value as RoomRnum);
                rnum == NOWHERE
            }; 
            if res {
                let ch = chars.get(chid);
                send_to_char(descs, ch, "No room exists with that number.\r\n");
                return false;
            }
            let vict = chars.get_mut(vict_id);
            if vict.in_room() != NOWHERE {
                /* Another Eric Green special. */
                db.char_from_room( objs,vict);
            }
            db.char_to_room(chars, objs,vict_id, rnum);
        }
        36 => {
            if on {
                vict.set_prf_flags_bits(PrefFlags::ROOMFLAGS)
            } else {
                vict.remove_prf_flags_bits(PrefFlags::ROOMFLAGS)
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
            let res = {
                i = parse_class(val_arg.chars().next().unwrap());
                i == Class::Undefined
            }; 
            if res {
                let ch = chars.get(chid);
                send_to_char(descs, ch, "That is not a class.\r\n");
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
                vict.set_prf_flags_bits(PrefFlags::QUEST)
            } else {
                vict.remove_prf_flags_bits(PrefFlags::QUEST)
            }
        }
        42 => {
            if val_arg == "off" {
                vict.remove_plr_flag(PLR_LOADROOM);
            } else if is_number(val_arg) {
                let rvnum = val_arg.parse::<i32>().unwrap() as RoomRnum;
                let ch = chars.get(chid);
                if db.real_room(rvnum) != NOWHERE {
                    let vict = chars.get_mut(vict_id);
                    vict.set_plr_flag_bit(PLR_LOADROOM);
                    vict.set_loadroom(rvnum);
                    let vict = chars.get(vict_id);
                    let ch = chars.get(chid);
                    send_to_char(descs, 
                        ch,
                        format!(
                            "{} will enter at room #{}.",
                            vict.get_name(),
                            vict.get_loadroom()
                        )
                        .as_str(),
                    );
                } else {
                    send_to_char(descs, ch, "That room does not exist!\r\n");
                    return false;
                }
            } else {
                let ch = chars.get(chid);
                send_to_char(descs, ch, "Must be 'off' or a room's virtual number.\r\n");
                return false;
            }
        }
        43 => {
            if on {
                vict.set_prf_flags_bits(PrefFlags::COLOR_1 | PrefFlags::COLOR_2)
            } else {
                vict.remove_prf_flags_bits(PrefFlags::COLOR_1 | PrefFlags::COLOR_2)
            }
        }
        44 => {
            let ch = chars.get(chid);
            let vict = chars.get(vict_id);
            if ch.get_idnum() != 1 || !vict.is_npc() {
                return false;
            }
            let vict = chars.get_mut(vict_id);
            vict.set_idnum(value as i64);
        }
        45 => {
            let ch = chars.get(chid);
            if ch.get_idnum() > 1 {
                send_to_char(descs, ch, "Please don't use this command, yet.\r\n");
                return false;
            }
            let vict = chars.get(vict_id);
            if vict.get_level() >= LVL_GRGOD as u8 {
                send_to_char(descs, ch, "You cannot change that.\r\n");
                return false;
            }
            let mut passwd2 = [0_u8; 16];
            let salt = vict.get_name();
            pbkdf2::pbkdf2::<Hmac<Sha256>>(val_arg.as_bytes(), salt.as_bytes(), 4, &mut passwd2)
                .expect("Error while encrypting password");
            let vict = chars.get_mut(vict_id);
            vict.set_passwd(passwd2);
            let ch = chars.get(chid);
            send_to_char(descs, 
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
            let res = {
                i = search_block(val_arg, &GENDERS, false);
                i.is_none()
            }; if res {
                let ch = chars.get(chid);
                send_to_char(descs, ch, "Must be 'male', 'female', or 'neutral'.\r\n");
                return false;
            }
            vict.set_sex((i.unwrap() as u8).into());
        }
        48 => {
            /* set age */
            if !(2..=200).contains(&value) {
                /* Arbitrary limits. */
                let ch = chars.get(chid);
                send_to_char(descs, ch, "Ages 2 to 200 accepted.\r\n");
                return false;
            }
            /*
             * NOTE: May not display the exact age specified due to the integer
             * division used elsewhere in the code.  Seems to only happen for
             * some values below the starting age (17) anyway. -gg 5/27/98
             */
            chars.get_mut(vict_id).player.time.birth =
                time_now() - ((value as u64 - 17) * SECS_PER_MUD_YEAR);
        }

        49 => {
            /* Blame/Thank Rick Glover. :) */
            vict.set_height(value as u8);
            affect_total(objs,vict);
        }

        50 => {
            vict.set_weight(value as u8);
            affect_total(objs,vict);
        }

        _ => {
            let ch = chars.get(chid);
            send_to_char(descs, ch, "Can't set that!\r\n");
            return false;
        }
    }

    true
}

#[allow(clippy::too_many_arguments)]
pub fn do_set(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut player_i = None;
    let mut is_file = false;
    let mut is_player = false;
    let mut argument = argument.to_string();
    let mut name = String::new();
    let mut buf = String::new();
    let mut field = String::new();
    let mut tmp_store = CharFileU::default();

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
        send_to_char(&mut game.descriptors, ch, "Usage: set <victim> <field> <value>\r\n");
        return;
    }
    let mut vict = None;
    /* find the target */
    if !is_file {
        if is_player {
            let res = {
                vict = get_player_vis(&game.descriptors, chars,db, ch, &mut name, None, FindFlags::CHAR_WORLD);
                vict.is_none()
            }; 
            if res {
                send_to_char(&mut game.descriptors, ch, "There is no such player.\r\n");
                return;
            }
        } else {
            /* is_mob */
            let res = {
                vict = get_char_vis(&game.descriptors, chars,db, ch, &mut name, None, FindFlags::CHAR_WORLD);
                vict.is_none()
            }; 
            if res {
                send_to_char(&mut game.descriptors, ch, "There is no such creature.\r\n");
                return;
            }
        }
    } else if is_file {
        /* try to load the player off disk */
        let mut cbuf = CharData::default();
        clear_char(&mut cbuf);
        let res = {
            player_i = db.load_char(&name, &mut tmp_store);
            player_i.is_some()
        }; 
        if res {
            store_to_char(texts,  &tmp_store, &mut cbuf);
            let ch = chars.get(chid);
            if cbuf.get_level() >= ch.get_level() {
                send_to_char(&mut game.descriptors, ch, "Sorry, you can't do that.\r\n");
                return;
            }
            let vict_id =chars.push(cbuf);
            vict = Some(chars.get(vict_id));
        } else {
            let ch = chars.get(chid);
            send_to_char(&mut game.descriptors, ch, "There is no such player.\r\n");
            return;
        }
    }

    /* find the command in the list */
    let mode = SET_FIELDS.iter().position(|e| e.cmd.starts_with(&field));
    let mode = mode.unwrap_or(SET_FIELDS.len() - 1);

    /* perform the set */
    let vict_id = vict.unwrap().id();
    let retval = perform_set(&mut game.descriptors, db,chars, objs, chid, vict_id, mode as i32, &buf);

    /* save the character if a change was made */
    if retval {
        if !is_file && !chars.get(vict_id).is_npc() {
            save_char(&mut game.descriptors, db,chars, texts,objs,vict_id);
        }
        if is_file {
            char_to_store(&mut game.descriptors, texts,objs,db,chars, vict_id, &mut tmp_store);

            unsafe {
                let player_slice = slice::from_raw_parts(
                    &mut tmp_store as *mut _ as *mut u8,
                    mem::size_of::<CharFileU>(),
                );
                db.player_fl
                    .as_mut()
                    .unwrap()
                    .write_all_at(
                        player_slice,
                        (player_i.unwrap() * mem::size_of::<CharFileU>()) as u64,
                    )
                    .expect("Error while writing player record to file");
            }
            let ch = chars.get(chid);
            send_to_char(&mut game.descriptors, ch, "Saved in file.\r\n");
        }
    }
    if is_file {
        db.free_char(&mut game.descriptors, chars, objs, vict_id);
    }
}
