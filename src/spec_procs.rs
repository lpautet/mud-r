/* ************************************************************************
*   File: spec_procs.rs                                 Part of CircleMUD *
*  Usage: implementation of special procedures for mobiles/objects/rooms  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::any::Any;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;

use crate::act_comm::do_say;
use crate::act_item::do_drop;
use crate::act_movement::{do_gen_door, perform_move};
use crate::act_social::do_action;
use crate::class::{GUILD_INFO, PRAC_PARAMS};
use crate::constants::INT_APP;
use crate::db::{DB, REAL};
use crate::interpreter::{
    cmd_is, find_command, is_move, two_arguments, SCMD_CLOSE, SCMD_DROP, SCMD_LOCK, SCMD_OPEN,
    SCMD_UNLOCK,
};
use crate::limits::gain_exp;
use crate::modify::page_string;
use crate::spell_parser::{call_magic, cast_spell, find_skill_num};
use crate::spells::{
    CAST_SPELL, SPELL_BLINDNESS, SPELL_BURNING_HANDS, SPELL_CHILL_TOUCH, SPELL_COLOR_SPRAY,
    SPELL_DISPEL_EVIL, SPELL_ENERGY_DRAIN, SPELL_FIREBALL, SPELL_LIGHTNING_BOLT,
    SPELL_MAGIC_MISSILE, SPELL_POISON, SPELL_SHOCKING_GRASP, TYPE_UNDEFINED,
};
use crate::structs::{
    CharData, AFF_BLIND, AFF_CHARM, ITEM_DRINKCON, ITEM_WEAR_TAKE, LVL_IMMORT, MAX_SKILLS, NOWHERE,
    PLR_KILLER, PLR_THIEF, POS_FIGHTING, POS_SLEEPING, POS_STANDING,
};
use crate::util::{add_follower, clone_vec, clone_vec2, rand_number};
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

    for sortpos in 1..(MAX_SKILLS + 1) {
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

    page_string(ch.desc.borrow().as_ref().unwrap(), &buf, true);
}

pub fn guild(game: &mut Game, ch: &Rc<CharData>, _me: &dyn Any, cmd: i32, argument: &str) -> bool {
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

pub fn dump(game: &mut Game, ch: &Rc<CharData>, _me: &dyn Any, cmd: i32, argument: &str) -> bool {
    let list = clone_vec2(&game.db.world[ch.in_room() as usize].contents);
    for k in &list {
        game.db.act(
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
    let list = clone_vec2(&game.db.world[ch.in_room() as usize].contents);
    for k in &list {
        game.db.act(
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
        game.db.act(
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

pub struct Mayor {
    pub path: &'static str,
    pub path_index: usize,
    pub move_: bool,
}

impl Mayor {
    pub fn new() -> Mayor {
        Mayor {
            path: "",
            path_index: 0,
            move_: false,
        }
    }
}

const OPEN_PATH: &str = "W3a3003b33000c111d0d111Oe333333Oe22c222112212111a1S.";
const CLOSE_PATH: &str = "W3a3003b33000c111d0d111CE333333CE22c222112212111a1S.";

pub fn mayor(game: &mut Game, ch: &Rc<CharData>, _me: &dyn Any, cmd: i32, _argument: &str) -> bool {
    let db = &game.db;

    if !game.db.mayor.borrow().move_ {
        if db.time_info.hours == 6 {
            game.db.mayor.borrow_mut().move_ = true;
            game.db.mayor.borrow_mut().path = OPEN_PATH;
            game.db.mayor.borrow_mut().path_index = 0;
        } else if db.time_info.hours == 20 {
            game.db.mayor.borrow_mut().move_ = true;
            game.db.mayor.borrow_mut().path = CLOSE_PATH;
            game.db.mayor.borrow_mut().path_index = 0;
        }
    }
    if cmd != 0
        || !game.db.mayor.borrow().move_
        || ch.get_pos() < POS_SLEEPING
        || ch.get_pos() == POS_FIGHTING
    {
        return false;
    }

    let a = &game.db.mayor.borrow().path
        [game.db.mayor.borrow().path_index..game.db.mayor.borrow().path_index + 1]
        .chars()
        .next()
        .unwrap();
    match a {
        '0' | '1' | '2' | '3' => {
            let dir = game.db.mayor.borrow().path
                [game.db.mayor.borrow().path_index..game.db.mayor.borrow().path_index + 1]
                .parse::<u8>()
                .unwrap();
            perform_move(game, ch, dir as i32, true);
        }

        'W' => {
            ch.set_pos(POS_STANDING);
            db.act(
                "$n awakens and groans loudly.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'S' => {
            ch.set_pos(POS_SLEEPING);
            db.act(
                "$n lies down and instantly falls asleep.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'a' => {
            db.act(
                "$n says 'Hello Honey!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            db.act("$n smirks.", false, Some(ch), None, None, TO_ROOM);
        }

        'b' => {
            db.act(
                "$n says 'What a view!  I must get something done about that dump!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'c' => {
            db.act(
                "$n says 'Vandals!  Youngsters nowadays have no respect for anything!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'd' => {
            db.act(
                "$n says 'Good day, citizens!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'e' => {
            db.act(
                "$n says 'I hereby declare the bazaar open!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'E' => {
            db.act(
                "$n says 'I hereby declare Midgaard closed!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'O' => {
            do_gen_door(game, ch, "gate", 0, SCMD_UNLOCK);
            do_gen_door(game, ch, "gate", 0, SCMD_OPEN);
        }

        'C' => {
            do_gen_door(game, ch, "gate", 0, SCMD_CLOSE);
            do_gen_door(game, ch, "gate", 0, SCMD_LOCK);
        }

        '.' => {
            game.db.mayor.borrow_mut().move_ = false;
        }
        _ => {}
    }

    game.db.mayor.borrow_mut().path_index += 1;
    return false;
}

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
pub fn snake(game: &mut Game, ch: &Rc<CharData>, _me: &dyn Any, cmd: i32, _argument: &str) -> bool {
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

pub fn thief(game: &mut Game, ch: &Rc<CharData>, _me: &dyn Any, cmd: i32, _argument: &str) -> bool {
    if cmd != 0 || ch.get_pos() != POS_STANDING {
        return false;
    }
    let db = &game.db;
    for cons in db.world[ch.in_room() as usize]
        .peoples
        .iter()
    {
        if !cons.is_npc() && cons.get_level() < LVL_IMMORT as u8 && rand_number(0, 4) == 0 {
            npc_steal(db, ch, cons);
            return true;
        }
    }
    return false;
}

pub fn magic_user(
    game: &mut Game,
    ch: &Rc<CharData>,
    _me: &dyn Any,
    cmd: i32,
    _argument: &str,
) -> bool {
    if cmd != 0 || ch.get_pos() != POS_FIGHTING {
        return false;
    }
    /* pseudo-randomly choose someone in the room who is fighting me */
    let mut vict = None;
    {
        let peoples = &game.db.world[ch.in_room() as usize].peoples;
        for v in peoples.iter() {
            if v.fighting().is_some()
                && Rc::ptr_eq(v.fighting().as_ref().unwrap(), ch)
                && rand_number(0, 4) == 0
            {
                vict = Some(v.clone());
                break;
            }
        }
    }

    let mut my_vict = None;
    /* if I didn't pick any of those, then just slam the guy I'm fighting */
    if vict.is_none() && ch.fighting().as_ref().unwrap().in_room() == ch.in_room() {
        my_vict = ch.fighting().clone();
    }
    if my_vict.is_some() {
        vict = my_vict;
    }

    /* Hm...didn't pick anyone...I'll wait a round. */
    if vict.is_none() {
        return true;
    }

    if ch.get_level() > 13 && rand_number(0, 10) == 0 {
        cast_spell(game, ch, vict.as_ref(), None, SPELL_POISON);
    }

    if ch.get_level() > 7 && rand_number(0, 8) == 0 {
        cast_spell(game, ch, vict.as_ref(), None, SPELL_BLINDNESS);
    }

    if ch.get_level() > 12 && rand_number(0, 12) == 0 {
        if ch.is_evil() {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_ENERGY_DRAIN);
        } else if ch.is_good() {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_DISPEL_EVIL);
        }
    }

    if rand_number(0, 4) != 0 {
        return true;
    }

    match ch.get_level() {
        4 | 5 => {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_MAGIC_MISSILE);
        }
        6 | 7 => {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_CHILL_TOUCH);
        }
        8 | 9 => {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_BURNING_HANDS);
        }
        10 | 11 => {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_SHOCKING_GRASP);
        }
        12 | 13 => {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_LIGHTNING_BOLT);
        }
        14 | 15 | 16 | 17 => {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_COLOR_SPRAY);
        }
        _ => {
            cast_spell(game, ch, vict.as_ref(), None, SPELL_FIREBALL);
        }
    }
    return true;
}

/* ********************************************************************
*  Special procedures for mobiles                                      *
******************************************************************** */
pub fn guild_guard(
    game: &mut Game,
    ch: &Rc<CharData>,
    me: &dyn Any,
    cmd: i32,
    _argument: &str,
) -> bool {
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

pub fn puff(game: &mut Game, ch: &Rc<CharData>, _me: &dyn Any, cmd: i32, _argument: &str) -> bool {
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

pub fn fido(game: &mut Game, ch: &Rc<CharData>, _me: &dyn Any, cmd: i32, _argument: &str) -> bool {
    if cmd != 0 || !ch.awake() {
        return false;
    }

    let list = clone_vec2(&game.db.world[ch.in_room() as usize].contents);
    for i in &list
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
            game.db.obj_to_room(temp, ch.in_room());
        }
        game.db.extract_obj(&i);
        return true;
    }

    return false;
}

pub fn janitor(
    game: &mut Game,
    ch: &Rc<CharData>,
    _me: &dyn Any,
    cmd: i32,
    _argument: &str,
) -> bool {
    if cmd != 0 || !ch.awake() {
        return false;
    }
    for i in clone_vec2(&game.db.world[ch.in_room() as usize].contents).iter() {
        if !i.can_wear(ITEM_WEAR_TAKE) {
            continue;
        }
        if i.get_obj_type() != ITEM_DRINKCON && i.get_obj_cost() >= 15 {
            continue;
        }
        game.db.act(
            "$n picks up some trash.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        game.db.obj_from_room(i);
        DB::obj_to_char(i, ch);
        return true;
    }

    return false;
}

pub fn cityguard(
    game: &mut Game,
    ch: &Rc<CharData>,
    _me: &dyn Any,
    cmd: i32,
    _argument: &str,
) -> bool {
    if cmd != 0 || !ch.awake() || ch.fighting().is_some() {
        return false;
    }

    let mut max_evil = 1000;
    let mut min_cha = 6;
    let mut spittle = None;
    let mut evil = None;
    let peoples = clone_vec2(&game.db.world[ch.in_room() as usize].peoples);
    for tch in peoples.iter() {
        if !game.db.can_see(ch, tch) {
            continue;
        }

        if !tch.is_npc() && tch.plr_flagged(PLR_KILLER) {
            game.db.act(
                "$n screams 'HEY!!!  You're one of those PLAYER KILLERS!!!!!!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            game.hit(ch, tch, TYPE_UNDEFINED);
            return true;
        }

        if !tch.is_npc() && tch.plr_flagged(PLR_THIEF) {
            game.db.act(
                "$n screams 'HEY!!!  You're one of those PLAYER THIEVES!!!!!!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            game.hit(ch, tch, TYPE_UNDEFINED);
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
        game.db.act(
            "$n screams 'PROTECT THE INNOCENT!  BANZAI!  CHARGE!  ARARARAGGGHH!'",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        game.hit(ch, evil.as_ref().unwrap(), TYPE_UNDEFINED);
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

fn pet_price(pet: &Rc<CharData>) -> i32 {
    pet.get_level() as i32 * 300
}

pub fn pet_shops(
    game: &mut Game,
    ch: &Rc<CharData>,
    _me: &dyn Any,
    cmd: i32,
    argument: &str,
) -> bool {
    /* Gross. */
    let pet_room = ch.in_room() + 1;

    if cmd_is(cmd, "list") {
        send_to_char(ch, "Available pets are:\r\n");
        for pet in game.db.world[pet_room as usize].peoples.iter() {
            /* No, you can't have the Implementor as a pet if he's in there. */
            if !pet.is_npc() {
                continue;
            }
            send_to_char(
                ch,
                format!("{:8} - {}\r\n", pet_price(pet), pet.get_name()).as_str(),
            );
        }
        return true;
    } else if cmd_is(cmd, "buy") {
        let mut buf = String::new();
        let mut pet_name = String::new();
        two_arguments(argument, &mut buf, &mut pet_name);
        let pet = game.db.get_char_room(&buf, None, pet_room);
        if pet.is_none() || !pet.as_ref().unwrap().is_npc() {
            send_to_char(ch, "There is no such pet!\r\n");
            return true;
        }
        let pet = pet.as_ref().unwrap();
        if ch.get_gold() < pet_price(pet) {
            send_to_char(ch, "You don't have enough gold!\r\n");
            return true;
        }
        ch.set_gold(ch.get_gold() - pet_price(pet));

        let pet = game.db.read_mobile(pet.get_mob_rnum(), REAL).unwrap();
        pet.set_exp(0);
        pet.set_aff_flags_bits(AFF_CHARM);

        if !pet_name.is_empty() {
            let buf = format!("{} {}", pet.player.borrow().name, pet_name);

            pet.player.borrow_mut().name = buf;

            let buf = format!(
                "{}A small sign on a chain around the neck says 'My name is {}'\r\n",
                RefCell::borrow(&pet.player.borrow().description),
                pet_name
            );
            /* free(pet->player.description); don't free the prototype! */
            *RefCell::borrow_mut(&pet.player.borrow().description) = buf;
        }
        game.db.char_to_room(&pet, ch.in_room());
        add_follower(&game.db, &pet, ch);

        /* Be certain that pets can't get/carry/use/wield/wear items */
        pet.set_is_carrying_w(1000);
        pet.set_is_carrying_n(100);

        send_to_char(ch, "May you enjoy your pet.\r\n");
        game.db.act(
            "$n buys $N as a pet.",
            false,
            Some(ch),
            None,
            Some(&pet),
            TO_ROOM,
        );

        return true;
    }

    /* All commands except list and buy */
    return false;
}

/* ********************************************************************
*  Special procedures for objects                                     *
******************************************************************** */

pub fn bank(game: &mut Game, ch: &Rc<CharData>, _me: &dyn Any, cmd: i32, argument: &str) -> bool {
    let db = &game.db;
    return if cmd_is(cmd, "balance") {
        if ch.get_bank_gold() > 0 {
            send_to_char(
                ch,
                format!("Your current balance is {} coins.\r\n", ch.get_bank_gold()).as_str(),
            );
        } else {
            send_to_char(ch, "You currently have no money deposited.\r\n");
        }
        true
    } else if cmd_is(cmd, "deposit") {
        let amount = argument.trim_start().parse::<i32>();
        let amount = if amount.is_ok() { amount.unwrap() } else { -1 };
        if amount <= 0 {
            send_to_char(ch, "How much do you want to deposit?\r\n");
            return true;
        }
        if ch.get_gold() < amount {
            send_to_char(ch, "You don't have that many coins!\r\n");
            return true;
        }
        ch.set_gold(ch.get_gold() - amount);
        ch.set_bank_gold(ch.get_bank_gold() + amount);
        send_to_char(ch, format!("You deposit {} coins.\r\n", amount).as_str());
        db.act(
            "$n makes a bank transaction.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        true
    } else if cmd_is(cmd, "withdraw") {
        let amount = argument.trim_start().parse::<i32>();
        let amount = if amount.is_ok() { amount.unwrap() } else { -1 };
        if amount <= 0 {
            send_to_char(ch, "How much do you want to withdraw?\r\n");
            return true;
        }
        if ch.get_bank_gold() < amount {
            send_to_char(ch, "You don't have that many coins deposited!\r\n");
            return true;
        }
        ch.set_gold(ch.get_gold() + amount);
        ch.set_bank_gold(ch.get_bank_gold() - amount);
        send_to_char(ch, format!("You withdraw {} coins.\r\n", amount).as_str());
        db.act(
            "$n makes a bank transaction.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        true
    } else {
        false
    }
}
