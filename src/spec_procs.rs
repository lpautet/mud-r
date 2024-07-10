/* ************************************************************************
*   File: spec_procs.rs                                 Part of CircleMUD *
*  Usage: implementation of special procedures for mobiles/objects/rooms  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;
use crate::depot::DepotId;
use crate::VictimRef;

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
    MeRef, CharData, AFF_BLIND, AFF_CHARM, ITEM_DRINKCON, ITEM_WEAR_TAKE, LVL_IMMORT, MAX_SKILLS, NOWHERE,
    PLR_KILLER, PLR_THIEF, POS_FIGHTING, POS_SLEEPING, POS_STANDING,
};
use crate::util::{add_follower, rand_number};
use crate::{ Game, TO_NOTVICT, TO_ROOM, TO_VICT};

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

fn learned(ch: &CharData) -> i8 {
    PRAC_PARAMS[LEARNED_LEVEL][ch.get_class() as usize] as i8
}

fn mingain(ch: &CharData) -> i32 {
    PRAC_PARAMS[MIN_PER_PRAC][ch.get_class() as usize]
}

fn maxgain(ch: &CharData) -> i32 {
    PRAC_PARAMS[MAX_PER_PRAC][ch.get_class() as usize]
}

fn splskl(ch: &CharData) -> &str {
    PRAC_TYPES[PRAC_PARAMS[PRAC_TYPE][ch.get_class() as usize] as usize]
}

pub fn list_skills(game: &mut Game, db: &mut DB, chid: DepotId) {
    let ch = db.ch(chid);
    if ch.get_practices() == 0 {
        game.send_to_char(ch, "You have no practice sessions remaining.\r\n");
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
    let d_id = ch.desc.unwrap();
    page_string(game, db, d_id , &buf, true);
}

pub fn guild(game: &mut Game, db: &mut DB, chid: DepotId, _me: MeRef, cmd: i32, argument: &str) -> bool {
    let ch = db.ch(chid);
    if ch.is_npc() || !cmd_is(cmd, "practice") {
        return false;
    }

    let argument = argument.trim();

    if argument.is_empty() {
        list_skills(game, db, chid);
        return true;
    }

    if ch.get_practices() <= 0 {
        game.send_to_char(ch, "You do not seem to be able to practice now.\r\n");
        return true;
    }

    let skill_num = find_skill_num(db, argument);

    if skill_num.is_none()
        || ch.get_level()
            < db.spell_info[skill_num.unwrap() as usize].min_level[ch.get_class() as usize] as u8
    {
        game.send_to_char(ch,
            format!("You do not know of that {}.\r\n", splskl(ch)).as_str(),
        );
        return true;
    }
    if ch.get_skill(skill_num.unwrap()) >= learned(ch) {
        game.send_to_char(ch, "You are already learned in that area.\r\n");
        return true;
    }
    game.send_to_char(ch, "You practice for a while...\r\n");
    let ch = db.ch_mut(chid);
    ch.set_practices(ch.get_practices() - 1);

    let mut percent = ch.get_skill(skill_num.unwrap());
    percent += min(
        maxgain(ch),
        max(mingain(ch), INT_APP[ch.get_int() as usize].learn as i32),
    ) as i8;

    ch.set_skill(skill_num.unwrap(), min(learned(ch), percent));

    if ch.get_skill(skill_num.unwrap()) >= learned(ch) {
        game.send_to_char(ch, "You are now learned in that area.\r\n");
    }

    true
}

pub fn dump(game: &mut Game, db: &mut DB, chid: DepotId, _me: MeRef, cmd: i32, argument: &str) -> bool {
    let ch = db.ch(chid);

    for k_id in db.world[ch.in_room() as usize].contents.clone() {
        let k = db.obj(k_id);
        game.act(db,
            "$p vanishes in a puff of smoke!",
            false,
            None,
            Some(k),
            None,
            TO_ROOM,
        );
        db.extract_obj(k_id);
    }

    if !cmd_is(cmd, "drop") {
        return false;
    }

    do_drop(game, db,chid, argument, cmd as usize, SCMD_DROP as i32);
    let mut value = 0;
    let ch = db.ch(chid);
    for k_id in db.world[ch.in_room() as usize].contents.clone() {
        let k = db.obj(k_id);
        game.act(db,
            "$p vanishes in a puff of smoke!",
            false,
            None,
            Some(k),
            None,
            TO_ROOM,
        );
        value += max(1, min(50, k.get_obj_cost() / 10));
        db.extract_obj(k_id);
    }
    let ch = db.ch(chid);
    if value != 0 {
        game.send_to_char(ch, "You are awarded for outstanding performance.\r\n");
        game.act(db,
            "$n has been awarded for being a good citizen.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let ch = db.ch(chid);
        if ch.get_level() < 3 {
            gain_exp(chid, value, game,db);
        } else {
            let ch = db.ch_mut(chid);
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

pub fn mayor(game: &mut Game, db: &mut DB, chid: DepotId, _me: MeRef, cmd: i32, _argument: &str) -> bool {
    if !db.mayor.move_ {
        if db.time_info.hours == 6 {
            db.mayor.move_ = true;
            db.mayor.path = OPEN_PATH;
            db.mayor.path_index = 0;
        } else if db.time_info.hours == 20 {
            db.mayor.move_ = true;
            db.mayor.path = CLOSE_PATH;
            db.mayor.path_index = 0;
        }
    }
    let ch = db.ch(chid);
    if cmd != 0
        || !db.mayor.move_
        || ch.get_pos() < POS_SLEEPING
        || ch.get_pos() == POS_FIGHTING
    {
        return false;
    }

    let a = &db.mayor.path
        [db.mayor.path_index..db.mayor.path_index + 1]
        .chars()
        .next()
        .unwrap();
    match a {
        '0' | '1' | '2' | '3' => {
            let dir = db.mayor.path
                [db.mayor.path_index..db.mayor.path_index + 1]
                .parse::<u8>()
                .unwrap();
            perform_move(game, db, chid, dir as i32, true);
        }

        'W' => {
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_STANDING);
            let ch = db.ch(chid);
            game.act(db,
                "$n awakens and groans loudly.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'S' => {
            let ch = db.ch_mut(chid);
            ch.set_pos(POS_SLEEPING);
            let ch = db.ch(chid);
            game.act(db,
                "$n lies down and instantly falls asleep.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'a' => {
            game.act(db,
                "$n says 'Hello Honey!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            game.act(db,"$n smirks.", false, Some(ch), None, None, TO_ROOM);
        }

        'b' => {
            game.act(db,
                "$n says 'What a view!  I must get something done about that dump!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'c' => {
            game.act(db,
                "$n says 'Vandals!  Youngsters nowadays have no respect for anything!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'd' => {
            game.act(db,
                "$n says 'Good day, citizens!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'e' => {
            game.act(db,
                "$n says 'I hereby declare the bazaar open!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'E' => {
            game.act(db,
                "$n says 'I hereby declare Midgaard closed!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'O' => {
            do_gen_door(game, db, chid, "gate", 0, SCMD_UNLOCK);
            do_gen_door(game, db, chid, "gate", 0, SCMD_OPEN);
        }

        'C' => {
            do_gen_door(game, db, chid, "gate", 0, SCMD_CLOSE);
            do_gen_door(game, db, chid, "gate", 0, SCMD_LOCK);
        }

        '.' => {
            db.mayor.move_ = false;
        }
        _ => {}
    }

    db.mayor.path_index += 1;
    return false;
}

/* ********************************************************************
*  General special procedures for mobiles                             *
******************************************************************** */

fn npc_steal(game: &mut Game, db: &mut DB, chid: DepotId, victim_id: DepotId) {
    let victim = db.ch(victim_id);
    let ch = db.ch(chid);

    if victim.is_npc() {
        return;
    }

    if victim.get_level() >= LVL_IMMORT as u8 {
        return;
    }
    if !game.can_see(db,ch, victim) {
        return;
    }

    if victim.awake() && rand_number(0, ch.get_level() as u32) == 0 {
        game.act(db,
            "You discover that $n has $s hands in your wallet.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(victim)),
            TO_VICT,
        );
        game.act(db,
            "$n tries to steal gold from $N.",
            true,
            Some(ch),
            None,
            Some(VictimRef::Char(victim)),
            TO_NOTVICT,
        );
    } else {
        /* Steal some gold coins */
        let gold = victim.get_gold() * rand_number(1, 10) as i32 / 100;
        if gold > 0 {
            let ch = db.ch_mut(chid);
            ch.set_gold(ch.get_gold() + gold);
            let victim = db.ch_mut(victim_id);
            victim.set_gold(victim.get_gold() - gold);
        }
    }
}

/*
 * Quite lethal to low-level characters.
 */
pub fn snake(game: &mut Game, db: &mut DB, chid: DepotId, _me: MeRef, cmd: i32, _argument: &str) -> bool {
    let ch = db.ch(chid);

    if cmd != 0 || ch.get_pos() != POS_FIGHTING || ch.fighting_id().is_none() {
        return false;
    }

    if db.ch(ch.fighting_id().unwrap()).in_room() != ch.in_room()
        || rand_number(0, ch.get_level() as u32) != 0
    {
        return false;
    }
    let fighting = db.ch(ch.fighting_id().unwrap());
    game.act(db,
        "$n bites $N!",
        true,
        Some(ch),
        None,
        Some(VictimRef::Char(fighting)),
        TO_NOTVICT,
    );
    game.act(db,
        "$n bites you!",
        true,
        Some(ch),
        None,
        Some(VictimRef::Char(fighting)),
        TO_VICT,
    );
    call_magic(
        game,db,
        chid,
        ch.fighting_id(),
        None,
        SPELL_POISON,
        ch.get_level() as i32,
        CAST_SPELL,
    );
    return true;
}

pub fn thief(game: &mut Game, db: &mut DB, chid: DepotId, _me: MeRef, cmd: i32, _argument: &str) -> bool {
    let ch = db.ch(chid);

    if cmd != 0 || ch.get_pos() != POS_STANDING {
        return false;
    }
    for cons_id in db.world[ch.in_room() as usize].peoples.clone() {
        let cons = db.ch(cons_id);
        if !cons.is_npc() && cons.get_level() < LVL_IMMORT as u8 && rand_number(0, 4) == 0 {
            npc_steal(game, db, chid, cons_id);
            return true;
        }
    }
    return false;
}

pub fn magic_user(
    game: &mut Game, db: &mut DB,
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = db.ch(chid);

    if cmd != 0 || ch.get_pos() != POS_FIGHTING {
        return false;
    }
    /* pseudo-randomly choose someone in the room who is fighting me */
    let mut vict_id = None;
    {
        for &v_id in &db.world[ch.in_room() as usize].peoples {
            let v = db.ch(v_id);
            if v.fighting_id().is_some()
                && v.fighting_id().unwrap() == chid
                && rand_number(0, 4) == 0
            {
                vict_id = Some(v_id);
                break;
            }
        }
    }

    let mut my_vict_id = None;
    /* if I didn't pick any of those, then just slam the guy I'm fighting */
    if vict_id.is_none() && db.ch(ch.fighting_id().unwrap()).in_room() == ch.in_room() {
        my_vict_id = ch.fighting_id();
    }
    if my_vict_id.is_some() {
        vict_id = my_vict_id;
    }

    /* Hm...didn't pick anyone...I'll wait a round. */
    if vict_id.is_none() {
        return true;
    }

    if ch.get_level() > 13 && rand_number(0, 10) == 0 {
        cast_spell(game,db, chid, vict_id, None, SPELL_POISON);
    }
    let ch = db.ch(chid);
    if ch.get_level() > 7 && rand_number(0, 8) == 0 {
        cast_spell(game,db, chid, vict_id, None, SPELL_BLINDNESS);
    }
    let ch = db.ch(chid);
    if ch.get_level() > 12 && rand_number(0, 12) == 0 {
        if ch.is_evil() {
            cast_spell(game,db, chid, vict_id, None, SPELL_ENERGY_DRAIN);
        } else if ch.is_good() {
            cast_spell(game,db, chid, vict_id, None, SPELL_DISPEL_EVIL);
        }
    }

    if rand_number(0, 4) != 0 {
        return true;
    }
    let ch = db.ch(chid);
    match ch.get_level() {
        4 | 5 => {
            cast_spell(game,db, chid, vict_id, None, SPELL_MAGIC_MISSILE);
        }
        6 | 7 => {
            cast_spell(game,db, chid, vict_id, None, SPELL_CHILL_TOUCH);
        }
        8 | 9 => {
            cast_spell(game,db, chid, vict_id, None, SPELL_BURNING_HANDS);
        }
        10 | 11 => {
            cast_spell(game,db, chid, vict_id, None, SPELL_SHOCKING_GRASP);
        }
        12 | 13 => {
            cast_spell(game,db, chid, vict_id, None, SPELL_LIGHTNING_BOLT);
        }
        14 | 15 | 16 | 17 => {
            cast_spell(game,db, chid, vict_id, None, SPELL_COLOR_SPRAY);
        }
        _ => {
            cast_spell(game,db, chid, vict_id, None, SPELL_FIREBALL);
        }
    }
    return true;
}

/* ********************************************************************
*  Special procedures for mobiles                                      *
******************************************************************** */
pub fn guild_guard(
    game: &mut Game,db: &mut DB,
    chid: DepotId,
    me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = db.ch(chid);

    let guard_id;
    match me {
        MeRef::Char(me_chid) => { guard_id = me_chid },
        _ => panic!("Unexpected MeRef type in guild_guard"),
    }
    let guard = db.ch(guard_id);
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
        if db.get_room_vnum(ch.in_room()) != gi.guild_room || cmd != gi.direction {
            continue;
        }
        /* Allow the people of the guild through. */
        if !ch.is_npc() && ch.get_class() == gi.pc_class {
            continue;
        }
        game.send_to_char(ch, buf);
        game.act(db,buf2, false, Some(ch), None, None, TO_ROOM);

        return true;
    }
    false
}

pub fn puff(game: &mut Game, db: &mut DB, chid: DepotId, _me: MeRef, cmd: i32, _argument: &str) -> bool {
    if cmd != 0 {
        return false;
    }

    return match rand_number(0, 60) {
        0 => {
            do_say(game, db, chid, "My god!  It's full of stars!", 0, 0);
            true
        }
        1 => {
            do_say(game, db, chid, "How'd all those fish get up here?", 0, 0);
            true
        }
        2 => {
            do_say(game, db,chid, "I'm a very female dragon.", 0, 0);
            true
        }
        3 => {
            do_say(game, db, chid, "I've got a peaceful, easy feeling.", 0, 0);
            true
        }
        _ => false,
    };
}

pub fn fido(game: &mut Game, db: &mut DB, chid: DepotId, _me: MeRef, cmd: i32, _argument: &str) -> bool {
    let ch = db.ch(chid);

    if cmd != 0 || !ch.awake() {
        return false;
    }

    for i in db.world[ch.in_room() as usize].contents.clone() {
        if !db.obj(i).is_corpse() {
            continue;
        }

        game.act(db,
            "$n savagely devours a corpse.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        for temp in db.obj(i).contains.clone().into_iter() {
            db.obj_from_obj(temp);
            let ch = db.ch(chid);
            db.obj_to_room(temp, ch.in_room());
        }
        db.extract_obj(i);
        return true;
    }

    return false;
}

pub fn janitor(
    game: &mut Game, db: &mut DB,
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = db.ch(chid);

    if cmd != 0 || !ch.awake() {
        return false;
    }
    for i in db.world[ch.in_room() as usize].contents.clone().into_iter() { 
        if !db.obj(i).can_wear(ITEM_WEAR_TAKE) {
            continue;
        }
        if db.obj(i).get_obj_type() != ITEM_DRINKCON && db.obj(i).get_obj_cost() >= 15 {
            continue;
        }
        game.act(db,
            "$n picks up some trash.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        db.obj_from_room(i);
        db.obj_to_char(i, chid);
        return true;
    }

    return false;
}

pub fn cityguard(
    game: &mut Game, db: &mut DB,
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = db.ch(chid);

    if cmd != 0 || !ch.awake() || ch.fighting_id().is_some() {
        return false;
    }

    let mut max_evil = 1000;
    let mut min_cha = 6;
    let mut spittle = None;
    let mut evil_id = None;
    for tch_id in db.world[ch.in_room() as usize].peoples.clone() {
        let tch = db.ch(tch_id);
        if !game.can_see(db,ch, tch) {
            continue;
        }

        if !tch.is_npc() && tch.plr_flagged(PLR_KILLER) {
            game.act(db,
                "$n screams 'HEY!!!  You're one of those PLAYER KILLERS!!!!!!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            game.hit(db,chid, tch_id, TYPE_UNDEFINED);
            return true;
        }

        if !tch.is_npc() && tch.plr_flagged(PLR_THIEF) {
            game.act(db,
                "$n screams 'HEY!!!  You're one of those PLAYER THIEVES!!!!!!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            game.hit(db,chid, tch_id, TYPE_UNDEFINED);
            return true;
        }

        if tch.fighting_id().is_some()
            && tch.get_alignment() < max_evil
            && (tch.is_npc() || db.ch(tch.fighting_id().unwrap()).is_npc())
        {
            max_evil = tch.get_alignment();
            evil_id = Some(tch_id);
        }

        if tch.get_cha() < min_cha {
            spittle = Some(tch);
            min_cha = tch.get_cha();
        }
    }

    if evil_id.is_some()
        && db.ch(db.ch(evil_id.unwrap())
            .fighting_id()
            .unwrap())
            .get_alignment()
            >= 0
    {
        game.act(db,
            "$n screams 'PROTECT THE INNOCENT!  BANZAI!  CHARGE!  ARARARAGGGHH!'",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        game.hit(db,chid, evil_id.unwrap(), TYPE_UNDEFINED);
        return true;
    }

    /* Reward the socially inept. */
    if spittle.is_some() && rand_number(0, 9) == 0 {
        let spit_social = find_command("spit");

        if spit_social.is_some() {
            let spit_social = spit_social.unwrap();

            do_action(
                game,db,
                chid,
                &spittle.as_ref().unwrap().get_name().clone(),
                spit_social,
                0,
            );
            return true;
        }
    }

    return false;
}

fn pet_price(pet: &CharData) -> i32 {
    pet.get_level() as i32 * 300
}

pub fn pet_shops(
    game: &mut Game, db: &mut DB,
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    argument: &str,
) -> bool {
    let ch = db.ch(chid);

    /* Gross. */
    let pet_room = ch.in_room() + 1;

    if cmd_is(cmd, "list") {
        game.send_to_char(ch, "Available pets are:\r\n");
        for &pet_id in &db.world[pet_room as usize].peoples {
            let pet = db.ch(pet_id);
            /* No, you can't have the Implementor as a pet if he's in there. */
            if !pet.is_npc() {
                continue;
            }
            game.send_to_char(ch,
                format!("{:8} - {}\r\n", pet_price(pet), pet.get_name()).as_str(),
            );
        }
        return true;
    } else if cmd_is(cmd, "buy") {
        let mut buf = String::new();
        let mut pet_name = String::new();
        two_arguments(argument, &mut buf, &mut pet_name);
        let pet_id = db.get_char_room(&buf, None, pet_room);
        if pet_id.is_none() || !db.ch(pet_id.unwrap()).is_npc() {
            game.send_to_char(ch, "There is no such pet!\r\n");
            return true;
        }
        let pet_id = pet_id.unwrap();
        if ch.get_gold() < pet_price(db.ch(pet_id)) {
            game.send_to_char(ch, "You don't have enough gold!\r\n");
            return true;
        }
        let pet_price = pet_price(db.ch(pet_id));
        let ch = db.ch_mut(chid);
        ch.set_gold(ch.get_gold() - pet_price );

        let pet_id = db.read_mobile(db.ch(pet_id).get_mob_rnum(), REAL).unwrap();
        let pet = db.ch_mut(pet_id);
        pet.set_exp(0);
        pet.set_aff_flags_bits(AFF_CHARM);

        if !pet_name.is_empty() {
            let buf = format!("{} {}", pet.player.name, pet_name);

            db.ch_mut(pet_id).player.name = Rc::from(buf.as_str());
            let pet = db.ch(pet_id);
            let buf = format!(
                "{}A small sign on a chain around the neck says 'My name is {}'\r\n",
                RefCell::borrow(&pet.player.description),
                pet_name
            );
            /* free(pet->player.description); don't free the prototype! */
            *RefCell::borrow_mut(&pet.player.description) = buf;
        }
        let ch = db.ch(chid);
        db.char_to_room(pet_id, ch.in_room());
        add_follower(game, db, pet_id, chid);

        /* Be certain that pets can't get/carry/use/wield/wear items */
        let pet = db.ch_mut(pet_id);
        pet.set_is_carrying_w(1000);
        pet.set_is_carrying_n(100);
        let ch = db.ch(chid);
        game.send_to_char(ch, "May you enjoy your pet.\r\n");
        let pet = db.ch(pet_id);
        game.act(db,
            "$n buys $N as a pet.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(pet)),
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

pub fn bank(game: &mut Game, db: &mut DB, chid: DepotId, _me: MeRef, cmd: i32, argument: &str) -> bool {
    let ch = db.ch(chid);

    return if cmd_is(cmd, "balance") {
        if ch.get_bank_gold() > 0 {
            game.send_to_char(ch,
                format!("Your current balance is {} coins.\r\n", ch.get_bank_gold()).as_str(),
            );
        } else {
            game.send_to_char(ch, "You currently have no money deposited.\r\n");
        }
        true
    } else if cmd_is(cmd, "deposit") {
        let amount = argument.trim_start().parse::<i32>();
        let amount = if amount.is_ok() { amount.unwrap() } else { -1 };
        if amount <= 0 {
            game.send_to_char(ch, "How much do you want to deposit?\r\n");
            return true;
        }
        if ch.get_gold() < amount {
            game.send_to_char(ch, "You don't have that many coins!\r\n");
            return true;
        }
        let ch = db.ch_mut(chid);
        ch.set_gold(ch.get_gold() - amount);
        ch.set_bank_gold(ch.get_bank_gold() + amount);
        game.send_to_char(ch, format!("You deposit {} coins.\r\n", amount).as_str());
        let ch = db.ch(chid);
        game.act(db,
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
            game.send_to_char(ch, "How much do you want to withdraw?\r\n");
            return true;
        }
        if ch.get_bank_gold() < amount {
            game.send_to_char(ch, "You don't have that many coins deposited!\r\n");
            return true;
        }
        let ch = db.ch_mut(chid);
        ch.set_gold(ch.get_gold() + amount);
        ch.set_bank_gold(ch.get_bank_gold() - amount);
        let ch = db.ch(chid);
        game.send_to_char(ch, format!("You withdraw {} coins.\r\n", amount).as_str());
        game.act(db,
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
