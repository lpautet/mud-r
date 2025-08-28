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

use crate::depot::{Depot, DepotId, HasId};
use crate::{act, send_to_char, CharData, ObjData, TextData, VictimRef, DB};
use crate::act_movement::do_simple_move;
use crate::config::{NOPERSON, OK, PK_ALLOWED};
use crate::fight::{check_killer, compute_armor_class, raw_kill};
use crate::handler::{get_char_vis, FindFlags};
use crate::interpreter::{command_interpreter, half_chop, is_abbrev, one_argument, SCMD_MURDER};
use crate::limits::gain_exp;
use crate::spells::{
    SKILL_BACKSTAB, SKILL_BASH, SKILL_KICK, SKILL_RESCUE, TYPE_HIT, TYPE_PIERCE, TYPE_UNDEFINED,
};
use crate::structs::{
     AffectFlags, RoomFlags, LVL_IMPL, MOB_AWARE, MOB_NOBASH, NUM_OF_DIRS,
     PULSE_VIOLENCE, Position, WEAR_WIELD
};
use crate::util::{can_see, rand_number};
use crate::{ Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

pub fn do_assist(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);

    let mut arg = String::new();

    if ch.fighting_id().is_some() {
        send_to_char(&mut game.descriptors, ch,
            "You're already fighting!  How can you assist someone else?\r\n",
        );
        return;
    }
    one_argument(argument, &mut arg);
    let helpee;
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Whom do you wish to assist?\r\n");
    } else if {
        helpee = get_char_vis(&game.descriptors, chars,db,ch, &mut arg, None, FindFlags::CHAR_ROOM);
        helpee.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, NOPERSON);
    } else if helpee.unwrap().id() == chid {
        send_to_char(&mut game.descriptors, ch, "You can't help yourself any more than this!\r\n");
    } else {
        /*
         * Hit the same enemy the person you're helping is.
         */
        let helpee = helpee.unwrap();
        let mut opponent_id = None;
        if helpee.fighting_id().is_some() {
            opponent_id = helpee.fighting_id();
        } else {
            for p_id in db.world[ch.in_room() as usize]
                .peoples
                .iter()
            {
                opponent_id = Some(*p_id);

                let fighting_id = chars.get(*p_id).fighting_id();
                if fighting_id.is_some()
                    && fighting_id.unwrap() == helpee.id()
                {
                    break;
                }
            }
        }

        if opponent_id.is_none() {
            act(&mut game.descriptors, chars, db,
                "But nobody is fighting $M!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(helpee)),
                TO_CHAR,
            );
        } else if can_see(&game.descriptors, chars, db,ch, chars.get(opponent_id.unwrap())) {
            act(&mut game.descriptors, chars, db,
                "You can't see who is fighting $M!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(helpee)),
                TO_CHAR,
            );
        } else if !PK_ALLOWED && !chars.get(opponent_id.unwrap()).is_npc() {
            /* prevent accidental pkill */
            let opponent = chars.get(opponent_id.unwrap());
            act(&mut game.descriptors, chars, db,
                "Use 'murder' if you really want to attack $N.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(opponent)),
                TO_CHAR,
            );
        } else {
            send_to_char(&mut game.descriptors, ch, "You join the fight!\r\n");
            act(&mut game.descriptors, chars, db,
                "$N assists you!",
                false,
                Some(helpee),
                None,
                Some(VictimRef::Char(ch)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, db,
                "$n assists $N.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(helpee)),
                TO_NOTVICT,
            );
            game.hit(chars, db, texts, objs,chid, opponent_id.unwrap(), TYPE_UNDEFINED);
        }
    }
}

pub fn do_hit(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>,texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>,  chid: DepotId, argument: &str, _cmd: usize, subcmd: i32) {
    let ch = chars.get(chid);

    let mut arg = String::new();
    let vict;

    one_argument(argument, &mut arg);
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Hit who?\r\n");
    } else if {
        vict = get_char_vis(&game.descriptors, chars,db,ch, &mut arg, None, FindFlags::CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, "They don't seem to be here.\r\n");
    } else if vict.unwrap().id() == chid {
        let vict = vict.unwrap();
        send_to_char(&mut game.descriptors, ch, "You hit yourself...OUCH!.\r\n");
        act(&mut game.descriptors, chars, db,
            "$n hits $mself, and says OUCH!",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_ROOM,
        );
    } else if ch.aff_flagged(AffectFlags::CHARM)
        && ch.master.unwrap() == vict.unwrap().id()
    {
        let vict = vict.unwrap();
        act(&mut game.descriptors, chars, db,
            "$N is just such a good friend, you simply can't hit $M.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR,
        );
    } else {
        let vict = vict.unwrap();
        let vict_id = vict.id();
        if !PK_ALLOWED {
            if !vict.is_npc() && !ch.is_npc() {
                if subcmd != SCMD_MURDER {
                    send_to_char(&mut game.descriptors, ch, "Use 'murder' to hit another player.\r\n");
                    return;
                } else {
                    check_killer(chid, vict_id, game,chars, db);
                }
            }
            let ch = chars.get(chid);
            let vict = chars.get(vict_id);
            if ch.aff_flagged(AffectFlags::CHARM)
                && !chars.get(ch.master.unwrap()).is_npc()
                && !vict.is_npc()
            {
                return; /* you can't order a charmed pet to attack a
                         * player */
            }
        }
        let ch = chars.get(chid);
        if ch.get_pos() == Position::Standing
            && (ch.fighting_id().is_none() || vict_id != ch.fighting_id().unwrap())
        {
            game.hit(chars, db,texts, objs,chid, vict_id, TYPE_UNDEFINED);
            let ch = chars.get_mut(chid);
            ch.set_wait_state((PULSE_VIOLENCE + 2) as i32);
        } else {
            send_to_char(&mut game.descriptors, ch, "You do the best you can!\r\n");
        }
    }
}

pub fn do_kill(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>,  chid: DepotId, argument: &str, cmd: usize, subcmd: i32) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    if ch.get_level() < LVL_IMPL as u8 || ch.is_npc() {
        do_hit(game, db,chars, texts, objs,chid, argument, cmd, subcmd);
        return;
    }
    one_argument(argument, &mut arg);
    let vict;
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Kill who?\r\n");
    } else {
        if {
            vict = get_char_vis(&game.descriptors, chars,db,ch, &mut arg, None, FindFlags::CHAR_ROOM);
            vict.is_none()
        } {
            send_to_char(&mut game.descriptors, ch, "They aren't here.\r\n");
        } else if chid ==  vict.unwrap().id() {
            send_to_char(&mut game.descriptors, ch, "Your mother would be so sad.. :(\r\n");
        } else {
            let vict = vict.unwrap();
            act(&mut game.descriptors, chars, db,
                "You chop $M to pieces!  Ah!  The blood!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, db,
                "$N chops you to pieces!",
                false,
                Some(vict),
                None,
                Some(VictimRef::Char(ch)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, db,
                "$n brutally slays $N!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_NOTVICT,
            );
            raw_kill(&mut game.descriptors, chars,db,objs, vict.id());
        }
    }
}

pub fn do_backstab(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);
    let mut buf = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_BACKSTAB) == 0 {
        send_to_char(&mut game.descriptors, ch, "You have no idea how to do that.\r\n");
        return;
    }

    one_argument(argument, &mut buf);
    let vict;
    if {
        vict = get_char_vis(&game.descriptors, chars,db,ch, &mut buf, None, FindFlags::CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, "Backstab who?\r\n");
        return;
    }
    let vict = vict.unwrap();
    if vict.id() == chid {
        send_to_char(&mut game.descriptors, ch, "How can you sneak up on yourself?\r\n");
        return;
    }
    if ch.get_eq(WEAR_WIELD).is_none() {
        send_to_char(&mut game.descriptors, ch, "You need to wield a weapon to make it a success.\r\n");
        return;
    }
    if objs.get(ch.get_eq(WEAR_WIELD).unwrap()).get_obj_val(3) != TYPE_PIERCE - TYPE_HIT {
        send_to_char(&mut game.descriptors, ch,
            "Only piercing weapons can be used for backstabbing.\r\n",
        );
        return;
    }
    if vict.fighting_id().is_some() {
        send_to_char(&mut game.descriptors, ch,
            "You can't backstab a fighting person -- they're too alert!\r\n",
        );
        return;
    }

    if vict.mob_flagged(MOB_AWARE) && vict.awake() {
        act(&mut game.descriptors, chars, db,
            "You notice $N lunging at you!",
            false,
            Some(vict),
            None,
            Some(VictimRef::Char(ch)),
            TO_CHAR,
        );
        act(&mut game.descriptors, chars, db,
            "$e notices you lunging at $m!",
            false,
            Some(vict),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        act(&mut game.descriptors, chars, db,
            "$n notices $N lunging at $m!",
            false,
            Some(vict),
            None,
            Some(VictimRef::Char(ch)),
            TO_NOTVICT,
        );
        game.hit(chars, db,texts,objs,vict.id(), chid, TYPE_UNDEFINED);
        return;
    }

    let percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_BACKSTAB);

    if vict.awake() && percent > prob as u32 {
        game.damage(chars, db, texts,objs,chid, vict.id(), 0, SKILL_BACKSTAB);
    } else {
        game.hit(chars, db, texts,objs,chid, vict.id(), SKILL_BACKSTAB);
    }
    let ch = chars.get_mut(chid);
    ch.set_wait_state((2 * PULSE_VIOLENCE) as i32);
}

pub fn do_order(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>,  chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);
    let mut name = String::new();
    let mut message = String::new();
    let mut found = false;
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut name, &mut message);
    let vict;
    if name.is_empty() || message.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Order who to do what?\r\n");
    } else if {
        vict = get_char_vis(&game.descriptors, chars,db,ch, &mut name, None, FindFlags::CHAR_ROOM);
        vict.is_none() && !is_abbrev(&name, "followers")
    } {
        send_to_char(&mut game.descriptors, ch, "That person isn't here.\r\n");
    } else if vict.is_some() && chid == vict.unwrap().id() {
        send_to_char(&mut game.descriptors, ch, "You obviously suffer from skitzofrenia.\r\n");
    } else {
        if ch.aff_flagged(AffectFlags::CHARM) {
            send_to_char(&mut game.descriptors, ch,
                "Your superior would not aprove of you giving orders.\r\n",
            );
            return;
        }
        if vict.is_some() {
            let vict = vict.unwrap();
            let buf = format!("$N orders you to '{}'", message);
            act(&mut game.descriptors, chars, db,&buf, false, Some(vict), None, Some(VictimRef::Char(ch)), TO_CHAR);
            act(&mut game.descriptors, chars, db,
                "$n gives $N an order.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_ROOM,
            );
            if vict.master.is_some()
                && vict.master.unwrap() != chid
                || !vict.aff_flagged(AffectFlags::CHARM)
            {
                act(&mut game.descriptors, chars, db,
                    "$n has an indifferent look.",
                    false,
                    Some(vict),
                    None,
                    None,
                    TO_ROOM,
                );
            } else {
                send_to_char(&mut game.descriptors, ch, OK);
                command_interpreter(game, db, chars, texts,objs, vict.id(), &message);
            }
        } else {
            /* This is order "followers" */

            let buf = format!("$n issues the order '{}'.", message);
            act(&mut game.descriptors, chars, db,&buf, false, Some(ch), None, None, TO_ROOM);
            let ch = chars.get(chid);
            for k_id in ch.followers.clone() {
                let follower = chars.get(k_id.follower);
                let ch = chars.get(chid);
                if ch.in_room() == follower.in_room() {
                    if follower.aff_flagged(AffectFlags::CHARM) {
                        found = true;
                        command_interpreter(game,db, chars, texts, objs,k_id.follower, &message);
                    }
                }
            }
            let ch = chars.get(chid);
            if found {
                send_to_char(&mut game.descriptors, ch, OK);
            } else {
                send_to_char(&mut game.descriptors, ch, "Nobody here is a loyal subject of yours!\r\n");
            }
        }
    }
}

pub fn do_flee(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, chid: DepotId, _argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);
    if ch.get_pos() < Position::Fighting {
        send_to_char(&mut game.descriptors, ch, "You are in pretty bad shape, unable to flee!\r\n");
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
                RoomFlags::DEATH,
            )
        {
            act(&mut game.descriptors, chars, db,
                "$n panics, and attempts to flee!",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let ch = chars.get(chid);
            was_fighting = ch.fighting_id();
            let r = do_simple_move(game, db,chars,texts,objs,chid, attempt as i32, true);
            let ch = chars.get(chid);
            if r {
                send_to_char(&mut game.descriptors, ch, "You flee head over heels.\r\n");
                let ch = chars.get(chid);
                if was_fighting.is_some() && !ch.is_npc() {
                    let was_fighting = chars.get(was_fighting.unwrap());
                    let mut loss = was_fighting.get_max_hit()
                        - was_fighting.get_hit();
                    loss *= was_fighting.get_level() as i16;
                    gain_exp(chid, -loss as i32, game, chars, db,texts,objs);
                }
            } else {
                act(&mut game.descriptors, chars, db,
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
    send_to_char(&mut game.descriptors, ch, "PANIC!  You couldn't escape!\r\n");
}

pub fn do_bash(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, texts: &mut  Depot<TextData>,objs: &mut Depot<ObjData>, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    one_argument(argument, &mut arg);

    if ch.is_npc() || ch.get_skill(SKILL_BASH) == 0 {
        send_to_char(&mut game.descriptors, ch, "You have no idea how.\r\n");
        return;
    }
    if db.room_flagged(ch.in_room(), RoomFlags::PEACEFUL) {
        send_to_char(&mut game.descriptors, ch,
            "This room just has such a peaceful, easy feeling...\r\n",
        );
        return;
    }
    if ch.get_eq(WEAR_WIELD).is_none() {
        send_to_char(&mut game.descriptors, ch, "You need to wield a weapon to make it a success.\r\n");
        return;
    }
    let mut victo;
    if {
        victo = get_char_vis(&game.descriptors, chars,db,ch, &mut arg, None, FindFlags::CHAR_ROOM);
        victo.is_some()
    } {
        if ch.fighting_id().is_some() && ch.in_room() == chars.get(ch.fighting_id().unwrap()).in_room() {
            victo = ch.fighting_id().map(|i| chars.get(i));
        } else {
            send_to_char(&mut game.descriptors, ch, "Bash who?\r\n");
            return;
        }
    }
    let vict = victo.unwrap();
    if vict.id() == chid {
        send_to_char(&mut game.descriptors, ch, "Aren't we funny today...\r\n");
        return;
    }
    let mut percent = rand_number(1, 101); /* 101% is a complete failure */
    let prob = ch.get_skill(SKILL_BASH);
    if vict.mob_flagged(MOB_NOBASH) {
        percent = 101;
    }

    if percent > prob as u32 {
        game.damage(chars, db, texts,objs,chid, vict.id(), 0, SKILL_BASH);
        let ch = chars.get_mut(chid);
        ch.set_pos(Position::Sitting);
    } else {
        let vict_id = vict.id();
        /*
         * If we bash a player and they wimp out, they will move to the previous
         * room before we set them sitting.  If we try to set the victim sitting
         * first to make sure they don't flee, then we can't bash them!  So now
         * we only set them sitting if they didn't flee. -gg 9/21/98
         */
        if game.damage(chars, db,texts,objs,chid, vict_id, 1, SKILL_BASH) > 0 {
            /* -1 = dead, 0 = miss */
            let vict = chars.get_mut(vict_id);
            vict.set_wait_state(PULSE_VIOLENCE as i32);
            let ch = chars.get(chid);
            let vict = chars.get(vict_id);
            if ch.in_room() == vict.in_room() {
                let vict = chars.get_mut(vict_id);
                vict.set_pos(Position::Sitting);
            }
        }
    }
    let ch = chars.get_mut(chid);
    ch.set_wait_state((PULSE_VIOLENCE * 2) as i32);
}

pub fn do_rescue(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>,  chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_RESCUE) == 0 {
        send_to_char(&mut game.descriptors, ch, "You have no idea how to do that.\r\n");
        return;
    }

    one_argument(argument, &mut arg);
    let vict;
    if {
        vict = get_char_vis(&game.descriptors, chars,db,ch, &mut arg, None, FindFlags::CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, "Whom do you want to rescue?\r\n");
        return;
    }
    let vict = vict.unwrap();
    if vict.id() == chid {
        send_to_char(&mut game.descriptors, ch, "What about fleeing instead?\r\n");
        return;
    }
    if ch.fighting_id().is_some() && ch.fighting_id().unwrap() == vict.id() {
        send_to_char(&mut game.descriptors, ch, "How can you rescue someone you are trying to kill?\r\n");
        return;
    }
    let mut tmp_ch_id = None;
    {
        for tch_id in db.world[ch.in_room() as usize].peoples.iter() {
            let tch = chars.get(*tch_id);
            if tch.fighting_id().is_some() && tch.fighting_id().unwrap() == vict.id() {
                tmp_ch_id = Some(*tch_id);
                break;
            }
        }
    }

    if tmp_ch_id.is_none() {
        act(&mut game.descriptors, chars, db,
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
        send_to_char(&mut game.descriptors, ch, "You fail the rescue!\r\n");
        return;
    }
    send_to_char(&mut game.descriptors, ch, "Banzai!  To the rescue...\r\n");
    act(&mut game.descriptors, chars, db,
        "You are rescued by $N, you are confused!",
        false,
        Some(vict),
        None,
        Some(VictimRef::Char(ch)),
        TO_CHAR,
    );
    act(&mut game.descriptors, chars, db,
        "$n heroically rescues $N!",
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_NOTVICT,
    );
    let vict_id = vict.id();
    let vict = chars.get_mut(vict_id);
    if vict.fighting_id().is_some() && vict.fighting_id().unwrap() == tmp_ch_id {
        db.stop_fighting(vict);
    }
    let tmp_ch = chars.get_mut(tmp_ch_id);
    if tmp_ch.fighting_id().is_some() {
        db.stop_fighting(tmp_ch);
    }
    let ch = chars.get_mut(chid);
    if ch.fighting_id().is_some() {
        db.stop_fighting(ch);
    }

    game.set_fighting(chars, db,objs,chid, tmp_ch_id);
    game.set_fighting(chars, db,objs,tmp_ch_id, chid);
    let vict = chars.get_mut(vict_id);
    vict.set_wait_state((2 * PULSE_VIOLENCE) as i32);
}

pub fn do_kick(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>,texts: &mut  Depot<TextData>, objs: &mut Depot<ObjData>, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    if ch.is_npc() || ch.get_skill(SKILL_KICK) == 0 {
        send_to_char(&mut game.descriptors, ch, "You have no idea how.\r\n");
        return;
    }
    one_argument(argument, &mut arg);
    let mut vict;
    if {
        vict = get_char_vis(&game.descriptors, chars,db,ch, &mut arg, None, FindFlags::CHAR_ROOM);
        vict.is_none()
    } {
        if ch.fighting_id().is_some() && ch.in_room() == chars.get(ch.fighting_id().unwrap()).in_room() {
            vict = ch.fighting_id().map(|i| chars.get(i));
        } else {
            send_to_char(&mut game.descriptors, ch, "Kick who?\r\n");
            return;
        }
    }
    let vict = vict.unwrap();
    if vict.id() == chid {
        send_to_char(&mut game.descriptors, ch, "Aren't we funny today...\r\n");
        return;
    }
    /* 101% is a complete failure */
    let percent = ((10 - (compute_armor_class(vict) / 10)) * 2) + rand_number(1, 101) as i16;
    let prob = ch.get_skill(SKILL_KICK);

    if percent > prob as i16 {
        game.damage(chars, db,texts,objs,chid, vict.id(), 0, SKILL_KICK);
    } else {
        game.damage(chars, db,texts,objs,chid, vict.id(), (ch.get_level() / 2) as i32, SKILL_KICK);
    }
    let ch = chars.get_mut(chid);
    ch.set_wait_state((PULSE_VIOLENCE * 3) as i32);
}
