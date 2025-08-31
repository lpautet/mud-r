/* ************************************************************************
*   File: act.informative.rs                            Part of CircleMUD *
*  Usage: Player-level commands of an informative nature                  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use std::rc::Rc;

use crate::act_social::{do_action, do_insult};
use crate::class::{find_class_bitvector, level_exp, title_female, title_male};
use crate::config::NOPERSON;
use crate::constants::{
    CIRCLEMUD_VERSION, COLOR_LIQUID, CONNECTED_TYPES, DIRS, FULLNESS, MONTH_NAME, ROOM_BITS,
    WEAR_WHERE, WEEKDAYS,
};
use crate::db::DB;
use crate::depot::{Depot, DepotId, HasId};
use crate::fight::compute_armor_class;
use crate::handler::{
    affected_by_spell, fname, generic_find, get_char_vis, get_number, isname, FindFlags,
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
use crate::structs::RoomFlags;
use crate::structs::{
    AffectFlags, ExitFlags, ExtraDescrData, ExtraFlags, ItemType, Position, PrefFlags, Sex,
    CONT_CLOSED, LVL_GOD, LVL_IMPL, NOWHERE, PLR_KILLER, PLR_MAILING, PLR_THIEF, PLR_WRITING,
};
use crate::structs::{DRUNK, FULL, LVL_IMMORT, NUM_WEARS, THIRST};
use crate::util::{
    age, can_see, can_see_obj, pers, rand_number, real_time_passed, sprintbit, sprinttype,
    time_now, SECS_PER_MUD_HOUR, SECS_PER_REAL_MIN,
};
use crate::{_clrlevel, an, clr, Game, CCCYN, CCGRN, CCRED, CCYEL, COLOR_LEV, TO_NOTVICT};
use crate::{act, send_to_char, CharData, DescriptorData, ObjData, TextData, VictimRef};
use crate::{CCNRM, TO_VICT};
use log::error;
use regex::Regex;

pub const SHOW_OBJ_LONG: i32 = 0;
pub const SHOW_OBJ_SHORT: i32 = 1;
pub const SHOW_OBJ_ACTION: i32 = 2;

fn show_obj_to_char(
    descs: &mut Depot<DescriptorData>,
    chars: &Depot<CharData>,
    texts: &Depot<TextData>,
    obj: &ObjData,
    ch: &CharData,
    mode: i32,
) {
    match mode {
        SHOW_OBJ_LONG => {
            send_to_char(descs, ch, format!("{}", obj.description).as_str());
        }

        SHOW_OBJ_SHORT => {
            send_to_char(descs, ch, format!("{}", obj.short_description).as_str());
        }

        SHOW_OBJ_ACTION => match obj.get_obj_type() {
            ItemType::Note => {
                let description = texts.get(obj.action_description);
                if !description.text.is_empty() {
                    let notebuf = format!(
                        "There is something written on it:\r\n\r\n{}",
                        description.text
                    );
                    let desc_id = ch.desc.unwrap();
                    page_string(descs, chars, desc_id, notebuf.as_str(), true);
                } else {
                    send_to_char(descs, ch, "It's blank.\r\n");
                }
                return;
            }
            ItemType::Drinkcon => {
                send_to_char(descs, ch, "It looks like a drink container.");
            }

            _ => {
                send_to_char(descs, ch, "You see nothing special..");
            }
        },

        _ => {
            error!("SYSERR: Bad display mode ({}) in show_obj_to_char().", mode);
            return;
        }
    }

    show_obj_modifiers(descs, obj, ch);
    send_to_char(descs, ch, "\r\n");
}

fn show_obj_modifiers(descs: &mut Depot<DescriptorData>, obj: &ObjData, ch: &CharData) {
    if obj.obj_flagged(ExtraFlags::INVISIBLE) {
        send_to_char(descs, ch, " (invisible)");
    }
    if obj.obj_flagged(ExtraFlags::BLESS) && ch.aff_flagged(AffectFlags::DETECT_ALIGN) {
        send_to_char(descs, ch, " ..It glows blue!");
    }
    if obj.obj_flagged(ExtraFlags::MAGIC) && ch.aff_flagged(AffectFlags::DETECT_MAGIC) {
        send_to_char(descs, ch, " ..It glows yellow!");
    }
    if obj.obj_flagged(ExtraFlags::GLOW) {
        send_to_char(descs, ch, " ..It has a soft glowing aura!");
    }
    if obj.obj_flagged(ExtraFlags::HUM) {
        send_to_char(descs, ch, " ..It emits a faint humming sound!");
    }
}

#[allow(clippy::too_many_arguments)]
fn list_obj_to_char(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    texts: &Depot<TextData>,
    objs: &Depot<ObjData>,
    list: &Vec<DepotId>,
    ch: &CharData,
    mode: i32,
    show: bool,
) {
    let mut found = true;

    for &oid in list {
        let obj = objs.get(oid);
        if can_see_obj(descs, chars, db, ch, obj) {
            show_obj_to_char(descs, chars, texts, obj, ch, mode);
            found = true;
        }
    }
    if !found && show {
        send_to_char(descs, ch, " Nothing.\r\n");
    }
}

fn diag_char_to_char(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    i: &CharData,
    ch: &CharData,
) {
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

    let pers = pers(descs, chars, db, i, ch);

    let percent = if i.get_max_hit() > 0 {
        (100 * i.get_hit() as i32) / i.get_max_hit() as i32
    } else {
        -1 /* How could MAX_HIT be < 1?? */
    };
    let mut ar_index: usize = 0;
    loop {
        if DIAGNOSIS[ar_index].percent < 0 || percent >= DIAGNOSIS[ar_index].percent as i32 {
            break;
        }
        ar_index += 1;
    }

    send_to_char(
        descs,
        ch,
        format!(
            "{}{} {}\r\n",
            pers.chars().next().unwrap().to_uppercase(),
            &pers[1..],
            DIAGNOSIS[ar_index].text
        )
        .as_str(),
    );
}

fn look_at_char(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    texts: &Depot<TextData>,
    objs: &Depot<ObjData>,
    i: &CharData,
    ch: &CharData,
) {
    let mut found;

    if ch.desc.is_none() {
        return;
    }
    let description = texts.get(i.player.description);
    if !description.text.is_empty() {
        send_to_char(descs, ch, &description.text);
    } else {
        act(
            descs,
            chars,
            db,
            "You see nothing special about $m.",
            false,
            Some(i),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
    }

    diag_char_to_char(descs, db, chars, i, ch);

    found = false;
    for j in 0..NUM_WEARS {
        if i.get_eq(j).is_some()
            && can_see_obj(descs, chars, db, ch, objs.get(i.get_eq(j).unwrap()))
        {
            found = true;
        }
    }

    if found {
        send_to_char(descs, ch, "\r\n"); /* act() does capitalization. */
        act(
            descs,
            chars,
            db,
            "$n is using:",
            false,
            Some(i),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        for (j, wear_where) in WEAR_WHERE.iter().enumerate() {
            if let Some(eq_j) = i.get_eq(j) {
                let obj = objs.get(eq_j);
                if can_see_obj(descs, chars, db, ch, obj) {
                    send_to_char(descs, ch, wear_where);
                    show_obj_to_char(descs, chars, texts, obj, ch, SHOW_OBJ_SHORT);
                }
            }
        }
    }
    if i.id() != ch.id() && (ch.is_thief() || ch.get_level() >= LVL_IMMORT) {
        found = false;
        act(
            descs,
            chars,
            db,
            "\r\nYou attempt to peek at $s inventory:",
            false,
            Some(i),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        for &tmp_obj_id in &i.carrying {
            let tmp_obj = objs.get(tmp_obj_id);
            if can_see_obj(descs, chars, db, ch, tmp_obj)
                && rand_number(0, 20) < ch.get_level() as u32
            {
                show_obj_to_char(descs, chars, texts, tmp_obj, ch, SHOW_OBJ_SHORT);
                found = true;
            }
        }
    }

    if !found {
        send_to_char(descs, ch, "You can't see anything.\r\n");
    }
}

fn list_one_char(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    i: &CharData,
    ch: &CharData,
) {
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

    if i.is_npc() && !i.player.long_descr.is_empty() && i.get_pos() == i.get_default_pos() {
        if i.aff_flagged(AffectFlags::INVISIBLE) {
            send_to_char(descs, ch, "*");
        }

        if ch.aff_flagged(AffectFlags::DETECT_ALIGN) {
            if i.is_evil() {
                send_to_char(descs, ch, "(Red Aura) ");
            } else if i.is_good() {
                send_to_char(descs, ch, "(Blue Aura) ");
            }
        }
        send_to_char(descs, ch, &i.player.long_descr);

        if i.aff_flagged(AffectFlags::SANCTUARY) {
            act(
                descs,
                chars,
                db,
                "...$e glows with a bright light!",
                false,
                Some(i),
                None,
                Some(VictimRef::Char(ch)),
                TO_VICT,
            );
        }
        if i.aff_flagged(AffectFlags::BLIND) {
            act(
                descs,
                chars,
                db,
                "...$e is groping around blindly!",
                false,
                Some(i),
                None,
                Some(VictimRef::Char(ch)),
                TO_VICT,
            );
        }
        return;
    }

    if i.is_npc() {
        send_to_char(
            descs,
            ch,
            format!(
                "{}{}",
                i.player.short_descr[0..1].to_uppercase(),
                &i.player.short_descr[1..]
            )
            .as_str(),
        );
    } else {
        send_to_char(
            descs,
            ch,
            format!("{} {}", i.player.name, i.get_title()).as_str(),
        );
    }

    if i.aff_flagged(AffectFlags::INVISIBLE) {
        send_to_char(descs, ch, " (invisible)");
    }
    if i.aff_flagged(AffectFlags::HIDE) {
        send_to_char(descs, ch, " (hidden)");
    }
    if !i.is_npc() && i.desc.is_none() {
        send_to_char(descs, ch, " (linkless)");
    }
    if !i.is_npc() && i.plr_flagged(PLR_WRITING) {
        send_to_char(descs, ch, " (writing)");
    }
    if i.get_pos() != Position::Fighting {
        send_to_char(descs, ch, POSITIONS[i.get_pos() as usize]);
    } else if let Some(fighting_id) = i.fighting_id() {
        send_to_char(descs, ch, " is here, fighting ");
        let fighting = chars.get(fighting_id);
        if fighting.id() == ch.id() {
            send_to_char(descs, ch, "YOU!");
        } else if i.in_room() == fighting.in_room() {
            let msg = format!("{}!", pers(descs, chars, db, fighting, ch));
            send_to_char(descs, ch, msg.as_str());
        } else {
            send_to_char(descs, ch, "someone who has already left!");
        }
    } else {
        /* NIL fighting pointer */
        send_to_char(descs, ch, " is here struggling with thin air.");
    }

    if ch.aff_flagged(AffectFlags::DETECT_ALIGN) {
        if i.is_evil() {
            send_to_char(descs, ch, " (Red Aura)");
        } else if i.is_good() {
            send_to_char(descs, ch, " (Blue Aura)");
        }
    }
    send_to_char(descs, ch, "\r\n");

    if i.aff_flagged(AffectFlags::SANCTUARY) {
        act(
            descs,
            chars,
            db,
            "...$e glows with a bright light!",
            false,
            Some(i),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
    }
}

fn list_char_to_char(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    list: &Vec<DepotId>,
    ch: &CharData,
) {
    for id in list {
        if *id != ch.id() {
            let obj = chars.get(*id);
            if can_see(descs, chars, db, ch, obj) {
                list_one_char(descs, db, chars, obj, ch);
            } else if db.is_dark(ch.in_room())
                && !ch.can_see_in_dark()
                && obj.aff_flagged(AffectFlags::INFRAVISION)
            {
                send_to_char(
                    descs,
                    ch,
                    "You see a pair of glowing red eyes looking your way.\r\n",
                );
            }
        }
    }
}

fn do_auto_exits(descs: &mut Depot<DescriptorData>, db: &DB, ch: &CharData) {
    let mut slen = 0;
    send_to_char(
        descs,
        ch,
        format!("{}[ Exits: ", CCCYN!(ch, C_NRM)).as_str(),
    );
    for (door, dir) in DIRS.iter().enumerate() {
        if let Some(exit) = db.exit(ch, door) {
            if exit.to_room == NOWHERE || exit.exit_flagged(ExitFlags::CLOSED) {
                continue;
            }
            send_to_char(descs, ch, format!("{} ", dir.to_lowercase()).as_str());
            slen += 1;
        } else {
            continue;
        }
    }
    send_to_char(
        descs,
        ch,
        format!(
            "{}]{}\r\n",
            if slen != 0 { "" } else { "None!" },
            CCNRM!(ch, C_NRM)
        )
        .as_str(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn do_exits(
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
    if ch.aff_flagged(AffectFlags::BLIND) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can't see a damned thing, you're blind!\r\n",
        );
        return;
    }
    send_to_char(&mut game.descriptors, ch, "Obvious exits:\r\n");
    let mut len = 0;
    for (door, dir) in DIRS.iter().enumerate() {
        let ch = chars.get(chid);
        if let Some(exit) = db.exit(ch, door) {
            if exit.to_room == NOWHERE || exit.exit_flagged(ExitFlags::CLOSED) {
                continue;
            }
        } else {
            continue;
        }
        len += 1;

        let oexit = db.exit(ch, door);
        let exit = oexit.as_ref().unwrap();
        if ch.get_level() >= LVL_IMMORT {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!(
                    "{} - [{:5}] {}\r\n",
                    dir,
                    db.get_room_vnum(exit.to_room),
                    db.world[exit.to_room as usize].name
                )
                .as_str(),
            );
        } else {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!(
                    "{} - {}\r\n",
                    dir,
                    if db.is_dark(exit.to_room) && !ch.can_see_in_dark() {
                        "Too dark to tell."
                    } else {
                        db.world[exit.to_room as usize].name.as_str()
                    }
                )
                .as_str(),
            );
        }
    }

    if len == 0 {
        send_to_char(&mut game.descriptors, ch, " None.\r\n");
    }
}

pub fn look_at_room(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    texts: &Depot<TextData>,
    objs: &Depot<ObjData>,
    ch: &CharData,
    ignore_brief: bool,
) {
    if ch.desc.is_none() {
        return;
    }

    if db.is_dark(ch.in_room()) && !ch.can_see_in_dark() {
        send_to_char(descs, ch, "It is pitch black...\r\n");
        return;
    } else if ch.aff_flagged(AffectFlags::BLIND) {
        send_to_char(descs, ch, "You see nothing but infinite darkness...\r\n");
        return;
    }
    send_to_char(descs, ch, CCCYN!(ch, C_NRM));

    if !ch.is_npc() && ch.prf_flagged(PrefFlags::ROOMFLAGS) {
        let mut buf = String::new();
        sprintbit(db.room_flags(ch.in_room()).bits(), &ROOM_BITS, &mut buf);
        send_to_char(
            descs,
            ch,
            format!(
                "[{}] {} [{}]",
                db.get_room_vnum(ch.in_room()),
                db.world[ch.in_room() as usize].name,
                buf
            )
            .as_str(),
        );
    } else {
        send_to_char(descs, ch, &db.world[ch.in_room() as usize].name);
    }

    send_to_char(descs, ch, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());

    if (!ch.is_npc() && !ch.prf_flagged(PrefFlags::BRIEF))
        || ignore_brief
        || db.room_flagged(ch.in_room(), RoomFlags::DEATH)
    {
        send_to_char(descs, ch, &db.world[ch.in_room() as usize].description);
    }

    /* autoexits */
    if !ch.is_npc() && ch.prf_flagged(PrefFlags::AUTOEXIT) {
        do_auto_exits(descs, db, ch);
    }

    /* now list characters & objects */
    send_to_char(descs, ch, CCGRN!(ch, C_NRM));
    list_obj_to_char(
        descs,
        db,
        chars,
        texts,
        objs,
        &db.world[ch.in_room() as usize].contents,
        ch,
        SHOW_OBJ_LONG,
        false,
    );
    send_to_char(descs, ch, CCYEL!(ch, C_NRM));
    list_char_to_char(
        descs,
        db,
        chars,
        &db.world[ch.in_room() as usize].peoples,
        ch,
    );
    send_to_char(descs, ch, CCNRM!(ch, C_NRM));
}

fn look_in_direction(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    chid: DepotId,
    dir: i32,
) {
    let ch = chars.get(chid);
    if let Some(exit) = db.exit(ch, dir as usize) {
        if !exit.general_description.is_empty() {
            send_to_char(descs, ch, format!("{}", exit.general_description).as_str());
        } else {
            send_to_char(descs, ch, "You see nothing special.\r\n");
        }
        if exit.exit_flagged(ExitFlags::CLOSED) && !exit.keyword.is_empty() {
            send_to_char(
                descs,
                ch,
                format!("The {} is closed.\r\n", fname(exit.keyword.as_ref())).as_str(),
            );
        } else if exit.exit_flagged(ExitFlags::ISDOOR) && !exit.keyword.is_empty() {
            send_to_char(
                descs,
                ch,
                format!("The {} is open.\r\n", fname(exit.keyword.as_ref())).as_str(),
            );
        } else {
            send_to_char(descs, ch, "Nothing special there...\r\n");
        }
    }
}

fn look_in_obj(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &Depot<ObjData>,
    ch: &CharData,
    arg: &str,
) {
    let mut dummy = None;
    let mut obj = None;

    if arg.is_empty() {
        send_to_char(descs, ch, "Look in what?\r\n");
        return;
    }
    let bits = generic_find(
        descs,
        chars,
        db,
        objs,
        arg,
        FindFlags::OBJ_INV | FindFlags::OBJ_ROOM | FindFlags::OBJ_EQUIP,
        ch,
        &mut dummy,
        &mut obj,
    );
    if bits.is_empty() {
        send_to_char(
            descs,
            ch,
            format!("There doesn't seem to be {} {} here.\r\n", an!(arg), arg).as_str(),
        );
    } else if obj.unwrap().get_obj_type() != ItemType::Drinkcon
        && obj.unwrap().get_obj_type() != ItemType::Fountain
        && obj.unwrap().get_obj_type() != ItemType::Container
    {
        send_to_char(descs, ch, "There's nothing inside that!\r\n");
    } else if obj.unwrap().get_obj_type() == ItemType::Container {
        if obj.unwrap().objval_flagged(CONT_CLOSED) {
            send_to_char(descs, ch, "It is closed.\r\n");
        } else {
            send_to_char(descs, ch, fname(obj.unwrap().name.as_ref()).as_ref());
            match bits {
                FindFlags::OBJ_INV => {
                    send_to_char(descs, ch, " (carried): \r\n");
                }
                FindFlags::OBJ_ROOM => {
                    send_to_char(descs, ch, " (here): \r\n");
                }
                FindFlags::OBJ_EQUIP => {
                    send_to_char(descs, ch, " (used): \r\n");
                }
                _ => {}
            }

            list_obj_to_char(
                descs,
                db,
                chars,
                texts,
                objs,
                &obj.unwrap().contains,
                ch,
                SHOW_OBJ_SHORT,
                true,
            );
        }
    } else {
        /* item must be a fountain or drink container */
        if obj.unwrap().get_obj_val(1) <= 0 {
            send_to_char(descs, ch, "It is empty.\r\n");
        } else if obj.unwrap().get_obj_val(0) <= 0
            || obj.unwrap().get_obj_val(1) > obj.unwrap().get_obj_val(0)
        {
            send_to_char(descs, ch, "Its contents seem somewhat murky.\r\n");
            /* BUG */
        } else {
            let mut buf2 = String::new();
            let amt = obj.unwrap().get_obj_val(1) * 3 / obj.unwrap().get_obj_val(0);
            sprinttype(obj.unwrap().get_obj_val(2), &COLOR_LIQUID, &mut buf2);
            send_to_char(
                descs,
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

fn find_exdesc<'a>(word: &str, list: &'a Vec<ExtraDescrData>) -> Option<&'a Rc<str>> {
    for i in list {
        if isname(word, i.keyword.as_ref()) {
            return Some(&i.description);
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
fn look_at_target(
    descs: &mut Depot<DescriptorData>,
    db: &DB,
    chars: &Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &Depot<ObjData>,
    ch: &CharData,
    arg: &str,
) {
    let mut i = 0;
    let mut found = false;
    let mut found_char = None;
    let mut found_obj = None;

    if ch.desc.is_none() {
        return;
    }

    if arg.is_empty() {
        send_to_char(descs, ch, "Look at what?\r\n");
        return;
    }

    let bits = generic_find(
        descs,
        chars,
        db,
        objs,
        arg,
        FindFlags::OBJ_INV | FindFlags::OBJ_ROOM | FindFlags::OBJ_EQUIP | FindFlags::CHAR_ROOM,
        ch,
        &mut found_char,
        &mut found_obj,
    );

    /* Is the target a character? */
    if let Some(found_char) = found_char {
        look_at_char(descs, db, chars, texts, objs, found_char, ch);
        if ch.id() != found_char.id() {
            if can_see(descs, chars, db, found_char, ch) {
                act(
                    descs,
                    chars,
                    db,
                    "$n looks at you.",
                    true,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(found_char)),
                    TO_VICT,
                );
            }
            act(
                descs,
                chars,
                db,
                "$n looks at $N.",
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(found_char)),
                TO_NOTVICT,
            );
        }
        return;
    }
    let mut arg = arg.to_string();
    let fnum = get_number(&mut arg);
    /* Strip off "number." from 2.foo and friends. */
    if fnum == 0 {
        send_to_char(descs, ch, "Look at what?\r\n");
        return;
    }

    /* Does the argument match an extra desc in the room? */
    let desc = find_exdesc(&arg, &db.world[ch.in_room() as usize].ex_descriptions);
    if let Some(desc) = desc {
        i += 1;
        if i == fnum {
            let d_id = ch.desc.unwrap();
            page_string(descs, chars, d_id, desc, false);
            return;
        }
    }

    /* Does the argument match an extra desc in the char's equipment? */
    for j in 0..NUM_WEARS {
        if let Some(eq_j) = ch.get_eq(j) {
            let eq = objs.get(eq_j);
            if can_see_obj(descs, chars, db, ch, eq) {
                if let Some(desc) = find_exdesc(&arg, &eq.ex_descriptions) {
                    i += 1;
                    if i == fnum {
                        send_to_char(descs, ch, desc);
                        found = true;
                    }
                }
            }
        }
    }

    /* Does the argument match an extra desc in the char's inventory? */
    for &oid in ch.carrying.iter() {
        if can_see_obj(descs, chars, db, ch, objs.get(oid)) {
            let desc = find_exdesc(&arg, &objs.get(oid).ex_descriptions);
            if let Some(desc) = desc {
                i += 1;
                if i == fnum {
                    send_to_char(descs, ch, desc);
                    found = true;
                }
            }
        }
    }

    /* Does the argument match an extra desc of an object in the room? */
    for &oid in db.world[ch.in_room() as usize].contents.iter() {
        if can_see_obj(descs, chars, db, ch, objs.get(oid)) {
            if let Some(desc) = find_exdesc(&arg, &objs.get(oid).ex_descriptions) {
                i += 1;
                if i == fnum {
                    send_to_char(descs, ch, desc.as_ref());
                    found = true;
                }
            }
        }
    }

    /* If an object was found back in generic_find */
    if !bits.is_empty() {
        if !found {
            show_obj_to_char(descs, chars, texts, found_obj.unwrap(), ch, SHOW_OBJ_ACTION);
        } else {
            show_obj_modifiers(descs, found_obj.unwrap(), ch);
            send_to_char(descs, ch, "\r\n");
        }
    } else if !found {
        send_to_char(descs, ch, "You do not see that here.\r\n");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_look(
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
    if ch.desc.is_none() {
        return;
    }
    if ch.get_pos() < Position::Sleeping {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can't see anything but stars!\r\n",
        );
    } else if ch.aff_flagged(AffectFlags::BLIND) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can't see a damned thing, you're blind!\r\n",
        );
    } else if db.is_dark(ch.in_room()) && !ch.can_see_in_dark() {
        send_to_char(&mut game.descriptors, ch, "It is pitch black...\r\n");
        list_char_to_char(
            &mut game.descriptors,
            db,
            chars,
            &db.world[ch.in_room() as usize].peoples,
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
                send_to_char(&mut game.descriptors, ch, "Read what?\r\n");
            } else {
                look_at_target(&mut game.descriptors, db, chars, texts, objs, ch, &arg);
            }
            return;
        }
        let look_type;
        if arg.is_empty() {
            /* "look" alone, without an argument at all */
            look_at_room(&mut game.descriptors, db, chars, texts, objs, ch, true);
        } else if is_abbrev(arg.as_ref(), "in") {
            look_in_obj(
                &mut game.descriptors,
                db,
                chars,
                texts,
                objs,
                ch,
                arg2.as_str(),
            );
            /* did the char type 'look <direction>?' */
        } else if {
            look_type = search_block(arg.as_str(), &DIRS, false);
            look_type
        }
        .is_some()
        {
            look_in_direction(
                &mut game.descriptors,
                db,
                chars,
                chid,
                look_type.unwrap() as i32,
            );
        } else if is_abbrev(arg.as_ref(), "at") {
            look_at_target(
                &mut game.descriptors,
                db,
                chars,
                texts,
                objs,
                ch,
                arg2.as_ref(),
            );
        } else {
            look_at_target(
                &mut game.descriptors,
                db,
                chars,
                texts,
                objs,
                ch,
                arg.as_ref(),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_examine(
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
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Examine what?\r\n");
        return;
    }

    /* look_at_target() eats the number. */
    look_at_target(&mut game.descriptors, db, chars, texts, objs, ch, &arg);
    let mut tmp_char = None;
    let mut tmp_object = None;
    generic_find(
        &game.descriptors,
        chars,
        db,
        objs,
        &arg,
        FindFlags::OBJ_INV | FindFlags::OBJ_ROOM | FindFlags::CHAR_ROOM | FindFlags::OBJ_EQUIP,
        ch,
        &mut tmp_char,
        &mut tmp_object,
    );

    if let Some(tmp_object) = tmp_object {
        if tmp_object.get_obj_type() == ItemType::Drinkcon
            || tmp_object.get_obj_type() == ItemType::Fountain
            || tmp_object.get_obj_type() == ItemType::Container
        {
            send_to_char(
                &mut game.descriptors,
                ch,
                "When you look inside, you see:\r\n",
            );
            look_in_obj(&mut game.descriptors, db, chars, texts, objs, ch, &arg);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_gold(
    game: &mut Game,
    _db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.get_gold() == 0 {
        send_to_char(&mut game.descriptors, ch, "You're broke!\r\n");
    } else if ch.get_gold() == 1 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You have one miserable little gold coin.\r\n",
        );
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You have {} gold coins.\r\n", ch.get_gold()).as_str(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_score(
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
    if ch.is_npc() {
        return;
    }

    send_to_char(
        &mut game.descriptors,
        ch,
        format!("You are {} years old.\r\n", ch.get_age()).as_str(),
    );
    let ch = chars.get(chid);
    if age(ch).month == 0 && age(ch).day == 0 {
        send_to_char(&mut game.descriptors, ch, "  It's your birthday today.\r\n");
    } else {
        send_to_char(&mut game.descriptors, ch, "\r\n");
    }
    let ch = chars.get(chid);
    send_to_char(
        &mut game.descriptors,
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
        &mut game.descriptors,
        ch,
        format!(
            "Your armor class is {}/10, and your alignment is {}.\r\n",
            compute_armor_class(ch),
            ch.get_alignment()
        )
        .as_str(),
    );
    send_to_char(
        &mut game.descriptors,
        ch,
        format!(
            "You have scored {} exp, and have {} gold coins.\r\n",
            ch.get_exp(),
            ch.get_gold()
        )
        .as_str(),
    );
    if ch.get_level() < LVL_IMMORT {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!(
                "You need {} exp to reach your next level.\r\n",
                level_exp(ch.get_class(), ch.get_level() + 1) - ch.get_exp()
            )
            .as_str(),
        );
    }
    let playing_time = real_time_passed(
        (time_now() - ch.player.time.logon) + ch.player.time.played as u64,
        0,
    );
    send_to_char(
        &mut game.descriptors,
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
        &mut game.descriptors,
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
        Position::Dead => {
            send_to_char(&mut game.descriptors, ch, "You are DEAD!\r\n");
        }
        Position::MortallyWounded => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You are mortally wounded!  You should seek help!\r\n",
            );
        }
        Position::Incapacitated => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You are incapacitated, slowly fading away...\r\n",
            );
        }
        Position::Stunned => {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You are stunned!  You can't move!\r\n",
            );
        }
        Position::Sleeping => {
            send_to_char(&mut game.descriptors, ch, "You are sleeping.\r\n");
        }
        Position::Resting => {
            send_to_char(&mut game.descriptors, ch, "You are resting.\r\n");
        }
        Position::Sitting => {
            send_to_char(&mut game.descriptors, ch, "You are sitting.\r\n");
        }
        Position::Fighting => {
            let v = pers(
                &game.descriptors,
                chars,
                db,
                chars.get(ch.fighting_id().unwrap()),
                ch,
            );
            send_to_char(
                &mut game.descriptors,
                ch,
                format!(
                    "You are fighting {}.\r\n",
                    if ch.fighting_id().is_some() {
                        v.as_ref()
                    } else {
                        "thin air"
                    }
                )
                .as_str(),
            );
        }
        Position::Standing => {
            send_to_char(&mut game.descriptors, ch, "You are standing.\r\n");
        }
    }
    if ch.get_cond(DRUNK) > 10 {
        send_to_char(&mut game.descriptors, ch, "You are intoxicated.\r\n");
    }
    if ch.get_cond(FULL) == 0 {
        send_to_char(&mut game.descriptors, ch, "You are hungry.\r\n");
    }
    if ch.get_cond(THIRST) == 0 {
        send_to_char(&mut game.descriptors, ch, "You are thirsty.\r\n");
    }
    if ch.aff_flagged(AffectFlags::BLIND) {
        send_to_char(&mut game.descriptors, ch, "You have been blinded!\r\n");
    }
    if ch.aff_flagged(AffectFlags::INVISIBLE) {
        send_to_char(&mut game.descriptors, ch, "You are invisible.\r\n");
    }
    if ch.aff_flagged(AffectFlags::DETECT_INVIS) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are sensitive to the presence of invisible things.\r\n",
        );
    }
    if ch.aff_flagged(AffectFlags::SANCTUARY) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are protected by Sanctuary.\r\n",
        );
    }
    if ch.aff_flagged(AffectFlags::POISON) {
        send_to_char(&mut game.descriptors, ch, "You are poisoned!\r\n");
    }
    if ch.aff_flagged(AffectFlags::CHARM) {
        send_to_char(&mut game.descriptors, ch, "You have been charmed!\r\n");
    }
    if affected_by_spell(ch, SPELL_ARMOR as i16) {
        send_to_char(&mut game.descriptors, ch, "You feel protected.\r\n");
    }
    if ch.aff_flagged(AffectFlags::INFRAVISION) {
        send_to_char(&mut game.descriptors, ch, "Your eyes are glowing red.\r\n");
    }
    if ch.prf_flagged(PrefFlags::SUMMONABLE) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are summonable by other players.\r\n",
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_inventory(
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
    send_to_char(&mut game.descriptors, ch, "You are carrying:\r\n");
    list_obj_to_char(
        &mut game.descriptors,
        db,
        chars,
        texts,
        objs,
        &ch.carrying,
        ch,
        SHOW_OBJ_SHORT,
        true,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn do_equipment(
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
    let mut found = false;
    send_to_char(&mut game.descriptors, ch, "You are using:\r\n");
    for (i, wear_where) in WEAR_WHERE.iter().enumerate() {
        if let Some(oid) = ch.get_eq(i) {
            let obj = objs.get(oid);
            if can_see_obj(&game.descriptors, chars, db, ch, obj) {
                send_to_char(&mut game.descriptors, ch, wear_where);
                show_obj_to_char(&mut game.descriptors, chars, texts, obj, ch, SHOW_OBJ_SHORT);
                found = true;
            } else {
                send_to_char(&mut game.descriptors, ch, wear_where);
                send_to_char(&mut game.descriptors, ch, "Something.\r\n");
                found = true;
            }
        }
    }
    if !found {
        send_to_char(&mut game.descriptors, ch, " Nothing.\r\n");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_time(
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
    /* day in [1..35] */
    let day = db.time_info.day + 1;

    /* 35 days in a month, 7 days a week */
    let weekday = ((35 * db.time_info.month) + day) % 7;

    send_to_char(
        &mut game.descriptors,
        ch,
        format!(
            "It is {} o'clock {}, on {}.\r\n",
            if db.time_info.hours % 12 == 0 {
                12
            } else {
                db.time_info.hours % 12
            },
            if db.time_info.hours >= 12 { "pm" } else { "am" },
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
        &mut game.descriptors,
        ch,
        format!(
            "The {}{} Day of the {}, Year {}.\r\n",
            day, suf, MONTH_NAME[db.time_info.month as usize], db.time_info.year
        )
        .as_str(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn do_weather(
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
    const SKY_LOOK: [&str; 4] = [
        "cloudless",
        "cloudy",
        "rainy",
        "lit by flashes of lightning",
    ];
    if db.outside(ch) {
        let messg = format!(
            "The sky is {} and {}.\r\n",
            SKY_LOOK[db.weather_info.sky as usize],
            if db.weather_info.change >= 0 {
                "you feel a warm wind from south"
            } else {
                "your foot tells you bad weather is due"
            }
        );
        send_to_char(&mut game.descriptors, ch, messg.as_str());
        let ch = chars.get(chid);
        if ch.get_level() >= LVL_GOD {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!(
                    "Pressure: {} (change: {}), Sky: {} ({})\r\n",
                    db.weather_info.pressure,
                    db.weather_info.change,
                    db.weather_info.sky as usize,
                    SKY_LOOK[db.weather_info.sky as usize],
                )
                .as_str(),
            );
        }
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You have no feeling about the weather at all.\r\n",
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_help(
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
    if ch.desc.is_none() {
        return;
    }

    let argument = argument.trim_start();
    let d_id = ch.desc.unwrap();

    if argument.is_empty() {
        page_string(&mut game.descriptors, chars, d_id, &db.help, false);
        return;
    }
    if db.help_table.is_empty() {
        send_to_char(&mut game.descriptors, ch, "No help available.\r\n");
        return;
    }

    let mut bot = 0;
    let mut top = db.help_table.len() - 1;

    loop {
        let mut mid = (bot + top) / 2;
        if bot > top {
            send_to_char(
                &mut game.descriptors,
                ch,
                "There is no help on that word.\r\n",
            );
            return;
        } else if db.help_table[mid].keyword.starts_with(argument) {
            /* trace backwards to find first matching entry. Thanks Jeff Fink! */
            while mid > 0 && db.help_table[mid - 1].keyword.starts_with(argument) {
                mid -= 1;
            }
            page_string(
                &mut game.descriptors,
                chars,
                d_id,
                &db.help_table[mid].entry,
                false,
            );
            return;
        } else if db.help_table[mid].keyword.as_ref() < argument {
            bot = mid + 1;
        } else {
            top = mid - 1;
        }
    }
}

const WHO_FORMAT: &str =
    "format: who [minlev[-maxlev]] [-n name] [-c classlist] [-s] [-o] [-q] [-r] [-z]\r\n";

/* FIXME: This whole thing just needs rewritten. */
#[allow(clippy::too_many_arguments)]
pub fn do_who(
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
    let argument = argument.trim_start();
    let mut buf = argument.to_string();
    let mut low = 0_u8;
    let mut high = LVL_IMPL;
    let mut outlaws = false;
    let mut localwho = false;
    let mut short_list = false;
    let mut questwho = false;
    let mut name_search = String::new();
    let mut who_room = false;
    let mut showclass = 0;

    let regex = Regex::new(r"^(\d{1,9})-(\d{1,9})").unwrap();
    while !buf.is_empty() {
        let mut arg = String::new();
        let mut buf1 = String::new();

        half_chop(&mut buf, &mut arg, &mut buf1);
        if arg.chars().next().unwrap().is_ascii_digit() {
            let f = regex.captures(&arg);
            if let Some(f) = f {
                low = f[1].parse::<u8>().unwrap();
                high = f[2].parse::<u8>().unwrap();
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
                    let f = regex.captures(&arg);
                    if let Some(f) = f {
                        low = f[1].parse::<u8>().unwrap();
                        high = f[2].parse::<u8>().unwrap();
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
                    send_to_char(&mut game.descriptors, ch, WHO_FORMAT);
                    return;
                }
            }
        } else {
            /* endif */
            send_to_char(&mut game.descriptors, ch, WHO_FORMAT);
            return;
        }
    } /* end while (parser) */

    send_to_char(&mut game.descriptors, ch, "Players\r\n-------\r\n");
    let mut num_can_see = 0;

    for d_id in game.descriptor_list.clone() {
        let d = game.desc(d_id);
        if d.state() != ConPlaying {
            continue;
        }

        let tch_id;
        if d.original.is_some() {
            tch_id = d.original;
        } else {
            tch_id = d.character;
            if tch_id.is_none() {
                continue;
            }
        }
        let tch_id = tch_id.unwrap();
        let tch = chars.get(tch_id);

        if !name_search.is_empty()
            && tch.get_name().as_ref() != name_search
            && !tch.get_title().contains(&name_search)
        {
            continue;
        }
        let ch = chars.get(chid);
        if !can_see(&game.descriptors, chars, db, ch, tch)
            || tch.get_level() < low
            || tch.get_level() > high
        {
            continue;
        }
        if outlaws && !tch.plr_flagged(PLR_KILLER) && !tch.plr_flagged(PLR_THIEF) {
            continue;
        }
        if questwho && !tch.prf_flagged(PrefFlags::QUEST) {
            continue;
        }
        if localwho && db.world[ch.in_room() as usize].zone != db.world[tch.in_room() as usize].zone
        {
            continue;
        }
        if who_room && tch.in_room() != ch.in_room() {
            continue;
        }
        if showclass != 0 && (showclass & (1 << tch.get_class() as i8)) == 0 {
            continue;
        }
        if short_list {
            #[allow(clippy::blocks_in_conditions)]
            let messg = format!(
                "{}[{:2} {}] {:12}{}{}",
                if tch.get_level() >= LVL_IMMORT {
                    CCYEL!(ch, C_SPR)
                } else {
                    ""
                },
                tch.get_level(),
                tch.class_abbr(),
                tch.get_name(),
                if tch.get_level() >= LVL_IMMORT {
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
            );
            send_to_char(&mut game.descriptors, ch, messg.as_str());
        } else {
            num_can_see += 1;
            let messg = format!(
                "{}[{:2} {}] {} {}",
                if tch.get_level() >= LVL_IMMORT {
                    CCYEL!(ch, C_SPR)
                } else {
                    ""
                },
                tch.get_level(),
                tch.class_abbr(),
                tch.get_name(),
                tch.get_title()
            );
            send_to_char(&mut game.descriptors, ch, messg.as_str());

            if tch.get_invis_lev() != 0 {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    format!(" (i{})", tch.get_invis_lev()).as_str(),
                );
            } else if tch.aff_flagged(AffectFlags::INVISIBLE) {
                send_to_char(&mut game.descriptors, ch, " (invis)");
            }
            if tch.plr_flagged(PLR_MAILING) {
                send_to_char(&mut game.descriptors, ch, " (mailing)");
            } else if tch.plr_flagged(PLR_WRITING) {
                send_to_char(&mut game.descriptors, ch, " (writing)");
            }
            if tch.prf_flagged(PrefFlags::DEAF) {
                send_to_char(&mut game.descriptors, ch, " (deaf)");
            }
            if tch.prf_flagged(PrefFlags::NOTELL) {
                send_to_char(&mut game.descriptors, ch, " (notell)");
            }
            if tch.prf_flagged(PrefFlags::QUEST) {
                send_to_char(&mut game.descriptors, ch, " (quest)");
            }
            if tch.plr_flagged(PLR_THIEF) {
                send_to_char(&mut game.descriptors, ch, " (THIEF)");
            }
            if tch.plr_flagged(PLR_KILLER) {
                send_to_char(&mut game.descriptors, ch, " (KILLER)");
            }
            if tch.get_level() >= LVL_IMMORT {
                let ch = chars.get(chid);
                send_to_char(&mut game.descriptors, ch, CCNRM!(ch, C_SPR));
            }
            send_to_char(&mut game.descriptors, ch, "\r\n");
        } /* endif shortlist */
    } /* end of for */
    if short_list && (num_can_see % 4) != 0 {
        send_to_char(&mut game.descriptors, ch, "\r\n");
    }
    if num_can_see == 0 {
        send_to_char(&mut game.descriptors, ch, "\r\nNobody at all!\r\n");
    } else if num_can_see == 1 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "\r\nOne lonely character displayed.\r\n",
        );
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("\r\n{} characters displayed.\r\n", num_can_see).as_str(),
        );
    }
}

const USERS_FORMAT: &str =
    "format: users [-l minlevel[-maxlevel]] [-n name] [-h host] [-c classlist] [-o] [-p]\r\n";

/* BIG OL' FIXME: Rewrite it all. Similar to do_who(). */
#[allow(clippy::too_many_arguments)]
pub fn do_users(
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
    let mut buf = argument.to_string();
    let mut arg = String::new();
    let mut outlaws = false;
    let mut playing = false;
    let mut deadweight = false;
    let mut low = 0_u8;
    let mut high = LVL_IMPL;
    let mut name_search = String::new();
    let mut host_search = String::new();
    let mut showclass = 0;
    let mut num_can_see = 0;
    let mut classname;

    let regex = Regex::new(r"^(\d{1,9})-(\d{1,9})").unwrap();
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
                    let f = regex.captures(&arg);
                    if let Some(f) = f {
                        low = f[1].parse::<u8>().unwrap();
                        high = f[2].parse::<u8>().unwrap();
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
                    send_to_char(&mut game.descriptors, ch, USERS_FORMAT);
                    return;
                }
            } /* end of switch */
        } else {
            /* endif */
            send_to_char(&mut game.descriptors, ch, USERS_FORMAT);
            return;
        }
    } /* end while (parser) */
    send_to_char(
        &mut game.descriptors,
        ch,
        "Num Class   Name         State          Idl Login@   Site\r\n\
--- ------- ------------ -------------- --- -------- ------------------------\r\n",
    );

    one_argument(argument, &mut arg);
    for d_id in game.descriptor_list.clone() {
        let d = game.desc(d_id);
        if d.state() != ConPlaying && playing {
            continue;
        }
        if d.state() == ConPlaying && deadweight {
            continue;
        }
        if d.state() == ConPlaying {
            let character;
            if d.original.is_some() {
                character = d.original;
            } else {
                character = d.character;
                if character.is_none() {
                    continue;
                }
            }
            let tch_id = character.unwrap();
            let tch = chars.get(tch_id);

            if !host_search.is_empty() && !d.host.contains(&host_search) {
                continue;
            }
            if !name_search.is_empty() && tch.get_name().as_ref() != name_search {
                continue;
            }
            let ch = chars.get(chid);
            if !can_see(&game.descriptors, chars, db, ch, tch)
                || tch.get_level() < low
                || tch.get_level() > high
            {
                continue;
            }
            if outlaws && !tch.plr_flagged(PLR_KILLER) && !tch.plr_flagged(PLR_THIEF) {
                continue;
            }
            if showclass != 0 && (showclass & (1 << tch.get_class() as i8)) == 0 {
                continue;
            }
            if ch.get_invis_lev() > ch.get_level() as i16 {
                continue;
            }

            if d.original.is_some() {
                classname = format!(
                    "[{:2} {}]",
                    chars.get(d.original.unwrap()).get_level(),
                    chars.get(d.original.unwrap()).class_abbr()
                );
            } else {
                classname = format!(
                    "[{:2} {}]",
                    chars.get(d.character.unwrap()).get_level(),
                    chars.get(d.character.unwrap()).class_abbr()
                );
            }
        } else {
            classname = "   -   ".to_string();
        }

        let timeptr = d.login_time.elapsed().as_secs().to_string();

        let state = if d.state() == ConPlaying && d.original.is_some() {
            "Switched"
        } else {
            CONNECTED_TYPES[d.state() as usize]
        };

        let idletime = if d.character.is_some()
            && d.state() == ConPlaying
            && chars.get(d.character.unwrap()).get_level() < LVL_GOD
        {
            format!(
                "{:3}",
                chars.get(d.character.unwrap()).char_specials.timer * SECS_PER_MUD_HOUR as i32
                    / SECS_PER_REAL_MIN as i32
            )
        } else {
            "".to_string()
        };

        let mut line = format!(
            "{:3} {:7} {:12} {:14} {:3} {:8} ",
            d.desc_num,
            classname,
            if game.desc(d_id).original.is_some()
                && !chars.get(d.original.unwrap()).player.name.is_empty()
            {
                &chars.get(d.original.unwrap()).player.name
            } else if d.character.is_some()
                && !chars.get(d.character.unwrap()).player.name.is_empty()
            {
                &chars.get(d.character.unwrap()).player.name
            } else {
                "UNDEFINED"
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
            let ch = chars.get(chid);
            line.push_str(&format!(
                "{}{}{}",
                CCGRN!(ch, C_SPR),
                line,
                CCNRM!(ch, C_SPR)
            ));
        }
        let ch = chars.get(chid);
        if d.state() != ConPlaying
            || (d.state() == ConPlaying
                && can_see(
                    &game.descriptors,
                    chars,
                    db,
                    ch,
                    chars.get(d.character.unwrap()),
                ))
        {
            send_to_char(&mut game.descriptors, ch, &line);
            num_can_see += 1;
        }
    }

    send_to_char(
        &mut game.descriptors,
        ch,
        format!("\r\n{} visible sockets connected.\r\n", num_can_see).as_str(),
    );
}

/* Generic page_string function for displaying text */
#[allow(clippy::too_many_arguments)]
pub fn do_gen_ps(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let d_id = ch.desc.unwrap();
    match subcmd {
        SCMD_CREDITS => {
            page_string(&mut game.descriptors, chars, d_id, &db.credits, false);
        }
        SCMD_NEWS => {
            page_string(&mut game.descriptors, chars, d_id, &db.news, false);
        }
        SCMD_INFO => {
            page_string(&mut game.descriptors, chars, d_id, &db.info, false);
        }
        SCMD_WIZLIST => {
            page_string(&mut game.descriptors, chars, d_id, &db.wizlist, false);
        }
        SCMD_IMMLIST => {
            page_string(&mut game.descriptors, chars, d_id, &db.immlist, false);
        }
        SCMD_HANDBOOK => {
            page_string(&mut game.descriptors, chars, d_id, &db.handbook, false);
        }
        SCMD_POLICIES => {
            page_string(&mut game.descriptors, chars, d_id, &db.policies, false);
        }
        SCMD_MOTD => {
            page_string(&mut game.descriptors, chars, d_id, &db.motd, false);
        }
        SCMD_IMOTD => {
            page_string(&mut game.descriptors, chars, d_id, &db.imotd, false);
        }
        SCMD_CLEAR => {
            send_to_char(&mut game.descriptors, ch, "\x1b[H\x1b[J");
        }
        SCMD_VERSION => {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!("{}\r\n", CIRCLEMUD_VERSION).as_str(),
            );
        }
        SCMD_WHOAMI => {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!("{}\r\n", ch.get_name()).as_str(),
            );
        }
        _ => {
            error!("SYSERR: Unhandled case in do_gen_ps. ({})", subcmd);
        }
    }
}

fn perform_mortal_where(
    game: &mut Game,
    db: &DB,
    chars: &Depot<CharData>,
    chid: DepotId,
    arg: &str,
) {
    let ch = chars.get(chid);
    if arg.is_empty() {
        send_to_char(
            &mut game.descriptors,
            ch,
            "Players in your Zone\r\n--------------------\r\n",
        );
        for d_id in game.descriptor_list.clone() {
            let d = game.desc(d_id);
            if d.state() != ConPlaying || (d.character.is_some() && d.character.unwrap() == chid) {
                continue;
            }
            let i;
            let res = {
                i = if d.original.is_some() {
                    d.original
                } else {
                    d.character
                };
                i.is_none()
            };
            if res {
                continue;
            }
            let i_id = i.unwrap();
            let i = chars.get(i_id);
            if i.in_room() == NOWHERE || !can_see(&game.descriptors, chars, db, ch, i) {
                continue;
            }
            if db.world[ch.in_room() as usize].zone != db.world[i.in_room() as usize].zone {
                continue;
            }
            let messg = format!(
                "%{:20} - {}\r\n",
                i.get_name(),
                db.world[i.in_room() as usize].name
            );
            send_to_char(&mut game.descriptors, ch, messg.as_str());
        }
    } else {
        /* print only FIRST char, not all. */
        for &i_id in &db.character_list {
            let i = chars.get(i_id);
            if i.in_room() == NOWHERE || i.id() == chid {
                continue;
            }
            if !can_see(&game.descriptors, chars, db, ch, i)
                || db.world[i.in_room() as usize].zone != db.world[ch.in_room() as usize].zone
            {
                continue;
            }
            if !isname(arg, &i.player.name) {
                continue;
            }
            send_to_char(
                &mut game.descriptors,
                ch,
                format!(
                    "{:25} - {}\r\n",
                    i.get_name(),
                    db.world[i.in_room() as usize].name
                )
                .as_str(),
            );
            return;
        }
        send_to_char(&mut game.descriptors, ch, "Nobody around by that name.\r\n");
    }
}

#[allow(clippy::too_many_arguments)]
fn print_object_location(
    descs: &mut Depot<DescriptorData>,
    objs: &Depot<ObjData>,
    db: &DB,
    chars: &Depot<CharData>,
    num: i32,
    oid: DepotId,
    chid: DepotId,
    recur: bool,
) {
    let ch = chars.get(chid);
    let obj = objs.get(oid);
    if num > 0 {
        send_to_char(
            descs,
            ch,
            format!("O{:3}. {:25} - ", num, obj.short_description).as_ref(),
        );
    } else {
        send_to_char(descs, ch, format!("{:33}", " - ").as_str());
    }

    if obj.in_room != NOWHERE {
        send_to_char(
            descs,
            ch,
            format!(
                "[{:5}] {}\r\n",
                db.get_room_vnum(obj.in_room()),
                db.world[obj.in_room() as usize].name
            )
            .as_str(),
        );
    } else if obj.carried_by.is_some() {
        let ch = chars.get(chid);
        let msg = format!(
            "carried by {}\r\n",
            pers(descs, chars, db, chars.get(obj.carried_by.unwrap()), ch)
        );
        send_to_char(descs, ch, msg.as_str());
    } else if obj.worn_by.is_some() {
        let ch = chars.get(chid);
        let msg = format!(
            "worn by {}\r\n",
            pers(descs, chars, db, chars.get(obj.worn_by.unwrap()), ch)
        );
        send_to_char(descs, ch, msg.as_str());
    } else if obj.in_obj.is_some() {
        send_to_char(
            descs,
            ch,
            format!(
                "inside {}{}\r\n",
                objs.get(obj.in_obj.unwrap()).short_description,
                if recur { ", which is" } else { " " }
            )
            .as_str(),
        );
        if recur {
            print_object_location(descs, objs, db, chars, 0, obj.in_obj.unwrap(), chid, recur);
        }
    } else {
        send_to_char(descs, ch, "in an unknown location\r\n");
    }
}

fn perform_immort_where(
    game: &mut Game,
    db: &DB,
    chars: &Depot<CharData>,
    objs: &Depot<ObjData>,
    chid: DepotId,
    arg: &str,
) {
    let ch = chars.get(chid);

    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Players\r\n-------\r\n");
        for d_id in game.descriptor_list.clone() {
            let d = game.desc(d_id);
            if d.state() == ConPlaying {
                let oi = if d.original.is_some() {
                    d.original.as_ref()
                } else {
                    d.character.as_ref()
                };
                if oi.is_none() {
                    continue;
                }

                let i_id = *oi.unwrap();
                let i = chars.get(i_id);
                if can_see(&game.descriptors, chars, db, ch, i) && (i.in_room() != NOWHERE) {
                    if d.original.is_some() {
                        let messg = format!(
                            "{:20} - [{:5}] {} (in {})\r\n",
                            i.get_name(),
                            db.get_room_vnum(chars.get(d.character.unwrap()).in_room),
                            db.world[chars.get(d.character.unwrap()).in_room as usize].name,
                            chars.get(d.character.unwrap()).get_name()
                        );
                        send_to_char(&mut game.descriptors, ch, messg.as_str());
                    } else {
                        let messg = format!(
                            "{:20} - [{:5}] {}\r\n",
                            i.get_name(),
                            db.get_room_vnum(i.in_room()),
                            db.world[i.in_room() as usize].name
                        );
                        send_to_char(&mut game.descriptors, ch, messg.as_str());
                    }
                }
            }
        }
    } else {
        let mut found = false;
        let mut num = 0;
        for &id in &db.character_list {
            let i = chars.get(id);
            if can_see(&game.descriptors, chars, db, ch, i)
                && i.in_room() != NOWHERE
                && isname(arg, &i.player.name)
            {
                found = true;
                let messg = format!(
                    "M{:3}. {:25} - [{:5}] {}\r\n",
                    {
                        num += 1;
                        num
                    },
                    i.get_name(),
                    db.get_room_vnum(i.in_room()),
                    db.world[i.in_room() as usize].name
                );
                send_to_char(&mut game.descriptors, ch, messg.as_str());
            }
        }
        num = 0;
        for &k in &db.object_list {
            if can_see_obj(&game.descriptors, chars, db, ch, objs.get(k))
                && isname(arg, objs.get(k).name.as_ref())
            {
                found = true;
                print_object_location(
                    &mut game.descriptors,
                    objs,
                    db,
                    chars,
                    {
                        num += 1;
                        num
                    },
                    k,
                    chid,
                    true,
                );
            }
        }
        if !found {
            send_to_char(
                &mut game.descriptors,
                ch,
                "Couldn't find any such thing.\r\n",
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_where(
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
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if ch.get_level() >= LVL_IMMORT {
        perform_immort_where(game, db, chars, objs, chid, &arg);
    } else {
        perform_mortal_where(game, db, chars, chid, &arg);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_levels(
    game: &mut Game,
    _db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You ain't nothin' but a hound-dog.\r\n",
        );
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
            Sex::Male | Sex::Neutral => {
                buf.push_str(format!("{}\r\n", title_male(ch.get_class(), i)).as_str());
            }
            Sex::Female => {
                buf.push_str(format!("{}\r\n", title_female(ch.get_class(), i)).as_str());
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
    let d_id = ch.desc.unwrap();
    page_string(&mut game.descriptors, chars, d_id, buf.as_str(), true);
}

#[allow(clippy::too_many_arguments)]
pub fn do_consider(
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
    let mut buf = String::new();
    one_argument(argument, &mut buf);

    let victim = get_char_vis(
        &game.descriptors,
        chars,
        db,
        ch,
        &mut buf,
        None,
        FindFlags::CHAR_ROOM,
    );
    if victim.is_none() {
        send_to_char(&mut game.descriptors, ch, "Consider killing who?\r\n");
        return;
    }
    let victim = victim.unwrap();
    if victim.id() == chid {
        send_to_char(&mut game.descriptors, ch, "Easy!  Very easy indeed!\r\n");
        return;
    }
    if !victim.is_npc() {
        send_to_char(
            &mut game.descriptors,
            ch,
            "Would you like to borrow a cross and a shovel?\r\n",
        );
        return;
    }
    let diff = victim.get_level() as i32 - ch.get_level() as i32;

    if diff <= -10 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "Now where did that chicken go?\r\n",
        );
    } else if diff <= -5 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You could do it with a needle!\r\n",
        );
    } else if diff <= -2 {
        send_to_char(&mut game.descriptors, ch, "Easy.\r\n");
    } else if diff <= -1 {
        send_to_char(&mut game.descriptors, ch, "Fairly easy.\r\n");
    } else if diff == 0 {
        send_to_char(&mut game.descriptors, ch, "The perfect match!\r\n");
    } else if diff <= 1 {
        send_to_char(&mut game.descriptors, ch, "You would need some luck!\r\n");
    } else if diff <= 2 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You would need a lot of luck!\r\n",
        );
    } else if diff <= 3 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You would need a lot of luck and great equipment!\r\n",
        );
    } else if diff <= 5 {
        send_to_char(&mut game.descriptors, ch, "Do you feel lucky, punk?\r\n");
    } else if diff <= 10 {
        send_to_char(&mut game.descriptors, ch, "Are you mad!?\r\n");
    } else if diff <= 100 {
        send_to_char(&mut game.descriptors, ch, "You ARE mad!\r\n");
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_diagnose(
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
    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let vict;
    if !buf.is_empty() {
        let res = {
            vict = get_char_vis(
                &game.descriptors,
                chars,
                db,
                ch,
                &mut buf,
                None,
                FindFlags::CHAR_ROOM,
            );
            vict.is_none()
        };
        if res {
            send_to_char(&mut game.descriptors, ch, NOPERSON);
        } else {
            diag_char_to_char(&mut game.descriptors, db, chars, vict.unwrap(), ch);
        }
    } else if ch.fighting_id().is_some() {
        let fighting_id = ch.fighting_id().unwrap();
        let fighting = chars.get(fighting_id);
        diag_char_to_char(&mut game.descriptors, db, chars, fighting, ch);
    } else {
        send_to_char(&mut game.descriptors, ch, "Diagnose who?\r\n");
    }
}

const CTYPES: [&str; 5] = ["off", "sparse", "normal", "complete", "\n"];

#[allow(clippy::too_many_arguments)]
pub fn do_color(
    game: &mut Game,
    _db: &mut DB,
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
    if ch.is_npc() {
        return;
    }

    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(
            &mut game.descriptors,
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
    let res = {
        tp = search_block(&arg, &CTYPES, false);
        tp.is_none()
    };
    if res {
        send_to_char(
            &mut game.descriptors,
            ch,
            "Usage: color { Off | Sparse | Normal | Complete }\r\n",
        );
        return;
    }
    let tp = tp.unwrap() as i64;
    let ch = chars.get_mut(chid);
    ch.remove_prf_flags_bits(PrefFlags::COLOR_1 | PrefFlags::COLOR_2);
    if (tp & 1) != 0 {
        ch.set_prf_flags_bits(PrefFlags::COLOR_1);
    }
    if (tp & 2) != 0 {
        ch.set_prf_flags_bits(PrefFlags::COLOR_2);
    }
    let ch = chars.get(chid);
    send_to_char(
        &mut game.descriptors,
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

#[allow(clippy::too_many_arguments)]
pub fn do_toggle(
    game: &mut Game,
    _db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf2 = String::new();
    if ch.is_npc() {
        return;
    }

    if ch.get_wimp_lev() == 0 {
        buf2.push_str("OFF");
    } else {
        buf2.push_str(format!("{:3}", ch.get_wimp_lev()).as_str());
    }

    if ch.get_level() >= LVL_IMMORT {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!(
                "      No Hassle: {:3}    Holylight: {:3}    Room Flags:{:3}\r\n",
                onoff!(ch.prf_flagged(PrefFlags::NOHASSLE)),
                onoff!(ch.prf_flagged(PrefFlags::HOLYLIGHT)),
                onoff!(ch.prf_flagged(PrefFlags::ROOMFLAGS))
            )
            .as_str(),
        );
    }

    send_to_char(
        &mut game.descriptors,
        ch,
        format!(
            "Hit Pnt Display: {:3}    Brief Mode: {:3}    Summon Protect: {:3}\r\n\
 Move Display: {:3}    Compact Mode: {:3}    On Quest: {:3}\r\n\
 Mana Display: {:3}    NoTell: {:3}    Repeat Comm.: {:3}\r\n\
 Auto Show Exit: {:3}    Deaf: {:3}    Wimp Level: {:3}\r\n\
 Gossip Channel: {:3}    Auction Channel: {:3}    Grats Channel: {:3}\r\n\
 Color Level: {}\r\n",
            onoff!(ch.prf_flagged(PrefFlags::DISPHP)),
            onoff!(ch.prf_flagged(PrefFlags::BRIEF)),
            onoff!(!ch.prf_flagged(PrefFlags::SUMMONABLE)),
            onoff!(ch.prf_flagged(PrefFlags::DISPMOVE)),
            onoff!(ch.prf_flagged(PrefFlags::COMPACT)),
            yesno!(ch.prf_flagged(PrefFlags::QUEST)),
            onoff!(ch.prf_flagged(PrefFlags::DISPMANA)),
            onoff!(ch.prf_flagged(PrefFlags::NOTELL)),
            yesno!(!ch.prf_flagged(PrefFlags::NOREPEAT)),
            onoff!(ch.prf_flagged(PrefFlags::AUTOEXIT)),
            yesno!(ch.prf_flagged(PrefFlags::DEAF)),
            buf2,
            onoff!(!ch.prf_flagged(PrefFlags::NOGOSS)),
            onoff!(!ch.prf_flagged(PrefFlags::NOAUCT)),
            onoff!(!ch.prf_flagged(PrefFlags::NOGRATZ)),
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

#[allow(clippy::too_many_arguments)]
pub fn do_commands(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);
    let vict;
    let victo;
    if !arg.is_empty() {
        victo = get_char_vis(
            &game.descriptors,
            chars,
            db,
            ch,
            &mut arg,
            None,
            FindFlags::CHAR_WORLD,
        );
        if victo.is_none() || victo.unwrap().is_npc() {
            send_to_char(&mut game.descriptors, ch, "Who is that?\r\n");
            return;
        }
        vict = victo.unwrap();
        if ch.get_level() < vict.get_level() {
            send_to_char(
                &mut game.descriptors,
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
        &mut game.descriptors,
        ch,
        format!(
            "The following {}{} are available to {}:\r\n",
            if wizhelp { "privileged " } else { "" },
            if socials { "socials" } else { "commands" },
            if vict.id() == chid {
                "you"
            } else {
                vic_name.as_ref()
            }
        )
        .as_str(),
    );

    /* cmd_num starts at 1, not 0, to remove 'RESERVED' */
    let mut no = 1;
    let vict_level = vict.get_level();
    for cmd_num in 1..CMD_INFO.len() {
        let i: usize = db.cmd_sort_info[cmd_num];
        if vict_level < CMD_INFO[i].minimum_level {
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
            &mut game.descriptors,
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
        send_to_char(&mut game.descriptors, ch, "\r\n");
    }
}
