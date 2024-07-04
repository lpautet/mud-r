/* ************************************************************************
*   File: act.offensive.rs                              Part of CircleMUD *
*  Usage: player-level commands of an offensive nature                    *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use crate::depot::{DepotId, HasId};
use crate::{VictimRef, DB};
use crate::act_movement::do_simple_move;
use crate::config::{NOPERSON, OK, PK_ALLOWED};
use crate::fight::{check_killer, compute_armor_class};
use crate::handler::FIND_CHAR_ROOM;
use crate::interpreter::{command_interpreter, half_chop, is_abbrev, one_argument, SCMD_MURDER};
use crate::limits::gain_exp;
use crate::spells::{
    SKILL_BACKSTAB, SKILL_BASH, SKILL_KICK, SKILL_RESCUE, TYPE_HIT, TYPE_PIERCE, TYPE_UNDEFINED,
};
use crate::structs::{
    AFF_CHARM, LVL_IMPL, MOB_AWARE, MOB_NOBASH, NUM_OF_DIRS, POS_FIGHTING, POS_SITTING,
    POS_STANDING, PULSE_VIOLENCE, ROOM_DEATH, ROOM_PEACEFUL, WEAR_WIELD,
};
use crate::util::rand_number;
use crate::{ Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

pub fn do_assist(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();

    if ch.fighting_id().is_some() {
        game.send_to_char(ch,
            "You're already fighting!  How can you assist someone else?\r\n",
        );
        return;
    }
    one_argument(argument, &mut arg);
    let helpee_id;
    if arg.is_empty() {
        game.send_to_char(ch, "Whom do you wish to assist?\r\n");
    } else if {
        helpee_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_ROOM);
        helpee_id.is_none()
    } {
        game.send_to_char(ch, NOPERSON);
    } else if helpee_id.unwrap() == chid {
        game.send_to_char(ch, "You can't help yourself any more than this!\r\n");
    } else {
        /*
         * Hit the same enemy the person you're helping is.
         */
        let helpee_id = helpee_id.unwrap();
        let helpee = db.ch(helpee_id);
        let mut opponent_id = None;
        if helpee.fighting_id().is_some() {
            opponent_id = helpee.fighting_id();
        } else {
            for p_id in db.world[ch.in_room() as usize]
                .peoples
                .iter()
            {
                opponent_id = Some(*p_id);

                let fighting_id = db.ch(*p_id).fighting_id();
                if fighting_id.is_some()
                    && fighting_id.unwrap() == helpee.id()
                {
                    break;
                }
            }
        }

        if opponent_id.is_none() {
            game.act(db,
                "But nobody is fighting $M!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(helpee)),
                TO_CHAR,
            );
        } else if game.can_see(db,ch, db.ch(opponent_id.unwrap())) {
            game.act(db,
                "You can't see who is fighting $M!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(helpee)),
                TO_CHAR,
            );
        } else if !PK_ALLOWED && !db.ch(opponent_id.unwrap()).is_npc() {
            /* prevent accidental pkill */
            let opponent = db.ch(opponent_id.unwrap());
            game.act(db,
                "Use 'murder' if you really want to attack $N.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(opponent)),
                TO_CHAR,
            );
        } else {
            let helpee = db.ch(helpee_id);
            game.send_to_char(ch, "You join the fight!\r\n");
            game.act(db,
                "$N assists you!",
                false,
                Some(helpee),
                None,
                Some(VictimRef::Char(ch)),
                TO_CHAR,
            );
            game.act(db,
                "$n assists $N.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(helpee)),
                TO_NOTVICT,
            );
            game.hit(db, chid, opponent_id.unwrap(), TYPE_UNDEFINED);
        }
    }
}

pub fn do_hit(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = db.ch(chid);

    let mut arg = String::new();
    let vict_id;

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        game.send_to_char(ch, "Hit who?\r\n");
    } else if {
        vict_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_ROOM);
        vict_id.is_none()
    } {
        game.send_to_char(ch, "They don't seem to be here.\r\n");
    } else if vict_id.unwrap() == chid {
        let vict_id = vict_id.unwrap();
        let vict = db.ch(vict_id);
        game.send_to_char(ch, "You hit yourself...OUCH!.\r\n");
        game.act(db,
            "$n hits $mself, and says OUCH!",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_ROOM,
        );
    } else if ch.aff_flagged(AFF_CHARM)
        && ch.master.unwrap() == vict_id.unwrap()
    {
        let vict_id = vict_id.unwrap();
        let vict = db.ch(vict_id);
        game.act(db,
            "$N is just such a good friend, you simply can't hit $M.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR,
        );
    } else {
        let vict_id = vict_id.unwrap();
        let vict = db.ch(vict_id);
        if !PK_ALLOWED {
            if !vict.is_npc() && !ch.is_npc() {
                if subcmd != SCMD_MURDER {
                    game.send_to_char(ch, "Use 'murder' to hit another player.\r\n");
                    return;
                } else {
                    check_killer(chid, vict_id, game,db);
                }
            }
            let ch = db.ch(chid);
            let vict = db.ch(vict_id);
            if ch.aff_flagged(AFF_CHARM)
                && !db.ch(ch.master.unwrap()).is_npc()
                && !vict.is_npc()
            {
                return; /* you can't order a charmed pet to attack a
                         * player */
            }
        }
        let ch = db.ch(chid);
        if ch.get_pos() == POS_STANDING
            && (ch.fighting_id().is_none() || vict_id != ch.fighting_id().unwrap())
        {
            game.hit(db,chid, vict_id, TYPE_UNDEFINED);
            let ch = db.ch_mut(chid);
            ch.set_wait_state((PULSE_VIOLENCE + 2) as i32);
        } else {
            game.send_to_char(ch, "You do the best you can!\r\n");
        }
    }
}

pub fn do_kill(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, cmd: usize, subcmd: i32) {
    let ch = db.ch(chid);
    let mut arg = String::new();

    if ch.get_level() < LVL_IMPL as u8 || ch.is_npc() {
        do_hit(game, db, chid, argument, cmd, subcmd);
        return;
    }
    one_argument(argument, &mut arg);
    let vict_id;
    if arg.is_empty() {
        game.send_to_char(ch, "Kill who?\r\n");
    } else {
        if {
            vict_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_ROOM);
            vict_id.is_none()
        } {
            game.send_to_char(ch, "They aren't here.\r\n");
        } else if chid ==  vict_id.unwrap() {
            game.send_to_char(ch, "Your mother would be so sad.. :(\r\n");
        } else {
            let vict = db.ch(vict_id.unwrap());
            game.act(db,
                "You chop $M to pieces!  Ah!  The blood!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
            game.act(db,
                "$N chops you to pieces!",
                false,
                Some(vict),
                None,
                Some(VictimRef::Char(ch)),
                TO_CHAR,
            );
            game.act(db,
                "$n brutally slays $N!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_NOTVICT,
            );
            game.raw_kill(db, vict_id.unwrap());
        }
    }
}

pub fn do_backstab(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut buf = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_BACKSTAB) == 0 {
        game.send_to_char(ch, "You have no idea how to do that.\r\n");
        return;
    }

    one_argument(argument, &mut buf);
    let vict_id;
    if {
        vict_id = game.get_char_vis(db,chid, &mut buf, None, FIND_CHAR_ROOM);
        vict_id.is_none()
    } {
        game.send_to_char(ch, "Backstab who?\r\n");
        return;
    }
    let vict_id = vict_id.unwrap();
    if vict_id == chid {
        game.send_to_char(ch, "How can you sneak up on yourself?\r\n");
        return;
    }
    if ch.get_eq(WEAR_WIELD as i8).is_none() {
        game.send_to_char(ch, "You need to wield a weapon to make it a success.\r\n");
        return;
    }
    if db.obj(ch.get_eq(WEAR_WIELD as i8).unwrap()).get_obj_val(3) != TYPE_PIERCE - TYPE_HIT {
        game.send_to_char(ch,
            "Only piercing weapons can be used for backstabbing.\r\n",
        );
        return;
    }
    let vict = db.ch(vict_id);
    if vict.fighting_id().is_some() {
        game.send_to_char(ch,
            "You can't backstab a fighting person -- they're too alert!\r\n",
        );
        return;
    }

    if vict.mob_flagged(MOB_AWARE) && vict.awake() {
        game.act(db,
            "You notice $N lunging at you!",
            false,
            Some(vict),
            None,
            Some(VictimRef::Char(ch)),
            TO_CHAR,
        );
        game.act(db,
            "$e notices you lunging at $m!",
            false,
            Some(vict),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        game.act(db,
            "$n notices $N lunging at $m!",
            false,
            Some(vict),
            None,
            Some(VictimRef::Char(ch)),
            TO_NOTVICT,
        );
        game.hit(db,vict_id, chid, TYPE_UNDEFINED);
        return;
    }

    let percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_BACKSTAB);

    if vict.awake() && percent > prob as u32 {
        game.damage(db, chid, vict_id, 0, SKILL_BACKSTAB);
    } else {
        game.hit(db, chid, vict_id, SKILL_BACKSTAB);
    }
    let ch = db.ch_mut(chid);
    ch.set_wait_state((2 * PULSE_VIOLENCE) as i32);
}

pub fn do_order(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut name = String::new();
    let mut message = String::new();
    let mut found = false;
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut name, &mut message);
    let vict_id;
    if name.is_empty() || message.is_empty() {
        game.send_to_char(ch, "Order who to do what?\r\n");
    } else if {
        vict_id = game.get_char_vis(db,chid, &mut name, None, FIND_CHAR_ROOM);
        vict_id.is_none() && !is_abbrev(&name, "followers")
    } {
        game.send_to_char(ch, "That person isn't here.\r\n");
    } else if vict_id.is_some() && chid == vict_id.unwrap() {
        game.send_to_char(ch, "You obviously suffer from skitzofrenia.\r\n");
    } else {
        if ch.aff_flagged(AFF_CHARM) {
            game.send_to_char(ch,
                "Your superior would not aprove of you giving orders.\r\n",
            );
            return;
        }
        if vict_id.is_some() {
            let vict_id = vict_id.unwrap();
            let vict = db.ch(vict_id);
            let buf = format!("$N orders you to '{}'", message);
            game.act(db,&buf, false, Some(vict), None, Some(VictimRef::Char(ch)), TO_CHAR);
            game.act(db,
                "$n gives $N an order.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_ROOM,
            );
            if vict.master.is_some()
                && vict.master.unwrap() != chid
                || !vict.aff_flagged(AFF_CHARM)
            {
                game.act(db,
                    "$n has an indifferent look.",
                    false,
                    Some(vict),
                    None,
                    None,
                    TO_ROOM,
                );
            } else {
                game.send_to_char(ch, OK);
                command_interpreter(game, db, vict_id, &message);
            }
        } else {
            /* This is order "followers" */

            let buf = format!("$n issues the order '{}'.", message);
            game.act(db,&buf, false, Some(ch), None, None, TO_ROOM);
            let ch = db.ch(chid);
            let list = ch.followers.clone();
            for k_id in list {
                let follower = db.ch(k_id.follower);
                let ch = db.ch(chid);
                if ch.in_room() == follower.in_room() {
                    if follower.aff_flagged(AFF_CHARM) {
                        found = true;
                        command_interpreter(game,db, k_id.follower, &message);
                    }
                }
            }
            let ch = db.ch(chid);
            if found {
                game.send_to_char(ch, OK);
            } else {
                game.send_to_char(ch, "Nobody here is a loyal subject of yours!\r\n");
            }
        }
    }
}

pub fn do_flee(game: &mut Game, db: &mut DB, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    if ch.get_pos() < POS_FIGHTING {
        game.send_to_char(ch, "You are in pretty bad shape, unable to flee!\r\n");
        return;
    }
    let was_fighting;
    for _ in 0..6 {
        let attempt = rand_number(0, (NUM_OF_DIRS - 1) as u32); /* Select a random direction */
        if db.can_go(ch, attempt as usize)
            && !db.room_flagged(
                db
                    .exit(ch, attempt as usize)
                    .as_ref()
                    .unwrap()
                    .to_room,
                ROOM_DEATH,
            )
        {
            game.act(db,
                "$n panics, and attempts to flee!",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = db.ch(chid);
            was_fighting = ch.fighting_id();
            let r = do_simple_move(game, db,chid, attempt as i32, true);
            let ch = db.ch(chid);
            if r {
                game.send_to_char(ch, "You flee head over heels.\r\n");
                let ch = db.ch(chid);
                if was_fighting.is_some() && !ch.is_npc() {
                    let was_fighting = db.ch(was_fighting.unwrap());
                    let mut loss = was_fighting.get_max_hit()
                        - was_fighting.get_hit();
                    loss *= was_fighting.get_level() as i16;
                    gain_exp(chid, -loss as i32, game, db);
                }
            } else {
                game.act(db,
                    "$n tries to flee, but can't!",
                    true,
                    Some(ch),
                    None,
                    None,
                    TO_ROOM,
                );
            }
            return;
        }
    }
    game.send_to_char(ch, "PANIC!  You couldn't escape!\r\n");
}

pub fn do_bash(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut arg = String::new();

    one_argument(argument, &mut arg);

    if ch.is_npc() || ch.get_skill(SKILL_BASH) == 0 {
        game.send_to_char(ch, "You have no idea how.\r\n");
        return;
    }
    if db.room_flagged(ch.in_room(), ROOM_PEACEFUL) {
        game.send_to_char(ch,
            "This room just has such a peaceful, easy feeling...\r\n",
        );
        return;
    }
    if ch.get_eq(WEAR_WIELD as i8).is_none() {
        game.send_to_char(ch, "You need to wield a weapon to make it a success.\r\n");
        return;
    }
    let mut victo;
    if {
        victo = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_ROOM);
        victo.is_some()
    } {
        if ch.fighting_id().is_some() && ch.in_room() == db.ch(ch.fighting_id().unwrap()).in_room() {
            victo = ch.fighting_id();
        } else {
            game.send_to_char(ch, "Bash who?\r\n");
            return;
        }
    }
    let vict_id = victo.unwrap();
    if vict_id == chid {
        game.send_to_char(ch, "Aren't we funny today...\r\n");
        return;
    }
    let mut percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_BASH);
    let vict = db.ch(vict_id);
    if vict.mob_flagged(MOB_NOBASH) {
        percent = 101;
    }

    if percent > prob as u32 {
        game.damage(db, chid, vict_id, 0, SKILL_BASH);
        let ch = db.ch_mut(chid);
        ch.set_pos(POS_SITTING);
    } else {
        /*
         * If we bash a player and they wimp out, they will move to the previous
         * room before we set them sitting.  If we try to set the victim sitting
         * first to make sure they don't flee, then we can't bash them!  So now
         * we only set them sitting if they didn't flee. -gg 9/21/98
         */
        if game.damage(db,chid, vict_id, 1, SKILL_BASH) > 0 {
            /* -1 = dead, 0 = miss */
            let vict = db.ch_mut(vict_id);
            vict.set_wait_state(PULSE_VIOLENCE as i32);
            let ch = db.ch(chid);
            let vict = db.ch(vict_id);
            if ch.in_room() == vict.in_room() {
                let vict = db.ch_mut(vict_id);
                vict.set_pos(POS_SITTING);
            }
        }
    }
    let ch = db.ch_mut(chid);
    ch.set_wait_state((PULSE_VIOLENCE * 2) as i32);
}

pub fn do_rescue(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut arg = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_RESCUE) == 0 {
        game.send_to_char(ch, "You have no idea how to do that.\r\n");
        return;
    }

    one_argument(argument, &mut arg);
    let vict_id;
    if {
        vict_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_ROOM);
        vict_id.is_none()
    } {
        game.send_to_char(ch, "Whom do you want to rescue?\r\n");
        return;
    }
    let vict_id = vict_id.unwrap();
    if vict_id == chid {
        game.send_to_char(ch, "What about fleeing instead?\r\n");
        return;
    }
    if ch.fighting_id().is_some() && ch.fighting_id().unwrap() == vict_id {
        game.send_to_char(ch, "How can you rescue someone you are trying to kill?\r\n");
        return;
    }
    let mut tmp_ch_id = None;
    {
        for tch_id in db.world[ch.in_room() as usize].peoples.iter() {
            let tch = db.ch(*tch_id);
            if tch.fighting_id().is_some() && tch.fighting_id().unwrap() == vict_id {
                tmp_ch_id = Some(*tch_id);
                break;
            }
        }
    }
    let vict = db.ch(vict_id);

    if tmp_ch_id.is_none() {
        game.act(db,
            "But nobody is fighting $M!",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR,
        );
        return;
    }
    let tmp_ch_id = tmp_ch_id.unwrap();
    let percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_RESCUE);

    if percent > prob as u32 {
        game.send_to_char(ch, "You fail the rescue!\r\n");
        return;
    }
    game.send_to_char(ch, "Banzai!  To the rescue...\r\n");
    game.act(db,
        "You are rescued by $N, you are confused!",
        false,
        Some(vict),
        None,
        Some(VictimRef::Char(ch)),
        TO_CHAR,
    );
    game.act(db,
        "$n heroically rescues $N!",
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_NOTVICT,
    );
    if vict.fighting_id().is_some() && vict.fighting_id().unwrap() == tmp_ch_id {
        db.stop_fighting(vict_id);
    }
    let tmp_ch = db.ch(tmp_ch_id);
    if tmp_ch.fighting_id().is_some() {
        db.stop_fighting(tmp_ch_id);
    }
    let ch = db.ch(chid);
    if ch.fighting_id().is_some() {
        db.stop_fighting(chid);
    }

    game.set_fighting(db,chid, tmp_ch_id);
    game.set_fighting(db,tmp_ch_id, chid);
    let vict = db.ch_mut(vict_id);
    vict.set_wait_state((2 * PULSE_VIOLENCE) as i32);
}

pub fn do_kick(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut arg = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_KICK) == 0 {
        game.send_to_char(ch, "You have no idea how.\r\n");
        return;
    }
    one_argument(argument, &mut arg);
    let mut vict_id;
    if {
        vict_id = game.get_char_vis(db,chid, &mut arg, None, FIND_CHAR_ROOM);
        vict_id.is_none()
    } {
        if ch.fighting_id().is_some() && ch.in_room() == db.ch(ch.fighting_id().unwrap()).in_room() {
            vict_id = ch.fighting_id();
        } else {
            game.send_to_char(ch, "Kick who?\r\n");
            return;
        }
    }
    let vict_id = vict_id.unwrap();
    let vict = db.ch(vict_id);
    if vict_id == chid {
        game.send_to_char(ch, "Aren't we funny today...\r\n");
        return;
    }
    /* 101% is a complete failure */
    let percent = ((10 - (compute_armor_class(vict) / 10)) * 2) + rand_number(1, 101) as i16;
    let prob = ch.get_skill(SKILL_KICK);

    if percent > prob as i16 {
        game.damage(db,chid, vict_id, 0, SKILL_KICK);
    } else {
        game.damage(db,chid, vict_id, (ch.get_level() / 2) as i32, SKILL_KICK);
    }
    let ch = db.ch_mut(chid);
    ch.set_wait_state((PULSE_VIOLENCE * 3) as i32);
}
