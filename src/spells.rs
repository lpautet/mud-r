/* ************************************************************************
*   File: spells.h                                      Part of CircleMUD *
*  Usage: header file: constants and fn prototypes for spell system       *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */
//
// #define DEFAULT_STAFF_LVL	12
// #define DEFAULT_WAND_LVL	12
//
// #define CAST_UNDEFINED	(-1)
// #define CAST_SPELL	0
// #define CAST_POTION	1
// #define CAST_WAND	2
// #define CAST_STAFF	3
// #define CAST_SCROLL	4
//
// #define MAG_DAMAGE	(1 << 0)
// #define MAG_AFFECTS	(1 << 1)
// #define MAG_UNAFFECTS	(1 << 2)
// #define MAG_POINTS	(1 << 3)
// #define MAG_ALTER_OBJS	(1 << 4)
// #define MAG_GROUPS	(1 << 5)
// #define MAG_MASSES	(1 << 6)
// #define MAG_AREAS	(1 << 7)
// #define MAG_SUMMONS	(1 << 8)
// #define MAG_CREATIONS	(1 << 9)
// #define MAG_MANUAL	(1 << 10)
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
pub const SPELL_VENTRILOQUATE: i32 = 41; /* Reserved Skill[] DO NOT CHANGE */
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
// #define MAX_SPELLS		    130
//
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
//
//
// /*
//  *  NON-PLAYER AND OBJECT SPELLS AND SKILLS
//  *  The practice levels for the spells and skills below are _not_ recorded
//  *  in the playerfile; therefore, the intended use is for spells and skills
//  *  associated with objects (such as SPELL_IDENTIFY used with scrolls of
//  *  identify) or non-players (such as NPC-only spells).
//  */
//
// #define SPELL_IDENTIFY               201
// #define SPELL_FIRE_BREATH            202
// #define SPELL_GAS_BREATH             203
// #define SPELL_FROST_BREATH           204
// #define SPELL_ACID_BREATH            205
// #define SPELL_LIGHTNING_BREATH       206
//
// #define TOP_SPELL_DEFINE	     299
// /* NEW NPC/OBJECT SPELLS can be inserted here up to 299 */
/* WEAPON ATTACK TYPES */

pub const TYPE_HIT: i32 = 300;
pub const TYPE_STING: i32 = 301;
pub const TYPE_WHIP: i32 = 302;
pub const TYPE_SLASH: i32 = 303;
pub const TYPE_BITE: i32 = 304;
pub const TYPE_BLUDGEON: i32 = 305;
pub const TYPE_CRUSH: i32 = 306;
pub const TYPE_POUND: i32 = 307;
pub const TYPE_CLAW: i32 = 308;
pub const TYPE_MAUL: i32 = 309;
pub const TYPE_THRASH: i32 = 310;
pub const TYPE_PIERCE: i32 = 311;
pub const TYPE_BLAST: i32 = 312;
pub const TYPE_PUNCH: i32 = 313;
pub const TYPE_STAB: i32 = 314;

// /* new attack types can be added here - up to TYPE_SUFFERING */
pub const TYPE_SUFFERING: i32 = 399;

pub const SAVING_PARA: i32 = 0;
pub const SAVING_ROD: i32 = 1;
pub const SAVING_PETRI: i32 = 2;
pub const SAVING_BREATH: i32 = 3;
pub const SAVING_SPELL: i32 = 4;

// #define TAR_IGNORE      (1 << 0)
// #define TAR_CHAR_ROOM   (1 << 1)
// #define TAR_CHAR_WORLD  (1 << 2)
// #define TAR_FIGHT_SELF  (1 << 3)
// #define TAR_FIGHT_VICT  (1 << 4)
// #define TAR_SELF_ONLY   (1 << 5) /* Only a check, use with i.e. TAR_CHAR_ROOM */
// #define TAR_NOT_SELF   	(1 << 6) /* Only a check, use with i.e. TAR_CHAR_ROOM */
// #define TAR_OBJ_INV     (1 << 7)
// #define TAR_OBJ_ROOM    (1 << 8)
// #define TAR_OBJ_WORLD   (1 << 9)
// #define TAR_OBJ_EQUIP	(1 << 10)
//
// struct spell_info_type {
//     byte min_position;	/* Position for caster	 */
//     int mana_min;	/* Min amount of mana used by a spell (highest lev) */
//     int mana_max;	/* Max amount of mana used by a spell (lowest lev) */
//     int mana_change;	/* Change in mana used by spell from lev to lev */
//
//     int min_level[NUM_CLASSES];
//     int routines;
//     byte violent;
//     int targets;         /* See below for use with TAR_XXX  */
//     const char *name;	/* Input size not limited. Originates from string constants. */
//     const char *wear_off_msg;	/* Input size not limited. Originates from string constants. */
// };
//
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
