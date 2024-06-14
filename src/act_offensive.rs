/* ************************************************************************
*   File: act.offensive.rs                              Part of CircleMUD *
*  Usage: player-level commands of an offensive nature                    *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use crate::depot::{DepotId, HasId};
use crate::VictimRef;
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

pub fn do_assist(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);

    let mut arg = String::new();

    if ch.fighting_id().is_some() {
        game.send_to_char(
            chid,
            "You're already fighting!  How can you assist someone else?\r\n",
        );
        return;
    }
    one_argument(argument, &mut arg);
    let helpee_id;
    if arg.is_empty() {
        game.send_to_char(chid, "Whom do you wish to assist?\r\n");
    } else if {
        helpee_id = game.get_char_vis(chid, &mut arg, None, FIND_CHAR_ROOM);
        helpee_id.is_none()
    } {
        game.send_to_char(chid, NOPERSON);
    } else if helpee_id.unwrap() == chid {
        game.send_to_char(chid, "You can't help yourself any more than this!\r\n");
    } else {
        /*
         * Hit the same enemy the person you're helping is.
         */
        let helpee_id = helpee_id.unwrap();
        let helpee = game.db.ch(helpee_id);
        let mut opponent_id = None;
        if helpee.fighting_id().is_some() {
            opponent_id = helpee.fighting_id();
        } else {
            for p_id in game.db.world[ch.in_room() as usize]
                .peoples
                .iter()
            {
                opponent_id = Some(*p_id);

                let fighting_id = game.db.ch(*p_id).fighting_id();
                if fighting_id.is_some()
                    && fighting_id.unwrap() == helpee.id()
                {
                    break;
                }
            }
        }

        if opponent_id.is_none() {
            game.act(
                "But nobody is fighting $M!",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(helpee_id)),
                TO_CHAR,
            );
        } else if game.can_see(ch, game.db.ch(opponent_id.unwrap())) {
            game.act(
                "You can't see who is fighting $M!",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(helpee_id)),
                TO_CHAR,
            );
        } else if !PK_ALLOWED && !game.db.ch(opponent_id.unwrap()).is_npc() {
            /* prevent accidental pkill */
            game.act(
                "Use 'murder' if you really want to attack $N.",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(opponent_id.unwrap())),
                TO_CHAR,
            );
        } else {
            game.send_to_char(chid, "You join the fight!\r\n");
            game.act(
                "$N assists you!",
                false,
                Some(helpee_id),
                None,
                Some(VictimRef::Char(chid)),
                TO_CHAR,
            );
            game.act(
                "$n assists $N.",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(helpee_id)),
                TO_NOTVICT,
            );
            game.hit(chid, opponent_id.unwrap(), TYPE_UNDEFINED);
        }
    }
}

pub fn do_hit(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = game.db.ch(chid);

    let mut arg = String::new();
    let vict_id;

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        game.send_to_char(chid, "Hit who?\r\n");
    } else if {
        vict_id = game.get_char_vis(chid, &mut arg, None, FIND_CHAR_ROOM);
        vict_id.is_none()
    } {
        game.send_to_char(chid, "They don't seem to be here.\r\n");
    } else if vict_id.unwrap() == chid {
        game.send_to_char(chid, "You hit yourself...OUCH!.\r\n");
        game.act(
            "$n hits $mself, and says OUCH!",
            false,
            Some(chid),
            None,
            Some(VictimRef::Char(vict_id.unwrap())),
            TO_ROOM,
        );
    } else if ch.aff_flagged(AFF_CHARM)
        && ch.master.borrow().unwrap() == vict_id.unwrap()
    {
        game.act(
            "$N is just such a good friend, you simply can't hit $M.",
            false,
            Some(chid),
            None,
            Some(VictimRef::Char(vict_id.unwrap())),
            TO_CHAR,
        );
    } else {
        let vict_id = vict_id.unwrap();
        let vict = game.db.ch(vict_id);
        if !PK_ALLOWED {
            if !vict.is_npc() && !ch.is_npc() {
                if subcmd != SCMD_MURDER {
                    game.send_to_char(chid, "Use 'murder' to hit another player.\r\n");
                    return;
                } else {
                    check_killer(chid, vict_id, game);
                }
            }
            let ch = game.db.ch(chid);
            let vict = game.db.ch(vict_id);
            if ch.aff_flagged(AFF_CHARM)
                && !game.db.ch(ch.master.borrow().unwrap()).is_npc()
                && !vict.is_npc()
            {
                return; /* you can't order a charmed pet to attack a
                         * player */
            }
        }
        let ch = game.db.ch(chid);
        if ch.get_pos() == POS_STANDING
            && (ch.fighting_id().is_none() || vict_id != ch.fighting_id().unwrap())
        {
            game.hit(chid, vict_id, TYPE_UNDEFINED);
            let ch = game.db.ch(chid);
            ch.set_wait_state((PULSE_VIOLENCE + 2) as i32);
        } else {
            game.send_to_char(chid, "You do the best you can!\r\n");
        }
    }
}

pub fn do_kill(game: &mut Game, chid: DepotId, argument: &str, cmd: usize, subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut arg = String::new();

    if ch.get_level() < LVL_IMPL as u8 || ch.is_npc() {
        do_hit(game, chid, argument, cmd, subcmd);
        return;
    }
    one_argument(argument, &mut arg);
    let vict_id;
    if arg.is_empty() {
        game.send_to_char(chid, "Kill who?\r\n");
    } else {
        if {
            vict_id = game.get_char_vis(chid, &mut arg, None, FIND_CHAR_ROOM);
            vict_id.is_none()
        } {
            game.send_to_char(chid, "They aren't here.\r\n");
        } else if chid ==  vict_id.unwrap() {
            game.send_to_char(chid, "Your mother would be so sad.. :(\r\n");
        } else {
            game.act(
                "You chop $M to pieces!  Ah!  The blood!",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(vict_id.unwrap())),
                TO_CHAR,
            );
            game.act(
                "$N chops you to pieces!",
                false,
                Some(vict_id.unwrap()),
                None,
                Some(VictimRef::Char(chid)),
                TO_CHAR,
            );
            game.act(
                "$n brutally slays $N!",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(vict_id.unwrap())),
                TO_NOTVICT,
            );
            game.raw_kill(vict_id.unwrap());
        }
    }
}

pub fn do_backstab(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut buf = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_BACKSTAB) == 0 {
        game.send_to_char(chid, "You have no idea how to do that.\r\n");
        return;
    }

    one_argument(argument, &mut buf);
    let vict_id;
    if {
        vict_id = game.get_char_vis(chid, &mut buf, None, FIND_CHAR_ROOM);
        vict_id.is_none()
    } {
        game.send_to_char(chid, "Backstab who?\r\n");
        return;
    }
    let vict_id = vict_id.unwrap();
    if vict_id == chid {
        game.send_to_char(chid, "How can you sneak up on yourself?\r\n");
        return;
    }
    if ch.get_eq(WEAR_WIELD as i8).is_none() {
        game.send_to_char(chid, "You need to wield a weapon to make it a success.\r\n");
        return;
    }
    if game.db.obj(ch.get_eq(WEAR_WIELD as i8).unwrap()).get_obj_val(3) != TYPE_PIERCE - TYPE_HIT {
        game.send_to_char(
            chid,
            "Only piercing weapons can be used for backstabbing.\r\n",
        );
        return;
    }
    let vict = game.db.ch(vict_id);
    if vict.fighting_id().is_some() {
        game.send_to_char(
            chid,
            "You can't backstab a fighting person -- they're too alert!\r\n",
        );
        return;
    }

    if vict.mob_flagged(MOB_AWARE) && vict.awake() {
        game.act(
            "You notice $N lunging at you!",
            false,
            Some(vict_id),
            None,
            Some(VictimRef::Char(chid)),
            TO_CHAR,
        );
        game.act(
            "$e notices you lunging at $m!",
            false,
            Some(vict_id),
            None,
            Some(VictimRef::Char(chid)),
            TO_VICT,
        );
        game.act(
            "$n notices $N lunging at $m!",
            false,
            Some(vict_id),
            None,
            Some(VictimRef::Char(chid)),
            TO_NOTVICT,
        );
        game.hit(vict_id, chid, TYPE_UNDEFINED);
        return;
    }

    let percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_BACKSTAB);

    if vict.awake() && percent > prob as u32 {
        game.damage(chid, vict_id, 0, SKILL_BACKSTAB);
    } else {
        game.hit(chid, vict_id, SKILL_BACKSTAB);
    }
    let ch = game.db.ch(chid);
    ch.set_wait_state((2 * PULSE_VIOLENCE) as i32);
}

pub fn do_order(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut name = String::new();
    let mut message = String::new();
    let mut found = false;
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut name, &mut message);
    let vict_id;
    if name.is_empty() || message.is_empty() {
        game.send_to_char(chid, "Order who to do what?\r\n");
    } else if {
        vict_id = game.get_char_vis(chid, &mut name, None, FIND_CHAR_ROOM);
        vict_id.is_none() && !is_abbrev(&name, "followers")
    } {
        game.send_to_char(chid, "That person isn't here.\r\n");
    } else if vict_id.is_some() && chid == vict_id.unwrap() {
        game.send_to_char(chid, "You obviously suffer from skitzofrenia.\r\n");
    } else {
        if ch.aff_flagged(AFF_CHARM) {
            game.send_to_char(
                chid,
                "Your superior would not aprove of you giving orders.\r\n",
            );
            return;
        }
        if vict_id.is_some() {
            let vict_id = vict_id.unwrap();

            let buf = format!("$N orders you to '{}'", message);
            game.act(&buf, false, Some(vict_id), None, Some(VictimRef::Char(chid)), TO_CHAR);
            game.act(
                "$n gives $N an order.",
                false,
                Some(chid),
                None,
                Some(VictimRef::Char(vict_id)),
                TO_ROOM,
            );
            let vict = game.db.ch(vict_id);
            if vict.master.borrow().is_some()
                && vict.master.borrow().unwrap() != chid
                || !vict.aff_flagged(AFF_CHARM)
            {
                game.act(
                    "$n has an indifferent look.",
                    false,
                    Some(vict_id),
                    None,
                    None,
                    TO_ROOM,
                );
            } else {
                game.send_to_char(chid, OK);
                command_interpreter(game, vict_id, &message);
            }
        } else {
            /* This is order "followers" */

            let buf = format!("$n issues the order '{}'.", message);
            game.act(&buf, false, Some(chid), None, None, TO_ROOM);
            let ch = game.db.ch(chid);
            let list = ch.followers.borrow().clone();
            for k_id in list {
                let follower = game.db.ch(k_id.follower);
                let ch = game.db.ch(chid);
                if ch.in_room() == follower.in_room() {
                    if follower.aff_flagged(AFF_CHARM) {
                        found = true;
                        command_interpreter(game, k_id.follower, &message);
                    }
                }
            }
            if found {
                game.send_to_char(chid, OK);
            } else {
                game.send_to_char(chid, "Nobody here is a loyal subject of yours!\r\n");
            }
        }
    }
}

pub fn do_flee(game: &mut Game, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    if ch.get_pos() < POS_FIGHTING {
        game.send_to_char(chid, "You are in pretty bad shape, unable to flee!\r\n");
        return;
    }
    let was_fighting;
    for _ in 0..6 {
        let attempt = rand_number(0, (NUM_OF_DIRS - 1) as u32); /* Select a random direction */
        if game.db.can_go(ch, attempt as usize)
            && !game.db.room_flagged(
                game.db
                    .exit(ch, attempt as usize)
                    .as_ref()
                    .unwrap()
                    .to_room,
                ROOM_DEATH,
            )
        {
            game.act(
                "$n panics, and attempts to flee!",
                true,
                Some(chid),
                None,
                None,
                TO_ROOM,
            );
            was_fighting = ch.fighting_id();
            let r = do_simple_move(game, chid, attempt as i32, true);
            if r {
                game.send_to_char(chid, "You flee head over heels.\r\n");
                if was_fighting.is_some() && !ch.is_npc() {
                    let was_fighting = game.db.ch(was_fighting.unwrap());
                    let mut loss = was_fighting.get_max_hit()
                        - was_fighting.get_hit();
                    loss *= was_fighting.get_level() as i16;
                    gain_exp(ch, -loss as i32, game);
                }
            } else {
                game.act(
                    "$n tries to flee, but can't!",
                    true,
                    Some(chid),
                    None,
                    None,
                    TO_ROOM,
                );
            }
            return;
        }
    }
    game.send_to_char(chid, "PANIC!  You couldn't escape!\r\n");
}

pub fn do_bash(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut arg = String::new();
    let db = &game.db;

    one_argument(argument, &mut arg);

    if ch.is_npc() || ch.get_skill(SKILL_BASH) == 0 {
        game.send_to_char(chid, "You have no idea how.\r\n");
        return;
    }
    if db.room_flagged(ch.in_room(), ROOM_PEACEFUL) {
        game.send_to_char(
            chid,
            "This room just has such a peaceful, easy feeling...\r\n",
        );
        return;
    }
    if ch.get_eq(WEAR_WIELD as i8).is_none() {
        game.send_to_char(chid, "You need to wield a weapon to make it a success.\r\n");
        return;
    }
    let mut victo;
    if {
        victo = game.get_char_vis(chid, &mut arg, None, FIND_CHAR_ROOM);
        victo.is_some()
    } {
        if ch.fighting_id().is_some() && ch.in_room() == game.db.ch(ch.fighting_id().unwrap()).in_room() {
            victo = ch.fighting_id();
        } else {
            game.send_to_char(chid, "Bash who?\r\n");
            return;
        }
    }
    let vict_id = victo.unwrap();
    if vict_id == chid {
        game.send_to_char(chid, "Aren't we funny today...\r\n");
        return;
    }
    let mut percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_BASH);
    let vict = game.db.ch(vict_id);
    if vict.mob_flagged(MOB_NOBASH) {
        percent = 101;
    }

    if percent > prob as u32 {
        game.damage(chid, vict_id, 0, SKILL_BASH);
        let ch = game.db.ch(chid);
        ch.set_pos(POS_SITTING);
    } else {
        /*
         * If we bash a player and they wimp out, they will move to the previous
         * room before we set them sitting.  If we try to set the victim sitting
         * first to make sure they don't flee, then we can't bash them!  So now
         * we only set them sitting if they didn't flee. -gg 9/21/98
         */
        if game.damage(chid, vict_id, 1, SKILL_BASH) > 0 {
            /* -1 = dead, 0 = miss */
            let vict = game.db.ch(vict_id);
            vict.set_wait_state(PULSE_VIOLENCE as i32);
            let ch = game.db.ch(chid);
            if ch.in_room() == vict.in_room() {
                vict.set_pos(POS_SITTING);
            }
        }
    }
    let ch = game.db.ch(chid);
    ch.set_wait_state((PULSE_VIOLENCE * 2) as i32);
}

pub fn do_rescue(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut arg = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_RESCUE) == 0 {
        game.send_to_char(chid, "You have no idea how to do that.\r\n");
        return;
    }

    one_argument(argument, &mut arg);
    let vict_id;
    if {
        vict_id = game.get_char_vis(chid, &mut arg, None, FIND_CHAR_ROOM);
        vict_id.is_none()
    } {
        game.send_to_char(chid, "Whom do you want to rescue?\r\n");
        return;
    }
    let vict_id = vict_id.unwrap();
    if vict_id == chid {
        game.send_to_char(chid, "What about fleeing instead?\r\n");
        return;
    }
    if ch.fighting_id().is_some() && ch.fighting_id().unwrap() == vict_id {
        game.send_to_char(chid, "How can you rescue someone you are trying to kill?\r\n");
        return;
    }
    let mut tmp_ch_id = None;
    {
        for tch_id in game.db.world[ch.in_room() as usize].peoples.iter() {
            let tch = game.db.ch(*tch_id);
            if tch.fighting_id().is_some() && tch.fighting_id().unwrap() == vict_id {
                tmp_ch_id = Some(*tch_id);
                break;
            }
        }
    }

    if tmp_ch_id.is_none() {
        game.act(
            "But nobody is fighting $M!",
            false,
            Some(chid),
            None,
            Some(VictimRef::Char(vict_id)),
            TO_CHAR,
        );
        return;
    }
    let tmp_ch_id = tmp_ch_id.unwrap();
    let percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_RESCUE);

    if percent > prob as u32 {
        game.send_to_char(chid, "You fail the rescue!\r\n");
        return;
    }
    game.send_to_char(chid, "Banzai!  To the rescue...\r\n");
    game.act(
        "You are rescued by $N, you are confused!",
        false,
        Some(vict_id),
        None,
        Some(VictimRef::Char(chid)),
        TO_CHAR,
    );
    game.act(
        "$n heroically rescues $N!",
        false,
        Some(chid),
        None,
        Some(VictimRef::Char(vict_id)),
        TO_NOTVICT,
    );
    let vict = game.db.ch(vict_id);
    if vict.fighting_id().is_some() && vict.fighting_id().unwrap() == tmp_ch_id {
        game.db.stop_fighting(vict_id);
    }
    let tmp_ch = game.db.ch(tmp_ch_id);
    if tmp_ch.fighting_id().is_some() {
        game.db.stop_fighting(tmp_ch_id);
    }
    let ch = game.db.ch(chid);
    if ch.fighting_id().is_some() {
        game.db.stop_fighting(chid);
    }

    game.set_fighting(chid, tmp_ch_id);
    game.set_fighting(tmp_ch_id, chid);
    let vict = game.db.ch(vict_id);
    vict.set_wait_state((2 * PULSE_VIOLENCE) as i32);
}

pub fn do_kick(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    let mut arg = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_KICK) == 0 {
        game.send_to_char(chid, "You have no idea how.\r\n");
        return;
    }
    one_argument(argument, &mut arg);
    let mut vict_id;
    if {
        vict_id = game.get_char_vis(chid, &mut arg, None, FIND_CHAR_ROOM);
        vict_id.is_none()
    } {
        if ch.fighting_id().is_some() && ch.in_room() == game.db.ch(ch.fighting_id().unwrap()).in_room() {
            vict_id = ch.fighting_id();
        } else {
            game.send_to_char(chid, "Kick who?\r\n");
            return;
        }
    }
    let vict_id = vict_id.unwrap();
    let vict = game.db.ch(vict_id);
    if vict_id == chid {
        game.send_to_char(chid, "Aren't we funny today...\r\n");
        return;
    }
    /* 101% is a complete failure */
    let percent = ((10 - (compute_armor_class(vict) / 10)) * 2) + rand_number(1, 101) as i16;
    let prob = ch.get_skill(SKILL_KICK);

    if percent > prob as i16 {
        game.damage(chid, vict_id, 0, SKILL_KICK);
    } else {
        game.damage(chid, vict_id, (ch.get_level() / 2) as i32, SKILL_KICK);
    }
    let ch = game.db.ch(chid);
    ch.set_wait_state((PULSE_VIOLENCE * 3) as i32);
}
