/* ************************************************************************
*   File: interpreter.c                                 Part of CircleMUD *
*  Usage: parse user commands, search for specials, call ACMD functions   *
*                                                                         *
*  All rights RESERVED.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::cell::Cell;
use std::cmp::max;
use std::rc::Rc;

use hmac::Hmac;
use log::error;
use sha2::Sha256;

use crate::act_informative::{
    do_color, do_commands, do_consider, do_diagnose, do_equipment, do_exits, do_gold, do_inventory,
    do_levels, do_look, do_score, do_time, do_weather,
};
use crate::act_item::{do_drop, do_get, do_remove, do_wear, do_wield};
use crate::act_movement::do_move;
use crate::act_offensive::{do_flee, do_hit};
use crate::act_other::{do_not_here, do_quit};
use crate::ban::valid_name;
use crate::class::{parse_class, CLASS_MENU};
use crate::config::{MAX_BAD_PWS, MENU, START_MESSG, WELC_MESSG};
use crate::db::{clear_char, reset_char, store_to_char};
use crate::screen::{C_SPR, KNRM, KNUL, KRED};
use crate::structs::ConState::{
    ConChpwdGetnew, ConChpwdGetold, ConChpwdVrfy, ConClose, ConCnfpasswd, ConDisconnect,
    ConGetName, ConMenu, ConNameCnfrm, ConNewpasswd, ConPassword, ConQclass, ConQsex, ConRmotd,
};
use crate::structs::ConState::{ConDelcnf1, ConExdesc, ConPlaying};
use crate::structs::{
    CharData, AFF_HIDE, LVL_GOD, LVL_IMPL, MOB_NOTDEADYET, NOWHERE, PLR_FROZEN, PLR_INVSTART,
    PLR_LOADROOM, POS_DEAD, POS_FIGHTING, POS_INCAP, POS_MORTALLYW, POS_RESTING, POS_SITTING,
    POS_SLEEPING, POS_STANDING, POS_STUNNED,
};
use crate::structs::{
    CharFileU, AFF_GROUP, CLASS_UNDEFINED, EXDSCR_LENGTH, LVL_IMMORT, MAX_NAME_LENGTH,
    MAX_PWD_LENGTH, PLR_CRYO, PLR_MAILING, PLR_WRITING, PRF_COLOR_1, PRF_COLOR_2, SEX_FEMALE,
    SEX_MALE,
};
use crate::util::{BRF, NRM};
use crate::{
    _clrlevel, clr, send_to_char, DescriptorData, MainGlobals, CCNRM, CCRED, PLR_DELETED, TO_ROOM,
};
use crate::{echo_off, echo_on, write_to_output};

/*
 * SUBCOMMANDS
 *   You can define these however you want to, and the definitions of the
 *   subcommands are independent from function to function.
 */

/* directions */
pub const SCMD_NORTH: i32 = 1;
pub const SCMD_EAST: i32 = 2;
pub const SCMD_SOUTH: i32 = 3;
pub const SCMD_WEST: i32 = 4;
pub const SCMD_UP: i32 = 5;
pub const SCMD_DOWN: i32 = 6;

/* do_quit */
pub const SCMD_QUI: i32 = 0;
pub const SCMD_QUIT: i32 = 1;

/* do_commands */
pub const SCMD_COMMANDS: i32 = 0;
pub const SCMD_SOCIALS: i32 = 1;
pub const SCMD_WIZHELP: i32 = 2;

/* do_drop */
pub const SCMD_DROP: u8 = 0;
pub const SCMD_JUNK: u8 = 1;
pub const SCMD_DONATE: u8 = 2;

/* do_hit */
pub const SCMD_HIT: i32 = 0;
pub const SCMD_MURDER: i32 = 1;

/* do_look */
pub const SCMD_LOOK: i32 = 0;
pub const SCMD_READ: i32 = 1;

pub fn cmd_is(cmd: i32, cmd_name: &str) -> bool {
    CMD_INFO[cmd as usize].command == cmd_name
}

/* This is the Master Command List(tm).

* You can put new commands in, take commands out, change the order
* they appear in, etc.  You can adjust the "priority" of commands
* simply by changing the order they appear in the command list.
* (For example, if you want "as" to mean "assist" instead of "ask",
* just put "assist" above "ask" in the Master Command List(tm).
*
* In general, utility commands such as "at" should have high priority;
* infrequently used and dangerously destructive commands should have low
* priority.
*/
type Command = fn(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32);

pub struct CommandInfo {
    pub(crate) command: &'static str,
    minimum_position: u8,
    pub(crate) command_pointer: Command,
    pub(crate) minimum_level: i16,
    subcmd: i32,
}

#[allow(unused_variables)]
pub fn do_nothing(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {}

pub const CMD_INFO: [CommandInfo; 38] = [
    CommandInfo {
        command: "",
        minimum_position: 0,
        command_pointer: do_nothing,
        minimum_level: 0,
        subcmd: 0,
    },
    /* directions must come before other commands but after RESERVED */
    CommandInfo {
        command: "north",
        minimum_position: POS_STANDING,
        command_pointer: do_move,
        minimum_level: 0,
        subcmd: SCMD_NORTH,
    },
    CommandInfo {
        command: "east",
        minimum_position: POS_STANDING,
        command_pointer: do_move,
        minimum_level: 0,
        subcmd: SCMD_EAST,
    },
    CommandInfo {
        command: "south",
        minimum_position: POS_STANDING,
        command_pointer: do_move,
        minimum_level: 0,
        subcmd: SCMD_SOUTH,
    },
    CommandInfo {
        command: "west",
        minimum_position: POS_STANDING,
        command_pointer: do_move,
        minimum_level: 0,
        subcmd: SCMD_WEST,
    },
    CommandInfo {
        command: "up",
        minimum_position: POS_STANDING,
        command_pointer: do_move,
        minimum_level: 0,
        subcmd: SCMD_UP,
    },
    CommandInfo {
        command: "down",
        minimum_position: POS_STANDING,
        command_pointer: do_move,
        minimum_level: 0,
        subcmd: SCMD_DOWN,
    },
    /* now, the main list */
    // { "at"       , POS_DEAD    , do_at       , LVL_IMMORT, 0 },
    // { "advance"  , POS_DEAD    , do_advance  , LVL_IMPL, 0 },
    // { "alias"    , POS_DEAD    , do_alias    , 0, 0 },
    // { "accuse"   , POS_SITTING , do_action   , 0, 0 },
    // { "applaud"  , POS_RESTING , do_action   , 0, 0 },
    // { "assist"   , POS_FIGHTING, do_assist   , 1, 0 },
    // { "ask"      , POS_RESTING , do_spec_comm, 0, SCMD_ASK },
    // { "auction"  , POS_SLEEPING, do_gen_comm , 0, SCMD_AUCTION },
    // { "autoexit" , POS_DEAD    , do_gen_tog  , 0, SCMD_AUTOEXIT },
    //
    // { "bounce"   , POS_STANDING, do_action   , 0, 0 },
    // { "backstab" , POS_STANDING, do_backstab , 1, 0 },
    // { "ban"      , POS_DEAD    , do_ban      , LVL_GRGOD, 0 },
    // { "balance"  , POS_STANDING, do_not_here , 1, 0 },
    // { "bash"     , POS_FIGHTING, do_bash     , 1, 0 },
    // { "beg"      , POS_RESTING , do_action   , 0, 0 },
    // { "bleed"    , POS_RESTING , do_action   , 0, 0 },
    // { "blush"    , POS_RESTING , do_action   , 0, 0 },
    // { "bow"      , POS_STANDING, do_action   , 0, 0 },
    // { "brb"      , POS_RESTING , do_action   , 0, 0 },
    // { "brief"    , POS_DEAD    , do_gen_tog  , 0, SCMD_BRIEF },
    // { "burp"     , POS_RESTING , do_action   , 0, 0 },
    // { "buy"      , POS_STANDING, do_not_here , 0, 0 },
    CommandInfo {
        command: "buy",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "bug"      , POS_DEAD    , do_gen_write, 0, SCMD_BUG },
    //
    // { "cast"     , POS_SITTING , do_cast     , 1, 0 },
    // { "cackle"   , POS_RESTING , do_action   , 0, 0 },
    // { "check"    , POS_STANDING, do_not_here , 1, 0 },
    // { "chuckle"  , POS_RESTING , do_action   , 0, 0 },
    // { "clap"     , POS_RESTING , do_action   , 0, 0 },
    // { "clear"    , POS_DEAD    , do_gen_ps   , 0, SCMD_CLEAR },
    // { "close"    , POS_SITTING , do_gen_door , 0, SCMD_CLOSE },
    // { "cls"      , POS_DEAD    , do_gen_ps   , 0, SCMD_CLEAR },
    // { "consider" , POS_RESTING , do_consider , 0, 0 },
    CommandInfo {
        command: "consider",
        minimum_position: POS_RESTING,
        command_pointer: do_consider,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "color"    , POS_DEAD    , do_color    , 0, 0 },
    CommandInfo {
        command: "color",
        minimum_position: POS_DEAD,
        command_pointer: do_color,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "comfort"  , POS_RESTING , do_action   , 0, 0 },
    // { "comb"     , POS_RESTING , do_action   , 0, 0 },
    // { "commands" , POS_DEAD    , do_commands , 0, SCMD_COMMANDS },
    CommandInfo {
        command: "commands",
        minimum_position: POS_DEAD,
        command_pointer: do_commands,
        minimum_level: 0,
        subcmd: SCMD_COMMANDS,
    },
    // { "compact"  , POS_DEAD    , do_gen_tog  , 0, SCMD_COMPACT },
    // { "cough"    , POS_RESTING , do_action   , 0, 0 },
    // { "credits"  , POS_DEAD    , do_gen_ps   , 0, SCMD_CREDITS },
    // { "cringe"   , POS_RESTING , do_action   , 0, 0 },
    // { "cry"      , POS_RESTING , do_action   , 0, 0 },
    // { "cuddle"   , POS_RESTING , do_action   , 0, 0 },
    // { "curse"    , POS_RESTING , do_action   , 0, 0 },
    // { "curtsey"  , POS_STANDING, do_action   , 0, 0 },
    //
    // { "dance"    , POS_STANDING, do_action   , 0, 0 },
    // { "date"     , POS_DEAD    , do_date     , LVL_IMMORT, SCMD_DATE },
    // { "daydream" , POS_SLEEPING, do_action   , 0, 0 },
    // { "dc"       , POS_DEAD    , do_dc       , LVL_GOD, 0 },
    // { "deposit"  , POS_STANDING, do_not_here , 1, 0 },
    // { "diagnose" , POS_RESTING , do_diagnose , 0, 0 },
    CommandInfo {
        command: "diagnose",
        minimum_position: POS_RESTING,
        command_pointer: do_diagnose,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "display"  , POS_DEAD    , do_display  , 0, 0 },
    // { "donate"   , POS_RESTING , do_drop     , 0, SCMD_DONATE },
    CommandInfo {
        command: "donate",
        minimum_position: POS_RESTING,
        command_pointer: do_drop,
        minimum_level: 0,
        subcmd: SCMD_DONATE as i32,
    },
    // { "drink"    , POS_RESTING , do_drink    , 0, SCMD_DRINK },
    // { "drop"     , POS_RESTING , do_drop     , 0, SCMD_DROP },
    CommandInfo {
        command: "drop",
        minimum_position: POS_RESTING,
        command_pointer: do_drop,
        minimum_level: 0,
        subcmd: SCMD_DROP as i32,
    },
    // { "drool"    , POS_RESTING , do_action   , 0, 0 },
    //
    // { "eat"      , POS_RESTING , do_eat      , 0, SCMD_EAT },
    // { "echo"     , POS_SLEEPING, do_echo     , LVL_IMMORT, SCMD_ECHO },
    // { "emote"    , POS_RESTING , do_echo     , 1, SCMD_EMOTE },
    // { ":"        , POS_RESTING, do_echo      , 1, SCMD_EMOTE },
    // { "embrace"  , POS_STANDING, do_action   , 0, 0 },
    // { "enter"    , POS_STANDING, do_enter    , 0, 0 },
    // { "equipment", POS_SLEEPING, do_equipment, 0, 0 },
    CommandInfo {
        command: "equipment",
        minimum_position: POS_SLEEPING,
        command_pointer: do_equipment,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "exits"    , POS_RESTING , do_exits    , 0, 0 },
    CommandInfo {
        command: "exits",
        minimum_position: POS_RESTING,
        command_pointer: do_exits,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "examine"  , POS_SITTING , do_examine  , 0, 0 },
    //
    // { "force"    , POS_SLEEPING, do_force    , LVL_GOD, 0 },
    // { "fart"     , POS_RESTING , do_action   , 0, 0 },
    // { "FILL"     , POS_STANDING, do_pour     , 0, SCMD_FILL },
    // { "flee"     , POS_FIGHTING, do_flee     , 1, 0 },
    CommandInfo {
        command: "flee",
        minimum_position: POS_FIGHTING,
        command_pointer: do_flee,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "flip"     , POS_STANDING, do_action   , 0, 0 },
    // { "flirt"    , POS_RESTING , do_action   , 0, 0 },
    // { "follow"   , POS_RESTING , do_follow   , 0, 0 },
    // { "fondle"   , POS_RESTING , do_action   , 0, 0 },
    // { "freeze"   , POS_DEAD    , do_wizutil  , LVL_FREEZE, SCMD_FREEZE },
    // { "french"   , POS_RESTING , do_action   , 0, 0 },
    // { "frown"    , POS_RESTING , do_action   , 0, 0 },
    // { "fume"     , POS_RESTING , do_action   , 0, 0 },
    //
    // { "get"      , POS_RESTING , do_get      , 0, 0 },
    CommandInfo {
        command: "get",
        minimum_position: POS_RESTING,
        command_pointer: do_get,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "gasp"     , POS_RESTING , do_action   , 0, 0 },
    // { "gecho"    , POS_DEAD    , do_gecho    , LVL_GOD, 0 },
    // { "give"     , POS_RESTING , do_give     , 0, 0 },
    // { "giggle"   , POS_RESTING , do_action   , 0, 0 },
    // { "glare"    , POS_RESTING , do_action   , 0, 0 },
    // { "goto"     , POS_SLEEPING, do_goto     , LVL_IMMORT, 0 },
    // { "gold"     , POS_RESTING , do_gold     , 0, 0 },
    CommandInfo {
        command: "gold",
        minimum_position: POS_RESTING,
        command_pointer: do_gold,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "gossip"   , POS_SLEEPING, do_gen_comm , 0, SCMD_GOSSIP },
    // { "group"    , POS_RESTING , do_group    , 1, 0 },
    // { "grab"     , POS_RESTING , do_grab     , 0, 0 },
    // { "grats"    , POS_SLEEPING, do_gen_comm , 0, SCMD_GRATZ },
    // { "greet"    , POS_RESTING , do_action   , 0, 0 },
    // { "grin"     , POS_RESTING , do_action   , 0, 0 },
    // { "groan"    , POS_RESTING , do_action   , 0, 0 },
    // { "grope"    , POS_RESTING , do_action   , 0, 0 },
    // { "grovel"   , POS_RESTING , do_action   , 0, 0 },
    // { "growl"    , POS_RESTING , do_action   , 0, 0 },
    // { "gsay"     , POS_SLEEPING, do_gsay     , 0, 0 },
    // { "gtell"    , POS_SLEEPING, do_gsay     , 0, 0 },
    //
    // { "help"     , POS_DEAD    , do_help     , 0, 0 },
    // { "handbook" , POS_DEAD    , do_gen_ps   , LVL_IMMORT, SCMD_HANDBOOK },
    // { "hcontrol" , POS_DEAD    , do_hcontrol , LVL_GRGOD, 0 },
    // { "hiccup"   , POS_RESTING , do_action   , 0, 0 },
    // { "hide"     , POS_RESTING , do_hide     , 1, 0 },
    // { "hit"      , POS_FIGHTING, do_hit      , 0, SCMD_HIT },
    CommandInfo {
        command: "hit",
        minimum_position: POS_FIGHTING,
        command_pointer: do_hit,
        minimum_level: 0,
        subcmd: SCMD_HIT,
    },
    // { "hold"     , POS_RESTING , do_grab     , 1, 0 },
    // { "holler"   , POS_RESTING , do_gen_comm , 1, SCMD_HOLLER },
    // { "holylight", POS_DEAD    , do_gen_tog  , LVL_IMMORT, SCMD_HOLYLIGHT },
    // { "hop"      , POS_RESTING , do_action   , 0, 0 },
    // { "house"    , POS_RESTING , do_house    , 0, 0 },
    // { "hug"      , POS_RESTING , do_action   , 0, 0 },
    //
    // { "inventory", POS_DEAD    , do_inventory, 0, 0 },
    CommandInfo {
        command: "inventory",
        minimum_position: POS_DEAD,
        command_pointer: do_inventory,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "idea"     , POS_DEAD    , do_gen_write, 0, SCMD_IDEA },
    // { "imotd"    , POS_DEAD    , do_gen_ps   , LVL_IMMORT, SCMD_IMOTD },
    // { "immlist"  , POS_DEAD    , do_gen_ps   , 0, SCMD_IMMLIST },
    // { "info"     , POS_SLEEPING, do_gen_ps   , 0, SCMD_INFO },
    // { "insult"   , POS_RESTING , do_insult   , 0, 0 },
    // { "invis"    , POS_DEAD    , do_invis    , LVL_IMMORT, 0 },
    //
    // { "junk"     , POS_RESTING , do_drop     , 0, SCMD_JUNK },
    CommandInfo {
        command: "junk",
        minimum_position: POS_RESTING,
        command_pointer: do_drop,
        minimum_level: 0,
        subcmd: SCMD_JUNK as i32,
    },
    // { "kill"     , POS_FIGHTING, do_kill     , 0, 0 },
    // { "kick"     , POS_FIGHTING, do_kick     , 1, 0 },
    // { "kiss"     , POS_RESTING , do_action   , 0, 0 },
    //
    // { "look"     , POS_RESTING , do_look     , 0, SCMD_LOOK },
    CommandInfo {
        command: "look",
        minimum_position: POS_RESTING,
        command_pointer: do_look,
        minimum_level: 0,
        subcmd: SCMD_LOOK,
    },
    // { "laugh"    , POS_RESTING , do_action   , 0, 0 },
    // { "last"     , POS_DEAD    , do_last     , LVL_GOD, 0 },
    // { "leave"    , POS_STANDING, do_leave    , 0, 0 },
    // { "levels"   , POS_DEAD    , do_levels   , 0, 0 },
    CommandInfo {
        command: "levels",
        minimum_position: POS_DEAD,
        command_pointer: do_levels,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "list"     , POS_STANDING, do_not_here , 0, 0 },
    CommandInfo {
        command: "list",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "lick"     , POS_RESTING , do_action   , 0, 0 },
    // { "lock"     , POS_SITTING , do_gen_door , 0, SCMD_LOCK },
    // { "load"     , POS_DEAD    , do_load     , LVL_GOD, 0 },
    // { "love"     , POS_RESTING , do_action   , 0, 0 },
    //
    // { "moan"     , POS_RESTING , do_action   , 0, 0 },
    // { "motd"     , POS_DEAD    , do_gen_ps   , 0, SCMD_MOTD },
    // { "mail"     , POS_STANDING, do_not_here , 1, 0 },
    // { "massage"  , POS_RESTING , do_action   , 0, 0 },
    // { "mute"     , POS_DEAD    , do_wizutil  , LVL_GOD, SCMD_SQUELCH },
    // { "murder"   , POS_FIGHTING, do_hit      , 0, SCMD_MURDER },
    CommandInfo {
        command: "murder",
        minimum_position: POS_FIGHTING,
        command_pointer: do_hit,
        minimum_level: 0,
        subcmd: SCMD_MURDER,
    },
    //
    // { "news"     , POS_SLEEPING, do_gen_ps   , 0, SCMD_NEWS },
    // { "nibble"   , POS_RESTING , do_action   , 0, 0 },
    // { "nod"      , POS_RESTING , do_action   , 0, 0 },
    // { "noauction", POS_DEAD    , do_gen_tog  , 0, SCMD_NOAUCTION },
    // { "nogossip" , POS_DEAD    , do_gen_tog  , 0, SCMD_NOGOSSIP },
    // { "nograts"  , POS_DEAD    , do_gen_tog  , 0, SCMD_NOGRATZ },
    // { "nohassle" , POS_DEAD    , do_gen_tog  , LVL_IMMORT, SCMD_NOHASSLE },
    // { "norepeat" , POS_DEAD    , do_gen_tog  , 0, SCMD_NOREPEAT },
    // { "noshout"  , POS_SLEEPING, do_gen_tog  , 1, SCMD_DEAF },
    // { "nosummon" , POS_DEAD    , do_gen_tog  , 1, SCMD_NOSUMMON },
    // { "notell"   , POS_DEAD    , do_gen_tog  , 1, SCMD_NOTELL },
    // { "notitle"  , POS_DEAD    , do_wizutil  , LVL_GOD, SCMD_NOTITLE },
    // { "nowiz"    , POS_DEAD    , do_gen_tog  , LVL_IMMORT, SCMD_NOWIZ },
    // { "nudge"    , POS_RESTING , do_action   , 0, 0 },
    // { "nuzzle"   , POS_RESTING , do_action   , 0, 0 },
    //
    // { "olc"      , POS_DEAD    , do_olc      , LVL_IMPL, 0 },
    // { "order"    , POS_RESTING , do_order    , 1, 0 },
    // { "offer"    , POS_STANDING, do_not_here , 1, 0 },
    // { "open"     , POS_SITTING , do_gen_door , 0, SCMD_OPEN },
    //
    // { "put"      , POS_RESTING , do_put      , 0, 0 },
    // { "pat"      , POS_RESTING , do_action   , 0, 0 },
    // { "page"     , POS_DEAD    , do_page     , LVL_GOD, 0 },
    // { "pardon"   , POS_DEAD    , do_wizutil  , LVL_GOD, SCMD_PARDON },
    // { "peer"     , POS_RESTING , do_action   , 0, 0 },
    // { "pick"     , POS_STANDING, do_gen_door , 1, SCMD_PICK },
    // { "point"    , POS_RESTING , do_action   , 0, 0 },
    // { "poke"     , POS_RESTING , do_action   , 0, 0 },
    // { "policy"   , POS_DEAD    , do_gen_ps   , 0, SCMD_POLICIES },
    // { "ponder"   , POS_RESTING , do_action   , 0, 0 },
    // { "poofin"   , POS_DEAD    , do_poofset  , LVL_IMMORT, SCMD_POOFIN },
    // { "poofout"  , POS_DEAD    , do_poofset  , LVL_IMMORT, SCMD_POOFOUT },
    // { "pour"     , POS_STANDING, do_pour     , 0, SCMD_POUR },
    // { "pout"     , POS_RESTING , do_action   , 0, 0 },
    // { "prompt"   , POS_DEAD    , do_display  , 0, 0 },
    // { "practice" , POS_RESTING , do_practice , 1, 0 },
    // { "pray"     , POS_SITTING , do_action   , 0, 0 },
    // { "puke"     , POS_RESTING , do_action   , 0, 0 },
    // { "punch"    , POS_RESTING , do_action   , 0, 0 },
    // { "purr"     , POS_RESTING , do_action   , 0, 0 },
    // { "purge"    , POS_DEAD    , do_purge    , LVL_GOD, 0 },
    //
    // { "quaff"    , POS_RESTING , do_use      , 0, SCMD_QUAFF },
    // { "qecho"    , POS_DEAD    , do_qcomm    , LVL_IMMORT, SCMD_QECHO },
    // { "quest"    , POS_DEAD    , do_gen_tog  , 0, SCMD_QUEST },
    // { "qui"      , POS_DEAD    , do_quit     , 0, 0 },
    // { "quit"     , POS_DEAD    , do_quit     , 0, SCMD_QUIT },
    CommandInfo {
        command: "quit",
        minimum_position: POS_DEAD,
        command_pointer: do_quit,
        minimum_level: 0,
        subcmd: SCMD_QUIT,
    },
    // { "qsay"     , POS_RESTING , do_qcomm    , 0, SCMD_QSAY },
    //
    // { "reply"    , POS_SLEEPING, do_reply    , 0, 0 },
    // { "rest"     , POS_RESTING , do_rest     , 0, 0 },
    // { "read"     , POS_RESTING , do_look     , 0, SCMD_READ },
    CommandInfo {
        command: "read",
        minimum_position: POS_RESTING,
        command_pointer: do_look,
        minimum_level: 0,
        subcmd: SCMD_READ,
    },
    // { "reload"   , POS_DEAD    , do_reboot   , LVL_IMPL, 0 },
    // { "recite"   , POS_RESTING , do_use      , 0, SCMD_RECITE },
    // { "receive"  , POS_STANDING, do_not_here , 1, 0 },
    // { "remove"   , POS_RESTING , do_remove   , 0, 0 },
    CommandInfo {
        command: "remove",
        minimum_position: POS_RESTING,
        command_pointer: do_remove,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "rent"     , POS_STANDING, do_not_here , 1, 0 },
    // { "report"   , POS_RESTING , do_report   , 0, 0 },
    // { "reroll"   , POS_DEAD    , do_wizutil  , LVL_GRGOD, SCMD_REROLL },
    // { "rescue"   , POS_FIGHTING, do_rescue   , 1, 0 },
    // { "restore"  , POS_DEAD    , do_restore  , LVL_GOD, 0 },
    // { "return"   , POS_DEAD    , do_return   , 0, 0 },
    // { "roll"     , POS_RESTING , do_action   , 0, 0 },
    // { "roomflags", POS_DEAD    , do_gen_tog  , LVL_IMMORT, SCMD_ROOMFLAGS },
    // { "ruffle"   , POS_STANDING, do_action   , 0, 0 },
    //
    // { "say"      , POS_RESTING , do_say      , 0, 0 },
    // { "'"        , POS_RESTING , do_say      , 0, 0 },
    // { "save"     , POS_SLEEPING, do_save     , 0, 0 },
    // { "score"    , POS_DEAD    , do_score    , 0, 0 },
    CommandInfo {
        command: "score",
        minimum_position: POS_DEAD,
        command_pointer: do_score,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "scream"   , POS_RESTING , do_action   , 0, 0 },
    // { "sell"     , POS_STANDING, do_not_here , 0, 0 },
    // { "send"     , POS_SLEEPING, do_send     , LVL_GOD, 0 },
    // { "set"      , POS_DEAD    , do_set      , LVL_GOD, 0 },
    // { "shout"    , POS_RESTING , do_gen_comm , 0, SCMD_SHOUT },
    // { "shake"    , POS_RESTING , do_action   , 0, 0 },
    // { "shiver"   , POS_RESTING , do_action   , 0, 0 },
    // { "show"     , POS_DEAD    , do_show     , LVL_IMMORT, 0 },
    // { "shrug"    , POS_RESTING , do_action   , 0, 0 },
    // { "shutdow"  , POS_DEAD    , do_shutdown , LVL_IMPL, 0 },
    // { "shutdown" , POS_DEAD    , do_shutdown , LVL_IMPL, SCMD_SHUTDOWN },
    // { "sigh"     , POS_RESTING , do_action   , 0, 0 },
    // { "sing"     , POS_RESTING , do_action   , 0, 0 },
    // { "sip"      , POS_RESTING , do_drink    , 0, SCMD_SIP },
    // { "sit"      , POS_RESTING , do_sit      , 0, 0 },
    // { "skillset" , POS_SLEEPING, do_skillset , LVL_GRGOD, 0 },
    // { "sleep"    , POS_SLEEPING, do_sleep    , 0, 0 },
    // { "slap"     , POS_RESTING , do_action   , 0, 0 },
    // { "slowns"   , POS_DEAD    , do_gen_tog  , LVL_IMPL, SCMD_SLOWNS },
    // { "smile"    , POS_RESTING , do_action   , 0, 0 },
    // { "smirk"    , POS_RESTING , do_action   , 0, 0 },
    // { "snicker"  , POS_RESTING , do_action   , 0, 0 },
    // { "snap"     , POS_RESTING , do_action   , 0, 0 },
    // { "snarl"    , POS_RESTING , do_action   , 0, 0 },
    // { "sneeze"   , POS_RESTING , do_action   , 0, 0 },
    // { "sneak"    , POS_STANDING, do_sneak    , 1, 0 },
    // { "sniff"    , POS_RESTING , do_action   , 0, 0 },
    // { "snore"    , POS_SLEEPING, do_action   , 0, 0 },
    // { "snowball" , POS_STANDING, do_action   , LVL_IMMORT, 0 },
    // { "snoop"    , POS_DEAD    , do_snoop    , LVL_GOD, 0 },
    // { "snuggle"  , POS_RESTING , do_action   , 0, 0 },
    // { "socials"  , POS_DEAD    , do_commands , 0, SCMD_SOCIALS },
    CommandInfo {
        command: "socials",
        minimum_position: POS_DEAD,
        command_pointer: do_commands,
        minimum_level: 0,
        subcmd: SCMD_SOCIALS,
    },
    // { "split"    , POS_SITTING , do_split    , 1, 0 },
    // { "spank"    , POS_RESTING , do_action   , 0, 0 },
    // { "spit"     , POS_STANDING, do_action   , 0, 0 },
    // { "squeeze"  , POS_RESTING , do_action   , 0, 0 },
    // { "stand"    , POS_RESTING , do_stand    , 0, 0 },
    // { "stare"    , POS_RESTING , do_action   , 0, 0 },
    // { "stat"     , POS_DEAD    , do_stat     , LVL_IMMORT, 0 },
    // { "steal"    , POS_STANDING, do_steal    , 1, 0 },
    // { "steam"    , POS_RESTING , do_action   , 0, 0 },
    // { "stroke"   , POS_RESTING , do_action   , 0, 0 },
    // { "strut"    , POS_STANDING, do_action   , 0, 0 },
    // { "sulk"     , POS_RESTING , do_action   , 0, 0 },
    // { "switch"   , POS_DEAD    , do_switch   , LVL_GRGOD, 0 },
    // { "syslog"   , POS_DEAD    , do_syslog   , LVL_IMMORT, 0 },
    //
    // { "tell"     , POS_DEAD    , do_tell     , 0, 0 },
    // { "tackle"   , POS_RESTING , do_action   , 0, 0 },
    // { "take"     , POS_RESTING , do_get      , 0, 0 },
    CommandInfo {
        command: "take",
        minimum_position: POS_RESTING,
        command_pointer: do_get,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "tango"    , POS_STANDING, do_action   , 0, 0 },
    // { "taunt"    , POS_RESTING , do_action   , 0, 0 },
    // { "taste"    , POS_RESTING , do_eat      , 0, SCMD_TASTE },
    // { "teleport" , POS_DEAD    , do_teleport , LVL_GOD, 0 },
    // { "thank"    , POS_RESTING , do_action   , 0, 0 },
    // { "think"    , POS_RESTING , do_action   , 0, 0 },
    // { "thaw"     , POS_DEAD    , do_wizutil  , LVL_FREEZE, SCMD_THAW },
    // { "title"    , POS_DEAD    , do_title    , 0, 0 },
    // { "tickle"   , POS_RESTING , do_action   , 0, 0 },
    // { "time"     , POS_DEAD    , do_time     , 0, 0 },
    CommandInfo {
        command: "time",
        minimum_position: POS_DEAD,
        command_pointer: do_time,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "toggle"   , POS_DEAD    , do_toggle   , 0, 0 },
    // { "track"    , POS_STANDING, do_track    , 0, 0 },
    // { "trackthru", POS_DEAD    , do_gen_tog  , LVL_IMPL, SCMD_TRACK },
    // { "transfer" , POS_SLEEPING, do_trans    , LVL_GOD, 0 },
    // { "twiddle"  , POS_RESTING , do_action   , 0, 0 },
    // { "typo"     , POS_DEAD    , do_gen_write, 0, SCMD_TYPO },
    //
    // { "unlock"   , POS_SITTING , do_gen_door , 0, SCMD_UNLOCK },
    // { "ungroup"  , POS_DEAD    , do_ungroup  , 0, 0 },
    // { "unban"    , POS_DEAD    , do_unban    , LVL_GRGOD, 0 },
    // { "unaffect" , POS_DEAD    , do_wizutil  , LVL_GOD, SCMD_UNAFFECT },
    // { "uptime"   , POS_DEAD    , do_date     , LVL_IMMORT, SCMD_UPTIME },
    // { "use"      , POS_SITTING , do_use      , 1, SCMD_USE },
    // { "users"    , POS_DEAD    , do_users    , LVL_IMMORT, 0 },
    //
    // { "value"    , POS_STANDING, do_not_here , 0, 0 },
    // { "version"  , POS_DEAD    , do_gen_ps   , 0, SCMD_VERSION },
    // { "visible"  , POS_RESTING , do_visible  , 1, 0 },
    // { "vnum"     , POS_DEAD    , do_vnum     , LVL_IMMORT, 0 },
    // { "vstat"    , POS_DEAD    , do_vstat    , LVL_IMMORT, 0 },
    //
    // { "wake"     , POS_SLEEPING, do_wake     , 0, 0 },
    // { "wave"     , POS_RESTING , do_action   , 0, 0 },
    // { "wear"     , POS_RESTING , do_wear     , 0, 0 },
    CommandInfo {
        command: "wear",
        minimum_position: POS_RESTING,
        command_pointer: do_wear,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "weather"  , POS_RESTING , do_weather  , 0, 0 },
    CommandInfo {
        command: "weather",
        minimum_position: POS_RESTING,
        command_pointer: do_weather,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "who"      , POS_DEAD    , do_who      , 0, 0 },
    // { "whoami"   , POS_DEAD    , do_gen_ps   , 0, SCMD_WHOAMI },
    // { "where"    , POS_RESTING , do_where    , 1, 0 },
    // { "whisper"  , POS_RESTING , do_spec_comm, 0, SCMD_WHISPER },
    // { "whine"    , POS_RESTING , do_action   , 0, 0 },
    // { "whistle"  , POS_RESTING , do_action   , 0, 0 },
    // { "wield"    , POS_RESTING , do_wield    , 0, 0 },
    CommandInfo {
        command: "wield",
        minimum_position: POS_RESTING,
        command_pointer: do_wield,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "wiggle"   , POS_STANDING, do_action   , 0, 0 },
    // { "wimpy"    , POS_DEAD    , do_wimpy    , 0, 0 },
    // { "wink"     , POS_RESTING , do_action   , 0, 0 },
    // { "withdraw" , POS_STANDING, do_not_here , 1, 0 },
    // { "wiznet"   , POS_DEAD    , do_wiznet   , LVL_IMMORT, 0 },
    // { ";"        , POS_DEAD    , do_wiznet   , LVL_IMMORT, 0 },
    // { "wizhelp"  , POS_SLEEPING, do_commands , LVL_IMMORT, SCMD_WIZHELP },
    CommandInfo {
        command: "wizhelp",
        minimum_position: POS_SLEEPING,
        command_pointer: do_commands,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_WIZHELP,
    },
    // { "wizlist"  , POS_DEAD    , do_gen_ps   , 0, SCMD_WIZLIST },
    // { "wizlock"  , POS_DEAD    , do_wizlock  , LVL_IMPL, 0 },
    // { "worship"  , POS_RESTING , do_action   , 0, 0 },
    // { "write"    , POS_STANDING, do_write    , 1, 0 },
    //
    // { "yawn"     , POS_RESTING , do_action   , 0, 0 },
    // { "yodel"    , POS_RESTING , do_action   , 0, 0 },
    //
    // { "zreset"   , POS_DEAD    , do_zreset   , LVL_GRGOD, 0 },
    //
    CommandInfo {
        command: "\n",
        minimum_position: 0,
        command_pointer: do_nothing,
        minimum_level: 0,
        subcmd: 0,
    }, /* this must be last */
];

const FILL: [&str; 8] = ["in", "from", "with", "the", "on", "at", "to", "\n"];

const RESERVED: [&str; 9] = [
    "a",
    "an",
    "self",
    "me",
    "all",
    "room",
    "someone",
    "something",
    "\n",
];

/*
 * This is the actual command interpreter called from game_loop() in comm.c
 * It makes sure you are the proper level and position to execute the command,
 * then calls the appropriate function.
 */
pub fn command_interpreter(game: &MainGlobals, ch: &Rc<CharData>, argument: &str) {
    let line: &str;
    let mut arg = String::new();

    ch.remove_aff_flags(AFF_HIDE);

    /* just drop to next line for hitting CR */
    let argument = argument.trim_start();

    if argument.len() == 0 {
        return;
    }
    /*
     * special case to handle one-character, non-alphanumeric commands;
     * requested by many people so "'hi" or ";godnet test" is possible.
     * Patch sent by Eric Green and Stefan Wasilewski.
     */
    if !argument.chars().next().unwrap().is_alphanumeric() {
        arg = argument.chars().next().unwrap().to_string();
        line = &argument[1..];
    } else {
        line = any_one_arg(argument, &mut arg);
    }

    /* otherwise, find the command */
    let mut cmd_idx = CMD_INFO.len() - 1;
    let mut cmd = &CMD_INFO[cmd_idx];
    for (i, cmd_info) in CMD_INFO.iter().enumerate() {
        if cmd_info.command == arg {
            if ch.get_level() >= cmd_info.minimum_level as u8 {
                cmd = &cmd_info;
                cmd_idx = i;
                break;
            }
        }
    }

    if cmd.command == "\n" {
        send_to_char(ch, "Huh?!?\r\n");
    } else if !ch.is_npc() && ch.plr_flagged(PLR_FROZEN) && ch.get_level() < LVL_IMPL as u8 {
        send_to_char(ch, "You try, but the mind-numbing cold prevents you...\r\n");
        // } else if cmd.command_pointer == do_nothing {
        //     send_to_char(ch, "Sorry, that command hasn't been implemented yet.\r\n");
    } else if ch.is_npc() && cmd.minimum_level >= LVL_IMMORT {
        send_to_char(ch, "You can't use immortal commands while switched.\r\n");
    } else if ch.get_pos() < cmd.minimum_position {
        match ch.get_pos() {
            POS_DEAD => {
                send_to_char(ch, "Lie still; you are DEAD!!! :-(\r\n");
            }
            POS_INCAP | POS_MORTALLYW => {
                send_to_char(
                    ch,
                    "You are in a pretty bad shape, unable to do anything!\r\n",
                );
            }
            POS_STUNNED => {
                send_to_char(ch, "All you can do right now is think about the stars!\r\n");
            }
            POS_SLEEPING => {
                send_to_char(ch, "In your dreams, or what?\r\n");
            }
            POS_RESTING => {
                send_to_char(ch, "Nah... You feel too relaxed to do that..\r\n");
            }
            POS_SITTING => {
                send_to_char(ch, "Maybe you should get on your feet first?\r\n");
            }
            POS_FIGHTING => {
                send_to_char(ch, "No way!  You're fighting for your life!\r\n");
            }
            _ => {}
        }
    } else if game.db.no_specials || !special(game, ch, cmd_idx as i32, line) {
        (cmd.command_pointer)(game, ch, line, cmd_idx, cmd.subcmd);
    }
}

/**************************************************************************
 * Routines to handle aliasing                                             *
 **************************************************************************/

//
// struct alias_data *find_alias(struct alias_data *alias_list, char *str)
// {
// while (alias_list != NULL) {
// if (*str == *alias_list->alias)	/* hey, every little bit counts :-) */
// if (!strcmp(str, alias_list->alias))
// return (alias_list);
//
// alias_list = alias_list->next;
// }
//
// return (NULL);
// }

// void free_alias(struct alias_data *a)
// {
// if (a->alias)
// free(a->alias);
// if (a->replacement)
// free(a->replacement);
// free(a);
// }

/* The interface to the outside world: do_alias */
// ACMD(do_alias)
// {
// char arg[MAX_INPUT_LENGTH];
// char *repl;
// struct alias_data *a, *temp;
//
// if (IS_NPC(ch))
// return;
//
// repl = any_one_arg(argument, arg);
//
// if (!*arg) {			/* no argument specified -- list currently defined aliases */
// send_to_char(ch, "Currently defined aliases:\r\n");
// if ((a = GET_ALIASES(ch)) == NULL)
// send_to_char(ch, " None.\r\n");
// else {
// while (a != NULL) {
// send_to_char(ch, "%-15s %s\r\n", a->alias, a->replacement);
// a = a->next;
// }
// }
// } else {			/* otherwise, add or remove aliases */
// /* is this an alias we've already defined? */
// if ((a = find_alias(GET_ALIASES(ch), arg)) != NULL) {
// REMOVE_FROM_LIST(a, GET_ALIASES(ch), next);
// free_alias(a);
// }
// /* if no replacement string is specified, assume we want to delete */
// if (!*repl) {
// if (a == NULL)
// send_to_char(ch, "No such alias.\r\n");
// else
// send_to_char(ch, "Alias deleted.\r\n");
// } else {			/* otherwise, either add or redefine an alias */
// if (!str_cmp(arg, "alias")) {
// send_to_char(ch, "You can't alias 'alias'.\r\n");
// return;
// }
// CREATE(a, struct alias_data, 1);
// a->alias = strdup(arg);
// delete_doubledollar(repl);
// a->replacement = strdup(repl);
// if (strchr(repl, ALIAS_SEP_CHAR) || strchr(repl, ALIAS_VAR_CHAR))
// a->type = ALIAS_COMPLEX;
// else
// a->type = ALIAS_SIMPLE;
// a->next = GET_ALIASES(ch);
// GET_ALIASES(ch) = a;
// send_to_char(ch, "Alias added.\r\n");
// }
// }
// }

/*
 * Valid numeric replacements are only $1 .. $9 (makes parsing a little
 * easier, and it's not that much of a limitation anyway.)  Also valid
 * is "$*", which stands for the entire original line after the alias.
 * ";" is used to delimit commands.
 */
// #define NUM_TOKENS       9
//
// void perform_complex_alias(struct txt_q *input_q, char *orig, struct alias_data *a)
// {
// struct txt_q temp_queue;
// char *tokens[NUM_TOKENS], *temp, *write_point;
// char buf2[MAX_RAW_INPUT_LENGTH], buf[MAX_RAW_INPUT_LENGTH];	/* raw? */
// int num_of_tokens = 0, num;
//
// /* First, parse the original string */
// strcpy(buf2, orig);	/* strcpy: OK (orig:MAX_INPUT_LENGTH < buf2:MAX_RAW_INPUT_LENGTH) */
// temp = strtok(buf2, " ");
// while (temp != NULL && num_of_tokens < NUM_TOKENS) {
// tokens[num_of_tokens++] = temp;
// temp = strtok(NULL, " ");
// }
//
// /* initialize */
// write_point = buf;
// temp_queue.head = temp_queue.tail = NULL;
//
// /* now parse the alias */
// for (temp = a->replacement; *temp; temp++) {
// if (*temp == ALIAS_SEP_CHAR) {
// *write_point = '\0';
// buf[MAX_INPUT_LENGTH - 1] = '\0';
// write_to_q(buf, &temp_queue, 1);
// write_point = buf;
// } else if (*temp == ALIAS_VAR_CHAR) {
// temp++;
// if ((num = *temp - '1') < num_of_tokens && num >= 0) {
// strcpy(write_point, tokens[num]);	/* strcpy: OK */
// write_point += strlen(tokens[num]);
// } else if (*temp == ALIAS_GLOB_CHAR) {
// strcpy(write_point, orig);		/* strcpy: OK */
// write_point += strlen(orig);
// } else if ((*(write_point++) = *temp) == '$')	/* redouble $ for act safety */
// *(write_point++) = '$';
// } else
// *(write_point++) = *temp;
// }
//
// *write_point = '\0';
// buf[MAX_INPUT_LENGTH - 1] = '\0';
// write_to_q(buf, &temp_queue, 1);
//
// /* push our temp_queue on to the _front_ of the input queue */
// if (input_q->head == NULL)
// *input_q = temp_queue;
// else {
// temp_queue.tail->next = input_q->head;
// input_q->head = temp_queue.head;
// }
// }

/*
 * Given a character and a string, perform alias replacement on it.
 *
 * Return values:
 *   0: String was modified in place; call command_interpreter immediately.
 *   1: String was _not_ modified in place; rather, the expanded aliases
 *      have been placed at the front of the character's input queue.
 */
// int perform_alias(struct descriptor_data *d, char *orig, size_t maxlen)
// {
// char first_arg[MAX_INPUT_LENGTH], *ptr;
// struct alias_data *a, *tmp;
//
// /* Mobs don't have alaises. */
// if (IS_NPC(d->character))
// return (0);
//
// /* bail out immediately if the guy doesn't have any aliases */
// if ((tmp = GET_ALIASES(d->character)) == NULL)
// return (0);
//
// /* find the alias we're supposed to match */
// ptr = any_one_arg(orig, first_arg);
//
// /* bail out if it's null */
// if (!*first_arg)
// return (0);
//
// /* if the first arg is not an alias, return without doing anything */
// if ((a = find_alias(tmp, first_arg)) == NULL)
// return (0);
//
// if (a->type == ALIAS_SIMPLE) {
// strlcpy(orig, a->replacement, maxlen);
// return (0);
// } else {
// perform_complex_alias(&d->input, ptr, a);
// return (1);
// }
// }

/***************************************************************************
 * Various other parsing utilities                                         *
 **************************************************************************/

/*
 * searches an array of strings for a target string.  "exact" can be
 * 0 or non-0, depending on whether or not the match must be exact for
 * it to be returned.  Returns -1 if not found; 0..n otherwise.  Array
 * must be terminated with a '\n' so it knows to stop searching.
 */
pub fn search_block(arg: &str, list: &[&str], exact: bool) -> Option<usize> {
    /*  We used to have \r as the first character on certain array items to
     *  prevent the explicit choice of that point.  It seems a bit silly to
     *  dump control characters into arrays to prevent that, so we'll just
     *  check in here to see if the first character of the argument is '!',
     *  and if so, just blindly return a '-1' for not found. - ae.
     */
    if arg.starts_with("!") {
        return None;
    }

    /* Make into lower case, and get length of string */
    let arg = arg.to_lowercase();
    let arg = arg.as_str();

    return if exact {
        let i = list.iter().position(|s| *s == arg);
        i
    } else {
        let i = list.iter().position(|s| (*s).starts_with(arg));
        i
    };
}

pub fn is_number(txt: &str) -> bool {
    return txt.parse::<i32>().is_ok();
}

/*
 * Function to skip over the leading spaces of a string.
 */
// fn skip_spaces(char **string)
// {
// for (; **string && isspace(**string); (*string)++);
// }

/*
 * Given a string, change all instances of double dollar signs ($$) to
 * single dollar signs ($).  When strings come in, all $'s are changed
 * to $$'s to avoid having users be able to crash the system if the
 * inputted string is eventually sent to act().  If you are using user
 * input to produce screen output AND YOU ARE SURE IT WILL NOT BE SENT
 * THROUGH THE act() FUNCTION (i.e., do_gecho, do_title, but NOT do_say),
 * you can call delete_doubledollar() to make the output look correct.
 *
 * Modifies the string in-place.
 */
// char *delete_doubledollar(char *string)
// {
// char *ddread, *ddwrite;
//
// /* If the string has no dollar signs, return immediately */
// if ((ddwrite = strchr(string, '$')) == NULL)
// return (string);
//
// /* Start from the location of the first dollar sign */
// ddread = ddwrite;
//
//
// while (*ddread)   /* Until we reach the end of the string... */
// if ((*(ddwrite++) = *(ddread++)) == '$') /* copy one char */
// if (*ddread == '$')
// ddread++; /* skip if we saw 2 $'s in a row */
//
// *ddwrite = '\0';
//
// return (string);
// }

fn fill_word(argument: &str) -> bool {
    return search_block(argument, &FILL, true).is_some();
}

fn reserved_word(argument: &str) -> bool {
    return search_block(argument, &RESERVED, true).is_some();
}

/*
 * copy the first non-FILL-word, space-delimited argument of 'argument'
 * to 'first_arg'; return a pointer to the remainder of the string.
 */
pub fn one_argument<'a>(argument: &'a str, first_arg: &mut String) -> &'a str {
    //char * begin = first_arg;
    // if (!argument) {
    // log("SYSERR: one_argument received a NULL pointer!");
    // *first_arg = '\0';
    // return (NULL);
    // }
    let mut ret;
    loop {
        let mut argument = argument.trim_start();
        first_arg.clear();

        let mut i = 0;
        for c in argument.chars() {
            if c.is_whitespace() {
                break;
            }
            first_arg.push(c.to_ascii_lowercase());
            i += 1;
        }

        ret = &argument[i..];
        if !fill_word(first_arg.as_str()) {
            break;
        }
    }

    ret
}

/*
 * one_word is like one_argument, except that words in quotes ("") are
 * considered one word.
 */
// char *one_word(char *argument, char *first_arg)
// {
// char *begin = first_arg;
//
// do {
// skip_spaces(&argument);
//
// first_arg = begin;
//
// if (*argument == '\"') {
// argument++;
// while (*argument && *argument != '\"') {
// *(first_arg++) = LOWER(*argument);
// argument++;
// }
// argument++;
// } else {
// while (*argument && !isspace(*argument)) {
// *(first_arg++) = LOWER(*argument);
// argument++;
// }
// }
//
// *first_arg = '\0';
// } while (fill_word(begin));
//
// return (argument);
// }

/* same as one_argument except that it doesn't ignore FILL words */
pub fn any_one_arg<'a, 'b>(argument: &'a str, first_arg: &'b mut String) -> &'a str {
    let mut argument = argument.trim_start();

    for c in argument.chars().into_iter() {
        if c.is_ascii_whitespace() {
            break;
        }
        first_arg.push(c);
        argument = &argument[1..];
    }

    return argument;
}

/*
 * Same as one_argument except that it takes two args and returns the rest;
 * ignores FILL words
 */
pub fn two_arguments<'a>(
    argument: &'a str,
    first_arg: &mut String,
    second_arg: &mut String,
) -> &'a str {
    return one_argument(one_argument(argument, first_arg), second_arg); /* :-) */
}

/*
 * determine if a given string is an abbreviation of another
 * (now works symmetrically -- JE 7/25/94)
 *
 * that was dumb.  it shouldn't be symmetrical.  JE 5/1/95
 *
 * returns 1 if arg1 is an abbreviation of arg2
 */
pub fn is_abbrev(arg1: &str, arg2: &str) -> bool {
    if arg1.is_empty() {
        return false;
    }

    arg2.to_lowercase()
        .starts_with(arg1.to_lowercase().as_str())
}

/*
 * Return first space-delimited token in arg1; remainder of string in arg2.
 *
 * NOTE: Requires sizeof(arg2) >= sizeof(string)
 */
pub fn half_chop(string: &mut String, arg1: &mut String, arg2: &mut String) {
    let temp = any_one_arg(string, arg1);
    arg2.push_str(temp.trim_start());
}

/* Used in specprocs, mostly.  (Exactly) matches "command" to cmd number */
// int find_command(const char *command)
// {
// int cmd;
//
// for (cmd = 0; *CMD_INFO[cmd].command != '\n'; cmd++)
// if (!strcmp(CMD_INFO[cmd].command, command))
// return (cmd);
//
// return (-1);
// }

fn special(game: &MainGlobals, ch: &Rc<CharData>, cmd: i32, arg: &str) -> bool {
    // struct obj_data *i;
    // struct char_data *k;
    // int j;
    let db = &game.db;

    /* special in room? */
    if db.get_room_spec(ch.in_room()).is_some() {
        if db.get_room_spec(ch.in_room()).unwrap()(
            game,
            ch,
            &game.db.world.borrow()[ch.in_room() as usize],
            cmd,
            arg,
        ) {
            return true;
        }
    }

    // TODO implement special in objects
    // /* special in equipment list? */
    // for (j = 0; j < NUM_WEARS; j++)
    // if (GET_EQ(ch, j) && GET_OBJ_SPEC(GET_EQ(ch, j)) != NULL)
    // if (GET_OBJ_SPEC(GET_EQ(ch, j)) (ch, GET_EQ(ch, j), cmd, arg))
    // return (1);

    //     // TODO implement special in inventory
    // /* special in inventory? */
    // for (i = ch->carrying; i; i = i->next_content)
    // if (GET_OBJ_SPEC(i) != NULL)
    // if (GET_OBJ_SPEC(i) (ch, i, cmd, arg))
    // return (1);

    // TODO implement special on mobile
    /* special in mobile present? */
    for k in db.world.borrow()[ch.in_room() as usize]
        .peoples
        .borrow()
        .iter()
    {
        if !k.mob_flagged(MOB_NOTDEADYET) {
            if db.get_mob_spec(k).is_some()
                && db.get_mob_spec(k).as_ref().unwrap()(game, ch, k, cmd, arg)
            {
                return true;
            }
        }
    }

    // /* special in object present? */
    // for (i = world[IN_ROOM(ch)].contents; i; i = i->next_content)
    // if (GET_OBJ_SPEC(i) != NULL)
    // if (GET_OBJ_SPEC(i) (ch, i, cmd, arg))
    // return (1);

    false
}

/* *************************************************************************
*  Stuff for controlling the non-playing sockets (get name, pwd etc)       *
************************************************************************* */

// /* This function needs to die. */
fn _parse_name(arg: &str) -> Option<&str> {
    let arg = arg.trim();

    if arg.is_empty() {
        return None;
    }

    for c in arg.chars() {
        if !c.is_alphanumeric() {
            return None;
        }
    }

    return Some(arg);
}

pub const RECON: u8 = 1;
pub const USURP: u8 = 2;
pub const UNSWITCH: u8 = 3;

/* This function seems a bit over-extended. */
fn perform_dupe_check(main_globals: &MainGlobals, d: Rc<DescriptorData>) -> bool {
    let mut target: Option<Rc<CharData>> = None;
    let mut mode = 0;
    let id: i64;
    let db = &main_globals.db;

    id = d.character.borrow().as_ref().unwrap().get_idnum();

    /*
     * Now that this descriptor has successfully logged in, disconnect all
     * other descriptors controlling a character with the same ID number.
     */
    for k in main_globals.descriptor_list.borrow().iter() {
        if Rc::ptr_eq(k, &d) {
            continue;
        }

        if k.original.borrow().is_some() && k.original.borrow().as_ref().unwrap().get_idnum() == id
        {
            /* Original descriptor was switched, booting it and restoring normal body control. */
            write_to_output(
                d.as_ref(),
                "\r\nMultiple login detected -- disconnecting.\r\n",
            );
            k.set_state(ConClose);
            if target.is_none() {
                target = k.original.borrow().clone();
                mode = UNSWITCH;
            }

            if k.character.borrow().is_some() {
                *k.character.borrow_mut().as_mut().unwrap().desc.borrow_mut() = None;
            }
            *k.character.borrow_mut() = None;
            *k.original.borrow_mut() = None;
        } else if k.character.borrow().is_some()
            && k.character.borrow().as_ref().unwrap().get_idnum() == id
            && k.original.borrow().is_some()
        {
            /* Character taking over their own body, while an immortal was switched to it. */
            // TODO implement do_return
            //do_return(k.character, NULL, 0, 0);
        } else if k.character.borrow().is_some()
            && k.character.borrow().as_ref().unwrap().get_idnum() == id
        {
            /* Character taking over their own body. */

            if target.is_none() && k.state() == ConPlaying {
                write_to_output(k.as_ref(), "\r\nThis body has been usurped!\r\n");
                //target = Some(Rc::new(RefCell::new(k.character.as_ref().unwrap())));
                mode = USURP;
            }
            *k.character.borrow().as_ref().unwrap().desc.borrow_mut() = None;
            *k.character.borrow_mut() = None;
            *k.original.borrow_mut() = None;
            write_to_output(
                k.as_ref(),
                "\r\nMultiple login detected -- disconnecting.\r\n",
            );
            k.set_state(ConClose);
        }
    }

    /*
     * now, go through the character list, deleting all characters that
     * are not already marked for deletion from the above step (i.e., in the
     * CON_HANGUP state), and have not already been selected as a target for
     * switching into.  In addition, if we haven't already found a target,
     * choose one if one is available (while still deleting the other
     * duplicates, though theoretically none should be able to exist).
     */

    for ch in db.character_list.borrow().iter() {
        if ch.is_npc() {
            continue;
        }
        if ch.get_idnum() != id {
            continue;
        }

        /* ignore chars with descriptors (already handled by above step) */
        if ch.desc.borrow().is_some() {
            continue;
        }

        /* don't extract the target char we've found one already */
        if target.is_some() && Rc::ptr_eq(ch, target.as_ref().unwrap()) {
            continue;
        }

        /* we don't already have a target and found a candidate for switching */
        if target.is_none() {
            target = Some(ch.clone());
            mode = RECON;
            continue;
        }

        /* we've found a duplicate - blow him away, dumping his eq in limbo. */
        if ch.in_room != Cell::from(NOWHERE) {
            db.char_from_room(ch);
        }
        db.char_to_room(Some(ch), 1);
        db.extract_char(ch);
    }

    /* no target for switching into was found - allow login to continue */

    if target.is_none() {
        return false;
    }
    let target = target.unwrap();

    /* Okay, we've found a target.  Connect d to target. */
    //free_char(d->character); /* get rid of the old char */
    *d.character.borrow_mut() = Some(target);
    {
        let c = d.character.borrow();
        let character = c.as_ref().unwrap();
        *character.desc.borrow_mut() = Some(d.clone());
        *d.original.borrow_mut() = None;
        character.char_specials.borrow_mut().timer.set(0);
        character.remove_plr_flag(PLR_MAILING | PLR_WRITING);
        character.remove_aff_flags(AFF_GROUP);
    }
    d.set_state(ConPlaying);

    match mode {
        RECON => {
            write_to_output(d.as_ref(), "Reconnecting.\r\n");
            db.act(
                "$n has reconnected.",
                true,
                d.character.borrow().as_ref(),
                None,
                None,
                TO_ROOM,
            );
            main_globals.mudlog(
                NRM,
                max(
                    LVL_IMMORT as i32,
                    d.character.borrow().as_ref().unwrap().get_invis_lev() as i32,
                ),
                true,
                format!(
                    "{} [{}] has reconnected.",
                    d.character.borrow().as_ref().unwrap().get_name(),
                    d.host.borrow()
                )
                .as_str(),
            );
        }
        USURP => {
            write_to_output(
                d.as_ref(),
                "You take over your own body, already in use!\r\n",
            );
            db.act("$n suddenly keels over in pain, surrounded by a white aura...\r\n$n's body has been taken over by a new spirit!", true, d.character.borrow().as_ref(), None, None, TO_ROOM);
            main_globals.mudlog(
                NRM,
                max(
                    LVL_IMMORT as i32,
                    d.character.borrow().as_ref().unwrap().get_invis_lev() as i32,
                ),
                true,
                format!(
                    "{} has re-logged in ... disconnecting old socket.",
                    d.character.borrow().as_ref().unwrap().get_name()
                )
                .as_str(),
            );
        }
        UNSWITCH => {
            write_to_output(d.as_ref(), "Reconnecting to unswitched char.");
            main_globals.mudlog(
                NRM,
                max(
                    LVL_IMMORT as i32,
                    d.character.borrow().as_ref().unwrap().get_invis_lev() as i32,
                ),
                true,
                format!(
                    "{} [{}] has reconnected.",
                    d.character.borrow().as_ref().unwrap().get_name(),
                    d.host.borrow()
                )
                .as_str(),
            );
        }
        _ => {}
    }
    return true;
}

/* deal with newcomers and other non-playing sockets */
pub fn nanny(main_globals: &MainGlobals, d: Rc<DescriptorData>, arg: &str) {
    let arg = arg.trim();
    let db = &main_globals.db;

    match d.state() {
        ConGetName => {
            /* wait for input of name */
            if d.character.borrow().is_none() {
                let mut ch = CharData::new();
                clear_char(&mut ch);
                *ch.desc.borrow_mut() = Some(d.clone());
                *d.character.borrow_mut() = Some(Rc::from(ch));
            }

            if arg.is_empty() {
                d.set_state(ConClose);
            } else {
                let tmp_name = _parse_name(arg);

                let desc_list = main_globals.descriptor_list.borrow();
                if tmp_name.is_none()
                    || tmp_name.unwrap().len() < 2
                    || tmp_name.unwrap().len() > MAX_NAME_LENGTH
                    || !valid_name(&desc_list, tmp_name.unwrap())
                    || fill_word(tmp_name.unwrap())
                    || reserved_word(tmp_name.unwrap())
                {
                    write_to_output(d.as_ref(), "Invalid name, please try another.\r\nName: ");
                    return;
                }
                let och = d.character.borrow();
                let character = och.as_ref().unwrap();
                let mut tmp_store = CharFileU::new();
                let db = &main_globals.db;
                let player_i = db.load_char(tmp_name.unwrap(), &mut tmp_store);
                if player_i.is_some() {
                    store_to_char(&tmp_store, character.as_ref());
                    character.set_pfilepos(player_i.unwrap() as i32);

                    if character.prf_flagged(PLR_DELETED) {
                        // TODO: support deleted players
                        // /* We get a false positive from the original deleted character. */
                        // free_char(d->character);
                        // /* Check for multiple creations... */
                        // if (!Valid_Name(tmp_name)) {
                        //     write_to_output(d, "Invalid name, please try another.\r\nName: ");
                        //     return;
                        // }
                        // CREATE(d->character, struct char_data, 1);
                        // clear_char(d->character);
                        // CREATE(d->character->player_specials, struct player_special_data, 1);
                        // d -> character -> desc = d;
                        // CREATE(d->character->player.name, char, strlen(tmp_name) + 1);
                        // strcpy(d->character->player.name, CAP(tmp_name));    /* strcpy: OK (size checked above) */
                        // GET_PFILEPOS(d->character) = player_i;
                        // write_to_output(&mut mut_d, format!("Did I get that right, {} (Y/N)? ", tmp_name.unwrap()).as_str());
                        // mut_d.state() = ConNameCnfrm;
                    } else {
                        /* undo it just in case they are set */
                        character.remove_plr_flag(PLR_WRITING | PLR_MAILING | PLR_CRYO);
                        character.remove_aff_flags(AFF_GROUP);
                        write_to_output(d.as_ref(), "Password: ");
                        echo_off(d.as_ref());
                        d.idle_tics.set(0);
                        d.set_state(ConPassword);
                    }
                } else {
                    /* player unknown -- make new character */

                    /* Check for multiple creations of a character. */
                    if !valid_name(
                        &main_globals.descriptor_list.borrow(),
                        tmp_name.unwrap().clone(),
                    ) {
                        write_to_output(d.as_ref(), "Invalid name, please try another.\r\nName: ");
                        return;
                    }
                    let och = d.character.borrow();
                    let character = och.as_ref().unwrap();
                    character.player.borrow_mut().name = String::from(tmp_name.unwrap());

                    write_to_output(
                        d.as_ref(),
                        format!("Did I get that right, {} (Y/N)? ", tmp_name.unwrap()).as_str(),
                    );
                    d.set_state(ConNameCnfrm);
                }
            }
        }
        ConNameCnfrm => {
            /* wait for conf. of new name    */
            if arg.to_uppercase().starts_with('Y') {
                // TODO: support banning
                // if (isbanned(d->host) >= BAN_NEW) {
                //     mudlog(NRM, LVL_GOD, true, "Request for new char %s denied from [%s] (siteban)", GET_PC_NAME(d->character), d->host);
                //     write_to_output(d, "Sorry, new characters are not allowed from your site!\r\n");
                //     STATE(d) = CON_CLOSE;
                //     return;
                // }
                // TODO: support restrict
                // if (circle_restrict) {
                //     write_to_output(d, "Sorry, new players can't be created at the moment.\r\n");
                //     mudlog(NRM, LVL_GOD, true, "Request for new char %s denied from [%s] (wizlock)", GET_PC_NAME(d->character), d->host);
                //     STATE(d) = CON_CLOSE;
                //     return;
                // }

                let msg = format!(
                    "New character.\r\nGive me a password for {}: ",
                    d.character.borrow().as_ref().unwrap().get_pc_name()
                );
                write_to_output(d.as_ref(), msg.as_str());
                echo_off(d.as_ref());
                d.set_state(ConNewpasswd);
            } else if arg.starts_with('n') || arg.starts_with('N') {
                write_to_output(d.as_ref(), "Okay, what IS it, then? ");
                //free_char(d->character);
                d.set_state(ConGetName);
            } else {
                write_to_output(d.as_ref(), "Please type Yes or No: ");
            }
        }
        ConPassword => {
            /* get pwd for known player      */
            /*
             * To really prevent duping correctly, the player's record should
             * be reloaded from disk at this point (after the password has been
             * typed).  However I'm afraid that trying to load a character over
             * an already loaded character is going to cause some problem down the
             * road that I can't see at the moment.  So to compensate, I'm going to
             * (1) add a 15 or 20-second time limit for entering a password, and (2)
             * re-add the code to cut off duplicates when a player quits.  JE 6 Feb 96
             */

            echo_on(d.as_ref()); /* turn echo back on */

            /* New echo_on() eats the return on telnet. Extra space better than none. */
            write_to_output(d.as_ref(), "\r\n");
            let load_result: i32;

            if arg.is_empty() {
                d.set_state(ConClose);
            } else {
                let matching_pwd: bool;
                {
                    let och = d.character.borrow();
                    let character = och.as_ref().unwrap();
                    let mut passwd2 = [0 as u8; 16];
                    let salt = character.get_pc_name();
                    let passwd = character.get_passwd();
                    pbkdf2::pbkdf2::<Hmac<Sha256>>(
                        arg.as_bytes(),
                        salt.as_bytes(),
                        4,
                        &mut passwd2,
                    )
                    .expect("Error while encrypting password");
                    matching_pwd = passwd == passwd2;

                    if !matching_pwd {
                        main_globals.mudlog(
                            BRF,
                            LVL_GOD as i32,
                            true,
                            format!("Bad PW: {} [{}]", character.get_name(), d.host.borrow())
                                .as_str(),
                        );

                        character.incr_bad_pws();
                        db.save_char(d.character.borrow().as_ref().unwrap());
                        d.bad_pws.set(d.bad_pws.get() + 1);
                        if d.bad_pws.get() >= MAX_BAD_PWS {
                            /* 3 strikes and you're out. */
                            write_to_output(d.as_ref(), "Wrong password... disconnecting.\r\n");
                            d.set_state(ConClose);
                        } else {
                            write_to_output(d.as_ref(), "Wrong password.\r\nPassword: ");
                            echo_off(d.as_ref());
                        }
                        return;
                    }

                    /* Password was correct. */
                    load_result = character.get_bad_pws() as i32;
                    character.reset_bad_pws();
                    d.bad_pws.set(0);
                    // TODO implement ban
                    // if (isbanned(d->host) == BAN_SELECT &&
                    //     !PLR_FLAGGED(d->character, PLR_SITEOK)) {
                    //     write_to_output(d, "Sorry, this char has not been cleared for login from your site!\r\n");
                    //     STATE(d) = CON_CLOSE;
                    //     mudlog(NRM, LVL_GOD, true, "Connection attempt for %s denied from %s", GET_NAME(d->character), d->host);
                    //     return;
                    // }
                    // TODO implement restrict
                    // if (GET_LEVEL(d->character) < circle_restrict) {
                    //     write_to_output(d, "The game is temporarily restricted.. try again later.\r\n");
                    //     STATE(d) = CON_CLOSE;
                    //     mudlog(NRM, LVL_GOD, true, "Request for login denied for %s [%s] (wizlock)", GET_NAME(d->character), d->host);
                    //     return;
                    // }
                }
                /* check and make sure no other copies of this player are logged in */
                if perform_dupe_check(&main_globals, d.clone()) {
                    return;
                }
                let och = d.character.borrow();
                let character = och.as_ref().unwrap();

                let level: u8;
                {
                    level = character.get_level();
                }
                if level >= LVL_IMMORT as u8 {
                    write_to_output(d.as_ref(), &db.imotd);
                } else {
                    write_to_output(d.as_ref(), &db.motd);
                }

                {
                    main_globals.mudlog(
                        BRF,
                        max(LVL_IMMORT as i32, character.get_invis_lev() as i32),
                        true,
                        format!(
                            "{} [{}] has connected.",
                            character.get_name(),
                            d.host.borrow()
                        )
                        .as_str(),
                    );
                }

                if load_result != 0 {
                    let color1: &str;
                    let color2: &str;
                    {
                        color1 = CCRED!(character, C_SPR);
                        color2 = CCNRM!(character, C_SPR);
                    }
                    write_to_output(
                        d.as_ref(),
                        format!("\r\n\r\n\007\007\007{}{} LOGIN FAILURE{} SINCE LAST SUCCESSFUL LOGIN.{}\r\n",
                                color1, load_result, if load_result > 1 { "S" } else { "" }, color2).as_str(),
                    );
                    character.get_bad_pws();
                }
                write_to_output(d.as_ref(), "\r\n*** PRESS RETURN: ");
                d.set_state(ConRmotd);
            }
        }
        ConNewpasswd | ConChpwdGetnew => {
            let och = d.character.borrow();
            let character = och.as_ref().unwrap();
            if arg.is_empty()
                || arg.len() > MAX_PWD_LENGTH
                || arg.len() < 3
                || arg == character.get_pc_name().as_ref()
            {
                write_to_output(&d, "\r\nIllegal password.\r\nPassword: ");
                return;
            }
            {
                let salt = character.get_pc_name().to_string();
                let mut tmp = [0; 16];
                pbkdf2::pbkdf2::<Hmac<Sha256>>(arg.as_bytes(), salt.as_bytes(), 4, &mut tmp)
                    .expect("Error while encrypting new password");
                character.set_passwd(tmp);
            }
            write_to_output(d.as_ref(), "\r\nPlease retype password: ");
            if d.state() == ConNewpasswd {
                d.set_state(ConCnfpasswd);
            } else {
                d.set_state(ConChpwdVrfy);
            }
        }
        ConCnfpasswd | ConChpwdVrfy => {
            let och = d.character.borrow();
            let character = och.as_ref().unwrap();
            let pwd_equals: bool;
            {
                let salt = character.get_pc_name();
                let passwd = character.get_passwd();
                let mut passwd2 = [0 as u8; 16];
                pbkdf2::pbkdf2::<Hmac<Sha256>>(arg.as_bytes(), salt.as_bytes(), 4, &mut passwd2)
                    .expect("Error while encrypting confirmation password");
                pwd_equals = passwd == passwd2;
            }
            if !pwd_equals {
                write_to_output(
                    d.as_ref(),
                    "\r\nPasswords don't match... start over.\r\nPassword: ",
                );
                if d.state() == ConCnfpasswd {
                    d.set_state(ConNewpasswd);
                } else {
                    d.set_state(ConChpwdGetnew);
                }
            }
            echo_on(d.as_ref());

            if d.state() == ConCnfpasswd {
                write_to_output(d.as_ref(), "\r\nWhat is your sex (M/F)? ");
                d.set_state(ConQsex);
            } else {
                write_to_output(d.as_ref(), format!("\r\nDone.\r\n{}", MENU).as_str());
                d.set_state(ConMenu);
            }
        }
        ConQsex => {
            let och = d.character.borrow();
            let character = och.as_ref().unwrap();
            /* query sex of new user         */
            match arg.chars().next().unwrap() {
                'm' | 'M' => {
                    character.player.borrow_mut().sex = SEX_MALE;
                }
                'f' | 'F' => {
                    character.player.borrow_mut().sex = SEX_FEMALE;
                }
                _ => {
                    write_to_output(d.as_ref(), "That is not a sex..\r\nWhat IS your sex? ");
                    return;
                }
            }

            write_to_output(d.as_ref(), format!("{}\r\nClass: ", CLASS_MENU).as_str());
            d.set_state(ConQclass);
        }
        ConQclass => {
            let och = d.character.borrow();
            let character = och.as_ref().unwrap();
            let load_result = parse_class(arg.chars().next().unwrap());
            if load_result == CLASS_UNDEFINED {
                write_to_output(&d, "\r\nThat's not a class.\r\nClass: ");
                return;
            } else {
                character.set_class(load_result);
            }

            {
                if character.get_pfilepos() < 0 {
                    character
                        .set_pfilepos(db.create_entry(character.get_pc_name().as_ref()) as i32);
                }

                /* Now GET_NAME() will work properly. */
                db.init_char(character.as_ref());
                db.save_char(character.as_ref());
            }
            write_to_output(
                d.as_ref(),
                format!("{}\r\n*** PRESS RETURN: ", db.motd).as_str(),
            );
            d.set_state(ConRmotd);

            {
                main_globals.mudlog(
                    NRM,
                    LVL_IMMORT as i32,
                    true,
                    format!(
                        "{} [{}] new player.",
                        character.get_pc_name(),
                        d.host.borrow()
                    )
                    .as_str(),
                );
            }
        }
        ConRmotd => {
            /* read CR after printing motd   */
            write_to_output(d.as_ref(), MENU);
            d.set_state(ConMenu);
        }
        ConMenu => {
            /* get selection from main menu  */
            // room_vnum
            // load_room;
            let och = d.character.borrow();
            let character = och.as_ref().unwrap();
            match if arg.chars().last().is_some() {
                arg.chars().last().unwrap()
            } else {
                '\0'
            } {
                '0' => {
                    write_to_output(d.as_ref(), "Goodbye.\r\n");
                    d.set_state(ConClose);
                }

                '1' => {
                    {
                        reset_char(character.as_ref());
                        // TODO implement aliases
                        //read_aliases(character);
                        if character.prf_flagged(PLR_INVSTART) {
                            character.set_invis_lev(character.get_level() as i16);
                        }

                        /*
                         * We have to place the character in a room before equipping them
                         * or equip_char() will gripe about the person in NOWHERE.
                         */
                        let mut load_room = character.get_loadroom();
                        if load_room != NOWHERE {
                            load_room = db.real_room(load_room);
                        }

                        /* If char was saved with NOWHERE, or real_room above failed... */
                        if load_room == NOWHERE {
                            if character.get_level() >= LVL_IMMORT as u8 {
                                load_room = *db.r_immort_start_room.borrow();
                            } else {
                                load_room = *db.r_mortal_start_room.borrow();
                            }
                        }

                        if character.plr_flagged(PLR_FROZEN) {
                            load_room = *db.r_frozen_start_room.borrow();
                        }

                        send_to_char(character.as_ref(), format!("{}", WELC_MESSG).as_str());
                        db.character_list.borrow_mut().push(character.clone());
                        db.char_to_room(Some(character), load_room);
                        //load_result = Crash_load(d->character);

                        /* Clear their load room if it's not persistant. */
                        if !character.plr_flagged(PLR_LOADROOM) {
                            character.set_loadroom(NOWHERE);
                        }
                        db.save_char(character.as_ref());

                        db.act(
                            "$n has entered the game.",
                            true,
                            Some(character),
                            None,
                            None,
                            TO_ROOM as i32,
                        );
                    }
                    d.set_state(ConPlaying);
                    if character.get_level() == 0 {
                        main_globals.do_start(character.as_ref());
                        send_to_char(character.as_ref(), format!("{}", START_MESSG).as_str());
                        main_globals.db.look_at_room(och.as_ref().unwrap(), false);
                    }
                    // if has_mail(GET_IDNUM(d->character)) {
                    //     send_to_char(d->character, "You have mail waiting.\r\n");
                    // }
                    // if load_result == 2 {
                    //     /* rented items lost */
                    //     send_to_char(d->character, "\r\n\007You could not afford your rent!\r\n"
                    //                  "Your possesions have been donated to the Salvation Army!\r\n");
                    // }
                    d.has_prompt.set(false);
                }

                '2' => {
                    if !character.player.borrow().description.is_empty() {
                        let player_description = character.player.borrow().description.clone();
                        write_to_output(
                            d.as_ref(),
                            format!("Old description:\r\n{}", player_description).as_str(),
                        );
                        character.player.borrow_mut().description.clear();
                    }
                    write_to_output(d.as_ref(), "Enter the new text you'd like others to see when they look at you.\r\nTerminate with a '@' on a new line.\r\n");
                    let description = character.player.borrow().description.clone();
                    *d.str.borrow_mut() = Some(description);
                    d.max_str.set(EXDSCR_LENGTH);
                    d.set_state(ConExdesc);
                }

                // '3' => {
                //     let db = RefCell::borrow(main_globals.db.as_ref().unwrap());
                //     page_string(mut_d, db.background, 0);
                //     mut_d.state() = ConRmotd;
                // }
                '4' => {
                    write_to_output(d.as_ref(), "\r\nEnter your old password: ");
                    echo_off(d.as_ref());
                    d.set_state(ConChpwdGetold);
                }

                '5' => {
                    write_to_output(d.as_ref(), "\r\nEnter your password for verification: ");
                    echo_off(d.as_ref());
                    d.set_state(ConDelcnf1);
                }

                _ => {
                    write_to_output(
                        d.as_ref(),
                        format!("\r\nThat's not a menu choice!\r\n{}", MENU).as_str(),
                    );
                }
            }
        }

        // ConChpwdGetold => {
        //     if (strncmp(CRYPT(arg, GET_PASSWD(d->character)), GET_PASSWD(d->character), MAX_PWD_LENGTH)) {
        //         echo_on(d);
        //         write_to_output(d, "\r\nIncorrect password.\r\n%s", MENU);
        //         STATE(d) = CON_MENU;
        //     } else {
        //         write_to_output(d, "\r\nEnter a new password: ");
        //         STATE(d) = CON_CHPWD_GETNEW;
        //     }
        //     return;
        // }
        //
        // ConDelcnf1 => {
        //     echo_on(d);
        //     if (strncmp(CRYPT(arg, GET_PASSWD(d->character)), GET_PASSWD(d->character), MAX_PWD_LENGTH)) {
        //         write_to_output(d, "\r\nIncorrect password.\r\n%s", MENU);
        //         STATE(d) = CON_MENU;
        //     } else {
        //         write_to_output(d, "\r\nYOU ARE ABOUT TO DELETE THIS CHARACTER PERMANENTLY.\r\n"
        //                         "ARE YOU ABSOLUTELY SURE?\r\n\r\n"
        //                         "Please type \"yes\" to confirm: ");
        //         STATE(d) = CON_DELCNF2;
        //     }
        // }
        //
        // ConDelcnf2 => {
        //     if (!strcmp(arg, "yes") || !strcmp(arg, "YES")) {
        //         if (PLR_FLAGGED(d -> character, PLR_FROZEN)) {
        //             write_to_output(d, "You try to kill yourself, but the ice stops you.\r\n"
        //                             "Character not deleted.\r\n\r\n");
        //             STATE(d) = CON_CLOSE;
        //             return;
        //         }
        //         if (GET_LEVEL(d -> character) < LVL_GRGOD) {
        //             SET_BIT(PLR_FLAGS(d -> character), PLR_DELETED);
        //         }
        //         save_char(d -> character);
        //         Crash_delete_file(GET_NAME(d -> character));
        //         delete_aliases(GET_NAME(d -> character));
        //         write_to_output(d, "Character '%s' deleted!\r\n"
        //                         "Goodbye.\r\n", GET_NAME(d -> character));
        //         mudlog(NRM, LVL_GOD, true, "%s (lev %d) has self-deleted.", GET_NAME(d -> character), GET_LEVEL(d -> character));
        //         STATE(d) = CON_CLOSE;
        //         return;
        //     } else {
        //         write_to_output(d, "\r\nCharacter not deleted.\r\n%s", MENU);
        //         STATE(d) = CON_MENU;
        //     }
        // }

        /*
         * It's possible, if enough pulses are missed, to kick someone off
         * while they are at the password prompt. We'll just defer to let
         * the game_loop() axe them.
         */
        ConClose => {}

        _ => {
            let char_name;
            if d.character.borrow().is_some() {
                let och = d.character.borrow();
                let character = och.as_ref().unwrap();
                char_name = String::from(character.get_name().as_ref());
            } else {
                char_name = "<unknown>".to_string();
            }
            error!(
                "SYSERR: Nanny: illegal state of con'ness ({:?}) for '{}'; closing connection.",
                d.state(),
                char_name
            );
            d.set_state(ConDisconnect); /* Safest to do. */
        }
    }
}
