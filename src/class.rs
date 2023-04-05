/* ************************************************************************
*   File: class.c                                       Part of CircleMUD *
*  Usage: Source file for class-specific code                             *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

/*
 * This file attempts to concentrate most of the code which must be changed
 * in order for new classes to be added.  If you're adding a new class,
 * you should go through this entire file from beginning to end and add
 * the appropriate new special cases for your new class.
 */

/* Names first */

use std::cell::RefCell;
use std::cmp::{max, min};

use log::{error, info};

use crate::constants::{CON_APP, WIS_APP};
use crate::db::DB;
use crate::interpreter::{SCMD_EAST, SCMD_NORTH, SCMD_SOUTH, SCMD_WEST};
use crate::spell_parser::spell_level;
use crate::spells::{
    MAG_AFFECTS, MAG_ALTER_OBJS, MAG_AREAS, MAG_CREATIONS, MAG_DAMAGE, MAG_GROUPS, MAG_MANUAL,
    MAG_POINTS, MAG_SUMMONS, MAG_UNAFFECTS, SKILL_BACKSTAB, SKILL_BASH, SKILL_HIDE, SKILL_KICK,
    SKILL_PICK_LOCK, SKILL_RESCUE, SKILL_SNEAK, SKILL_STEAL, SKILL_TRACK, SPELL_ACID_BREATH,
    SPELL_ANIMATE_DEAD, SPELL_ARMOR, SPELL_BLESS, SPELL_BLINDNESS, SPELL_BURNING_HANDS,
    SPELL_CALL_LIGHTNING, SPELL_CHARM, SPELL_CHILL_TOUCH, SPELL_CLONE, SPELL_COLOR_SPRAY,
    SPELL_CONTROL_WEATHER, SPELL_CREATE_FOOD, SPELL_CREATE_WATER, SPELL_CURE_BLIND,
    SPELL_CURE_CRITIC, SPELL_CURE_LIGHT, SPELL_CURSE, SPELL_DETECT_ALIGN, SPELL_DETECT_INVIS,
    SPELL_DETECT_MAGIC, SPELL_DETECT_POISON, SPELL_DISPEL_EVIL, SPELL_DISPEL_GOOD,
    SPELL_EARTHQUAKE, SPELL_ENCHANT_WEAPON, SPELL_ENERGY_DRAIN, SPELL_FIREBALL, SPELL_FIRE_BREATH,
    SPELL_FROST_BREATH, SPELL_GAS_BREATH, SPELL_GROUP_ARMOR, SPELL_GROUP_HEAL, SPELL_HARM,
    SPELL_HEAL, SPELL_IDENTIFY, SPELL_INFRAVISION, SPELL_INVISIBLE, SPELL_LIGHTNING_BOLT,
    SPELL_LIGHTNING_BREATH, SPELL_LOCATE_OBJECT, SPELL_MAGIC_MISSILE, SPELL_POISON,
    SPELL_PROT_FROM_EVIL, SPELL_REMOVE_CURSE, SPELL_REMOVE_POISON, SPELL_SANCTUARY,
    SPELL_SENSE_LIFE, SPELL_SHOCKING_GRASP, SPELL_SLEEP, SPELL_STRENGTH, SPELL_SUMMON,
    SPELL_TELEPORT, SPELL_WATERWALK, SPELL_WORD_OF_RECALL, TAR_CHAR_ROOM, TAR_CHAR_WORLD,
    TAR_FIGHT_VICT, TAR_IGNORE, TAR_NOT_SELF, TAR_OBJ_EQUIP, TAR_OBJ_INV, TAR_OBJ_ROOM,
    TAR_OBJ_WORLD, TAR_SELF_ONLY, TOP_SPELL_DEFINE,
};
use crate::structs::{
    CharData, GuildInfoType, ObjData, CLASS_CLERIC, CLASS_MAGIC_USER, CLASS_THIEF, CLASS_UNDEFINED,
    CLASS_WARRIOR, DRUNK, FULL, ITEM_ANTI_CLERIC, ITEM_ANTI_MAGIC_USER, ITEM_ANTI_THIEF,
    ITEM_ANTI_WARRIOR, LVL_GOD, LVL_GRGOD, LVL_IMMORT, LVL_IMPL, NOWHERE, NUM_CLASSES,
    PRF_HOLYLIGHT, THIRST,
};
use crate::util::{rand_number, BRF};
use crate::{check_player_special, set_skill, MainGlobals};

const CLASS_ABBREVS: [&str; 4] = ["Mu", "Cl", "Th", "Wa"];

const PC_CLASS_TYPES: [&str; 4] = ["Magic User", "Cleric", "Thief", "Warrior"];

/* The menu for choosing a class in interpreter.c: */
pub const CLASS_MENU: &str = "\r\n\
     Select a class:\r\n\
        [C]leric\r\n\
        [T]hief\r\n\
        [W]arrior\r\n\
        [M]agic-user\r\n";

/*
 * The code to interpret a class letter -- used in interpreter.c when a
 * new character is selecting a class and by 'set class' in act.wizard.c.
 */

pub fn parse_class(arg: char) -> i8 {
    let arg = arg.to_lowercase().last().unwrap();

    return match arg {
        'm' => CLASS_MAGIC_USER,
        'c' => CLASS_CLERIC,
        'w' => CLASS_WARRIOR,
        't' => CLASS_THIEF,
        _ => CLASS_UNDEFINED,
    };
}

/*
 * bitvectors (i.e., powers of two) for each class, mainly for use in
 * do_who and do_users.  Add new classes at the end so that all classes
 * use sequential powers of two (1 << 0, 1 << 1, 1 << 2, 1 << 3, 1 << 4,
 * 1 << 5, etc.) up to the limit of your bitvector_t, typically 0-31.
 */
// bitvector_t find_class_bitvector(const char *arg)
// {
// size_t rpos, ret = 0;
//
// for (rpos = 0; rpos < strlen(arg); rpos++)
// ret |= (1 << parse_class(arg[rpos]));
//
// return (ret);
// }

/*
 * These are definitions which control the guildmasters for each class.
 *
 * The first field (top line) controls the highest percentage skill level
 * a character of the class is allowed to attain in any skill.  (After
 * this level, attempts to practice will say "You are already learned in
 * this area."
 *
 * The second line controls the maximum percent gain in learnedness a
 * character is allowed per practice -- in other words, if the random
 * die throw comes out higher than this number, the gain will only be
 * this number instead.
 *
 * The third line controls the minimu percent gain in learnedness a
 * character is allowed per practice -- in other words, if the random
 * die throw comes out below this number, the gain will be set up to
 * this number.
 *
 * The fourth line simply sets whether the character knows 'spells'
 * or 'skills'.  This does not affect anything except the message given
 * to the character when trying to practice (i.e. "You know of the
 * following spells" vs. "You know of the following skills"
 */

const SPELL: i32 = 0;
const SKILL: i32 = 1;

/* #define LEARNED_LEVEL	0  % known which is considered "learned" */
/* #define MAX_PER_PRAC		1  max percent gain in skill per practice */
/* #define min_PER_PRAC		2  min percent gain in skill per practice */
/* #define PRAC_TYPE		3  should it say 'spell' or 'skill'?	*/

pub const PRAC_PARAMS: [[i32; 4]; NUM_CLASSES as usize] = [
    /* MAG	CLE	THE	WAR */
    [95, 95, 85, 80],             /* learned level */
    [100, 100, 12, 12],           /* max per practice */
    [25, 25, 0, 0],               /* min per practice */
    [SPELL, SPELL, SKILL, SKILL], /* prac name */
];

/*
 * ...And the appropriate rooms for each guildmaster/guildguard; controls
 * which types of people the various guildguards let through.  i.e., the
 * first line shows that from room 3017, only MAGIC_USERS are allowed
 * to go south.
 *
 * Don't forget to visit spec_assign.c if you create any new mobiles that
 * should be a guild master or guard so they can act appropriately. If you
 * "recycle" the existing mobs that are used in other guilds for your new
 * guild, then you don't have to change that file, only here.
 */
pub const GUILD_INFO: [GuildInfoType; 6] = [
    /* Midgaard */
    GuildInfoType {
        pc_class: CLASS_MAGIC_USER,
        guild_room: 3017,
        direction: SCMD_SOUTH,
    },
    GuildInfoType {
        pc_class: CLASS_CLERIC,
        guild_room: 3004,
        direction: SCMD_NORTH,
    },
    GuildInfoType {
        pc_class: CLASS_THIEF,
        guild_room: 3027,
        direction: SCMD_EAST,
    },
    GuildInfoType {
        pc_class: CLASS_WARRIOR,
        guild_room: 3021,
        direction: SCMD_EAST,
    },
    /* Brass Dragon */
    GuildInfoType {
        pc_class: -127, /* all */
        guild_room: 5065,
        direction: SCMD_WEST,
    },
    /* this must go last -- add new guards above! */
    GuildInfoType {
        pc_class: -1,
        guild_room: NOWHERE,
        direction: -1,
    },
];

/*
 * Saving throws for:
 * MCTW
 *   PARA, ROD, PETRI, BREATH, SPELL
 *     Levels 0-40
 *
 * Do not forget to change extern declaration in magic.c if you add to this.
 */

// byte saving_throws(int class_num, int type, int level)
// {
// switch (class_num) {
// CLASS_MAGIC_USER => { // switch (type) { }
// SAVING_PARA => { /* Paralyzation */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 70; }
// 2 => { return 69; }
// 3 => { return 68; }
// 4 => { return 67; }
// 5 => { return 66; }
// 6 => { return 65; }
// 7 => { return 63; }
// 8 => { return 61; }
// 9 => { return 60; }
// 10 => { return 59; }
// 11 => { return 57; }
// 12 => { return 55; }
// 13 => { return 54; }
// 14 => { return 53; }
// 15 => { return 53; }
// 16 => { return 52; }
// 17 => { return 51; }
// 18 => { return 50; }
// 19 => { return 48; }
// 20 => { return 46; }
// 21 => { return 45; }
// 22 => { return 44; }
// 23 => { return 42; }
// 24 => { return 40; }
// 25 => { return 38; }
// 26 => { return 36; }
// 27 => { return 34; }
// 28 => { return 32; }
// 29 => { return 30; }
// 30 => { return 28; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for mage paralyzation saving throw.");
// break;
// }
// SAVING_ROD => { /* Rods */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 55; }
// 2 => { return 53; }
// 3 => { return 51; }
// 4 => { return 49; }
// 5 => { return 47; }
// 6 => { return 45; }
// 7 => { return 43; }
// 8 => { return 41; }
// 9 => { return 40; }
// 10 => { return 39; }
// 11 => { return 37; }
// 12 => { return 35; }
// 13 => { return 33; }
// 14 => { return 31; }
// 15 => { return 30; }
// 16 => { return 29; }
// 17 => { return 27; }
// 18 => { return 25; }
// 19 => { return 23; }
// 20 => { return 21; }
// 21 => { return 20; }
// 22 => { return 19; }
// 23 => { return 17; }
// 24 => { return 15; }
// 25 => { return 14; }
// 26 => { return 13; }
// 27 => { return 12; }
// 28 => { return 11; }
// 29 => { return 10; }
// 30 => { return  9; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for mage rod saving throw.");
// break;
// }
// SAVING_PETRI => { /* Petrification */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 65; }
// 2 => { return 63; }
// 3 => { return 61; }
// 4 => { return 59; }
// 5 => { return 57; }
// 6 => { return 55; }
// 7 => { return 53; }
// 8 => { return 51; }
// 9 => { return 50; }
// 10 => { return 49; }
// 11 => { return 47; }
// 12 => { return 45; }
// 13 => { return 43; }
// 14 => { return 41; }
// 15 => { return 40; }
// 16 => { return 39; }
// 17 => { return 37; }
// 18 => { return 35; }
// 19 => { return 33; }
// 20 => { return 31; }
// 21 => { return 30; }
// 22 => { return 29; }
// 23 => { return 27; }
// 24 => { return 25; }
// 25 => { return 23; }
// 26 => { return 21; }
// 27 => { return 19; }
// 28 => { return 17; }
// 29 => { return 15; }
// 30 => { return 13; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for mage petrification saving throw.");
// break;
// }
// SAVING_BREATH => { /* Breath weapons */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 75; }
// 2 => { return 73; }
// 3 => { return 71; }
// 4 => { return 69; }
// 5 => { return 67; }
// 6 => { return 65; }
// 7 => { return 63; }
// 8 => { return 61; }
// 9 => { return 60; }
// 10 => { return 59; }
// 11 => { return 57; }
// 12 => { return 55; }
// 13 => { return 53; }
// 14 => { return 51; }
// 15 => { return 50; }
// 16 => { return 49; }
// 17 => { return 47; }
// 18 => { return 45; }
// 19 => { return 43; }
// 20 => { return 41; }
// 21 => { return 40; }
// 22 => { return 39; }
// 23 => { return 37; }
// 24 => { return 35; }
// 25 => { return 33; }
// 26 => { return 31; }
// 27 => { return 29; }
// 28 => { return 27; }
// 29 => { return 25; }
// 30 => { return 23; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for mage breath saving throw.");
// break;
// }
// SAVING_SPELL => { /* Generic spells */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 60; }
// 2 => { return 58; }
// 3 => { return 56; }
// 4 => { return 54; }
// 5 => { return 52; }
// 6 => { return 50; }
// 7 => { return 48; }
// 8 => { return 46; }
// 9 => { return 45; }
// 10 => { return 44; }
// 11 => { return 42; }
// 12 => { return 40; }
// 13 => { return 38; }
// 14 => { return 36; }
// 15 => { return 35; }
// 16 => { return 34; }
// 17 => { return 32; }
// 18 => { return 30; }
// 19 => { return 28; }
// 20 => { return 26; }
// 21 => { return 25; }
// 22 => { return 24; }
// 23 => { return 22; }
// 24 => { return 20; }
// 25 => { return 18; }
// 26 => { return 16; }
// 27 => { return 14; }
// 28 => { return 12; }
// 29 => { return 10; }
// 30 => { return  8; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for mage spell saving throw.");
// break;
// }
// default:
// log("SYSERR: Invalid saving throw type.");
// break;
// }
// break;
// CLASS_CLERIC => { // switch (type) { }
// SAVING_PARA => { /* Paralyzation */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 60; }
// 2 => { return 59; }
// 3 => { return 48; }
// 4 => { return 46; }
// 5 => { return 45; }
// 6 => { return 43; }
// 7 => { return 40; }
// 8 => { return 37; }
// 9 => { return 35; }
// 10 => { return 34; }
// 11 => { return 33; }
// 12 => { return 31; }
// 13 => { return 30; }
// 14 => { return 29; }
// 15 => { return 27; }
// 16 => { return 26; }
// 17 => { return 25; }
// 18 => { return 24; }
// 19 => { return 23; }
// 20 => { return 22; }
// 21 => { return 21; }
// 22 => { return 20; }
// 23 => { return 18; }
// 24 => { return 15; }
// 25 => { return 14; }
// 26 => { return 12; }
// 27 => { return 10; }
// 28 => { return  9; }
// 29 => { return  8; }
// 30 => { return  7; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for cleric paralyzation saving throw.");
// break;
// }
// SAVING_ROD => { /* Rods */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 70; }
// 2 => { return 69; }
// 3 => { return 68; }
// 4 => { return 66; }
// 5 => { return 65; }
// 6 => { return 63; }
// 7 => { return 60; }
// 8 => { return 57; }
// 9 => { return 55; }
// 10 => { return 54; }
// 11 => { return 53; }
// 12 => { return 51; }
// 13 => { return 50; }
// 14 => { return 49; }
// 15 => { return 47; }
// 16 => { return 46; }
// 17 => { return 45; }
// 18 => { return 44; }
// 19 => { return 43; }
// 20 => { return 42; }
// 21 => { return 41; }
// 22 => { return 40; }
// 23 => { return 38; }
// 24 => { return 35; }
// 25 => { return 34; }
// 26 => { return 32; }
// 27 => { return 30; }
// 28 => { return 29; }
// 29 => { return 28; }
// 30 => { return 27; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for cleric rod saving throw.");
// break;
// }
// SAVING_PETRI => { /* Petrification */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 65; }
// 2 => { return 64; }
// 3 => { return 63; }
// 4 => { return 61; }
// 5 => { return 60; }
// 6 => { return 58; }
// 7 => { return 55; }
// 8 => { return 53; }
// 9 => { return 50; }
// 10 => { return 49; }
// 11 => { return 48; }
// 12 => { return 46; }
// 13 => { return 45; }
// 14 => { return 44; }
// 15 => { return 43; }
// 16 => { return 41; }
// 17 => { return 40; }
// 18 => { return 39; }
// 19 => { return 38; }
// 20 => { return 37; }
// 21 => { return 36; }
// 22 => { return 35; }
// 23 => { return 33; }
// 24 => { return 31; }
// 25 => { return 29; }
// 26 => { return 27; }
// 27 => { return 25; }
// 28 => { return 24; }
// 29 => { return 23; }
// 30 => { return 22; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for cleric petrification saving throw.");
// break;
// }
// SAVING_BREATH => { /* Breath weapons */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 80; }
// 2 => { return 79; }
// 3 => { return 78; }
// 4 => { return 76; }
// 5 => { return 75; }
// 6 => { return 73; }
// 7 => { return 70; }
// 8 => { return 67; }
// 9 => { return 65; }
// 10 => { return 64; }
// 11 => { return 63; }
// 12 => { return 61; }
// 13 => { return 60; }
// 14 => { return 59; }
// 15 => { return 57; }
// 16 => { return 56; }
// 17 => { return 55; }
// 18 => { return 54; }
// 19 => { return 53; }
// 20 => { return 52; }
// 21 => { return 51; }
// 22 => { return 50; }
// 23 => { return 48; }
// 24 => { return 45; }
// 25 => { return 44; }
// 26 => { return 42; }
// 27 => { return 40; }
// 28 => { return 39; }
// 29 => { return 38; }
// 30 => { return 37; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for cleric breath saving throw.");
// break;
// }
// SAVING_SPELL => { /* Generic spells */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 75; }
// 2 => { return 74; }
// 3 => { return 73; }
// 4 => { return 71; }
// 5 => { return 70; }
// 6 => { return 68; }
// 7 => { return 65; }
// 8 => { return 63; }
// 9 => { return 60; }
// 10 => { return 59; }
// 11 => { return 58; }
// 12 => { return 56; }
// 13 => { return 55; }
// 14 => { return 54; }
// 15 => { return 53; }
// 16 => { return 51; }
// 17 => { return 50; }
// 18 => { return 49; }
// 19 => { return 48; }
// 20 => { return 47; }
// 21 => { return 46; }
// 22 => { return 45; }
// 23 => { return 43; }
// 24 => { return 41; }
// 25 => { return 39; }
// 26 => { return 37; }
// 27 => { return 35; }
// 28 => { return 34; }
// 29 => { return 33; }
// 30 => { return 32; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for cleric spell saving throw.");
// break;
// }
// default:
// log("SYSERR: Invalid saving throw type.");
// break;
// }
// break;
// CLASS_THIEF => { // switch (type) { }
// SAVING_PARA => { /* Paralyzation */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 65; }
// 2 => { return 64; }
// 3 => { return 63; }
// 4 => { return 62; }
// 5 => { return 61; }
// 6 => { return 60; }
// 7 => { return 59; }
// 8 => { return 58; }
// 9 => { return 57; }
// 10 => { return 56; }
// 11 => { return 55; }
// 12 => { return 54; }
// 13 => { return 53; }
// 14 => { return 52; }
// 15 => { return 51; }
// 16 => { return 50; }
// 17 => { return 49; }
// 18 => { return 48; }
// 19 => { return 47; }
// 20 => { return 46; }
// 21 => { return 45; }
// 22 => { return 44; }
// 23 => { return 43; }
// 24 => { return 42; }
// 25 => { return 41; }
// 26 => { return 40; }
// 27 => { return 39; }
// 28 => { return 38; }
// 29 => { return 37; }
// 30 => { return 36; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for thief paralyzation saving throw.");
// break;
// }
// SAVING_ROD => { /* Rods */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 70; }
// 2 => { return 68; }
// 3 => { return 66; }
// 4 => { return 64; }
// 5 => { return 62; }
// 6 => { return 60; }
// 7 => { return 58; }
// 8 => { return 56; }
// 9 => { return 54; }
// 10 => { return 52; }
// 11 => { return 50; }
// 12 => { return 48; }
// 13 => { return 46; }
// 14 => { return 44; }
// 15 => { return 42; }
// 16 => { return 40; }
// 17 => { return 38; }
// 18 => { return 36; }
// 19 => { return 34; }
// 20 => { return 32; }
// 21 => { return 30; }
// 22 => { return 28; }
// 23 => { return 26; }
// 24 => { return 24; }
// 25 => { return 22; }
// 26 => { return 20; }
// 27 => { return 18; }
// 28 => { return 16; }
// 29 => { return 14; }
// 30 => { return 13; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for thief rod saving throw.");
// break;
// }
// SAVING_PETRI => { /* Petrification */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 60; }
// 2 => { return 59; }
// 3 => { return 58; }
// 4 => { return 58; }
// 5 => { return 56; }
// 6 => { return 55; }
// 7 => { return 54; }
// 8 => { return 53; }
// 9 => { return 52; }
// 10 => { return 51; }
// 11 => { return 50; }
// 12 => { return 49; }
// 13 => { return 48; }
// 14 => { return 47; }
// 15 => { return 46; }
// 16 => { return 45; }
// 17 => { return 44; }
// 18 => { return 43; }
// 19 => { return 42; }
// 20 => { return 41; }
// 21 => { return 40; }
// 22 => { return 39; }
// 23 => { return 38; }
// 24 => { return 37; }
// 25 => { return 36; }
// 26 => { return 35; }
// 27 => { return 34; }
// 28 => { return 33; }
// 29 => { return 32; }
// 30 => { return 31; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for thief petrification saving throw.");
// break;
// }
// SAVING_BREATH => { /* Breath weapons */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 80; }
// 2 => { return 79; }
// 3 => { return 78; }
// 4 => { return 77; }
// 5 => { return 76; }
// 6 => { return 75; }
// 7 => { return 74; }
// 8 => { return 73; }
// 9 => { return 72; }
// 10 => { return 71; }
// 11 => { return 70; }
// 12 => { return 69; }
// 13 => { return 68; }
// 14 => { return 67; }
// 15 => { return 66; }
// 16 => { return 65; }
// 17 => { return 64; }
// 18 => { return 63; }
// 19 => { return 62; }
// 20 => { return 61; }
// 21 => { return 60; }
// 22 => { return 59; }
// 23 => { return 58; }
// 24 => { return 57; }
// 25 => { return 56; }
// 26 => { return 55; }
// 27 => { return 54; }
// 28 => { return 53; }
// 29 => { return 52; }
// 30 => { return 51; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for thief breath saving throw.");
// break;
// }
// SAVING_SPELL => { /* Generic spells */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 75; }
// 2 => { return 73; }
// 3 => { return 71; }
// 4 => { return 69; }
// 5 => { return 67; }
// 6 => { return 65; }
// 7 => { return 63; }
// 8 => { return 61; }
// 9 => { return 59; }
// 10 => { return 57; }
// 11 => { return 55; }
// 12 => { return 53; }
// 13 => { return 51; }
// 14 => { return 49; }
// 15 => { return 47; }
// 16 => { return 45; }
// 17 => { return 43; }
// 18 => { return 41; }
// 19 => { return 39; }
// 20 => { return 37; }
// 21 => { return 35; }
// 22 => { return 33; }
// 23 => { return 31; }
// 24 => { return 29; }
// 25 => { return 27; }
// 26 => { return 25; }
// 27 => { return 23; }
// 28 => { return 21; }
// 29 => { return 19; }
// 30 => { return 17; }
// 31 => { return  0; }
// 32 => { return  0; }
// 33 => { return  0; }
// 34 => { return  0; }
// 35 => { return  0; }
// 36 => { return  0; }
// 37 => { return  0; }
// 38 => { return  0; }
// 39 => { return  0; }
// 40 => { return  0; }
// default:
// log("SYSERR: Missing level for thief spell saving throw.");
// break;
// }
// default:
// log("SYSERR: Invalid saving throw type.");
// break;
// }
// break;
// CLASS_WARRIOR => { // switch (type) { }
// SAVING_PARA => { /* Paralyzation */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 70; }
// 2 => { return 68; }
// 3 => { return 67; }
// 4 => { return 65; }
// 5 => { return 62; }
// 6 => { return 58; }
// 7 => { return 55; }
// 8 => { return 53; }
// 9 => { return 52; }
// 10 => { return 50; }
// 11 => { return 47; }
// 12 => { return 43; }
// 13 => { return 40; }
// 14 => { return 38; }
// 15 => { return 37; }
// 16 => { return 35; }
// 17 => { return 32; }
// 18 => { return 28; }
// 19 => { return 25; }
// 20 => { return 24; }
// 21 => { return 23; }
// 22 => { return 22; }
// 23 => { return 20; }
// 24 => { return 19; }
// 25 => { return 17; }
// 26 => { return 16; }
// 27 => { return 15; }
// 28 => { return 14; }
// 29 => { return 13; }
// 30 => { return 12; }
// 31 => { return 11; }
// 32 => { return 10; }
// 33 => { return  9; }
// 34 => { return  8; }
// 35 => { return  7; }
// 36 => { return  6; }
// 37 => { return  5; }
// 38 => { return  4; }
// 39 => { return  3; }
// 40 => { return  2; }
// 41 => { return  1;	/* Some mobiles. */ }
// 42 => { return  0; }
// 43 => { return  0; }
// 44 => { return  0; }
// 45 => { return  0; }
// 46 => { return  0; }
// 47 => { return  0; }
// 48 => { return  0; }
// 49 => { return  0; }
// 50 => { return  0; }
// default:
// log("SYSERR: Missing level for warrior paralyzation saving throw.");
// break;
// }
// SAVING_ROD => { /* Rods */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 80; }
// 2 => { return 78; }
// 3 => { return 77; }
// 4 => { return 75; }
// 5 => { return 72; }
// 6 => { return 68; }
// 7 => { return 65; }
// 8 => { return 63; }
// 9 => { return 62; }
// 10 => { return 60; }
// 11 => { return 57; }
// 12 => { return 53; }
// 13 => { return 50; }
// 14 => { return 48; }
// 15 => { return 47; }
// 16 => { return 45; }
// 17 => { return 42; }
// 18 => { return 38; }
// 19 => { return 35; }
// 20 => { return 34; }
// 21 => { return 33; }
// 22 => { return 32; }
// 23 => { return 30; }
// 24 => { return 29; }
// 25 => { return 27; }
// 26 => { return 26; }
// 27 => { return 25; }
// 28 => { return 24; }
// 29 => { return 23; }
// 30 => { return 22; }
// 31 => { return 20; }
// 32 => { return 18; }
// 33 => { return 16; }
// 34 => { return 14; }
// 35 => { return 12; }
// 36 => { return 10; }
// 37 => { return  8; }
// 38 => { return  6; }
// 39 => { return  5; }
// 40 => { return  4; }
// 41 => { return  3; }
// 42 => { return  2; }
// 43 => { return  1; }
// 44 => { return  0; }
// 45 => { return  0; }
// 46 => { return  0; }
// 47 => { return  0; }
// 48 => { return  0; }
// 49 => { return  0; }
// 50 => { return  0; }
// default:
// log("SYSERR: Missing level for warrior rod saving throw.");
// break;
// }
// SAVING_PETRI => { /* Petrification */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 75; }
// 2 => { return 73; }
// 3 => { return 72; }
// 4 => { return 70; }
// 5 => { return 67; }
// 6 => { return 63; }
// 7 => { return 60; }
// 8 => { return 58; }
// 9 => { return 57; }
// 10 => { return 55; }
// 11 => { return 52; }
// 12 => { return 48; }
// 13 => { return 45; }
// 14 => { return 43; }
// 15 => { return 42; }
// 16 => { return 40; }
// 17 => { return 37; }
// 18 => { return 33; }
// 19 => { return 30; }
// 20 => { return 29; }
// 21 => { return 28; }
// 22 => { return 26; }
// 23 => { return 25; }
// 24 => { return 24; }
// 25 => { return 23; }
// 26 => { return 21; }
// 27 => { return 20; }
// 28 => { return 19; }
// 29 => { return 18; }
// 30 => { return 17; }
// 31 => { return 16; }
// 32 => { return 15; }
// 33 => { return 14; }
// 34 => { return 13; }
// 35 => { return 12; }
// 36 => { return 11; }
// 37 => { return 10; }
// 38 => { return  9; }
// 39 => { return  8; }
// 40 => { return  7; }
// 41 => { return  6; }
// 42 => { return  5; }
// 43 => { return  4; }
// 44 => { return  3; }
// 45 => { return  2; }
// 46 => { return  1; }
// 47 => { return  0; }
// 48 => { return  0; }
// 49 => { return  0; }
// 50 => { return  0; }
// default:
// log("SYSERR: Missing level for warrior petrification saving throw.");
// break;
// }
// SAVING_BREATH => { /* Breath weapons */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 85; }
// 2 => { return 83; }
// 3 => { return 82; }
// 4 => { return 80; }
// 5 => { return 75; }
// 6 => { return 70; }
// 7 => { return 65; }
// 8 => { return 63; }
// 9 => { return 62; }
// 10 => { return 60; }
// 11 => { return 55; }
// 12 => { return 50; }
// 13 => { return 45; }
// 14 => { return 43; }
// 15 => { return 42; }
// 16 => { return 40; }
// 17 => { return 37; }
// 18 => { return 33; }
// 19 => { return 30; }
// 20 => { return 29; }
// 21 => { return 28; }
// 22 => { return 26; }
// 23 => { return 25; }
// 24 => { return 24; }
// 25 => { return 23; }
// 26 => { return 21; }
// 27 => { return 20; }
// 28 => { return 19; }
// 29 => { return 18; }
// 30 => { return 17; }
// 31 => { return 16; }
// 32 => { return 15; }
// 33 => { return 14; }
// 34 => { return 13; }
// 35 => { return 12; }
// 36 => { return 11; }
// 37 => { return 10; }
// 38 => { return  9; }
// 39 => { return  8; }
// 40 => { return  7; }
// 41 => { return  6; }
// 42 => { return  5; }
// 43 => { return  4; }
// 44 => { return  3; }
// 45 => { return  2; }
// 46 => { return  1; }
// 47 => { return  0; }
// 48 => { return  0; }
// 49 => { return  0; }
// 50 => { return  0; }
// default:
// log("SYSERR: Missing level for warrior breath saving throw.");
// break;
// }
// SAVING_SPELL => { /* Generic spells */ }
// switch (level) {
// 0 => { return 90; }
// 1 => { return 85; }
// 2 => { return 83; }
// 3 => { return 82; }
// 4 => { return 80; }
// 5 => { return 77; }
// 6 => { return 73; }
// 7 => { return 70; }
// 8 => { return 68; }
// 9 => { return 67; }
// 10 => { return 65; }
// 11 => { return 62; }
// 12 => { return 58; }
// 13 => { return 55; }
// 14 => { return 53; }
// 15 => { return 52; }
// 16 => { return 50; }
// 17 => { return 47; }
// 18 => { return 43; }
// 19 => { return 40; }
// 20 => { return 39; }
// 21 => { return 38; }
// 22 => { return 36; }
// 23 => { return 35; }
// 24 => { return 34; }
// 25 => { return 33; }
// 26 => { return 31; }
// 27 => { return 30; }
// 28 => { return 29; }
// 29 => { return 28; }
// 30 => { return 27; }
// 31 => { return 25; }
// 32 => { return 23; }
// 33 => { return 21; }
// 34 => { return 19; }
// 35 => { return 17; }
// 36 => { return 15; }
// 37 => { return 13; }
// 38 => { return 11; }
// 39 => { return  9; }
// 40 => { return  7; }
// 41 => { return  6; }
// 42 => { return  5; }
// 43 => { return  4; }
// 44 => { return  3; }
// 45 => { return  2; }
// 46 => { return  1; }
// 47 => { return  0; }
// 48 => { return  0; }
// 49 => { return  0; }
// 50 => { return  0; }
// default:
// log("SYSERR: Missing level for warrior spell saving throw.");
// break;
// }
// default:
// log("SYSERR: Invalid saving throw type.");
// break;
// }
// default:
// log("SYSERR: Invalid class saving throw.");
// break;
// }
//
// /* Should not get here unless something is wrong. */
// return 100;
// }

/* THAC0 for classes and levels.  (To Hit Armor Class 0) */
pub fn thaco(class_num: i8, level: u8) -> i32 {
    match class_num {
        CLASS_MAGIC_USER => match level {
            0 => {
                return 100;
            }
            1 => {
                return 20;
            }
            2 => {
                return 20;
            }
            3 => {
                return 20;
            }
            4 => {
                return 19;
            }
            5 => {
                return 19;
            }
            6 => {
                return 19;
            }
            7 => {
                return 18;
            }
            8 => {
                return 18;
            }
            9 => {
                return 18;
            }
            10 => {
                return 17;
            }
            11 => {
                return 17;
            }
            12 => {
                return 17;
            }
            13 => {
                return 16;
            }
            14 => {
                return 16;
            }
            15 => {
                return 16;
            }
            16 => {
                return 15;
            }
            17 => {
                return 15;
            }
            18 => {
                return 15;
            }
            19 => {
                return 14;
            }
            20 => {
                return 14;
            }
            21 => {
                return 14;
            }
            22 => {
                return 13;
            }
            23 => {
                return 13;
            }
            24 => {
                return 13;
            }
            25 => {
                return 12;
            }
            26 => {
                return 12;
            }
            27 => {
                return 12;
            }
            28 => {
                return 11;
            }
            29 => {
                return 11;
            }
            30 => {
                return 11;
            }
            31 => {
                return 10;
            }
            32 => {
                return 10;
            }
            33 => {
                return 10;
            }
            34 => {
                return 9;
            }
            _ => {
                error!("SYSERR: Missing level for mage thac0.");
            }
        },
        CLASS_CLERIC => match level {
            0 => {
                return 100;
            }
            1 => {
                return 20;
            }
            2 => {
                return 20;
            }
            3 => {
                return 20;
            }
            4 => {
                return 18;
            }
            5 => {
                return 18;
            }
            6 => {
                return 18;
            }
            7 => {
                return 16;
            }
            8 => {
                return 16;
            }
            9 => {
                return 16;
            }
            10 => {
                return 14;
            }
            11 => {
                return 14;
            }
            12 => {
                return 14;
            }
            13 => {
                return 12;
            }
            14 => {
                return 12;
            }
            15 => {
                return 12;
            }
            16 => {
                return 10;
            }
            17 => {
                return 10;
            }
            18 => {
                return 10;
            }
            19 => {
                return 8;
            }
            20 => {
                return 8;
            }
            21 => {
                return 8;
            }
            22 => {
                return 6;
            }
            23 => {
                return 6;
            }
            24 => {
                return 6;
            }
            25 => {
                return 4;
            }
            26 => {
                return 4;
            }
            27 => {
                return 4;
            }
            28 => {
                return 2;
            }
            29 => {
                return 2;
            }
            30 => {
                return 2;
            }
            31 => {
                return 1;
            }
            32 => {
                return 1;
            }
            33 => {
                return 1;
            }
            34 => {
                return 1;
            }
            _ => {
                error!("SYSERR: Missing level for cleric thac0.");
            }
        },
        CLASS_THIEF => match level {
            0 => {
                return 100;
            }
            1 => {
                return 20;
            }
            2 => {
                return 20;
            }
            3 => {
                return 19;
            }
            4 => {
                return 19;
            }
            5 => {
                return 18;
            }
            6 => {
                return 18;
            }
            7 => {
                return 17;
            }
            8 => {
                return 17;
            }
            9 => {
                return 16;
            }
            10 => {
                return 16;
            }
            11 => {
                return 15;
            }
            12 => {
                return 15;
            }
            13 => {
                return 14;
            }
            14 => {
                return 14;
            }
            15 => {
                return 13;
            }
            16 => {
                return 13;
            }
            17 => {
                return 12;
            }
            18 => {
                return 12;
            }
            19 => {
                return 11;
            }
            20 => {
                return 11;
            }
            21 => {
                return 10;
            }
            22 => {
                return 10;
            }
            23 => {
                return 9;
            }
            24 => {
                return 9;
            }
            25 => {
                return 8;
            }
            26 => {
                return 8;
            }
            27 => {
                return 7;
            }
            28 => {
                return 7;
            }
            29 => {
                return 6;
            }
            30 => {
                return 6;
            }
            31 => {
                return 5;
            }
            32 => {
                return 5;
            }
            33 => {
                return 4;
            }
            34 => {
                return 4;
            }
            _ => {
                error!("SYSERR: Missing level for thief thac0.");
            }
        },
        CLASS_WARRIOR => match level {
            0 => {
                return 100;
            }
            1 => {
                return 20;
            }
            2 => {
                return 19;
            }
            3 => {
                return 18;
            }
            4 => {
                return 17;
            }
            5 => {
                return 16;
            }
            6 => {
                return 15;
            }
            7 => {
                return 14;
            }
            8 => {
                return 14;
            }
            9 => {
                return 13;
            }
            10 => {
                return 12;
            }
            11 => {
                return 11;
            }
            12 => {
                return 10;
            }
            13 => {
                return 9;
            }
            14 => {
                return 8;
            }
            15 => {
                return 7;
            }
            16 => {
                return 6;
            }
            17 => {
                return 5;
            }
            18 => {
                return 4;
            }
            19 => {
                return 3;
            }
            20 => {
                return 2;
            }
            21 => {
                return 1;
            }
            22 => {
                return 1;
            }
            23 => {
                return 1;
            }
            24 => {
                return 1;
            }
            25 => {
                return 1;
            }
            26 => {
                return 1;
            }
            27 => {
                return 1;
            }
            28 => {
                return 1;
            }
            29 => {
                return 1;
            }
            30 => {
                return 1;
            }
            31 => {
                return 1;
            }
            32 => {
                return 1;
            }
            33 => {
                return 1;
            }
            34 => {
                return 1;
            }
            _ => {
                error!("SYSERR: Missing level for warrior thac0.");
            }
        },
        _ => {
            error!("SYSERR: Unknown class in thac0 chart.");
        }
    }
    /* Will not get there unless something is wrong. */
    100
}

/*
 * Roll the 6 stats for a character... each stat is made of the sum of
 * the best 3 out of 4 rolls of a 6-sided die.  Each class then decides
 * which priority will be given for the best to worst stats.
 */
fn roll_real_abils(ch: &CharData) {
    //int i, j, k, temp;
    let mut table: [u8; 6] = [0; 6];
    let mut rolls: [u8; 4] = [0; 4];

    for _ in 0..6 {
        for j in 0..4 {
            rolls[j] = rand_number(1, 6) as u8;
        }

        let mut temp = rolls[0] + rolls[1] + rolls[2] + rolls[3]
            - min(rolls[0], min(rolls[1], min(rolls[2], rolls[3])));

        for k in 0..6 {
            if table[k] < temp {
                temp ^= table[k];
                table[k] ^= temp;
                temp ^= table[k];
            }
        }
    }

    ch.real_abils.borrow_mut().str_add = 0;

    match ch.get_class() {
        CLASS_MAGIC_USER => {
            ch.real_abils.borrow_mut().intel = table[0] as i8;
            ch.real_abils.borrow_mut().wis = table[1] as i8;
            ch.real_abils.borrow_mut().dex = table[2] as i8;
            ch.real_abils.borrow_mut().str = table[3] as i8;
            ch.real_abils.borrow_mut().con = table[4] as i8;
            ch.real_abils.borrow_mut().cha = table[5] as i8;
        }
        CLASS_CLERIC => {
            ch.real_abils.borrow_mut().wis = table[0] as i8;
            ch.real_abils.borrow_mut().intel = table[1] as i8;
            ch.real_abils.borrow_mut().str = table[2] as i8;
            ch.real_abils.borrow_mut().dex = table[3] as i8;
            ch.real_abils.borrow_mut().con = table[4] as i8;
            ch.real_abils.borrow_mut().cha = table[5] as i8;
        }
        CLASS_THIEF => {
            ch.real_abils.borrow_mut().dex = table[0] as i8;
            ch.real_abils.borrow_mut().str = table[1] as i8;
            ch.real_abils.borrow_mut().con = table[2] as i8;
            ch.real_abils.borrow_mut().intel = table[3] as i8;
            ch.real_abils.borrow_mut().wis = table[4] as i8;
            ch.real_abils.borrow_mut().cha = table[5] as i8;
        }
        CLASS_WARRIOR => {
            ch.real_abils.borrow_mut().str = table[0] as i8;
            ch.real_abils.borrow_mut().dex = table[1] as i8;
            ch.real_abils.borrow_mut().con = table[2] as i8;
            ch.real_abils.borrow_mut().wis = table[3] as i8;
            ch.real_abils.borrow_mut().intel = table[4] as i8;
            ch.real_abils.borrow_mut().cha = table[5] as i8;
            if ch.real_abils.borrow_mut().str == 18 {
                ch.real_abils.borrow_mut().str_add = rand_number(0, 100) as i8;
            }
        }
        _ => {}
    }
    *ch.aff_abils.borrow_mut() = *ch.real_abils.borrow();
}

/* Some initializations for characters, including initial skills */
impl MainGlobals {
    pub fn do_start(&self, ch: &CharData) {
        ch.set_level(1);
        ch.set_exp(1);

        ch.set_title(Some("".to_string()));
        roll_real_abils(ch);

        ch.set_max_hit(10);
        ch.set_max_mana(100);
        ch.set_max_move(82);

        match ch.get_class() {
            CLASS_MAGIC_USER => {}

            CLASS_CLERIC => {}

            CLASS_THIEF => {
                set_skill!(ch, SKILL_SNEAK, 10);
                set_skill!(ch, SKILL_HIDE, 5);
                set_skill!(ch, SKILL_STEAL, 15);
                set_skill!(ch, SKILL_BACKSTAB, 10);
                set_skill!(ch, SKILL_PICK_LOCK, 10);
                set_skill!(ch, SKILL_TRACK, 10);
            }

            CLASS_WARRIOR => {}
            _ => {}
        }

        advance_level(ch, &self.db);

        self.mudlog(
            BRF,
            max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
            true,
            format!("{} advanced to level {}", ch.get_name(), ch.get_level()).as_str(),
        );

        ch.set_hit(ch.get_max_hit());
        ch.set_mana(ch.get_max_mana());
        ch.set_move(ch.get_max_move());

        ch.set_cond(THIRST, 24);
        ch.set_cond(FULL, 24);
        ch.set_cond(DRUNK, 0);

        // if (siteok_everyone)
        // SET_BIT(PLR_FLAGS(ch), PLR_SITEOK);
    }
}

/*
 * This function controls the change to maxmove, maxmana, and maxhp for
 * each class every time they gain a level.
 */
pub fn advance_level(ch: &CharData, db: &DB) {
    //int add_hp, add_mana = 0, add_move = 0, i;

    let mut add_hp = CON_APP[ch.get_con() as usize].hitp;
    let mut add_mana = 0;
    let mut add_move = 0;

    match ch.get_class() {
        CLASS_MAGIC_USER => {
            add_hp += rand_number(3, 8) as i16;
            add_mana = rand_number(ch.get_level() as u32, (3 * ch.get_level() / 2) as u32);
            add_mana = min(add_mana, 10);
            add_move = rand_number(0, 2);
        }

        CLASS_CLERIC => {
            add_hp += rand_number(5, 10) as i16;
            add_mana = rand_number(ch.get_level() as u32, (3 * ch.get_level() / 2) as u32);
            add_mana = min(add_mana, 10);
            add_move = rand_number(0, 2);
        }

        CLASS_THIEF => {
            add_hp += rand_number(7, 13) as i16;
            add_mana = 0;
            add_move = rand_number(1, 3);
        }

        CLASS_WARRIOR => {
            add_hp += rand_number(10, 15) as i16;
            add_mana = 0;
            add_move = rand_number(1, 3);
        }
        _ => {}
    }

    ch.incr_max_hit(max(1, add_hp));
    ch.incr_max_move(max(1, add_move) as i16);

    if ch.get_level() > 1 {
        ch.incr_max_mana(add_mana as i16);
    }

    if ch.is_magic_user() || ch.is_cleric() {
        ch.incr_practices(max(2, WIS_APP[ch.get_wis() as usize].bonus) as i32);
    } else {
        ch.incr_practices(min(2, max(1, WIS_APP[ch.get_wis() as usize].bonus)) as i32);
    }

    if ch.get_level() >= LVL_IMMORT as u8 {
        for i in 0..3 {
            ch.set_cond(i, -1);
        }
        ch.set_prf_flags_bits(PRF_HOLYLIGHT);
    }

    //snoop_check(ch);
    db.save_char(ch);
}

/*
 * This simply calculates the backstab multiplier based on a character's
 * level.  This used to be an array, but was changed to be a function so
 * that it would be easier to add more levels to your MUD.  This doesn't
 * really create a big performance hit because it's not used very often.
 */
pub fn backstab_mult(level: u8) -> i32 {
    if level <= 0 {
        return 1; /* level 0 */
    } else if level <= 7 {
        return 2; /* level 1 - 7 */
    } else if level <= 13 {
        return 3; /* level 8 - 13 */
    } else if level <= 20 {
        return 4; /* level 14 - 20 */
    } else if level <= 28 {
        return 5; /* level 21 - 28 */
    } else if level < LVL_IMMORT as u8 {
        return 6; /* all remaining mortal levels */
    } else {
        return 20; /* immortals */
    }
}

/*
 * invalid_class is used by handler.c to determine if a piece of equipment is
 * usable by a particular class, based on the ITEM_ANTI_{class} bitvectors.
 */
pub fn invalid_class(ch: &CharData, obj: &ObjData) -> bool {
    if obj.obj_flagged(ITEM_ANTI_MAGIC_USER) && ch.is_magic_user() {
        return true;
    }

    if obj.obj_flagged(ITEM_ANTI_CLERIC) && ch.is_cleric() {
        return true;
    }
    if obj.obj_flagged(ITEM_ANTI_WARRIOR) && ch.is_warrior() {
        return true;
    }

    if obj.obj_flagged(ITEM_ANTI_THIEF) && ch.is_thief() {
        return true;
    }

    false
}

/*
 * SPELLS AND SKILLS.  This area defines which spells are assigned to
 * which classes, and the minimum level the character must be to use
 * the spell or skill.
 */
pub fn init_spell_levels(db: &mut DB) {
    /* MAGES */
    spell_level(db, SPELL_MAGIC_MISSILE, CLASS_MAGIC_USER, 1);
    spell_level(db, SPELL_DETECT_INVIS, CLASS_MAGIC_USER, 2);
    spell_level(db, SPELL_DETECT_MAGIC, CLASS_MAGIC_USER, 2);
    spell_level(db, SPELL_CHILL_TOUCH, CLASS_MAGIC_USER, 3);
    spell_level(db, SPELL_INFRAVISION, CLASS_MAGIC_USER, 3);
    spell_level(db, SPELL_INVISIBLE, CLASS_MAGIC_USER, 4);
    spell_level(db, SPELL_ARMOR, CLASS_MAGIC_USER, 4);
    spell_level(db, SPELL_BURNING_HANDS, CLASS_MAGIC_USER, 5);
    spell_level(db, SPELL_LOCATE_OBJECT, CLASS_MAGIC_USER, 6);
    spell_level(db, SPELL_STRENGTH, CLASS_MAGIC_USER, 6);
    spell_level(db, SPELL_SHOCKING_GRASP, CLASS_MAGIC_USER, 7);
    spell_level(db, SPELL_SLEEP, CLASS_MAGIC_USER, 8);
    spell_level(db, SPELL_LIGHTNING_BOLT, CLASS_MAGIC_USER, 9);
    spell_level(db, SPELL_BLINDNESS, CLASS_MAGIC_USER, 9);
    spell_level(db, SPELL_DETECT_POISON, CLASS_MAGIC_USER, 10);
    spell_level(db, SPELL_COLOR_SPRAY, CLASS_MAGIC_USER, 11);
    spell_level(db, SPELL_ENERGY_DRAIN, CLASS_MAGIC_USER, 13);
    spell_level(db, SPELL_CURSE, CLASS_MAGIC_USER, 14);
    spell_level(db, SPELL_POISON, CLASS_MAGIC_USER, 14);
    spell_level(db, SPELL_FIREBALL, CLASS_MAGIC_USER, 15);
    spell_level(db, SPELL_CHARM, CLASS_MAGIC_USER, 16);
    spell_level(db, SPELL_ENCHANT_WEAPON, CLASS_MAGIC_USER, 26);
    spell_level(db, SPELL_CLONE, CLASS_MAGIC_USER, 30);

    /* CLERICS */
    spell_level(db, SPELL_CURE_LIGHT, CLASS_CLERIC, 1);
    spell_level(db, SPELL_ARMOR, CLASS_CLERIC, 1);
    spell_level(db, SPELL_CREATE_FOOD, CLASS_CLERIC, 2);
    spell_level(db, SPELL_CREATE_WATER, CLASS_CLERIC, 2);
    spell_level(db, SPELL_DETECT_POISON, CLASS_CLERIC, 3);
    spell_level(db, SPELL_DETECT_ALIGN, CLASS_CLERIC, 4);
    spell_level(db, SPELL_CURE_BLIND, CLASS_CLERIC, 4);
    spell_level(db, SPELL_BLESS, CLASS_CLERIC, 5);
    spell_level(db, SPELL_DETECT_INVIS, CLASS_CLERIC, 6);
    spell_level(db, SPELL_BLINDNESS, CLASS_CLERIC, 6);
    spell_level(db, SPELL_INFRAVISION, CLASS_CLERIC, 7);
    spell_level(db, SPELL_PROT_FROM_EVIL, CLASS_CLERIC, 8);
    spell_level(db, SPELL_POISON, CLASS_CLERIC, 8);
    spell_level(db, SPELL_GROUP_ARMOR, CLASS_CLERIC, 9);
    spell_level(db, SPELL_CURE_CRITIC, CLASS_CLERIC, 9);
    spell_level(db, SPELL_SUMMON, CLASS_CLERIC, 10);
    spell_level(db, SPELL_REMOVE_POISON, CLASS_CLERIC, 10);
    spell_level(db, SPELL_WORD_OF_RECALL, CLASS_CLERIC, 12);
    spell_level(db, SPELL_EARTHQUAKE, CLASS_CLERIC, 12);
    spell_level(db, SPELL_DISPEL_EVIL, CLASS_CLERIC, 14);
    spell_level(db, SPELL_DISPEL_GOOD, CLASS_CLERIC, 14);
    spell_level(db, SPELL_SANCTUARY, CLASS_CLERIC, 15);
    spell_level(db, SPELL_CALL_LIGHTNING, CLASS_CLERIC, 15);
    spell_level(db, SPELL_HEAL, CLASS_CLERIC, 16);
    spell_level(db, SPELL_CONTROL_WEATHER, CLASS_CLERIC, 17);
    spell_level(db, SPELL_SENSE_LIFE, CLASS_CLERIC, 18);
    spell_level(db, SPELL_HARM, CLASS_CLERIC, 19);
    spell_level(db, SPELL_GROUP_HEAL, CLASS_CLERIC, 22);
    spell_level(db, SPELL_REMOVE_CURSE, CLASS_CLERIC, 26);

    /* THIEVES */
    spell_level(db, SKILL_SNEAK, CLASS_THIEF, 1);
    spell_level(db, SKILL_PICK_LOCK, CLASS_THIEF, 2);
    spell_level(db, SKILL_BACKSTAB, CLASS_THIEF, 3);
    spell_level(db, SKILL_STEAL, CLASS_THIEF, 4);
    spell_level(db, SKILL_HIDE, CLASS_THIEF, 5);
    spell_level(db, SKILL_TRACK, CLASS_THIEF, 6);

    /* WARRIORS */
    spell_level(db, SKILL_KICK, CLASS_WARRIOR, 1);
    spell_level(db, SKILL_RESCUE, CLASS_WARRIOR, 3);
    spell_level(db, SKILL_TRACK, CLASS_WARRIOR, 9);
    spell_level(db, SKILL_BASH, CLASS_WARRIOR, 12);
}

/*
 * This is the exp given to implementors -- it must always be greater
 * than the exp required for immortality, plus at least 20,000 or so.
 */
pub const EXP_MAX: i32 = 10000000;

/* Function to return the exp required for each class/level */
pub fn level_exp(chclass: i8, level: i16) -> i32 {
    if level > LVL_IMPL || level < 0 {
        info!("SYSERR: Requesting exp for invalid level {}!", level);
        return 0;
    }

    /*
     * Gods have exp close to EXP_MAX.  This statement should never have to
     * changed, regardless of how many mortal or immortal levels exist.
     */
    if level > LVL_IMMORT {
        return EXP_MAX - ((LVL_IMPL - level) * 1000) as i32;
    }

    /* Exp required for normal mortals is below */

    match chclass {
        CLASS_MAGIC_USER => {
            match level {
                0 => {
                    return 0;
                }
                1 => {
                    return 1;
                }
                2 => {
                    return 2500;
                }
                3 => {
                    return 5000;
                }
                4 => {
                    return 10000;
                }
                5 => {
                    return 20000;
                }
                6 => {
                    return 40000;
                }
                7 => {
                    return 60000;
                }
                8 => {
                    return 90000;
                }
                9 => {
                    return 135000;
                }
                10 => {
                    return 250000;
                }
                11 => {
                    return 375000;
                }
                12 => {
                    return 750000;
                }
                13 => {
                    return 1125000;
                }
                14 => {
                    return 1500000;
                }
                15 => {
                    return 1875000;
                }
                16 => {
                    return 2250000;
                }
                17 => {
                    return 2625000;
                }
                18 => {
                    return 3000000;
                }
                19 => {
                    return 3375000;
                }
                20 => {
                    return 3750000;
                }
                21 => {
                    return 4000000;
                }
                22 => {
                    return 4300000;
                }
                23 => {
                    return 4600000;
                }
                24 => {
                    return 4900000;
                }
                25 => {
                    return 5200000;
                }
                26 => {
                    return 5500000;
                }
                27 => {
                    return 5950000;
                }
                28 => {
                    return 6400000;
                }
                29 => {
                    return 6850000;
                }
                30 => {
                    return 7400000;
                }
                /* add new levels here */
                LVL_IMMORT => {
                    return 800000;
                }
                _ => {}
            }
        }

        CLASS_CLERIC => {
            match level {
                0 => {
                    return 0;
                }
                1 => {
                    return 1;
                }
                2 => {
                    return 1500;
                }
                3 => {
                    return 3000;
                }
                4 => {
                    return 6000;
                }
                5 => {
                    return 13000;
                }
                6 => {
                    return 27500;
                }
                7 => {
                    return 55000;
                }
                8 => {
                    return 110000;
                }
                9 => {
                    return 225000;
                }
                10 => {
                    return 450000;
                }
                11 => {
                    return 675000;
                }
                12 => {
                    return 900000;
                }
                13 => {
                    return 1125000;
                }
                14 => {
                    return 1350000;
                }
                15 => {
                    return 1575000;
                }
                16 => {
                    return 1800000;
                }
                17 => {
                    return 2100000;
                }
                18 => {
                    return 2400000;
                }
                19 => {
                    return 2700000;
                }
                20 => {
                    return 3000000;
                }
                21 => {
                    return 3250000;
                }
                22 => {
                    return 3500000;
                }
                23 => {
                    return 3800000;
                }
                24 => {
                    return 4100000;
                }
                25 => {
                    return 4400000;
                }
                26 => {
                    return 4800000;
                }
                27 => {
                    return 5200000;
                }
                28 => {
                    return 5600000;
                }
                29 => {
                    return 6000000;
                }
                30 => {
                    return 6400000;
                }
                /* add new levels here */
                LVL_IMMORT => {
                    return 7000000;
                }
                _ => {}
            }
        }
        CLASS_THIEF => {
            match level {
                0 => {
                    return 0;
                }
                1 => {
                    return 1;
                }
                2 => {
                    return 1250;
                }
                3 => {
                    return 2500;
                }
                4 => {
                    return 5000;
                }
                5 => {
                    return 10000;
                }
                6 => {
                    return 20000;
                }
                7 => {
                    return 40000;
                }
                8 => {
                    return 70000;
                }
                9 => {
                    return 110000;
                }
                10 => {
                    return 160000;
                }
                11 => {
                    return 220000;
                }
                12 => {
                    return 440000;
                }
                13 => {
                    return 660000;
                }
                14 => {
                    return 880000;
                }
                15 => {
                    return 1100000;
                }
                16 => {
                    return 1500000;
                }
                17 => {
                    return 2000000;
                }
                18 => {
                    return 2500000;
                }
                19 => {
                    return 3000000;
                }
                20 => {
                    return 3500000;
                }
                21 => {
                    return 3650000;
                }
                22 => {
                    return 3800000;
                }
                23 => {
                    return 4100000;
                }
                24 => {
                    return 4400000;
                }
                25 => {
                    return 4700000;
                }
                26 => {
                    return 5100000;
                }
                27 => {
                    return 5500000;
                }
                28 => {
                    return 5900000;
                }
                29 => {
                    return 6300000;
                }
                30 => {
                    return 6650000;
                }
                /* add new levels here */
                LVL_IMMORT => {
                    return 7000000;
                }
                _ => {}
            }
        }
        CLASS_WARRIOR => {
            match level {
                0 => {
                    return 0;
                }
                1 => {
                    return 1;
                }
                2 => {
                    return 2000;
                }
                3 => {
                    return 4000;
                }
                4 => {
                    return 8000;
                }
                5 => {
                    return 16000;
                }
                6 => {
                    return 32000;
                }
                7 => {
                    return 64000;
                }
                8 => {
                    return 125000;
                }
                9 => {
                    return 250000;
                }
                10 => {
                    return 500000;
                }
                11 => {
                    return 750000;
                }
                12 => {
                    return 1000000;
                }
                13 => {
                    return 1250000;
                }
                14 => {
                    return 1500000;
                }
                15 => {
                    return 1850000;
                }
                16 => {
                    return 2200000;
                }
                17 => {
                    return 2550000;
                }
                18 => {
                    return 2900000;
                }
                19 => {
                    return 3250000;
                }
                20 => {
                    return 3600000;
                }
                21 => {
                    return 3900000;
                }
                22 => {
                    return 4200000;
                }
                23 => {
                    return 4500000;
                }
                24 => {
                    return 4800000;
                }
                25 => {
                    return 5150000;
                }
                26 => {
                    return 5500000;
                }
                27 => {
                    return 5950000;
                }
                28 => {
                    return 6400000;
                }
                29 => {
                    return 6850000;
                }
                30 => {
                    return 7400000;
                }
                /* add new levels here */
                LVL_IMMORT => {
                    return 8000000;
                }
                _ => {}
            }
        }
        _ => {}
    }

    /*
     * This statement should never be reached if the exp tables in this function
     * are set up properly.  If you see exp of 123456 then the tables above are
     * incomplete -- so, complete them!
     */
    error!("SYSERR: XP tables not set up correctly in class.c!");
    return 123456;
}

/*
 * Default titles of male characters.
 */
pub fn title_male(chclass: i32, level: i32) -> &'static str {
    if level <= 0 || level > LVL_IMPL as i32 {
        return "the Man";
    }
    if level == LVL_IMPL as i32 {
        return "the Implementor";
    }

    return match chclass as i8 {
        CLASS_MAGIC_USER => match level as i16 {
            1 => "the Apprentice of Magic",
            2 => "the Spell Student",
            3 => "the Scholar of Magic",
            4 => "the Delver in Spells",
            5 => "the Medium of Magic",
            6 => "the Scribe of Magic",
            7 => "the Seer",
            8 => "the Sage",
            9 => "the Illusionist",
            10 => "the Abjurer",
            11 => "the Invoker",
            12 => "the Enchanter",
            13 => "the Conjurer",
            14 => "the Magician",
            15 => "the Creator",
            16 => "the Savant",
            17 => "the Magus",
            18 => "the Wizard",
            19 => "the Warlock",
            20 => "the Sorcerer",
            21 => "the Necromancer",
            22 => "the Thaumaturge",
            23 => "the Student of the Occult",
            24 => "the Disciple of the Uncanny",
            25 => "the minor Elemental",
            26 => "the Greater Elemental",
            27 => "the Crafter of Magics",
            28 => "the Shaman",
            29 => "the Keeper of Talismans",
            30 => "the Archmage",
            LVL_IMMORT => "the Immortal Warlock",
            LVL_GOD => "the Avatar of Magic",
            LVL_GRGOD => "the God of Magic",
            _ => "the Mage",
        },
        CLASS_CLERIC => {
            match level as i16 {
                1 => "the Believer",
                2 => "the Attendant",
                3 => "the Acolyte",
                4 => "the Novice",
                5 => "the Missionary",
                6 => "the Adept",
                7 => "the Deacon",
                8 => "the Vicar",
                9 => "the Priest",
                10 => "the minister",
                11 => "the Canon",
                12 => "the Levite",
                13 => "the Curate",
                14 => "the Monk",
                15 => "the Healer",
                16 => "the Chaplain",
                17 => "the Expositor",
                18 => "the Bishop",
                19 => "the Arch Bishop",
                20 => "the Patriarch",
                /* no one ever thought up these titles 21-30 */
                LVL_IMMORT => "the Immortal Cardinal",
                LVL_GOD => "the Inquisitor",
                LVL_GRGOD => "the God of good and evil",
                _ => "the Cleric",
            }
        }

        CLASS_THIEF => {
            match level as i16 {
                1 => "the Pilferer",
                2 => "the Footpad",
                3 => "the Filcher",
                4 => "the Pick-Pocket",
                5 => "the Sneak",
                6 => "the Pincher",
                7 => "the Cut-Purse",
                8 => "the Snatcher",
                9 => "the Sharper",
                10 => "the Rogue",
                11 => "the Robber",
                12 => "the Magsman",
                13 => "the Highwayman",
                14 => "the Burglar",
                15 => "the Thief",
                16 => "the Knifer",
                17 => "the Quick-Blade",
                18 => "the Killer",
                19 => "the Brigand",
                20 => "the Cut-Throat",
                /* no one ever thought up these titles 21-30 */
                LVL_IMMORT => "the Immortal Assasin",
                LVL_GOD => "the Demi God of thieves",
                LVL_GRGOD => "the God of thieves and tradesmen",
                _ => "the Thief",
            }
        }

        CLASS_WARRIOR => {
            match level as i16 {
                1 => "the Swordpupil",
                2 => "the Recruit",
                3 => "the Sentry",
                4 => "the Fighter",
                5 => "the Soldier",
                6 => "the Warrior",
                7 => "the Veteran",
                8 => "the Swordsman",
                9 => "the Fencer",
                10 => "the Combatant",
                11 => "the Hero",
                12 => "the Myrmidon",
                13 => "the Swashbuckler",
                14 => "the Mercenary",
                15 => "the Swordmaster",
                16 => "the Lieutenant",
                17 => "the Champion",
                18 => "the Dragoon",
                19 => "the Cavalier",
                20 => "the Knight",
                /* no one ever thought up these titles 21-30 */
                LVL_IMMORT => "the Immortal Warlord",
                LVL_GOD => "the Extirpator",
                LVL_GRGOD => "the God of war",
                _ => "the Warrior",
            }
        }
        _ => {
            /* Default title for classes which do not have titles defined */
            "the Classless"
        }
    };
}

/*
 * Default titles of female characters.
 */
pub fn title_female(chclass: i32, level: i32) -> &'static str {
    if level <= 0 || level > LVL_IMPL as i32 {
        return "the Woman";
    }
    if level == LVL_IMPL as i32 {
        return "the Implementress";
    }

    match chclass as i8 {
        CLASS_MAGIC_USER => match level as i16 {
            1 => {
                return "the Apprentice of Magic";
            }
            2 => {
                return "the Spell Student";
            }
            3 => {
                return "the Scholar of Magic";
            }
            4 => {
                return "the Delveress in Spells";
            }
            5 => {
                return "the Medium of Magic";
            }
            6 => {
                return "the Scribess of Magic";
            }
            7 => {
                return "the Seeress";
            }
            8 => {
                return "the Sage";
            }
            9 => {
                return "the Illusionist";
            }
            10 => {
                return "the Abjuress";
            }
            11 => {
                return "the Invoker";
            }
            12 => {
                return "the Enchantress";
            }
            13 => {
                return "the Conjuress";
            }
            14 => {
                return "the Witch";
            }
            15 => {
                return "the Creator";
            }
            16 => {
                return "the Savant";
            }
            17 => {
                return "the Craftess";
            }
            18 => {
                return "the Wizard";
            }
            19 => {
                return "the War Witch";
            }
            20 => {
                return "the Sorceress";
            }
            21 => {
                return "the Necromancress";
            }
            22 => {
                return "the Thaumaturgess";
            }
            23 => {
                return "the Student of the Occult";
            }
            24 => {
                return "the Disciple of the Uncanny";
            }
            25 => {
                return "the minor Elementress";
            }
            26 => {
                return "the Greater Elementress";
            }
            27 => {
                return "the Crafter of Magics";
            }
            28 => {
                return "Shaman";
            }
            29 => {
                return "the Keeper of Talismans";
            }
            30 => {
                return "Archwitch";
            }
            LVL_IMMORT => {
                return "the Immortal Enchantress";
            }
            LVL_GOD => {
                return "the Empress of Magic";
            }
            LVL_GRGOD => {
                return "the Goddess of Magic";
            }
            _ => {
                return "the Witch";
            }
        },

        CLASS_CLERIC => {
            match level as i16 {
                1 => {
                    return "the Believer";
                }
                2 => {
                    return "the Attendant";
                }
                3 => {
                    return "the Acolyte";
                }
                4 => {
                    return "the Novice";
                }
                5 => {
                    return "the Missionary";
                }
                6 => {
                    return "the Adept";
                }
                7 => {
                    return "the Deaconess";
                }
                8 => {
                    return "the Vicaress";
                }
                9 => {
                    return "the Priestess";
                }
                10 => {
                    return "the Lady minister";
                }
                11 => {
                    return "the Canon";
                }
                12 => {
                    return "the Levitess";
                }
                13 => {
                    return "the Curess";
                }
                14 => {
                    return "the Nunne";
                }
                15 => {
                    return "the Healess";
                }
                16 => {
                    return "the Chaplain";
                }
                17 => {
                    return "the Expositress";
                }
                18 => {
                    return "the Bishop";
                }
                19 => {
                    return "the Arch Lady of the Church";
                }
                20 => {
                    return "the Matriarch";
                }
                /* no one ever thought up these titles 21-30 */
                LVL_IMMORT => {
                    return "the Immortal Priestess";
                }
                LVL_GOD => {
                    return "the Inquisitress";
                }
                LVL_GRGOD => {
                    return "the Goddess of good and evil";
                }
                _ => {
                    return "the Cleric";
                }
            }
        }
        CLASS_THIEF => {
            match level as i16 {
                1 => {
                    return "the Pilferess";
                }
                2 => {
                    return "the Footpad";
                }
                3 => {
                    return "the Filcheress";
                }
                4 => {
                    return "the Pick-Pocket";
                }
                5 => {
                    return "the Sneak";
                }
                6 => {
                    return "the Pincheress";
                }
                7 => {
                    return "the Cut-Purse";
                }
                8 => {
                    return "the Snatcheress";
                }
                9 => {
                    return "the Sharpress";
                }
                10 => {
                    return "the Rogue";
                }
                11 => {
                    return "the Robber";
                }
                12 => {
                    return "the Magswoman";
                }
                13 => {
                    return "the Highwaywoman";
                }
                14 => {
                    return "the Burglaress";
                }
                15 => {
                    return "the Thief";
                }
                16 => {
                    return "the Knifer";
                }
                17 => {
                    return "the Quick-Blade";
                }
                18 => {
                    return "the Murderess";
                }
                19 => {
                    return "the Brigand";
                }
                20 => {
                    return "the Cut-Throat";
                }
                /* no one ever thought up these titles 21-30 */
                LVL_IMMORT => {
                    return "the Immortal Assasin";
                }
                LVL_GOD => {
                    return "the Demi Goddess of thieves";
                }
                LVL_GRGOD => {
                    return "the Goddess of thieves and tradesmen";
                }
                _ => {
                    return "the Thief";
                }
            }
        }

        CLASS_WARRIOR => {
            match level as i16 {
                1 => {
                    return "the Swordpupil";
                }
                2 => {
                    return "the Recruit";
                }
                3 => {
                    return "the Sentress";
                }
                4 => {
                    return "the Fighter";
                }
                5 => {
                    return "the Soldier";
                }
                6 => {
                    return "the Warrior";
                }
                7 => {
                    return "the Veteran";
                }
                8 => {
                    return "the Swordswoman";
                }
                9 => {
                    return "the Fenceress";
                }
                10 => {
                    return "the Combatess";
                }
                11 => {
                    return "the Heroine";
                }
                12 => {
                    return "the Myrmidon";
                }
                13 => {
                    return "the Swashbuckleress";
                }
                14 => {
                    return "the Mercenaress";
                }
                15 => {
                    return "the Swordmistress";
                }
                16 => {
                    return "the Lieutenant";
                }
                17 => {
                    return "the Lady Champion";
                }
                18 => {
                    return "the Lady Dragoon";
                }
                19 => {
                    return "the Cavalier";
                }
                20 => {
                    return "the Lady Knight";
                }
                /* no one ever thought up these titles 21-30 */
                LVL_IMMORT => {
                    return "the Immortal Lady of War";
                }
                LVL_GOD => {
                    return "the Queen of Destruction";
                }
                LVL_GRGOD => {
                    return "the Goddess of war";
                }
                _ => {
                    return "the Warrior";
                }
            }
        }
        _ => {
            /* Default title for classes which do not have titles defined */
            return "the Classless";
        }
    }
}
