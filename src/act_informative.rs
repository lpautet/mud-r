/* ************************************************************************
*   File: act.informative.c                             Part of CircleMUD *
*  Usage: Player-level commands of an informative nature                  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::rc::Rc;

use log::{error, info};

use crate::class::{level_exp, title_female, title_male};
use crate::config::NOPERSON;
use crate::constants::{COLOR_LIQUID, DIRS, FULLNESS, MONTH_NAME, ROOM_BITS, WEAR_WHERE, WEEKDAYS};
use crate::db::DB;
use crate::fight::compute_armor_class;
use crate::handler::{
    affected_by_spell, fname, get_number, isname, FIND_CHAR_ROOM, FIND_CHAR_WORLD, FIND_OBJ_EQUIP,
    FIND_OBJ_INV, FIND_OBJ_ROOM,
};
use crate::interpreter::{
    half_chop, is_abbrev, one_argument, search_block, CMD_INFO, SCMD_READ, SCMD_SOCIALS,
    SCMD_WIZHELP,
};
use crate::modify::page_string;
use crate::screen::{C_NRM, C_OFF, C_SPR, KCYN, KGRN, KNRM, KNUL, KRED, KYEL};
use crate::spells::SPELL_ARMOR;
use crate::structs::{
    CharData, ExtraDescrData, ObjData, AFF_DETECT_ALIGN, AFF_DETECT_MAGIC, AFF_HIDE, AFF_INVISIBLE,
    AFF_SANCTUARY, CONT_CLOSED, EX_CLOSED, EX_ISDOOR, ITEM_BLESS, ITEM_CONTAINER, ITEM_DRINKCON,
    ITEM_FOUNTAIN, ITEM_GLOW, ITEM_HUM, ITEM_INVISIBLE, ITEM_MAGIC, ITEM_NOTE, LVL_GOD, NOWHERE,
    NUM_OF_DIRS, PLR_WRITING, POS_FIGHTING, PRF_COLOR_1, PRF_COLOR_2, SEX_FEMALE, SEX_MALE,
    SEX_NEUTRAL,
};
use crate::structs::{AFF_BLIND, PRF_AUTOEXIT, PRF_BRIEF, PRF_ROOMFLAGS, ROOM_DEATH};
use crate::structs::{
    AFF_CHARM, AFF_DETECT_INVIS, AFF_INFRAVISION, AFF_POISON, DRUNK, FULL, LVL_IMMORT, NUM_WEARS,
    POS_DEAD, POS_INCAP, POS_MORTALLYW, POS_RESTING, POS_SITTING, POS_SLEEPING, POS_STANDING,
    POS_STUNNED, PRF_SUMMONABLE, THIRST,
};
use crate::util::{age, rand_number, real_time_passed, sprintbit, sprinttype, time_now};
use crate::{
    _clrlevel, an, clr, send_to_char, MainGlobals, CCCYN, CCGRN, CCRED, CCYEL, COLOR_LEV,
    TO_NOTVICT,
};
use crate::{CCNRM, TO_VICT};

pub const SHOW_OBJ_LONG: i32 = 0;
pub const SHOW_OBJ_SHORT: i32 = 1;
pub const SHOW_OBJ_ACTION: i32 = 2;

pub fn show_obj_to_char(obj: &ObjData, ch: &CharData, mode: i32) {
    // if (!obj || !ch) {
    // log("SYSERR: NULL pointer in show_obj_to_char(): obj=%p ch=%p", obj, ch);
    // return;
    // }

    match mode {
        SHOW_OBJ_LONG => {
            send_to_char(ch, format!("{}", obj.description).as_str());
        }

        SHOW_OBJ_SHORT => {
            send_to_char(ch, format!("{}", obj.short_description).as_str());
        }

        SHOW_OBJ_ACTION => match obj.get_obj_type() {
            ITEM_NOTE => {
                if !obj.action_description.is_empty() {
                    let notebuf = format!(
                        "There is something written on it:\r\n\r\n{}",
                        obj.action_description
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

pub fn show_obj_modifiers(obj: &ObjData, ch: &CharData) {
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

impl DB {
    pub fn list_obj_to_char(&self, list: &Vec<Rc<ObjData>>, ch: &CharData, mode: i32, show: bool) {
        let mut found = true;

        for obj in list {
            if self.can_see_obj(ch, obj) {
                show_obj_to_char(obj, ch, mode);
                found = true;
            }
        }
        if !found && show {
            send_to_char(ch, " Nothing.\r\n");
        }
    }
}

impl DB {
    pub fn diag_char_to_char(&self, i: &Rc<CharData>, ch: &Rc<CharData>) {
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

        let pers = self.pers(i, ch);

        let percent;
        if i.get_max_hit() > 0 {
            percent = (100 * i.get_hit() as i32) / i.get_max_hit() as i32;
        } else {
            percent = -1; /* How could MAX_HIT be < 1?? */
        }
        let mut ar_index: usize = 0;
        loop {
            if DIAGNOSIS[ar_index].percent < 0
                || percent >= DIAGNOSIS[ar_index as usize].percent as i32
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

    pub fn look_at_char(&self, i: &Rc<CharData>, ch: &Rc<CharData>) {
        let mut found;
        //struct obj_data *tmp_obj;

        if ch.desc.borrow().is_none() {
            return;
        }

        if !i.player.borrow().description.is_empty() {
            send_to_char(ch, i.player.borrow().description.as_str());
        } else {
            self.act(
                "You see nothing special about $m.",
                false,
                Some(i),
                None,
                Some(ch),
                TO_VICT,
            );
        }

        self.diag_char_to_char(i, ch);

        found = false;
        for j in 0..NUM_WEARS {
            if i.get_eq(j).is_some() && self.can_see_obj(ch, i.get_eq(j).as_ref().unwrap()) {
                found = true;
            }
        }

        if found {
            send_to_char(ch, "\r\n"); /* act() does capitalization. */
            self.act("$n is using:", false, Some(i), None, Some(ch), TO_VICT);
            for j in 0..NUM_WEARS {
                if i.get_eq(j).is_some() && self.can_see_obj(ch, i.get_eq(j).as_ref().unwrap()) {
                    send_to_char(ch, WEAR_WHERE[j as usize]);
                    show_obj_to_char(i.get_eq(j).as_ref().unwrap(), ch, SHOW_OBJ_SHORT);
                }
            }
        }
        if !Rc::ptr_eq(i, ch) && (ch.is_thief() || ch.get_level() >= LVL_IMMORT as u8) {
            found = false;
            self.act(
                "\r\nYou attempt to peek at $s inventory:",
                false,
                Some(i),
                None,
                Some(ch),
                TO_VICT,
            );
            for tmp_obj in i.carrying.borrow().iter() {
                if self.can_see_obj(ch, tmp_obj) && rand_number(0, 20) < ch.get_level() as u32 {
                    show_obj_to_char(tmp_obj, ch, SHOW_OBJ_SHORT);
                    found = true;
                }
            }
        }

        if !found {
            send_to_char(ch, "You can't see anything.\r\n");
        }
    }

    pub fn list_one_char(&self, i: &Rc<CharData>, ch: &Rc<CharData>) {
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

        if i.is_npc()
            && !i.player.borrow().long_descr.is_empty()
            && i.get_pos() == i.get_default_pos()
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
                self.act(
                    "...$e glows with a bright light!",
                    false,
                    Some(i),
                    None,
                    Some(ch),
                    TO_VICT,
                );
            }
            if i.aff_flagged(AFF_BLIND) {
                self.act(
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
                            format!(
                                "{}!",
                                self.pers(i.fighting().as_ref().unwrap(), ch.as_ref())
                            )
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
            self.act(
                "...$e glows with a bright light!",
                false,
                Some(i),
                None,
                Some(ch),
                TO_VICT,
            );
        }
    }

    pub fn list_char_to_char(&self, list: &Vec<Rc<CharData>>, ch: &Rc<CharData>) {
        for i in list {
            if !Rc::ptr_eq(i, &ch) {
                if self.can_see(ch.as_ref(), i) {
                    self.list_one_char(i, &ch);
                } else if self.is_dark(ch.in_room())
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

    pub fn do_auto_exits(&self, ch: &CharData) {
        //int door, slen = 0;
        let mut slen = 0;
        send_to_char(ch, format!("{}[ Exits: ", CCCYN!(ch, C_NRM)).as_str());
        for door in 0..NUM_OF_DIRS {
            if self.exit(ch, door).is_none()
                || self.exit(ch, door).as_ref().unwrap().to_room.get() == NOWHERE
            {
                continue;
            }
            if self
                .exit(ch, door)
                .as_ref()
                .unwrap()
                .exit_flagged(EX_CLOSED)
            {
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
}

#[allow(unused_variables)]
pub fn do_exits(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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

impl DB {
    pub fn look_at_room(&self, ch: &Rc<CharData>, ignore_brief: bool) {
        if ch.desc.borrow().is_none() {
            return;
        }

        if self.is_dark(ch.in_room()) && !ch.can_see_in_dark() {
            send_to_char(ch, "It is pitch black...\r\n");
            return;
        } else if ch.aff_flagged(AFF_BLIND) {
            send_to_char(ch, "You see nothing but infinite darkness...\r\n");
            return;
        }
        send_to_char(ch, format!("{}", CCCYN!(ch, C_NRM)).as_str());

        if !ch.is_npc() && ch.prf_flagged(PRF_ROOMFLAGS) {
            let mut buf = String::new();
            sprintbit(self.room_flags(ch.in_room()) as i64, &ROOM_BITS, &mut buf);
            send_to_char(
                ch,
                format!(
                    "[{}] {} [{}]",
                    self.get_room_vnum(ch.in_room()),
                    self.world.borrow()[ch.in_room() as usize].name,
                    buf
                )
                .as_str(),
            );
        } else {
            send_to_char(
                ch,
                format!("{}", self.world.borrow()[ch.in_room() as usize].name).as_str(),
            );
        }

        send_to_char(ch, format!("{}\r\n", CCNRM!(ch, C_NRM)).as_str());

        if (!ch.is_npc() && !ch.prf_flagged(PRF_BRIEF))
            || ignore_brief
            || self.room_flagged(ch.in_room(), ROOM_DEATH)
        {
            send_to_char(
                ch,
                format!("{}", self.world.borrow()[ch.in_room() as usize].description).as_str(),
            );
        }

        /* autoexits */
        if !ch.is_npc() && ch.prf_flagged(PRF_AUTOEXIT) {
            self.do_auto_exits(ch);
        }

        /* now list characters & objects */
        send_to_char(ch, format!("{}", CCGRN!(ch, C_NRM)).as_str());
        self.list_obj_to_char(
            self.world.borrow()[ch.in_room() as usize]
                .contents
                .borrow()
                .as_ref(),
            ch,
            SHOW_OBJ_LONG,
            false,
        );
        send_to_char(ch, format!("{}", CCYEL!(ch, C_NRM)).as_str());
        self.list_char_to_char(
            self.world.borrow()[ch.in_room() as usize]
                .peoples
                .borrow()
                .as_ref(),
            ch,
        );
        send_to_char(ch, format!("{}", CCNRM!(ch, C_NRM)).as_str());
    }
}

impl DB {
    pub fn look_in_direction(&self, ch: &Rc<CharData>, dir: i32) {
        if self.exit(ch, dir as usize).is_some() {
            if !self
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
                        self.exit(ch, dir as usize)
                            .as_ref()
                            .unwrap()
                            .general_description
                    )
                    .as_str(),
                );
            } else {
                send_to_char(ch, "You see nothing special.\r\n");
            }
            if self
                .exit(ch, dir as usize)
                .as_ref()
                .unwrap()
                .exit_flagged(EX_CLOSED)
                && !self
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
                        fname(
                            self.exit(ch, dir as usize)
                                .as_ref()
                                .unwrap()
                                .keyword
                                .as_str()
                        )
                    )
                    .as_str(),
                );
            } else if self
                .exit(ch, dir as usize)
                .as_ref()
                .unwrap()
                .exit_flagged(EX_ISDOOR)
                && !self
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
                        fname(
                            self.exit(ch, dir as usize)
                                .as_ref()
                                .unwrap()
                                .keyword
                                .as_str()
                        )
                    )
                    .as_str(),
                )
            } else {
                send_to_char(ch, "Nothing special there...\r\n");
            }
        }
    }

    pub fn look_in_obj(&self, ch: &Rc<CharData>, arg: &str) {
        // struct obj_data *obj = NULL;
        // struct char_data *dummy = NULL;
        // int amt, bits;
        let mut dummy: Option<Rc<CharData>> = None;
        let mut obj: Option<Rc<ObjData>> = None;
        let bits;

        if arg.is_empty() {
            send_to_char(ch, "Look in what?\r\n");
            return;
        }
        bits = self.generic_find(
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

                    self.list_obj_to_char(
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
                        || obj.as_ref().unwrap().get_obj_val(1)
                            > obj.as_ref().unwrap().get_obj_val(0)
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
}

pub fn find_exdesc(word: &str, list: &Vec<ExtraDescrData>) -> Option<String> {
    for i in list {
        if isname(word, i.keyword.as_str()) != 0 {
            return Some(i.description.clone());
        }
    }
    None
}

//
// /*
//  * Given the argument "look at <target>", figure out what object or char
//  * matches the target.  First, see if there is another char in the room
//  * with the name.  Then check local objs for exdescs.
//  *
//  * Thanks to Angus Mezick <angus@EDGIL.CCMAIL.COMPUSERVE.COM> for the
//  * suggested fix to this problem.
//  */
impl DB {
    pub fn look_at_target(&self, ch: &Rc<CharData>, arg: &str) {
        // int bits, found = FALSE, j, fnum, i = 0;
        // struct char_data *found_char = NULL;
        // struct obj_data *obj, *found_obj = NULL;
        // char *desc;
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

        let bits = self.generic_find(
            arg,
            (FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_OBJ_EQUIP | FIND_CHAR_ROOM) as i64,
            ch,
            &mut found_char,
            &mut found_obj,
        );

        /* Is the target a character? */
        if found_char.is_some() {
            let found_char = found_char.as_ref().unwrap();
            self.look_at_char(found_char, ch);
            if !Rc::ptr_eq(ch, found_char) {
                if self.can_see(found_char, ch) {
                    self.act(
                        "$n looks at you.",
                        true,
                        Some(ch),
                        None,
                        Some(found_char),
                        TO_VICT,
                    );
                }
                self.act(
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
            &self.world.borrow()[ch.in_room() as usize].ex_descriptions,
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
            if ch.get_eq(j).is_some() && self.can_see_obj(ch, ch.get_eq(j).as_ref().unwrap()) {
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
            if self.can_see_obj(ch, obj) {
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
        for obj in self.world.borrow()[ch.in_room() as usize]
            .contents
            .borrow()
            .iter()
        {
            if self.can_see_obj(ch, obj) {
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
}

#[allow(unused_variables)]
pub fn do_look(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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
        db.list_char_to_char(
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
                game.db.look_at_target(ch, &mut arg);
            }
            return;
        }
        let look_type;
        if arg.is_empty() {
            /* "look" alone, without an argument at all */
            game.db.look_at_room(ch, true);
        } else if is_abbrev(arg.as_ref(), "in") {
            game.db.look_in_obj(ch, arg2.as_str());
            /* did the char type 'look <direction>?' */
        } else if {
            look_type = search_block(arg.as_str(), &DIRS, false);
            look_type
        } != None
        {
            game.db.look_in_direction(ch, look_type.unwrap() as i32);
        } else if is_abbrev(arg.as_ref(), "at") {
            game.db.look_at_target(ch, arg2.as_ref());
        } else {
            game.db.look_at_target(ch, arg.as_ref());
        }
    }
}

// ACMD(do_examine)
// {
// struct char_data *tmp_char;
// struct obj_data *tmp_object;
// char tempsave[MAX_INPUT_LENGTH], arg[MAX_INPUT_LENGTH];
//
// one_argument(argument, arg);
//
// if (!*arg) {
// send_to_char(ch, "Examine what?\r\n");
// return;
// }
//
// /* look_at_target() eats the number. */
// look_at_target(ch, strcpy(tempsave, arg));	/* strcpy: OK */
//
// generic_find(arg, FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_CHAR_ROOM |
// FIND_OBJ_EQUIP, ch, &tmp_char, &tmp_object);
//
// if (tmp_object) {
// if ((GET_OBJ_TYPE(tmp_object) == ITEM_DRINKCON) ||
// (GET_OBJ_TYPE(tmp_object) == ITEM_FOUNTAIN) ||
// (GET_OBJ_TYPE(tmp_object) == ITEM_CONTAINER)) {
// send_to_char(ch, "When you look inside, you see:\r\n");
// look_in_obj(ch, arg);
// }
// }
// }

#[allow(unused_variables)]
pub fn do_gold(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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

#[allow(unused_variables)]
pub fn do_score(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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

#[allow(unused_variables)]
pub fn do_inventory(
    game: &MainGlobals,
    ch: &Rc<CharData>,
    argument: &str,
    cmd: usize,
    subcmd: i32,
) {
    send_to_char(ch, "You are carrying:\r\n");
    game.db
        .list_obj_to_char(ch.carrying.borrow().as_ref(), ch, SHOW_OBJ_SHORT, true);
}

#[allow(unused_variables)]
pub fn do_equipment(
    game: &MainGlobals,
    ch: &Rc<CharData>,
    argument: &str,
    cmd: usize,
    subcmd: i32,
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

#[allow(unused_variables)]
pub fn do_time(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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

#[allow(unused_variables)]
pub fn do_weather(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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

#[allow(unused_variables)]
pub fn do_help(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    // int chk, bot, top, mid, minlen;

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
    let minlen = argument.len();

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

// #define WHO_FORMAT \
// "format: who [minlev[-maxlev]] [-n name] [-c classlist] [-s] [-o] [-q] [-r] [-z]\r\n"
//
// /* FIXME: This whole thing just needs rewritten. */
// ACMD(do_who)
// {
// struct descriptor_data *d;
// struct char_data *tch;
// char name_search[MAX_INPUT_LENGTH], buf[MAX_INPUT_LENGTH];
// char mode;
// int low = 0, high = LVL_IMPL, localwho = 0, questwho = 0;
// int showclass = 0, short_list = 0, outlaws = 0, num_can_see = 0;
// int who_room = 0;
//
// skip_spaces(&argument);
// strcpy(buf, argument);	/* strcpy: OK (sizeof: argument == buf) */
// name_search[0] = '\0';
//
// while (*buf) {
// char arg[MAX_INPUT_LENGTH], buf1[MAX_INPUT_LENGTH];
//
// half_chop(buf, arg, buf1);
// if (isdigit(*arg)) {
// sscanf(arg, "%d-%d", &low, &high);
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// } else if (*arg == '-') {
// mode = *(arg + 1);       /* just in case; we destroy arg in the switch */
// switch (mode) {
// case 'o':
// case 'k':
// outlaws = 1;
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// break;
// case 'z':
// localwho = 1;
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// break;
// case 's':
// short_list = 1;
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// break;
// case 'q':
// questwho = 1;
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// break;
// case 'l':
// half_chop(buf1, arg, buf);
// sscanf(arg, "%d-%d", &low, &high);
// break;
// case 'n':
// half_chop(buf1, name_search, buf);
// break;
// case 'r':
// who_room = 1;
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// break;
// case 'c':
// half_chop(buf1, arg, buf);
// showclass = find_class_bitvector(arg);
// break;
// default:
// send_to_char(ch, "%s", WHO_FORMAT);
// return;
// }				/* end of switch */
//
// } else {			/* endif */
// send_to_char(ch, "%s", WHO_FORMAT);
// return;
// }
// }				/* end while (parser) */
//
// send_to_char(ch, "Players\r\n-------\r\n");
//
// for (d = descriptor_list; d; d = d.next) {
// if (STATE(d) != CON_PLAYING)
// continue;
//
// if (d.original)
// tch = d.original;
// else if (!(tch = d.character))
// continue;
//
// if (*name_search && str_cmp(GET_NAME(tch), name_search) &&
// !strstr(GET_TITLE(tch), name_search))
// continue;
// if (!CAN_SEE(ch, tch) || GET_LEVEL(tch) < low || GET_LEVEL(tch) > high)
// continue;
// if (outlaws && !PLR_FLAGGED(tch, PLR_KILLER) &&
// !PLR_FLAGGED(tch, PLR_THIEF))
// continue;
// if (questwho && !PRF_FLAGGED(tch, PRF_QUEST))
// continue;
// if (localwho && world[IN_ROOM(ch)].zone != world[IN_ROOM(tch)].zone)
// continue;
// if (who_room && (IN_ROOM(tch) != IN_ROOM(ch)))
// continue;
// if (showclass && !(showclass & (1 << GET_CLASS(tch))))
// continue;
// if (short_list) {
// send_to_char(ch, "%s[%2d %s] %-12.12s%s%s",
// (GET_LEVEL(tch) >= LVL_IMMORT ? CCYEL(ch, C_SPR) : ""),
// GET_LEVEL(tch), CLASS_ABBR(tch), GET_NAME(tch),
// (GET_LEVEL(tch) >= LVL_IMMORT ? CCNRM(ch, C_SPR) : ""),
// ((!(++num_can_see % 4)) ? "\r\n" : ""));
// } else {
// num_can_see++;
// send_to_char(ch, "%s[%2d %s] %s %s",
// (GET_LEVEL(tch) >= LVL_IMMORT ? CCYEL(ch, C_SPR) : ""),
// GET_LEVEL(tch), CLASS_ABBR(tch), GET_NAME(tch),
// GET_TITLE(tch));
//
// if (GET_INVIS_LEV(tch))
// send_to_char(ch, " (i%d)", GET_INVIS_LEV(tch));
// else if (AFF_FLAGGED(tch, AFF_INVISIBLE))
// send_to_char(ch, " (invis)");
//
// if (PLR_FLAGGED(tch, PLR_MAILING))
// send_to_char(ch, " (mailing)");
// else if (PLR_FLAGGED(tch, PLR_WRITING))
// send_to_char(ch, " (writing)");
//
// if (PRF_FLAGGED(tch, PRF_DEAF))
// send_to_char(ch, " (deaf)");
// if (PRF_FLAGGED(tch, PRF_NOTELL))
// send_to_char(ch, " (notell)");
// if (PRF_FLAGGED(tch, PRF_QUEST))
// send_to_char(ch, " (quest)");
// if (PLR_FLAGGED(tch, PLR_THIEF))
// send_to_char(ch, " (THIEF)");
// if (PLR_FLAGGED(tch, PLR_KILLER))
// send_to_char(ch, " (KILLER)");
// if (GET_LEVEL(tch) >= LVL_IMMORT)
// send_to_char(ch, CCNRM(ch, C_SPR));
// send_to_char(ch, "\r\n");
// }				/* endif shortlist */
// }				/* end of for */
// if (short_list && (num_can_see % 4))
// send_to_char(ch, "\r\n");
// if (num_can_see == 0)
// send_to_char(ch, "\r\nNobody at all!\r\n");
// else if (num_can_see == 1)
// send_to_char(ch, "\r\nOne lonely character displayed.\r\n");
// else
// send_to_char(ch, "\r\n%d characters displayed.\r\n", num_can_see);
// }
//
//
// #define USERS_FORMAT \
// "format: users [-l minlevel[-maxlevel]] [-n name] [-h host] [-c classlist] [-o] [-p]\r\n"
//
// /* BIG OL' FIXME: Rewrite it all. Similar to do_who(). */
// ACMD(do_users)
// {
// char line[200], line2[220], idletime[10], classname[20];
// char state[30], *timeptr, mode;
// char name_search[MAX_INPUT_LENGTH], host_search[MAX_INPUT_LENGTH];
// struct char_data *tch;
// struct descriptor_data *d;
// int low = 0, high = LVL_IMPL, num_can_see = 0;
// int showclass = 0, outlaws = 0, playing = 0, deadweight = 0;
// char buf[MAX_INPUT_LENGTH], arg[MAX_INPUT_LENGTH];
//
// host_search[0] = name_search[0] = '\0';
//
// strcpy(buf, argument);	/* strcpy: OK (sizeof: argument == buf) */
// while (*buf) {
// char buf1[MAX_INPUT_LENGTH];
//
// half_chop(buf, arg, buf1);
// if (*arg == '-') {
// mode = *(arg + 1);  /* just in case; we destroy arg in the switch */
// switch (mode) {
// case 'o':
// case 'k':
// outlaws = 1;
// playing = 1;
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// break;
// case 'p':
// playing = 1;
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// break;
// case 'd':
// deadweight = 1;
// strcpy(buf, buf1);	/* strcpy: OK (sizeof: buf1 == buf) */
// break;
// case 'l':
// playing = 1;
// half_chop(buf1, arg, buf);
// sscanf(arg, "%d-%d", &low, &high);
// break;
// case 'n':
// playing = 1;
// half_chop(buf1, name_search, buf);
// break;
// case 'h':
// playing = 1;
// half_chop(buf1, host_search, buf);
// break;
// case 'c':
// playing = 1;
// half_chop(buf1, arg, buf);
// showclass = find_class_bitvector(arg);
// break;
// default:
// send_to_char(ch, "%s", USERS_FORMAT);
// return;
// }				/* end of switch */
//
// } else {			/* endif */
// send_to_char(ch, "%s", USERS_FORMAT);
// return;
// }
// }				/* end while (parser) */
// send_to_char(ch,
// "Num Class   Name         State          Idl Login@   Site\r\n"
// "--- ------- ------------ -------------- --- -------- ------------------------\r\n");
//
// one_argument(argument, arg);
//
// for (d = descriptor_list; d; d = d.next) {
// if (STATE(d) != CON_PLAYING && playing)
// continue;
// if (STATE(d) == CON_PLAYING && deadweight)
// continue;
// if (STATE(d) == CON_PLAYING) {
// if (d.original)
// tch = d.original;
// else if (!(tch = d.character))
// continue;
//
// if (*host_search && !strstr(d.host, host_search))
// continue;
// if (*name_search && str_cmp(GET_NAME(tch), name_search))
// continue;
// if (!CAN_SEE(ch, tch) || GET_LEVEL(tch) < low || GET_LEVEL(tch) > high)
// continue;
// if (outlaws && !PLR_FLAGGED(tch, PLR_KILLER) &&
// !PLR_FLAGGED(tch, PLR_THIEF))
// continue;
// if (showclass && !(showclass & (1 << GET_CLASS(tch))))
// continue;
// if (GET_INVIS_LEV(ch) > GET_LEVEL(ch))
// continue;
//
// if (d.original)
// sprintf(classname, "[%2d %s]", GET_LEVEL(d.original),
// CLASS_ABBR(d.original));
// else
// sprintf(classname, "[%2d %s]", GET_LEVEL(d.character),
// CLASS_ABBR(d.character));
// } else
// strcpy(classname, "   -   ");
//
// timeptr = asctime(localtime(&d.login_time));
// timeptr += 11;
// *(timeptr + 8) = '\0';
//
// if (STATE(d) == CON_PLAYING && d.original)
// strcpy(state, "Switched");
// else
// strcpy(state, connected_types[STATE(d)]);
//
// if (d.character && STATE(d) == CON_PLAYING && GET_LEVEL(d.character) < LVL_GOD)
// sprintf(idletime, "%3d", d.character.char_specials.timer *
// SECS_PER_MUD_HOUR / SECS_PER_REAL_MIN);
// else
// strcpy(idletime, "");
//
// sprintf(line, "%3d %-7s %-12s %-14s %-3s %-8s ", d.desc_num, classname,
// d.original && d.original.player.name ? d.original.player.name :
// d.character && d.character.player.name ? d.character.player.name :
// "UNDEFINED",
// state, idletime, timeptr);
//
// if (d.host && *d.host)
// sprintf(line + strlen(line), "[%s]\r\n", d.host);
// else
// strcat(line, "[Hostname unknown]\r\n");
//
// if (STATE(d) != CON_PLAYING) {
// sprintf(line2, "%s%s%s", CCGRN(ch, C_SPR), line, CCNRM(ch, C_SPR));
// strcpy(line, line2);
// }
// if (STATE(d) != CON_PLAYING ||
// (STATE(d) == CON_PLAYING && CAN_SEE(ch, d.character))) {
// send_to_char(ch, "%s", line);
// num_can_see++;
// }
// }
//
// send_to_char(ch, "\r\n%d visible sockets connected.\r\n", num_can_see);
// }
//
//
// /* Generic page_string function for displaying text */
// ACMD(do_gen_ps)
// {
// switch (subcmd) {
// case SCMD_CREDITS:
// page_string(ch.desc, credits, 0);
// break;
// case SCMD_NEWS:
// page_string(ch.desc, news, 0);
// break;
// case SCMD_INFO:
// page_string(ch.desc, info, 0);
// break;
// case SCMD_WIZLIST:
// page_string(ch.desc, wizlist, 0);
// break;
// case SCMD_IMMLIST:
// page_string(ch.desc, immlist, 0);
// break;
// case SCMD_HANDBOOK:
// page_string(ch.desc, handbook, 0);
// break;
// case SCMD_POLICIES:
// page_string(ch.desc, policies, 0);
// break;
// case SCMD_MOTD:
// page_string(ch.desc, motd, 0);
// break;
// case SCMD_IMOTD:
// page_string(ch.desc, imotd, 0);
// break;
// case SCMD_CLEAR:
// send_to_char(ch, "\033[H\033[J");
// break;
// case SCMD_VERSION:
// send_to_char(ch, "%s\r\n", circlemud_version);
// break;
// case SCMD_WHOAMI:
// send_to_char(ch, "%s\r\n", GET_NAME(ch));
// break;
// default:
// log("SYSERR: Unhandled case in do_gen_ps. (%d)", subcmd);
// return;
// }
// }
//
//
// void perform_mortal_where(struct char_data *ch, char *arg)
// {
// struct char_data *i;
// struct descriptor_data *d;
//
// if (!*arg) {
// send_to_char(ch, "Players in your Zone\r\n--------------------\r\n");
// for (d = descriptor_list; d; d = d.next) {
// if (STATE(d) != CON_PLAYING || d.character == ch)
// continue;
// if ((i = (d.original ? d.original : d.character)) == NULL)
// continue;
// if (IN_ROOM(i) == NOWHERE || !CAN_SEE(ch, i))
// continue;
// if (world[IN_ROOM(ch)].zone != world[IN_ROOM(i)].zone)
// continue;
// send_to_char(ch, "%-20s - %s\r\n", GET_NAME(i), world[IN_ROOM(i)].name);
// }
// } else {			/* print only FIRST char, not all. */
// for (i = character_list; i; i = i.next) {
// if (IN_ROOM(i) == NOWHERE || i == ch)
// continue;
// if (!CAN_SEE(ch, i) || world[IN_ROOM(i)].zone != world[IN_ROOM(ch)].zone)
// continue;
// if (!isname(arg, i.player.name))
// continue;
// send_to_char(ch, "%-25s - %s\r\n", GET_NAME(i), world[IN_ROOM(i)].name);
// return;
// }
// send_to_char(ch, "Nobody around by that name.\r\n");
// }
// }
//
//
// void print_object_location(int num, struct obj_data *obj, struct char_data *ch,
// int recur)
// {
// if (num > 0)
// send_to_char(ch, "O%3d. %-25s - ", num, obj.short_description);
// else
// send_to_char(ch, "%33s", " - ");
//
// if (IN_ROOM(obj) != NOWHERE)
// send_to_char(ch, "[%5d] %s\r\n", GET_ROOM_VNUM(IN_ROOM(obj)), world[IN_ROOM(obj)].name);
// else if (obj.carried_by)
// send_to_char(ch, "carried by %s\r\n", PERS(obj.carried_by, ch));
// else if (obj.worn_by)
// send_to_char(ch, "worn by %s\r\n", PERS(obj.worn_by, ch));
// else if (obj.in_obj) {
// send_to_char(ch, "inside %s%s\r\n", obj.in_obj.short_description, (recur ? ", which is" : " "));
// if (recur)
// print_object_location(0, obj.in_obj, ch, recur);
// } else
// send_to_char(ch, "in an unknown location\r\n");
// }
//
//
//
// void perform_immort_where(struct char_data *ch, char *arg)
// {
// struct char_data *i;
// struct obj_data *k;
// struct descriptor_data *d;
// int num = 0, found = 0;
//
// if (!*arg) {
// send_to_char(ch, "Players\r\n-------\r\n");
// for (d = descriptor_list; d; d = d.next)
// if (STATE(d) == CON_PLAYING) {
// i = (d.original ? d.original : d.character);
// if (i && CAN_SEE(ch, i) && (IN_ROOM(i) != NOWHERE)) {
// if (d.original)
// send_to_char(ch, "%-20s - [%5d] %s (in %s)\r\n",
// GET_NAME(i), GET_ROOM_VNUM(IN_ROOM(d.character)),
// world[IN_ROOM(d.character)].name, GET_NAME(d.character));
// else
// send_to_char(ch, "%-20s - [%5d] %s\r\n", GET_NAME(i), GET_ROOM_VNUM(IN_ROOM(i)), world[IN_ROOM(i)].name);
// }
// }
// } else {
// for (i = character_list; i; i = i.next)
// if (CAN_SEE(ch, i) && IN_ROOM(i) != NOWHERE && isname(arg, i.player.name)) {
// found = 1;
// send_to_char(ch, "M%3d. %-25s - [%5d] %s\r\n", ++num, GET_NAME(i),
// GET_ROOM_VNUM(IN_ROOM(i)), world[IN_ROOM(i)].name);
// }
// for (num = 0, k = object_list; k; k = k.next)
// if (CAN_SEE_OBJ(ch, k) && isname(arg, k.name)) {
// found = 1;
// print_object_location(++num, k, ch, TRUE);
// }
// if (!found)
// send_to_char(ch, "Couldn't find any such thing.\r\n");
// }
// }
//
//
//
// ACMD(do_where)
// {
// char arg[MAX_INPUT_LENGTH];
//
// one_argument(argument, arg);
//
// if (GET_LEVEL(ch) >= LVL_IMMORT)
// perform_immort_where(ch, arg);
// else
// perform_mortal_where(ch, arg);
// }

#[allow(unused_variables)]
pub fn do_levels(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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

#[allow(unused_variables)]
pub fn do_consider(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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

#[allow(unused_variables)]
pub fn do_diagnose(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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
            game.db.diag_char_to_char(vict.as_ref().unwrap(), ch);
        }
    } else {
        if ch.fighting().is_some() {
            game.db
                .diag_char_to_char(ch.fighting().as_ref().unwrap(), ch);
        } else {
            send_to_char(ch, "Diagnose who?\r\n");
        }
    }
}

const CTYPES: [&str; 5] = ["off", "sparse", "normal", "complete", "\n"];

#[allow(unused_variables)]
pub fn do_color(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
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

// ACMD(do_toggle)
// {
// char buf2[4];
//
// if (IS_NPC(ch))
// return;
//
// if (GET_WIMP_LEV(ch) == 0)
// strcpy(buf2, "OFF");	/* strcpy: OK */
// else
// sprintf(buf2, "%-3.3d", GET_WIMP_LEV(ch));	/* sprintf: OK */
//
// if (GET_LEVEL(ch) >= LVL_IMMORT) {
// send_to_char(ch,
// "      No Hassle: %-3s    "
// "      Holylight: %-3s    "
// "     Room Flags: %-3s\r\n",
// ONOFF(PRF_FLAGGED(ch, PRF_NOHASSLE)),
// ONOFF(PRF_FLAGGED(ch, PRF_HOLYLIGHT)),
// ONOFF(PRF_FLAGGED(ch, PRF_ROOMFLAGS))
// );
// }
//
// send_to_char(ch,
// "Hit Pnt Display: %-3s    "
// "     Brief Mode: %-3s    "
// " Summon Protect: %-3s\r\n"
//
// "   Move Display: %-3s    "
// "   Compact Mode: %-3s    "
// "       On Quest: %-3s\r\n"
//
// "   Mana Display: %-3s    "
// "         NoTell: %-3s    "
// "   Repeat Comm.: %-3s\r\n"
//
// " Auto Show Exit: %-3s    "
// "           Deaf: %-3s    "
// "     Wimp Level: %-3s\r\n"
//
// " Gossip Channel: %-3s    "
// "Auction Channel: %-3s    "
// "  Grats Channel: %-3s\r\n"
//
// "    Color Level: %s\r\n",
//
// ONOFF(PRF_FLAGGED(ch, PRF_DISPHP)),
// ONOFF(PRF_FLAGGED(ch, PRF_BRIEF)),
// ONOFF(!PRF_FLAGGED(ch, PRF_SUMMONABLE)),
//
// ONOFF(PRF_FLAGGED(ch, PRF_DISPMOVE)),
// ONOFF(PRF_FLAGGED(ch, PRF_COMPACT)),
// YESNO(PRF_FLAGGED(ch, PRF_QUEST)),
//
// ONOFF(PRF_FLAGGED(ch, PRF_DISPMANA)),
// ONOFF(PRF_FLAGGED(ch, PRF_NOTELL)),
// YESNO(!PRF_FLAGGED(ch, PRF_NOREPEAT)),
//
// ONOFF(PRF_FLAGGED(ch, PRF_AUTOEXIT)),
// YESNO(PRF_FLAGGED(ch, PRF_DEAF)),
// buf2,
//
// ONOFF(!PRF_FLAGGED(ch, PRF_NOGOSS)),
// ONOFF(!PRF_FLAGGED(ch, PRF_NOAUCT)),
// ONOFF(!PRF_FLAGGED(ch, PRF_NOGRATZ)),
//
// ctypes[COLOR_LEV(ch)]);
// }

impl DB {
    pub fn sort_commands(&mut self) {
        self.cmd_sort_info.reserve_exact(CMD_INFO.len());

        for a in 0..CMD_INFO.len() {
            self.cmd_sort_info.push(a);
        }

        self.cmd_sort_info
            .sort_by(|a, b| str::cmp(CMD_INFO[*a].command, CMD_INFO[*b].command));
    }
}

#[allow(unused_variables)]
pub fn do_commands(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    // int no, i, cmd_num;
    // int wizhelp = 0, socials = 0;
    // struct char_data *vict;
    // char arg[MAX_INPUT_LENGTH];
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
        // TODO implement do_action and do_insult
        // if !wizhelp && socials != (CMD_INFO[i].command_pointer == do_action || CMD_INFO[i].command_pointer == do_insult) {
        //     continue;
        // }
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
