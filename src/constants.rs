/* ************************************************************************
*   File: constants.rs                                  Part of CircleMUD *
*  Usage: Numeric and string contants used by the MUD                     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use crate::structs::{ConAppType, DexAppType, DexSkillType, IntAppType, StrAppType, WisAppType};

pub const CIRCLEMUD_VERSION: &str = "mud-r version 1.0, based on CircleMUD, version 3.1";

/* strings corresponding to ordinals/bitvectors in structs.h ***********/

/* (Note: strings for class definitions in class.c instead of here) */

/* cardinal directions */
pub const DIRS: [&str; 7] = ["north", "east", "south", "west", "up", "down", "\n"];

/* ROOM_x */
pub const ROOM_BITS: [&str; 17] = [
    "DARK",
    "DEATH",
    "NO_MOB",
    "INDOORS",
    "PEACEFUL",
    "SOUNDPROOF",
    "NO_TRACK",
    "NO_MAGIC",
    "TUNNEL",
    "PRIVATE",
    "GODROOM",
    "HOUSE",
    "HCRSH",
    "ATRIUM",
    "OLC",
    "*", /* BFS MARK */
    "\n",
];

/* EX_x */
pub const EXIT_BITS: [&str; 5] = ["DOOR", "CLOSED", "LOCKED", "PICKPROOF", "\n"];

/* SECT_ */
pub const SECTOR_TYPES: [&str; 11] = [
    "Inside",
    "City",
    "Field",
    "Forest",
    "Hills",
    "Mountains",
    "Water (Swim)",
    "Water (No Swim)",
    "In Flight",
    "Underwater",
    "\n",
];

/*
 * SEX_x
 * Not used in sprinttype() so no \n.
 */
pub const GENDERS: [&str; 4] = ["neutral", "male", "female", "\n"];

/* POS_x */
pub const POSITION_TYPES: [&str; 10] = [
    "Dead",
    "Mortally wounded",
    "Incapacitated",
    "Stunned",
    "Sleeping",
    "Resting",
    "Sitting",
    "Fighting",
    "Standing",
    "\n",
];

/* PLR_x */
pub const PLAYER_BITS: [&str; 18] = [
    "KILLER", "THIEF", "FROZEN", "DONTSET", "WRITING", "MAILING", "CSH", "SITEOK", "NOSHOUT",
    "NOTITLE", "DELETED", "LOADRM", "NO_WIZL", "NO_DEL", "INVST", "CRYO",
    "DEAD", /* You should never see this. */
    "\n",
];

/* MOB_x */
pub const ACTION_BITS: [&str; 20] = [
    "SPEC",
    "SENTINEL",
    "SCAVENGER",
    "ISNPC",
    "AWARE",
    "AGGR",
    "STAY-ZONE",
    "WIMPY",
    "AGGR_EVIL",
    "AGGR_GOOD",
    "AGGR_NEUTRAL",
    "MEMORY",
    "HELPER",
    "NO_CHARM",
    "NO_SUMMN",
    "NO_SLEEP",
    "NO_BASH",
    "NO_BLIND",
    "DEAD", /* You should never see this. */
    "\n",
];

/* PRF_x */
pub const PREFERENCE_BITS: [&str; 23] = [
    "BRIEF", "COMPACT", "DEAF", "NO_TELL", "D_HP", "D_MANA", "D_MOVE", "AUTOEX", "NO_HASS",
    "QUEST", "SUMN", "NO_REP", "LIGHT", "C1", "C2", "NO_WIZ", "L1", "L2", "NO_AUC", "NO_GOS",
    "NO_GTZ", "RMFLG", "\n",
];

/* AFF_x */
pub const AFFECTED_BITS: [&str; 23] = [
    "BLIND",
    "INVIS",
    "DET-ALIGN",
    "DET-INVIS",
    "DET-MAGIC",
    "SENSE-LIFE",
    "WATWALK",
    "SANCT",
    "GROUP",
    "CURSE",
    "INFRA",
    "POISON",
    "PROT-EVIL",
    "PROT-GOOD",
    "SLEEP",
    "NO_TRACK",
    "UNUSED",
    "UNUSED",
    "SNEAK",
    "HIDE",
    "UNUSED",
    "CHARM",
    "\n",
];

/* CON_x */
pub const CONNECTED_TYPES: [&str; 19] = [
    "Playing",
    "Disconnecting",
    "Get name",
    "Confirm name",
    "Get password",
    "Get new PW",
    "Confirm new PW",
    "Select sex",
    "Select class",
    "Reading MOTD",
    "Main Menu",
    "Get descript.",
    "Changing PW 1",
    "Changing PW 2",
    "Changing PW 3",
    "Self-Delete 1",
    "Self-Delete 2",
    "Disconnecting",
    "\n",
];

/*
 * WEAR_x - for eq list
 * Not use in sprinttype() so no \n.
 */
pub const WEAR_WHERE: [&str; 18] = [
    "<used as light>      ",
    "<worn on finger>     ",
    "<worn on finger>     ",
    "<worn around neck>   ",
    "<worn around neck>   ",
    "<worn on body>       ",
    "<worn on head>       ",
    "<worn on legs>       ",
    "<worn on feet>       ",
    "<worn on hands>      ",
    "<worn on arms>       ",
    "<worn as shield>     ",
    "<worn about body>    ",
    "<worn about waist>   ",
    "<worn around wrist>  ",
    "<worn around wrist>  ",
    "<wielded>            ",
    "<held>               ",
];

/* ITEM_x (ordinal object types) */
pub const ITEM_TYPES: [&str; 25] = [
    "UNDEFINED",
    "LIGHT",
    "SCROLL",
    "WAND",
    "STAFF",
    "WEAPON",
    "FIRE WEAPON",
    "MISSILE",
    "TREASURE",
    "ARMOR",
    "POTION",
    "WORN",
    "OTHER",
    "TRASH",
    "TRAP",
    "CONTAINER",
    "NOTE",
    "LIQ CONTAINER",
    "KEY",
    "FOOD",
    "MONEY",
    "PEN",
    "BOAT",
    "FOUNTAIN",
    "\n",
];

/* ITEM_WEAR_ (wear bitvector) */
pub const WEAR_BITS: [&str; 16] = [
    "TAKE", "FINGER", "NECK", "BODY", "HEAD", "LEGS", "FEET", "HANDS", "ARMS", "SHIELD", "ABOUT",
    "WAIST", "WRIST", "WIELD", "HOLD", "\n",
];

/* ITEM_x (extra bits) */
pub const EXTRA_BITS: [&str; 18] = [
    "GLOW",
    "HUM",
    "NO_RENT",
    "NO_DONATE",
    "NO_INVIS",
    "INVISIBLE",
    "MAGIC",
    "NO_DROP",
    "BLESS",
    "ANTI_GOOD",
    "ANTI_EVIL",
    "ANTI_NEUTRAL",
    "ANTI_MAGE",
    "ANTI_CLERIC",
    "ANTI_THIEF",
    "ANTI_WARRIOR",
    "NO_SELL",
    "\n",
];

/* APPLY_x */
pub const APPLY_TYPES: [&str; 26] = [
    "NONE",
    "STR",
    "DEX",
    "INT",
    "WIS",
    "CON",
    "CHA",
    "CLASS",
    "LEVEL",
    "AGE",
    "CHAR_WEIGHT",
    "CHAR_HEIGHT",
    "MAXMANA",
    "MAXHIT",
    "MAXMOVE",
    "GOLD",
    "EXP",
    "ARMOR",
    "HITROLL",
    "DAMROLL",
    "SAVING_PARA",
    "SAVING_ROD",
    "SAVING_PETRI",
    "SAVING_BREATH",
    "SAVING_SPELL",
    "\n",
];

/* CONT_x */
pub const CONTAINER_BITS: [&str; 5] = ["CLOSEABLE", "PICKPROOF", "CLOSED", "LOCKED", "\n"];

/* LIQ_x */
pub const DRINKS: [&str; 17] = [
    "water",
    "beer",
    "wine",
    "ale",
    "dark ale",
    "whisky",
    "lemonade",
    "firebreather",
    "local speciality",
    "slime mold juice",
    "milk",
    "tea",
    "coffee",
    "blood",
    "salt water",
    "clear water",
    "\n",
];

/* other constants for liquids ******************************************/

/* one-word alias for each drink */
pub const DRINKNAMES: [&str; 17] = [
    "water",
    "beer",
    "wine",
    "ale",
    "ale",
    "whisky",
    "lemonade",
    "firebreather",
    "local",
    "juice",
    "milk",
    "tea",
    "coffee",
    "blood",
    "salt",
    "water",
    "\n",
];

/* effect of DRINKS on hunger, thirst, and drunkenness -- see values.doc */
pub const DRINK_AFF: [[i32; 3]; 16] = [
    [0, 1, 10],
    [3, 2, 5],
    [5, 2, 5],
    [2, 2, 5],
    [1, 2, 5],
    [6, 1, 4],
    [0, 1, 8],
    [10, 0, 0],
    [3, 3, 3],
    [0, 4, -8],
    [0, 3, 6],
    [0, 1, 6],
    [0, 1, 6],
    [0, 2, -1],
    [0, 1, -2],
    [0, 0, 13],
];

/* color of the various DRINKS */
pub const COLOR_LIQUID: [&str; 17] = [
    "clear",
    "brown",
    "clear",
    "brown",
    "dark",
    "golden",
    "red",
    "green",
    "clear",
    "light green",
    "white",
    "brown",
    "black",
    "red",
    "clear",
    "crystal clear",
    "\n",
];

/*
 * level of FULLNESS for drink containers
 * Not used in sprinttype() so no \n.
 */
pub const FULLNESS: [&str; 4] = ["less than half ", "about half ", "more than half ", ""];

/* str, int, wis, dex, con applies **************************************/

/* [ch] strength apply (all) */
pub const STR_APP: [StrAppType; 31] = [
    StrAppType {
        tohit: -5,
        todam: -4,
        carry_w: 0,
        wield_w: 0,
    }, /* str = 0 */
    StrAppType {
        tohit: -5,
        todam: -4,
        carry_w: 3,
        wield_w: 1,
    }, /* str = 1 */
    StrAppType {
        tohit: -3,
        todam: -2,
        carry_w: 3,
        wield_w: 2,
    },
    StrAppType {
        tohit: -3,
        todam: -1,
        carry_w: 10,
        wield_w: 3,
    },
    StrAppType {
        tohit: -2,
        todam: -1,
        carry_w: 25,
        wield_w: 4,
    },
    StrAppType {
        tohit: -2,
        todam: -1,
        carry_w: 55,
        wield_w: 5,
    }, /* str = 5 */
    StrAppType {
        tohit: -1,
        todam: 0,
        carry_w: 80,
        wield_w: 6,
    },
    StrAppType {
        tohit: -1,
        todam: 0,
        carry_w: 90,
        wield_w: 7,
    },
    StrAppType {
        tohit: 0,
        todam: 0,
        carry_w: 100,
        wield_w: 8,
    },
    StrAppType {
        tohit: 0,
        todam: 0,
        carry_w: 100,
        wield_w: 9,
    },
    StrAppType {
        tohit: 0,
        todam: 0,
        carry_w: 115,
        wield_w: 10,
    }, /* str = 10 */
    StrAppType {
        tohit: 0,
        todam: 0,
        carry_w: 115,
        wield_w: 11,
    },
    StrAppType {
        tohit: 0,
        todam: 0,
        carry_w: 140,
        wield_w: 12,
    },
    StrAppType {
        tohit: 0,
        todam: 0,
        carry_w: 140,
        wield_w: 13,
    },
    StrAppType {
        tohit: 0,
        todam: 0,
        carry_w: 170,
        wield_w: 14,
    },
    StrAppType {
        tohit: 0,
        todam: 0,
        carry_w: 170,
        wield_w: 15,
    }, /* str = 15 */
    StrAppType {
        tohit: 0,
        todam: 1,
        carry_w: 195,
        wield_w: 16,
    },
    StrAppType {
        tohit: 1,
        todam: 1,
        carry_w: 220,
        wield_w: 18,
    },
    StrAppType {
        tohit: 1,
        todam: 2,
        carry_w: 255,
        wield_w: 20,
    }, /* str = 18 */
    StrAppType {
        tohit: 3,
        todam: 7,
        carry_w: 640,
        wield_w: 40,
    },
    StrAppType {
        tohit: 3,
        todam: 8,
        carry_w: 700,
        wield_w: 40,
    }, /* str = 20 */
    StrAppType {
        tohit: 4,
        todam: 9,
        carry_w: 810,
        wield_w: 40,
    },
    StrAppType {
        tohit: 4,
        todam: 10,
        carry_w: 970,
        wield_w: 40,
    },
    StrAppType {
        tohit: 5,
        todam: 11,
        carry_w: 1130,
        wield_w: 40,
    },
    StrAppType {
        tohit: 6,
        todam: 12,
        carry_w: 1440,
        wield_w: 40,
    },
    StrAppType {
        tohit: 7,
        todam: 14,
        carry_w: 1750,
        wield_w: 40,
    }, /* str = 25 */
    StrAppType {
        tohit: 1,
        todam: 3,
        carry_w: 280,
        wield_w: 22,
    }, /* str = 18/0 - 18-50 */
    StrAppType {
        tohit: 2,
        todam: 3,
        carry_w: 305,
        wield_w: 24,
    }, /* str = 18/51 - 18-75 */
    StrAppType {
        tohit: 2,
        todam: 4,
        carry_w: 330,
        wield_w: 26,
    }, /* str = 18/76 - 18-90 */
    StrAppType {
        tohit: 2,
        todam: 5,
        carry_w: 380,
        wield_w: 28,
    }, /* str = 18/91 - 18-99 */
    StrAppType {
        tohit: 3,
        todam: 6,
        carry_w: 480,
        wield_w: 30,
    }, /* str = 18/100 */
];

/* [dex] skill apply (thieves only) */
pub const DEX_APP_SKILL: [DexSkillType; 26] = [
    DexSkillType {
        p_pocket: -99,
        p_locks: -99,
        //traps: -90,
        sneak: -99,
        hide: -60,
    }, /* dex = 0 */
    DexSkillType {
        p_pocket: -90,
        p_locks: -90,
        //traps: -60,
        sneak: -90,
        hide: -50,
    }, /* dex = 1 */
    DexSkillType {
        p_pocket: -80,
        p_locks: -80,
        //traps: -40,
        sneak: -80,
        hide: -45,
    },
    DexSkillType {
        p_pocket: -70,
        p_locks: -70,
        //traps: -30,
        sneak: -70,
        hide: -40,
    },
    DexSkillType {
        p_pocket: -60,
        p_locks: -60,
        //traps: -30,
        sneak: -60,
        hide: -35,
    },
    DexSkillType {
        p_pocket: -50,
        p_locks: -50,
        //traps: -20,
        sneak: -50,
        hide: -30,
    }, /* dex = 5 */
    DexSkillType {
        p_pocket: -40,
        p_locks: -40,
        //traps: -20,
        sneak: -40,
        hide: -25,
    },
    DexSkillType {
        p_pocket: -30,
        p_locks: -30,
        //traps: -15,
        sneak: -30,
        hide: -20,
    },
    DexSkillType {
        p_pocket: -20,
        p_locks: -20,
        //traps: -15,
        sneak: -20,
        hide: -15,
    },
    DexSkillType {
        p_pocket: -15,
        p_locks: -10,
        //traps: -10,
        sneak: -20,
        hide: -10,
    },
    DexSkillType {
        p_pocket: -10,
        p_locks: -5,
        //traps: -10,
        sneak: -15,
        hide: -5,
    }, /* dex = 10 */
    DexSkillType {
        p_pocket: -5,
        p_locks: 0,
        //traps: -5,
        sneak: -10,
        hide: 0,
    },
    DexSkillType {
        p_pocket: 0,
        p_locks: 0,
        //traps: 0,
        sneak: -5,
        hide: 0,
    },
    DexSkillType {
        p_pocket: 0,
        p_locks: 0,
        //traps: 0,
        sneak: 0,
        hide: 0,
    },
    DexSkillType {
        p_pocket: 0,
        p_locks: 0,
        //traps: 0,
        sneak: 0,
        hide: 0,
    },
    DexSkillType {
        p_pocket: 0,
        p_locks: 0,
        //traps: 0,
        sneak: 0,
        hide: 0,
    }, /* dex = 15 */
    DexSkillType {
        p_pocket: 0,
        p_locks: 5,
        //traps: 0,
        sneak: 0,
        hide: 0,
    },
    DexSkillType {
        p_pocket: 5,
        p_locks: 10,
        //traps: 0,
        sneak: 5,
        hide: 5,
    },
    DexSkillType {
        p_pocket: 10,
        p_locks: 15,
        //traps: 5,
        sneak: 10,
        hide: 10,
    }, /* dex = 18 */
    DexSkillType {
        p_pocket: 15,
        p_locks: 20,
        //traps: 10,
        sneak: 15,
        hide: 15,
    },
    DexSkillType {
        p_pocket: 15,
        p_locks: 20,
        //traps: 10,
        sneak: 15,
        hide: 15,
    }, /* dex = 20 */
    DexSkillType {
        p_pocket: 20,
        p_locks: 25,
        //traps: 10,
        sneak: 15,
        hide: 20,
    },
    DexSkillType {
        p_pocket: 20,
        p_locks: 25,
        //traps: 15,
        sneak: 20,
        hide: 20,
    },
    DexSkillType {
        p_pocket: 25,
        p_locks: 25,
        //traps: 15,
        sneak: 20,
        hide: 20,
    },
    DexSkillType {
        p_pocket: 25,
        p_locks: 30,
        //traps: 15,
        sneak: 25,
        hide: 25,
    },
    DexSkillType {
        p_pocket: 25,
        p_locks: 30,
        //traps: 15,
        sneak: 25,
        hide: 25,
    }, /* dex = 25 */
];

/* [dex] apply (all) */
pub const DEX_APP: [DexAppType; 26] = [
    DexAppType {
        //reaction: -7,
        //miss_att: -7,
        defensive: 6,
    }, /* dex = 0 */
    DexAppType {
        //reaction: -6,
        //miss_att: -6,
        defensive: 5,
    }, /* dex = 1 */
    DexAppType {
        //reaction: -4,
        //miss_att: -4,
        defensive: 5,
    },
    DexAppType {
        //reaction: -3,
        //miss_att: -3,
        defensive: 4,
    },
    DexAppType {
        //reaction: -2,
        //miss_att: -2,
        defensive: 3,
    },
    DexAppType {
        //reaction: -1,
        //miss_att: -1,
        defensive: 2,
    }, /* dex = 5 */
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 1,
    },
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 0,
    },
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 0,
    },
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 0,
    },
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 0,
    }, /* dex = 10 */
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 0,
    },
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 0,
    },
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 0,
    },
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: 0,
    },
    DexAppType {
        //reaction: 0,
        //miss_att: 0,
        defensive: -1,
    }, /* dex = 15 */
    DexAppType {
        //reaction: 1,
        //miss_att: 1,
        defensive: -2,
    },
    DexAppType {
        //reaction: 2,
        //miss_att: 2,
        defensive: -3,
    },
    DexAppType {
        //reaction: 2,
        //miss_att: 2,
        defensive: -4,
    }, /* dex = 18 */
    DexAppType {
        //reaction: 3,
        //miss_att: 3,
        defensive: -4,
    },
    DexAppType {
        //reaction: 3,
        //miss_att: 3,
        defensive: -4,
    }, /* dex = 20 */
    DexAppType {
        //reaction: 4,
        //miss_att: 4,
        defensive: -5,
    },
    DexAppType {
        //reaction: 4,
        //miss_att: 4,
        defensive: -5,
    },
    DexAppType {
        //reaction: 4,
        //miss_att: 4,
        defensive: -5,
    },
    DexAppType {
        //reaction: 5,
        //miss_att: 5,
        defensive: -6,
    },
    DexAppType {
        //reaction: 5,
        //miss_att: 5,
        defensive: -6,
    }, /* dex = 25 */
];

/* [con] apply (all) */
pub const CON_APP: [ConAppType; 26] = [
    ConAppType {
        hitp: -4,
        /*shock: 20*/
    }, /* con = 0 */
    ConAppType {
        hitp: -3,
        /*shock: 25*/
    }, /* con = 1 */
    ConAppType {
        hitp: -2,
        /*shock: 30*/
    },
    ConAppType {
        hitp: -2,
        /*shock: 35*/
    },
    ConAppType {
        hitp: -1,
        /*shock: 40*/
    },
    ConAppType {
        hitp: -1,
        /*shock: 45*/
    }, /* con = 5 */
    ConAppType {
        hitp: -1,
        /*shock: 50*/
    },
    ConAppType {
        hitp: 0, /*shock: 55*/
    },
    ConAppType {
        hitp: 0, /*shock: 60*/
    },
    ConAppType {
        hitp: 0, /*shock: 65*/
    },
    ConAppType {
        hitp: 0, /*shock: 70*/
    }, /* con = 10 */
    ConAppType {
        hitp: 0, /*shock: 75*/
    },
    ConAppType {
        hitp: 0, /*shock: 80*/
    },
    ConAppType {
        hitp: 0, /*shock: 85*/
    },
    ConAppType {
        hitp: 0, /*shock: 88*/
    },
    ConAppType {
        hitp: 1, /*shock: 90*/
    }, /* con = 15 */
    ConAppType {
        hitp: 2, /*shock: 95*/
    },
    ConAppType {
        hitp: 2, /*shock: 97*/
    },
    ConAppType {
        hitp: 3, /*shock: 99*/
    }, /* con = 18 */
    ConAppType {
        hitp: 3, /*shock: 99*/
    },
    ConAppType {
        hitp: 4, /*shock: 99*/
    }, /* con = 20 */
    ConAppType {
        hitp: 5, /*shock: 99*/
    },
    ConAppType {
        hitp: 5, /*shock: 99*/
    },
    ConAppType {
        hitp: 5, /*shock: 99*/
    },
    ConAppType {
        hitp: 6, /*shock: 99*/
    },
    ConAppType {
        hitp: 6, /*shock: 99*/
    }, /* con = 25 */
];

/* [int] apply (all) */
pub const INT_APP: [IntAppType; 26] = [
    IntAppType { learn: 3 }, /* int = 0 */
    IntAppType { learn: 5 }, /* int = 1 */
    IntAppType { learn: 7 },
    IntAppType { learn: 8 },
    IntAppType { learn: 9 },
    IntAppType { learn: 10 }, /* int = 5 */
    IntAppType { learn: 11 },
    IntAppType { learn: 12 },
    IntAppType { learn: 13 },
    IntAppType { learn: 15 },
    IntAppType { learn: 17 }, /* int = 10 */
    IntAppType { learn: 19 },
    IntAppType { learn: 22 },
    IntAppType { learn: 25 },
    IntAppType { learn: 30 },
    IntAppType { learn: 35 }, /* int = 15 */
    IntAppType { learn: 40 },
    IntAppType { learn: 45 },
    IntAppType { learn: 50 }, /* int = 18 */
    IntAppType { learn: 53 },
    IntAppType { learn: 55 }, /* int = 20 */
    IntAppType { learn: 56 },
    IntAppType { learn: 57 },
    IntAppType { learn: 58 },
    IntAppType { learn: 59 },
    IntAppType { learn: 60 }, /* int = 25 */
];

/* [wis] apply (all) */
pub const WIS_APP: [WisAppType; 26] = [
    WisAppType { bonus: 0 },
    /* wis = 0 */ WisAppType { bonus: 0 },
    /* wis = 1 */ WisAppType { bonus: 0 },
    WisAppType { bonus: 0 },
    WisAppType { bonus: 0 },
    WisAppType { bonus: 0 },
    /* wis = 5 */ WisAppType { bonus: 0 },
    WisAppType { bonus: 0 },
    WisAppType { bonus: 0 },
    WisAppType { bonus: 0 },
    WisAppType { bonus: 0 },
    /* wis = 10 */ WisAppType { bonus: 0 },
    WisAppType { bonus: 2 },
    WisAppType { bonus: 2 },
    WisAppType { bonus: 3 },
    WisAppType { bonus: 3 },
    /* wis = 15 */ WisAppType { bonus: 3 },
    WisAppType { bonus: 4 },
    WisAppType { bonus: 5 },
    /* wis = 18 */ WisAppType { bonus: 6 },
    WisAppType { bonus: 6 },
    /* wis = 20 */ WisAppType { bonus: 6 },
    WisAppType { bonus: 6 },
    WisAppType { bonus: 7 },
    WisAppType { bonus: 7 },
    WisAppType { bonus: 7 }, /* wis = 25 */
];

pub const NPC_CLASS_TYPES: [&str; 3] = ["Normal", "Undead", "\n"];

pub const REV_DIR: [i32; 6] = [2, 3, 0, 1, 5, 4];

pub const MOVEMENT_LOSS: [i32; 10] = [
    1, /* Inside     */
    1, /* City       */
    2, /* Field      */
    3, /* Forest     */
    4, /* Hills      */
    6, /* Mountains  */
    4, /* Swimming   */
    1, /* Unswimable */
    1, /* Flying     */
    5, /* Underwater */
];

/* Not used in sprinttype(). */
pub const WEEKDAYS: [&str; 7] = [
    "the Day of the Moon",
    "the Day of the Bull",
    "the Day of the Deception",
    "the Day of Thunder",
    "the Day of Freedom",
    "the Day of the Great Gods",
    "the Day of the Sun",
];

/* Not used in sprinttype(). */
pub const MONTH_NAME: [&str; 17] = [
    "Month of Winter", /* 0 */
    "Month of the Winter Wolf",
    "Month of the Frost Giant",
    "Month of the Old Forces",
    "Month of the Grand Struggle",
    "Month of the Spring",
    "Month of Nature",
    "Month of Futility",
    "Month of the Dragon",
    "Month of the Sun",
    "Month of the Heat",
    "Month of the Battle",
    "Month of the Dark Shades",
    "Month of the Shadows",
    "Month of the Long Shadows",
    "Month of the Ancient Darkness",
    "Month of the Great Evil",
];

/* --- End of constants arrays. --- */

/*
 * Various arrays we count so we can check the world files.  These
 * must be at the bottom of the file so they're pre-declared.
 */
pub const ACTION_BITS_COUNT: usize = ACTION_BITS.len() - 1;
pub const ROOM_BITS_COUNT: usize = ROOM_BITS.len() - 1;
pub const AFFECTED_BITS_COUNT: usize = AFFECTED_BITS.len() - 1;
pub const EXTRA_BITS_COUNT: usize = EXTRA_BITS.len() - 1;
pub const WEAR_BITS_COUNT: usize = WEAR_BITS.len() - 1;
