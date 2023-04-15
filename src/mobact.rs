/* ************************************************************************
*   File: mobact.c                                      Part of CircleMUD *
*  Usage: Functions for generating intelligent (?) behavior in mobiles    *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */
// #define MOB_AGGR_TO_ALIGN (MOB_AGGR_EVIL | MOB_AGGR_NEUTRAL | MOB_AGGR_GOOD)

use std::rc::Rc;

use crate::act_movement::perform_move;
use crate::db::DB;
use crate::spells::TYPE_UNDEFINED;
use crate::structs::{
    CharData, AFF_BLIND, AFF_CHARM, MOB_AGGRESSIVE, MOB_AGGR_EVIL, MOB_AGGR_GOOD, MOB_AGGR_NEUTRAL,
    MOB_HELPER, MOB_MEMORY, MOB_SCAVENGER, MOB_SENTINEL, MOB_STAY_ZONE, MOB_WIMPY, NUM_OF_DIRS,
    POS_STANDING, PRF_NOHASSLE, ROOM_DEATH, ROOM_NOMOB,
};
use crate::util::{num_followers_charmed, rand_number};
use crate::{Game, TO_ROOM};

impl DB {
    pub fn mobile_activity(&self, game: &Game) {
        // struct char_data *ch, *next_ch, *vict;
        // struct obj_data *obj, *best_obj;
        // int door, found, max;
        // memory_rec *names;

        for ch in self.character_list.borrow().iter() {
            if !self.is_mob(ch) {
                continue;
            }
            // TODO implement spec proc
            /* Examine call for special procedure */
            //     if ch.mob_flagged(MOB_SPEC) && !no_specials {
            //         if self.mob_index[ch.get_mob_rnum() as usize].func.isNone() {
            //             error!("SYSERR: {} (#{}): Attempting to call non-existing mob function.", ch.get_name(), ch.get_mob_vnum());
            //             REMOVE_BIT(MOB_FLAGS(ch), MOB_SPEC);
            //         } else {
            //             char actbuf[MAX_INPUT_LENGTH] = "";
            //             if ((mob_index[GET_MOB_RNUM(ch)].func) (ch, ch, 0, actbuf))
            // continue;		/* go to next char */
            //         }
            //     }

            /* If the mob has no specproc, do the default actions */
            if ch.fighting().is_some() || !ch.awake() {
                continue;
            }

            /* Scavenger (picking up objects) */
            if ch.mob_flagged(MOB_SCAVENGER) {
                if self.world.borrow()[ch.in_room() as usize]
                    .contents
                    .borrow()
                    .len()
                    != 0
                    && rand_number(0, 10) == 0
                {
                    let mut max = 1;
                    let mut best_obj = None;
                    {
                        let world = self.world.borrow();
                        let contents = world[ch.in_room() as usize].contents.borrow();
                        for obj in contents.iter() {
                            if self.can_get_obj(ch, obj) && obj.get_obj_cost() > max {
                                best_obj = Some(obj.clone());
                                max = obj.get_obj_cost();
                            }
                        }
                    }
                    if best_obj.is_some() {
                        self.obj_from_room(Some(best_obj.as_ref().unwrap()));
                        DB::obj_to_char(Some(best_obj.as_ref().unwrap()), Some(ch));
                        self.act(
                            "$n gets $p.",
                            false,
                            Some(ch),
                            Some(best_obj.as_ref().unwrap()),
                            None,
                            TO_ROOM,
                        );
                    }
                }
            }

            /* Mob Movement */
            let door = rand_number(0, 18);
            if !ch.mob_flagged(MOB_SENTINEL)
                && ch.get_pos() == POS_STANDING
                && door < NUM_OF_DIRS as u32
                && self.can_go(ch, door as usize)
                && !self.room_flagged(
                    self.exit(ch, door as usize).unwrap().to_room.get(),
                    ROOM_NOMOB | ROOM_DEATH,
                )
                && (!ch.mob_flagged(MOB_STAY_ZONE)
                    || self.world.borrow()
                        [self.exit(ch, door as usize).unwrap().to_room.get() as usize]
                        .zone
                        == self.world.borrow()[ch.in_room() as usize].zone)
            {
                perform_move(game, ch, door as i32, 1);
            }

            /* Aggressive Mobs */
            if ch.mob_flagged(MOB_AGGRESSIVE | MOB_AGGR_EVIL | MOB_AGGR_NEUTRAL | MOB_AGGR_GOOD) {
                let mut found = false;
                for vict in self.world.borrow()[ch.in_room() as usize]
                    .peoples
                    .borrow()
                    .iter()
                {
                    if found {
                        break;
                    }
                    if vict.is_npc() || !self.can_see(ch, vict) || vict.prf_flagged(PRF_NOHASSLE) {
                        continue;
                    }

                    if ch.mob_flagged(MOB_WIMPY) && vict.awake() {
                        continue;
                    }

                    if ch.mob_flagged(MOB_AGGRESSIVE)
                        || (ch.mob_flagged(MOB_AGGR_EVIL) && vict.is_evil())
                        || (ch.mob_flagged(MOB_AGGR_NEUTRAL) && vict.is_neutral())
                        || (ch.mob_flagged(MOB_AGGR_GOOD) && vict.is_good())
                    {
                        /* Can a master successfully control the charmed monster? */
                        if self.aggressive_mob_on_a_leash(ch, ch.master.borrow().as_ref(), vict) {
                            continue;
                        }
                        self.hit(ch, vict, TYPE_UNDEFINED, game);
                        found = true;
                    }
                }
            }

            /* Mob Memory */
            if ch.mob_flagged(MOB_MEMORY) && ch.memory().borrow().len() != 0 {
                let mut found = false;
                for vict in self.world.borrow()[ch.in_room() as usize]
                    .peoples
                    .borrow()
                    .iter()
                {
                    if found {
                        break;
                    }
                    if vict.is_npc() || !self.can_see(ch, vict) || vict.prf_flagged(PRF_NOHASSLE) {
                        continue;
                    }
                    for id in ch.memory().borrow().iter() {
                        if *id != vict.get_idnum() {
                            continue;
                        }

                        /* Can a master successfully control the charmed monster? */
                        if self.aggressive_mob_on_a_leash(ch, ch.master.borrow().as_ref(), vict) {
                            continue;
                        }

                        found = true;
                        self.act(
                            "'Hey!  You're the fiend that attacked me!!!', exclaims $n.",
                            false,
                            Some(ch),
                            None,
                            None,
                            TO_ROOM,
                        );
                        self.hit(ch, vict, TYPE_UNDEFINED, game);
                    }
                }
            }

            /*
             * Charmed Mob Rebellion
             *
             * In order to rebel, there need to be more charmed monsters
             * than the person can feasibly control at a time.  Then the
             * mobiles have a chance based on the charisma of their leader.
             *
             * 1-4 = 0, 5-7 = 1, 8-10 = 2, 11-13 = 3, 14-16 = 4, 17-19 = 5, etc.
             */
            if ch.aff_flagged(AFF_CHARM)
                && ch.master.borrow().is_some()
                && num_followers_charmed(ch.master.borrow().as_ref().unwrap())
                    > ((ch.master.borrow().as_ref().unwrap().get_cha() - 2) / 3) as i32
            {
                if !self.aggressive_mob_on_a_leash(
                    ch,
                    Some(ch.master.borrow().as_ref().unwrap()),
                    ch.master.borrow().as_ref().unwrap(),
                ) {
                    if self.can_see(ch, ch.master.borrow().as_ref().unwrap())
                        && !ch
                            .master
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .prf_flagged(PRF_NOHASSLE)
                    {
                        self.hit(
                            ch,
                            ch.master.borrow().as_ref().unwrap(),
                            TYPE_UNDEFINED,
                            game,
                        );
                        self.stop_follower(ch);
                    }
                }
            }

            /* Helper Mobs */
            if ch.mob_flagged(MOB_HELPER) && !ch.aff_flagged(AFF_BLIND | AFF_CHARM) {
                let mut found = false;
                for vict in self.world.borrow()[ch.in_room() as usize]
                    .peoples
                    .borrow()
                    .iter()
                {
                    if found {
                        break;
                    }
                    if Rc::ptr_eq(vict, &ch) || !vict.is_npc() || vict.fighting().is_none() {
                        continue;
                    }
                    if vict.fighting().as_ref().unwrap().is_npc()
                        || Rc::ptr_eq(ch, vict.fighting().as_ref().unwrap())
                    {
                        continue;
                    }

                    self.act(
                        "$n jumps to the aid of $N!",
                        false,
                        Some(ch),
                        None,
                        Some(vict),
                        TO_ROOM,
                    );
                    self.hit(ch, vict.fighting().as_ref().unwrap(), TYPE_UNDEFINED, game);
                    found = true;
                }
            }

            /* Add new mobile actions here */
        } /* end for() */
    }
}

/* Mob Memory Routines */

/* make ch remember victim */
pub fn remember(ch: &Rc<CharData>, victim: &Rc<CharData>) {
    if !ch.is_npc() || victim.is_npc() || victim.prf_flagged(PRF_NOHASSLE) {
        return;
    }

    if !ch.memory().borrow().contains(&victim.get_idnum()) {
        ch.memory().borrow_mut().push(victim.get_idnum());
    }
}

/* make ch forget victim */
pub fn forget(ch: &Rc<CharData>, victim: &Rc<CharData>) {
    ch.memory()
        .borrow_mut()
        .retain(|id| id != &victim.get_idnum());
}

impl CharData {
    /* erase ch's memory */
    pub fn clear_memory(&self) {
        self.memory().borrow_mut().clear();
    }
}

/*
 * An aggressive mobile wants to attack something.  If
 * they're under the influence of mind altering PC, then
 * see if their master can talk them out of it, eye them
 * down, or otherwise intimidate the slave.
 */
impl DB {
    fn aggressive_mob_on_a_leash(
        &self,
        slave: &CharData,
        master: Option<&Rc<CharData>>,
        attack: &CharData,
    ) -> bool {
        if master.is_none() || slave.aff_flagged(AFF_CHARM) {
            return false;
        }
        // let master = master.unwrap();
        // TODO implement snarl
        // if (!self.snarl_cmd)
        // self.snarl_cmd = find_command("snarl");

        /* Sit. Down boy! HEEEEeeeel! */
        // let dieroll = rand_number(1, 20);
        // if dieroll != 1 && (dieroll == 20 || dieroll > (10 - master.get_cha() + slave.get_int()) as u32) {
        // if snarl_cmd > 0 && attack && rand_number(0, 3) != 0 {
        // char victbuf[MAX_NAME_LENGTH + 1];
        //
        // strncpy(victbuf, GET_NAME(attack), sizeof(victbuf));	/* strncpy: OK */
        // victbuf[sizeof(victbuf) - 1] = '\0';
        //
        // do_action(slave, victbuf, snarl_cmd, 0);
        // }

        //     /* Success! But for how long? Hehe. */
        //     return true
        // }

        /* So sorry, now you're a player killer... Tsk tsk. */
        return false;
    }
}
