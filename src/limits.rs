/* ************************************************************************
*   File: limits.rs                                     Part of CircleMUD *
*  Usage: limits & gain funcs for HMV, exp, hunger/thirst, idle time      *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */
use std::cmp::{max, min};
use std::rc::Rc;

use crate::class::{advance_level, level_exp, title_female, title_male};
use crate::config::{
    FREE_RENT, IDLE_MAX_LEVEL, IDLE_RENT_TIME, IDLE_VOID, IMMORT_LEVEL_OK, MAX_EXP_GAIN,
    MAX_EXP_LOSS,
};
use crate::depot::{Depot, DepotId};
use crate::fight::update_pos;
use crate::handler::{obj_from_obj, obj_to_obj, update_char_objects};
use crate::objsave::{crash_crashsave, crash_idlesave, crash_rentsave};
use crate::spells::{SPELL_POISON, TYPE_SUFFERING};
use crate::structs::ConState::ConDisconnect;
use crate::structs::{
    AffectFlags, CharData, Position, Sex, FULL, LVL_GOD, LVL_IMMORT, LVL_IMPL, NOWHERE, THIRST
};
use crate::structs::{
    DRUNK, PLR_WRITING, 
};
use crate::util::{age, DisplayMode};
use crate::{act, save_char, send_to_char, DescriptorData, Game, ObjData, TextData, DB, TO_CHAR, TO_ROOM};

/* When age < 15 return the value p0 */
/* When age in 15..29 calculate the line between p1 & p2 */
/* When age in 30..44 calculate the line between p2 & p3 */
/* When age in 45..59 calculate the line between p3 & p4 */
/* When age in 60..79 calculate the line between p4 & p5 */
/* When age >= 80 return the value p6 */
fn graf(grafage: i32, p0: i32, p1: i32, p2: i32, p3: i32, p4: i32, p5: i32, p6: i32) -> i32 {
    return if grafage < 15 {
        p0 /* < 15   */
    } else if grafage <= 29 {
        p1 + (((grafage - 15) * (p2 - p1)) / 15) /* 15..29 */
    } else if grafage <= 44 {
        p2 + (((grafage - 30) * (p3 - p2)) / 15) /* 30..44 */
    } else if grafage <= 59 {
        p3 + (((grafage - 45) * (p4 - p3)) / 15) /* 45..59 */
    } else if grafage <= 79 {
        p4 + (((grafage - 60) * (p5 - p4)) / 20) /* 60..79 */
    } else {
        p6 /* >= 80 */
    };
}

/*
 * The hit_limit, mana_limit, and move_limit functions are gone.  They
 * added an unnecessary level of complexity to the internal structure,
 * weren't particularly useful, and led to some annoying bugs.  From the
 * players' point of view, the only difference the removal of these
 * functions will make is that a character's age will now only affect
 * the HMV gain per tick, and _not_ the HMV maximums.
 */

/* manapoint gain pr. game hour */
pub fn mana_gain(ch: &CharData) -> u8 {
    let mut gain;

    if ch.is_npc() {
        /* Neat and fast */
        gain = ch.get_level();
    } else {
        gain = graf(age(ch).year as i32, 4, 8, 12, 16, 12, 10, 8) as u8;

        /* Class calculations */

        /* Skill/Spell calculations */

        /* Position calculations    */
        match ch.get_pos() {
            Position::Sleeping => {
                gain *= 2; /* Divide by 2 */
            }
            Position::Resting => {
                gain += gain / 2; /* Divide by 4 */
            }
            Position::Sitting => {
                gain += gain / 4; /* Divide by 8 */
            }
            _ => {}
        }
        if ch.is_magic_user() || ch.is_cleric() {
            gain *= 2;
        }
        if ch.get_cond(FULL) == 0 || ch.get_cond(THIRST) == 0 {
            gain /= 4;
        }
        if ch.aff_flagged(AffectFlags::POISON) {
            gain /= 4;
        }
    }
    gain
}

/* Hitpoint gain pr. game hour */
pub fn hit_gain(ch: &CharData) -> u8 {
    let mut gain;
    if ch.is_npc() {
        /* Neat and fast */
        gain = ch.get_level();
    } else {
        gain = graf(age(ch).year as i32, 8, 12, 20, 32, 16, 10, 4) as u8;

        /* Class/Level calculations */

        /* Skill/Spell calculations */

        /* Position calculations    */

        match ch.get_pos() {
            Position::Sleeping => {
                gain += gain / 2; /* Divide by 2 */
            }
            Position::Resting => {
                gain += gain / 4; /* Divide by 4 */
            }
            Position::Sitting => {
                gain += gain / 8; /* Divide by 8 */
            }
            _ => {}
        }
        if ch.is_magic_user() || ch.is_cleric() {
            gain /= 2; /* Ouch. */
        }
        if ch.get_cond(FULL) == 0 || ch.get_cond(THIRST) == 0 {
            gain /= 4;
        }

        if ch.aff_flagged(AffectFlags::POISON) {
            gain /= 4;
        }
    }
    gain
}

/* move gain pr. game hour */
pub fn move_gain(ch: &CharData) -> u8 {
    let mut gain;

    if ch.is_npc() {
        /* Neat and fast */
        gain = ch.get_level();
    } else {
        gain = graf(age(ch).year as i32, 16, 20, 24, 20, 16, 12, 10) as u8;

        /* Class/Level calculations */

        /* Skill/Spell calculations */

        /* Position calculations    */
        match ch.get_pos() {
            Position::Sleeping => {
                gain += gain / 2; /* Divide by 2 */
            }
            Position::Resting => {
                gain += gain / 4; /* Divide by 4 */
            }
            Position::Sitting => {
                gain += gain / 8; /* Divide by 8 */
            }
            _ => {}
        }

        if ch.get_cond(FULL) == 0 || ch.get_cond(THIRST) == 0 {
            gain /= 4;
        }

        if ch.aff_flagged(AffectFlags::POISON) {
            gain /= 4;
        }
    }
    gain
}

pub fn set_title(ch: &mut CharData, title: Option<&str>) {
    let mut title = title;
    if title.is_none() || title.unwrap().is_empty() {
        if ch.get_sex() == Sex::Female {
            title = Some(title_female(ch.get_class(), ch.get_level() as i32));
        } else {
            title = Some(title_male(ch.get_class(), ch.get_level() as i32));
        }
    }

    ch.set_title(title.map(|t| Rc::from(t)));
}

// void run_autowiz(void)
// {
// #if defined(CIRCLE_UNIX) || defined(CIRCLE_WINDOWS)
// if (use_autowiz) {
// size_t res;
// char buf[256];
//
// #if defined(CIRCLE_UNIX)
// res = snprintf(buf, sizeof(buf), "nice ../bin/autowiz %d %s %d %s %d &",
// min_wizlist_lev, WIZLIST_FILE, LVL_IMMORT, IMMLIST_FILE, (int) getpid());
// #elif defined(CIRCLE_WINDOWS)
// res = snprintf(buf, sizeof(buf), "autowiz %d %s %d %s",
// min_wizlist_lev, WIZLIST_FILE, LVL_IMMORT, IMMLIST_FILE);
// #endif /* CIRCLE_WINDOWS */
//
// /* Abusing signed -> unsigned conversion to avoid '-1' check. */
// if (res < sizeof(buf)) {
// mudlog(CMP, LVL_IMMORT, FALSE, "Initiating autowiz.");
// system(buf);
// reboot_wizlists();
// } else
// log("Cannot run autowiz: command-line doesn't fit in buffer.");
// }
// #endif /* CIRCLE_UNIX || CIRCLE_WINDOWS */
// }

pub fn gain_exp(
    chid: DepotId,
    gain: i32,
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
) {
    let ch = chars.get(chid);
    let mut is_altered = false;
    let mut num_levels = 0;

    if !ch.is_npc() && (ch.get_level() < 1 || ch.get_level() > LVL_IMMORT as u8) {
        return;
    }

    if ch.is_npc() {
        let ch = chars.get_mut(chid);
        ch.set_exp(ch.get_exp() + gain);
    }

    if gain > 0 {
        let gain = min(MAX_EXP_GAIN, gain); /* put a cap on the max gain per kill */
        let ch = chars.get_mut(chid);
        ch.set_exp(ch.get_exp() + gain);
        while {
            let ch = chars.get(chid);
            ch.get_level() < (LVL_IMMORT - IMMORT_LEVEL_OK) as u8
                && ch.get_exp() >= level_exp(ch.get_class(), (ch.get_level() + 1) as i16)
        } {
            let ch = chars.get_mut(chid);
            ch.set_level(ch.get_level() + 1);

            num_levels += 1;
            advance_level(chid, game, chars, db, texts, objs);
            is_altered = true;
        }

        if is_altered {
            let ch = chars.get(chid);
            game.mudlog(
                chars,
                DisplayMode::Brief,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "{} advanced {} level{} to level {}.",
                    ch.get_name(),
                    num_levels,
                    if num_levels == 1 { "" } else { "s" },
                    ch.get_level()
                )
                .as_str(),
            );
            if num_levels == 1 {
                send_to_char(&mut game.descriptors, ch, "You rise a level!\r\n");
            } else {
                send_to_char(&mut game.descriptors, ch, format!("You rise {} levels!\r\n", num_levels).as_str());
                let ch = chars.get_mut(chid);
                set_title(ch, None);
                let ch = chars.get(chid);
                if ch.get_level() >= LVL_IMMORT as u8 {
                    // TODO implement autowiz
                    //run_autowiz();
                }
            }
        }
    } else if gain < 0 {
        let gain = max(-MAX_EXP_LOSS, gain); /* Cap max exp lost per death */
        let ch = chars.get_mut(chid);
        ch.set_exp(ch.get_exp() + gain);
        if ch.get_exp() < 0 {
            ch.set_exp(0);
        }
    }
}

pub fn gain_exp_regardless(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    chid: DepotId,
    gain: i32,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
) {
    let ch = chars.get_mut(chid);
    let mut is_altered = false;
    let mut num_levels = 0;

    ch.set_exp(ch.get_exp() + gain);
    if ch.get_exp() < 0 {
        ch.set_exp(0);
    }

    if !ch.is_npc() {
        while {
            let ch = chars.get(chid);
            ch.get_level() < LVL_IMPL as u8
                && ch.get_exp() >= level_exp(ch.get_class(), (ch.get_level() + 1) as i16)
        } {
            let ch = chars.get_mut(chid);
            ch.set_level(ch.get_level() + 1);
            num_levels += 1;
            advance_level(chid, game, chars, db, texts, objs);
            is_altered = true;
        }

        if is_altered {
            let ch = chars.get(chid);
            game.mudlog(
                chars,
                DisplayMode::Brief,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "{} advanced {} level{} to level {}.",
                    ch.get_name(),
                    num_levels,
                    if num_levels == 1 { "" } else { "s" },
                    ch.get_level()
                )
                .as_str(),
            );
            if num_levels == 1 {
                send_to_char(&mut game.descriptors, ch, "You rise a level!\r\n");
            } else {
                send_to_char(&mut game.descriptors, ch, format!("You rise {} levels!\r\n", num_levels).as_str());
            }
            let ch = chars.get_mut(chid);
            set_title(ch, None);
            if ch.get_level() >= LVL_IMMORT as u8 {
                // TODO run_autowiz();
            }
        }
    }
}

    pub(crate) fn gain_condition(
        descs: &mut Depot<DescriptorData>,
        ch: &mut CharData,
        condition: usize,
        value: i32,
    ) {
        if ch.is_npc() || ch.get_cond(condition) == -1 {
            /* No change */
            return;
        }

        let intoxicated = ch.get_cond(DRUNK) > 0;

        ch.incr_cond(condition, value as i16);
        let mut v = ch.get_cond(condition);
        v = max(0, v);
        v = min(24, v);
        ch.set_cond(condition, v);

        if ch.get_cond(condition) == 0 || ch.plr_flagged(PLR_WRITING) {
            return;
        }

        match condition {
            FULL => {
                send_to_char(descs, ch, "You are hungry.\r\n");
            }
            THIRST => {
                send_to_char(descs, ch, "You are thirsty.\r\n");
            }
            DRUNK => {
                if intoxicated {
                    send_to_char(descs, ch, "You are now sober.\r\n");
                }
            }
            _ => {}
        }
    }
impl Game {
    fn check_idling(
        &mut self,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        texts: &mut Depot<TextData>,
        objs: &mut Depot<ObjData>,
        chid: DepotId,
    ) {
        let ch = chars.get_mut(chid);
        ch.char_specials.timer += 1;
        if ch.char_specials.timer > IDLE_VOID {
            if ch.get_was_in() == NOWHERE && ch.in_room() != NOWHERE {
                let ch_in_room = ch.in_room();
                chars.get_mut(chid).set_was_in(ch_in_room);
                let ch = chars.get(chid);
                if ch.fighting_id().is_some() {
                    db.stop_fighting(chars.get_mut(ch.fighting_id().unwrap()));
                    db.stop_fighting(chars.get_mut(chid));
                }
                let ch = chars.get(chid);
                act(&mut self.descriptors, 
                    chars,
                    db,
                    "$n disappears into the void.",
                    true,
                    Some(ch),
                    None,
                    None,
                    TO_ROOM,
                );
                let ch = chars.get(chid);
                send_to_char(&mut self.descriptors, ch, "You have been idle, and are pulled into a void.\r\n");
                save_char(&mut self.descriptors, db, chars, texts, objs, chid);
                crash_crashsave(chars, db, objs, chid);
                db.char_from_room(objs, chars.get_mut(chid));
                db.char_to_room(chars, objs, chid, 1);
            } else if ch.char_specials.timer > IDLE_RENT_TIME {
                if ch.in_room() != NOWHERE {
                    db.char_from_room(objs, chars.get_mut(chid));
                }
                db.char_to_room(chars, objs, chid, 3);
                let ch = chars.get(chid);
                if ch.desc.is_some() {
                    let desc_id = ch.desc.unwrap();
                    self.desc_mut(desc_id).set_state(ConDisconnect);

                    /*
                     * For the 'if (d->character)' test in close_socket().
                     * -gg 3/1/98 (Happy anniversary.)
                     */
                    let ch = chars.get(chid);
                    let desc_id = ch.desc.unwrap();
                    self.desc_mut(desc_id).character = None;
                    let ch = chars.get_mut(chid);
                    ch.desc = None;
                }
                if FREE_RENT {
                    crash_rentsave(self, chars, db, objs, chid, 0);
                } else {
                    crash_idlesave(self, chars, db, objs, chid);
                }
                let ch = chars.get(chid);
                self.mudlog(
                    chars,
                    DisplayMode::Complete,
                    LVL_GOD as i32,
                    true,
                    format!("{} force-rented and extracted (idle).", ch.get_name()).as_str(),
                );
                db.extract_char(chars, chid);
            }
        }
    }

    /* Update PCs, NPCs, and objects */
    pub fn point_update(
        &mut self,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        texts: &mut Depot<TextData>,
        objs: &mut Depot<ObjData>,
    ) {
        /* characters */
        for &i_id in &db.character_list.clone() {
            let i = chars.get_mut(i_id);
            let descs = &mut self.descriptors;
            gain_condition(descs, i, FULL, -1);
        gain_condition(descs, i, DRUNK, -1);
            gain_condition(descs, i, THIRST, -1);
            if i.get_pos() >= Position::Stunned {
                i.set_hit(min(i.get_hit() + hit_gain(i) as i16, i.get_max_hit()));
                i.set_mana(min(i.get_mana() + mana_gain(i) as i16, i.get_max_mana()));
                i.set_move(min(i.get_move() + move_gain(i) as i16, i.get_max_move()));
                if i.aff_flagged(AffectFlags::POISON) {
                    if self.damage(chars, db, texts, objs, i_id, i_id, 2, SPELL_POISON) == -1 {
                        continue; /* Oops, they died. -gg 6/24/98 */
                    }
                }
                let i = chars.get_mut(i_id);
                if i.get_pos() <= Position::Stunned {
                    update_pos(i);
                }
            } else if i.get_pos() == Position::Incapacitated {
                if self.damage(chars, db, texts, objs, i_id, i_id, 1, TYPE_SUFFERING) == -1 {
                    continue;
                }
            } else if i.get_pos() == Position::MortallyWounded {
                if self.damage(chars, db, texts, objs, i_id, i_id, 2, TYPE_SUFFERING) == -1 {
                    continue;
                }
            }
            let i = chars.get(i_id);
            if !i.is_npc() {
                update_char_objects(&mut self.descriptors, chars, objs, db, i_id);
                let i = chars.get(i_id);
                if i.get_level() < IDLE_MAX_LEVEL as u8 {
                    self.check_idling(chars, db, texts, objs, i_id);
                }
            }
        }

        /* objects */
        let mut old_object_list = vec![];
        for &o in &db.object_list {
            old_object_list.push(o);
        }
        for j_id in old_object_list.into_iter() {
            /* If this is a corpse */
            let j_obj = objs.get(j_id);
            if j_obj.is_corpse() {
                /* timer count down */
                if j_obj.get_obj_timer() > 0 {
                    objs.get_mut(j_id).decr_obj_timer(1);
                }
                let j_obj = objs.get(j_id);
                if j_obj.get_obj_timer() == 0 {
                    if j_obj.carried_by.is_some() {
                        let chid = j_obj.carried_by.unwrap();
                        let ch = chars.get(chid);
                        act(&mut self.descriptors, 
                            chars,
                            db,
                            "$p decays in your hands.",
                            false,
                            Some(ch),
                            Some(j_obj),
                            None,
                            TO_CHAR,
                        );
                    } else if j_obj.in_room() != NOWHERE
                        && db.world[j_obj.in_room() as usize].peoples.len() != 0
                    {
                        let chid = db.world[j_obj.in_room() as usize].peoples[0];
                        let ch = chars.get(chid);
                        act(&mut self.descriptors, 
                            chars,
                            db,
                            "A quivering horde of maggots consumes $p.",
                            true,
                            Some(ch),
                            Some(j_obj),
                            None,
                            TO_ROOM,
                        );
                        act(&mut self.descriptors, 
                            chars,
                            db,
                            "A quivering horde of maggots consumes $p.",
                            true,
                            Some(ch),
                            Some(j_obj),
                            None,
                            TO_CHAR,
                        );
                    }
                    let mut old_contains = vec![];
                    for &c in &j_obj.contains {
                        old_contains.push(c);
                    }

                    for contained_id in old_contains.into_iter() {
                        obj_from_obj(chars, objs, contained_id);
                        let j = objs.get_mut(j_id);
                        if j.in_obj.is_some() {
                            let to_obj_id = j.in_obj.unwrap();
                            obj_to_obj(chars, objs, contained_id, to_obj_id);
                        } else if j.carried_by.is_some() {
                            let to_room = chars.get(j.carried_by.unwrap()).in_room();
                            db.obj_to_room(
                                j,
                                to_room,
                            );
                        } else if j.in_room() != NOWHERE {
                            db.obj_to_room(j, j.in_room());
                        } else {
                            //   core_dump();
                        }
                    }
                    db.extract_obj(chars, objs, j_id);
                }
            }
        }
    }
}
