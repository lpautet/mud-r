/* ************************************************************************
*   File: spells.h                                      Part of CircleMUD *
*  Usage: header file: constants and fn prototypes for spell system       *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */
use std::cmp::{max, min};
use std::rc::Rc;

use crate::act_item::{name_from_drinkcon, name_to_drinkcon, weight_change_object};
use crate::config::PK_ALLOWED;
use crate::constants::{AFFECTED_BITS, APPLY_TYPES, EXTRA_BITS, ITEM_TYPES};
use crate::db::DB;
use crate::fight::compute_armor_class;
use crate::handler::{affect_to_char, isname};
use crate::magic::mag_savingthrow;
use crate::spell_parser::{skill_name, UNUSED_SPELLNAME};
use crate::structs::{
    AffectedType, CharData, ObjData, RoomRnum, AFF_CHARM, AFF_POISON, AFF_SANCTUARY, APPLY_DAMROLL,
    APPLY_HITROLL, APPLY_NONE, ITEM_ANTI_EVIL, ITEM_ANTI_GOOD, ITEM_ARMOR, ITEM_DRINKCON,
    ITEM_FOOD, ITEM_FOUNTAIN, ITEM_MAGIC, ITEM_POTION, ITEM_SCROLL, ITEM_STAFF, ITEM_WAND,
    ITEM_WEAPON, LIQ_SLIME, LIQ_WATER, LVL_IMMORT, LVL_IMPL, MAX_OBJ_AFFECT, MOB_AGGRESSIVE,
    MOB_NOCHARM, MOB_NOSUMMON, MOB_SPEC, NOWHERE, NUM_CLASSES, PLR_KILLER, PRF_SUMMONABLE,
    ROOM_DEATH, ROOM_GODROOM, ROOM_PRIVATE, SEX_MALE,
};
use crate::util::{add_follower, age, circle_follow, rand_number, sprintbit, sprinttype, BRF};
use crate::{send_to_char, Game, TO_CHAR, TO_ROOM, TO_VICT};

pub const DEFAULT_STAFF_LVL: i32 = 12;
pub const DEFAULT_WAND_LVL: i32 = 12;

// pub const CAST_UNDEFINED: i32 = -1;
pub const CAST_SPELL: i32 = 0;
pub const CAST_POTION: i32 = 1;
pub const CAST_WAND: i32 = 2;
pub const CAST_STAFF: i32 = 3;
pub const CAST_SCROLL: i32 = 4;

pub const MAG_DAMAGE: i32 = 1 << 0;
pub const MAG_AFFECTS: i32 = 1 << 1;
pub const MAG_UNAFFECTS: i32 = 1 << 2;
pub const MAG_POINTS: i32 = 1 << 3;
pub const MAG_ALTER_OBJS: i32 = 1 << 4;
pub const MAG_GROUPS: i32 = 1 << 5;
pub const MAG_MASSES: i32 = 1 << 6;
pub const MAG_AREAS: i32 = 1 << 7;
pub const MAG_SUMMONS: i32 = 1 << 8;
pub const MAG_CREATIONS: i32 = 1 << 9;
pub const MAG_MANUAL: i32 = 1 << 10;
//
//
pub const TYPE_UNDEFINED: i32 = -1;
// #define SPELL_RESERVED_DBC            0  /* SKILL NUMBER ZERO -- RESERVED */
/* PLAYER SPELLS -- Numbered from 1 to MAX_SPELLS */

pub const SPELL_ARMOR: i32 = 1; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_TELEPORT: i32 = 2; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_BLESS: i32 = 3; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_BLINDNESS: i32 = 4; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_BURNING_HANDS: i32 = 5; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CALL_LIGHTNING: i32 = 6; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CHARM: i32 = 7; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CHILL_TOUCH: i32 = 8; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CLONE: i32 = 9; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_COLOR_SPRAY: i32 = 10; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CONTROL_WEATHER: i32 = 11; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CREATE_FOOD: i32 = 12; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CREATE_WATER: i32 = 13; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CURE_BLIND: i32 = 14; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CURE_CRITIC: i32 = 15; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CURE_LIGHT: i32 = 16; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_CURSE: i32 = 17; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_DETECT_ALIGN: i32 = 18; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_DETECT_INVIS: i32 = 19; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_DETECT_MAGIC: i32 = 20; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_DETECT_POISON: i32 = 21; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_DISPEL_EVIL: i32 = 22; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_EARTHQUAKE: i32 = 23; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_ENCHANT_WEAPON: i32 = 24; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_ENERGY_DRAIN: i32 = 25; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_FIREBALL: i32 = 26; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_HARM: i32 = 27; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_HEAL: i32 = 28; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_INVISIBLE: i32 = 29; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_LIGHTNING_BOLT: i32 = 30; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_LOCATE_OBJECT: i32 = 31; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_MAGIC_MISSILE: i32 = 32; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_POISON: i32 = 33; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_PROT_FROM_EVIL: i32 = 34; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_REMOVE_CURSE: i32 = 35; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_SANCTUARY: i32 = 36; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_SHOCKING_GRASP: i32 = 37; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_SLEEP: i32 = 38; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_STRENGTH: i32 = 39; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_SUMMON: i32 = 40; /* Reserved Skill[] DO NOT CHANGE */
// pub const SPELL_VENTRILOQUATE: i32 = 41; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_WORD_OF_RECALL: i32 = 42; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_REMOVE_POISON: i32 = 43; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_SENSE_LIFE: i32 = 44; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_ANIMATE_DEAD: i32 = 45; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_DISPEL_GOOD: i32 = 46; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_GROUP_ARMOR: i32 = 47; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_GROUP_HEAL: i32 = 48; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_GROUP_RECALL: i32 = 49; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_INFRAVISION: i32 = 50; /* Reserved Skill[] DO NOT CHANGE */
pub const SPELL_WATERWALK: i32 = 51; /* Reserved Skill[] DO NOT CHANGE */
/* Insert new spells here, up to MAX_SPELLS */
pub const MAX_SPELLS: i32 = 130;

/* PLAYER SKILLS - Numbered from MAX_SPELLS+1 to MAX_SKILLS */
pub const SKILL_BACKSTAB: i32 = 131; /* Reserved Skill[] DO NOT CHANGE */
pub const SKILL_BASH: i32 = 132; /* Reserved Skill[] DO NOT CHANGE */
pub const SKILL_HIDE: i32 = 133; /* Reserved Skill[] DO NOT CHANGE */
pub const SKILL_KICK: i32 = 134; /* Reserved Skill[] DO NOT CHANGE */
pub const SKILL_PICK_LOCK: i32 = 135; /* Reserved Skill[] DO NOT CHANGE */
/* Undefined                        136 */
pub const SKILL_RESCUE: i32 = 137; /* Reserved Skill[] DO NOT CHANGE */
pub const SKILL_SNEAK: i32 = 138; /* Reserved Skill[] DO NOT CHANGE */
pub const SKILL_STEAL: i32 = 139; /* Reserved Skill[] DO NOT CHANGE */
pub const SKILL_TRACK: i32 = 140; /* Reserved Skill[] DO NOT CHANGE */
/* New skills may be added here up to MAX_SKILLS (200) */

/*
 *  NON-PLAYER AND OBJECT SPELLS AND SKILLS
 *  The practice levels for the spells and skills below are _not_ recorded
 *  in the playerfile; therefore, the intended use is for spells and skills
 *  associated with objects (such as SPELL_IDENTIFY used with scrolls of
 *  identify) or non-players (such as NPC-only spells).
 */

pub const SPELL_IDENTIFY: i32 = 201;
pub const SPELL_FIRE_BREATH: i32 = 202;
pub const SPELL_GAS_BREATH: i32 = 203;
pub const SPELL_FROST_BREATH: i32 = 204;
pub const SPELL_ACID_BREATH: i32 = 205;
pub const SPELL_LIGHTNING_BREATH: i32 = 206;

pub const TOP_SPELL_DEFINE: usize = 299;
// /* NEW NPC/OBJECT SPELLS can be inserted here up to 299 */
/* WEAPON ATTACK TYPES */

pub const TYPE_HIT: i32 = 300;
// pub const TYPE_STING: i32 = 301;
// pub const TYPE_WHIP: i32 = 302;
// pub const TYPE_SLASH: i32 = 303;
// pub const TYPE_BITE: i32 = 304;
// pub const TYPE_BLUDGEON: i32 = 305;
// pub const TYPE_CRUSH: i32 = 306;
// pub const TYPE_POUND: i32 = 307;
// pub const TYPE_CLAW: i32 = 308;
// pub const TYPE_MAUL: i32 = 309;
// pub const TYPE_THRASH: i32 = 310;
pub const TYPE_PIERCE: i32 = 311;
// pub const TYPE_BLAST: i32 = 312;
// pub const TYPE_PUNCH: i32 = 313;
// pub const TYPE_STAB: i32 = 314;

// /* new attack types can be added here - up to TYPE_SUFFERING */
pub const TYPE_SUFFERING: i32 = 399;

pub const SAVING_PARA: i32 = 0;
pub const SAVING_ROD: i32 = 1;
pub const SAVING_PETRI: i32 = 2;
pub const SAVING_BREATH: i32 = 3;
pub const SAVING_SPELL: i32 = 4;

pub const TAR_IGNORE: i32 = 1 << 0;
pub const TAR_CHAR_ROOM: i32 = 1 << 1;
pub const TAR_CHAR_WORLD: i32 = 1 << 2;
pub const TAR_FIGHT_SELF: i32 = 1 << 3;
pub const TAR_FIGHT_VICT: i32 = 1 << 4;
pub const TAR_SELF_ONLY: i32 = 1 << 5; /* Only a check, use with i.e. TAR_CHAR_ROOM */
pub const TAR_NOT_SELF: i32 = 1 << 6; /* Only a check, use with i.e. TAR_CHAR_ROOM */
pub const TAR_OBJ_INV: i32 = 1 << 7;
pub const TAR_OBJ_ROOM: i32 = 1 << 8;
pub const TAR_OBJ_WORLD: i32 = 1 << 9;
pub const TAR_OBJ_EQUIP: i32 = 1 << 10;

pub struct SpellInfoType {
    pub min_position: u8,
    /* Position for caster	 */
    pub mana_min: i32,
    /* Min amount of mana used by a spell (highest lev) */
    pub mana_max: i32,
    /* Max amount of mana used by a spell (lowest lev) */
    pub mana_change: i32,
    /* Change in mana used by spell from lev to lev */
    pub min_level: [i32; NUM_CLASSES as usize],
    pub routines: i32,
    pub violent: bool,
    pub targets: i32,
    /* See below for use with TAR_XXX  */
    pub name: &'static str,
    /* Input size not limited. Originates from string constants. */
    pub wear_off_msg: Option<&'static str>,
    /* Input size not limited. Originates from string constants. */
}

impl Default for SpellInfoType {
    fn default() -> Self {
        SpellInfoType {
            min_position: 0,
            mana_min: 0,
            mana_max: 0,
            mana_change: 0,
            min_level: [(LVL_IMPL + 1) as i32; NUM_CLASSES as usize],
            routines: 0,
            violent: false,
            targets: 0,
            name: UNUSED_SPELLNAME,
            wear_off_msg: None,
        }
    }
}

impl Copy for SpellInfoType {}

impl Clone for SpellInfoType {
    fn clone(&self) -> Self {
        SpellInfoType {
            min_position: self.min_position,
            mana_min: self.mana_min,
            mana_max: self.mana_max,
            mana_change: self.mana_change,
            min_level: self.min_level,
            routines: self.routines,
            violent: self.violent,
            targets: self.targets,
            name: self.name.clone(),
            wear_off_msg: self.wear_off_msg.clone(),
        }
    }
}

// /* Possible Targets:
//
//    bit 0 : IGNORE TARGET
//    bit 1 : PC/NPC in room
//    bit 2 : PC/NPC in world
//    bit 3 : Object held
//    bit 4 : Object in inventory
//    bit 5 : Object in room
//    bit 6 : Object in world
//    bit 7 : If fighting, and no argument, select tar_char as self
//    bit 8 : If fighting, and no argument, select tar_char as victim (fighting)
//    bit 9 : If no argument, select self, if argument check that it IS self.
//
// */
//
// #define SPELL_TYPE_SPELL   0
// #define SPELL_TYPE_POTION  1
// #define SPELL_TYPE_WAND    2
// #define SPELL_TYPE_STAFF   3
// #define SPELL_TYPE_SCROLL  4

/* Attacktypes with grammar */

pub struct AttackHitType {
    pub singular: &'static str,
    pub plural: &'static str,
}

// #define ASPELL(spellname) \
// void	spellname(int level, struct char_data *ch, \
// struct char_data *victim, struct obj_data *obj)
//
// #define MANUAL_SPELL(spellname)	spellname(level, caster, cvict, ovict);
//
// ASPELL(spell_create_water);
// ASPELL(spell_recall);
// ASPELL(spell_teleport);
// ASPELL(spell_summon);
// ASPELL(spell_locate_object);
// ASPELL(spell_charm);
// ASPELL(spell_information);
// ASPELL(spell_identify);
// ASPELL(spell_enchant_weapon);
// ASPELL(spell_detect_poison);
//
// /* basic magic calling functions */
//
// int find_skill_num(char *name);
//
// int mag_damage(int level, struct char_data *ch, struct char_data *victim,
// int spellnum, int savetype);
//
// void mag_affects(int level, struct char_data *ch, struct char_data *victim,
// int spellnum, int savetype);
//
// void mag_groups(int level, struct char_data *ch, int spellnum, int savetype);
//
// void mag_masses(int level, struct char_data *ch, int spellnum, int savetype);
//
// void mag_areas(int level, struct char_data *ch, int spellnum, int savetype);
//
// void mag_summons(int level, struct char_data *ch, struct obj_data *obj,
// int spellnum, int savetype);
//
// void mag_points(int level, struct char_data *ch, struct char_data *victim,
// int spellnum, int savetype);
//
// void mag_unaffects(int level, struct char_data *ch, struct char_data *victim,
// int spellnum, int type);
//
// void mag_alter_objs(int level, struct char_data *ch, struct obj_data *obj,
// int spellnum, int type);
//
// void mag_creations(int level, struct char_data *ch, int spellnum);
//
// int	call_magic(struct char_data *caster, struct char_data *cvict,
// struct obj_data *ovict, int spellnum, int level, int casttype);
//
// void	mag_objectmagic(struct char_data *ch, struct obj_data *obj,
// char *argument);
//
// int	cast_spell(struct char_data *ch, struct char_data *tch,
// struct obj_data *tobj, int spellnum);
//
//
// /* other prototypes */
// void spell_level(int spell, int chclass, int level);
// void init_spell_levels(void);
// const char *skill_name(int num);
//
// /* ************************************************************************
// *   File: spells.c                                      Part of CircleMUD *
// *  Usage: Implementation of "manual spells".  Circle 2.2 spell compat.    *
// *                                                                         *
// *  All rights reserved.  See license.doc for complete information.        *
// *                                                                         *
// *  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
// *  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
// ************************************************************************ */
//
//
// #include "conf.h"
// #include "sysdep.h"
//
// #include "structs.h"
// #include "utils.h"
// #include "comm.h"
// #include "spells.h"
// #include "handler.h"
// #include "db.h"
// #include "constants.h"
// #include "interpreter.h"
//
//
// /* external variables */
// extern room_rnum r_mortal_start_room;
// extern int mini_mud;
// extern int pk_allowed;
//
// /* external functions */
// void clearMemory(struct char_data *ch);
// void weight_change_object(struct obj_data *obj, int weight);
// int mag_savingthrow(struct char_data *ch, int type, int modifier);
// void name_to_drinkcon(struct obj_data *obj, int type);
// void name_from_drinkcon(struct obj_data *obj);
// int compute_armor_class(struct char_data *ch);

/*
 * Special spells appear below.
 */
#[allow(unused_variables)]
pub fn spell_create_water(
    db: &DB,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    if ch.is_none() || obj.is_none() {
        return;
    }
    let ch = ch.unwrap();
    let obj = obj.unwrap();
    /* level = MAX(MIN(level, LVL_IMPL), 1);	 - not used */

    if obj.get_obj_type() == ITEM_DRINKCON {
        if obj.get_obj_val(2) != LIQ_WATER && obj.get_obj_val(1) != 0 {
            name_from_drinkcon(Some(obj));
            obj.set_obj_val(2, LIQ_SLIME);
            name_to_drinkcon(Some(obj), LIQ_SLIME);
        } else {
            let water = max(obj.get_obj_val(0) - obj.get_obj_val(1), 0);
            if water > 0 {
                if obj.get_obj_val(1) >= 0 {
                    name_from_drinkcon(Some(obj));
                }
                obj.set_obj_val(2, LIQ_WATER);
                obj.set_obj_val(1, obj.get_obj_val(1) + water);
                name_to_drinkcon(Some(obj), LIQ_WATER);
                weight_change_object(db, obj, water);
                db.act("$p is filled.", false, Some(ch), Some(obj), None, TO_CHAR);
            }
        }
    }
}

#[allow(unused_variables)]
pub fn spell_recall(
    db: &DB,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    if victim.is_none() || victim.unwrap().is_npc() {
        return;
    }

    let victim = victim.unwrap();

    db.act("$n disappears.", true, Some(victim), None, None, TO_ROOM);
    db.char_from_room(victim);
    db.char_to_room(Some(victim), *db.r_mortal_start_room.borrow());
    db.act(
        "$n appears in the middle of the room.",
        true,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    db.look_at_room(victim, false);
}

#[allow(unused_variables)]
pub fn spell_teleport(
    db: &DB,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    let mut to_room;

    if victim.is_none() || victim.unwrap().is_npc() {
        return;
    }
    let victim = victim.unwrap();

    loop {
        to_room = rand_number(0, db.world.borrow().len() as u32);
        if !db.room_flagged(
            to_room as RoomRnum,
            ROOM_PRIVATE | ROOM_DEATH | ROOM_GODROOM,
        ) {
            break;
        }
    }

    db.act(
        "$n slowly fades out of existence and is gone.",
        false,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    db.char_from_room(victim);
    db.char_to_room(Some(victim), to_room as RoomRnum);
    db.act(
        "$n slowly fades into existence.",
        false,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    db.look_at_room(victim, false);
}

const SUMMON_FAIL: &str = "You failed.\r\n";

#[allow(unused_variables)]
pub fn spell_summon(
    game: &Game,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    let db = &game.db;
    if ch.is_none() || victim.is_none() {
        return;
    }
    let victim = victim.unwrap();
    let ch = ch.unwrap();
    if victim.get_level() > min((LVL_IMMORT - 1) as u8, (level + 3) as u8) {
        send_to_char(ch, SUMMON_FAIL);
        return;
    }

    if !PK_ALLOWED {
        if victim.mob_flagged(MOB_AGGRESSIVE) {
            db.act("As the words escape your lips and $N travels\r\nthrough time and space towards you, you realize that $E is\r\naggressive and might harm you, so you wisely send $M back.",
                   false, Some(ch), None, Some(victim), TO_CHAR);
            return;
        }
        if !victim.is_npc()
            && !victim.prf_flagged(PRF_SUMMONABLE)
            && !victim.plr_flagged(PLR_KILLER)
        {
            send_to_char(victim, format!("{} just tried to summon you to: {}.\r\n{} failed because you have summon protection on.\r\nType NOSUMMON to allow other players to summon you.\r\n",
                                         ch.get_name(), db.world.borrow()[ch.in_room() as usize].name,
                                         if ch.player.borrow().sex == SEX_MALE { "He" } else { "She" }).as_str());

            send_to_char(
                ch,
                format!(
                    "You failed because {} has summon protection on.\r\n",
                    victim.get_name()
                )
                .as_str(),
            );
            game.mudlog(
                BRF,
                LVL_IMMORT as i32,
                true,
                format!(
                    "{} failed summoning {} to {}.",
                    ch.get_name(),
                    victim.get_name(),
                    db.world.borrow()[ch.in_room() as usize].name
                )
                .as_str(),
            );
            return;
        }
    }

    if victim.mob_flagged(MOB_NOSUMMON)
        || victim.is_npc() && mag_savingthrow(victim, SAVING_SPELL, 0)
    {
        send_to_char(ch, SUMMON_FAIL);
        return;
    }

    db.act(
        "$n disappears suddenly.",
        true,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );

    db.char_from_room(victim);
    db.char_to_room(Some(victim), ch.in_room());

    db.act(
        "$n arrives suddenly.",
        true,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    db.act(
        "$n has summoned you!",
        false,
        Some(ch),
        None,
        Some(victim),
        TO_VICT,
    );
    db.look_at_room(victim, false);
}

#[allow(unused_variables)]
pub fn spell_locate_object(
    db: &DB,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    // struct obj_data *i;
    // char name[MAX_INPUT_LENGTH];
    // int j;

    /*
     * FIXME: This is broken.  The spell parser routines took the argument
     * the player gave to the spell and located an object with that keyword.
     * Since we're passed the object and not the keyword we can only guess
     * at what the player originally meant to search for. -gg
     */
    let obj = obj.unwrap();
    let mut name = String::new();
    name.push_str(&obj.name.borrow());
    let mut j = level / 2;

    for i in db.object_list.borrow().iter() {
        if !isname(&name, &i.name.borrow()) {
            continue;
        }

        send_to_char(
            ch.unwrap(),
            format!(
                "{}{}",
                &i.short_description[0..0].to_uppercase(),
                &i.short_description[1..]
            )
            .as_str(),
        );

        if i.carried_by.borrow().is_some() {
            send_to_char(
                ch.unwrap(),
                format!(
                    " is being carried by {}.\r\n",
                    db.pers(i.carried_by.borrow().as_ref().unwrap(), ch.unwrap())
                )
                .as_str(),
            );
        } else if i.in_room() != NOWHERE {
            send_to_char(
                ch.unwrap(),
                format!(
                    " is in {}.\r\n",
                    db.world.borrow()[i.in_room() as usize].name
                )
                .as_str(),
            );
        } else if i.in_obj.borrow().is_some() {
            send_to_char(
                ch.unwrap(),
                format!(
                    " is in {}.\r\n",
                    i.in_obj.borrow().as_ref().unwrap().short_description
                )
                .as_str(),
            );
        } else if i.worn_by.borrow().is_some() {
            send_to_char(
                ch.unwrap(),
                format!(
                    " is being worn by {}.\r\n",
                    db.pers(i.worn_by.borrow().as_ref().unwrap(), ch.unwrap())
                )
                .as_str(),
            );
        } else {
            send_to_char(ch.unwrap(), "'s location is uncertain.\r\n");
        }

        j -= 1;
    }

    if j == level / 2 {
        send_to_char(ch.unwrap(), "You sense nothing.\r\n");
    }
}

#[allow(unused_variables)]
pub fn spell_charm(
    db: &DB,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    if victim.is_none() || ch.is_none() {
        return;
    }
    let victim = victim.unwrap();
    let ch = ch.unwrap();

    if Rc::ptr_eq(victim, ch) {
        send_to_char(ch, "You like yourself even better!\r\n");
    } else if !victim.is_npc() && !victim.prf_flagged(PRF_SUMMONABLE) {
        send_to_char(ch, "You fail because SUMMON protection is on!\r\n");
    } else if victim.aff_flagged(AFF_SANCTUARY) {
        send_to_char(ch, "Your victim is protected by sanctuary!\r\n");
    } else if victim.mob_flagged(MOB_NOCHARM) {
        send_to_char(ch, "Your victim resists!\r\n");
    } else if ch.aff_flagged(AFF_CHARM) {
        send_to_char(ch, "You can't have any followers of your own!\r\n");
    } else if victim.aff_flagged(AFF_CHARM) || level < victim.get_level() as i32 {
        send_to_char(ch, "You fail.\r\n");
        /* player charming another player - no legal reason for this */
    } else if !PK_ALLOWED && !victim.is_npc() {
        send_to_char(ch, "You fail - shouldn't be doing it anyway.\r\n");
    } else if circle_follow(victim, Some(ch)) {
        send_to_char(ch, "Sorry, following in circles can not be allowed.\r\n");
    } else if mag_savingthrow(victim, SAVING_PARA, 0) {
        send_to_char(ch, "Your victim resists!\r\n");
    } else {
        if victim.master.borrow().is_some() {
            db.stop_follower(victim);
        }

        add_follower(db, victim, ch);
        let mut af = AffectedType {
            _type: SPELL_CHARM as i16,
            duration: 24 * 2,
            modifier: 0,
            location: 0,
            bitvector: AFF_CHARM,
        };
        if ch.get_cha() != 0 {
            af.duration *= ch.get_cha() as i16;
        }
        if victim.get_int() != 0 {
            af.duration /= victim.get_int() as i16;
        }
        affect_to_char(victim, &af);

        db.act(
            "Isn't $n just such a nice fellow?",
            false,
            Some(ch),
            None,
            Some(victim),
            TO_VICT,
        );
        if victim.is_npc() {
            victim.remove_mob_flags_bit(MOB_SPEC);
        }
    }
}

#[allow(unused_variables)]
pub fn spell_identify(
    db: &DB,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    // int i, found;
    // size_t len;

    if obj.is_some() {
        let obj = obj.unwrap();
        let mut bitbuf = String::new();

        sprinttype(obj.get_obj_type() as i32, &ITEM_TYPES, &mut bitbuf);
        send_to_char(
            ch.unwrap(),
            format!(
                "You feel informed:\r\nObject '{}', Item type: {}\r\n",
                obj.short_description, bitbuf
            )
            .as_str(),
        );

        if obj.get_obj_affect() != 0 {
            sprintbit(obj.get_obj_affect(), &AFFECTED_BITS, &mut bitbuf);
            send_to_char(
                ch.unwrap(),
                format!("Item will give you following abilities:  %{}\r\n", bitbuf).as_str(),
            );
        }

        sprintbit(obj.get_obj_extra() as i64, &EXTRA_BITS, &mut bitbuf);
        send_to_char(ch.unwrap(), format!("Item is: {}\r\n", bitbuf).as_str());

        send_to_char(
            ch.unwrap(),
            format!(
                "Weight: {}, Value: {}, Rent: {}\r\n",
                obj.get_obj_weight(),
                obj.get_obj_cost(),
                obj.get_obj_rent()
            )
            .as_str(),
        );

        match obj.get_obj_type() {
            ITEM_SCROLL | ITEM_POTION => {
                if obj.get_obj_val(1) >= 1 {
                    bitbuf.push_str(skill_name(db, obj.get_obj_val(1)));
                }

                if obj.get_obj_val(2) >= 1 {
                    bitbuf.push_str(skill_name(db, obj.get_obj_val(2)));
                }

                if obj.get_obj_val(3) >= 1 {
                    bitbuf.push_str(skill_name(db, obj.get_obj_val(3)));
                }

                send_to_char(
                    ch.unwrap(),
                    format!(
                        "This {} casts: {}\r\n",
                        ITEM_TYPES[obj.get_obj_type() as usize],
                        bitbuf
                    )
                    .as_str(),
                );
            }
            ITEM_WAND | ITEM_STAFF => {
                send_to_char(
                    ch.unwrap(),
                    format!(
                        "This {} casts: {}\r\nIt has {} maximum charge{} and {} remaining.\r\n",
                        ITEM_TYPES[obj.get_obj_type() as usize],
                        skill_name(db, obj.get_obj_val(3)),
                        obj.get_obj_val(1),
                        if obj.get_obj_val(1) == 1 { "" } else { "s" },
                        obj.get_obj_val(2)
                    )
                    .as_str(),
                );
            }
            ITEM_WEAPON => {
                send_to_char(
                    ch.unwrap(),
                    format!(
                        "Damage Dice is '{}D{}' for an average per-round damage of {}.\r\n",
                        obj.get_obj_val(1),
                        obj.get_obj_val(2),
                        (obj.get_obj_val(2) + 1) * obj.get_obj_val(1) / 2
                    )
                    .as_str(),
                );
            }
            ITEM_ARMOR => {
                send_to_char(
                    ch.unwrap(),
                    format!("AC-apply is {}\r\n", obj.get_obj_val(0)).as_str(),
                );
            }
            _ => {}
        }
        let mut found = false;
        for i in 0..MAX_OBJ_AFFECT as usize {
            if obj.affected[i].get().location != APPLY_NONE as u8
                && obj.affected[i].get().modifier != 0
            {
                if !found {
                    send_to_char(ch.unwrap(), "Can affect you as :\r\n");
                    found = true;
                }
                sprinttype(
                    obj.affected[i].get().location as i32,
                    &APPLY_TYPES,
                    &mut bitbuf,
                );
                send_to_char(
                    ch.unwrap(),
                    format!(
                        "   Affects: {} By {}\r\n",
                        bitbuf,
                        obj.affected[i].get().modifier
                    )
                    .as_str(),
                );
            }
        }
    } else if victim.is_some() {
        /* victim */
        let victim = victim.unwrap();
        send_to_char(
            ch.unwrap(),
            format!("Name: {}\r\n", victim.get_name()).as_str(),
        );
        if !victim.is_npc() {
            send_to_char(
                ch.unwrap(),
                format!(
                    "{} is {} years, {} months, {} days and {} hours old.\r\n",
                    victim.get_name(),
                    age(victim).year,
                    age(victim).month,
                    age(victim).day,
                    age(victim).hours
                )
                .as_str(),
            );
            send_to_char(
                ch.unwrap(),
                format!(
                    "Height {} cm, Weight {} pounds\r\n",
                    victim.get_height(),
                    victim.get_weight()
                )
                .as_str(),
            );
            send_to_char(
                ch.unwrap(),
                format!(
                    "Level: {}, Hits: {}, Mana: {}\r\n",
                    victim.get_level(),
                    victim.get_hit(),
                    victim.get_mana()
                )
                .as_str(),
            );
            send_to_char(
                ch.unwrap(),
                format!(
                    "AC: {}, Hitroll: {}, Damroll: {}\r\n",
                    compute_armor_class(victim),
                    victim.get_hitroll(),
                    victim.get_damroll()
                )
                .as_str(),
            );
            send_to_char(
                ch.unwrap(),
                format!(
                    "Str: {}/{}, Int: {}, Wis: {}, Dex: {}, Con: {}, Cha: {}\r\n",
                    victim.get_str(),
                    victim.get_add(),
                    victim.get_int(),
                    victim.get_wis(),
                    victim.get_dex(),
                    victim.get_con(),
                    victim.get_cha()
                )
                .as_str(),
            );
        }
    }
}

/*
 * Cannot use this spell on an equipped object or it will mess up the
 * wielding character's hit/dam totals.
 */
#[allow(unused_variables)]
pub fn spell_enchant_weapon(
    db: &DB,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    if ch.is_none() || obj.is_none() {
        return;
    }
    let ch = ch.unwrap();
    let obj = obj.unwrap();

    /* Either already enchanted or not a weapon. */
    if obj.get_obj_type() != ITEM_WEAPON || obj.obj_flagged(ITEM_MAGIC) {
        return;
    }

    /* Make sure no other affections. */
    for i in 0..MAX_OBJ_AFFECT as usize {
        if obj.affected[i].get().location != APPLY_NONE as u8 {
            return;
        }
    }

    obj.set_obj_extra_bit(ITEM_MAGIC);

    let mut af0 = obj.affected[0].get();
    af0.location = APPLY_HITROLL as u8;
    af0.modifier = 1 + if level >= 18 { 1 } else { 0 };
    obj.affected[0].set(af0);

    let mut af1 = obj.affected[1].get();
    af1.location = APPLY_DAMROLL as u8;
    af1.modifier = 1 + if level >= 20 { 1 } else { 0 };
    obj.affected[1].set(af1);

    if ch.is_good() {
        obj.set_obj_extra_bit(ITEM_ANTI_EVIL);
        db.act("$p glows blue.", false, Some(ch), Some(obj), None, TO_CHAR);
    } else if ch.is_evil() {
        obj.set_obj_extra_bit(ITEM_ANTI_GOOD);
        db.act("$p glows red.", false, Some(ch), Some(obj), None, TO_CHAR);
    } else {
        db.act(
            "$p glows yellow.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
    }
}

#[allow(unused_variables)]
pub fn spell_detect_poison(
    db: &DB,
    level: i32,
    ch: Option<&Rc<CharData>>,
    victim: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
) {
    if victim.is_some() {
        let victim = victim.unwrap();
        let ch = ch.unwrap();
        if Rc::ptr_eq(ch, victim) {
            if victim.aff_flagged(AFF_POISON) {
                send_to_char(ch, "You can sense poison in your blood.\r\n");
            } else {
                send_to_char(ch, "You feel healthy.\r\n");
            }
        } else {
            if victim.aff_flagged(AFF_POISON) {
                db.act(
                    "You sense that $E is poisoned.",
                    false,
                    Some(ch),
                    None,
                    Some(victim),
                    TO_CHAR,
                );
            } else {
                db.act(
                    "You sense that $E is healthy.",
                    false,
                    Some(ch),
                    None,
                    Some(victim),
                    TO_CHAR,
                );
            }
        }

        if obj.is_some() {
            let obj = obj.unwrap();

            match obj.get_obj_type() {
                ITEM_DRINKCON | ITEM_FOUNTAIN | ITEM_FOOD => {
                    if obj.get_obj_val(3) != 0 {
                        db.act(
                            "You sense that $p has been contaminated.",
                            false,
                            Some(ch),
                            Some(obj),
                            None,
                            TO_CHAR,
                        );
                    } else {
                        db.act(
                            "You sense that $p is safe for consumption.",
                            false,
                            Some(ch),
                            Some(obj),
                            None,
                            TO_CHAR,
                        );
                    }
                }
                _ => {
                    send_to_char(ch, "You sense that it should not be consumed.\r\n");
                }
            }
        }
    }
}
