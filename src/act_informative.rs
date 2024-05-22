/* ************************************************************************
*   File: act.informative.rs                            Part of CircleMUD *
*  Usage: Player-level commands of an informative nature                  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::cell::RefCell;
use std::rc::Rc;

use log::{error, info};
use regex::Regex;

use crate::act_social::{do_action, do_insult};
use crate::class::{find_class_bitvector, level_exp, title_female, title_male};
use crate::config::NOPERSON;
use crate::constants::{
    CIRCLEMUD_VERSION, COLOR_LIQUID, CONNECTED_TYPES, DIRS, FULLNESS, MONTH_NAME, ROOM_BITS,
    WEAR_WHERE, WEEKDAYS,
};
use crate::db::DB;
use crate::fight::compute_armor_class;
use crate::handler::{
    affected_by_spell, fname, get_number, isname, FIND_CHAR_ROOM, FIND_CHAR_WORLD, FIND_OBJ_EQUIP,
    FIND_OBJ_INV, FIND_OBJ_ROOM,
};
use crate::interpreter::{
    half_chop, is_abbrev, one_argument, search_block, CMD_INFO, SCMD_CLEAR, SCMD_CREDITS,
    SCMD_HANDBOOK, SCMD_IMMLIST, SCMD_IMOTD, SCMD_INFO, SCMD_MOTD, SCMD_NEWS, SCMD_POLICIES,
    SCMD_READ, SCMD_SOCIALS, SCMD_VERSION, SCMD_WHOAMI, SCMD_WIZHELP, SCMD_WIZLIST,
};
use crate::modify::page_string;
use crate::screen::{C_NRM, C_OFF, C_SPR, KCYN, KGRN, KNRM, KNUL, KRED, KYEL};
use crate::spells::SPELL_ARMOR;
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    CharData, ExtraDescrData, ObjData, AFF_DETECT_ALIGN, AFF_DETECT_MAGIC, AFF_HIDE, AFF_INVISIBLE,
    AFF_SANCTUARY, CONT_CLOSED, EX_CLOSED, EX_ISDOOR, ITEM_BLESS, ITEM_CONTAINER, ITEM_DRINKCON,
    ITEM_FOUNTAIN, ITEM_GLOW, ITEM_HUM, ITEM_INVISIBLE, ITEM_MAGIC, ITEM_NOTE, LVL_GOD, LVL_IMPL,
    NOWHERE, NUM_OF_DIRS, PLR_KILLER, PLR_MAILING, PLR_THIEF, PLR_WRITING, POS_FIGHTING,
    PRF_COLOR_1, PRF_COLOR_2, PRF_COMPACT, PRF_DEAF, PRF_DISPHP, PRF_DISPMANA, PRF_DISPMOVE,
    PRF_HOLYLIGHT, PRF_NOAUCT, PRF_NOGOSS, PRF_NOGRATZ, PRF_NOHASSLE, PRF_NOREPEAT, PRF_NOTELL,
    PRF_QUEST, SEX_FEMALE, SEX_MALE, SEX_NEUTRAL,
};
use crate::structs::{AFF_BLIND, PRF_AUTOEXIT, PRF_BRIEF, PRF_ROOMFLAGS, ROOM_DEATH};
use crate::structs::{
    AFF_CHARM, AFF_DETECT_INVIS, AFF_INFRAVISION, AFF_POISON, DRUNK, FULL, LVL_IMMORT, NUM_WEARS,
    POS_DEAD, POS_INCAP, POS_MORTALLYW, POS_RESTING, POS_SITTING, POS_SLEEPING, POS_STANDING,
    POS_STUNNED, PRF_SUMMONABLE, THIRST,
};
use crate::util::{
    age, rand_number, real_time_passed, sprintbit, sprinttype, time_now, SECS_PER_MUD_HOUR,
    SECS_PER_REAL_MIN,
};
use crate::{
    _clrlevel, an, clr, send_to_char, Game, CCCYN, CCGRN, CCRED, CCYEL, COLOR_LEV, TO_NOTVICT,
};
use crate::{CCNRM, TO_VICT};

pub const SHOW_OBJ_LONG: i32 = 0;
pub const SHOW_OBJ_SHORT: i32 = 1;
pub const SHOW_OBJ_ACTION: i32 = 2;

fn show_obj_to_char(obj: &ObjData, ch: &CharData, mode: i32) {
    match mode {
        SHOW_OBJ_LONG => {
            send_to_char(ch, format!("{}", obj.description).as_str());
        }

        SHOW_OBJ_SHORT => {
            send_to_char(ch, format!("{}", obj.short_description).as_str());
        }

        SHOW_OBJ_ACTION => match obj.get_obj_type() {
            ITEM_NOTE => {
                if !RefCell::borrow(&obj.action_description).is_empty() {
                    let notebuf = format!(
                        "There is something written on it:\r\n\r\n{}",
                        RefCell::borrow(&obj.action_description)
                    );
                    page_string(ch.desc.borrow().as_ref(), notebuf.as_str(), true);
                } else {
                    send_to_char(ch, "It's blank.\r\n");
                }
                return;
            }
            ITEM_DRINKCON => {
                send_to_char(ch, "It looks like a drink container.");
            }

            _ => {
                send_to_char(ch, "You see nothing special..");
            }
        },

        _ => {
            error!("SYSERR: Bad display mode ({}) in show_obj_to_char().", mode);
            return;
        }
    }

    show_obj_modifiers(obj, ch);
    send_to_char(ch, "\r\n");
}

fn show_obj_modifiers(obj: &ObjData, ch: &CharData) {
    if obj.obj_flagged(ITEM_INVISIBLE) {
        send_to_char(ch, " (invisible)");
    }

    if obj.obj_flagged(ITEM_BLESS) && ch.aff_flagged(AFF_DETECT_ALIGN) {
        send_to_char(ch, " ..It glows blue!");
    }

    if obj.obj_flagged(ITEM_MAGIC) && ch.aff_flagged(AFF_DETECT_MAGIC) {
        send_to_char(ch, " ..It glows yellow!");
    }

    if obj.obj_flagged(ITEM_GLOW) {
        send_to_char(ch, " ..It has a soft glowing aura!");
    }

    if obj.obj_flagged(ITEM_HUM) {
        send_to_char(ch, " ..It emits a faint humming sound!");
    }
}

fn list_obj_to_char(db: &DB, list: &Vec<Rc<ObjData>>, ch: &CharData, mode: i32, show: bool) {
    let mut found = true;

    for obj in list {
        if db.can_see_obj(ch, obj) {
            show_obj_to_char(obj, ch, mode);
            found = true;
        }
    }
    if !found && show {
        send_to_char(ch, " Nothing.\r\n");
    }
}

fn diag_char_to_char(db: &DB, i: &Rc<CharData>, ch: &Rc<CharData>) {
    struct Item {
        percent: i8,
        text: &'static str,
    }
    const DIAGNOSIS: [Item; 8] = [
        Item {
            percent: 100,
            text: "is in excellent condition.",
        },
        Item {
            percent: 90,
            text: "has a few scratches.",
        },
        Item {
            percent: 75,
            text: "has some small wounds and bruises.",
        },
        Item {
            percent: 50,
            text: "has quite a few wounds.",
        },
        Item {
            percent: 30,
            text: "has some big nasty wounds and scratches.",
        },
        Item {
            percent: 15,
            text: "looks pretty hurt.",
        },
        Item {
            percent: 0,
            text: "is in awful condition.",
        },
        Item {
            percent: -1,
            text: "is bleeding awfully from big wounds.",
        },
    ];

    let pers = db.pers(i, ch);

    let percent;
    if i.get_max_hit() > 0 {
        percent = (100 * i.get_hit() as i32) / i.get_max_hit() as i32;
    } else {
        percent = -1; /* How could MAX_HIT be < 1?? */
    }
    let mut ar_index: usize = 0;
    loop {
        if DIAGNOSIS[ar_index].percent < 0 || percent >= DIAGNOSIS[ar_index as usize].percent as i32
        {
            break;
        }
        ar_index += 1;
    }

    send_to_char(
        ch,
        format!(
            "{}{} {}\r\n",
            pers.chars().next().unwrap().to_uppercase(),
            &pers[1..],
            DIAGNOSIS[ar_index as usize].text
        )
        .as_str(),
    );
}

fn look_at_char(db: &DB, i: &Rc<CharData>, ch: &Rc<CharData>) {
    let mut found;
    //struct obj_data *tmp_obj;

    if ch.desc.borrow().is_none() {
        return;
    }

    if !RefCell::borrow(&i.player.borrow().description).is_empty() {
        send_to_char(ch, RefCell::borrow(&i.player.borrow().description).as_str());
    } else {
        db.act(
            "You see nothing special about $m.",
            false,
            Some(i),
            None,
            Some(ch),
            TO_VICT,
        );
    }

    diag_char_to_char(db, i, ch);

    found = false;
    for j in 0..NUM_WEARS {
        if i.get_eq(j).is_some() && db.can_see_obj(ch, i.get_eq(j).as_ref().unwrap()) {
            found = true;
        }
    }

    if found {
        send_to_char(ch, "\r\n"); /* act() does capitalization. */
        db.act("$n is using:", false, Some(i), None, Some(ch), TO_VICT);
        for j in 0..NUM_WEARS {
            if i.get_eq(j).is_some() && db.can_see_obj(ch, i.get_eq(j).as_ref().unwrap()) {
                send_to_char(ch, WEAR_WHERE[j as usize]);
                show_obj_to_char(i.get_eq(j).as_ref().unwrap(), ch, SHOW_OBJ_SHORT);
            }
        }
    }
    if !Rc::ptr_eq(i, ch) && (ch.is_thief() || ch.get_level() >= LVL_IMMORT as u8) {
        found = false;
        db.act(
            "\r\nYou attempt to peek at $s inventory:",
            false,
            Some(i),
            None,
            Some(ch),
            TO_VICT,
        );
        for tmp_obj in i.carrying.borrow().iter() {
            if db.can_see_obj(ch, tmp_obj) && rand_number(0, 20) < ch.get_level() as u32 {
                show_obj_to_char(tmp_obj, ch, SHOW_OBJ_SHORT);
                found = true;
            }
        }
    }

    if !found {
        send_to_char(ch, "You can't see anything.\r\n");
    }
}

fn list_one_char(db: &DB, i: &Rc<CharData>, ch: &Rc<CharData>) {
    const POSITIONS: [&str; 9] = [
        " is lying here, dead.",
        " is lying here, mortally wounded.",
        " is lying here, incapacitated.",
        " is lying here, stunned.",
        " is sleeping here.",
        " is resting here.",
        " is sitting here.",
        "!FIGHTING!",
        " is standing here.",
    ];

    if i.is_npc() && !i.player.borrow().long_descr.is_empty() && i.get_pos() == i.get_default_pos()
    {
        if i.aff_flagged(AFF_INVISIBLE) {
            send_to_char(ch, "*");
        }

        if ch.aff_flagged(AFF_DETECT_ALIGN) {
            if i.is_evil() {
                send_to_char(ch, "(Red Aura) ");
            } else if i.is_good() {
                send_to_char(ch, "(Blue Aura) ");
            }
        }
        send_to_char(ch, i.player.borrow().long_descr.as_str());

        if i.aff_flagged(AFF_SANCTUARY) {
            db.act(
                "...$e glows with a bright light!",
                false,
                Some(i),
                None,
                Some(ch),
                TO_VICT,
            );
        }
        if i.aff_flagged(AFF_BLIND) {
            db.act(
                "...$e is groping around blindly!",
                false,
                Some(i),
                None,
                Some(ch),
                TO_VICT,
            );
        }
        return;
    }

    if i.is_npc() {
        send_to_char(
            ch,
            format!(
                "{}{}",
                i.player.borrow().short_descr.as_str()[0..1].to_uppercase(),
                &i.player.borrow().short_descr.as_str()[1..]
            )
            .as_str(),
        );
    } else {
        send_to_char(
            ch,
            format!("{} {}", i.player.borrow().name.as_str(), i.get_title()).as_str(),
        );
    }

    if i.aff_flagged(AFF_INVISIBLE) {
        send_to_char(ch, " (invisible)");
    }
    if i.aff_flagged(AFF_HIDE) {
        send_to_char(ch, " (hidden)");
    }
    if !i.is_npc() && i.desc.borrow().is_none() {
        send_to_char(ch, " (linkless)");
    }
    if !i.is_npc() && i.plr_flagged(PLR_WRITING) {
        send_to_char(ch, " (writing)");
    }
    if i.get_pos() != POS_FIGHTING {
        send_to_char(ch, POSITIONS[i.get_pos() as usize]);
    } else {
        if i.fighting().is_some() {
            send_to_char(ch, " is here, fighting ");
            if Rc::ptr_eq(i.fighting().as_ref().unwrap(), &ch) {
                send_to_char(ch, "YOU!");
            } else {
                if i.in_room() == i.fighting().as_ref().unwrap().in_room() {
                    send_to_char(
                        ch,
                        format!("{}!", db.pers(i.fighting().as_ref().unwrap(), ch.as_ref()))
                            .as_str(),
                    );
                } else {
                    send_to_char(ch, "someone who has already left!");
                }
            }
        } else {
            /* NIL fighting pointer */
            send_to_char(ch, " is here struggling with thin air.");
        }
    }

    if ch.aff_flagged(AFF_DETECT_ALIGN) {
        if i.is_evil() {
            send_to_char(ch, " (Red Aura)");
        } else if i.is_good() {
            send_to_char(ch, " (Blue Aura)");
        }
    }
    send_to_char(ch, "\r\n");

    if i.aff_flagged(AFF_SANCTUARY) {
        db.act(
            "...$e glows with a bright light!",
            false,
            Some(i),
            None,
            Some(ch),
            TO_VICT,
        );
    }
}

fn list_char_to_char(db: &DB, list: &Vec<Rc<CharData>>, ch: &Rc<CharData>) {
    for i in list {
        if !Rc::ptr_eq(i, &ch) {
            if db.can_see(ch.as_ref(), i) {
                list_one_char(db, i, &ch);
            } else if db.is_dark(ch.in_room())
                && !ch.can_see_in_dark()
                && i.aff_flagged(AFF_INFRAVISION)
            {
                send_to_char(
                    ch.as_ref(),
                    "You see a pair of glowing red eyes looking your way.\r\n",
                );
            }
        }
    }
}

fn do_auto_exits(db: &DB, ch: &CharData) {
    let mut slen = 0;
    send_to_char(ch, format!("{}[ Exits: ", CCCYN!(ch, C_NRM)).as_str());
    for door in 0..NUM_OF_DIRS {
        if db.exit(ch, door).is_none()
            || db.exit(ch, door).as_ref().unwrap().to_room.get() == NOWHERE
        {
            continue;
        }
        if db.exit(ch, door).as_ref().unwrap().exit_flagged(EX_CLOSED) {
            continue;
        }
        send_to_char(ch, format!("{} ", DIRS[door].to_lowercase()).as_str());
        slen += 1;
    }
    send_to_char(
        ch,
        format!(
            "{}]{}\r\n",
            if slen != 0 { "" } else { "None!" },
            CCNRM!(ch, C_NRM)
        )
        .as_str(),
    );
}

pub fn do_exits(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.aff_flagged(AFF_BLIND) {
        send_to_char(ch, "You can't see a damned thing, you're blind!\r\n");
        return;
    }
    let db = &game.db;
    send_to_char(ch, "Obvious exits:\r\n");
    let mut len = 0;
    for door in 0..NUM_OF_DIRS {
        if db.exit(ch, door).is_none()
            || db.exit(ch, door).as_ref().unwrap().to_room.get() == NOWHERE
        {
            continue;
        }
        if db.exit(ch, door).as_ref().unwrap().exit_flagged(EX_CLOSED) {
            continue;
        }
        len += 1;

        let oexit = db.exit(ch, door);
        let exit = oexit.as_ref().unwrap();
        if ch.get_level() >= LVL_IMMORT as u8 {
            send_to_char(
                ch,
                format!(
                    "{} - [{:5}] {}\r\n",
                    DIRS[door as usize],
                    db.get_room_vnum(exit.to_room.get()),
                    db.world.borrow()[exit.to_room.get() as usize].name
                )
                .as_str(),
            );
        } else {
            let world = db.world.borrow();
            send_to_char(
                ch,
                format!(
                    "{} - {}\r\n",
                    DIRS[door as usize],
                    if db.is_dark(exit.to_room.get()) && !ch.can_see_in_dark() {
                        "Too dark to tell."
                    } else {
                        world[exit.to_room.get() as usize].name.as_str()
                    }
                )
                .as_str(),
            );
        }
    }

    if len == 0 {
        send_to_char(ch, " None.\r\n");
    }
}

pub fn look_at_room(db: &DB, ch: &Rc<CharData>, ignore_brief: bool) {
    if ch.desc.borrow().is_none() {
        return;
    }

    if db.is_dark(ch.in_room()) && !ch.can_see_in_dark() {
        send_to_char(ch, "It is pitch black...\r\n");
        return;
    } else if ch.aff_flagged(AFF_BLIND) {
        send_to_char(ch, "You see nothing but infinite darkness...\r\n");
        return;
    }
    send_to_char(ch, format!("{}", CCCYN!(ch, C_NRM)).as_str());

    if !ch.is_npc() && ch.prf_flagged(PRF_ROOMFLAGS) {
        let mut buf = String::new();
        sprintbit(db.room_flags(ch.in_room()) as i64, &ROOM_BITS, &mut buf);
        send_to_char(
            ch,
            format!(
                "[{}] {} [{}]",
                db.get_room_vnum(ch.in_room()),
                db.world.borrow()[ch.in_room() as usize].name,
                buf
            )
            .as_str(),
        );
    } else {
        send_to_char(
            ch,
            format!("{}", db.world.borrow()[ch.in_room() as usize].name).as_str(),
        );
    }

    send_to_char(ch, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());

    if (!ch.is_npc() && !ch.prf_flagged(PRF_BRIEF))
        || ignore_brief
        || db.room_flagged(ch.in_room(), ROOM_DEATH)
    {
        send_to_char(
            ch,
            format!("{}", db.world.borrow()[ch.in_room() as usize].description).as_str(),
        );
    }

    /* autoexits */
    if !ch.is_npc() && ch.prf_flagged(PRF_AUTOEXIT) {
        do_auto_exits(db, ch);
    }

    /* now list characters & objects */
    send_to_char(ch, format!("{}", CCGRN!(ch, C_NRM)).as_str());
    list_obj_to_char(
        db,
        db.world.borrow()[ch.in_room() as usize]
            .contents
            .borrow()
            .as_ref(),
        ch,
        SHOW_OBJ_LONG,
        false,
    );
    send_to_char(ch, format!("{}", CCYEL!(ch, C_NRM)).as_str());
    list_char_to_char(
        db,
        db.world.borrow()[ch.in_room() as usize]
            .peoples
            .borrow()
            .as_ref(),
        ch,
    );
    send_to_char(ch, format!("{}", CCNRM!(ch, C_NRM)).as_str());
}

fn look_in_direction(db: &DB, ch: &Rc<CharData>, dir: i32) {
    if db.exit(ch, dir as usize).is_some() {
        if !db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .general_description
            .is_empty()
        {
            send_to_char(
                ch,
                format!(
                    "{}",
                    db.exit(ch, dir as usize)
                        .as_ref()
                        .unwrap()
                        .general_description
                )
                .as_str(),
            );
        } else {
            send_to_char(ch, "You see nothing special.\r\n");
        }
        if db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .exit_flagged(EX_CLOSED)
            && !db
                .exit(ch, dir as usize)
                .as_ref()
                .unwrap()
                .keyword
                .is_empty()
        {
            send_to_char(
                ch,
                format!(
                    "The {} is closed.\r\n",
                    fname(db.exit(ch, dir as usize).as_ref().unwrap().keyword.as_str())
                )
                .as_str(),
            );
        } else if db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .exit_flagged(EX_ISDOOR)
            && !db
                .exit(ch, dir as usize)
                .as_ref()
                .unwrap()
                .keyword
                .is_empty()
        {
            send_to_char(
                ch,
                format!(
                    "The {} is open.\r\n",
                    fname(db.exit(ch, dir as usize).as_ref().unwrap().keyword.as_str())
                )
                .as_str(),
            );
        } else {
            send_to_char(ch, "Nothing special there...\r\n");
        }
    }
}

fn look_in_obj(db: &DB, ch: &Rc<CharData>, arg: &str) {
    let mut dummy: Option<Rc<CharData>> = None;
    let mut obj: Option<Rc<ObjData>> = None;
    let bits;

    if arg.is_empty() {
        send_to_char(ch, "Look in what?\r\n");
        return;
    }
    bits = db.generic_find(
        arg,
        (FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_OBJ_EQUIP) as i64,
        ch,
        &mut dummy,
        &mut obj,
    );
    if bits == 0 {
        send_to_char(
            ch,
            format!("There doesn't seem to be {} {} here.\r\n", an!(arg), arg).as_str(),
        );
    } else if obj.as_ref().unwrap().get_obj_type() != ITEM_DRINKCON
        && obj.as_ref().unwrap().get_obj_type() != ITEM_FOUNTAIN
        && obj.as_ref().unwrap().get_obj_type() != ITEM_CONTAINER
    {
        send_to_char(ch, "There's nothing inside that!\r\n");
    } else {
        if obj.as_ref().unwrap().get_obj_type() == ITEM_CONTAINER {
            if obj.as_ref().unwrap().objval_flagged(CONT_CLOSED) {
                send_to_char(ch, "It is closed.\r\n");
            } else {
                send_to_char(
                    ch,
                    fname(obj.as_ref().unwrap().name.borrow().as_str()).as_ref(),
                );
                match bits {
                    FIND_OBJ_INV => {
                        send_to_char(ch, " (carried): \r\n");
                    }
                    FIND_OBJ_ROOM => {
                        send_to_char(ch, " (here): \r\n");
                    }
                    FIND_OBJ_EQUIP => {
                        send_to_char(ch, " (used): \r\n");
                    }
                    _ => {}
                }

                list_obj_to_char(
                    db,
                    &obj.as_ref().unwrap().contains.borrow(),
                    ch,
                    SHOW_OBJ_SHORT,
                    true,
                );
            }
        } else {
            /* item must be a fountain or drink container */
            if obj.as_ref().unwrap().get_obj_val(1) <= 0 {
                send_to_char(ch, "It is empty.\r\n");
            } else {
                if obj.as_ref().unwrap().get_obj_val(0) <= 0
                    || obj.as_ref().unwrap().get_obj_val(1) > obj.as_ref().unwrap().get_obj_val(0)
                {
                    send_to_char(ch, "Its contents seem somewhat murky.\r\n");
                    /* BUG */
                } else {
                    let mut buf2 = String::new();
                    let amt = obj.as_ref().unwrap().get_obj_val(1) * 3
                        / obj.as_ref().unwrap().get_obj_val(0);
                    sprinttype(
                        obj.as_ref().unwrap().get_obj_val(2),
                        &COLOR_LIQUID,
                        &mut buf2,
                    );
                    send_to_char(
                        ch,
                        format!(
                            "It's {}full of a {} liquid.\r\n",
                            FULLNESS[amt as usize], buf2
                        )
                        .as_str(),
                    );
                }
            }
        }
    }
}

fn find_exdesc(word: &str, list: &Vec<ExtraDescrData>) -> Option<String> {
    for i in list {
        if isname(word, i.keyword.as_str()) {
            return Some(i.description.clone());
        }
    }
    None
}

/*
 * Given the argument "look at <target>", figure out what object or char
 * matches the target.  First, see if there is another char in the room
 * with the name.  Then check local objs for exdescs.
 *
 * Thanks to Angus Mezick <angus@EDGIL.CCMAIL.COMPUSERVE.COM> for the
 * suggested fix to this problem.
 */
fn look_at_target(db: &DB, ch: &Rc<CharData>, arg: &str) {
    let mut i = 0;
    let mut found = false;
    let mut found_char: Option<Rc<CharData>> = None;
    let mut found_obj: Option<Rc<ObjData>> = None;

    if ch.desc.borrow().is_none() {
        return;
    }

    if arg.is_empty() {
        send_to_char(ch, "Look at what?\r\n");
        return;
    }

    let bits = db.generic_find(
        arg,
        (FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_OBJ_EQUIP | FIND_CHAR_ROOM) as i64,
        ch,
        &mut found_char,
        &mut found_obj,
    );

    /* Is the target a character? */
    if found_char.is_some() {
        let found_char = found_char.as_ref().unwrap();
        look_at_char(db, found_char, ch);
        if !Rc::ptr_eq(ch, found_char) {
            if db.can_see(found_char, ch) {
                db.act(
                    "$n looks at you.",
                    true,
                    Some(ch),
                    None,
                    Some(found_char),
                    TO_VICT,
                );
            }
            db.act(
                "$n looks at $N.",
                true,
                Some(ch),
                None,
                Some(found_char),
                TO_NOTVICT,
            );
        }
        return;
    }
    let mut arg = arg.to_string();
    let fnum = get_number(&mut arg);
    /* Strip off "number." from 2.foo and friends. */
    if fnum == 0 {
        send_to_char(ch, "Look at what?\r\n");
        return;
    }

    /* Does the argument match an extra desc in the room? */
    let desc = find_exdesc(
        &arg,
        &db.world.borrow()[ch.in_room() as usize].ex_descriptions,
    );
    if desc.is_some() {
        i += 1;
        if i == fnum {
            page_string(
                ch.desc.borrow().as_ref(),
                desc.as_ref().unwrap().as_str(),
                false,
            );
            return;
        }
    }

    /* Does the argument match an extra desc in the char's equipment? */
    for j in 0..NUM_WEARS {
        if ch.get_eq(j).is_some() && db.can_see_obj(ch, ch.get_eq(j).as_ref().unwrap()) {
            let desc = find_exdesc(&arg, &ch.get_eq(j).as_ref().unwrap().ex_descriptions);
            if desc.is_some() {
                i += 1;
                if i == fnum {
                    send_to_char(ch, desc.as_ref().unwrap());
                    found = true;
                }
            }
        }
    }

    /* Does the argument match an extra desc in the char's inventory? */
    for obj in ch.carrying.borrow().iter() {
        if db.can_see_obj(ch, obj) {
            let desc = find_exdesc(&arg, &obj.ex_descriptions);
            if desc.is_some() {
                i += 1;
                if i == fnum {
                    send_to_char(ch, desc.as_ref().unwrap());
                    found = true;
                }
            }
        }
    }

    /* Does the argument match an extra desc of an object in the room? */
    for obj in db.world.borrow()[ch.in_room() as usize]
        .contents
        .borrow()
        .iter()
    {
        if db.can_see_obj(ch, obj) {
            if let Some(desc) = find_exdesc(&arg, &obj.ex_descriptions) {
                i += 1;
                if i == fnum {
                    send_to_char(ch, desc.as_str());
                    found = true;
                }
            }
        }
    }

    /* If an object was found back in generic_find */
    if bits != 0 {
        if !found {
            show_obj_to_char(found_obj.as_ref().unwrap(), ch, SHOW_OBJ_ACTION);
        } else {
            show_obj_modifiers(found_obj.as_ref().unwrap(), ch);
            send_to_char(ch, "\r\n");
        }
    } else if !found {
        send_to_char(ch, "You do not see that here.\r\n");
    }
}

pub fn do_look(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    if ch.desc.borrow().is_none() {
        return;
    }
    let db = &game.db;
    if ch.get_pos() < POS_SLEEPING {
        send_to_char(ch, "You can't see anything but stars!\r\n");
    } else if ch.aff_flagged(AFF_BLIND) {
        send_to_char(ch, "You can't see a damned thing, you're blind!\r\n");
    } else if db.is_dark(ch.in_room()) && !ch.can_see_in_dark() {
        send_to_char(ch, "It is pitch black...\r\n");
        list_char_to_char(
            db,
            &db.world.borrow()[ch.in_room() as usize].peoples.borrow(),
            ch,
        );
        /* glowing red eyes */
    } else {
        let mut argument = argument.to_string();
        let mut arg = String::new();
        let mut arg2 = String::new();

        half_chop(&mut argument, &mut arg, &mut arg2);

        if subcmd == SCMD_READ {
            if arg.is_empty() {
                send_to_char(ch, "Read what?\r\n");
            } else {
                look_at_target(&game.db, ch, &mut arg);
            }
            return;
        }
        let look_type;
        if arg.is_empty() {
            /* "look" alone, without an argument at all */
            look_at_room(db, ch, true);
        } else if is_abbrev(arg.as_ref(), "in") {
            look_in_obj(&game.db, ch, arg2.as_str());
            /* did the char type 'look <direction>?' */
        } else if {
            look_type = search_block(arg.as_str(), &DIRS, false);
            look_type
        } != None
        {
            look_in_direction(&game.db, ch, look_type.unwrap() as i32);
        } else if is_abbrev(arg.as_ref(), "at") {
            look_at_target(&game.db, ch, arg2.as_ref());
        } else {
            look_at_target(&game.db, ch, arg.as_ref());
        }
    }
}

pub fn do_examine(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    // struct char_data *tmp_char;
    // struct obj_data *tmp_object;
    // char tempsave[MAX_INPUT_LENGTH], arg[MAX_INPUT_LENGTH];

    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(ch, "Examine what?\r\n");
        return;
    }

    /* look_at_target() eats the number. */
    look_at_target(&game.db, ch, &arg);
    let mut tmp_char = None;
    let mut tmp_object = None;
    game.db.generic_find(
        &arg,
        (FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_CHAR_ROOM | FIND_OBJ_EQUIP) as i64,
        ch,
        &mut tmp_char,
        &mut tmp_object,
    );

    if tmp_object.is_some() {
        let tmp_object = tmp_object.unwrap();
        if tmp_object.get_obj_type() == ITEM_DRINKCON
            || tmp_object.get_obj_type() == ITEM_FOUNTAIN
            || tmp_object.get_obj_type() == ITEM_CONTAINER
        {
            send_to_char(ch, "When you look inside, you see:\r\n");
            look_in_obj(&game.db, ch, &arg);
        }
    }
}

pub fn do_gold(_game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.get_gold() == 0 {
        send_to_char(ch, "You're broke!\r\n");
    } else if ch.get_gold() == 1 {
        send_to_char(ch, "You have one miserable little gold coin.\r\n");
    } else {
        send_to_char(
            ch,
            format!("You have {} gold coins.\r\n", ch.get_gold()).as_str(),
        );
    }
}

pub fn do_score(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.is_npc() {
        return;
    }

    send_to_char(
        ch,
        format!("You are {} years old.\r\n", ch.get_age()).as_str(),
    );

    if age(ch).month == 0 && age(ch).day == 0 {
        send_to_char(ch, "  It's your birthday today.\r\n");
    } else {
        send_to_char(ch, "\r\n");
    }

    send_to_char(
        ch,
        format!(
            "You have {}({}) hit, {}({}) mana and {}({}) movement points.\r\n",
            ch.get_hit(),
            ch.get_max_hit(),
            ch.get_mana(),
            ch.get_max_mana(),
            ch.get_move(),
            ch.get_max_move()
        )
        .as_str(),
    );

    send_to_char(
        ch,
        format!(
            "Your armor class is {}/10, and your alignment is {}.\r\n",
            compute_armor_class(ch),
            ch.get_alignment()
        )
        .as_str(),
    );

    send_to_char(
        ch,
        format!(
            "You have scored {} exp, and have {} gold coins.\r\n",
            ch.get_exp(),
            ch.get_gold()
        )
        .as_str(),
    );

    if ch.get_level() < LVL_IMMORT as u8 {
        send_to_char(
            ch,
            format!(
                "You need {} exp to reach your next level.\r\n",
                level_exp(ch.get_class(), (ch.get_level() + 1) as i16) - ch.get_exp()
            )
            .as_str(),
        );
    }

    let playing_time = real_time_passed(
        (time_now() - ch.player.borrow().time.logon) + ch.player.borrow().time.played as u64,
        0,
    );
    send_to_char(
        ch,
        format!(
            "You have been playing for {} day{} and {} hour{}.\r\n",
            playing_time.day,
            if playing_time.day == 1 { "" } else { "s" },
            playing_time.hours,
            if playing_time.hours == 1 { "" } else { "s" }
        )
        .as_str(),
    );

    send_to_char(
        ch,
        format!(
            "This ranks you as {} {} (level {}).\r\n",
            ch.get_name(),
            ch.get_title(),
            ch.get_level()
        )
        .as_str(),
    );

    match ch.get_pos() {
        POS_DEAD => {
            send_to_char(ch, "You are DEAD!\r\n");
        }
        POS_MORTALLYW => {
            send_to_char(ch, "You are mortally wounded!  You should seek help!\r\n");
        }
        POS_INCAP => {
            send_to_char(ch, "You are incapacitated, slowly fading away...\r\n");
        }
        POS_STUNNED => {
            send_to_char(ch, "You are stunned!  You can't move!\r\n");
        }
        POS_SLEEPING => {
            send_to_char(ch, "You are sleeping.\r\n");
        }
        POS_RESTING => {
            send_to_char(ch, "You are resting.\r\n");
        }
        POS_SITTING => {
            send_to_char(ch, "You are sitting.\r\n");
        }
        POS_FIGHTING => {
            let v = game.db.pers(ch.fighting().as_ref().unwrap(), ch);
            send_to_char(
                ch,
                format!(
                    "You are fighting {}.\r\n",
                    if ch.fighting().is_some() {
                        v.as_ref()
                    } else {
                        "thin air"
                    }
                )
                .as_str(),
            );
        }
        POS_STANDING => {
            send_to_char(ch, "You are standing.\r\n");
        }
        _ => {
            send_to_char(ch, "You are floating.\r\n");
        }
    }

    if ch.get_cond(DRUNK) > 10 {
        send_to_char(ch, "You are intoxicated.\r\n");
    }
    if ch.get_cond(FULL) == 0 {
        send_to_char(ch, "You are hungry.\r\n");
    }
    if ch.get_cond(THIRST) == 0 {
        send_to_char(ch, "You are thirsty.\r\n");
    }
    if ch.aff_flagged(AFF_BLIND) {
        send_to_char(ch, "You have been blinded!\r\n");
    }
    if ch.aff_flagged(AFF_INVISIBLE) {
        send_to_char(ch, "You are invisible.\r\n");
    }
    if ch.aff_flagged(AFF_DETECT_INVIS) {
        send_to_char(
            ch,
            "You are sensitive to the presence of invisible things.\r\n",
        );
    }
    if ch.aff_flagged(AFF_SANCTUARY) {
        send_to_char(ch, "You are protected by Sanctuary.\r\n");
    }
    if ch.aff_flagged(AFF_POISON) {
        send_to_char(ch, "You are poisoned!\r\n");
    }
    if ch.aff_flagged(AFF_CHARM) {
        send_to_char(ch, "You have been charmed!\r\n");
    }

    if affected_by_spell(ch, SPELL_ARMOR as i16) {
        send_to_char(ch, "You feel protected.\r\n");
    }

    if ch.aff_flagged(AFF_INFRAVISION) {
        send_to_char(ch, "Your eyes are glowing red.\r\n");
    }
    if ch.aff_flagged(PRF_SUMMONABLE) {
        send_to_char(ch, "You are summonable by other players.\r\n");
    }
}

pub fn do_inventory(
    game: &mut Game,
    ch: &Rc<CharData>,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    send_to_char(ch, "You are carrying:\r\n");
    list_obj_to_char(
        &game.db,
        ch.carrying.borrow().as_ref(),
        ch,
        SHOW_OBJ_SHORT,
        true,
    );
}

pub fn do_equipment(
    game: &mut Game,
    ch: &Rc<CharData>,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let mut found = false;
    send_to_char(ch, "You are using:\r\n");
    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some() {
            if game.db.can_see_obj(ch, ch.get_eq(i).as_ref().unwrap()) {
                send_to_char(ch, format!("{}", WEAR_WHERE[i as usize]).as_str());
                show_obj_to_char(ch.get_eq(i).as_ref().unwrap(), ch, SHOW_OBJ_SHORT);
                found = true;
            } else {
                send_to_char(ch, format!("{}", WEAR_WHERE[i as usize]).as_str());
                send_to_char(ch, "Something.\r\n");
                found = true;
            }
        }
    }
    if !found {
        send_to_char(ch, " Nothing.\r\n");
    }
}

pub fn do_time(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    /* day in [1..35] */
    let day = game.db.time_info.borrow().day + 1;

    /* 35 days in a month, 7 days a week */
    let weekday = ((35 * game.db.time_info.borrow().month) + day) % 7;

    send_to_char(
        ch,
        format!(
            "It is {} o'clock {}, on {}.\r\n",
            if game.db.time_info.borrow().hours % 12 == 0 {
                12
            } else {
                game.db.time_info.borrow().hours % 12
            },
            if game.db.time_info.borrow().hours >= 12 {
                "pm"
            } else {
                "am"
            },
            WEEKDAYS[weekday as usize]
        )
        .as_str(),
    );

    /*
     * Peter Ajamian <peter@PAJAMIAN.DHS.ORG> supplied the following as a fix
     * for a bug introduced in the ordinal display that caused 11, 12, and 13
     * to be incorrectly displayed as 11st, 12nd, and 13rd.  Nate Winters
     * <wintersn@HOTMAIL.COM> had already submitted a fix, but it hard-coded a
     * limit on ordinal display which I want to avoid.	-dak
     */

    let mut suf = "th";

    if ((day % 100) / 10) != 1 {
        match day % 10 {
            1 => {
                suf = "st";
            }
            2 => {
                suf = "nd";
            }
            3 => {
                suf = "rd";
            }
            _ => {}
        }
    }

    send_to_char(
        ch,
        format!(
            "The {}{} Day of the {}, Year {}.\r\n",
            day,
            suf,
            MONTH_NAME[game.db.time_info.borrow().month as usize],
            game.db.time_info.borrow().year
        )
        .as_str(),
    );
}

pub fn do_weather(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    const SKY_LOOK: [&str; 4] = [
        "cloudless",
        "cloudy",
        "rainy",
        "lit by flashes of lightning",
    ];
    let db = &game.db;
    if game.db.outside(ch) {
        send_to_char(
            ch,
            format!(
                "The sky is {} and {}.\r\n",
                SKY_LOOK[db.weather_info.borrow().sky as usize],
                if db.weather_info.borrow().change >= 0 {
                    "you feel a warm wind from south"
                } else {
                    "your foot tells you bad weather is due"
                }
            )
            .as_str(),
        );
        if ch.get_level() >= LVL_GOD as u8 {
            send_to_char(
                ch,
                format!(
                    "Pressure: {} (change: {}), Sky: {} ({})\r\n",
                    db.weather_info.borrow().pressure,
                    db.weather_info.borrow().change,
                    db.weather_info.borrow().sky,
                    SKY_LOOK[db.weather_info.borrow().sky as usize],
                )
                .as_str(),
            );
        }
    } else {
        send_to_char(ch, "You have no feeling about the weather at all.\r\n");
    }
}

pub fn do_help(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.desc.borrow().is_none() {
        return;
    }

    let argument = argument.trim_start();

    if argument.len() == 0 {
        page_string(ch.desc.borrow().as_ref(), &game.db.help, false);
        return;
    }
    if game.db.help_table.len() == 0 {
        send_to_char(ch, "No help available.\r\n");
        return;
    }

    let mut bot = 0;
    let mut top = game.db.help_table.len() - 1;

    loop {
        let mut mid = (bot + top) / 2;
        if bot > top {
            send_to_char(ch, "There is no help on that word.\r\n");
            return;
        } else if game.db.help_table[mid].keyword.starts_with(argument) {
            /* trace backwards to find first matching entry. Thanks Jeff Fink! */
            while mid > 0 && game.db.help_table[mid - 1].keyword.starts_with(argument) {
                mid -= 1;
            }
            page_string(
                ch.desc.borrow().as_ref(),
                &game.db.help_table[mid].entry,
                false,
            );
            return;
        } else {
            if game.db.help_table[mid].keyword.as_ref() < argument {
                bot = mid + 1;
            } else {
                top = mid - 1;
            }
        }
    }
}

const WHO_FORMAT: &str =
    "format: who [minlev[-maxlev]] [-n name] [-c classlist] [-s] [-o] [-q] [-r] [-z]\r\n";

/* FIXME: This whole thing just needs rewritten. */
pub fn do_who(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let argument = argument.trim_start();
    let mut buf = argument.to_string();
    let mut low = 0 as i16;
    let mut high = LVL_IMPL;
    let mut outlaws = false;
    let mut localwho = false;
    let mut short_list = false;
    let mut questwho = false;
    let mut name_search = String::new();
    let mut who_room = false;
    let mut showclass = 0;

    while !buf.is_empty() {
        let mut arg = String::new();
        let mut buf1 = String::new();

        half_chop(&mut buf, &mut arg, &mut buf1);
        if arg.chars().next().unwrap().is_digit(10) {
            let regex = Regex::new(r"^(\d{1,9})-(\d{1,9})").unwrap();
            let f = regex.captures(&arg);
            if f.is_some() {
                let f = f.unwrap();
                low = f[1].parse::<i16>().unwrap();
                high = f[2].parse::<i16>().unwrap();
            }
        } else if arg.starts_with('-') {
            arg.remove(0);
            let mode = arg.chars().next().unwrap(); /* just in case; we destroy arg in the switch */
            match mode {
                'o' | 'k' => {
                    outlaws = true;
                    buf.push_str(&buf1);
                }
                'z' => {
                    localwho = true;
                    buf.push_str(&buf1);
                }
                's' => {
                    short_list = true;
                    buf.push_str(&buf1);
                }
                'q' => {
                    questwho = true;
                    buf.push_str(&buf1);
                }
                'l' => {
                    half_chop(&mut buf1, &mut arg, &mut buf);
                    let regex = Regex::new(r"^(\d{1,9})-(\d{1,9})").unwrap();
                    let f = regex.captures(&arg);
                    if f.is_some() {
                        let f = f.unwrap();
                        low = f[1].parse::<i16>().unwrap();
                        high = f[2].parse::<i16>().unwrap();
                    }
                }
                'n' => {
                    half_chop(&mut buf1, &mut name_search, &mut buf);
                }
                'r' => {
                    who_room = true;
                    buf.push_str(&buf1);
                }
                'c' => {
                    half_chop(&mut buf1, &mut arg, &mut buf);
                    showclass = find_class_bitvector(&arg);
                }
                _ => {
                    send_to_char(ch, WHO_FORMAT);
                    return;
                }
            }
        } else {
            /* endif */
            send_to_char(ch, WHO_FORMAT);
            return;
        }
    } /* end while (parser) */

    send_to_char(ch, "Players\r\n-------\r\n");
    let db = &game.db;
    let mut num_can_see = 0;

    for d in game.descriptor_list.borrow().iter() {
        if d.state() != ConPlaying {
            continue;
        }

        let tch;
        let character;
        if d.original.borrow().is_some() {
            character = d.original.borrow();
            tch = character.as_ref();
        } else if {
            character = d.character.borrow();
            tch = character.as_ref();
            tch.is_none()
        } {
            continue;
        }
        let tch = tch.unwrap();

        if !name_search.is_empty()
            && tch.get_name().as_ref() != &name_search
            && !tch.get_title().contains(&name_search)
        {
            continue;
        }
        if !db.can_see(ch, tch) || tch.get_level() < low as u8 || tch.get_level() > high as u8 {
            continue;
        }
        if outlaws && !tch.plr_flagged(PLR_KILLER) && !tch.plr_flagged(PLR_THIEF) {
            continue;
        }
        if questwho && !tch.prf_flagged(PRF_QUEST) {
            continue;
        }
        if localwho
            && db.world.borrow()[ch.in_room() as usize].zone
                != db.world.borrow()[tch.in_room() as usize].zone
        {
            continue;
        }
        if who_room && tch.in_room() != ch.in_room() {
            continue;
        }
        if showclass != 0 && (showclass & (1 << tch.get_class())) == 0 {
            continue;
        }
        if short_list {
            send_to_char(
                ch,
                format!(
                    "{}[{:2} {}] {:12}{}{}",
                    if tch.get_level() >= LVL_IMMORT as u8 {
                        CCYEL!(ch, C_SPR)
                    } else {
                        ""
                    },
                    tch.get_level(),
                    tch.class_abbr(),
                    tch.get_name(),
                    if tch.get_level() >= LVL_IMMORT as u8 {
                        CCNRM!(ch, C_SPR)
                    } else {
                        ""
                    },
                    if {
                        num_can_see += 1;
                        num_can_see % 4 == 0
                    } {
                        "\r\n"
                    } else {
                        ""
                    }
                )
                .as_str(),
            );
        } else {
            num_can_see += 1;
            send_to_char(
                ch,
                format!(
                    "{}[{:2} {}] {} {}",
                    if tch.get_level() >= LVL_IMMORT as u8 {
                        CCYEL!(ch, C_SPR)
                    } else {
                        ""
                    },
                    tch.get_level(),
                    tch.class_abbr(),
                    tch.get_name(),
                    tch.get_title()
                )
                .as_str(),
            );

            if tch.get_invis_lev() != 0 {
                send_to_char(ch, format!(" (i{})", tch.get_invis_lev()).as_str());
            } else if tch.aff_flagged(AFF_INVISIBLE) {
                send_to_char(ch, " (invis)");
            }

            if tch.plr_flagged(PLR_MAILING) {
                send_to_char(ch, " (mailing)");
            } else if tch.plr_flagged(PLR_WRITING) {
                send_to_char(ch, " (writing)");
            }

            if tch.plr_flagged(PRF_DEAF) {
                send_to_char(ch, " (deaf)");
            }
            if tch.prf_flagged(PRF_NOTELL) {
                send_to_char(ch, " (notell)");
            }
            if tch.prf_flagged(PRF_QUEST) {
                send_to_char(ch, " (quest)");
            }
            if tch.plr_flagged(PLR_THIEF) {
                send_to_char(ch, " (THIEF)");
            }
            if tch.plr_flagged(PLR_KILLER) {
                send_to_char(ch, " (KILLER)");
            }
            if tch.get_level() >= LVL_IMMORT as u8 {
                send_to_char(ch, CCNRM!(ch, C_SPR));
            }
            send_to_char(ch, "\r\n");
        } /* endif shortlist */
    } /* end of for */
    if short_list && (num_can_see % 4) != 0 {
        send_to_char(ch, "\r\n");
    }
    if num_can_see == 0 {
        send_to_char(ch, "\r\nNobody at all!\r\n");
    } else if num_can_see == 1 {
        send_to_char(ch, "\r\nOne lonely character displayed.\r\n");
    } else {
        send_to_char(
            ch,
            format!("\r\n{} characters displayed.\r\n", num_can_see).as_str(),
        );
    }
}

const USERS_FORMAT: &str =
    "format: users [-l minlevel[-maxlevel]] [-n name] [-h host] [-c classlist] [-o] [-p]\r\n";

/* BIG OL' FIXME: Rewrite it all. Similar to do_who(). */
pub fn do_users(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = argument.to_string();
    let mut arg = String::new();
    let mut outlaws = false;
    let mut playing = false;
    let mut deadweight = false;
    let mut low = 0;
    let mut high = LVL_IMPL;
    let mut name_search = String::new();
    let mut host_search = String::new();
    let mut showclass = 0;
    let mut num_can_see = 0;
    let mut classname;

    while !buf.is_empty() {
        let mut buf1 = String::new();

        half_chop(&mut buf, &mut arg, &mut buf1);
        if arg.starts_with('-') {
            arg.remove(0);
            let mode = arg.chars().next().unwrap(); /* just in case; we destroy arg in the switch */
            match mode {
                'o' | 'k' => {
                    outlaws = true;
                    playing = true;
                    buf.push_str(&buf1);
                }
                'p' => {
                    playing = true;
                    buf.push_str(&buf1);
                }
                'd' => {
                    deadweight = true;
                    buf.push_str(&buf1);
                }
                'l' => {
                    playing = true;
                    half_chop(&mut buf1, &mut arg, &mut buf);
                    let regex = Regex::new(r"^(\d{1,9})-(\d{1,9})").unwrap();
                    let f = regex.captures(&arg);
                    if f.is_some() {
                        let f = f.unwrap();
                        low = f[1].parse::<i16>().unwrap();
                        high = f[2].parse::<i16>().unwrap();
                    }
                }
                'n' => {
                    playing = true;
                    half_chop(&mut buf1, &mut name_search, &mut buf);
                }
                'h' => {
                    playing = true;
                    half_chop(&mut buf1, &mut host_search, &mut buf);
                }
                'c' => {
                    playing = true;
                    half_chop(&mut buf1, &mut arg, &mut buf);
                    showclass = find_class_bitvector(&arg);
                }
                _ => {
                    send_to_char(ch, USERS_FORMAT);
                    return;
                }
            } /* end of switch */
        } else {
            /* endif */
            send_to_char(ch, USERS_FORMAT);
            return;
        }
    } /* end while (parser) */
    send_to_char(
        ch,
        "Num Class   Name         State          Idl Login@   Site\r\n\
--- ------- ------------ -------------- --- -------- ------------------------\r\n",
    );

    one_argument(argument, &mut arg);
    let db = &game.db;
    for d in game.descriptor_list.borrow().iter() {
        if d.state() != ConPlaying && playing {
            continue;
        }
        if d.state() == ConPlaying && deadweight {
            continue;
        }
        if d.state() == ConPlaying {
            let character;
            if d.original.borrow().is_some() {
                character = d.original.borrow();
            } else if {
                character = d.character.borrow();
                character.is_none()
            } {
                continue;
            }
            let tch = character.as_ref().unwrap();

            if !host_search.is_empty() && !d.host.contains(&host_search) {
                continue;
            }
            if !name_search.is_empty() && tch.get_name().as_ref() != &name_search {
                continue;
            }
            if !db.can_see(ch, tch) || tch.get_level() < low as u8 || tch.get_level() > high as u8 {
                continue;
            }
            if outlaws && !tch.plr_flagged(PLR_KILLER) && !tch.plr_flagged(PLR_THIEF) {
                continue;
            }
            if showclass != 0 && (showclass & (1 << tch.get_class())) == 0 {
                continue;
            }
            if ch.get_invis_lev() > ch.get_level() as i16 {
                continue;
            }

            if d.original.borrow().is_some() {
                classname = format!(
                    "[{:2} {}]",
                    d.original.borrow().as_ref().unwrap().get_level(),
                    d.original.borrow().as_ref().unwrap().class_abbr()
                );
            } else {
                classname = format!(
                    "[{:2} {}]",
                    d.character.borrow().as_ref().unwrap().get_level(),
                    d.character.borrow().as_ref().unwrap().class_abbr()
                );
            }
        } else {
            classname = "   -   ".to_string();
        }

        let timeptr = d.login_time.elapsed().as_secs().to_string();

        let state;
        if d.state() == ConPlaying && d.original.borrow().is_some() {
            state = "Switched";
        } else {
            state = CONNECTED_TYPES[d.state() as usize];
        }

        let idletime;
        if d.character.borrow().is_some()
            && d.state() == ConPlaying
            && d.character.borrow().as_ref().unwrap().get_level() < LVL_GOD as u8
        {
            idletime = format!(
                "{:3}",
                d.character
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .char_specials
                    .borrow()
                    .timer
                    .get()
                    * SECS_PER_MUD_HOUR as i32
                    / SECS_PER_REAL_MIN as i32
            );
        } else {
            idletime = "".to_string();
        }

        let mut line = format!(
            "{:3} {:7} {:12} {:14} {:3} {:8} ",
            d.desc_num.get(),
            classname,
            if d.original.borrow().is_some()
                && !d
                    .original
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .player
                    .borrow()
                    .name
                    .is_empty()
            {
                d.original
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .player
                    .borrow()
                    .name
                    .clone()
            } else if d.character.borrow().is_some()
                && !d
                    .character
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .player
                    .borrow()
                    .name
                    .is_empty()
            {
                d.character
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .player
                    .borrow()
                    .name
                    .clone()
            } else {
                "UNDEFINED".to_string()
            },
            state,
            idletime,
            timeptr
        );

        if !d.host.is_empty() {
            line.push_str(&format!("[{}]\r\n", d.host));
        } else {
            line.push_str("[Hostname unknown]\r\n");
        }

        if d.state() != ConPlaying {
            line.push_str(&format!("{}{}{}", CCGRN!(ch, C_SPR), line, CCNRM!(ch, C_SPR)));
        }
        if d.state() != ConPlaying
            || (d.state() == ConPlaying && db.can_see(ch, d.character.borrow().as_ref().unwrap()))
        {
            send_to_char(ch, &line);
            num_can_see += 1;
        }
    }

    send_to_char(
        ch,
        format!("\r\n{} visible sockets connected.\r\n", num_can_see).as_str(),
    );
}

/* Generic page_string function for displaying text */
pub fn do_gen_ps(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, subcmd: i32) {
    match subcmd {
        SCMD_CREDITS => {
            page_string(ch.desc.borrow().as_ref(), &game.db.credits, false);
        }
        SCMD_NEWS => {
            page_string(ch.desc.borrow().as_ref(), &game.db.news, false);
        }
        SCMD_INFO => {
            page_string(ch.desc.borrow().as_ref(), &game.db.info, false);
        }
        SCMD_WIZLIST => {
            page_string(ch.desc.borrow().as_ref(), &game.db.wizlist, false);
        }
        SCMD_IMMLIST => {
            page_string(ch.desc.borrow().as_ref(), &game.db.immlist, false);
        }
        SCMD_HANDBOOK => {
            page_string(ch.desc.borrow().as_ref(), &game.db.handbook, false);
        }
        SCMD_POLICIES => {
            page_string(ch.desc.borrow().as_ref(), &game.db.policies, false);
        }
        SCMD_MOTD => {
            page_string(ch.desc.borrow().as_ref(), &game.db.motd, false);
        }
        SCMD_IMOTD => {
            page_string(ch.desc.borrow().as_ref(), &game.db.imotd, false);
        }
        SCMD_CLEAR => {
            send_to_char(ch, "\x21[H\x21[J");
        }
        SCMD_VERSION => {
            send_to_char(ch, format!("{}\r\n", CIRCLEMUD_VERSION).as_str());
        }
        SCMD_WHOAMI => {
            send_to_char(ch, format!("{}\r\n", ch.get_name()).as_str());
        }
        _ => {
            error!("SYSERR: Unhandled case in do_gen_ps. ({})", subcmd);
            return;
        }
    }
}

fn perform_mortal_where(game: &mut Game, ch: &Rc<CharData>, arg: &str) {
    if arg.is_empty() {
        send_to_char(ch, "Players in your Zone\r\n--------------------\r\n");
        for d in game.descriptor_list.borrow().iter() {
            if d.state() != ConPlaying
                || (d.character.borrow().is_some()
                    && Rc::ptr_eq(d.character.borrow().as_ref().unwrap(), ch))
            {
                continue;
            }
            let i;
            if {
                i = if d.original.borrow().is_some() {
                    d.original.borrow()
                } else {
                    d.character.borrow()
                };
                i.is_none()
            } {
                continue;
            }
            let i = i.as_ref().unwrap();
            if i.in_room() == NOWHERE || !game.db.can_see(ch, i) {
                continue;
            }
            if game.db.world.borrow()[ch.in_room() as usize].zone
                != game.db.world.borrow()[i.in_room() as usize].zone
            {
                continue;
            }
            send_to_char(
                ch,
                format!(
                    "%{:20} - {}\r\n",
                    i.get_name(),
                    game.db.world.borrow()[i.in_room() as usize].name
                )
                .as_str(),
            );
        }
    } else {
        /* print only FIRST char, not all. */
        for i in game.db.character_list.borrow().iter() {
            if i.in_room() == NOWHERE || Rc::ptr_eq(i, ch) {
                continue;
            }
            if !game.db.can_see(ch, i)
                || game.db.world.borrow()[i.in_room() as usize].zone
                    != game.db.world.borrow()[ch.in_room() as usize].zone
            {
                continue;
            }
            if !isname(arg, &i.player.borrow().name) {
                continue;
            }
            send_to_char(
                ch,
                format!(
                    "{:25} - {}\r\n",
                    i.get_name(),
                    game.db.world.borrow()[i.in_room() as usize].name
                )
                .as_str(),
            );
            return;
        }
        send_to_char(ch, "Nobody around by that name.\r\n");
    }
}

fn print_object_location(db: &DB, num: i32, obj: &Rc<ObjData>, ch: &Rc<CharData>, recur: bool) {
    if num > 0 {
        send_to_char(
            ch,
            format!("O{:3}. {:25} - ", num, obj.short_description).as_str(),
        );
    } else {
        send_to_char(ch, format!("{:33}", " - ").as_str());
    }

    if obj.in_room.get() != NOWHERE {
        send_to_char(
            ch,
            format!(
                "[{:5}] {}\r\n",
                db.get_room_vnum(obj.in_room()),
                db.world.borrow()[obj.in_room() as usize].name
            )
            .as_str(),
        );
    } else if obj.carried_by.borrow().is_some() {
        send_to_char(
            ch,
            format!(
                "carried by {}\r\n",
                db.pers(obj.carried_by.borrow().as_ref().unwrap(), ch)
            )
            .as_str(),
        );
    } else if obj.worn_by.borrow().is_some() {
        send_to_char(
            ch,
            format!(
                "worn by {}\r\n",
                db.pers(obj.worn_by.borrow().as_ref().unwrap(), ch)
            )
            .as_str(),
        );
    } else if obj.in_obj.borrow().is_some() {
        send_to_char(
            ch,
            format!(
                "inside {}{}\r\n",
                obj.in_obj.borrow().as_ref().unwrap().short_description,
                if recur { ", which is" } else { " " }
            )
            .as_str(),
        );
        if recur {
            print_object_location(db, 0, obj.in_obj.borrow().as_ref().unwrap(), ch, recur);
        }
    } else {
        send_to_char(ch, "in an unknown location\r\n");
    }
}

fn perform_immort_where(game: &mut Game, ch: &Rc<CharData>, arg: &str) {
    if arg.is_empty() {
        send_to_char(ch, "Players\r\n-------\r\n");
        for d in game.descriptor_list.borrow().iter() {
            if d.state() == ConPlaying {
                let oi = if d.original.borrow().is_some() {
                    d.original.borrow()
                } else {
                    d.character.borrow()
                };
                if oi.is_none() {
                    continue;
                }

                let i = oi.as_ref().unwrap();
                if game.db.can_see(ch, i) && (i.in_room() != NOWHERE) {
                    if d.original.borrow().is_some() {
                        send_to_char(
                            ch,
                            format!(
                                "{:20} - [{:5}] {} (in {})\r\n",
                                i.get_name(),
                                game.db.get_room_vnum(
                                    d.character.borrow().as_ref().unwrap().in_room.get()
                                ),
                                game.db.world.borrow()
                                    [d.character.borrow().as_ref().unwrap().in_room.get() as usize]
                                    .name,
                                d.character.borrow().as_ref().unwrap().get_name()
                            )
                            .as_str(),
                        );
                    } else {
                        send_to_char(
                            ch,
                            format!(
                                "{:20} - [{:5}] {}\r\n",
                                i.get_name(),
                                game.db.get_room_vnum(i.in_room()),
                                game.db.world.borrow()[i.in_room() as usize].name
                            )
                            .as_str(),
                        );
                    }
                }
            }
        }
    } else {
        let mut found = false;
        let mut num = 0;
        for i in game.db.character_list.borrow().iter() {
            if game.db.can_see(ch, i)
                && i.in_room() != NOWHERE
                && isname(arg, &i.player.borrow().name)
            {
                found = true;
                send_to_char(
                    ch,
                    format!(
                        "M{:3}. {:25} - [{:5}] {}\r\n",
                        {
                            num += 1;
                            num
                        },
                        i.get_name(),
                        game.db.get_room_vnum(i.in_room()),
                        game.db.world.borrow()[i.in_room() as usize].name
                    )
                    .as_str(),
                );
            }
        }
        num = 0;
        for k in game.db.object_list.borrow().iter() {
            if game.db.can_see_obj(ch, k) && isname(arg, &k.name.borrow()) {
                found = true;
                print_object_location(
                    &game.db,
                    {
                        num += 1;
                        num
                    },
                    k,
                    ch,
                    true,
                );
            }
        }
        if !found {
            send_to_char(ch, "Couldn't find any such thing.\r\n");
        }
    }
}

pub fn do_where(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if ch.get_level() >= LVL_IMMORT as u8 {
        perform_immort_where(game, ch, &arg);
    } else {
        perform_mortal_where(game, ch, &arg);
    }
}

pub fn do_levels(_game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.is_npc() {
        send_to_char(ch, "You ain't nothin' but a hound-dog.\r\n");
        return;
    }
    let mut buf = String::new();
    for i in 1..LVL_IMMORT {
        buf = format!(
            "[{}] {}-{} : ",
            i,
            level_exp(ch.get_class(), i),
            level_exp(ch.get_class(), i + 1) - 1
        );

        match ch.get_sex() {
            SEX_MALE | SEX_NEUTRAL => {
                buf.push_str(
                    format!("{}\r\n", title_male(ch.get_class() as i32, i as i32)).as_str(),
                );
            }
            SEX_FEMALE => {
                buf.push_str(
                    format!("{}\r\n", title_female(ch.get_class() as i32, i as i32)).as_str(),
                );
            }
            _ => {
                buf.push_str("Oh dear.  You seem to be sexless.\r\n");
            }
        }
    }
    buf.push_str(
        format!(
            "[{}] {}          : Immortality\r\n",
            LVL_IMMORT,
            level_exp(ch.get_class(), LVL_IMMORT)
        )
        .as_str(),
    );
    page_string(ch.desc.borrow().as_ref(), buf.as_str(), true);
}

pub fn do_consider(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    one_argument(argument, &mut buf);

    let victim = game.db.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
    if victim.is_none() {
        send_to_char(ch, "Consider killing who?\r\n");
        return;
    }
    let victim = victim.unwrap();
    if Rc::ptr_eq(&victim, ch) {
        send_to_char(ch, "Easy!  Very easy indeed!\r\n");
        return;
    }
    if !victim.is_npc() {
        send_to_char(ch, "Would you like to borrow a cross and a shovel?\r\n");
        return;
    }
    let diff = victim.get_level() as i32 - ch.get_level() as i32;

    if diff <= -10 {
        send_to_char(ch, "Now where did that chicken go?\r\n");
    } else if diff <= -5 {
        send_to_char(ch, "You could do it with a needle!\r\n");
    } else if diff <= -2 {
        send_to_char(ch, "Easy.\r\n");
    } else if diff <= -1 {
        send_to_char(ch, "Fairly easy.\r\n");
    } else if diff == 0 {
        send_to_char(ch, "The perfect match!\r\n");
    } else if diff <= 1 {
        send_to_char(ch, "You would need some luck!\r\n");
    } else if diff <= 2 {
        send_to_char(ch, "You would need a lot of luck!\r\n");
    } else if diff <= 3 {
        send_to_char(ch, "You would need a lot of luck and great equipment!\r\n");
    } else if diff <= 5 {
        send_to_char(ch, "Do you feel lucky, punk?\r\n");
    } else if diff <= 10 {
        send_to_char(ch, "Are you mad!?\r\n");
    } else if diff <= 100 {
        send_to_char(ch, "You ARE mad!\r\n");
    }
}

pub fn do_diagnose(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let vict;
    if !buf.is_empty() {
        if {
            vict = game.db.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
            vict.is_none()
        } {
            send_to_char(ch, NOPERSON);
        } else {
            diag_char_to_char(&game.db, vict.as_ref().unwrap(), ch);
        }
    } else {
        if ch.fighting().is_some() {
            diag_char_to_char(&game.db, ch.fighting().as_ref().unwrap(), ch);
        } else {
            send_to_char(ch, "Diagnose who?\r\n");
        }
    }
}

const CTYPES: [&str; 5] = ["off", "sparse", "normal", "complete", "\n"];

pub fn do_color(_game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    if ch.is_npc() {
        return;
    }

    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(
            ch,
            format!(
                "Your current color level is {}.\r\n",
                CTYPES[COLOR_LEV!(ch) as usize]
            )
            .as_str(),
        );
        return;
    }
    let tp;
    if {
        tp = search_block(&arg, &CTYPES, false);
        tp.is_none()
    } {
        send_to_char(ch, "Usage: color { Off | Sparse | Normal | Complete }\r\n");
        return;
    }
    let tp = tp.unwrap() as i64;
    ch.remove_prf_flags_bits(PRF_COLOR_1 | PRF_COLOR_2);
    ch.set_prf_flags_bits(PRF_COLOR_1 * (tp & 1) | (PRF_COLOR_2 * (tp & 2) >> 1));
    info!(
        "[DEBUG] {} {}",
        PRF_COLOR_1 * (tp & 1),
        (PRF_COLOR_2 * (tp & 2) >> 1)
    );

    send_to_char(
        ch,
        format!(
            "Your {}color{} is now {}.\r\n",
            CCRED!(ch, C_SPR),
            CCNRM!(ch, C_OFF),
            CTYPES[tp as usize]
        )
        .as_str(),
    );
}

#[macro_export]
macro_rules! onoff {
    ($a:expr) => {
        if ($a) {
            "ON"
        } else {
            "OFF"
        }
    };
}

#[macro_export]
macro_rules! yesno {
    ($a:expr) => {
        if ($a) {
            "YES"
        } else {
            "NO"
        }
    };
}

pub fn do_toggle(_game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    // char buf2[4];
    let mut buf2 = String::new();
    if ch.is_npc() {
        return;
    }

    if ch.get_wimp_lev() == 0 {
        buf2.push_str("OFF");
    } else {
        buf2.push_str(format!("{:3}", ch.get_wimp_lev()).as_str());
    }

    if ch.get_level() >= LVL_IMMORT as u8 {
        send_to_char(
            ch,
            format!(
                "      No Hassle: {:3}    Holylight: {:3}    Room Flags:{:3}\r\n",
                onoff!(ch.prf_flagged(PRF_NOHASSLE)),
                onoff!(ch.prf_flagged(PRF_HOLYLIGHT)),
                onoff!(ch.prf_flagged(PRF_ROOMFLAGS))
            )
            .as_str(),
        );
    }

    send_to_char(
        ch,
        format!(
            "Hit Pnt Display: {:3}    Brief Mode: {:3}    Summon Protect: {:3}\r\n\
 Move Display: {:3}    Compact Mode: {:3}    On Quest: {:3}\r\n\
 Mana Display: {:3}    NoTell: {:3}    Repeat Comm.: {:3}\r\n\
 Auto Show Exit: {:3}    Deaf: {:3}    Wimp Level: {:3}\r\n\
 Gossip Channel: {:3}    Auction Channel: {:3}    Grats Channel: {:3}\r\n\
 Color Level: {}\r\n",
            onoff!(ch.prf_flagged(PRF_DISPHP)),
            onoff!(ch.prf_flagged(PRF_BRIEF)),
            onoff!(!ch.prf_flagged(PRF_SUMMONABLE)),
            onoff!(ch.prf_flagged(PRF_DISPMOVE)),
            onoff!(ch.prf_flagged(PRF_COMPACT)),
            yesno!(ch.prf_flagged(PRF_QUEST)),
            onoff!(ch.prf_flagged(PRF_DISPMANA)),
            onoff!(ch.prf_flagged(PRF_NOTELL)),
            yesno!(!ch.prf_flagged(PRF_NOREPEAT)),
            onoff!(ch.prf_flagged(PRF_AUTOEXIT)),
            yesno!(ch.prf_flagged(PRF_DEAF)),
            buf2,
            onoff!(!ch.prf_flagged(PRF_NOGOSS)),
            onoff!(!ch.prf_flagged(PRF_NOAUCT)),
            onoff!(!ch.prf_flagged(PRF_NOGRATZ)),
            CTYPES[COLOR_LEV!(ch) as usize]
        )
        .as_str(),
    );
}

pub fn sort_commands(db: &mut DB) {
    db.cmd_sort_info.reserve_exact(CMD_INFO.len());

    for a in 0..CMD_INFO.len() {
        db.cmd_sort_info.push(a);
    }

    db.cmd_sort_info
        .sort_by(|a, b| str::cmp(CMD_INFO[*a].command, CMD_INFO[*b].command));
}

pub fn do_commands(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let mut arg = String::new();
    one_argument(argument, &mut arg);
    let vict;
    let victo;
    if !arg.is_empty() {
        victo = game.db.get_char_vis(ch, &mut arg, None, FIND_CHAR_WORLD);
        if victo.is_none() || victo.as_ref().unwrap().is_npc() {
            send_to_char(ch, "Who is that?\r\n");
            return;
        }
        vict = victo.as_ref().unwrap();
        if ch.get_level() < vict.get_level() {
            send_to_char(
                ch,
                "You can't see the commands of people above your level.\r\n",
            );
            return;
        }
    } else {
        vict = ch;
    }

    let mut socials = false;
    let mut wizhelp = false;
    if subcmd == SCMD_SOCIALS {
        socials = true;
    } else if subcmd == SCMD_WIZHELP {
        wizhelp = true;
    }

    let vic_name = vict.get_name();
    send_to_char(
        ch,
        format!(
            "The following {}{} are available to {}:\r\n",
            if wizhelp { "privileged " } else { "" },
            if socials { "socials" } else { "commands" },
            if Rc::ptr_eq(vict, ch) {
                "you"
            } else {
                vic_name.as_ref()
            }
        )
        .as_str(),
    );

    /* cmd_num starts at 1, not 0, to remove 'RESERVED' */
    let mut no = 1;
    for cmd_num in 1..CMD_INFO.len() {
        let i: usize = game.db.cmd_sort_info[cmd_num];
        if CMD_INFO[i].minimum_level < 0 || vict.get_level() < CMD_INFO[i].minimum_level as u8 {
            continue;
        }
        if (CMD_INFO[i].minimum_level >= LVL_IMMORT) != wizhelp {
            continue;
        }
        if !wizhelp
            && socials
                != (CMD_INFO[i].command_pointer as usize == do_action as usize
                    || CMD_INFO[i].command_pointer as usize == do_insult as usize)
        {
            continue;
        }
        send_to_char(
            ch,
            format!(
                "{:11}{}",
                CMD_INFO[i].command,
                if no % 7 == 0 { "\r\n" } else { "" }
            )
            .as_str(),
        );
        no += 1;
    }

    if no % 7 != 1 {
        send_to_char(ch, "\r\n");
    }
}
