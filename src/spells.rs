/* ************************************************************************
*   File: spells.rs                                     Part of CircleMUD *
*  Usage: header file: constants and fn prototypes for spell system       *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */
use std::cmp::{max, min};
use crate::depot::{Depot, DepotId};
use crate::{act, send_to_char, CharData, ObjData, TextData, VictimRef, DB};

use crate::act_informative::look_at_room;
use crate::act_item::{name_from_drinkcon, name_to_drinkcon, weight_change_object};
use crate::config::PK_ALLOWED;
use crate::constants::{AFFECTED_BITS, APPLY_TYPES, EXTRA_BITS, ITEM_TYPES};
use crate::fight::compute_armor_class;
use crate::handler::{affect_to_char, isname};
use crate::magic::mag_savingthrow;
use crate::spell_parser::{skill_name, UNUSED_SPELLNAME};
use crate::structs::{
    AffectFlags, AffectedType, ApplyType, ExtraFlags, ItemType, Position, PrefFlags, RoomFlags, RoomRnum, Sex, LIQ_SLIME, LIQ_WATER, LVL_IMMORT, LVL_IMPL, MAX_OBJ_AFFECT, MOB_AGGRESSIVE, MOB_NOCHARM, MOB_NOSUMMON, MOB_SPEC, NOWHERE, NUM_CLASSES, PLR_KILLER
};
use crate::util::{add_follower, age, circle_follow, pers, rand_number, sprintbit, sprinttype, stop_follower, BRF};
use crate::{ Game, TO_CHAR, TO_ROOM, TO_VICT};

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
    pub min_position: Position,
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
            min_position: Position::Dead,
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
            name: self.name,
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

/*
 * Special spells appear below.
 */
pub fn spell_create_water(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>, 
    _level: i32,
    chid: Option<DepotId>,
    _victim_id: Option<DepotId>,
    obj_id: Option<DepotId>,
) {
    if chid.is_none() || obj_id.is_none() {
        return;
    }
    let chid = chid.unwrap();
    let obj_id = obj_id.unwrap();
    /* level = MAX(MIN(level, LVL_IMPL), 1);	 - not used */

    if  objs.get(obj_id).get_obj_type() == ItemType::Drinkcon {
        if  objs.get(obj_id).get_obj_val(2) != LIQ_WATER &&  objs.get(obj_id).get_obj_val(1) != 0 {
            name_from_drinkcon(objs,Some(obj_id));
             objs.get_mut(obj_id).set_obj_val(2, LIQ_SLIME);
            name_to_drinkcon(objs,Some(obj_id), LIQ_SLIME);
        } else {
            let water = max( objs.get(obj_id).get_obj_val(0) -  objs.get(obj_id).get_obj_val(1), 0);
            if water > 0 {
                if  objs.get(obj_id).get_obj_val(1) >= 0 {
                    name_from_drinkcon( objs,Some(obj_id));
                }
                 objs.get_mut(obj_id).set_obj_val(2, LIQ_WATER);
                 let v = objs.get(obj_id).get_obj_val(1) + water;
                 objs.get_mut(obj_id).set_obj_val(1,v  );
                name_to_drinkcon( objs, Some(obj_id), LIQ_WATER);
                weight_change_object(chars, objs,obj_id, water);
                let ch = chars.get(chid);
                let obj = objs.get(obj_id);
                act(&mut game.descriptors, chars, db,"$p is filled.", false, Some(ch), Some(obj), None, TO_CHAR);
            }
        }
    }
}

pub fn spell_recall(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    _level: i32,
    _chid: Option<DepotId>,
    victim_id: Option<DepotId>,
    _obj: Option<DepotId>,
) {
    if victim_id.is_none() || chars.get(victim_id.unwrap()).is_npc() {
        return;
    }

    let victim_id = victim_id.unwrap();
    let victim = chars.get(victim_id);

    act(&mut game.descriptors, chars, db,"$n disappears.", true, Some(victim), None, None, TO_ROOM);
    let victim = chars.get_mut(victim_id);
    db.char_from_room( objs,victim);
    db.char_to_room(chars, objs,victim_id, db.r_mortal_start_room);
    let victim = chars.get(victim_id);
    act(&mut game.descriptors, chars, db,
        "$n appears in the middle of the room.",
        true,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    look_at_room(&mut game.descriptors, db, chars, texts,objs,victim, false);
}

pub fn spell_teleport(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    _level: i32,
    _chid: Option<DepotId>,
    victim_id: Option<DepotId>,
    _obj: Option<DepotId>,
) {
    let mut to_room;

    if victim_id.is_none() || chars.get(victim_id.unwrap()).is_npc() {
        return;
    }
    let victim_id = victim_id.unwrap();
    let victim = chars.get(victim_id);
    loop {
        to_room = rand_number(0, db.world.len() as u32);
        if !db.room_flagged(
            to_room as RoomRnum,
            RoomFlags::PRIVATE | RoomFlags::DEATH | RoomFlags::GODROOM,
        ) {
            break;
        }
    }

    act(&mut game.descriptors, chars, db,
        "$n slowly fades out of existence and is gone.",
        false,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    let victim = chars.get_mut(victim_id);
    db.char_from_room( objs,victim);
    db.char_to_room(chars, objs,victim_id, to_room as RoomRnum);
    let victim = chars.get(victim_id);
    act(&mut game.descriptors, chars, db,
        "$n slowly fades into existence.",
        false,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    look_at_room(&mut game.descriptors, db, chars, texts,objs,victim, false);
}

const SUMMON_FAIL: &str = "You failed.\r\n";

pub fn spell_summon(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    level: i32,
    chid: Option<DepotId>,
    victim_id: Option<DepotId>,
    _obj: Option<DepotId>,
) {
    if chid.is_none() || victim_id.is_none() {
        return;
    }
    let victim_id = victim_id.unwrap();
    let victim = chars.get(victim_id);

    let chid = chid.unwrap();
    let ch = chars.get(chid);
        if victim.get_level() > min((LVL_IMMORT - 1) as u8, (level + 3) as u8) {
        send_to_char(&mut game.descriptors, ch, SUMMON_FAIL);
        return;
    }

    if !PK_ALLOWED {
        if victim.mob_flagged(MOB_AGGRESSIVE) {
            act(&mut game.descriptors, chars, db,"As the words escape your lips and $N travels\r\nthrough time and space towards you, you realize that $E is\r\naggressive and might harm you, so you wisely send $M back.",
                   false, Some(ch), None, Some(VictimRef::Char(victim)), TO_CHAR);
            return;
        }
        if !victim.is_npc()
            && !victim.prf_flagged(PrefFlags::SUMMONABLE)
            && !victim.plr_flagged(PLR_KILLER)
        {
            send_to_char(&mut game.descriptors, victim, format!("{} just tried to summon you to: {}.\r\n{} failed because you have summon protection on.\r\nType NOSUMMON to allow other players to summon you.\r\n",
                                         ch.get_name(), db.world[ch.in_room() as usize].name,
                                         if ch.player.sex == Sex::Male { "He" } else { "She" }).as_str());
                                         let victim = chars.get(victim_id);
            send_to_char(&mut game.descriptors, ch,
                format!(
                    "You failed because {} has summon protection on.\r\n",
                    victim.get_name()
                )
                .as_str(),
            );
            let victim = chars.get(victim_id);
            let ch = chars.get(chid);
            game.mudlog(chars,
                BRF,
                LVL_IMMORT as i32,
                true,
                format!(
                    "{} failed summoning {} to {}.",
                    ch.get_name(),
                    victim.get_name(),
                    db.world[ch.in_room() as usize].name
                )
                .as_str(),
            );
            return;
        }
    }

    if victim.mob_flagged(MOB_NOSUMMON)
        || victim.is_npc() && mag_savingthrow(victim, SAVING_SPELL, 0)
    {
        send_to_char(&mut game.descriptors, ch, SUMMON_FAIL);
        return;
    }

    act(&mut game.descriptors, chars, db,
        "$n disappears suddenly.",
        true,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    let victim = chars.get_mut(victim_id);
    db.char_from_room(objs,victim);
    let ch = chars.get(chid);
    db.char_to_room(chars, objs,victim_id, ch.in_room());
    let victim = chars.get(victim_id);
    act(&mut game.descriptors, chars, db,
        "$n arrives suddenly.",
        true,
        Some(victim),
        None,
        None,
        TO_ROOM,
    );
    let ch = chars.get(chid);
    act(&mut game.descriptors, chars, db,
        "$n has summoned you!",
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(victim)),
        TO_VICT,
    );
    look_at_room(&mut game.descriptors, db,chars,texts,objs,victim, false);
}

pub fn spell_locate_object(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>, 
    level: i32,
    chid: Option<DepotId>,
    _victim_id: Option<DepotId>,
    oid: Option<DepotId>,
) {
    /*
     * FIXME: This is broken.  The spell parser routines took the argument
     * the player gave to the spell and located an object with that keyword.
     * Since we're passed the object and not the keyword we can only guess
     * at what the player originally meant to search for. -gg
     */
    let ch = chars.get(chid.unwrap());
    let oid = oid.unwrap();
    let mut name = String::new();
    name.push_str(& objs.get(oid).name);
    let mut j = level / 2;

    for &i in &db.object_list {
        if !isname(&name, objs.get(i).name.as_ref()) {
            continue;
        }

        send_to_char(&mut game.descriptors, ch,
            format!(
                "{}{}",
                &objs.get(i).short_description[0..0].to_uppercase(),
                &objs.get(i).short_description[1..]
            )
            .as_str(),
        );

        if objs.get(i).carried_by.is_some() {
            let msg = format!(
                " is being carried by {}.\r\n",
                pers(&game.descriptors, chars, db,chars.get(objs.get(i).carried_by.unwrap()), chars.get(chid.unwrap()))
            );
            send_to_char(&mut game.descriptors, ch,
                msg.as_str(),
            );
        } else if objs.get(i).in_room() != NOWHERE {
            send_to_char(&mut game.descriptors, ch,
                format!(
                    " is in {}.\r\n",
                    db.world[objs.get(i).in_room() as usize].name
                )
                .as_str(),
            );
        } else if objs.get(i).in_obj.is_some() {
            send_to_char(&mut game.descriptors, ch,
                format!(
                    " is in {}.\r\n",
                    objs.get(objs.get(i).in_obj.unwrap()).short_description
                )
                .as_str(),
            );
        } else if objs.get(i).worn_by.is_some() {
            let msg = format!(
                " is being worn by {}.\r\n",
                pers(&game.descriptors, chars, db,chars.get(objs.get(i).worn_by.unwrap()), chars.get(chid.unwrap()))
            );
            send_to_char(&mut game.descriptors, ch,
                msg.as_str(),
            );
        } else {
            send_to_char(&mut game.descriptors, ch, "'s location is uncertain.\r\n");
        }

        j -= 1;
    }

    if j == level / 2 {
        send_to_char(&mut game.descriptors, ch, "You sense nothing.\r\n");
    }
}

pub fn spell_charm(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>, 
    level: i32,
    chid: Option<DepotId>,
    victim_id: Option<DepotId>,
    _oid: Option<DepotId>,
) {
    if victim_id.is_none() || chid.is_none() {
        return;
    }
    let victim_id = victim_id.unwrap();
    let victim = chars.get(victim_id);

    let chid = chid.unwrap();
    let ch = chars.get(chid);

    if victim_id ==  chid {
        send_to_char(&mut game.descriptors, ch, "You like yourself even better!\r\n");
    } else if !victim.is_npc() && !victim.prf_flagged(PrefFlags::SUMMONABLE) {
        send_to_char(&mut game.descriptors, ch, "You fail because SUMMON protection is on!\r\n");
    } else if victim.aff_flagged(AffectFlags::SANCTUARY) {
        send_to_char(&mut game.descriptors, ch, "Your victim is protected by sanctuary!\r\n");
    } else if victim.mob_flagged(MOB_NOCHARM) {
        send_to_char(&mut game.descriptors, ch, "Your victim resists!\r\n");
    } else if ch.aff_flagged(AffectFlags::CHARM) {
        send_to_char(&mut game.descriptors, ch, "You can't have any followers of your own!\r\n");
    } else if victim.aff_flagged(AffectFlags::CHARM) || level < victim.get_level() as i32 {
        send_to_char(&mut game.descriptors, ch, "You fail.\r\n");
        /* player charming another player - no legal reason for this */
    } else if !PK_ALLOWED && !victim.is_npc() {
        send_to_char(&mut game.descriptors, ch, "You fail - shouldn't be doing it anyway.\r\n");
    } else if circle_follow(chars,  victim, Some(ch)) {
        send_to_char(&mut game.descriptors, ch, "Sorry, following in circles can not be allowed.\r\n");
    } else if mag_savingthrow(victim, SAVING_PARA, 0) {
        send_to_char(&mut game.descriptors, ch, "Your victim resists!\r\n");
    } else {
        if victim.master.is_some() {
            stop_follower(&mut game.descriptors, chars, db, objs,victim_id);
        }

        add_follower(&mut game.descriptors, chars, db, victim_id, chid);
        let mut af = AffectedType {
            _type: SPELL_CHARM as i16,
            duration: 24 * 2,
            modifier: 0,
            location: ApplyType::None,
            bitvector: AffectFlags::CHARM,
        };
        let ch = chars.get(chid);
        if ch.get_cha() != 0 {
            af.duration *= ch.get_cha() as i16;
        }
        let victim = chars.get_mut(victim_id);
        if victim.get_int() != 0 {
            af.duration /= victim.get_int() as i16;
        }
        affect_to_char( objs,victim, af);
        let victim = chars.get(victim_id);
        let ch = chars.get(chid);
        act(&mut game.descriptors, chars, db,
            "Isn't $n just such a nice fellow?",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(victim)),
            TO_VICT,
        );
        let victim = chars.get_mut(victim_id);
        if victim.is_npc() {
            victim.remove_mob_flags_bit(MOB_SPEC);
        }
    }
}

pub fn spell_identify(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>, 
    _level: i32,
    chid: Option<DepotId>,
    victim_id: Option<DepotId>,
    oid: Option<DepotId>,
) {
    let ch = chars.get(chid.unwrap());

    if oid.is_some() {
        let oid = oid.unwrap();
        let mut bitbuf = String::new();

        sprinttype(objs.get(oid).get_obj_type() as i32, &ITEM_TYPES, &mut bitbuf);
        send_to_char(&mut game.descriptors, ch,
            format!(
                "You feel informed:\r\nObject '{}', Item type: {}\r\n",
                objs.get(oid).short_description, bitbuf
            )
            .as_str(),
        );

        if !objs.get(oid).get_obj_affect().is_empty() {
            sprintbit(objs.get(oid).get_obj_affect().bits() as i64, &AFFECTED_BITS, &mut bitbuf);
            send_to_char(&mut game.descriptors, ch,
                format!("Item will give you following abilities:  %{}\r\n", bitbuf).as_str(),
            );
        }

        sprintbit(objs.get(oid).get_obj_extra().bits() as i64, &EXTRA_BITS, &mut bitbuf);
        send_to_char(&mut game.descriptors, ch, format!("Item is: {}\r\n", bitbuf).as_str());

        send_to_char(&mut game.descriptors, ch,
            format!(
                "Weight: {}, Value: {}, Rent: {}\r\n",
                objs.get(oid).get_obj_weight(),
                objs.get(oid).get_obj_cost(),
                objs.get(oid).get_obj_rent()
            )
            .as_str(),
        );

        match objs.get(oid).get_obj_type() {
            ItemType::Scroll | ItemType::Potion => {
                if objs.get(oid).get_obj_val(1) >= 1 {
                    bitbuf.push_str(skill_name(&db, objs.get(oid).get_obj_val(1)));
                }

                if objs.get(oid).get_obj_val(2) >= 1 {
                    bitbuf.push_str(skill_name(&db, objs.get(oid).get_obj_val(2)));
                }

                if objs.get(oid).get_obj_val(3) >= 1 {
                    bitbuf.push_str(skill_name(&db, objs.get(oid).get_obj_val(3)));
                }

                send_to_char(&mut game.descriptors, ch,
                    format!(
                        "This {} casts: {}\r\n",
                        ITEM_TYPES[objs.get(oid).get_obj_type() as usize],
                        bitbuf
                    )
                    .as_str(),
                );
            }
            ItemType::Wand | ItemType::Staff => {
                send_to_char(&mut game.descriptors, ch,
                    format!(
                        "This {} casts: {}\r\nIt has {} maximum charge{} and {} remaining.\r\n",
                        ITEM_TYPES[objs.get(oid).get_obj_type() as usize],
                        skill_name(&db, objs.get(oid).get_obj_val(3)),
                        objs.get(oid).get_obj_val(1),
                        if objs.get(oid).get_obj_val(1) == 1 { "" } else { "s" },
                        objs.get(oid).get_obj_val(2)
                    )
                    .as_str(),
                );
            }
            ItemType::Weapon => {
                send_to_char(&mut game.descriptors, ch,
                    format!(
                        "Damage Dice is '{}D{}' for an average per-round damage of {}.\r\n",
                        objs.get(oid).get_obj_val(1),
                        objs.get(oid).get_obj_val(2),
                        (objs.get(oid).get_obj_val(2) + 1) * objs.get(oid).get_obj_val(1) / 2
                    )
                    .as_str(),
                );
            }
            ItemType::Armor => {
                send_to_char(&mut game.descriptors, ch,
                    format!("AC-apply is {}\r\n", objs.get(oid).get_obj_val(0)).as_str(),
                );
            }
            _ => {}
        }
        let mut found = false;
        for i in 0..MAX_OBJ_AFFECT as usize {
            if objs.get(oid).affected[i].location != ApplyType::None
                && objs.get(oid).affected[i].modifier != 0
            {
                if !found {
                    send_to_char(&mut game.descriptors, ch, "Can affect you as :\r\n");
                    found = true;
                }
                sprinttype(
                    objs.get(oid).affected[i].location as i32,
                    &APPLY_TYPES,
                    &mut bitbuf,
                );
                send_to_char(&mut game.descriptors, ch,
                    format!(
                        "   Affects: {} By {}\r\n",
                        bitbuf,
                        objs.get(oid).affected[i].modifier
                    )
                    .as_str(),
                );
            }
        }
    } else if victim_id.is_some() {
        /* victim */
        let victim_id = victim_id.unwrap();
        let victim = chars.get(victim_id);
        send_to_char(&mut game.descriptors, ch,
            format!("Name: {}\r\n", victim.get_name()).as_str(),
        );
        let victim = chars.get(victim_id);
        if !victim.is_npc() {
            send_to_char(&mut game.descriptors, ch,
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
            let victim = chars.get(victim_id);
            send_to_char(&mut game.descriptors, ch,
                format!(
                    "Height {} cm, Weight {} pounds\r\n",
                    victim.get_height(),
                    victim.get_weight()
                )
                .as_str(),
            );
            let victim = chars.get(victim_id);
            send_to_char(&mut game.descriptors, ch,
                format!(
                    "Level: {}, Hits: {}, Mana: {}\r\n",
                    victim.get_level(),
                    victim.get_hit(),
                    victim.get_mana()
                )
                .as_str(),
            );
            let victim = chars.get(victim_id);
            send_to_char(&mut game.descriptors, ch,
                format!(
                    "AC: {}, Hitroll: {}, Damroll: {}\r\n",
                    compute_armor_class(victim),
                    victim.get_hitroll(),
                    victim.get_damroll()
                )
                .as_str(),
            );
            let victim = chars.get(victim_id);
            send_to_char(&mut game.descriptors, ch,
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
pub fn spell_enchant_weapon(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>, 
    level: i32,
    chid: Option<DepotId>,
    _victim_id: Option<DepotId>,
    oid: Option<DepotId>,
) {
    if chid.is_none() || oid.is_none() {
        return;
    }
 let chid = chid.unwrap();
        let oid = oid.unwrap();

    /* Either already enchanted or not a weapon. */
    let obj = objs.get_mut(oid);
    if obj.get_obj_type() != ItemType::Weapon || obj.obj_flagged(ExtraFlags::MAGIC) {
        return;
    }

    /* Make sure no other affections. */
    for i in 0..MAX_OBJ_AFFECT as usize {
        if obj.affected[i].location != ApplyType::None {
            return;
        }
    }

    obj.set_obj_extra_bit(ExtraFlags::MAGIC);

    let mut af0 = obj.affected[0];
    af0.location = ApplyType::Hitroll;
    af0.modifier = 1 + if level >= 18 { 1 } else { 0 };
    obj.affected[0] = af0;

    let mut af1 = obj.affected[1];
    af1.location = ApplyType::Damroll;
    af1.modifier = 1 + if level >= 20 { 1 } else { 0 };
    obj.affected[1] = af1;
    let ch = chars.get(chid);
    if ch.is_good() {
        let obj = objs.get_mut(oid);
        obj.set_obj_extra_bit(ExtraFlags::ANTI_EVIL);
        let ch = chars.get(chid);
        let obj=objs.get(oid);
        act(&mut game.descriptors, chars, db,"$p glows blue.", false, Some(ch), Some(obj), None, TO_CHAR);
    } else if ch.is_evil() {
        let obj = objs.get_mut(oid);
        obj.set_obj_extra_bit(ExtraFlags::ANTI_GOOD);
        let ch = chars.get(chid);
        let obj=objs.get(oid);
        act(&mut game.descriptors, chars, db,"$p glows red.", false, Some(ch), Some(obj), None, TO_CHAR);
    } else {
        let obj=objs.get(oid);
        act(&mut game.descriptors, chars, db,
            "$p glows yellow.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
    }
}

pub fn spell_detect_poison(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>, 
    _level: i32,
    chid: Option<DepotId>,
    victim_id: Option<DepotId>,
    oid: Option<DepotId>,
) {
    if victim_id.is_some() {
        let victim_id = victim_id.unwrap();
        let victim = chars.get(victim_id);
        let chid = chid.unwrap();
        let ch = chars.get(chid);
        if chid == victim_id {
            if victim.aff_flagged(AffectFlags::POISON) {
                send_to_char(&mut game.descriptors, ch, "You can sense poison in your blood.\r\n");
            } else {
                send_to_char(&mut game.descriptors, ch, "You feel healthy.\r\n");
            }
        } else {
            if victim.aff_flagged(AffectFlags::POISON) {
                act(&mut game.descriptors, chars, db,
                    "You sense that $E is poisoned.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(victim)),
                    TO_CHAR,
                );
            } else {
                act(&mut game.descriptors, chars, db,
                    "You sense that $E is healthy.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(victim)),
                    TO_CHAR,
                );
            }
        }

        if oid.is_some() {
            let oid = oid.unwrap();
            let obj = objs.get(oid);
            match obj.get_obj_type() {
                ItemType::Drinkcon | ItemType::Fountain | ItemType::Food => {
                    if obj.get_obj_val(3) != 0 {
                        act(&mut game.descriptors, chars, db,
                            "You sense that $p has been contaminated.",
                            false,
                            Some(ch),
                            Some(obj),
                            None,
                            TO_CHAR,
                        );
                    } else {
                        act(&mut game.descriptors, chars, db,
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
                    send_to_char(&mut game.descriptors, ch, "You sense that it should not be consumed.\r\n");
                }
            }
        }
    }
}
