/* ************************************************************************
*   File: mobact.rs                                     Part of CircleMUD *
*  Usage: Functions for generating intelligent (?) behavior in mobiles    *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use log::error;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::act_movement::perform_move;
use crate::act_social::do_action;
use crate::db::DB;
use crate::interpreter::find_command;
use crate::spells::TYPE_UNDEFINED;
use crate::structs::{
    CharData, AFF_BLIND, AFF_CHARM, MOB_AGGRESSIVE, MOB_AGGR_EVIL, MOB_AGGR_GOOD, MOB_AGGR_NEUTRAL,
    MOB_HELPER, MOB_MEMORY, MOB_SCAVENGER, MOB_SENTINEL, MOB_SPEC, MOB_STAY_ZONE, MOB_WIMPY,
    NUM_OF_DIRS, POS_STANDING, PRF_NOHASSLE, ROOM_DEATH, ROOM_NOMOB,
};
use crate::util::{clone_vec, clone_vec2, num_followers_charmed, rand_number};
use crate::{Game, TO_ROOM};

impl Game {
    pub fn mobile_activity(&mut self) {
        let characters = clone_vec2(&self.db.character_list);
        for ch in characters.iter() {
            if !self.db.is_mob(ch) {
                continue;
            }
            /* Examine call for special procedure */
            if ch.mob_flagged(MOB_SPEC) && !self.db.no_specials {
                if self.db.mob_index[ch.get_mob_rnum() as usize].func.is_none() {
                    ch.remove_mob_flags_bit(MOB_SPEC);
                    error!(
                        "SYSERR: {} (#{}): Attempting to call non-existing mob function.",
                        ch.get_name(),
                        self.db.get_mob_vnum(ch)
                    );
                } else {
                    if self.db.mob_index[ch.get_mob_rnum() as usize].func.unwrap()(
                        self, ch, ch, 0, "",
                    ) {
                        continue; /* go to next char */
                    }
                }
            }

            /* If the mob has no specproc, do the default actions */
            if ch.fighting().is_some() || !ch.awake() {
                continue;
            }

            /* Scavenger (picking up objects) */
            if ch.mob_flagged(MOB_SCAVENGER) {
                if self.db.world[ch.in_room() as usize]
                    .contents
                    .borrow()
                    .len()
                    != 0
                    && rand_number(0, 10) == 0
                {
                    let mut max = 1;
                    let mut best_obj = None;
                    {
                        let contents = self.db.world[ch.in_room() as usize].contents.borrow();
                        for obj in contents.iter() {
                            if self.db.can_get_obj(ch, obj) && obj.get_obj_cost() > max {
                                best_obj = Some(obj.clone());
                                max = obj.get_obj_cost();
                            }
                        }
                    }
                    if best_obj.is_some() {
                        self.db.obj_from_room(best_obj.as_ref().unwrap());
                        DB::obj_to_char(best_obj.as_ref().unwrap(), ch);
                        self.db.act(
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
                && self.db.can_go(ch, door as usize)
                && !self.db.room_flagged(
                    self.db.exit(ch, door as usize).unwrap().to_room.get(),
                    ROOM_NOMOB | ROOM_DEATH,
                )
                && (!ch.mob_flagged(MOB_STAY_ZONE)
                    || self.db.world
                        [self.db.exit(ch, door as usize).unwrap().to_room.get() as usize]
                        .zone
                        == self.db.world[ch.in_room() as usize].zone)
            {
                perform_move(self, ch, door as i32, true);
            }

            /* Aggressive Mobs */
            if ch.mob_flagged(MOB_AGGRESSIVE | MOB_AGGR_EVIL | MOB_AGGR_NEUTRAL | MOB_AGGR_GOOD) {
                let mut found = false;
                let peoples_in_room =
                    clone_vec(&self.db.world[ch.in_room() as usize].peoples);
                for vict in peoples_in_room.iter() {
                    if found {
                        break;
                    }
                    if vict.is_npc() || !self.db.can_see(ch, vict) || vict.prf_flagged(PRF_NOHASSLE)
                    {
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
                        self.hit(ch, vict, TYPE_UNDEFINED);
                        found = true;
                    }
                }
            }

            /* Mob Memory */
            if ch.mob_flagged(MOB_MEMORY) && ch.memory().borrow().len() != 0 {
                let mut found = false;
                let peoples_in_room =
                    clone_vec(&self.db.world[ch.in_room() as usize].peoples);
                for vict in peoples_in_room.iter() {
                    if found {
                        break;
                    }
                    if vict.is_npc() || !self.db.can_see(ch, vict) || vict.prf_flagged(PRF_NOHASSLE)
                    {
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
                        self.db.act(
                            "'Hey!  You're the fiend that attacked me!!!', exclaims $n.",
                            false,
                            Some(ch),
                            None,
                            None,
                            TO_ROOM,
                        );
                        self.hit(ch, vict, TYPE_UNDEFINED);
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
                    if self.db.can_see(ch, ch.master.borrow().as_ref().unwrap())
                        && !ch
                            .master
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .prf_flagged(PRF_NOHASSLE)
                    {
                        self.hit(ch, ch.master.borrow().as_ref().unwrap(), TYPE_UNDEFINED);
                        self.db.stop_follower(ch);
                    }
                }
            }

            /* Helper Mobs */
            if ch.mob_flagged(MOB_HELPER) && !ch.aff_flagged(AFF_BLIND | AFF_CHARM) {
                let mut found = false;
                let peoples_in_room =
                    clone_vec(&self.db.world[ch.in_room() as usize].peoples);
                for vict in peoples_in_room.iter() {
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

                    self.db.act(
                        "$n jumps to the aid of $N!",
                        false,
                        Some(ch),
                        None,
                        Some(vict),
                        TO_ROOM,
                    );
                    self.hit(ch, vict.fighting().as_ref().unwrap(), TYPE_UNDEFINED);
                    found = true;
                }
            }

            /* Add new mobile actions here */
        } /* end for() */
    }
}

/* Mob Memory Routines */

/* make ch remember victim */
pub fn remember(ch: &CharData, victim: &CharData) {
    if !ch.is_npc() || victim.is_npc() || victim.prf_flagged(PRF_NOHASSLE) {
        return;
    }

    if !ch.memory().borrow().contains(&victim.get_idnum()) {
        ch.memory().borrow_mut().push(victim.get_idnum());
    }
}

/* make ch forget victim */
pub fn forget(ch: &CharData, victim: &CharData) {
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
const SNARL_CMD: AtomicUsize = AtomicUsize::new(0);
impl Game {
    fn aggressive_mob_on_a_leash(
        &mut self,
        slave: &Rc<CharData>,
        master: Option<&Rc<CharData>>,
        attack: &Rc<CharData>,
    ) -> bool {
        if master.is_none() || slave.aff_flagged(AFF_CHARM) {
            return false;
        }
        if SNARL_CMD.load(Ordering::Acquire) == 0 {
            SNARL_CMD.store(find_command("snarl").unwrap(), Ordering::Release)
        }

        let master = master.unwrap();
        /* Sit. Down boy! HEEEEeeeel! */
        let dieroll = rand_number(1, 20);
        if dieroll != 1
            && (dieroll == 20 || dieroll > (10 - master.get_cha() + slave.get_int()) as u32)
        {
            if rand_number(0, 3) != 0 {
                let victbuf = attack.get_name();

                do_action(
                    self,
                    slave,
                    victbuf.as_ref(),
                    SNARL_CMD.load(Ordering::Relaxed),
                    0,
                );
            }

            /* Success! But for how long? Hehe. */
            return true;
        }

        /* So sorry, now you're a player killer... Tsk tsk. */
        return false;
    }
}
