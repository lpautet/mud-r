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
use crate::depot::DepotId;
use crate::fight::update_pos;
use crate::objsave::{crash_crashsave, crash_idlesave, crash_rentsave};
use crate::spells::{SPELL_POISON, TYPE_SUFFERING};
use crate::structs::ConState::ConDisconnect;
use crate::structs::{
    CharData, AFF_POISON, FULL, LVL_GOD, LVL_IMMORT, LVL_IMPL, NOWHERE, POS_INCAP, POS_MORTALLYW,
    THIRST,
};
use crate::structs::{
    DRUNK, PLR_WRITING, POS_RESTING, POS_SITTING, POS_SLEEPING, POS_STUNNED, SEX_FEMALE,
};
use crate::util::{age, BRF, CMP};
use crate::{Game, DB, TO_CHAR, TO_ROOM};

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
            POS_SLEEPING => {
                gain *= 2; /* Divide by 2 */
            }
            POS_RESTING => {
                gain += gain / 2; /* Divide by 4 */
            }
            POS_SITTING => {
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
        if ch.aff_flagged(AFF_POISON) {
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
            POS_SLEEPING => {
                gain += gain / 2; /* Divide by 2 */
            }
            POS_RESTING => {
                gain += gain / 4; /* Divide by 4 */
            }
            POS_SITTING => {
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

        if ch.aff_flagged(AFF_POISON) {
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
            POS_SLEEPING => {
                gain += gain / 2; /* Divide by 2 */
            }
            POS_RESTING => {
                gain += gain / 4; /* Divide by 4 */
            }
            POS_SITTING => {
                gain += gain / 8; /* Divide by 8 */
            }
            _ => {}
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

pub fn set_title(ch: &mut CharData, title: Option<String>) {
    let mut title = title;
    if title.is_none() || title.clone().unwrap().is_empty() {
        if ch.get_sex() == SEX_FEMALE {
            title = Some(title_female(ch.get_class() as i32, ch.get_level() as i32).to_string());
        } else {
            title = Some(title_male(ch.get_class() as i32, ch.get_level() as i32).to_string());
        }
    }

    ch.set_title(title.map(|t| Rc::from(t.as_str())));
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

pub fn gain_exp(chid: DepotId, gain: i32, game: &mut Game, db: &mut DB) {
    let ch = db.ch(chid);
    let mut is_altered = false;
    let mut num_levels = 0;

    if !ch.is_npc() && (ch.get_level() < 1 || ch.get_level() > LVL_IMMORT as u8) {
        return;
    }

    if ch.is_npc() {
        let ch = db.ch_mut(chid);
        ch.set_exp(ch.get_exp() + gain);
    }

    if gain > 0 {
        let gain = min(MAX_EXP_GAIN, gain); /* put a cap on the max gain per kill */
        let ch = db.ch_mut(chid);
        ch.set_exp(ch.get_exp() + gain);
        while {
            let ch = db.ch(chid);
            ch.get_level() < (LVL_IMMORT - IMMORT_LEVEL_OK) as u8
                && ch.get_exp() >= level_exp(ch.get_class(), (ch.get_level() + 1) as i16)
        } {
            let ch = db.ch_mut(chid);
            ch.set_level(ch.get_level() + 1);

            num_levels += 1;
            advance_level(chid, game, db);
            is_altered = true;
        }

        if is_altered {
            let ch = db.ch(chid);
            game.mudlog(db,
                BRF,
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
                game.send_to_char(ch, "You rise a level!\r\n");
            } else {
                game.send_to_char(ch,
                    format!("You rise {} levels!\r\n", num_levels).as_str(),
                );
                let ch = db.ch_mut(chid);
                set_title(ch, None);
                let ch = db.ch(chid);
                if ch.get_level() >= LVL_IMMORT as u8 {
                    // TODO implement autowiz
                    //run_autowiz();
                }
            }
        }
    } else if gain < 0 {
        let gain = max(-MAX_EXP_LOSS, gain); /* Cap max exp lost per death */
        let ch = db.ch_mut(chid);
        ch.set_exp(ch.get_exp() + gain);
        if ch.get_exp() < 0 {
            ch.set_exp(0);
        }
    }
}

pub fn gain_exp_regardless(game: &mut Game, db: &mut DB, chid: DepotId, gain: i32) {
    let ch = db.ch_mut(chid);
    let mut is_altered = false;
    let mut num_levels = 0;

    ch.set_exp(ch.get_exp() + gain);
    if ch.get_exp() < 0 {
        ch.set_exp(0);
    }

    if !ch.is_npc() {
        while {
            let ch = db.ch(chid);
            ch.get_level() < LVL_IMPL as u8
                && ch.get_exp() >= level_exp(ch.get_class(), (ch.get_level() + 1) as i16)
        } {
            let ch = db.ch_mut(chid);
            ch.set_level(ch.get_level() + 1);
            num_levels += 1;
            advance_level(chid, game,db);
            is_altered = true;
        }

        if is_altered {
            let ch = db.ch(chid);
            game.mudlog(db,
                BRF,
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
                game.send_to_char(ch, "You rise a level!\r\n");
            } else {
                game.send_to_char(ch,
                    format!("You rise {} levels!\r\n", num_levels).as_str(),
                );
            }
            let ch = db.ch_mut(chid);
            set_title(ch, None);
            if ch.get_level() >= LVL_IMMORT as u8 {
                // TODO run_autowiz();
            }
        }
    }
}

impl Game {
    pub(crate) fn gain_condition(&mut self, db: &mut DB, chid: DepotId, condition: i32, value: i32) {
        let ch = db.ch_mut(chid);
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
                self.send_to_char(ch, "You are hungry.\r\n");
            }
            THIRST => {
                self.send_to_char(ch, "You are thirsty.\r\n");
            }
            DRUNK => {
                if intoxicated {
                    self.send_to_char(ch, "You are now sober.\r\n");
                }
            }
            _ => {}
        }
    }

    fn check_idling(&mut self, db: &mut DB, chid: DepotId) {
        let ch = db.ch_mut(chid);
        ch.char_specials
            .timer += 1;
        if ch.char_specials.timer > IDLE_VOID {
            if ch.get_was_in() == NOWHERE && ch.in_room() != NOWHERE {
                let ch_in_room = ch.in_room();
                db.ch_mut(chid).set_was_in(ch_in_room);
                let ch = db.ch(chid);
                if ch.fighting_id().is_some() {
                    db.stop_fighting(ch.fighting_id().unwrap());
                    db.stop_fighting(chid);
                }
                self.act(db,
                    "$n disappears into the void.",
                    true,
                    Some(chid),
                    None,
                    None,
                    TO_ROOM,
                );
                let ch = db.ch(chid);
                self.send_to_char(ch, "You have been idle, and are pulled into a void.\r\n");
                self.save_char(db, chid);
                crash_crashsave( db, chid);
                db.char_from_room(chid);
                db.char_to_room(chid, 1);
            } else if ch.char_specials.timer > IDLE_RENT_TIME {
                if ch.in_room() != NOWHERE {
                    db.char_from_room(chid);
                }
                db.char_to_room(chid, 3);
                let ch = db.ch(chid);
                if ch.desc.is_some() {
                    let desc_id = ch.desc.unwrap();
                    self.desc_mut(desc_id)
                        .set_state(ConDisconnect);

                    /*
                     * For the 'if (d->character)' test in close_socket().
                     * -gg 3/1/98 (Happy anniversary.)
                     */
                    let ch = db.ch(chid);
                    let desc_id = ch.desc.unwrap();
                    self.desc_mut(desc_id).character = None;
                    let ch = db.ch_mut(chid);
                    ch.desc = None;
                }
                if FREE_RENT {
                    crash_rentsave(self,db, chid, 0);
                } else {
                    crash_idlesave(self, db, chid);
                }
                let ch = db.ch(chid);
                self.mudlog(db,
                    CMP,
                    LVL_GOD as i32,
                    true,
                    format!("{} force-rented and extracted (idle).", ch.get_name()).as_str(),
                );
                db.extract_char(chid);
            }
        }
    }

    /* Update PCs, NPCs, and objects */
    pub fn point_update(&mut self, db: &mut DB) {
        /* characters */
        for i_id in db.character_list.ids() {
            self.gain_condition(db,i_id, FULL, -1);
            self.gain_condition(db,i_id, DRUNK, -1);
            self.gain_condition(db,i_id, THIRST, -1);
            let i = db.ch_mut(i_id);
            if i.get_pos() >= POS_STUNNED {
                i.set_hit(min(i.get_hit() + hit_gain(i) as i16, i.get_max_hit()));
                i.set_mana(min(i.get_mana() + mana_gain(i) as i16, i.get_max_mana()));
                i.set_move(min(i.get_move() + move_gain(i) as i16, i.get_max_move()));
                if i.aff_flagged(AFF_POISON) {
                    if self.damage(db,i_id, i_id, 2, SPELL_POISON) == -1 {
                        continue; /* Oops, they died. -gg 6/24/98 */
                    }
                }
                let i = db.ch_mut(i_id);
                if i.get_pos() <= POS_STUNNED {
                    update_pos(i);
                }
            } else if i.get_pos() == POS_INCAP {
                if self.damage(db,i_id, i_id, 1, TYPE_SUFFERING) == -1 {
                    continue;
                }
            } else if i.get_pos() == POS_MORTALLYW {
                if self.damage(db, i_id, i_id, 2, TYPE_SUFFERING) == -1 {
                    continue;
                }
            }
            let i = db.ch(i_id);
            if !i.is_npc() {
                self.update_char_objects(db, i_id);
                let i = db.ch(i_id);
                if i.get_level() < IDLE_MAX_LEVEL as u8 {
                    self.check_idling(db, i_id);
                }
            }
        }

        /* objects */
        let mut old_object_list = vec![];
        for o in db.object_list.ids() {
            old_object_list.push(o);
        }
        for j in old_object_list.into_iter() {
            /* If this is a corpse */
            if db.obj(j).is_corpse() {
                /* timer count down */
                if db.obj(j).get_obj_timer() > 0 {
                    db.obj_mut(j).decr_obj_timer(1);
                }

                if db.obj(j).get_obj_timer() == 0 {
                    if db.obj(j).carried_by.is_some() {
                        let chid = db.obj(j).carried_by.unwrap();
                        self.act(db, 
                            "$p decays in your hands.",
                            false,
                            Some(chid),
                            Some(j),
                            None,
                            TO_CHAR,
                        );
                    } else if db.obj(j).in_room() != NOWHERE
                        && db.world[db.obj(j).in_room() as usize]
                            .peoples
                            .len()
                            != 0
                    {
                        let chid =
                            db.world[db.obj(j).in_room() as usize].peoples[0].clone();
                        self.act(db, 
                            "A quivering horde of maggots consumes $p.",
                            true,
                            Some(chid),
                            Some(j),
                            None,
                            TO_ROOM,
                        );
                        self.act(db,
                            "A quivering horde of maggots consumes $p.",
                            true,
                            Some(chid),
                            Some(j),
                            None,
                            TO_CHAR,
                        );
                    }
                    let mut old_contains = vec![];
                    for c in db.obj(j).contains.clone().into_iter() {
                        old_contains.push(c);
                    }

                    for jj in old_contains.into_iter() {
                        db.obj_from_obj(jj);

                        if db.obj(j).in_obj.is_some() {
                            db.obj_to_obj(jj, db.obj(j).in_obj.unwrap());
                        } else if db.obj(j).carried_by.is_some() {
                            db.obj_to_room(
                                jj,
                                db.ch(db.obj(j).carried_by.unwrap()).in_room(),
                            );
                        } else if db.obj(j).in_room() != NOWHERE {
                            db.obj_to_room(jj, db.obj(j).in_room());
                        } else {
                            //   core_dump();
                        }
                    }
                    self.extract_obj(db, j);
                }
            }
        }
    }
}
