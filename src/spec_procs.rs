/* ************************************************************************
*   File: spec_procs.c                                  Part of CircleMUD *
*  Usage: implementation of special procedures for mobiles/objects/rooms  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::any::Any;
use std::cmp::{max, min};
use std::rc::Rc;

use crate::act_comm::do_say;
use crate::act_item::do_drop;
use crate::act_social::do_action;
use crate::class::{GUILD_INFO, PRAC_PARAMS};
use crate::constants::INT_APP;
use crate::db::DB;
use crate::interpreter::{cmd_is, find_command, is_move, SCMD_DROP};
use crate::limits::gain_exp;
use crate::modify::page_string;
use crate::spell_parser::{call_magic, cast_spell, find_skill_num};
use crate::spells::{
    CAST_SPELL, SPELL_BLINDNESS, SPELL_BURNING_HANDS, SPELL_CHILL_TOUCH, SPELL_COLOR_SPRAY,
    SPELL_DISPEL_EVIL, SPELL_ENERGY_DRAIN, SPELL_FIREBALL, SPELL_LIGHTNING_BOLT,
    SPELL_MAGIC_MISSILE, SPELL_POISON, SPELL_SHOCKING_GRASP, TYPE_UNDEFINED,
};
use crate::structs::{
    CharData, AFF_BLIND, ITEM_DRINKCON, ITEM_WEAR_TAKE, LVL_IMMORT, MAX_SKILLS, NOWHERE,
    PLR_KILLER, PLR_THIEF, POS_FIGHTING, POS_STANDING,
};
use crate::util::{clone_vec, rand_number};
use crate::{send_to_char, Game, TO_NOTVICT, TO_ROOM, TO_VICT};

/* ********************************************************************
*  Special procedures for mobiles                                     *
******************************************************************** */

pub fn sort_spells(db: &mut DB) {
    /* initialize array, avoiding reserved. */
    for a in 1..(MAX_SKILLS + 1) {
        db.spell_sort_info[a] = a as i32;
    }

    db.spell_sort_info
        .sort_by_key(|s| db.spell_info[*s as usize].name);
}

fn how_good(percent: i8) -> &'static str {
    if percent < 0 {
        return " error)";
    };
    if percent == 0 {
        return " (not learned)";
    };
    if percent <= 10 {
        return " (awful)";
    }
    if percent <= 20 {
        return " (bad)";
    }
    if percent <= 40 {
        return " (poor)";
    }
    if percent <= 55 {
        return " (average)";
    }
    if percent <= 70 {
        return " (fair)";
    }
    if percent <= 80 {
        return " (good)";
    }
    if percent <= 85 {
        return " (very good)";
    }

    " (superb)"
}

const PRAC_TYPES: [&str; 2] = ["spell", "skill"];

const LEARNED_LEVEL: usize = 0; /* % known which is considered "learned" */
const MAX_PER_PRAC: usize = 1; /* max percent gain in skill per practice */
const MIN_PER_PRAC: usize = 2; /* min percent gain in skill per practice */
const PRAC_TYPE: usize = 3; /* should it say 'spell' or 'skill'?	 */

fn learned(ch: &Rc<CharData>) -> i8 {
    PRAC_PARAMS[LEARNED_LEVEL][ch.get_class() as usize] as i8
}

fn mingain(ch: &Rc<CharData>) -> i32 {
    PRAC_PARAMS[MIN_PER_PRAC][ch.get_class() as usize]
}

fn maxgain(ch: &Rc<CharData>) -> i32 {
    PRAC_PARAMS[MAX_PER_PRAC][ch.get_class() as usize]
}

fn splskl(ch: &Rc<CharData>) -> &str {
    PRAC_TYPES[PRAC_PARAMS[PRAC_TYPE][ch.get_class() as usize] as usize]
}

pub fn list_skills(db: &DB, ch: &Rc<CharData>) {
    if ch.get_practices() == 0 {
        send_to_char(ch, "You have no practice sessions remaining.\r\n");
        return;
    }

    let mut buf = format!(
        "You have {} practice session{} remaining.\r\nYou know of the following {}s:\r\n",
        ch.get_practices(),
        if ch.get_practices() == 1 { "" } else { "s" },
        splskl(ch)
    );

    for sortpos in 1..(MAX_SKILLS + 1) as usize {
        let i = db.spell_sort_info[sortpos] as usize;
        if ch.get_level() >= db.spell_info[i].min_level[ch.get_class() as usize] as u8 {
            buf.push_str(
                format!(
                    "{:20} {}\r\n",
                    db.spell_info[i].name,
                    how_good(ch.get_skill(i as i32))
                )
                .as_str(),
            );
        }
    }

    page_string(ch.desc.borrow().as_ref(), &buf, true);
}

#[allow(unused_variables)]
pub fn guild(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    let db = &game.db;
    if ch.is_npc() || !cmd_is(cmd, "practice") {
        return false;
    }

    let argument = argument.trim();

    if argument.is_empty() {
        list_skills(db, ch);
        return true;
    }

    if ch.get_practices() <= 0 {
        send_to_char(ch, "You do not seem to be able to practice now.\r\n");
        return true;
    }

    let skill_num = find_skill_num(db, argument);

    if skill_num.is_none()
        || ch.get_level()
            < db.spell_info[skill_num.unwrap() as usize].min_level[ch.get_class() as usize] as u8
    {
        send_to_char(
            ch,
            format!("You do not know of that {}.\r\n", splskl(ch)).as_str(),
        );
        return true;
    }
    if ch.get_skill(skill_num.unwrap()) >= learned(ch) {
        send_to_char(ch, "You are already learned in that area.\r\n");
        return true;
    }
    send_to_char(ch, "You practice for a while...\r\n");
    ch.set_practices(ch.get_practices() - 1);

    let mut percent = ch.get_skill(skill_num.unwrap());
    percent += min(
        maxgain(ch),
        max(mingain(ch), INT_APP[ch.get_int() as usize].learn as i32),
    ) as i8;

    ch.set_skill(skill_num.unwrap(), min(learned(ch), percent));

    if ch.get_skill(skill_num.unwrap()) >= learned(ch) {
        send_to_char(ch, "You are now learned in that area.\r\n");
    }

    true
}

#[allow(unused_variables)]
pub fn dump(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    let db = &game.db;
    for k in clone_vec(&game.db.world.borrow()[ch.in_room() as usize].contents) {
        db.act(
            "$p vanishes in a puff of smoke!",
            false,
            None,
            Some(&k),
            None,
            TO_ROOM,
        );
        game.db.extract_obj(&k);
    }

    if !cmd_is(cmd, "drop") {
        return false;
    }

    do_drop(game, ch, argument, cmd as usize, SCMD_DROP as i32);
    let mut value = 0;
    for k in clone_vec(&game.db.world.borrow()[ch.in_room() as usize].contents) {
        db.act(
            "$p vanishes in a puff of smoke!",
            false,
            None,
            Some(&k),
            None,
            TO_ROOM,
        );
        value += max(1, min(50, k.get_obj_cost() / 10));
        game.db.extract_obj(&k);
    }

    if value != 0 {
        send_to_char(ch, "You are awarded for outstanding performance.\r\n");
        db.act(
            "$n has been awarded for being a good citizen.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );

        if ch.get_level() < 3 {
            gain_exp(ch, value, game);
        } else {
            ch.set_gold(ch.get_gold() + value);
        }
    }
    true
}

// pub fn mayor(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
//
// const OPEN_PATH: &str = "W3a3003b33000c111d0d111Oe333333Oe22c222112212111a1S.";
// const CLOSE_PATH: &str = "W3a3003b33000c111d0d111CE333333CE22c222112212111a1S.";
//
// static const char *path = None;
// static int path_index;
// static bool move = false;
//
// if (!move) {
// if (time_info.hours == 6) {
// move = true;
// path = OPEN_PATH;
// path_index = 0;
// } else if (time_info.hours == 20) {
// move = true;
// path = CLOSE_PATH;
// path_index = 0;
// }
// }
// if (cmd || !move || (GET_POS(ch) < POS_SLEEPING) ||
// (GET_POS(ch) == POS_FIGHTING))
// return (false);
//
// switch (path[path_index]) {
// case '0':
// case '1':
// case '2':
// case '3':
// perform_move(ch, path[path_index] - '0', 1);
// break;
//
// case 'W':
// GET_POS(ch) = POS_STANDING;
// act("$n awakens and groans loudly.", false, ch, 0, 0, TO_ROOM);
// break;
//
// case 'S':
// GET_POS(ch) = POS_SLEEPING;
// act("$n lies down and instantly falls asleep.", false, ch, 0, 0, TO_ROOM);
// break;
//
// case 'a':
// act("$n says 'Hello Honey!'", false, ch, 0, 0, TO_ROOM);
// act("$n smirks.", false, ch, 0, 0, TO_ROOM);
// break;
//
// case 'b':
// act("$n says 'What a view!  I must get something done about that dump!'",
// false, ch, 0, 0, TO_ROOM);
// break;
//
// case 'c':
// act("$n says 'Vandals!  Youngsters nowadays have no respect for anything!'",
// false, ch, 0, 0, TO_ROOM);
// break;
//
// case 'd':
// act("$n says 'Good day, citizens!'", false, ch, 0, 0, TO_ROOM);
// break;
//
// case 'e':
// act("$n says 'I hereby declare the bazaar open!'", false, ch, 0, 0, TO_ROOM);
// break;
//
// case 'E':
// act("$n says 'I hereby declare Midgaard closed!'", false, ch, 0, 0, TO_ROOM);
// break;
//
// case 'O':
// do_gen_door(ch, strcpy(actbuf, "gate"), 0, SCMD_UNLOCK);	/* strcpy: OK */
// do_gen_door(ch, strcpy(actbuf, "gate"), 0, SCMD_OPEN);	/* strcpy: OK */
// break;
//
// case 'C':
// do_gen_door(ch, strcpy(actbuf, "gate"), 0, SCMD_CLOSE);	/* strcpy: OK */
// do_gen_door(ch, strcpy(actbuf, "gate"), 0, SCMD_LOCK);	/* strcpy: OK */
// break;
//
// case '.':
// move = false;
// break;
//
// }
//
// path_index++;
// return (false);
// }

/* ********************************************************************
*  General special procedures for mobiles                             *
******************************************************************** */

fn npc_steal(db: &DB, ch: &Rc<CharData>, victim: &Rc<CharData>) {
    if victim.is_npc() {
        return;
    }

    if victim.get_level() >= LVL_IMMORT as u8 {
        return;
    }
    if !db.can_see(ch, victim) {
        return;
    }

    if victim.awake() && rand_number(0, ch.get_level() as u32) == 0 {
        db.act(
            "You discover that $n has $s hands in your wallet.",
            false,
            Some(ch),
            None,
            Some(victim),
            TO_VICT,
        );
        db.act(
            "$n tries to steal gold from $N.",
            true,
            Some(ch),
            None,
            Some(victim),
            TO_NOTVICT,
        );
    } else {
        /* Steal some gold coins */
        let gold = victim.get_gold() * rand_number(1, 10) as i32 / 100;
        if gold > 0 {
            ch.set_gold(ch.get_gold() + gold);
            victim.set_gold(ch.get_gold() - gold);
        }
    }
}

/*
 * Quite lethal to low-level characters.
 */
#[allow(unused_variables)]
pub fn snake(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    if cmd != 0 || ch.get_pos() != POS_FIGHTING || ch.fighting().is_none() {
        return false;
    }

    if ch.fighting().as_ref().unwrap().in_room() != ch.in_room()
        || rand_number(0, ch.get_level() as u32) != 0
    {
        return false;
    }
    let db = &game.db;
    db.act(
        "$n bites $N!",
        true,
        Some(ch),
        None,
        Some(ch.fighting().as_ref().unwrap()),
        TO_NOTVICT,
    );
    db.act(
        "$n bites you!",
        true,
        Some(ch),
        None,
        Some(ch.fighting().as_ref().unwrap()),
        TO_VICT,
    );
    call_magic(
        game,
        ch,
        ch.fighting().as_ref(),
        None,
        SPELL_POISON,
        ch.get_level() as i32,
        CAST_SPELL,
    );
    return true;
}

#[allow(unused_variables)]
pub fn thief(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    if cmd != 0 || ch.get_pos() != POS_STANDING {
        return false;
    }
    let db = &game.db;
    for cons in db.world.borrow()[ch.in_room() as usize]
        .peoples
        .borrow()
        .iter()
    {
        if !cons.is_npc() && cons.get_level() < LVL_IMMORT as u8 && rand_number(0, 4) == 0 {
            npc_steal(db, ch, cons);
            return true;
        }
    }
    return false;
}

#[allow(unused_variables)]
pub fn magic_user(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    if cmd != 0 || ch.get_pos() != POS_FIGHTING {
        return false;
    }
    let db = &game.db;
    /* pseudo-randomly choose someone in the room who is fighting me */
    let mut vict = None;
    let w = db.world.borrow();
    let peoples = w[ch.in_room() as usize].peoples.borrow();
    for v in peoples.iter() {
        if v.fighting().is_some()
            && Rc::ptr_eq(v.fighting().as_ref().unwrap(), ch)
            && rand_number(0, 4) == 0
        {
            vict = Some(v);
            break;
        }
    }

    let mut my_vict = None;
    /* if I didn't pick any of those, then just slam the guy I'm fighting */
    if vict.is_none() && ch.fighting().as_ref().unwrap().in_room() == ch.in_room() {
        my_vict = ch.fighting().clone();
    }
    if my_vict.is_some() {
        vict = my_vict.as_ref();
    }

    /* Hm...didn't pick anyone...I'll wait a round. */
    if vict.is_none() {
        return true;
    }

    if ch.get_level() > 13 && rand_number(0, 10) == 0 {
        cast_spell(game, ch, vict, None, SPELL_POISON);
    }

    if ch.get_level() > 7 && rand_number(0, 8) == 0 {
        cast_spell(game, ch, vict, None, SPELL_BLINDNESS);
    }

    if ch.get_level() > 12 && rand_number(0, 12) == 0 {
        if ch.is_evil() {
            cast_spell(game, ch, vict, None, SPELL_ENERGY_DRAIN);
        } else if ch.is_good() {
            cast_spell(game, ch, vict, None, SPELL_DISPEL_EVIL);
        }
    }

    if rand_number(0, 4) != 0 {
        return true;
    }

    match ch.get_level() {
        4 | 5 => {
            cast_spell(game, ch, vict, None, SPELL_MAGIC_MISSILE);
        }
        6 | 7 => {
            cast_spell(game, ch, vict, None, SPELL_CHILL_TOUCH);
        }
        8 | 9 => {
            cast_spell(game, ch, vict, None, SPELL_BURNING_HANDS);
        }
        10 | 11 => {
            cast_spell(game, ch, vict, None, SPELL_SHOCKING_GRASP);
        }
        12 | 13 => {
            cast_spell(game, ch, vict, None, SPELL_LIGHTNING_BOLT);
        }
        14 | 15 | 16 | 17 => {
            cast_spell(game, ch, vict, None, SPELL_COLOR_SPRAY);
        }
        _ => {
            cast_spell(game, ch, vict, None, SPELL_FIREBALL);
        }
    }
    return true;
}

/* ********************************************************************
*  Special procedures for mobiles                                      *
******************************************************************** */
#[allow(unused_variables)]
pub fn guild_guard(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    let guard = me.downcast_ref::<Rc<CharData>>().unwrap();
    let buf = "The guard humiliates you, and blocks your way.\r\n";
    let buf2 = "The guard humiliates $n, and blocks $s way.";

    if !is_move(cmd) || guard.aff_flagged(AFF_BLIND) {
        return false;
    }

    if ch.get_level() >= LVL_IMMORT as u8 {
        return false;
    }

    for gi in GUILD_INFO {
        if gi.guild_room == NOWHERE {
            break;
        }
        /* Wrong guild or not trying to enter. */
        if game.db.get_room_vnum(ch.in_room()) != gi.guild_room || cmd != gi.direction {
            continue;
        }
        /* Allow the people of the guild through. */
        if !ch.is_npc() && ch.get_class() == gi.pc_class {
            continue;
        }
        send_to_char(ch, buf);
        game.db.act(buf2, false, Some(ch), None, None, TO_ROOM);

        return true;
    }
    false
}

#[allow(unused_variables)]
pub fn puff(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    if cmd != 0 {
        return false;
    }

    return match rand_number(0, 60) {
        0 => {
            do_say(game, ch, "My god!  It's full of stars!", 0, 0);
            true
        }
        1 => {
            do_say(game, ch, "How'd all those fish get up here?", 0, 0);
            true
        }
        2 => {
            do_say(game, ch, "I'm a very female dragon.", 0, 0);
            true
        }
        3 => {
            do_say(game, ch, "I've got a peaceful, easy feeling.", 0, 0);
            true
        }
        _ => false,
    };
}

#[allow(unused_variables)]
pub fn fido(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    if cmd != 0 || !ch.awake() {
        return false;
    }

    for i in game.db.world.borrow()[ch.in_room() as usize]
        .contents
        .borrow()
        .iter()
    {
        if !i.is_corpse() {
            continue;
        }

        game.db.act(
            "$n savagely devours a corpse.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        for temp in clone_vec(&i.contains).iter() {
            DB::obj_from_obj(temp);
            game.db.obj_to_room(Some(temp), ch.in_room());
        }
        game.db.extract_obj(i);
        return true;
    }

    return false;
}

#[allow(unused_variables)]
pub fn janitor(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    if cmd != 0 || !ch.awake() {
        return false;
    }
    let db = &game.db;
    for i in clone_vec(&db.world.borrow()[ch.in_room() as usize].contents).iter() {
        if !i.can_wear(ITEM_WEAR_TAKE) {
            continue;
        }
        if i.get_obj_type() != ITEM_DRINKCON && i.get_obj_cost() >= 15 {
            continue;
        }
        db.act(
            "$n picks up some trash.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        db.obj_from_room(Some(i));
        DB::obj_to_char(Some(i), Some(ch));
        return true;
    }

    return false;
}

#[allow(unused_variables)]
pub fn cityguard(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    if cmd != 0 || !ch.awake() || ch.fighting().is_some() {
        return false;
    }

    let mut max_evil = 1000;
    let mut min_cha = 6;
    let mut spittle = None;
    let mut evil = None;
    let db = &game.db;
    let w = db.world.borrow();
    let peoples = w[ch.in_room() as usize].peoples.borrow();
    for tch in peoples.iter() {
        if !db.can_see(ch, tch) {
            continue;
        }

        if !tch.is_npc() && tch.plr_flagged(PLR_KILLER) {
            db.act(
                "$n screams 'HEY!!!  You're one of those PLAYER KILLERS!!!!!!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            db.hit(ch, tch, TYPE_UNDEFINED, game);
            return true;
        }

        if !tch.is_npc() && tch.plr_flagged(PLR_THIEF) {
            db.act(
                "$n screams 'HEY!!!  You're one of those PLAYER THIEVES!!!!!!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            db.hit(ch, tch, TYPE_UNDEFINED, game);
            return true;
        }

        if tch.fighting().is_some()
            && tch.get_alignment() < max_evil
            && (tch.is_npc() || tch.fighting().as_ref().unwrap().is_npc())
        {
            max_evil = tch.get_alignment();
            evil = Some(tch);
        }

        if tch.get_cha() < min_cha {
            spittle = Some(tch);
            min_cha = tch.get_cha();
        }
    }

    if evil.is_some()
        && evil
            .as_ref()
            .unwrap()
            .fighting()
            .as_ref()
            .unwrap()
            .get_alignment()
            >= 0
    {
        db.act(
            "$n screams 'PROTECT THE INNOCENT!  BANZAI!  CHARGE!  ARARARAGGGHH!'",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        db.hit(ch, evil.as_ref().unwrap(), TYPE_UNDEFINED, game);
        return true;
    }

    /* Reward the socially inept. */
    if spittle.is_some() && rand_number(0, 9) == 0 {
        let spit_social = find_command("spit");

        if spit_social.is_some() {
            let spit_social = spit_social.unwrap();

            do_action(
                game,
                ch,
                &spittle.as_ref().unwrap().get_name(),
                spit_social,
                0,
            );
            return true;
        }
    }

    return false;
}

// #define PET_PRICE(pet) (GET_LEVEL(pet) * 300)
//
// SPECIAL(pet_shops)
// {
// char buf[MAX_STRING_LENGTH], pet_name[256];
// room_rnum pet_room;
// struct char_data *pet;
//
// /* Gross. */
// pet_room = IN_ROOM(ch) + 1;
//
// if (CMD_IS("list")) {
// send_to_char(ch, "Available pets are:\r\n");
// for (pet = world[pet_room].people; pet; pet = pet->next_in_room) {
// /* No, you can't have the Implementor as a pet if he's in there. */
// if (!IS_NPC(pet))
// continue;
// send_to_char(ch, "%8d - %s\r\n", PET_PRICE(pet), GET_NAME(pet));
// }
// return (true);
// } else if (CMD_IS("buy")) {
//
// two_arguments(argument, buf, pet_name);
//
// if (!(pet = get_char_room(buf, None, pet_room)) || !IS_NPC(pet)) {
// send_to_char(ch, "There is no such pet!\r\n");
// return (true);
// }
// if (GET_GOLD(ch) < PET_PRICE(pet)) {
// send_to_char(ch, "You don't have enough gold!\r\n");
// return (true);
// }
// GET_GOLD(ch) -= PET_PRICE(pet);
//
// pet = read_mobile(GET_MOB_RNUM(pet), REAL);
// GET_EXP(pet) = 0;
// SET_BIT(AFF_FLAGS(pet), AFF_CHARM);
//
// if (*pet_name) {
// snprintf(buf, sizeof(buf), "%s %s", pet->player.name, pet_name);
// /* free(pet->player.name); don't free the prototype! */
// pet->player.name = strdup(buf);
//
// snprintf(buf, sizeof(buf), "%sA small sign on a chain around the neck says 'My name is %s'\r\n",
// pet->player.description, pet_name);
// /* free(pet->player.description); don't free the prototype! */
// pet->player.description = strdup(buf);
// }
// char_to_room(pet, IN_ROOM(ch));
// add_follower(pet, ch);
//
// /* Be certain that pets can't get/carry/use/wield/wear items */
// IS_CARRYING_W(pet) = 1000;
// IS_CARRYING_N(pet) = 100;
//
// send_to_char(ch, "May you enjoy your pet.\r\n");
// act("$n buys $N as a pet.", false, ch, 0, pet, TO_ROOM);
//
// return (true);
// }
//
// /* All commands except list and buy */
// return (false);
// }
//
//
//
// /* ********************************************************************
// *  Special procedures for objects                                     *
// ******************************************************************** */
//
//
// SPECIAL(bank)
// {
// int amount;
//
// if (CMD_IS("balance")) {
// if (GET_BANK_GOLD(ch) > 0)
// send_to_char(ch, "Your current balance is %d coins.\r\n", GET_BANK_GOLD(ch));
// else
// send_to_char(ch, "You currently have no money deposited.\r\n");
// return (true);
// } else if (CMD_IS("deposit")) {
// if ((amount = atoi(argument)) <= 0) {
// send_to_char(ch, "How much do you want to deposit?\r\n");
// return (true);
// }
// if (GET_GOLD(ch) < amount) {
// send_to_char(ch, "You don't have that many coins!\r\n");
// return (true);
// }
// GET_GOLD(ch) -= amount;
// GET_BANK_GOLD(ch) += amount;
// send_to_char(ch, "You deposit %d coins.\r\n", amount);
// act("$n makes a bank transaction.", true, ch, 0, false, TO_ROOM);
// return (true);
// } else if (CMD_IS("withdraw")) {
// if ((amount = atoi(argument)) <= 0) {
// send_to_char(ch, "How much do you want to withdraw?\r\n");
// return (true);
// }
// if (GET_BANK_GOLD(ch) < amount) {
// send_to_char(ch, "You don't have that many coins deposited!\r\n");
// return (true);
// }
// GET_GOLD(ch) += amount;
// GET_BANK_GOLD(ch) -= amount;
// send_to_char(ch, "You withdraw %d coins.\r\n", amount);
// act("$n makes a bank transaction.", true, ch, 0, false, TO_ROOM);
// return (true);
// } else
// return (false);
// }
//
