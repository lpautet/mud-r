/* ************************************************************************
*   File: mobact.rs                                     Part of CircleMUD *
*  Usage: Functions for generating intelligent (?) behavior in mobiles    *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use log::error;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::act_movement::perform_move;
use crate::act_social::do_action;
use crate::depot::DepotId;
use crate::interpreter::find_command;
use crate::spells::TYPE_UNDEFINED;
use crate::structs::{
    MeRef, CharData, AFF_BLIND, AFF_CHARM, MOB_AGGRESSIVE, MOB_AGGR_EVIL, MOB_AGGR_GOOD, MOB_AGGR_NEUTRAL,
    MOB_HELPER, MOB_MEMORY, MOB_SCAVENGER, MOB_SENTINEL, MOB_SPEC, MOB_STAY_ZONE, MOB_WIMPY,
    NUM_OF_DIRS, POS_STANDING, PRF_NOHASSLE, ROOM_DEATH, ROOM_NOMOB,
};
use crate::util::{ clone_vec2, rand_number};
use crate::{Game, DB, TO_ROOM};
use crate::VictimRef;

impl Game {
    pub fn mobile_activity(&mut self, db: &mut DB) {
        for chid in db.character_list.ids() {
            let ch = db.ch(chid);
            if !db.is_mob(ch) {
                continue;
            }
            /* Examine call for special procedure */
            if ch.mob_flagged(MOB_SPEC) && !db.no_specials {
                if db.mob_index[ch.get_mob_rnum() as usize].func.is_none() {
                    let ch = db.ch_mut(chid);
                    ch.remove_mob_flags_bit(MOB_SPEC);
                    let ch = db.ch(chid);
                    error!(
                        "SYSERR: {} (#{}): Attempting to call non-existing mob function.",
                        ch.get_name(),
                        db.get_mob_vnum(ch)
                    );
                } else {
                    if db.mob_index[ch.get_mob_rnum() as usize].func.unwrap()(
                        self, db, chid, MeRef::Char(chid), 0, "",
                    ) {
                        continue; /* go to next char */
                    }
                }
            }

            /* If the mob has no specproc, do the default actions */
            let ch = db.ch(chid);
            if ch.fighting_id().is_some() || !ch.awake() {
                continue;
            }

            /* Scavenger (picking up objects) */
            if ch.mob_flagged(MOB_SCAVENGER) {
                if db.world[ch.in_room() as usize]
                    .contents
                    .len()
                    != 0
                    && rand_number(0, 10) == 0
                {
                    let mut max = 1;
                    let mut best_obj = None;
                    {
                        let contents = clone_vec2(&db.world[ch.in_room() as usize].contents);
                        for oid in contents.into_iter() {
                            let obj = db.obj(oid);
                            if self.can_get_obj(db, ch, obj) && obj.get_obj_cost() > max {
                                best_obj = Some(oid);
                                max = obj.get_obj_cost();
                            }
                        }
                    }
                    if best_obj.is_some() {
                        db.obj_from_room(best_obj.unwrap());
                        db.obj_to_char(best_obj.unwrap(), chid);
                        self.act(db,
                            "$n gets $p.",
                            false,
                            Some(chid),
                            Some(best_obj.unwrap()),
                            None,
                            TO_ROOM,
                        );
                    }
                }
            }

            /* Mob Movement */
            let door = rand_number(0, 18);
            let ch = db.ch(chid);
            if !ch.mob_flagged(MOB_SENTINEL)
                && ch.get_pos() == POS_STANDING
                && door < NUM_OF_DIRS as u32
                && db.can_go(ch, door as usize)
                && !db.room_flagged(
                    db.exit(ch, door as usize).unwrap().to_room,
                    ROOM_NOMOB | ROOM_DEATH,
                )
                && (!ch.mob_flagged(MOB_STAY_ZONE)
                    || db.world
                        [db.exit(ch, door as usize).unwrap().to_room as usize]
                        .zone
                        == db.world[ch.in_room() as usize].zone)
            {
                perform_move(self, db, chid, door as i32, true);
            }

            /* Aggressive Mobs */
            let ch = db.ch(chid);
            if ch.mob_flagged(MOB_AGGRESSIVE | MOB_AGGR_EVIL | MOB_AGGR_NEUTRAL | MOB_AGGR_GOOD) {
                let mut found = false;
                let peoples_in_room =
                    clone_vec2(&db.world[ch.in_room() as usize].peoples);
                for vict_id in peoples_in_room {
                    let vict = db.ch(vict_id);
                    if found {
                        break;
                    }
                    let ch = db.ch(chid);
                    if vict.is_npc() || !self.can_see(db,ch, vict) || vict.prf_flagged(PRF_NOHASSLE)
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
                        let master_id = ch.master.clone();
                        if self.aggressive_mob_on_a_leash(db, chid, master_id , vict_id) {
                            continue;
                        }
                        self.hit(db, chid, vict_id, TYPE_UNDEFINED);
                        found = true;
                    }
                }
            }

            /* Mob Memory */
            let ch = db.ch(chid);
            if ch.mob_flagged(MOB_MEMORY) && ch.memory().len() != 0 {
                let mut found = false;
                let peoples_in_room =
                    clone_vec2(&db.world[ch.in_room() as usize].peoples);
                for vict_id in peoples_in_room {
                    let vict = db.ch(vict_id);
                    if found {
                        break;
                    }
                    let ch = db.ch(chid);
                    if vict.is_npc() || !self.can_see(db,ch, vict) || vict.prf_flagged(PRF_NOHASSLE)
                    {
                        continue;
                    }
                    let list =  ch.memory().clone();
                    for id in list{
                        let vict = db.ch(vict_id);
                        if id != vict.get_idnum() {
                            continue;
                        }

                        /* Can a master successfully control the charmed monster? */
                        let ch = db.ch(chid);
                        let master_id = ch.master.clone();
                        if self.aggressive_mob_on_a_leash(db, chid, master_id, vict_id) {
                            continue;
                        }

                        found = true;
                        self.act(db,
                            "'Hey!  You're the fiend that attacked me!!!', exclaims $n.",
                            false,
                            Some(chid),
                            None,
                            None,
                            TO_ROOM,
                        );
                        self.hit(db, chid, vict_id, TYPE_UNDEFINED);
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
            let ch = db.ch(chid);
            if ch.aff_flagged(AFF_CHARM)
                && ch.master.is_some()
                && self.num_followers_charmed(db, ch.master.unwrap())
                    > ((db.ch(ch.master.unwrap()).get_cha() - 2) / 3) as i32
            {
                let master_id = ch.master.unwrap();
                if !self.aggressive_mob_on_a_leash(db,
                    chid,
                    Some(master_id),
                    master_id,
                ) {
                    let ch = db.ch(chid);
                    if self.can_see(db,ch, db.ch(ch.master.unwrap()))
                        && !db.ch(ch
                            .master
                            .unwrap())
                            .prf_flagged(PRF_NOHASSLE)
                    {
                        let victim_id = ch.master.unwrap();
                        self.hit(db, chid, victim_id, TYPE_UNDEFINED);
                        self.stop_follower(db, chid);
                    }
                }
            }

            /* Helper Mobs */
            let ch = db.ch(chid);
            if ch.mob_flagged(MOB_HELPER) && !ch.aff_flagged(AFF_BLIND | AFF_CHARM) {
                let mut found = false;
                let peoples_in_room =
                    clone_vec2(&db.world[ch.in_room() as usize].peoples);
                for vict_id in peoples_in_room {
                    let vict = db.ch(vict_id);
                    if found {
                        break;
                    }
                    if vict_id == chid || !vict.is_npc() || vict.fighting_id().is_none() {
                        continue;
                    }
                    if db.ch(vict.fighting_id().unwrap()).is_npc()
                        || chid == vict.fighting_id().unwrap()
                    {
                        continue;
                    }

                    self.act(db,
                        "$n jumps to the aid of $N!",
                        false,
                        Some(chid),
                        None,
                        Some(VictimRef::Char(vict_id)),
                        TO_ROOM,
                    );
let vict = db.ch(vict_id);
                    self.hit(db, chid, vict.fighting_id().unwrap(), TYPE_UNDEFINED);
                    found = true;
                }
            }

            /* Add new mobile actions here */
        } /* end for() */
    }
}

/* Mob Memory Routines */

/* make ch remember victim */

pub fn remember(db: &mut DB, chid: DepotId, victim_id: DepotId) {
    let ch = db.ch(chid);
    let victim = db.ch(victim_id);
    if !ch.is_npc() || victim.is_npc() || victim.prf_flagged(PRF_NOHASSLE) {
        return;
    }
    let victim_idnum = victim.get_idnum();
    let ch = db.ch_mut(chid);
    if !ch.memory().contains(&victim_idnum) {
        ch.mob_specials.memory.push(victim_idnum);
    }
}

/* make ch forget victim */
pub fn forget(db: &mut DB, chid: DepotId, victim_id: DepotId) {
    let victim = db.ch(victim_id);
    let victim_idnum = victim.get_idnum();
    let ch = db.ch_mut(chid);

    ch.mob_specials.memory
        .retain(|id| *id != victim_idnum);
}

impl CharData {
    /* erase ch's memory */
    pub fn clear_memory(&mut self) {
        self.mob_specials.memory.clear();
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
        &mut self, db: &mut DB,
        slave_id: DepotId,
        master_id: Option<DepotId>,
        attack_id: DepotId,
    ) -> bool {
        let slave = db.ch(slave_id);
        if master_id.is_none() || slave.aff_flagged(AFF_CHARM) {
            return false;
        }
        let master_id = master_id.unwrap();
        let master = db.ch(master_id);
        let attack = db.ch(attack_id);

        if SNARL_CMD.load(Ordering::Acquire) == 0 {
            SNARL_CMD.store(find_command("snarl").unwrap(), Ordering::Release)
        }

        /* Sit. Down boy! HEEEEeeeel! */
        let dieroll = rand_number(1, 20);
        if dieroll != 1
            && (dieroll == 20 || dieroll > (10 - master.get_cha() + slave.get_int()) as u32)
        {
            if rand_number(0, 3) != 0 {
                let victbuf = attack.get_name();

                do_action(
                    self, db,
                    slave_id,
                    &victbuf.clone(),
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
