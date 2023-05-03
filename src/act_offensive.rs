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

use std::borrow::Borrow;
use std::rc::Rc;

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
    CharData, AFF_CHARM, LVL_IMPL, MOB_AWARE, MOB_NOBASH, NUM_OF_DIRS, POS_FIGHTING, POS_SITTING,
    POS_STANDING, PULSE_VIOLENCE, ROOM_DEATH, ROOM_PEACEFUL, WEAR_WIELD,
};
use crate::util::rand_number;
use crate::{send_to_char, Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

pub fn do_assist(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();

    if ch.fighting().is_some() {
        send_to_char(
            ch,
            "You're already fighting!  How can you assist someone else?\r\n",
        );
        return;
    }
    one_argument(argument, &mut arg);
    let helpee;
    if arg.is_empty() {
        send_to_char(ch, "Whom do you wish to assist?\r\n");
    } else if {
        helpee = game.db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
        helpee.is_none()
    } {
        send_to_char(ch, NOPERSON);
    } else if Rc::ptr_eq(helpee.as_ref().unwrap(), ch) {
        send_to_char(ch, "You can't help yourself any more than this!\r\n");
    } else {
        /*
         * Hit the same enemy the person you're helping is.
         */
        let helpee = helpee.as_ref().unwrap();
        let mut opponent = None;
        if helpee.fighting().is_some() {
            opponent = helpee.fighting();
        } else {
            for p in game.db.world.borrow()[ch.in_room() as usize]
                .peoples
                .borrow()
                .iter()
            {
                opponent = Some(p.clone());
                if p.fighting().borrow().is_some()
                    && Rc::ptr_eq(p.fighting().borrow().as_ref().unwrap(), helpee)
                {
                    break;
                }
            }
        }

        if opponent.is_none() {
            game.db.act(
                "But nobody is fighting $M!",
                false,
                Some(ch),
                None,
                Some(helpee),
                TO_CHAR,
            );
        } else if game.db.can_see(ch, opponent.as_ref().unwrap()) {
            game.db.act(
                "You can't see who is fighting $M!",
                false,
                Some(ch),
                None,
                Some(helpee),
                TO_CHAR,
            );
        } else if !PK_ALLOWED && !opponent.as_ref().unwrap().is_npc() {
            /* prevent accidental pkill */
            game.db.act(
                "Use 'murder' if you really want to attack $N.",
                false,
                Some(ch),
                None,
                Some(opponent.as_ref().unwrap()),
                TO_CHAR,
            );
        } else {
            send_to_char(ch, "You join the fight!\r\n");
            game.db.act(
                "$N assists you!",
                false,
                Some(helpee),
                None,
                Some(ch),
                TO_CHAR,
            );
            game.db.act(
                "$n assists $N.",
                false,
                Some(ch),
                None,
                Some(helpee),
                TO_NOTVICT,
            );
            game.hit(ch, opponent.as_ref().unwrap(), TYPE_UNDEFINED);
        }
    }
}

pub fn do_hit(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let mut arg = String::new();
    let vict: Option<Rc<CharData>>;

    one_argument(argument, &mut arg);
    let db = &game.db;
    if arg.is_empty() {
        send_to_char(ch, "Hit who?\r\n");
    } else if {
        vict = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(ch, "They don't seem to be here.\r\n");
    } else if Rc::ptr_eq(vict.as_ref().unwrap(), ch) {
        send_to_char(ch, "You hit yourself...OUCH!.\r\n");
        db.act(
            "$n hits $mself, and says OUCH!",
            false,
            Some(ch),
            None,
            Some(vict.as_ref().unwrap()),
            TO_ROOM,
        );
    } else if ch.aff_flagged(AFF_CHARM)
        && Rc::ptr_eq(ch.master.borrow().as_ref().unwrap(), vict.as_ref().unwrap())
    {
        db.act(
            "$N is just such a good friend, you simply can't hit $M.",
            false,
            Some(ch),
            None,
            Some(vict.as_ref().unwrap()),
            TO_CHAR,
        );
    } else {
        let vict = vict.as_ref().unwrap();
        if !PK_ALLOWED {
            if !vict.is_npc() && !ch.is_npc() {
                if subcmd != SCMD_MURDER {
                    send_to_char(ch, "Use 'murder' to hit another player.\r\n");
                    return;
                } else {
                    check_killer(ch, vict, game);
                }
            }
            if ch.aff_flagged(AFF_CHARM)
                && !ch.master.borrow().as_ref().unwrap().is_npc()
                && !vict.is_npc()
            {
                return; /* you can't order a charmed pet to attack a
                         * player */
            }
        }
        if ch.get_pos() == POS_STANDING
            && (ch.fighting().is_none() || !Rc::ptr_eq(vict, ch.fighting().as_ref().unwrap()))
        {
            game.hit(ch, vict, TYPE_UNDEFINED);
            ch.set_wait_state((PULSE_VIOLENCE + 2) as i32);
        } else {
            send_to_char(ch, "You do the best you can!\r\n");
        }
    }
}

pub fn do_kill(game: &mut Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let mut arg = String::new();
    let db = &game.db;

    if ch.get_level() < LVL_IMPL as u8 || ch.is_npc() {
        do_hit(game, ch, argument, cmd, subcmd);
        return;
    }
    one_argument(argument, &mut arg);
    let vict;
    if arg.is_empty() {
        send_to_char(ch, "Kill who?\r\n");
    } else {
        if {
            vict = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
            vict.is_none()
        } {
            send_to_char(ch, "They aren't here.\r\n");
        } else if Rc::ptr_eq(ch, vict.as_ref().unwrap()) {
            send_to_char(ch, "Your mother would be so sad.. :(\r\n");
        } else {
            db.act(
                "You chop $M to pieces!  Ah!  The blood!",
                false,
                Some(ch),
                None,
                Some(vict.as_ref().unwrap()),
                TO_CHAR,
            );
            db.act(
                "$N chops you to pieces!",
                false,
                Some(vict.as_ref().unwrap()),
                None,
                Some(ch),
                TO_CHAR,
            );
            db.act(
                "$n brutally slays $N!",
                false,
                Some(ch),
                None,
                Some(vict.as_ref().unwrap()),
                TO_NOTVICT,
            );
            db.raw_kill(vict.as_ref().unwrap());
        }
    }
}

pub fn do_backstab(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_BACKSTAB) == 0 {
        send_to_char(ch, "You have no idea how to do that.\r\n");
        return;
    }

    one_argument(argument, &mut buf);
    let vict;
    if {
        vict = game.db.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(ch, "Backstab who?\r\n");
        return;
    }
    let vict = vict.as_ref().unwrap();
    if Rc::ptr_eq(vict, ch) {
        send_to_char(ch, "How can you sneak up on yourself?\r\n");
        return;
    }
    if ch.get_eq(WEAR_WIELD as i8).is_none() {
        send_to_char(ch, "You need to wield a weapon to make it a success.\r\n");
        return;
    }
    if ch.get_eq(WEAR_WIELD as i8).as_ref().unwrap().get_obj_val(3) != TYPE_PIERCE - TYPE_HIT {
        send_to_char(
            ch,
            "Only piercing weapons can be used for backstabbing.\r\n",
        );
        return;
    }
    if vict.fighting().is_some() {
        send_to_char(
            ch,
            "You can't backstab a fighting person -- they're too alert!\r\n",
        );
        return;
    }

    if vict.mob_flagged(MOB_AWARE) && vict.awake() {
        game.db.act(
            "You notice $N lunging at you!",
            false,
            Some(vict),
            None,
            Some(ch),
            TO_CHAR,
        );
        game.db.act(
            "$e notices you lunging at $m!",
            false,
            Some(vict),
            None,
            Some(ch),
            TO_VICT,
        );
        game.db.act(
            "$n notices $N lunging at $m!",
            false,
            Some(vict),
            None,
            Some(ch),
            TO_NOTVICT,
        );
        game.hit(vict, ch, TYPE_UNDEFINED);
        return;
    }

    let percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_BACKSTAB);

    if vict.awake() && percent > prob as u32 {
        game.damage(ch, vict, 0, SKILL_BACKSTAB);
    } else {
        game.hit(ch, vict, SKILL_BACKSTAB);
    }
    ch.set_wait_state((2 * PULSE_VIOLENCE) as i32);
}

pub fn do_order(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut name = String::new();
    let mut message = String::new();
    let mut found = false;
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut name, &mut message);
    let vict;
    if name.is_empty() || message.is_empty() {
        send_to_char(ch, "Order who to do what?\r\n");
    } else if {
        vict = db.get_char_vis(ch, &mut name, None, FIND_CHAR_ROOM);
        vict.is_none() && !is_abbrev(&name, "followers")
    } {
        send_to_char(ch, "That person isn't here.\r\n");
    } else if vict.is_some() && Rc::ptr_eq(ch, vict.as_ref().unwrap()) {
        send_to_char(ch, "You obviously suffer from skitzofrenia.\r\n");
    } else {
        if ch.aff_flagged(AFF_CHARM) {
            send_to_char(
                ch,
                "Your superior would not aprove of you giving orders.\r\n",
            );
            return;
        }
        if vict.is_some() {
            let vict = vict.as_ref().unwrap();

            let buf = format!("$N orders you to '{}'", message);
            db.act(&buf, false, Some(vict), None, Some(ch), TO_CHAR);
            db.act(
                "$n gives $N an order.",
                false,
                Some(ch),
                None,
                Some(vict),
                TO_ROOM,
            );

            if vict.master.borrow().is_some()
                && !Rc::ptr_eq(vict.master.borrow().as_ref().unwrap(), ch)
                || !vict.aff_flagged(AFF_CHARM)
            {
                db.act(
                    "$n has an indifferent look.",
                    false,
                    Some(vict),
                    None,
                    None,
                    TO_ROOM,
                );
            } else {
                send_to_char(ch, OK);
                command_interpreter(game, vict, &message);
            }
        } else {
            /* This is order "followers" */

            let buf = format!("$n issues the order '{}'.", message);
            db.act(&buf, false, Some(ch), None, None, TO_ROOM);
            for k in ch.followers.borrow().iter() {
                if ch.in_room() == k.follower.in_room() {
                    if k.follower.aff_flagged(AFF_CHARM) {
                        found = true;
                        command_interpreter(game, &k.follower, &message);
                    }
                }
            }
            if found {
                send_to_char(ch, OK);
            } else {
                send_to_char(ch, "Nobody here is a loyal subject of yours!\r\n");
            }
        }
    }
}

pub fn do_flee(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.get_pos() < POS_FIGHTING {
        send_to_char(ch, "You are in pretty bad shape, unable to flee!\r\n");
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
                    .to_room
                    .get(),
                ROOM_DEATH,
            )
        {
            game.db.act(
                "$n panics, and attempts to flee!",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            was_fighting = ch.fighting();
            let r = do_simple_move(game, ch, attempt as i32, true);
            if r {
                send_to_char(ch, "You flee head over heels.\r\n");
                if was_fighting.is_some() && !ch.is_npc() {
                    let mut loss = was_fighting.as_ref().unwrap().get_max_hit()
                        - was_fighting.as_ref().unwrap().get_hit();
                    loss *= was_fighting.as_ref().unwrap().get_level() as i16;
                    gain_exp(ch, -loss as i32, game);
                }
            } else {
                game.db.act(
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
    send_to_char(ch, "PANIC!  You couldn't escape!\r\n");
}

pub fn do_bash(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    let db = &game.db;

    one_argument(argument, &mut arg);

    if ch.is_npc() || ch.get_skill(SKILL_BASH) == 0 {
        send_to_char(ch, "You have no idea how.\r\n");
        return;
    }
    if db.room_flagged(ch.in_room(), ROOM_PEACEFUL) {
        send_to_char(
            ch,
            "This room just has such a peaceful, easy feeling...\r\n",
        );
        return;
    }
    if ch.get_eq(WEAR_WIELD as i8).is_none() {
        send_to_char(ch, "You need to wield a weapon to make it a success.\r\n");
        return;
    }
    let mut victo;
    if {
        victo = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
        victo.is_some()
    } {
        if ch.fighting().is_some() && ch.in_room() == ch.fighting().as_ref().unwrap().in_room() {
            victo = ch.fighting();
        } else {
            send_to_char(ch, "Bash who?\r\n");
            return;
        }
    }
    let vict = victo.as_ref().unwrap();
    if Rc::ptr_eq(vict, ch) {
        send_to_char(ch, "Aren't we funny today...\r\n");
        return;
    }
    let mut percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_BASH);

    if vict.mob_flagged(MOB_NOBASH) {
        percent = 101;
    }

    if percent > prob as u32 {
        game.damage(ch, vict, 0, SKILL_BASH);
        ch.set_pos(POS_SITTING);
    } else {
        /*
         * If we bash a player and they wimp out, they will move to the previous
         * room before we set them sitting.  If we try to set the victim sitting
         * first to make sure they don't flee, then we can't bash them!  So now
         * we only set them sitting if they didn't flee. -gg 9/21/98
         */
        if game.damage(ch, vict, 1, SKILL_BASH) > 0 {
            /* -1 = dead, 0 = miss */
            vict.set_wait_state(PULSE_VIOLENCE as i32);
            if ch.in_room() == vict.in_room() {
                vict.set_pos(POS_SITTING);
            }
        }
    }
    ch.set_wait_state((PULSE_VIOLENCE * 2) as i32);
}

pub fn do_rescue(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    let db = &game.db;

    if ch.is_npc() || ch.get_skill(SKILL_RESCUE) == 0 {
        send_to_char(ch, "You have no idea how to do that.\r\n");
        return;
    }

    one_argument(argument, &mut arg);
    let vict;
    if {
        vict = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(ch, "Whom do you want to rescue?\r\n");
        return;
    }
    let vict = vict.as_ref().unwrap();
    if Rc::ptr_eq(vict, ch) {
        send_to_char(ch, "What about fleeing instead?\r\n");
        return;
    }
    if ch.fighting().is_some() && Rc::ptr_eq(ch.fighting().as_ref().unwrap(), vict) {
        send_to_char(ch, "How can you rescue someone you are trying to kill?\r\n");
        return;
    }
    let mut tmp_ch = None;
    {
        let w = db.world.borrow();
        for tch in w[ch.in_room() as usize].peoples.borrow().iter() {
            if tch.fighting().is_some() && Rc::ptr_eq(tch.fighting().as_ref().unwrap(), vict) {
                tmp_ch = Some(tch.clone());
                break;
            }
        }
    }

    if tmp_ch.is_none() {
        db.act(
            "But nobody is fighting $M!",
            false,
            Some(ch),
            None,
            Some(vict),
            TO_CHAR,
        );
        return;
    }
    let tmp_ch = tmp_ch.unwrap();
    let percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_RESCUE);

    if percent > prob as u32 {
        send_to_char(ch, "You fail the rescue!\r\n");
        return;
    }
    send_to_char(ch, "Banzai!  To the rescue...\r\n");
    db.act(
        "You are rescued by $N, you are confused!",
        false,
        Some(vict),
        None,
        Some(ch),
        TO_CHAR,
    );
    db.act(
        "$n heroically rescues $N!",
        false,
        Some(ch),
        None,
        Some(vict),
        TO_NOTVICT,
    );

    if vict.fighting().is_some() && Rc::ptr_eq(vict.fighting().as_ref().unwrap(), &tmp_ch) {
        db.stop_fighting(vict);
    }
    if tmp_ch.fighting().is_some() {
        db.stop_fighting(&tmp_ch);
    }
    if ch.fighting().is_some() {
        db.stop_fighting(ch);
    }

    db.set_fighting(ch, &tmp_ch, game);
    db.set_fighting(&tmp_ch, ch, game);

    vict.set_wait_state((2 * PULSE_VIOLENCE) as i32);
}

pub fn do_kick(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    let db = &game.db;

    if ch.is_npc() || ch.get_skill(SKILL_KICK) == 0 {
        send_to_char(ch, "You have no idea how.\r\n");
        return;
    }
    one_argument(argument, &mut arg);
    let mut vict;
    if {
        vict = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        if ch.fighting().is_some() && ch.in_room() == ch.fighting().as_ref().unwrap().in_room() {
            vict = ch.fighting();
        } else {
            send_to_char(ch, "Kick who?\r\n");
            return;
        }
    }
    let vict = vict.as_ref().unwrap();
    if Rc::ptr_eq(vict, ch) {
        send_to_char(ch, "Aren't we funny today...\r\n");
        return;
    }
    /* 101% is a complete failure */
    let percent = ((10 - (compute_armor_class(vict) / 10)) * 2) + rand_number(1, 101) as i16;
    let prob = ch.get_skill(SKILL_KICK);

    if percent > prob as i16 {
        game.damage(ch, vict, 0, SKILL_KICK);
    } else {
        game.damage(ch, vict, (ch.get_level() / 2) as i32, SKILL_KICK);
    }
    ch.set_wait_state((PULSE_VIOLENCE * 3) as i32);
}
