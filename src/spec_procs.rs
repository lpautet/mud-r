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

use crate::depot::{Depot, DepotId};
use crate::handler::{obj_from_obj, obj_to_char};
use crate::{act, send_to_char, ObjData, TextData, VictimRef};
use std::cmp::{max, min};
use std::rc::Rc;

use crate::act_comm::do_say;
use crate::act_item::do_drop;
use crate::act_movement::{do_gen_door, perform_move};
use crate::act_social::do_action;
use crate::class::{GUILD_INFO, PRAC_PARAMS};
use crate::constants::INT_APP;
use crate::db::{LoadType, DB};
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
    AffectFlags, CharData, ItemType, MeRef, Position, WearFlags, LVL_IMMORT, MAX_SKILLS, NOWHERE,
    PLR_KILLER, PLR_THIEF,
};
use crate::util::{add_follower, can_see, rand_number};
use crate::{Game, TO_NOTVICT, TO_ROOM, TO_VICT};

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

pub fn list_skills(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, chid: DepotId) {
    let ch = chars.get(chid);
    if ch.get_practices() == 0 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You have no practice sessions remaining.\r\n",
        );
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
    page_string(&mut game.descriptors, chars, d_id, &buf, true);
}

#[allow(clippy::too_many_arguments)]
pub fn guild(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    argument: &str,
) -> bool {
    let ch = chars.get(chid);
    if ch.is_npc() || !cmd_is(cmd, "practice") {
        return false;
    }

    let argument = argument.trim();

    if argument.is_empty() {
        list_skills(game, chars, db, chid);
        return true;
    }

    if ch.get_practices() <= 0 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You do not seem to be able to practice now.\r\n",
        );
        return true;
    }

    let skill_num = find_skill_num(db, argument);

    if skill_num.is_none()
        || ch.get_level()
            < db.spell_info[skill_num.unwrap() as usize].min_level[ch.get_class() as usize] as u8
    {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You do not know of that {}.\r\n", splskl(ch)).as_str(),
        );
        return true;
    }
    if ch.get_skill(skill_num.unwrap()) >= learned(ch) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are already learned in that area.\r\n",
        );
        return true;
    }
    send_to_char(&mut game.descriptors, ch, "You practice for a while...\r\n");
    let ch = chars.get_mut(chid);
    ch.set_practices(ch.get_practices() - 1);

    let mut percent = ch.get_skill(skill_num.unwrap());
    percent += min(
        maxgain(ch),
        max(mingain(ch), INT_APP[ch.get_int() as usize].learn as i32),
    ) as i8;

    ch.set_skill(skill_num.unwrap(), min(learned(ch), percent));

    if ch.get_skill(skill_num.unwrap()) >= learned(ch) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are now learned in that area.\r\n",
        );
    }

    true
}

#[allow(clippy::too_many_arguments)]
pub fn dump(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    argument: &str,
) -> bool {
    let ch = chars.get(chid);

    for k_id in db.world[ch.in_room() as usize].contents.clone() {
        let k = objs.get(k_id);
        act(
            &mut game.descriptors,
            chars,
            db,
            "$p vanishes in a puff of smoke!",
            false,
            None,
            Some(k),
            None,
            TO_ROOM,
        );
        db.extract_obj(chars, objs, k_id);
    }

    if !cmd_is(cmd, "drop") {
        return false;
    }

    do_drop(
        game,
        db,
        chars,
        texts,
        objs,
        chid,
        argument,
        cmd,
        SCMD_DROP as i32,
    );
    let mut value = 0;
    let ch = chars.get(chid);
    for k_id in db.world[ch.in_room() as usize].contents.clone() {
        let k = objs.get(k_id);
        act(
            &mut game.descriptors,
            chars,
            db,
            "$p vanishes in a puff of smoke!",
            false,
            None,
            Some(k),
            None,
            TO_ROOM,
        );
        value += (k.get_obj_cost() / 10).clamp(1, 50);
        db.extract_obj(chars, objs, k_id);
    }
    let ch = chars.get(chid);
    if value != 0 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are awarded for outstanding performance.\r\n",
        );
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n has been awarded for being a good citizen.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let ch = chars.get(chid);
        if ch.get_level() < 3 {
            gain_exp(chid, value, game, chars, db, texts, objs);
        } else {
            let ch = chars.get_mut(chid);
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

#[allow(clippy::too_many_arguments)]
pub fn mayor(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
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
    let ch = chars.get(chid);
    if cmd != 0
        || !db.mayor.move_
        || ch.get_pos() < Position::Sleeping
        || ch.get_pos() == Position::Fighting
    {
        return false;
    }

    let a = &db.mayor.path[db.mayor.path_index..db.mayor.path_index + 1]
        .chars()
        .next()
        .unwrap();
    match a {
        '0' | '1' | '2' | '3' => {
            let dir = db.mayor.path[db.mayor.path_index..db.mayor.path_index + 1]
                .parse::<u8>()
                .unwrap();
            perform_move(game, db, chars, texts, objs, chid, dir as i32, true);
        }

        'W' => {
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Standing);
            let ch = chars.get(chid);
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n awakens and groans loudly.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'S' => {
            let ch = chars.get_mut(chid);
            ch.set_pos(Position::Sleeping);
            let ch = chars.get(chid);
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n lies down and instantly falls asleep.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'a' => {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n says 'Hello Honey!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n smirks.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'b' => {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n says 'What a view!  I must get something done about that dump!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'c' => {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n says 'Vandals!  Youngsters nowadays have no respect for anything!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'd' => {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n says 'Good day, citizens!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'e' => {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n says 'I hereby declare the bazaar open!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'E' => {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n says 'I hereby declare Midgaard closed!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'O' => {
            do_gen_door(game, db, chars, texts, objs, chid, "gate", 0, SCMD_UNLOCK);
            do_gen_door(game, db, chars, texts, objs, chid, "gate", 0, SCMD_OPEN);
        }

        'C' => {
            do_gen_door(game, db, chars, texts, objs, chid, "gate", 0, SCMD_CLOSE);
            do_gen_door(game, db, chars, texts, objs, chid, "gate", 0, SCMD_LOCK);
        }

        '.' => {
            db.mayor.move_ = false;
        }
        _ => {}
    }

    db.mayor.path_index += 1;
    false
}

/* ********************************************************************
*  General special procedures for mobiles                             *
******************************************************************** */

fn npc_steal(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    chid: DepotId,
    victim_id: DepotId,
) {
    let victim = chars.get(victim_id);
    let ch = chars.get(chid);

    if victim.is_npc() {
        return;
    }

    if victim.get_level() >= LVL_IMMORT {
        return;
    }
    if !can_see(&game.descriptors, chars, db, ch, victim) {
        return;
    }

    if victim.awake() && rand_number(0, ch.get_level() as u32) == 0 {
        act(
            &mut game.descriptors,
            chars,
            db,
            "You discover that $n has $s hands in your wallet.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(victim)),
            TO_VICT,
        );
        act(
            &mut game.descriptors,
            chars,
            db,
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
            let ch = chars.get_mut(chid);
            ch.set_gold(ch.get_gold() + gold);
            let victim = chars.get_mut(victim_id);
            victim.set_gold(victim.get_gold() - gold);
        }
    }
}

/*
 * Quite lethal to low-level characters.
 */
#[allow(clippy::too_many_arguments)]
pub fn snake(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || ch.get_pos() != Position::Fighting || ch.fighting_id().is_none() {
        return false;
    }

    if chars.get(ch.fighting_id().unwrap()).in_room() != ch.in_room()
        || rand_number(0, ch.get_level() as u32) != 0
    {
        return false;
    }
    let fighting = chars.get(ch.fighting_id().unwrap());
    act(
        &mut game.descriptors,
        chars,
        db,
        "$n bites $N!",
        true,
        Some(ch),
        None,
        Some(VictimRef::Char(fighting)),
        TO_NOTVICT,
    );
    act(
        &mut game.descriptors,
        chars,
        db,
        "$n bites you!",
        true,
        Some(ch),
        None,
        Some(VictimRef::Char(fighting)),
        TO_VICT,
    );
    call_magic(
        game,
        chars,
        db,
        texts,
        objs,
        chid,
        ch.fighting_id(),
        None,
        SPELL_POISON,
        ch.get_level(),
        CAST_SPELL,
    );
    true
}

#[allow(clippy::too_many_arguments)]
pub fn thief(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || ch.get_pos() != Position::Standing {
        return false;
    }
    for cons_id in db.world[ch.in_room() as usize].peoples.clone() {
        let cons = chars.get(cons_id);
        if !cons.is_npc() && cons.get_level() < LVL_IMMORT && rand_number(0, 4) == 0 {
            npc_steal(game, chars, db, chid, cons_id);
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn magic_user(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || ch.get_pos() != Position::Fighting {
        return false;
    }
    /* pseudo-randomly choose someone in the room who is fighting me */
    let mut vict_id = None;
    {
        for &v_id in &db.world[ch.in_room() as usize].peoples {
            let v = chars.get(v_id);
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
    if vict_id.is_none() && chars.get(ch.fighting_id().unwrap()).in_room() == ch.in_room() {
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
        cast_spell(
            game,
            chars,
            db,
            texts,
            objs,
            chid,
            vict_id,
            None,
            SPELL_POISON,
        );
    }
    let ch = chars.get(chid);
    if ch.get_level() > 7 && rand_number(0, 8) == 0 {
        cast_spell(
            game,
            chars,
            db,
            texts,
            objs,
            chid,
            vict_id,
            None,
            SPELL_BLINDNESS,
        );
    }
    let ch = chars.get(chid);
    if ch.get_level() > 12 && rand_number(0, 12) == 0 {
        if ch.is_evil() {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_ENERGY_DRAIN,
            );
        } else if ch.is_good() {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_DISPEL_EVIL,
            );
        }
    }

    if rand_number(0, 4) != 0 {
        return true;
    }
    let ch = chars.get(chid);
    match ch.get_level() {
        4 | 5 => {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_MAGIC_MISSILE,
            );
        }
        6 | 7 => {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_CHILL_TOUCH,
            );
        }
        8 | 9 => {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_BURNING_HANDS,
            );
        }
        10 | 11 => {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_SHOCKING_GRASP,
            );
        }
        12 | 13 => {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_LIGHTNING_BOLT,
            );
        }
        14..=17 => {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_COLOR_SPRAY,
            );
        }
        _ => {
            cast_spell(
                game,
                chars,
                db,
                texts,
                objs,
                chid,
                vict_id,
                None,
                SPELL_FIREBALL,
            );
        }
    }
    true
}

/* ********************************************************************
*  Special procedures for mobiles                                      *
******************************************************************** */
#[allow(clippy::too_many_arguments)]
pub fn guild_guard(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    let guard_id = match me {
        MeRef::Char(me_chid) => me_chid,
        _ => panic!("Unexpected MeRef type in guild_guard"),
    };
    let guard = chars.get(guard_id);
    let buf = "The guard humiliates you, and blocks your way.\r\n";
    let buf2 = "The guard humiliates $n, and blocks $s way.";

    if !is_move(cmd) || guard.aff_flagged(AffectFlags::BLIND) {
        return false;
    }

    if ch.get_level() >= LVL_IMMORT {
        return false;
    }

    for gi in GUILD_INFO {
        if gi.guild_room == NOWHERE {
            break;
        }
        /* Wrong guild or not trying to enter. */
        if db.get_room_vnum(ch.in_room()) != gi.guild_room || cmd != gi.direction as usize {
            continue;
        }
        /* Allow the people of the guild through. */
        if !ch.is_npc() && ch.get_class() == gi.pc_class {
            continue;
        }
        send_to_char(&mut game.descriptors, ch, buf);
        act(
            &mut game.descriptors,
            chars,
            db,
            buf2,
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );

        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn puff(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
    if cmd != 0 {
        return false;
    }

    match rand_number(0, 60) {
        0 => {
            do_say(
                game,
                db,
                chars,
                texts,
                objs,
                chid,
                "My god!  It's full of stars!",
                0,
                0,
            );
            true
        }
        1 => {
            do_say(
                game,
                db,
                chars,
                texts,
                objs,
                chid,
                "How'd all those fish get up here?",
                0,
                0,
            );
            true
        }
        2 => {
            do_say(
                game,
                db,
                chars,
                texts,
                objs,
                chid,
                "I'm a very female dragon.",
                0,
                0,
            );
            true
        }
        3 => {
            do_say(
                game,
                db,
                chars,
                texts,
                objs,
                chid,
                "I've got a peaceful, easy feeling.",
                0,
                0,
            );
            true
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn fido(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || !ch.awake() {
        return false;
    }

    for i in db.world[ch.in_room() as usize].contents.clone() {
        if !objs.get(i).is_corpse() {
            continue;
        }

        act(
            &mut game.descriptors,
            chars,
            db,
            "$n savagely devours a corpse.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        for temp_id in objs.get(i).contains.clone().into_iter() {
            obj_from_obj(chars, objs, temp_id);
            let ch = chars.get(chid);
            db.obj_to_room(objs.get_mut(temp_id), ch.in_room());
        }
        db.extract_obj(chars, objs, i);
        return true;
    }

    false
}

#[allow(clippy::too_many_arguments)]
pub fn janitor(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || !ch.awake() {
        return false;
    }
    for i_id in db.world[ch.in_room() as usize].contents.clone().into_iter() {
        let i = objs.get_mut(i_id);
        if !i.can_wear(WearFlags::TAKE) {
            continue;
        }
        if i.get_obj_type() != ItemType::Drinkcon && i.get_obj_cost() >= 15 {
            continue;
        }
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n picks up some trash.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        db.obj_from_room(i);
        obj_to_char(i, chars.get_mut(chid));
        return true;
    }

    false
}

#[allow(clippy::too_many_arguments)]
pub fn cityguard(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || !ch.awake() || ch.fighting_id().is_some() {
        return false;
    }

    let mut max_evil = 1000;
    let mut min_cha = 6;
    let mut spittle = None;
    let mut evil_id = None;
    for tch_id in db.world[ch.in_room() as usize].peoples.clone() {
        let tch = chars.get(tch_id);
        if !can_see(&game.descriptors, chars, db, ch, tch) {
            continue;
        }

        if !tch.is_npc() && tch.plr_flagged(PLR_KILLER) {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n screams 'HEY!!!  You're one of those PLAYER KILLERS!!!!!!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            game.hit(chars, db, texts, objs, chid, tch_id, TYPE_UNDEFINED);
            return true;
        }

        if !tch.is_npc() && tch.plr_flagged(PLR_THIEF) {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n screams 'HEY!!!  You're one of those PLAYER THIEVES!!!!!!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            game.hit(chars, db, texts, objs, chid, tch_id, TYPE_UNDEFINED);
            return true;
        }

        if tch.fighting_id().is_some()
            && tch.get_alignment() < max_evil
            && (tch.is_npc() || chars.get(tch.fighting_id().unwrap()).is_npc())
        {
            max_evil = tch.get_alignment();
            evil_id = Some(tch_id);
        }

        if tch.get_cha() < min_cha {
            spittle = Some(tch);
            min_cha = tch.get_cha();
        }
    }

    #[allow(clippy::unnecessary_unwrap)]
    if evil_id.is_some()
        && chars
            .get(chars.get(evil_id.unwrap()).fighting_id().unwrap())
            .get_alignment()
            >= 0
    {
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n screams 'PROTECT THE INNOCENT!  BANZAI!  CHARGE!  ARARARAGGGHH!'",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        game.hit(
            chars,
            db,
            texts,
            objs,
            chid,
            evil_id.unwrap(),
            TYPE_UNDEFINED,
        );
        return true;
    }

    /* Reward the socially inept. */
    if spittle.is_some() && rand_number(0, 9) == 0 {
        let spit_social = find_command("spit");

        if let Some(spit_social) = spit_social {
            #[allow(clippy::unnecessary_unwrap)]
            do_action(
                game,
                db,
                chars,
                texts,
                objs,
                chid,
                &spittle.as_ref().unwrap().get_name().clone(),
                spit_social,
                0,
            );
            return true;
        }
    }

    false
}

fn pet_price(pet: &CharData) -> i32 {
    pet.get_level() as i32 * 300
}

#[allow(clippy::too_many_arguments)]
pub fn pet_shops(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    argument: &str,
) -> bool {
    let ch = chars.get(chid);

    /* Gross. */
    let pet_room = ch.in_room() + 1;

    if cmd_is(cmd, "list") {
        send_to_char(&mut game.descriptors, ch, "Available pets are:\r\n");
        for &pet_id in &db.world[pet_room as usize].peoples {
            let pet = chars.get(pet_id);
            /* No, you can't have the Implementor as a pet if he's in there. */
            if !pet.is_npc() {
                continue;
            }
            send_to_char(
                &mut game.descriptors,
                ch,
                format!("{:8} - {}\r\n", pet_price(pet), pet.get_name()).as_str(),
            );
        }
        return true;
    } else if cmd_is(cmd, "buy") {
        let mut buf = String::new();
        let mut pet_name = String::new();
        two_arguments(argument, &mut buf, &mut pet_name);
        let pet = db.get_char_room(chars, &buf, None, pet_room);
        if pet.is_none() || !pet.unwrap().is_npc() {
            send_to_char(&mut game.descriptors, ch, "There is no such pet!\r\n");
            return true;
        }
        let pet = pet.unwrap();
        if ch.get_gold() < pet_price(pet) {
            send_to_char(&mut game.descriptors, ch, "You don't have enough gold!\r\n");
            return true;
        }
        let pet_price = pet_price(pet);
        let pet_mob_rnum = pet.get_mob_rnum();
        let ch = chars.get_mut(chid);
        ch.set_gold(ch.get_gold() - pet_price);

        let pet_id = db.read_mobile(chars, pet_mob_rnum, LoadType::Real).unwrap();
        let pet = chars.get_mut(pet_id);
        pet.set_exp(0);
        pet.set_aff_flags_bits(AffectFlags::CHARM);

        if !pet_name.is_empty() {
            let buf = format!("{} {}", pet.player.name, pet_name);

            chars.get_mut(pet_id).player.name = Rc::from(buf.as_str());
            let pet = chars.get(pet_id);
            let text = &mut texts.get_mut(pet.player.description).text;
            let buf = format!(
                "{}A small sign on a chain around the neck says 'My name is {}'\r\n",
                text, pet_name
            );
            /* free(pet->player.description); don't free the prototype! */
            *text = buf;
        }
        let ch = chars.get(chid);
        db.char_to_room(chars, objs, pet_id, ch.in_room());
        add_follower(&mut game.descriptors, chars, db, pet_id, chid);

        /* Be certain that pets can't get/carry/use/wield/wear items */
        let pet = chars.get_mut(pet_id);
        pet.set_is_carrying_w(1000);
        pet.set_is_carrying_n(100);
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, "May you enjoy your pet.\r\n");
        let pet = chars.get(pet_id);
        act(
            &mut game.descriptors,
            chars,
            db,
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
    false
}

/* ********************************************************************
*  Special procedures for objects                                     *
******************************************************************** */
#[allow(clippy::too_many_arguments)]
pub fn bank(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    _me: MeRef,
    cmd: usize,
    argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd_is(cmd, "balance") {
        if ch.get_bank_gold() > 0 {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!("Your current balance is {} coins.\r\n", ch.get_bank_gold()).as_str(),
            );
        } else {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You currently have no money deposited.\r\n",
            );
        }
        true
    } else if cmd_is(cmd, "deposit") {
        let amount = argument.trim_start().parse::<i32>();
        let amount = amount.unwrap_or(-1);
        if amount <= 0 {
            send_to_char(
                &mut game.descriptors,
                ch,
                "How much do you want to deposit?\r\n",
            );
            return true;
        }
        if ch.get_gold() < amount {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You don't have that many coins!\r\n",
            );
            return true;
        }
        let ch = chars.get_mut(chid);
        ch.set_gold(ch.get_gold() - amount);
        ch.set_bank_gold(ch.get_bank_gold() + amount);
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You deposit {} coins.\r\n", amount).as_str(),
        );
        let ch = chars.get(chid);
        act(
            &mut game.descriptors,
            chars,
            db,
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
        let amount = amount.unwrap_or(-1);
        if amount <= 0 {
            send_to_char(
                &mut game.descriptors,
                ch,
                "How much do you want to withdraw?\r\n",
            );
            return true;
        }
        if ch.get_bank_gold() < amount {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You don't have that many coins deposited!\r\n",
            );
            return true;
        }
        let ch = chars.get_mut(chid);
        ch.set_gold(ch.get_gold() + amount);
        ch.set_bank_gold(ch.get_bank_gold() - amount);
        let ch = chars.get(chid);
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You withdraw {} coins.\r\n", amount).as_str(),
        );
        act(
            &mut game.descriptors,
            chars,
            db,
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
