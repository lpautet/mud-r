/* ************************************************************************
*   File: limits.c                                      Part of CircleMUD *
*  Usage: limits & gain funcs for HMV, exp, hunger/thirst, idle time      *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */
use std::cmp::{max, min};
use std::rc::Rc;

use crate::class::{title_female, title_male};
use crate::config::{IDLE_MAX_LEVEL, IDLE_RENT_TIME, IDLE_VOID};
use crate::db::DB;
use crate::structs::ConState::ConDisconnect;
use crate::structs::{
    CharData, AFF_POISON, FULL, LVL_GOD, NOWHERE, POS_INCAP, POS_MORTALLYW, THIRST,
};
use crate::structs::{DRUNK, PLR_WRITING, POS_RESTING, POS_SITTING, POS_STUNNED, SEX_FEMALE};
use crate::util::{age, CMP};
use crate::{send_to_char, MainGlobals, TO_CHAR, TO_ROOM};

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
fn mana_gain(ch: &CharData) -> u8 {
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
            POS_SLEEPING => {
                gain *= 2; /* Divide by 2 */
            }
            POS_RESTING => {
                gain += gain / 2; /* Divide by 4 */
            }
            POS_SITTING => {
                gain += gain / 4; /* Divide by 8 */
            }
        }
        if ch.is_magic_user() || ch.is_cleric() {
            gain *= 2;
        }
        if ch.get_cond(FULL) == 0 || ch.get_cond(THIRST) == 0 {
            gain /= 4;
        }
        if ch.aff_flagged(AFF_POISON) {
            gain /= 4;
        }
    }
    gain
}

/* Hitpoint gain pr. game hour */
fn hit_gain(ch: &CharData) -> u8 {
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
            POS_SLEEPING => {
                gain += gain / 2; /* Divide by 2 */
            }
            POS_RESTING => {
                gain += gain / 4; /* Divide by 4 */
            }
            POS_SITTING => {
                gain += gain / 8; /* Divide by 8 */
            }
        }
        if ch.is_magic_user() || ch.is_cleric() {
            gain /= 2; /* Ouch. */
        }
        if ch.get_cond(FULL) == 0 || ch.get_cond(THIRST) == 0 {
            gain /= 4;
        }

        if ch.aff_flagged(AFF_POISON) {
            gain /= 4;
        }
    }
    gain
}

/* move gain pr. game hour */
fn move_gain(ch: &CharData) -> u8 {
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
            POS_SLEEPING => {
                gain += gain / 2; /* Divide by 2 */
            }
            POS_RESTING => {
                gain += gain / 4; /* Divide by 4 */
            }
            POS_SITTING => {
                gain += gain / 8; /* Divide by 8 */
            }
        }

        if ch.get_cond(FULL) == 0 || ch.get_cond(THIRST) == 0 {
            gain /= 4;
        }

        if ch.aff_flagged(AFF_POISON) {
            gain /= 4;
        }
    }
    gain
}

pub fn set_title(ch: &mut CharData, title: &str) {
    let mut title = title;
    if title.is_empty() {
        if ch.get_sex() == SEX_FEMALE {
            title = title_female(ch.get_class() as i32, ch.get_level() as i32);
        } else {
            title = title_male(ch.get_class() as i32, ch.get_level() as i32);
        }
    }

    ch.set_title(Some(title.parse().unwrap()));
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
//
//
//
// void gain_exp(struct char_data *ch, int gain)
// {
// int is_altered = FALSE;
// int num_levels = 0;
//
// if (!IS_NPC(ch) && ((GET_LEVEL(ch) < 1 || GET_LEVEL(ch) >= LVL_IMMORT)))
// return;
//
// if (IS_NPC(ch)) {
// GET_EXP(ch) += gain;
// return;
// }
// if (gain > 0) {
// gain = MIN(max_exp_gain, gain);	/* put a cap on the max gain per kill */
// GET_EXP(ch) += gain;
// while (GET_LEVEL(ch) < LVL_IMMORT - immort_level_ok &&
// GET_EXP(ch) >= level_exp(GET_CLASS(ch), GET_LEVEL(ch) + 1)) {
// GET_LEVEL(ch) += 1;
// num_levels++;
// advance_level(ch);
// is_altered = TRUE;
// }
//
// if (is_altered) {
// mudlog(BRF, MAX(LVL_IMMORT, GET_INVIS_LEV(ch)), TRUE, "%s advanced %d level%s to level %d.",
// GET_NAME(ch), num_levels, num_levels == 1 ? "" : "s", GET_LEVEL(ch));
// if (num_levels == 1)
// send_to_char(ch, "You rise a level!\r\n");
// else
// send_to_char(ch, "You rise %d levels!\r\n", num_levels);
// set_title(ch, NULL);
// if (GET_LEVEL(ch) >= LVL_IMMORT)
// run_autowiz();
// }
// } else if (gain < 0) {
// gain = MAX(-max_exp_loss, gain);	/* Cap max exp lost per death */
// GET_EXP(ch) += gain;
// if (GET_EXP(ch) < 0)
// GET_EXP(ch) = 0;
// }
// }
//
//
// void gain_exp_regardless(struct char_data *ch, int gain)
// {
// int is_altered = FALSE;
// int num_levels = 0;
//
// GET_EXP(ch) += gain;
// if (GET_EXP(ch) < 0)
// GET_EXP(ch) = 0;
//
// if (!IS_NPC(ch)) {
// while (GET_LEVEL(ch) < LVL_IMPL &&
// GET_EXP(ch) >= level_exp(GET_CLASS(ch), GET_LEVEL(ch) + 1)) {
// GET_LEVEL(ch) += 1;
// num_levels++;
// advance_level(ch);
// is_altered = TRUE;
// }
//
// if (is_altered) {
// mudlog(BRF, MAX(LVL_IMMORT, GET_INVIS_LEV(ch)), TRUE, "%s advanced %d level%s to level %d.",
// GET_NAME(ch), num_levels, num_levels == 1 ? "" : "s", GET_LEVEL(ch));
// if (num_levels == 1)
// send_to_char(ch, "You rise a level!\r\n");
// else
// send_to_char(ch, "You rise %d levels!\r\n", num_levels);
// set_title(ch, NULL);
// if (GET_LEVEL(ch) >= LVL_IMMORT)
// run_autowiz();
// }
// }
// }

impl DB {
    fn gain_condition(&self, ch: &CharData, condition: i32, value: i32) {
        //bool intoxicated;

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
                send_to_char(ch, "You are hungry.\r\n");
            }
            THIRST => {
                send_to_char(ch, "You are thirsty.\r\n");
            }
            DRUNK => {
                if intoxicated {
                    send_to_char(ch, "You are now sober.\r\n");
                }
            }
            _ => {}
        }
    }
}
impl DB {
    fn check_idling(&self, main_globals: &MainGlobals, ch: &Rc<CharData>) {
        ch.char_specials
            .borrow()
            .timer
            .set(ch.char_specials.borrow().timer.get() + 1);
        if ch.char_specials.borrow().timer.get() > IDLE_VOID {
            if ch.get_was_in() == NOWHERE && ch.in_room() != NOWHERE {
                ch.set_was_in(ch.in_room());
                // TODO implement fighting
                // if (FIGHTING(ch)) {
                // stop_fighting(FIGHTING(ch));
                // stop_fighting(ch);
                // }
                self.act(
                    "$n disappears into the void.",
                    true,
                    Some(ch.clone()),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(ch, "You have been idle, and are pulled into a void.\r\n");
                self.save_char(ch);
                // TODO implement crashsave
                // Crash_crashsave(ch);
                self.char_from_room(ch.clone());
                self.char_to_room(Some(ch.clone()), 1);
            } else if ch.char_specials.borrow().timer.get() > IDLE_RENT_TIME {
                if ch.in_room() != NOWHERE {
                    self.char_from_room(ch.clone());
                }
                self.char_to_room(Some(ch.clone()), 3);
                if ch.desc.borrow().is_some() {
                    ch.desc.borrow().as_ref().unwrap().set_state(ConDisconnect);

                    /*
                     * For the 'if (d->character)' test in close_socket().
                     * -gg 3/1/98 (Happy anniversary.)
                     */
                    *ch.desc.borrow().as_ref().unwrap().character.borrow_mut() = None;
                    *ch.desc.borrow_mut() = None;
                }
                // if (free_rent)
                // Crash_rentsave(ch, 0);
                // else
                // Crash_idlesave(ch);
                main_globals.mudlog(
                    CMP,
                    LVL_GOD as i32,
                    true,
                    format!("{} force-rented and extracted (idle).", ch.get_name()).as_str(),
                );
                self.extract_char(ch.clone());
            }
        }
    }

    /* Update PCs, NPCs, and objects */
    pub fn point_update(&self, main_globals: &MainGlobals) {
        // struct char_data * i, * next_char;
        // struct obj_data * j, * next_thing, * jj, *next_thing2;

        /* characters */
        for i in self.character_list.borrow().iter() {
            self.gain_condition(i, FULL, -1);
            self.gain_condition(i, DRUNK, -1);
            self.gain_condition(i, THIRST, -1);

            if i.get_pos() >= POS_STUNNED {
                i.set_hit(min(i.get_hit() + hit_gain(i) as i16, i.get_max_hit()));
                i.set_mana(min(i.get_mana() + mana_gain(i) as i16, i.get_max_mana()));
                i.set_move(min(i.get_move() + move_gain(i) as i16, i.get_max_move()));
                if i.aff_flagged(AFF_POISON) {
                    // TODO implement damage
                    // if (damage(i, i, 2, SPELL_POISON) == -1) {
                    //     continue; /* Oops, they died. -gg 6/24/98 */
                    // }
                }
                if i.get_pos() <= POS_STUNNED {
                    // TODO implement fighting
                    // update_pos(i);
                }
            } else if i.get_pos() == POS_INCAP {
                // TODO implement damage
                // if (damage(i, i, 1, TYPE_SUFFERING) == -1)
                // continue;
            } else if i.get_pos() == POS_MORTALLYW {
                // TODO implement damage
                // if (damage(i, i, 2, TYPE_SUFFERING) == -1)
                // continue;
            }
            if !i.is_npc() {
                self.update_char_objects(i);
                if i.get_level() < IDLE_MAX_LEVEL as u8 {
                    self.check_idling(main_globals, i);
                }
            }
        }

        /* objects */
        for j in self.object_list.borrow().iter() {
            /* If this is a corpse */
            if j.is_corpse() {
                /* timer count down */
                if j.get_obj_timer() > 0 {
                    j.decr_obj_timer(1)
                }

                if j.get_obj_timer() == 0 {
                    if j.carried_by.borrow().is_some() {
                        self.act(
                            "$p decays in your hands.",
                            false,
                            j.carried_by.borrow().clone(),
                            Some(j.as_ref()),
                            None,
                            TO_CHAR,
                        );
                    } else if j.in_room() != NOWHERE
                        && self.world.borrow()[j.in_room() as usize]
                            .peoples
                            .borrow()
                            .len()
                            != 0
                    {
                        self.act(
                            "A quivering horde of maggots consumes $p.",
                            true,
                            Some(
                                self.world.borrow()[j.in_room() as usize].peoples.borrow()[0]
                                    .clone(),
                            ),
                            Some(j.as_ref()),
                            None,
                            TO_ROOM,
                        );
                        self.act(
                            "A quivering horde of maggots consumes $p.",
                            true,
                            Some(
                                self.world.borrow()[j.in_room() as usize].peoples.borrow()[0]
                                    .clone(),
                            ),
                            Some(j.as_ref()),
                            None,
                            TO_CHAR,
                        );
                    }
                    for jj in j.contains.borrow().iter() {
                        DB::obj_from_obj(jj.clone());

                        if j.in_obj.borrow().is_some() {
                            self.obj_to_obj(Some(jj.clone()), j.in_obj.borrow().clone());
                        } else if j.carried_by.borrow().is_some() {
                            self.obj_to_room(
                                Some(jj.clone()),
                                j.carried_by.borrow().as_ref().unwrap().in_room(),
                            );
                        } else if j.in_room() != NOWHERE {
                            self.obj_to_room(Some(jj.clone()), j.in_room());
                        } else {
                            //   core_dump();
                        }
                    }
                    self.extract_obj(j.clone());
                }
            }
        }
    }
}
