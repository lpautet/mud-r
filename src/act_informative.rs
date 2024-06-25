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

use crate::act_social::{do_action, do_insult};
use crate::class::{find_class_bitvector, level_exp, title_female, title_male};
use crate::config::NOPERSON;
use crate::constants::{
    CIRCLEMUD_VERSION, COLOR_LIQUID, CONNECTED_TYPES, DIRS, FULLNESS, MONTH_NAME, ROOM_BITS,
    WEAR_WHERE, WEEKDAYS,
};
use crate::db::DB;
use crate::depot::{DepotId, HasId};
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
    ExtraDescrData, AFF_DETECT_ALIGN, AFF_DETECT_MAGIC, AFF_HIDE, AFF_INVISIBLE,
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
    age, clone_vec2, rand_number, real_time_passed, sprintbit, sprinttype, time_now,
    SECS_PER_MUD_HOUR, SECS_PER_REAL_MIN,
};
use crate::VictimRef;
use crate::{_clrlevel, an, clr, Game, CCCYN, CCGRN, CCRED, CCYEL, COLOR_LEV, TO_NOTVICT};
use crate::{CCNRM, TO_VICT};
use log::{error, info};
use regex::Regex;

pub const SHOW_OBJ_LONG: i32 = 0;
pub const SHOW_OBJ_SHORT: i32 = 1;
pub const SHOW_OBJ_ACTION: i32 = 2;

impl Game {
    fn show_obj_to_char(&mut self, oid: DepotId, chid: DepotId, mode: i32) {
        match mode {
            SHOW_OBJ_LONG => {
                self.send_to_char(chid, format!("{}", self.db.obj(oid).description).as_str());
            }

            SHOW_OBJ_SHORT => {
                self.send_to_char(
                    chid,
                    format!("{}", self.db.obj(oid).short_description).as_str(),
                );
            }

            SHOW_OBJ_ACTION => match self.db.obj(oid).get_obj_type() {
                ITEM_NOTE => {
                    if !RefCell::borrow(&self.db.obj(oid).action_description).is_empty() {
                        let notebuf = format!(
                            "There is something written on it:\r\n\r\n{}",
                            RefCell::borrow(&self.db.obj(oid).action_description)
                        );
                        let d_id = self.db.ch(chid).desc.unwrap();
                        page_string(self, d_id, notebuf.as_str(), true);
                    } else {
                        self.send_to_char(chid, "It's blank.\r\n");
                    }
                    return;
                }
                ITEM_DRINKCON => {
                    self.send_to_char(chid, "It looks like a drink container.");
                }

                _ => {
                    self.send_to_char(chid, "You see nothing special..");
                }
            },

            _ => {
                error!("SYSERR: Bad display mode ({}) in show_obj_to_char().", mode);
                return;
            }
        }

        self.show_obj_modifiers(oid, chid);
        self.send_to_char(chid, "\r\n");
    }

    fn show_obj_modifiers(&mut self, oid: DepotId, chid: DepotId) {
        if self.db.obj(oid).obj_flagged(ITEM_INVISIBLE) {
            self.send_to_char(chid, " (invisible)");
        }
        let ch = self.db.ch(chid);

        if self.db.obj(oid).obj_flagged(ITEM_BLESS) && ch.aff_flagged(AFF_DETECT_ALIGN) {
            self.send_to_char(chid, " ..It glows blue!");
        }
        let ch = self.db.ch(chid);

        if self.db.obj(oid).obj_flagged(ITEM_MAGIC) && ch.aff_flagged(AFF_DETECT_MAGIC) {
            self.send_to_char(chid, " ..It glows yellow!");
        }

        if self.db.obj(oid).obj_flagged(ITEM_GLOW) {
            self.send_to_char(chid, " ..It has a soft glowing aura!");
        }

        if self.db.obj(oid).obj_flagged(ITEM_HUM) {
            self.send_to_char(chid, " ..It emits a faint humming sound!");
        }
    }
}
fn list_obj_to_char(game: &mut Game, list: &Vec<DepotId>, chid: DepotId, mode: i32, show: bool) {
    let mut found = true;

    for oid in list {
        if game.can_see_obj(game.db.ch(chid), game.db.obj(*oid)) {
            game.show_obj_to_char(*oid, chid, mode);
            found = true;
        }
    }
    if !found && show {
        game.send_to_char(chid, " Nothing.\r\n");
    }
}

fn diag_char_to_char(game: &mut Game, i_id: DepotId, chid: DepotId) {
    let i = game.db.ch(i_id);
    let ch = game.db.ch(chid);
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

    let pers = game.pers(i, ch);

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

    game.send_to_char(
        chid,
        format!(
            "{}{} {}\r\n",
            pers.chars().next().unwrap().to_uppercase(),
            &pers[1..],
            DIAGNOSIS[ar_index as usize].text
        )
        .as_str(),
    );
}

fn look_at_char(game: &mut Game, i_id: DepotId, chid: DepotId) {
    let i = game.db.ch(i_id);
    let ch = game.db.ch(chid);
    let mut found;
    //struct obj_data *tmp_obj;

    if ch.desc.is_none() {
        return;
    }

    if !RefCell::borrow(&i.player.description).is_empty() {
        let messg = i.player.description.borrow().clone();
        game.send_to_char(chid, messg.as_str());
    } else {
        game.act(
            "You see nothing special about $m.",
            false,
            Some(i_id),
            None,
            Some(VictimRef::Char(chid)),
            TO_VICT,
        );
    }

    diag_char_to_char(game, i_id, chid);

    let i = game.db.ch(i_id);
    let ch = game.db.ch(chid);

    found = false;
    for j in 0..NUM_WEARS {
        if i.get_eq(j).is_some() && game.can_see_obj(ch, game.db.obj(i.get_eq(j).unwrap())) {
            found = true;
        }
    }

    if found {
        game.send_to_char(chid, "\r\n"); /* act() does capitalization. */
        game.act(
            "$n is using:",
            false,
            Some(i_id),
            None,
            Some(VictimRef::Char(chid)),
            TO_VICT,
        );
        for j in 0..NUM_WEARS {
            let ch = game.db.ch(chid);
            let i = game.db.ch(i_id);
            if i.get_eq(j).is_some() && game.can_see_obj(ch, game.db.obj(i.get_eq(j).unwrap())) {
                game.send_to_char(chid, WEAR_WHERE[j as usize]);
                let i = game.db.ch(i_id);
                game.show_obj_to_char(i.get_eq(j).unwrap(), chid, SHOW_OBJ_SHORT);
            }
        }
    }
    let ch = game.db.ch(chid);
    if i_id != chid && (ch.is_thief() || ch.get_level() >= LVL_IMMORT as u8) {
        found = false;
        game.act(
            "\r\nYou attempt to peek at $s inventory:",
            false,
            Some(i_id),
            None,
            Some(VictimRef::Char(chid)),
            TO_VICT,
        );
        let i = game.db.ch(i_id);
        let list = i.carrying.clone();
        for tmp_obj_id in list {
            let ch = game.db.ch(chid);
            if game.can_see_obj(ch, game.db.obj(tmp_obj_id))
                && rand_number(0, 20) < ch.get_level() as u32
            {
                game.show_obj_to_char(tmp_obj_id, chid, SHOW_OBJ_SHORT);
                found = true;
            }
        }
    }

    if !found {
        game.send_to_char(chid, "You can't see anything.\r\n");
    }
}

fn list_one_char(game: &mut Game, id: DepotId, chid: DepotId) {
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

    if {
        let i = game.db.ch(id);
        i.is_npc() && !i.player.long_descr.is_empty() && i.get_pos() == i.get_default_pos()
    } {
        if game.db.ch(id).aff_flagged(AFF_INVISIBLE) {
            game.send_to_char(chid, "*");
        }

        if game.db.ch(chid).aff_flagged(AFF_DETECT_ALIGN) {
            if game.db.ch(id).is_evil() {
                game.send_to_char(chid, "(Red Aura) ");
            } else if game.db.ch(id).is_good() {
                game.send_to_char(chid, "(Blue Aura) ");
            }
        }
        let messg = game.db.ch(id).player.long_descr.clone();
        game.send_to_char(chid, &messg);

        if game.db.ch(id).aff_flagged(AFF_SANCTUARY) {
            game.act(
                "...$e glows with a bright light!",
                false,
                Some(id),
                None,
                Some(VictimRef::Char(chid)),
                TO_VICT,
            );
        }
        if game.db.ch(id).aff_flagged(AFF_BLIND) {
            game.act(
                "...$e is groping around blindly!",
                false,
                Some(id),
                None,
                Some(VictimRef::Char(chid)),
                TO_VICT,
            );
        }
        return;
    }

    if game.db.ch(id).is_npc() {
        game.send_to_char(
            chid,
            format!(
                "{}{}",
                game.db.ch(id).player.short_descr[0..1].to_uppercase(),
                &game.db.ch(id).player.short_descr[1..]
            )
            .as_str(),
        );
    } else {
        game.send_to_char(
            chid,
            format!(
                "{} {}",
                game.db.ch(id).player.name,
                game.db.ch(id).get_title()
            )
            .as_str(),
        );
    }

    if game.db.ch(id).aff_flagged(AFF_INVISIBLE) {
        game.send_to_char(chid, " (invisible)");
    }
    if game.db.ch(id).aff_flagged(AFF_HIDE) {
        game.send_to_char(chid, " (hidden)");
    }
    if !game.db.ch(id).is_npc() && game.db.ch(id).desc.is_none() {
        game.send_to_char(chid, " (linkless)");
    }
    if !game.db.ch(id).is_npc() && game.db.ch(id).plr_flagged(PLR_WRITING) {
        game.send_to_char(chid, " (writing)");
    }
    if game.db.ch(id).get_pos() != POS_FIGHTING {
        game.send_to_char(chid, POSITIONS[game.db.ch(id).get_pos() as usize]);
    } else {
        if game.db.ch(id).fighting_id().is_some() {
            game.send_to_char(chid, " is here, fighting ");
            if game.db.ch(game.db.ch(id).fighting_id().unwrap()).id() == chid {
                game.send_to_char(chid, "YOU!");
            } else {
                if game.db.ch(id).in_room()
                    == game.db.ch(game.db.ch(id).fighting_id().unwrap()).in_room()
                {
                    game.send_to_char(
                        chid,
                        format!(
                            "{}!",
                            game.pers(
                                game.db.ch(game.db.ch(id).fighting_id().unwrap()),
                                game.db.ch(chid)
                            )
                        )
                        .as_str(),
                    );
                } else {
                    game.send_to_char(chid, "someone who has already left!");
                }
            }
        } else {
            /* NIL fighting pointer */
            game.send_to_char(chid, " is here struggling with thin air.");
        }
    }

    if game.db.ch(chid).aff_flagged(AFF_DETECT_ALIGN) {
        if game.db.ch(id).is_evil() {
            game.send_to_char(chid, " (Red Aura)");
        } else if game.db.ch(id).is_good() {
            game.send_to_char(chid, " (Blue Aura)");
        }
    }
    game.send_to_char(chid, "\r\n");

    if game.db.ch(id).aff_flagged(AFF_SANCTUARY) {
        game.act(
            "...$e glows with a bright light!",
            false,
            Some(id),
            None,
            Some(VictimRef::Char(chid)),
            TO_VICT,
        );
    }
}

fn list_char_to_char(game: &mut Game, list: &Vec<DepotId>, chid: DepotId) {
    for id in list {
        if *id != chid {
            let ch = game.db.ch(chid);
            let obj = game.db.ch(*id);
            if game.can_see(ch, obj) {
                list_one_char(game, *id, chid);
            } else if game.db.is_dark(ch.in_room())
                && !ch.can_see_in_dark()
                && obj.aff_flagged(AFF_INFRAVISION)
            {
                game.send_to_char(
                    chid,
                    "You see a pair of glowing red eyes looking your way.\r\n",
                );
            }
        }
    }
}

fn do_auto_exits(game: &mut Game, chid: DepotId) {
    let ch = game.db.ch(chid);
    let mut slen = 0;
    game.send_to_char(chid, format!("{}[ Exits: ", CCCYN!(ch, C_NRM)).as_str());
    for door in 0..NUM_OF_DIRS {
        let ch = game.db.ch(chid);
        if game.db.exit(ch, door).is_none()
            || game.db.exit(ch, door).as_ref().unwrap().to_room == NOWHERE
        {
            continue;
        }
        if game
            .db
            .exit(ch, door)
            .as_ref()
            .unwrap()
            .exit_flagged(EX_CLOSED)
        {
            continue;
        }
        game.send_to_char(chid, format!("{} ", DIRS[door].to_lowercase()).as_str());
        slen += 1;
    }
    let ch = game.db.ch(chid);
    game.send_to_char(
        chid,
        format!(
            "{}]{}\r\n",
            if slen != 0 { "" } else { "None!" },
            CCNRM!(ch, C_NRM)
        )
        .as_str(),
    );
}

pub fn do_exits(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    if ch.aff_flagged(AFF_BLIND) {
        game.send_to_char(chid, "You can't see a damned thing, you're blind!\r\n");
        return;
    }
    game.send_to_char(chid, "Obvious exits:\r\n");
    let mut len = 0;
    for door in 0..NUM_OF_DIRS {
        let ch = game.db.ch(chid);
        if game.db.exit(ch, door).is_none()
            || game.db.exit(ch, door).as_ref().unwrap().to_room == NOWHERE
        {
            continue;
        }
        if game
            .db
            .exit(ch, door)
            .as_ref()
            .unwrap()
            .exit_flagged(EX_CLOSED)
        {
            continue;
        }
        len += 1;

        let oexit = game.db.exit(ch, door);
        let exit = oexit.as_ref().unwrap();
        if ch.get_level() >= LVL_IMMORT as u8 {
            game.send_to_char(
                chid,
                format!(
                    "{} - [{:5}] {}\r\n",
                    DIRS[door as usize],
                    game.db.get_room_vnum(exit.to_room),
                    game.db.world[exit.to_room as usize].name
                )
                .as_str(),
            );
        } else {
            game.send_to_char(
                chid,
                format!(
                    "{} - {}\r\n",
                    DIRS[door as usize],
                    if game.db.is_dark(exit.to_room) && !ch.can_see_in_dark() {
                        "Too dark to tell."
                    } else {
                        game.db.world[exit.to_room as usize].name.as_str()
                    }
                )
                .as_str(),
            );
        }
    }

    if len == 0 {
        game.send_to_char(chid, " None.\r\n");
    }
}

pub fn look_at_room(game: &mut Game, chid: DepotId, ignore_brief: bool) {
    let ch = game.db.ch(chid);
    if ch.desc.is_none() {
        return;
    }

    if game.db.is_dark(ch.in_room()) && !ch.can_see_in_dark() {
        game.send_to_char(chid, "It is pitch black...\r\n");
        return;
    } else if ch.aff_flagged(AFF_BLIND) {
        game.send_to_char(chid, "You see nothing but infinite darkness...\r\n");
        return;
    }
    game.send_to_char(chid, format!("{}", CCCYN!(ch, C_NRM)).as_str());

    let ch = game.db.ch(chid);

    if !ch.is_npc() && ch.prf_flagged(PRF_ROOMFLAGS) {
        let mut buf = String::new();
        sprintbit(
            game.db.room_flags(ch.in_room()) as i64,
            &ROOM_BITS,
            &mut buf,
        );
        game.send_to_char(
            chid,
            format!(
                "[{}] {} [{}]",
                game.db.get_room_vnum(ch.in_room()),
                game.db.world[ch.in_room() as usize].name,
                buf
            )
            .as_str(),
        );
    } else {
        game.send_to_char(
            chid,
            format!("{}", game.db.world[ch.in_room() as usize].name).as_str(),
        );
    }

    let ch = game.db.ch(chid);
    game.send_to_char(chid, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());
    let ch = game.db.ch(chid);

    if (!ch.is_npc() && !ch.prf_flagged(PRF_BRIEF))
        || ignore_brief
        || game.db.room_flagged(ch.in_room(), ROOM_DEATH)
    {
        game.send_to_char(
            chid,
            format!("{}", game.db.world[ch.in_room() as usize].description).as_str(),
        );
    }

    /* autoexits */
    let ch = game.db.ch(chid);
    if !ch.is_npc() && ch.prf_flagged(PRF_AUTOEXIT) {
        do_auto_exits(game, chid);
    }

    /* now list characters & objects */
    let ch = game.db.ch(chid);
    game.send_to_char(chid, format!("{}", CCGRN!(ch, C_NRM)).as_str());
    let ch = game.db.ch(chid);
    let list = clone_vec2(&game.db.world[ch.in_room() as usize].contents);
    list_obj_to_char(game, &list, chid, SHOW_OBJ_LONG, false);
    let ch = game.db.ch(chid);
    game.send_to_char(chid, format!("{}", CCYEL!(ch, C_NRM)).as_str());
    let ch = game.db.ch(chid);
    let list = clone_vec2(&game.db.world[ch.in_room() as usize].peoples);
    list_char_to_char(game, &list, chid);
    let ch = game.db.ch(chid);
    game.send_to_char(chid, format!("{}", CCNRM!(ch, C_NRM)).as_str());
}

fn look_in_direction(game: &mut Game, chid: DepotId, dir: i32) {
    let ch = game.db.ch(chid);
    if game.db.exit(ch, dir as usize).is_some() {
        if !game
            .db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .general_description
            .is_empty()
        {
            game.send_to_char(
                chid,
                format!(
                    "{}",
                    game.db
                        .exit(ch, dir as usize)
                        .as_ref()
                        .unwrap()
                        .general_description
                )
                .as_str(),
            );
        } else {
            game.send_to_char(chid, "You see nothing special.\r\n");
        }
        let ch = game.db.ch(chid);
        if game
            .db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .exit_flagged(EX_CLOSED)
            && !game
                .db
                .exit(ch, dir as usize)
                .as_ref()
                .unwrap()
                .keyword
                .is_empty()
        {
            game.send_to_char(
                chid,
                format!(
                    "The {} is closed.\r\n",
                    fname(
                        game.db
                            .exit(ch, dir as usize)
                            .as_ref()
                            .unwrap()
                            .keyword
                            .as_ref()
                    )
                )
                .as_str(),
            );
        } else if game
            .db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .exit_flagged(EX_ISDOOR)
            && !game
                .db
                .exit(ch, dir as usize)
                .as_ref()
                .unwrap()
                .keyword
                .is_empty()
        {
            game.send_to_char(
                chid,
                format!(
                    "The {} is open.\r\n",
                    fname(
                        game.db
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
            game.send_to_char(chid, "Nothing special there...\r\n");
        }
    }
}

fn look_in_obj(game: &mut Game, chid: DepotId, arg: &str) {
    let mut dummy = None;
    let mut oid = None;
    let bits;

    if arg.is_empty() {
        game.send_to_char(chid, "Look in what?\r\n");
        return;
    }
    bits = game.generic_find(
        arg,
        (FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_OBJ_EQUIP) as i64,
        chid,
        &mut dummy,
        &mut oid,
    );
    if bits == 0 {
        game.send_to_char(
            chid,
            format!("There doesn't seem to be {} {} here.\r\n", an!(arg), arg).as_str(),
        );
    } else if game.db.obj(oid.unwrap()).get_obj_type() != ITEM_DRINKCON
        && game.db.obj(oid.unwrap()).get_obj_type() != ITEM_FOUNTAIN
        && game.db.obj(oid.unwrap()).get_obj_type() != ITEM_CONTAINER
    {
        game.send_to_char(chid, "There's nothing inside that!\r\n");
    } else {
        if game.db.obj(oid.unwrap()).get_obj_type() == ITEM_CONTAINER {
            if game.db.obj(oid.unwrap()).objval_flagged(CONT_CLOSED) {
                game.send_to_char(chid, "It is closed.\r\n");
            } else {
                game.send_to_char(
                    chid,
                    fname(game.db.obj(oid.unwrap()).name.as_ref()).as_ref(),
                );
                match bits {
                    FIND_OBJ_INV => {
                        game.send_to_char(chid, " (carried): \r\n");
                    }
                    FIND_OBJ_ROOM => {
                        game.send_to_char(chid, " (here): \r\n");
                    }
                    FIND_OBJ_EQUIP => {
                        game.send_to_char(chid, " (used): \r\n");
                    }
                    _ => {}
                }

                list_obj_to_char(
                    game,
                    &game.db.obj(oid.unwrap()).contains.clone(),
                    chid,
                    SHOW_OBJ_SHORT,
                    true,
                );
            }
        } else {
            /* item must be a fountain or drink container */
            if game.db.obj(oid.unwrap()).get_obj_val(1) <= 0 {
                game.send_to_char(chid, "It is empty.\r\n");
            } else {
                if game.db.obj(oid.unwrap()).get_obj_val(0) <= 0
                    || game.db.obj(oid.unwrap()).get_obj_val(1)
                        > game.db.obj(oid.unwrap()).get_obj_val(0)
                {
                    game.send_to_char(chid, "Its contents seem somewhat murky.\r\n");
                    /* BUG */
                } else {
                    let mut buf2 = String::new();
                    let amt = game.db.obj(oid.unwrap()).get_obj_val(1) * 3
                        / game.db.obj(oid.unwrap()).get_obj_val(0);
                    sprinttype(
                        game.db.obj(oid.unwrap()).get_obj_val(2),
                        &COLOR_LIQUID,
                        &mut buf2,
                    );
                    game.send_to_char(
                        chid,
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

fn find_exdesc(word: &str, list: &Vec<ExtraDescrData>) -> Option<Rc<str>> {
    for i in list {
        if isname(word, i.keyword.as_ref()) {
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
fn look_at_target(game: &mut Game, chid: DepotId, arg: &str) {
    let ch = game.db.ch(chid);
    let mut i = 0;
    let mut found = false;
    let mut found_char_id = None;
    let mut found_obj_id = None;

    if ch.desc.is_none() {
        return;
    }

    if arg.is_empty() {
        game.send_to_char(chid, "Look at what?\r\n");
        return;
    }

    let bits = game.generic_find(
        arg,
        (FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_OBJ_EQUIP | FIND_CHAR_ROOM) as i64,
        chid,
        &mut found_char_id,
        &mut found_obj_id,
    );

    /* Is the target a character? */
    if found_char_id.is_some() {
        let found_char_id = found_char_id.unwrap();
        look_at_char(game, found_char_id, chid);
        if chid != found_char_id {
            let ch = game.db.ch(chid);
            if game.can_see(game.db.ch(found_char_id), ch) {
                game.act(
                    "$n looks at you.",
                    true,
                    Some(chid),
                    None,
                    Some(VictimRef::Char(found_char_id)),
                    TO_VICT,
                );
            }
            game.act(
                "$n looks at $N.",
                true,
                Some(chid),
                None,
                Some(VictimRef::Char(found_char_id)),
                TO_NOTVICT,
            );
        }
        return;
    }
    let mut arg = arg.to_string();
    let fnum = get_number(&mut arg);
    /* Strip off "number." from 2.foo and friends. */
    if fnum == 0 {
        game.send_to_char(chid, "Look at what?\r\n");
        return;
    }

    let ch = game.db.ch(chid);
    /* Does the argument match an extra desc in the room? */
    let desc = find_exdesc(&arg, &game.db.world[ch.in_room() as usize].ex_descriptions);
    if desc.is_some() {
        i += 1;
        if i == fnum {
            let d_id = ch.desc.unwrap();
            page_string(game, d_id, desc.as_ref().unwrap(), false);
            return;
        }
    }

    /* Does the argument match an extra desc in the char's equipment? */
    for j in 0..NUM_WEARS {
        let ch = game.db.ch(chid);
        if ch.get_eq(j).is_some() && game.can_see_obj(ch, game.db.obj(ch.get_eq(j).unwrap())) {
            let desc = find_exdesc(&arg, &game.db.obj(ch.get_eq(j).unwrap()).ex_descriptions);
            if desc.is_some() {
                i += 1;
                if i == fnum {
                    game.send_to_char(chid, desc.as_ref().unwrap());
                    found = true;
                }
            }
        }
    }

    /* Does the argument match an extra desc in the char's inventory? */
    let ch = game.db.ch(chid);
    let list = ch.carrying.clone();
    for oid in list.into_iter() {
        if game.can_see_obj(game.db.ch(chid), game.db.obj(oid)) {
            let desc = find_exdesc(&arg, &game.db.obj(oid).ex_descriptions);
            if desc.is_some() {
                i += 1;
                if i == fnum {
                    game.send_to_char(chid, desc.as_ref().unwrap());
                    found = true;
                }
            }
        }
    }

    /* Does the argument match an extra desc of an object in the room? */
    let ch = game.db.ch(chid);
    for oid in game.db.world[ch.in_room() as usize].contents.clone() {
        if game.can_see_obj(game.db.ch(chid), game.db.obj(oid)) {
            if let Some(desc) = find_exdesc(&arg, &game.db.obj(oid).ex_descriptions) {
                i += 1;
                if i == fnum {
                    game.send_to_char(chid, desc.as_ref());
                    found = true;
                }
            }
        }
    }

    /* If an object was found back in generic_find */
    if bits != 0 {
        if !found {
            game.show_obj_to_char(found_obj_id.unwrap(), chid, SHOW_OBJ_ACTION);
        } else {
            game.show_obj_modifiers(found_obj_id.unwrap(), chid);
            game.send_to_char(chid, "\r\n");
        }
    } else if !found {
        game.send_to_char(chid, "You do not see that here.\r\n");
    }
}

pub fn do_look(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = game.db.ch(chid);
    if ch.desc.is_none() {
        return;
    }
    if ch.get_pos() < POS_SLEEPING {
        game.send_to_char(chid, "You can't see anything but stars!\r\n");
    } else if ch.aff_flagged(AFF_BLIND) {
        game.send_to_char(chid, "You can't see a damned thing, you're blind!\r\n");
    } else if game.db.is_dark(ch.in_room()) && !ch.can_see_in_dark() {
        game.send_to_char(chid, "It is pitch black...\r\n");
        let ch = game.db.ch(chid);
        let list = clone_vec2(&game.db.world[ch.in_room() as usize].peoples);
        list_char_to_char(game, &list, chid);
        /* glowing red eyes */
    } else {
        let mut argument = argument.to_string();
        let mut arg = String::new();
        let mut arg2 = String::new();

        half_chop(&mut argument, &mut arg, &mut arg2);

        if subcmd == SCMD_READ {
            if arg.is_empty() {
                game.send_to_char(chid, "Read what?\r\n");
            } else {
                look_at_target(game, chid, &mut arg);
            }
            return;
        }
        let look_type;
        if arg.is_empty() {
            /* "look" alone, without an argument at all */
            look_at_room(game, chid, true);
        } else if is_abbrev(arg.as_ref(), "in") {
            look_in_obj(game, chid, arg2.as_str());
            /* did the char type 'look <direction>?' */
        } else if {
            look_type = search_block(arg.as_str(), &DIRS, false);
            look_type
        } != None
        {
            look_in_direction(game, chid, look_type.unwrap() as i32);
        } else if is_abbrev(arg.as_ref(), "at") {
            look_at_target(game, chid, arg2.as_ref());
        } else {
            look_at_target(game, chid, arg.as_ref());
        }
    }
}

pub fn do_examine(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    // struct char_data *tmp_char;
    // struct obj_data *tmp_object;
    // char tempsave[MAX_INPUT_LENGTH], arg[MAX_INPUT_LENGTH];
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        game.send_to_char(chid, "Examine what?\r\n");
        return;
    }

    /* look_at_target() eats the number. */
    look_at_target(game, chid, &arg);
    let mut tmp_char_id = None;
    let mut tmp_object_id = None;
    game.generic_find(
        &arg,
        (FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_CHAR_ROOM | FIND_OBJ_EQUIP) as i64,
        chid,
        &mut tmp_char_id,
        &mut tmp_object_id,
    );

    if tmp_object_id.is_some() {
        let tmp_object_id = tmp_object_id.unwrap();
        if game.db.obj(tmp_object_id).get_obj_type() == ITEM_DRINKCON
            || game.db.obj(tmp_object_id).get_obj_type() == ITEM_FOUNTAIN
            || game.db.obj(tmp_object_id).get_obj_type() == ITEM_CONTAINER
        {
            game.send_to_char(chid, "When you look inside, you see:\r\n");
            look_in_obj(game, chid, &arg);
        }
    }
}

pub fn do_gold(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    if ch.get_gold() == 0 {
        game.send_to_char(chid, "You're broke!\r\n");
    } else if ch.get_gold() == 1 {
        game.send_to_char(chid, "You have one miserable little gold coin.\r\n");
    } else {
        game.send_to_char(
            chid,
            format!("You have {} gold coins.\r\n", ch.get_gold()).as_str(),
        );
    }
}

pub fn do_score(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    if ch.is_npc() {
        return;
    }

    game.send_to_char(
        chid,
        format!("You are {} years old.\r\n", ch.get_age()).as_str(),
    );
    let ch = game.db.ch(chid);
    if age(ch).month == 0 && age(ch).day == 0 {
        game.send_to_char(chid, "  It's your birthday today.\r\n");
    } else {
        game.send_to_char(chid, "\r\n");
    }
    let ch = game.db.ch(chid);
    game.send_to_char(
        chid,
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
    let ch = game.db.ch(chid);
    game.send_to_char(
        chid,
        format!(
            "Your armor class is {}/10, and your alignment is {}.\r\n",
            compute_armor_class(ch),
            ch.get_alignment()
        )
        .as_str(),
    );
    let ch = game.db.ch(chid);
    game.send_to_char(
        chid,
        format!(
            "You have scored {} exp, and have {} gold coins.\r\n",
            ch.get_exp(),
            ch.get_gold()
        )
        .as_str(),
    );
    let ch = game.db.ch(chid);
    if ch.get_level() < LVL_IMMORT as u8 {
        game.send_to_char(
            chid,
            format!(
                "You need {} exp to reach your next level.\r\n",
                level_exp(ch.get_class(), (ch.get_level() + 1) as i16) - ch.get_exp()
            )
            .as_str(),
        );
    }
    let ch = game.db.ch(chid);
    let playing_time = real_time_passed(
        (time_now() - ch.player.time.logon) + ch.player.time.played as u64,
        0,
    );
    game.send_to_char(
        chid,
        format!(
            "You have been playing for {} day{} and {} hour{}.\r\n",
            playing_time.day,
            if playing_time.day == 1 { "" } else { "s" },
            playing_time.hours,
            if playing_time.hours == 1 { "" } else { "s" }
        )
        .as_str(),
    );
    let ch = game.db.ch(chid);
    game.send_to_char(
        chid,
        format!(
            "This ranks you as {} {} (level {}).\r\n",
            ch.get_name(),
            ch.get_title(),
            ch.get_level()
        )
        .as_str(),
    );
    let ch = game.db.ch(chid);
    match ch.get_pos() {
        POS_DEAD => {
            game.send_to_char(chid, "You are DEAD!\r\n");
        }
        POS_MORTALLYW => {
            game.send_to_char(chid, "You are mortally wounded!  You should seek help!\r\n");
        }
        POS_INCAP => {
            game.send_to_char(chid, "You are incapacitated, slowly fading away...\r\n");
        }
        POS_STUNNED => {
            game.send_to_char(chid, "You are stunned!  You can't move!\r\n");
        }
        POS_SLEEPING => {
            game.send_to_char(chid, "You are sleeping.\r\n");
        }
        POS_RESTING => {
            game.send_to_char(chid, "You are resting.\r\n");
        }
        POS_SITTING => {
            game.send_to_char(chid, "You are sitting.\r\n");
        }
        POS_FIGHTING => {
            let v = game.pers(game.db.ch(ch.fighting_id().unwrap()), ch);
            game.send_to_char(
                chid,
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
        POS_STANDING => {
            game.send_to_char(chid, "You are standing.\r\n");
        }
        _ => {
            game.send_to_char(chid, "You are floating.\r\n");
        }
    }
    let ch = game.db.ch(chid);
    if ch.get_cond(DRUNK) > 10 {
        game.send_to_char(chid, "You are intoxicated.\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.get_cond(FULL) == 0 {
        game.send_to_char(chid, "You are hungry.\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.get_cond(THIRST) == 0 {
        game.send_to_char(chid, "You are thirsty.\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.aff_flagged(AFF_BLIND) {
        game.send_to_char(chid, "You have been blinded!\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.aff_flagged(AFF_INVISIBLE) {
        game.send_to_char(chid, "You are invisible.\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.aff_flagged(AFF_DETECT_INVIS) {
        game.send_to_char(
            chid,
            "You are sensitive to the presence of invisible things.\r\n",
        );
    }
    let ch = game.db.ch(chid);
    if ch.aff_flagged(AFF_SANCTUARY) {
        game.send_to_char(chid, "You are protected by Sanctuary.\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.aff_flagged(AFF_POISON) {
        game.send_to_char(chid, "You are poisoned!\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.aff_flagged(AFF_CHARM) {
        game.send_to_char(chid, "You have been charmed!\r\n");
    }
    let ch = game.db.ch(chid);
    if affected_by_spell(ch, SPELL_ARMOR as i16) {
        game.send_to_char(chid, "You feel protected.\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.aff_flagged(AFF_INFRAVISION) {
        game.send_to_char(chid, "Your eyes are glowing red.\r\n");
    }
    let ch = game.db.ch(chid);
    if ch.aff_flagged(PRF_SUMMONABLE) {
        game.send_to_char(chid, "You are summonable by other players.\r\n");
    }
}

pub fn do_inventory(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    game.send_to_char(chid, "You are carrying:\r\n");
    let list = game.db.ch(chid).carrying.clone();
    list_obj_to_char(game, &list, chid, SHOW_OBJ_SHORT, true);
}

pub fn do_equipment(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let mut found = false;
    game.send_to_char(chid, "You are using:\r\n");
    for i in 0..NUM_WEARS {
        let ch = game.db.ch(chid);
        if ch.get_eq(i).is_some() {
            if game.can_see_obj(ch, game.db.obj(ch.get_eq(i).unwrap())) {
                let oid = ch.get_eq(i).unwrap();
                game.send_to_char(chid, format!("{}", WEAR_WHERE[i as usize]).as_str());
                game.show_obj_to_char(oid, chid, SHOW_OBJ_SHORT);
                found = true;
            } else {
                game.send_to_char(chid, format!("{}", WEAR_WHERE[i as usize]).as_str());
                game.send_to_char(chid, "Something.\r\n");
                found = true;
            }
        }
    }
    if !found {
        game.send_to_char(chid, " Nothing.\r\n");
    }
}

pub fn do_time(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    /* day in [1..35] */
    let day = game.db.time_info.day + 1;

    /* 35 days in a month, 7 days a week */
    let weekday = ((35 * game.db.time_info.month) + day) % 7;

    game.send_to_char(
        chid,
        format!(
            "It is {} o'clock {}, on {}.\r\n",
            if game.db.time_info.hours % 12 == 0 {
                12
            } else {
                game.db.time_info.hours % 12
            },
            if game.db.time_info.hours >= 12 {
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

    game.send_to_char(
        chid,
        format!(
            "The {}{} Day of the {}, Year {}.\r\n",
            day, suf, MONTH_NAME[game.db.time_info.month as usize], game.db.time_info.year
        )
        .as_str(),
    );
}

pub fn do_weather(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    const SKY_LOOK: [&str; 4] = [
        "cloudless",
        "cloudy",
        "rainy",
        "lit by flashes of lightning",
    ];
    if game.db.outside(ch) {
        let messg = format!(
            "The sky is {} and {}.\r\n",
            SKY_LOOK[game.db.weather_info.sky as usize],
            if game.db.weather_info.change >= 0 {
                "you feel a warm wind from south"
            } else {
                "your foot tells you bad weather is due"
            }
        );
        game.send_to_char(chid, messg.as_str());
        let ch = game.db.ch(chid);
        if ch.get_level() >= LVL_GOD as u8 {
            game.send_to_char(
                chid,
                format!(
                    "Pressure: {} (change: {}), Sky: {} ({})\r\n",
                    game.db.weather_info.pressure,
                    game.db.weather_info.change,
                    game.db.weather_info.sky,
                    SKY_LOOK[game.db.weather_info.sky as usize],
                )
                .as_str(),
            );
        }
    } else {
        game.send_to_char(chid, "You have no feeling about the weather at all.\r\n");
    }
}

pub fn do_help(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    if ch.desc.is_none() {
        return;
    }

    let argument = argument.trim_start();
    let d_id = ch.desc.unwrap();

    if argument.len() == 0 {
        page_string(
            game,
            d_id,
            &game.db.help.clone(),
            false,
        );
        return;
    }
    if game.db.help_table.len() == 0 {
        game.send_to_char(chid, "No help available.\r\n");
        return;
    }

    let mut bot = 0;
    let mut top = game.db.help_table.len() - 1;

    loop {
        let mut mid = (bot + top) / 2;
        if bot > top {
            game.send_to_char(chid, "There is no help on that word.\r\n");
            return;
        } else if game.db.help_table[mid].keyword.starts_with(argument) {
            /* trace backwards to find first matching entry. Thanks Jeff Fink! */
            while mid > 0 && game.db.help_table[mid - 1].keyword.starts_with(argument) {
                mid -= 1;
            }
            page_string(
                game,
                d_id,
                &game.db.help_table[mid].entry.clone(),
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
pub fn do_who(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
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
                    game.send_to_char(chid, WHO_FORMAT);
                    return;
                }
            }
        } else {
            /* endif */
            game.send_to_char(chid, WHO_FORMAT);
            return;
        }
    } /* end while (parser) */

    game.send_to_char(chid, "Players\r\n-------\r\n");
    let mut num_can_see = 0;

    for d_id in game.descriptor_list.ids() {
        if game.desc(d_id).state() != ConPlaying {
            continue;
        }

        let tch_id;
        if game.desc(d_id).original.is_some() {
            tch_id = game.desc(d_id).original;
        } else if {
            tch_id = game.desc(d_id).character;
            tch_id.is_none()
        } {
            continue;
        }
        let tch_id = tch_id.unwrap().clone();
        let tch = game.db.ch(tch_id);

        if !name_search.is_empty()
            && tch.get_name().as_ref() != &name_search
            && !tch.get_title().contains(&name_search)
        {
            continue;
        }
        let ch = game.db.ch(chid);
        if !game.can_see(ch, &tch) || tch.get_level() < low as u8 || tch.get_level() > high as u8 {
            continue;
        }
        if outlaws && !tch.plr_flagged(PLR_KILLER) && !tch.plr_flagged(PLR_THIEF) {
            continue;
        }
        if questwho && !tch.prf_flagged(PRF_QUEST) {
            continue;
        }
        if localwho
            && game.db.world[ch.in_room() as usize].zone
                != game.db.world[tch.in_room() as usize].zone
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
            let messg = format!(
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
            );
            game.send_to_char(chid, messg.as_str());
        } else {
            num_can_see += 1;
            let messg = format!(
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
            );
            game.send_to_char(chid, messg.as_str());
            let tch = game.db.ch(tch_id);

            if tch.get_invis_lev() != 0 {
                game.send_to_char(chid, format!(" (i{})", tch.get_invis_lev()).as_str());
            } else if tch.aff_flagged(AFF_INVISIBLE) {
                game.send_to_char(chid, " (invis)");
            }
            let tch = game.db.ch(tch_id);

            if tch.plr_flagged(PLR_MAILING) {
                game.send_to_char(chid, " (mailing)");
            } else if tch.plr_flagged(PLR_WRITING) {
                game.send_to_char(chid, " (writing)");
            }
            let tch = game.db.ch(tch_id);

            if tch.plr_flagged(PRF_DEAF) {
                game.send_to_char(chid, " (deaf)");
            }
            let tch = game.db.ch(tch_id);

            if tch.prf_flagged(PRF_NOTELL) {
                game.send_to_char(chid, " (notell)");
            }
            let tch = game.db.ch(tch_id);

            if tch.prf_flagged(PRF_QUEST) {
                game.send_to_char(chid, " (quest)");
            }
            let tch = game.db.ch(tch_id);

            if tch.plr_flagged(PLR_THIEF) {
                game.send_to_char(chid, " (THIEF)");
            }
            let tch = game.db.ch(tch_id);

            if tch.plr_flagged(PLR_KILLER) {
                game.send_to_char(chid, " (KILLER)");
            }
            let tch = game.db.ch(tch_id);

            if tch.get_level() >= LVL_IMMORT as u8 {
                let ch = game.db.ch(chid);
                game.send_to_char(chid, CCNRM!(ch, C_SPR));
            }
            game.send_to_char(chid, "\r\n");
        } /* endif shortlist */
    } /* end of for */
    if short_list && (num_can_see % 4) != 0 {
        game.send_to_char(chid, "\r\n");
    }
    if num_can_see == 0 {
        game.send_to_char(chid, "\r\nNobody at all!\r\n");
    } else if num_can_see == 1 {
        game.send_to_char(chid, "\r\nOne lonely character displayed.\r\n");
    } else {
        game.send_to_char(
            chid,
            format!("\r\n{} characters displayed.\r\n", num_can_see).as_str(),
        );
    }
}

const USERS_FORMAT: &str =
    "format: users [-l minlevel[-maxlevel]] [-n name] [-h host] [-c classlist] [-o] [-p]\r\n";

/* BIG OL' FIXME: Rewrite it all. Similar to do_who(). */
pub fn do_users(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
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
                    game.send_to_char(chid, USERS_FORMAT);
                    return;
                }
            } /* end of switch */
        } else {
            /* endif */
            game.send_to_char(chid, USERS_FORMAT);
            return;
        }
    } /* end while (parser) */
    game.send_to_char(
        chid,
        "Num Class   Name         State          Idl Login@   Site\r\n\
--- ------- ------------ -------------- --- -------- ------------------------\r\n",
    );

    one_argument(argument, &mut arg);
    for d_id in game.descriptor_list.ids() {
        if game.desc(d_id).state() != ConPlaying && playing {
            continue;
        }
        if game.desc(d_id).state() == ConPlaying && deadweight {
            continue;
        }
        if game.desc(d_id).state() == ConPlaying {
            let character;
            if game.desc(d_id).original.is_some() {
                character = game.desc(d_id).original;
            } else if {
                character = game.desc(d_id).character;
                character.is_none()
            } {
                continue;
            }
            let tch_id = character.unwrap();
            let tch = game.db.ch(tch_id);

            if !host_search.is_empty() && !game.desc(d_id).host.contains(&host_search) {
                continue;
            }
            if !name_search.is_empty() && tch.get_name().as_ref() != &name_search {
                continue;
            }
            let ch = game.db.ch(chid);
            if !game.can_see(ch, tch) || tch.get_level() < low as u8 || tch.get_level() > high as u8
            {
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

            if game.desc(d_id).original.is_some() {
                classname = format!(
                    "[{:2} {}]",
                    game.db.ch(game.desc(d_id).original.unwrap()).get_level(),
                    game.db.ch(game.desc(d_id).original.unwrap()).class_abbr()
                );
            } else {
                classname = format!(
                    "[{:2} {}]",
                    game.db.ch(game.desc(d_id).character.unwrap()).get_level(),
                    game.db.ch(game.desc(d_id).character.unwrap()).class_abbr()
                );
            }
        } else {
            classname = "   -   ".to_string();
        }

        let timeptr = game.desc(d_id).login_time.elapsed().as_secs().to_string();

        let state;
        if game.desc(d_id).state() == ConPlaying && game.desc(d_id).original.is_some() {
            state = "Switched";
        } else {
            state = CONNECTED_TYPES[game.desc(d_id).state() as usize];
        }

        let idletime;
        if game.desc(d_id).character.is_some()
            && game.desc(d_id).state() == ConPlaying
            && game.db.ch(game.desc(d_id).character.unwrap()).get_level() < LVL_GOD as u8
        {
            idletime = format!(
                "{:3}",
                game.db.ch(game.desc(d_id)
                    .character
                    .unwrap())
                    .char_specials
                    .timer
                    * SECS_PER_MUD_HOUR as i32
                    / SECS_PER_REAL_MIN as i32
            );
        } else {
            idletime = "".to_string();
        }

        let mut line = format!(
            "{:3} {:7} {:12} {:14} {:3} {:8} ",
            game.desc(d_id).desc_num,
            classname,
            if game.desc(d_id).original.is_some()
                && !game.db.ch(game
                    .desc(d_id)
                    .original
                    .unwrap())
                    .player
                    .name
                    .is_empty()
            {
                game.db.ch(game.desc(d_id)
                    .original
                    .unwrap())
                    .player
                    .name
                    .clone()
            } else if game.desc(d_id).character.is_some()
                && !game.db.ch(game
                    .desc(d_id)
                    .character
                    .unwrap())
                    .player
                    .name
                    .is_empty()
            {
                game.db.ch(game.desc(d_id)
                    .character
                    .unwrap())
                    .player
                    .name
                    .clone()
            } else {
                Rc::from("UNDEFINED")
            },
            state,
            idletime,
            timeptr
        );

        if !game.desc(d_id).host.is_empty() {
            line.push_str(&format!("[{}]\r\n", game.desc(d_id).host));
        } else {
            line.push_str("[Hostname unknown]\r\n");
        }

        if game.desc(d_id).state() != ConPlaying {
            let ch = game.db.ch(chid);
            line.push_str(&format!(
                "{}{}{}",
                CCGRN!(ch, C_SPR),
                line,
                CCNRM!(ch, C_SPR)
            ));
        }
        let ch = game.db.ch(chid);
        if game.desc(d_id).state() != ConPlaying
            || (game.desc(d_id).state() == ConPlaying
                && game.can_see(ch, game.db.ch(game.desc(d_id).character.unwrap())))
        {
            game.send_to_char(chid, &line);
            num_can_see += 1;
        }
    }

    game.send_to_char(
        chid,
        format!("\r\n{} visible sockets connected.\r\n", num_can_see).as_str(),
    );
}

/* Generic page_string function for displaying text */
pub fn do_gen_ps(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, subcmd: i32) {
    let ch = game.db.ch(chid);
    let d_id = ch.desc.unwrap();
    match subcmd {
        SCMD_CREDITS => {
            page_string(game, d_id, game.db.credits.clone().as_ref(), false);
        }
        SCMD_NEWS => {
            page_string(game, d_id, &game.db.news.clone().as_ref(), false);
        }
        SCMD_INFO => {
            page_string(game, d_id, &game.db.info.clone().as_ref(), false);
        }
        SCMD_WIZLIST => {
            page_string(game, d_id, &game.db.wizlist.clone().as_ref(), false);
        }
        SCMD_IMMLIST => {
            page_string(game, d_id, &game.db.immlist.clone().as_ref(), false);
        }
        SCMD_HANDBOOK => {
            page_string(game, d_id, &game.db.handbook.clone().as_ref(), false);
        }
        SCMD_POLICIES => {
            page_string(game, d_id, &game.db.policies.clone().as_ref(), false);
        }
        SCMD_MOTD => {
            page_string(game, d_id, &game.db.motd.clone().as_ref(), false);
        }
        SCMD_IMOTD => {
            page_string(game, d_id, &game.db.imotd.clone().as_ref(), false);
        }
        SCMD_CLEAR => {
            game.send_to_char(chid, "\x21[H\x21[J");
        }
        SCMD_VERSION => {
            game.send_to_char(chid, format!("{}\r\n", CIRCLEMUD_VERSION).as_str());
        }
        SCMD_WHOAMI => {
            game.send_to_char(chid, format!("{}\r\n", ch.get_name()).as_str());
        }
        _ => {
            error!("SYSERR: Unhandled case in do_gen_ps. ({})", subcmd);
            return;
        }
    }
}

fn perform_mortal_where(game: &mut Game, chid: DepotId, arg: &str) {
    let ch = game.db.ch(chid);
    if arg.is_empty() {
        game.send_to_char(chid, "Players in your Zone\r\n--------------------\r\n");
        for d_id in game.descriptor_list.ids() {
            if game.desc(d_id).state() != ConPlaying
                || (game.desc(d_id).character.is_some()
                    && game.desc(d_id).character.unwrap() == chid)
            {
                continue;
            }
            let i;
            if {
                i = if game.desc(d_id).original.is_some() {
                    game.desc(d_id).original
                } else {
                    game.desc(d_id).character
                };
                i.is_none()
            } {
                continue;
            }
            let i_id = i.unwrap();
            let i = game.db.ch(i_id);
            let ch = game.db.ch(chid);
            if i.in_room() == NOWHERE || !game.can_see(ch, i) {
                continue;
            }
            if game.db.world[ch.in_room() as usize].zone != game.db.world[i.in_room() as usize].zone
            {
                continue;
            }
            let messg = format!(
                "%{:20} - {}\r\n",
                i.get_name(),
                game.db.world[i.in_room() as usize].name
            );
            game.send_to_char(chid, messg.as_str());
        }
    } else {
        /* print only FIRST char, not all. */
        for i in game.db.character_list.iter() {
            if i.in_room() == NOWHERE || i.id() == chid {
                continue;
            }
            if !game.can_see(ch, i)
                || game.db.world[i.in_room() as usize].zone
                    != game.db.world[ch.in_room() as usize].zone
            {
                continue;
            }
            if !isname(arg, &i.player.name) {
                continue;
            }
            game.send_to_char(
                chid,
                format!(
                    "{:25} - {}\r\n",
                    i.get_name(),
                    game.db.world[i.in_room() as usize].name
                )
                .as_str(),
            );
            return;
        }
        game.send_to_char(chid, "Nobody around by that name.\r\n");
    }
}

fn print_object_location(game: &mut Game, num: i32, oid: DepotId, chid: DepotId, recur: bool) {
    if num > 0 {
        game.send_to_char(
            chid,
            format!("O{:3}. {:25} - ", num, game.db.obj(oid).short_description).as_ref(),
        );
    } else {
        game.send_to_char(chid, format!("{:33}", " - ").as_str());
    }

    if game.db.obj(oid).in_room != NOWHERE {
        game.send_to_char(
            chid,
            format!(
                "[{:5}] {}\r\n",
                game.db.get_room_vnum(game.db.obj(oid).in_room()),
                game.db.world[game.db.obj(oid).in_room() as usize].name
            )
            .as_str(),
        );
    } else if game.db.obj(oid).carried_by.is_some() {
        let ch = game.db.ch(chid);
        game.send_to_char(
            chid,
            format!(
                "carried by {}\r\n",
                game.pers(game.db.ch(game.db.obj(oid).carried_by.unwrap()), ch)
            )
            .as_str(),
        );
    } else if game.db.obj(oid).worn_by.is_some() {
        let ch = game.db.ch(chid);
        game.send_to_char(
            chid,
            format!(
                "worn by {}\r\n",
                game.pers(game.db.ch(game.db.obj(oid).worn_by.unwrap()), ch)
            )
            .as_str(),
        );
    } else if game.db.obj(oid).in_obj.is_some() {
        game.send_to_char(
            chid,
            format!(
                "inside {}{}\r\n",
                game.db
                    .obj(game.db.obj(oid).in_obj.unwrap())
                    .short_description,
                if recur { ", which is" } else { " " }
            )
            .as_str(),
        );
        if recur {
            print_object_location(game, 0, game.db.obj(oid).in_obj.unwrap(), chid, recur);
        }
    } else {
        game.send_to_char(chid, "in an unknown location\r\n");
    }
}

fn perform_immort_where(game: &mut Game, chid: DepotId, arg: &str) {
    if arg.is_empty() {
        game.send_to_char(chid, "Players\r\n-------\r\n");
        for d_id in game.descriptor_list.ids() {
            if game.desc(d_id).state() == ConPlaying {
                let oi = if game.desc(d_id).original.is_some() {
                    game.desc(d_id).original.as_ref()
                } else {
                    game.desc(d_id).character.as_ref()
                };
                if oi.is_none() {
                    continue;
                }

                let i_id = *oi.unwrap();
                let i = game.db.ch(i_id);
                let ch = game.db.ch(chid);
                if game.can_see(ch, i) && (i.in_room() != NOWHERE) {
                    if game.desc(d_id).original.is_some() {
                        let messg = format!(
                            "{:20} - [{:5}] {} (in {})\r\n",
                            i.get_name(),
                            game.db.get_room_vnum(
                                game.db.ch(game.desc(d_id).character.unwrap()).in_room
                            ),
                            game.db.world[game.db.ch(game.desc(d_id).character.unwrap()).in_room
                                as usize]
                                .name,
                            game.db.ch(game.desc(d_id).character.unwrap()).get_name()
                        );
                        game.send_to_char(chid, messg.as_str());
                    } else {
                        let messg = format!(
                            "{:20} - [{:5}] {}\r\n",
                            i.get_name(),
                            game.db.get_room_vnum(i.in_room()),
                            game.db.world[i.in_room() as usize].name
                        );
                        game.send_to_char(chid, messg.as_str());
                    }
                }
            }
        }
    } else {
        let mut found = false;
        let mut num = 0;
        for id in game.db.character_list.ids() {
            let i = game.db.ch(id);
            let ch = game.db.ch(chid);
            if game.can_see(ch, i) && i.in_room() != NOWHERE && isname(arg, &i.player.name)
            {
                found = true;
                let messg = format!(
                    "M{:3}. {:25} - [{:5}] {}\r\n",
                    {
                        num += 1;
                        num
                    },
                    i.get_name(),
                    game.db.get_room_vnum(i.in_room()),
                    game.db.world[i.in_room() as usize].name
                );
                game.send_to_char(chid, messg.as_str());
            }
        }
        num = 0;
        for k in game.db.object_list.ids() {
            let ch = game.db.ch(chid);
            if game.can_see_obj(ch, game.db.obj(k)) && isname(arg, game.db.obj(k).name.as_ref()) {
                found = true;
                print_object_location(
                    game,
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
            game.send_to_char(chid, "Couldn't find any such thing.\r\n");
        }
    }
}

pub fn do_where(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if ch.get_level() >= LVL_IMMORT as u8 {
        perform_immort_where(game, chid, &arg);
    } else {
        perform_mortal_where(game, chid, &arg);
    }
}

pub fn do_levels(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    if ch.is_npc() {
        game.send_to_char(chid, "You ain't nothin' but a hound-dog.\r\n");
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
    let d_id = ch.desc.unwrap();
    page_string(game, d_id, buf.as_str(), true);
}

pub fn do_consider(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut buf = String::new();
    one_argument(argument, &mut buf);

    let victim_id = game.get_char_vis(chid, &mut buf, None, FIND_CHAR_ROOM);
    if victim_id.is_none() {
        game.send_to_char(chid, "Consider killing who?\r\n");
        return;
    }
    let victim_id = victim_id.unwrap();
    if victim_id == chid {
        game.send_to_char(chid, "Easy!  Very easy indeed!\r\n");
        return;
    }
    let victim = game.db.ch(victim_id);
    if !victim.is_npc() {
        game.send_to_char(chid, "Would you like to borrow a cross and a shovel?\r\n");
        return;
    }
    let victim = game.db.ch(victim_id);
    let diff = victim.get_level() as i32 - ch.get_level() as i32;

    if diff <= -10 {
        game.send_to_char(chid, "Now where did that chicken go?\r\n");
    } else if diff <= -5 {
        game.send_to_char(chid, "You could do it with a needle!\r\n");
    } else if diff <= -2 {
        game.send_to_char(chid, "Easy.\r\n");
    } else if diff <= -1 {
        game.send_to_char(chid, "Fairly easy.\r\n");
    } else if diff == 0 {
        game.send_to_char(chid, "The perfect match!\r\n");
    } else if diff <= 1 {
        game.send_to_char(chid, "You would need some luck!\r\n");
    } else if diff <= 2 {
        game.send_to_char(chid, "You would need a lot of luck!\r\n");
    } else if diff <= 3 {
        game.send_to_char(
            chid,
            "You would need a lot of luck and great equipment!\r\n",
        );
    } else if diff <= 5 {
        game.send_to_char(chid, "Do you feel lucky, punk?\r\n");
    } else if diff <= 10 {
        game.send_to_char(chid, "Are you mad!?\r\n");
    } else if diff <= 100 {
        game.send_to_char(chid, "You ARE mad!\r\n");
    }
}

pub fn do_diagnose(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut buf = String::new();

    one_argument(argument, &mut buf);
    let vict_id;
    if !buf.is_empty() {
        if {
            vict_id = game.get_char_vis(chid, &mut buf, None, FIND_CHAR_ROOM);
            vict_id.is_none()
        } {
            game.send_to_char(chid, NOPERSON);
        } else {
            diag_char_to_char(game, vict_id.unwrap(), chid);
        }
    } else {
        if ch.fighting_id().is_some() {
            let fighting_id = ch.fighting_id().unwrap();
            diag_char_to_char(game, fighting_id, chid);
        } else {
            game.send_to_char(chid, "Diagnose who?\r\n");
        }
    }
}

const CTYPES: [&str; 5] = ["off", "sparse", "normal", "complete", "\n"];

pub fn do_color(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut arg = String::new();
    if ch.is_npc() {
        return;
    }

    one_argument(argument, &mut arg);

    if arg.is_empty() {
        game.send_to_char(
            chid,
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
        game.send_to_char(
            chid,
            "Usage: color { Off | Sparse | Normal | Complete }\r\n",
        );
        return;
    }
    let tp = tp.unwrap() as i64;
    let ch = game.db.ch_mut(chid);
    ch.remove_prf_flags_bits(PRF_COLOR_1 | PRF_COLOR_2);
    ch.set_prf_flags_bits(PRF_COLOR_1 * (tp & 1) | (PRF_COLOR_2 * (tp & 2) >> 1));
    info!(
        "[DEBUG] {} {}",
        PRF_COLOR_1 * (tp & 1),
        (PRF_COLOR_2 * (tp & 2) >> 1)
    );
    let ch = game.db.ch(chid);
    game.send_to_char(
        chid,
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

pub fn do_toggle(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
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
        game.send_to_char(
            chid,
            format!(
                "      No Hassle: {:3}    Holylight: {:3}    Room Flags:{:3}\r\n",
                onoff!(ch.prf_flagged(PRF_NOHASSLE)),
                onoff!(ch.prf_flagged(PRF_HOLYLIGHT)),
                onoff!(ch.prf_flagged(PRF_ROOMFLAGS))
            )
            .as_str(),
        );
    }

    let ch = game.db.ch(chid);
    game.send_to_char(
        chid,
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

pub fn do_commands(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);
    let vict_id;
    let victo;
    if !arg.is_empty() {
        victo = game.get_char_vis(chid, &mut arg, None, FIND_CHAR_WORLD);
        if victo.is_none() || game.db.ch(victo.unwrap()).is_npc() {
            game.send_to_char(chid, "Who is that?\r\n");
            return;
        }
        vict_id = victo.unwrap();
        let vict = game.db.ch(vict_id);
        if ch.get_level() < vict.get_level() {
            game.send_to_char(
                chid,
                "You can't see the commands of people above your level.\r\n",
            );
            return;
        }
    } else {
        vict_id = chid;
    }

    let mut socials = false;
    let mut wizhelp = false;
    if subcmd == SCMD_SOCIALS {
        socials = true;
    } else if subcmd == SCMD_WIZHELP {
        wizhelp = true;
    }

    let vict = game.db.ch(vict_id);
    let vic_name = vict.get_name();
    game.send_to_char(
        chid,
        format!(
            "The following {}{} are available to {}:\r\n",
            if wizhelp { "privileged " } else { "" },
            if socials { "socials" } else { "commands" },
            if vict_id == chid {
                "you"
            } else {
                vic_name.as_ref()
            }
        )
        .as_str(),
    );

    /* cmd_num starts at 1, not 0, to remove 'RESERVED' */
    let mut no = 1;
    let vict_level = game.db.ch(vict_id).get_level();
    for cmd_num in 1..CMD_INFO.len() {
        let i: usize = game.db.cmd_sort_info[cmd_num];
        if CMD_INFO[i].minimum_level < 0 || vict_level < CMD_INFO[i].minimum_level as u8 {
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
        game.send_to_char(
            chid,
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
        game.send_to_char(chid, "\r\n");
    }
}
