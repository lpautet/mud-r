/* ************************************************************************
*   File: interpreter.rs                                Part of CircleMUD *
*  Usage: parse user commands, search for specials, call ACMD functions   *
*                                                                         *
*  All rights RESERVED.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use std::cmp::max;
use std::collections::LinkedList;
use std::rc::Rc;

use hmac::Hmac;
use log::error;
use sha2::Sha256;

use crate::act_comm::{
    do_gen_comm, do_gsay, do_page, do_qcomm, do_reply, do_say, do_spec_comm, do_tell, do_write,
};
use crate::act_informative::{
    do_color, do_commands, do_consider, do_diagnose, do_equipment, do_examine, do_exits, do_gen_ps,
    do_gold, do_help, do_inventory, do_levels, do_look, do_score, do_time, do_toggle, do_users,
    do_weather, do_where, do_who, look_at_room,
};
use crate::act_item::{
    do_drink, do_drop, do_eat, do_get, do_give, do_grab, do_pour, do_put, do_remove, do_wear,
    do_wield,
};
use crate::act_movement::{
    do_enter, do_follow, do_gen_door, do_leave, do_move, do_rest, do_sit, do_sleep, do_stand,
    do_wake,
};
use crate::act_offensive::{
    do_assist, do_backstab, do_bash, do_flee, do_hit, do_kick, do_kill, do_order, do_rescue,
};
use crate::act_other::{
    do_display, do_gen_tog, do_gen_write, do_group, do_hide, do_not_here, do_practice, do_quit,
    do_report, do_save, do_sneak, do_split, do_steal, do_title, do_ungroup, do_use, do_visible,
    do_wimpy,
};
use crate::act_social::{do_action, do_insult};
use crate::act_wizard::{
    do_advance, do_at, do_date, do_dc, do_echo, do_force, do_gecho, do_goto, do_invis, do_last,
    do_load, do_poofset, do_purge, do_restore, do_return, do_send, do_set, do_show, do_shutdown,
    do_snoop, do_stat, do_switch, do_syslog, do_teleport, do_trans, do_vnum, do_vstat, do_wizlock,
    do_wiznet, do_wizutil, do_zreset,
};
use crate::alias::{delete_aliases, read_aliases};
use crate::ban::{do_ban, do_unban, isbanned, valid_name};
use crate::class::{do_start, parse_class, CLASS_MENU};
use crate::config::{MAX_BAD_PWS, MENU, START_MESSG, WELC_MESSG};
use crate::db::{clear_char, do_reboot, reset_char, store_to_char, BanType};
use crate::depot::{Depot, DepotId, HasId};
use crate::graph::do_track;
use crate::house::{do_hcontrol, do_house};
use crate::modify::{do_skillset, page_string};
use crate::objsave::{crash_delete_file, crash_load};
use crate::screen::{C_SPR, KNRM, KNUL, KRED};
use crate::spell_parser::do_cast;
use crate::structs::ConState::{
    ConChpwdGetnew, ConChpwdGetold, ConChpwdVrfy, ConClose, ConCnfpasswd, ConDelcnf2,
    ConDisconnect, ConGetName, ConMenu, ConNameCnfrm, ConNewpasswd, ConPassword, ConQclass,
    ConQsex, ConRmotd,
};
use crate::structs::ConState::{ConDelcnf1, ConExdesc, ConPlaying};
use crate::structs::{
    CharData, MeRef, TxtBlock, AffectFlags, LVL_FREEZE, LVL_GOD, LVL_GRGOD, LVL_IMPL, MOB_NOTDEADYET,
    NOWHERE, NUM_WEARS, PLR_FROZEN, PLR_INVSTART, PLR_LOADROOM, PLR_SITEOK, POS_DEAD, POS_FIGHTING,
    POS_INCAP, POS_MORTALLYW, POS_RESTING, POS_SITTING, POS_SLEEPING, POS_STANDING, POS_STUNNED,
};
use crate::structs::{
    CharFileU, CLASS_UNDEFINED, EXDSCR_LENGTH, LVL_IMMORT, MAX_NAME_LENGTH,
    MAX_PWD_LENGTH, PLR_CRYO, PLR_MAILING, PLR_WRITING, PRF_COLOR_1, PRF_COLOR_2, SEX_FEMALE,
    SEX_MALE,
};
use crate::util::{BRF, NRM};
use crate::{_clrlevel, act, clr, save_char, send_to_char, write_to_q, Game, ObjData, TextData, CCNRM, CCRED, DB, PLR_DELETED, TO_ROOM};

/*
 * Alert! Changed from 'struct alias' to 'struct AliasData' in bpl15
 * because a Windows 95 compiler gives a warning about it having similiar
 * named member.
 */
#[derive(Debug, Clone)]
pub struct AliasData {
    pub alias: Rc<str>,
    pub replacement: Rc<str>,
    pub type_: i32,
}

pub const ALIAS_SIMPLE: i32 = 0;
pub const ALIAS_COMPLEX: i32 = 1;

pub const ALIAS_SEP_CHAR: char = ';';
pub const ALIAS_VAR_CHAR: char = '$';
pub const ALIAS_GLOB_CHAR: char = '*';

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

/* do_gen_ps */
pub const SCMD_INFO: i32 = 0;
pub const SCMD_HANDBOOK: i32 = 1;
pub const SCMD_CREDITS: i32 = 2;
pub const SCMD_NEWS: i32 = 3;
pub const SCMD_WIZLIST: i32 = 4;
pub const SCMD_POLICIES: i32 = 5;
pub const SCMD_VERSION: i32 = 6;
pub const SCMD_IMMLIST: i32 = 7;
pub const SCMD_MOTD: i32 = 8;
pub const SCMD_IMOTD: i32 = 9;
pub const SCMD_CLEAR: i32 = 10;
pub const SCMD_WHOAMI: i32 = 11;

/* do_gen_tog */
pub const SCMD_NOSUMMON: i32 = 0;
pub const SCMD_NOHASSLE: i32 = 1;
pub const SCMD_BRIEF: i32 = 2;
pub const SCMD_COMPACT: i32 = 3;
pub const SCMD_NOTELL: i32 = 4;
pub const SCMD_NOAUCTION: i32 = 5;
pub const SCMD_DEAF: i32 = 6;
pub const SCMD_NOGOSSIP: i32 = 7;
pub const SCMD_NOGRATZ: i32 = 8;
pub const SCMD_NOWIZ: i32 = 9;
pub const SCMD_QUEST: i32 = 10;
pub const SCMD_ROOMFLAGS: i32 = 11;
pub const SCMD_NOREPEAT: i32 = 12;
pub const SCMD_HOLYLIGHT: i32 = 13;
pub const SCMD_SLOWNS: i32 = 14;
pub const SCMD_AUTOEXIT: i32 = 15;
pub const SCMD_TRACK: i32 = 16;

/* do_wizutil */
pub const SCMD_REROLL: i32 = 0;
pub const SCMD_PARDON: i32 = 1;
pub const SCMD_NOTITLE: i32 = 2;
pub const SCMD_SQUELCH: i32 = 3;
pub const SCMD_FREEZE: i32 = 4;
pub const SCMD_THAW: i32 = 5;
pub const SCMD_UNAFFECT: i32 = 6;

/* do_spec_com */
pub const SCMD_WHISPER: i32 = 0;
pub const SCMD_ASK: i32 = 1;

/* do_gen_com */
pub const SCMD_HOLLER: i32 = 0;
pub const SCMD_SHOUT: i32 = 1;
pub const SCMD_GOSSIP: i32 = 2;
pub const SCMD_AUCTION: i32 = 3;
pub const SCMD_GRATZ: i32 = 4;

/* do_shutdown */
pub const SCMD_SHUTDOW: i32 = 0;
pub const SCMD_SHUTDOWN: i32 = 1;

/* do_quit */
pub const SCMD_QUI: i32 = 0;
pub const SCMD_QUIT: i32 = 1;

/* do_date */
pub const SCMD_DATE: i32 = 0;
pub const SCMD_UPTIME: i32 = 1;

/* do_commands */
pub const SCMD_COMMANDS: i32 = 0;
pub const SCMD_SOCIALS: i32 = 1;
pub const SCMD_WIZHELP: i32 = 2;

/* do_drop */
pub const SCMD_DROP: u8 = 0;
pub const SCMD_JUNK: u8 = 1;
pub const SCMD_DONATE: u8 = 2;

/* do_gen_write */
pub const SCMD_BUG: i32 = 0;
pub const SCMD_TYPO: i32 = 1;
pub const SCMD_IDEA: i32 = 2;

/* do_pour */
pub const SCMD_POUR: i32 = 0;
pub const SCMD_FILL: i32 = 1;

/* do_poof */
pub const SCMD_POOFIN: i32 = 0;
pub const SCMD_POOFOUT: i32 = 1;

/* do_hit */
pub const SCMD_HIT: i32 = 0;
pub const SCMD_MURDER: i32 = 1;

/* do_eat */
pub const SCMD_EAT: i32 = 0;
pub const SCMD_TASTE: i32 = 1;
pub const SCMD_DRINK: i32 = 2;
pub const SCMD_SIP: i32 = 3;

/* do_use */
pub const SCMD_USE: i32 = 0;
pub const SCMD_QUAFF: i32 = 1;
pub const SCMD_RECITE: i32 = 2;

/* do_look */
pub const SCMD_LOOK: i32 = 0;
pub const SCMD_READ: i32 = 1;

/* do_qcomm */
pub const SCMD_QSAY: i32 = 0;
pub const SCMD_QECHO: i32 = 1;

/* do_echo */
pub const SCMD_ECHO: i32 = 0;
pub const SCMD_EMOTE: i32 = 1;

/* do_gen_door */
pub const SCMD_OPEN: i32 = 0;
pub const SCMD_CLOSE: i32 = 1;
pub const SCMD_UNLOCK: i32 = 2;
pub const SCMD_LOCK: i32 = 3;
pub const SCMD_PICK: i32 = 4;

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
type Command = fn(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    cmd: usize,
    subcmd: i32,
);

pub struct CommandInfo {
    pub(crate) command: &'static str,
    minimum_position: u8,
    pub(crate) command_pointer: Command,
    pub(crate) minimum_level: i16,
    subcmd: i32,
}

pub fn do_nothing(
    _game: &mut Game,
    _db: &mut DB,
    _chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>, 
    _chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
}

pub const CMD_INFO: [CommandInfo; 308] = [
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
    CommandInfo {
        command: "at",
        minimum_position: POS_DEAD,
        command_pointer: do_at,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    // { "advance"  , POS_DEAD    , do_advance  , LVL_IMPL, 0 },
    CommandInfo {
        command: "advance",
        minimum_position: POS_DEAD,
        command_pointer: do_advance,
        minimum_level: LVL_IMPL,
        subcmd: 0,
    },
    // { "alias"    , POS_DEAD    , do_alias    , 0, 0 },
    CommandInfo {
        command: "alias",
        minimum_position: POS_DEAD,
        command_pointer: do_alias,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "accuse"   , POS_SITTING , do_action   , 0, 0 },
    CommandInfo {
        command: "accuse",
        minimum_position: POS_SITTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "applaud"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "applaud",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "assist"   , POS_FIGHTING, do_assist   , 1, 0 },
    CommandInfo {
        command: "assist",
        minimum_position: POS_FIGHTING,
        command_pointer: do_assist,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "ask"      , POS_RESTING , do_spec_comm, 0, SCMD_ASK },
    CommandInfo {
        command: "ask",
        minimum_position: POS_RESTING,
        command_pointer: do_spec_comm,
        minimum_level: 0,
        subcmd: SCMD_ASK,
    },
    // { "auction"  , POS_SLEEPING, do_gen_comm , 0, SCMD_AUCTION },
    CommandInfo {
        command: "auction",
        minimum_position: POS_SLEEPING,
        command_pointer: do_gen_comm,
        minimum_level: 0,
        subcmd: SCMD_AUCTION,
    },
    // { "autoexit" , POS_DEAD    , do_gen_tog  , 0, SCMD_AUTOEXIT },
    CommandInfo {
        command: "autoexit",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 0,
        subcmd: SCMD_AUTOEXIT,
    },
    //
    // { "bounce"   , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "bounce",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "backstab" , POS_STANDING, do_backstab , 1, 0 },
    CommandInfo {
        command: "backstab",
        minimum_position: POS_STANDING,
        command_pointer: do_backstab,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "ban"      , POS_DEAD    , do_ban      , LVL_GRGOD, 0 },
    CommandInfo {
        command: "ban",
        minimum_position: POS_DEAD,
        command_pointer: do_ban,
        minimum_level: LVL_GRGOD,
        subcmd: 0,
    },
    // { "balance"  , POS_STANDING, do_not_here , 1, 0 },
    CommandInfo {
        command: "balance",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "bash"     , POS_FIGHTING, do_bash     , 1, 0 },
    CommandInfo {
        command: "bash",
        minimum_position: POS_FIGHTING,
        command_pointer: do_bash,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "beg"      , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "beg",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "bleed"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "bleed",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "blush"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "blush",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "bow"      , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "bow",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "brb"      , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "brb",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "brief"    , POS_DEAD    , do_gen_tog  , 0, SCMD_BRIEF },
    CommandInfo {
        command: "brief",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 0,
        subcmd: SCMD_BRIEF,
    },
    // { "burp"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "burp",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "buy"      , POS_STANDING, do_not_here , 0, 0 },
    CommandInfo {
        command: "buy",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "bug"      , POS_DEAD    , do_gen_write, 0, SCMD_BUG },
    CommandInfo {
        command: "bug",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_write,
        minimum_level: 0,
        subcmd: SCMD_BUG,
    },
    //
    // { "cast"     , POS_SITTING , do_cast     , 1, 0 },
    CommandInfo {
        command: "cast",
        minimum_position: POS_SITTING,
        command_pointer: do_cast,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "cackle"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "cackle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "check"    , POS_STANDING, do_not_here , 1, 0 },
    CommandInfo {
        command: "check",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "chuckle"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "chuckle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "clap"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "clap",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "clear"    , POS_DEAD    , do_gen_ps   , 0, SCMD_CLEAR },
    CommandInfo {
        command: "clear",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_CLEAR,
    },
    // { "close"    , POS_SITTING , do_gen_door , 0, SCMD_CLOSE },
    CommandInfo {
        command: "close",
        minimum_position: POS_SITTING,
        command_pointer: do_gen_door,
        minimum_level: 0,
        subcmd: SCMD_CLOSE,
    },
    // { "cls"      , POS_DEAD    , do_gen_ps   , 0, SCMD_CLEAR },
    CommandInfo {
        command: "cls",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_CLEAR,
    },
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
    CommandInfo {
        command: "comfort",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "comb"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "comb",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "commands" , POS_DEAD    , do_commands , 0, SCMD_COMMANDS },
    CommandInfo {
        command: "commands",
        minimum_position: POS_DEAD,
        command_pointer: do_commands,
        minimum_level: 0,
        subcmd: SCMD_COMMANDS,
    },
    // { "compact"  , POS_DEAD    , do_gen_tog  , 0, SCMD_COMPACT },
    CommandInfo {
        command: "compact",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 0,
        subcmd: SCMD_COMPACT,
    },
    // { "cough"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "cough",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "credits"  , POS_DEAD    , do_gen_ps   , 0, SCMD_CREDITS },
    CommandInfo {
        command: "credits",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_CREDITS,
    },
    // { "cringe"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "cringe",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "cry"      , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "cry",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "cuddle"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "cuddle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "curse"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "curse",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "curtsey"  , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "curtsey",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    //
    // { "dance"    , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "dance",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "date"     , POS_DEAD    , do_date     , LVL_IMMORT, SCMD_DATE },
    CommandInfo {
        command: "date",
        minimum_position: POS_DEAD,
        command_pointer: do_date,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_DATE,
    },
    // { "daydream" , POS_SLEEPING, do_action   , 0, 0 },
    CommandInfo {
        command: "daydream",
        minimum_position: POS_SLEEPING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "dc"       , POS_DEAD    , do_dc       , LVL_GOD, 0 },
    CommandInfo {
        command: "dc",
        minimum_position: POS_DEAD,
        command_pointer: do_dc,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "deposit"  , POS_STANDING, do_not_here , 1, 0 },
    CommandInfo {
        command: "deposit",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "diagnose" , POS_RESTING , do_diagnose , 0, 0 },
    CommandInfo {
        command: "diagnose",
        minimum_position: POS_RESTING,
        command_pointer: do_diagnose,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "display"  , POS_DEAD    , do_display  , 0, 0 },
    CommandInfo {
        command: "display",
        minimum_position: POS_DEAD,
        command_pointer: do_display,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "donate"   , POS_RESTING , do_drop     , 0, SCMD_DONATE },
    CommandInfo {
        command: "donate",
        minimum_position: POS_RESTING,
        command_pointer: do_drop,
        minimum_level: 0,
        subcmd: SCMD_DONATE as i32,
    },
    // { "drink"    , POS_RESTING , do_drink    , 0, SCMD_DRINK },
    CommandInfo {
        command: "drink",
        minimum_position: POS_RESTING,
        command_pointer: do_drink,
        minimum_level: 0,
        subcmd: SCMD_DRINK,
    },
    // { "drop"     , POS_RESTING , do_drop     , 0, SCMD_DROP },
    CommandInfo {
        command: "drop",
        minimum_position: POS_RESTING,
        command_pointer: do_drop,
        minimum_level: 0,
        subcmd: SCMD_DROP as i32,
    },
    // { "drool"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "drool",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    //
    // { "eat"      , POS_RESTING , do_eat      , 0, SCMD_EAT },
    CommandInfo {
        command: "eat",
        minimum_position: POS_RESTING,
        command_pointer: do_eat,
        minimum_level: 0,
        subcmd: SCMD_EAT,
    },
    // { "echo"     , POS_SLEEPING, do_echo     , LVL_IMMORT, SCMD_ECHO },
    CommandInfo {
        command: "echo",
        minimum_position: POS_SLEEPING,
        command_pointer: do_echo,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_ECHO,
    },
    // { "emote"    , POS_RESTING , do_echo     , 1, SCMD_EMOTE },
    CommandInfo {
        command: "emote",
        minimum_position: POS_RESTING,
        command_pointer: do_echo,
        minimum_level: 1,
        subcmd: SCMD_ECHO,
    },
    // { ":"        , POS_RESTING, do_echo      , 1, SCMD_EMOTE },
    CommandInfo {
        command: ":",
        minimum_position: POS_RESTING,
        command_pointer: do_echo,
        minimum_level: 1,
        subcmd: SCMD_ECHO,
    },
    // { "embrace"  , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "embrace",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "enter"    , POS_STANDING, do_enter    , 0, 0 },
    CommandInfo {
        command: "enter",
        minimum_position: POS_STANDING,
        command_pointer: do_enter,
        minimum_level: 0,
        subcmd: 0,
    },
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
    CommandInfo {
        command: "examine",
        minimum_position: POS_SITTING,
        command_pointer: do_examine,
        minimum_level: 0,
        subcmd: 0,
    },
    //
    // { "force"    , POS_SLEEPING, do_force    , LVL_GOD, 0 },
    CommandInfo {
        command: "force",
        minimum_position: POS_SLEEPING,
        command_pointer: do_force,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "fart"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "fart",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "FILL"     , POS_STANDING, do_pour     , 0, SCMD_FILL },
    CommandInfo {
        command: "FILL",
        minimum_position: POS_STANDING,
        command_pointer: do_pour,
        minimum_level: 0,
        subcmd: SCMD_FILL,
    },
    // { "flee"     , POS_FIGHTING, do_flee     , 1, 0 },
    CommandInfo {
        command: "flee",
        minimum_position: POS_FIGHTING,
        command_pointer: do_flee,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "flip"     , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "flip",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "flirt"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "flirt",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "follow"   , POS_RESTING , do_follow   , 0, 0 },
    CommandInfo {
        command: "follow",
        minimum_position: POS_RESTING,
        command_pointer: do_follow,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "fondle"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "fondle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "freeze"   , POS_DEAD    , do_wizutil  , LVL_FREEZE, SCMD_FREEZE },
    CommandInfo {
        command: "freeze",
        minimum_position: POS_DEAD,
        command_pointer: do_wizutil,
        minimum_level: LVL_FREEZE as i16,
        subcmd: SCMD_FREEZE,
    },
    // { "french"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "french",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "frown"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "frown",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "fume"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "fume",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
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
    CommandInfo {
        command: "gasp",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "gecho"    , POS_DEAD    , do_gecho    , LVL_GOD, 0 },
    CommandInfo {
        command: "gecho",
        minimum_position: POS_DEAD,
        command_pointer: do_gecho,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "give"     , POS_RESTING , do_give     , 0, 0 },
    CommandInfo {
        command: "give",
        minimum_position: POS_RESTING,
        command_pointer: do_give,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "giggle"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "giggle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "glare"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "glare",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "goto"     , POS_SLEEPING, do_goto     , LVL_IMMORT, 0 },
    CommandInfo {
        command: "goto",
        minimum_position: POS_SLEEPING,
        command_pointer: do_goto,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    // { "gold"     , POS_RESTING , do_gold     , 0, 0 },
    CommandInfo {
        command: "gold",
        minimum_position: POS_RESTING,
        command_pointer: do_gold,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "gossip"   , POS_SLEEPING, do_gen_comm , 0, SCMD_GOSSIP },
    CommandInfo {
        command: "gossip",
        minimum_position: POS_SLEEPING,
        command_pointer: do_gen_comm,
        minimum_level: 0,
        subcmd: SCMD_GOSSIP,
    },
    // { "group"    , POS_RESTING , do_group    , 1, 0 },
    CommandInfo {
        command: "group",
        minimum_position: POS_RESTING,
        command_pointer: do_group,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "grab"     , POS_RESTING , do_grab     , 0, 0 },
    CommandInfo {
        command: "grab",
        minimum_position: POS_RESTING,
        command_pointer: do_grab,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "grats"    , POS_SLEEPING, do_gen_comm , 0, SCMD_GRATZ },
    CommandInfo {
        command: "grats",
        minimum_position: POS_SLEEPING,
        command_pointer: do_gen_comm,
        minimum_level: 0,
        subcmd: SCMD_GRATZ,
    },
    // { "greet"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "greet",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "grin"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "grin",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "groan"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "groan",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "grope"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "grope",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "grovel"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "grovel",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "growl"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "growl",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "gsay"     , POS_SLEEPING, do_gsay     , 0, 0 },
    CommandInfo {
        command: "gsay",
        minimum_position: POS_SLEEPING,
        command_pointer: do_gsay,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "gtell"    , POS_SLEEPING, do_gsay     , 0, 0 },
    //
    // { "help"     , POS_DEAD    , do_help     , 0, 0 },
    CommandInfo {
        command: "help",
        minimum_position: POS_DEAD,
        command_pointer: do_help,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "handbook" , POS_DEAD    , do_gen_ps   , LVL_IMMORT, SCMD_HANDBOOK },
    CommandInfo {
        command: "handbook",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_HANDBOOK,
    },
    // { "hcontrol" , POS_DEAD    , do_hcontrol , LVL_GRGOD, 0 },
    CommandInfo {
        command: "hcontrol",
        minimum_position: POS_DEAD,
        command_pointer: do_hcontrol,
        minimum_level: LVL_GRGOD,
        subcmd: 0,
    },
    // { "hiccup"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "hiccup",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "hide"     , POS_RESTING , do_hide     , 1, 0 },
    CommandInfo {
        command: "hide",
        minimum_position: POS_RESTING,
        command_pointer: do_hide,
        minimum_level: 1,
        subcmd: 0,
    },
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
    CommandInfo {
        command: "holler",
        minimum_position: POS_RESTING,
        command_pointer: do_gen_comm,
        minimum_level: 0,
        subcmd: SCMD_HOLLER,
    },
    // { "holylight", POS_DEAD    , do_gen_tog  , LVL_IMMORT, SCMD_HOLYLIGHT },
    CommandInfo {
        command: "holylight",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_HOLYLIGHT,
    },
    // { "hop"      , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "hop",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "house"    , POS_RESTING , do_house    , 0, 0 },
    CommandInfo {
        command: "house",
        minimum_position: POS_RESTING,
        command_pointer: do_house,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "hug"      , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "hug",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
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
    CommandInfo {
        command: "idea",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_write,
        minimum_level: 0,
        subcmd: SCMD_IDEA,
    },
    // { "imotd"    , POS_DEAD    , do_gen_ps   , LVL_IMMORT, SCMD_IMOTD },
    CommandInfo {
        command: "imotd",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_IMOTD,
    },
    // { "immlist"  , POS_DEAD    , do_gen_ps   , 0, SCMD_IMMLIST },
    CommandInfo {
        command: "immlist",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_IMMLIST,
    },
    // { "info"     , POS_SLEEPING, do_gen_ps   , 0, SCMD_INFO },
    CommandInfo {
        command: "info",
        minimum_position: POS_SLEEPING,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_INFO,
    },
    // { "insult"   , POS_RESTING , do_insult   , 0, 0 },
    CommandInfo {
        command: "insult",
        minimum_position: POS_RESTING,
        command_pointer: do_insult,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "invis"    , POS_DEAD    , do_invis    , LVL_IMMORT, 0 },
    CommandInfo {
        command: "invis",
        minimum_position: POS_DEAD,
        command_pointer: do_invis,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
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
    CommandInfo {
        command: "kill",
        minimum_position: POS_FIGHTING,
        command_pointer: do_kill,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "kick"     , POS_FIGHTING, do_kick     , 1, 0 },
    CommandInfo {
        command: "kick",
        minimum_position: POS_FIGHTING,
        command_pointer: do_kick,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "kiss"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "kiss",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
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
    CommandInfo {
        command: "laugh",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "last"     , POS_DEAD    , do_last     , LVL_GOD, 0 },
    CommandInfo {
        command: "last",
        minimum_position: POS_DEAD,
        command_pointer: do_last,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "leave"    , POS_STANDING, do_leave    , 0, 0 },
    CommandInfo {
        command: "leave",
        minimum_position: POS_STANDING,
        command_pointer: do_leave,
        minimum_level: 0,
        subcmd: 0,
    },
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
    CommandInfo {
        command: "lick",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "lock"     , POS_SITTING , do_gen_door , 0, SCMD_LOCK },
    CommandInfo {
        command: "lock",
        minimum_position: POS_SITTING,
        command_pointer: do_gen_door,
        minimum_level: 0,
        subcmd: SCMD_LOCK,
    },
    // { "load"     , POS_DEAD    , do_load     , LVL_GOD, 0 },
    CommandInfo {
        command: "load",
        minimum_position: POS_DEAD,
        command_pointer: do_load,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "love"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "love",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    //
    // { "moan"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "moan",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "motd"     , POS_DEAD    , do_gen_ps   , 0, SCMD_MOTD },
    CommandInfo {
        command: "motd",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_MOTD,
    },
    // { "mail"     , POS_STANDING, do_not_here , 1, 0 },
    CommandInfo {
        command: "mail",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "massage"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "massage",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "mute"     , POS_DEAD    , do_wizutil  , LVL_GOD, SCMD_SQUELCH },
    CommandInfo {
        command: "mute",
        minimum_position: POS_DEAD,
        command_pointer: do_wizutil,
        minimum_level: LVL_GOD,
        subcmd: SCMD_SQUELCH,
    },
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
    CommandInfo {
        command: "news",
        minimum_position: POS_SLEEPING,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_NEWS,
    },
    // { "nibble"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "nibble",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "nod"      , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "nod",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "noauction", POS_DEAD    , do_gen_tog  , 0, SCMD_NOAUCTION },
    CommandInfo {
        command: "noauction",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 0,
        subcmd: SCMD_NOAUCTION,
    },
    // { "nogossip" , POS_DEAD    , do_gen_tog  , 0, SCMD_NOGOSSIP },
    CommandInfo {
        command: "nogossip",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 0,
        subcmd: SCMD_NOGOSSIP,
    },
    // { "nograts"  , POS_DEAD    , do_gen_tog  , 0, SCMD_NOGRATZ },
    CommandInfo {
        command: "nograts",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 0,
        subcmd: SCMD_NOGRATZ,
    },
    // { "nohassle" , POS_DEAD    , do_gen_tog  , LVL_IMMORT, SCMD_NOHASSLE },
    CommandInfo {
        command: "nohassle",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_NOHASSLE,
    },
    // { "norepeat" , POS_DEAD    , do_gen_tog  , 0, SCMD_NOREPEAT },
    CommandInfo {
        command: "norepeat",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 0,
        subcmd: SCMD_NOREPEAT,
    },
    // { "noshout"  , POS_SLEEPING, do_gen_tog  , 1, SCMD_DEAF },
    CommandInfo {
        command: "noshout",
        minimum_position: POS_SLEEPING,
        command_pointer: do_gen_tog,
        minimum_level: 1,
        subcmd: SCMD_DEAF,
    },
    // { "nosummon" , POS_DEAD    , do_gen_tog  , 1, SCMD_NOSUMMON },
    CommandInfo {
        command: "nosummon",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 1,
        subcmd: SCMD_NOSUMMON,
    },
    // { "notell"   , POS_DEAD    , do_gen_tog  , 1, SCMD_NOTELL },
    CommandInfo {
        command: "notell",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 1,
        subcmd: SCMD_NOTELL,
    },
    // { "notitle"  , POS_DEAD    , do_wizutil  , LVL_GOD, SCMD_NOTITLE },
    CommandInfo {
        command: "notitle",
        minimum_position: POS_DEAD,
        command_pointer: do_wizutil,
        minimum_level: LVL_GOD,
        subcmd: SCMD_NOTITLE,
    },
    // { "nowiz"    , POS_DEAD    , do_gen_tog  , LVL_IMMORT, SCMD_NOWIZ },
    CommandInfo {
        command: "nowiz",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_NOWIZ,
    },
    // { "nudge"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "nudge",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "nuzzle"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "nuzzle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    //
    // { "olc"      , POS_DEAD    , do_olc      , LVL_IMPL, 0 },
    // { "order"    , POS_RESTING , do_order    , 1, 0 },
    CommandInfo {
        command: "order",
        minimum_position: POS_RESTING,
        command_pointer: do_order,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "offer"    , POS_STANDING, do_not_here , 1, 0 },
    CommandInfo {
        command: "offer",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "open"     , POS_SITTING , do_gen_door , 0, SCMD_OPEN },
    CommandInfo {
        command: "open",
        minimum_position: POS_SITTING,
        command_pointer: do_gen_door,
        minimum_level: 0,
        subcmd: SCMD_OPEN,
    },
    //
    // { "put"      , POS_RESTING , do_put      , 0, 0 },
    CommandInfo {
        command: "put",
        minimum_position: POS_RESTING,
        command_pointer: do_put,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "pat"      , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "pat",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "page"     , POS_DEAD    , do_page     , LVL_GOD, 0 },
    CommandInfo {
        command: "page",
        minimum_position: POS_DEAD,
        command_pointer: do_page,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "pardon"   , POS_DEAD    , do_wizutil  , LVL_GOD, SCMD_PARDON },
    CommandInfo {
        command: "pardon",
        minimum_position: POS_DEAD,
        command_pointer: do_wizutil,
        minimum_level: LVL_GOD,
        subcmd: SCMD_PARDON,
    },
    // { "peer"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "peer",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "pick"     , POS_STANDING, do_gen_door , 1, SCMD_PICK },
    CommandInfo {
        command: "pick",
        minimum_position: POS_STANDING,
        command_pointer: do_gen_door,
        minimum_level: 0,
        subcmd: SCMD_PICK,
    },
    // { "point"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "point",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "poke"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "poke",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "policy"   , POS_DEAD    , do_gen_ps   , 0, SCMD_POLICIES },
    CommandInfo {
        command: "policy",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_POLICIES,
    },
    // { "ponder"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "ponder",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "poofin"   , POS_DEAD    , do_poofset  , LVL_IMMORT, SCMD_POOFIN },
    CommandInfo {
        command: "poofin",
        minimum_position: POS_DEAD,
        command_pointer: do_poofset,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_POOFIN,
    },
    // { "poofout"  , POS_DEAD    , do_poofset  , LVL_IMMORT, SCMD_POOFOUT },
    CommandInfo {
        command: "poofout",
        minimum_position: POS_DEAD,
        command_pointer: do_poofset,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_POOFOUT,
    },
    // { "pour"     , POS_STANDING, do_pour     , 0, SCMD_POUR },
    CommandInfo {
        command: "pour",
        minimum_position: POS_STANDING,
        command_pointer: do_pour,
        minimum_level: 0,
        subcmd: SCMD_POUR,
    },
    // { "pout"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "pout",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "prompt"   , POS_DEAD    , do_display  , 0, 0 },
    CommandInfo {
        command: "prompt",
        minimum_position: POS_DEAD,
        command_pointer: do_display,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "practice" , POS_RESTING , do_practice , 1, 0 },
    CommandInfo {
        command: "practice",
        minimum_position: POS_RESTING,
        command_pointer: do_practice,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "pray"     , POS_SITTING , do_action   , 0, 0 },
    CommandInfo {
        command: "pray",
        minimum_position: POS_SITTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "puke"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "puke",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "punch"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "punch",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "purr"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "purr",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "purge"    , POS_DEAD    , do_purge    , LVL_GOD, 0 },
    CommandInfo {
        command: "purge",
        minimum_position: POS_DEAD,
        command_pointer: do_purge,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    //
    // { "quaff"    , POS_RESTING , do_use      , 0, SCMD_QUAFF },
    CommandInfo {
        command: "quaff",
        minimum_position: POS_RESTING,
        command_pointer: do_use,
        minimum_level: 0,
        subcmd: SCMD_QUAFF,
    },
    // { "qecho"    , POS_DEAD    , do_qcomm    , LVL_IMMORT, SCMD_QECHO },
    CommandInfo {
        command: "qecho",
        minimum_position: POS_DEAD,
        command_pointer: do_qcomm,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_QECHO,
    },
    // { "quest"    , POS_DEAD    , do_gen_tog  , 0, SCMD_QUEST },
    CommandInfo {
        command: "quest",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: 0,
        subcmd: SCMD_QUEST,
    },
    // { "qui"      , POS_DEAD    , do_quit     , 0, 0 },
    CommandInfo {
        command: "qui",
        minimum_position: POS_DEAD,
        command_pointer: do_quit,
        minimum_level: 0,
        subcmd: SCMD_QUI,
    },
    // { "quit"     , POS_DEAD    , do_quit     , 0, SCMD_QUIT },
    CommandInfo {
        command: "quit",
        minimum_position: POS_DEAD,
        command_pointer: do_quit,
        minimum_level: 0,
        subcmd: SCMD_QUIT,
    },
    // { "qsay"     , POS_RESTING , do_qcomm    , 0, SCMD_QSAY },
    CommandInfo {
        command: "qsay",
        minimum_position: POS_RESTING,
        command_pointer: do_qcomm,
        minimum_level: 0,
        subcmd: SCMD_QSAY,
    },
    //
    // { "reply"    , POS_SLEEPING, do_reply    , 0, 0 },
    CommandInfo {
        command: "reply",
        minimum_position: POS_SLEEPING,
        command_pointer: do_reply,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "rest"     , POS_RESTING , do_rest     , 0, 0 },
    CommandInfo {
        command: "rest",
        minimum_position: POS_RESTING,
        command_pointer: do_rest,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "read"     , POS_RESTING , do_look     , 0, SCMD_READ },
    CommandInfo {
        command: "read",
        minimum_position: POS_RESTING,
        command_pointer: do_look,
        minimum_level: 0,
        subcmd: SCMD_READ,
    },
    // { "reload"   , POS_DEAD    , do_reboot   , LVL_IMPL, 0 },
    CommandInfo {
        command: "reload",
        minimum_position: POS_DEAD,
        command_pointer: do_reboot,
        minimum_level: LVL_IMPL,
        subcmd: SCMD_READ,
    },
    // { "recite"   , POS_RESTING , do_use      , 0, SCMD_RECITE },
    CommandInfo {
        command: "recite",
        minimum_position: POS_RESTING,
        command_pointer: do_use,
        minimum_level: 0,
        subcmd: SCMD_RECITE,
    },
    // { "receive"  , POS_STANDING, do_not_here , 1, 0 },
    CommandInfo {
        command: "receive",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "remove"   , POS_RESTING , do_remove   , 0, 0 },
    CommandInfo {
        command: "remove",
        minimum_position: POS_RESTING,
        command_pointer: do_remove,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "rent"     , POS_STANDING, do_not_here , 1, 0 },
    CommandInfo {
        command: "rent",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "report"   , POS_RESTING , do_report   , 0, 0 },
    CommandInfo {
        command: "report",
        minimum_position: POS_RESTING,
        command_pointer: do_report,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "reroll"   , POS_DEAD    , do_wizutil  , LVL_GRGOD, SCMD_REROLL },
    CommandInfo {
        command: "reroll",
        minimum_position: POS_DEAD,
        command_pointer: do_wizutil,
        minimum_level: LVL_GRGOD,
        subcmd: SCMD_REROLL,
    },
    // { "rescue"   , POS_FIGHTING, do_rescue   , 1, 0 },
    CommandInfo {
        command: "rescue",
        minimum_position: POS_FIGHTING,
        command_pointer: do_rescue,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "restore"  , POS_DEAD    , do_restore  , LVL_GOD, 0 },
    CommandInfo {
        command: "restore",
        minimum_position: POS_DEAD,
        command_pointer: do_restore,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "return"   , POS_DEAD    , do_return   , 0, 0 },
    CommandInfo {
        command: "return",
        minimum_position: POS_DEAD,
        command_pointer: do_return,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "roll"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "roll",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "roomflags", POS_DEAD    , do_gen_tog  , LVL_IMMORT, SCMD_ROOMFLAGS },
    CommandInfo {
        command: "roomflags",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_ROOMFLAGS,
    },
    // { "ruffle"   , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "ruffle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    //
    // { "say"      , POS_RESTING , do_say      , 0, 0 },
    CommandInfo {
        command: "say",
        minimum_position: POS_RESTING,
        command_pointer: do_say,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "'"        , POS_RESTING , do_say      , 0, 0 },
    CommandInfo {
        command: "'",
        minimum_position: POS_RESTING,
        command_pointer: do_say,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "save"     , POS_SLEEPING, do_save     , 0, 0 },
    CommandInfo {
        command: "save",
        minimum_position: POS_SLEEPING,
        command_pointer: do_save,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "score"    , POS_DEAD    , do_score    , 0, 0 },
    CommandInfo {
        command: "score",
        minimum_position: POS_DEAD,
        command_pointer: do_score,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "scream"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "scream",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "sell"     , POS_STANDING, do_not_here , 0, 0 },
    CommandInfo {
        command: "sell",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "send"     , POS_SLEEPING, do_send     , LVL_GOD, 0 },
    CommandInfo {
        command: "send",
        minimum_position: POS_SLEEPING,
        command_pointer: do_send,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "set"      , POS_DEAD    , do_set      , LVL_GOD, 0 },
    CommandInfo {
        command: "set",
        minimum_position: POS_DEAD,
        command_pointer: do_set,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "shout"    , POS_RESTING , do_gen_comm , 0, SCMD_SHOUT },
    CommandInfo {
        command: "shout",
        minimum_position: POS_RESTING,
        command_pointer: do_gen_comm,
        minimum_level: 0,
        subcmd: SCMD_SHOUT,
    },
    // { "shake"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "shake",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "shiver"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "shiver",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "show"     , POS_DEAD    , do_show     , LVL_IMMORT, 0 },
    CommandInfo {
        command: "show",
        minimum_position: POS_DEAD,
        command_pointer: do_show,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    // { "shrug"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "shrug",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "shutdow"  , POS_DEAD    , do_shutdown , LVL_IMPL, 0 },
    CommandInfo {
        command: "shutdow",
        minimum_position: POS_DEAD,
        command_pointer: do_shutdown,
        minimum_level: LVL_IMPL,
        subcmd: SCMD_SHUTDOW,
    },
    // { "shutdown" , POS_DEAD    , do_shutdown , LVL_IMPL, SCMD_SHUTDOWN },
    CommandInfo {
        command: "shutdown",
        minimum_position: POS_DEAD,
        command_pointer: do_shutdown,
        minimum_level: LVL_IMPL,
        subcmd: SCMD_SHUTDOWN,
    },
    // { "sigh"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "sigh",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "sing"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "sing",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "sip"      , POS_RESTING , do_drink    , 0, SCMD_SIP },
    CommandInfo {
        command: "sip",
        minimum_position: POS_RESTING,
        command_pointer: do_drink,
        minimum_level: 0,
        subcmd: SCMD_SIP,
    },
    // { "sit"      , POS_RESTING , do_sit      , 0, 0 },
    CommandInfo {
        command: "sit",
        minimum_position: POS_RESTING,
        command_pointer: do_sit,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "skillset" , POS_SLEEPING, do_skillset , LVL_GRGOD, 0 },
    CommandInfo {
        command: "skillset",
        minimum_position: POS_SLEEPING,
        command_pointer: do_skillset,
        minimum_level: LVL_GRGOD,
        subcmd: 0,
    },
    // { "sleep"    , POS_SLEEPING, do_sleep    , 0, 0 },
    CommandInfo {
        command: "sleep",
        minimum_position: POS_SLEEPING,
        command_pointer: do_sleep,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "slap"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "slap",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "slowns"   , POS_DEAD    , do_gen_tog  , LVL_IMPL, SCMD_SLOWNS },
    CommandInfo {
        command: "slowns",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: LVL_IMPL,
        subcmd: SCMD_SLOWNS,
    },
    // { "smile"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "smile",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "smirk"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "smirk",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "snicker"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "snicker",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "snap"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "snap",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "snarl"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "snarl",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "sneeze"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "sneeze",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "sneak"    , POS_STANDING, do_sneak    , 1, 0 },
    CommandInfo {
        command: "sneak",
        minimum_position: POS_STANDING,
        command_pointer: do_sneak,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "sniff"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "sniff",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "snore"    , POS_SLEEPING, do_action   , 0, 0 },
    CommandInfo {
        command: "snore",
        minimum_position: POS_SLEEPING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "snowball" , POS_STANDING, do_action   , LVL_IMMORT, 0 },
    CommandInfo {
        command: "snowball",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "snoop"    , POS_DEAD    , do_snoop    , LVL_GOD, 0 },
    CommandInfo {
        command: "snoop",
        minimum_position: POS_DEAD,
        command_pointer: do_snoop,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "snuggle"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "snuggle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "socials"  , POS_DEAD    , do_commands , 0, SCMD_SOCIALS },
    CommandInfo {
        command: "socials",
        minimum_position: POS_DEAD,
        command_pointer: do_commands,
        minimum_level: 0,
        subcmd: SCMD_SOCIALS,
    },
    // { "split"    , POS_SITTING , do_split    , 1, 0 },
    CommandInfo {
        command: "split",
        minimum_position: POS_SITTING,
        command_pointer: do_split,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "spank"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "spank",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "spit"     , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "spit",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "squeeze"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "squeeze",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "stand"    , POS_RESTING , do_stand    , 0, 0 },
    CommandInfo {
        command: "stand",
        minimum_position: POS_RESTING,
        command_pointer: do_stand,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "stare"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "stare",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "stat"     , POS_DEAD    , do_stat     , LVL_IMMORT, 0 },
    CommandInfo {
        command: "stat",
        minimum_position: POS_DEAD,
        command_pointer: do_stat,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    // { "steal"    , POS_STANDING, do_steal    , 1, 0 },
    CommandInfo {
        command: "steal",
        minimum_position: POS_STANDING,
        command_pointer: do_steal,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "steam"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "steam",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "stroke"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "stroke",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "strut"    , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "strut",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "sulk"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "sulk",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "switch"   , POS_DEAD    , do_switch   , LVL_GRGOD, 0 },
    CommandInfo {
        command: "switch",
        minimum_position: POS_DEAD,
        command_pointer: do_switch,
        minimum_level: LVL_GRGOD,
        subcmd: 0,
    },
    // { "syslog"   , POS_DEAD    , do_syslog   , LVL_IMMORT, 0 },
    CommandInfo {
        command: "syslog",
        minimum_position: POS_DEAD,
        command_pointer: do_syslog,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    //
    // { "tell"     , POS_DEAD    , do_tell     , 0, 0 },
    CommandInfo {
        command: "tell",
        minimum_position: POS_DEAD,
        command_pointer: do_tell,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "tackle"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "tackle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "take"     , POS_RESTING , do_get      , 0, 0 },
    CommandInfo {
        command: "take",
        minimum_position: POS_RESTING,
        command_pointer: do_get,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "tango"    , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "tango",
        minimum_position: POS_STANDING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "taunt"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "taunt",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "taste"    , POS_RESTING , do_eat      , 0, SCMD_TASTE },
    CommandInfo {
        command: "taste",
        minimum_position: POS_RESTING,
        command_pointer: do_eat,
        minimum_level: 0,
        subcmd: SCMD_TASTE,
    },
    // { "teleport" , POS_DEAD    , do_teleport , LVL_GOD, 0 },
    CommandInfo {
        command: "teleport",
        minimum_position: POS_DEAD,
        command_pointer: do_teleport,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "thank"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "thank",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "think"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "think",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "thaw"     , POS_DEAD    , do_wizutil  , LVL_FREEZE, SCMD_THAW },
    CommandInfo {
        command: "thaw",
        minimum_position: POS_DEAD,
        command_pointer: do_wizutil,
        minimum_level: LVL_FREEZE as i16,
        subcmd: SCMD_THAW,
    },
    // { "title"    , POS_DEAD    , do_title    , 0, 0 },
    CommandInfo {
        command: "title",
        minimum_position: POS_DEAD,
        command_pointer: do_title,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "tickle"   , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "tickle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "time"     , POS_DEAD    , do_time     , 0, 0 },
    CommandInfo {
        command: "time",
        minimum_position: POS_DEAD,
        command_pointer: do_time,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "toggle"   , POS_DEAD    , do_toggle   , 0, 0 },
    CommandInfo {
        command: "toggle",
        minimum_position: POS_DEAD,
        command_pointer: do_toggle,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "track"    , POS_STANDING, do_track    , 0, 0 },
    CommandInfo {
        command: "track",
        minimum_position: POS_STANDING,
        command_pointer: do_track,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "trackthru", POS_DEAD    , do_gen_tog  , LVL_IMPL, SCMD_TRACK },
    CommandInfo {
        command: "trackthru",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_tog,
        minimum_level: LVL_IMPL,
        subcmd: SCMD_TRACK,
    },
    // { "transfer" , POS_SLEEPING, do_trans    , LVL_GOD, 0 },
    CommandInfo {
        command: "transfer",
        minimum_position: POS_SLEEPING,
        command_pointer: do_trans,
        minimum_level: LVL_GOD,
        subcmd: 0,
    },
    // { "twiddle"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "twiddle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "typo"     , POS_DEAD    , do_gen_write, 0, SCMD_TYPO },
    CommandInfo {
        command: "typo",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_write,
        minimum_level: 0,
        subcmd: SCMD_TYPO,
    },
    //
    // { "unlock"   , POS_SITTING , do_gen_door , 0, SCMD_UNLOCK },
    CommandInfo {
        command: "unlock",
        minimum_position: POS_SITTING,
        command_pointer: do_gen_door,
        minimum_level: 0,
        subcmd: SCMD_UNLOCK,
    },
    // { "ungroup"  , POS_DEAD    , do_ungroup  , 0, 0 },
    CommandInfo {
        command: "ungroup",
        minimum_position: POS_DEAD,
        command_pointer: do_ungroup,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "unban"    , POS_DEAD    , do_unban    , LVL_GRGOD, 0 },
    CommandInfo {
        command: "unban",
        minimum_position: POS_DEAD,
        command_pointer: do_unban,
        minimum_level: LVL_GRGOD,
        subcmd: 0,
    },
    // { "unaffect" , POS_DEAD    , do_wizutil  , LVL_GOD, SCMD_UNAFFECT },
    CommandInfo {
        command: "unaffect",
        minimum_position: POS_DEAD,
        command_pointer: do_wizutil,
        minimum_level: LVL_GOD,
        subcmd: SCMD_UNAFFECT,
    },
    // { "uptime"   , POS_DEAD    , do_date     , LVL_IMMORT, SCMD_UPTIME },
    CommandInfo {
        command: "uptime",
        minimum_position: POS_DEAD,
        command_pointer: do_date,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_UPTIME,
    },
    // { "use"      , POS_SITTING , do_use      , 1, SCMD_USE },
    CommandInfo {
        command: "use",
        minimum_position: POS_SITTING,
        command_pointer: do_use,
        minimum_level: 1,
        subcmd: SCMD_USE,
    },
    // { "users"    , POS_DEAD    , do_users    , LVL_IMMORT, 0 },
    CommandInfo {
        command: "users",
        minimum_position: POS_DEAD,
        command_pointer: do_users,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    //
    // { "value"    , POS_STANDING, do_not_here , 0, 0 },
    CommandInfo {
        command: "value",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "version"  , POS_DEAD    , do_gen_ps   , 0, SCMD_VERSION },
    CommandInfo {
        command: "version",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_VERSION,
    },
    // { "visible"  , POS_RESTING , do_visible  , 1, 0 },
    CommandInfo {
        command: "visible",
        minimum_position: POS_RESTING,
        command_pointer: do_visible,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "vnum"     , POS_DEAD    , do_vnum     , LVL_IMMORT, 0 },
    CommandInfo {
        command: "vnum",
        minimum_position: POS_DEAD,
        command_pointer: do_vnum,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    // { "vstat"    , POS_DEAD    , do_vstat    , LVL_IMMORT, 0 },
    CommandInfo {
        command: "vstat",
        minimum_position: POS_DEAD,
        command_pointer: do_vstat,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    // { "wake"     , POS_SLEEPING, do_wake     , 0, 0 },
    CommandInfo {
        command: "wake",
        minimum_position: POS_SLEEPING,
        command_pointer: do_wake,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "wave"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "wave",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
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
    CommandInfo {
        command: "who",
        minimum_position: POS_DEAD,
        command_pointer: do_who,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "whoami"   , POS_DEAD    , do_gen_ps   , 0, SCMD_WHOAMI },
    CommandInfo {
        command: "whoami",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_WHOAMI,
    },
    // { "where"    , POS_RESTING , do_where    , 1, 0 },
    CommandInfo {
        command: "where",
        minimum_position: POS_RESTING,
        command_pointer: do_where,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "whisper"  , POS_RESTING , do_spec_comm, 0, SCMD_WHISPER },
    CommandInfo {
        command: "whisper",
        minimum_position: POS_RESTING,
        command_pointer: do_spec_comm,
        minimum_level: 0,
        subcmd: SCMD_WHISPER,
    },
    // { "whine"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "whine",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "whistle"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "whistle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "wield"    , POS_RESTING , do_wield    , 0, 0 },
    CommandInfo {
        command: "wield",
        minimum_position: POS_RESTING,
        command_pointer: do_wield,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "wiggle"   , POS_STANDING, do_action   , 0, 0 },
    CommandInfo {
        command: "wiggle",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "wimpy"    , POS_DEAD    , do_wimpy    , 0, 0 },
    CommandInfo {
        command: "wimpy",
        minimum_position: POS_DEAD,
        command_pointer: do_wimpy,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "wink"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "wink",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "withdraw" , POS_STANDING, do_not_here , 1, 0 },
    CommandInfo {
        command: "withdraw",
        minimum_position: POS_STANDING,
        command_pointer: do_not_here,
        minimum_level: 1,
        subcmd: 0,
    },
    // { "wiznet"   , POS_DEAD    , do_wiznet   , LVL_IMMORT, 0 },
    CommandInfo {
        command: "wiznet",
        minimum_position: POS_DEAD,
        command_pointer: do_wiznet,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    // { ";"        , POS_DEAD    , do_wiznet   , LVL_IMMORT, 0 },
    CommandInfo {
        command: ";",
        minimum_position: POS_DEAD,
        command_pointer: do_wiznet,
        minimum_level: LVL_IMMORT,
        subcmd: 0,
    },
    // { "wizhelp"  , POS_SLEEPING, do_commands , LVL_IMMORT, SCMD_WIZHELP },
    CommandInfo {
        command: "wizhelp",
        minimum_position: POS_SLEEPING,
        command_pointer: do_commands,
        minimum_level: LVL_IMMORT,
        subcmd: SCMD_WIZHELP,
    },
    // { "wizlist"  , POS_DEAD    , do_gen_ps   , 0, SCMD_WIZLIST },
    CommandInfo {
        command: "wizlist",
        minimum_position: POS_DEAD,
        command_pointer: do_gen_ps,
        minimum_level: 0,
        subcmd: SCMD_WIZLIST,
    },
    // { "wizlock"  , POS_DEAD    , do_wizlock  , LVL_IMPL, 0 },
    CommandInfo {
        command: "wizlock",
        minimum_position: POS_DEAD,
        command_pointer: do_wizlock,
        minimum_level: LVL_IMPL,
        subcmd: 0,
    },
    // { "worship"  , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "worship",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "write"    , POS_STANDING, do_write    , 1, 0 },
    CommandInfo {
        command: "write",
        minimum_position: POS_STANDING,
        command_pointer: do_write,
        minimum_level: 1,
        subcmd: 0,
    },
    //
    // { "yawn"     , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "yawn",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    // { "yodel"    , POS_RESTING , do_action   , 0, 0 },
    CommandInfo {
        command: "yodel",
        minimum_position: POS_RESTING,
        command_pointer: do_action,
        minimum_level: 0,
        subcmd: 0,
    },
    //
    // { "zreset"   , POS_DEAD    , do_zreset   , LVL_GRGOD, 0 },
    CommandInfo {
        command: "zreset",
        minimum_position: POS_DEAD,
        command_pointer: do_zreset,
        minimum_level: LVL_GRGOD,
        subcmd: 0,
    },
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
pub fn command_interpreter(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
) {
    let ch = chars.get_mut(chid);
    let line: &str;
    let mut arg = String::new();

    ch.remove_aff_flags(AffectFlags::HIDE);

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
        send_to_char(&mut game.descriptors, ch, "Huh?!?\r\n");
    } else if !ch.is_npc() && ch.plr_flagged(PLR_FROZEN) && ch.get_level() < LVL_IMPL as u8 {
        send_to_char(&mut game.descriptors, ch, "You try, but the mind-numbing cold prevents you...\r\n");
    } else if cmd.command_pointer as usize == do_nothing as usize {
        send_to_char(&mut game.descriptors, ch, "Sorry, that command hasn't been implemented yet.\r\n");
    } else if ch.is_npc() && cmd.minimum_level >= LVL_IMMORT {
        send_to_char(&mut game.descriptors, ch, "You can't use immortal commands while switched.\r\n");
    } else if ch.get_pos() < cmd.minimum_position {
        match ch.get_pos() {
            POS_DEAD => {
                send_to_char(&mut game.descriptors, ch, "Lie still; you are DEAD!!! :-(\r\n");
            }
            POS_INCAP | POS_MORTALLYW => {
                send_to_char(&mut game.descriptors, 
                    ch,
                    "You are in a pretty bad shape, unable to do anything!\r\n",
                );
            }
            POS_STUNNED => {
                send_to_char(&mut game.descriptors, ch, "All you can do right now is think about the stars!\r\n");
            }
            POS_SLEEPING => {
                send_to_char(&mut game.descriptors, ch, "In your dreams, or what?\r\n");
            }
            POS_RESTING => {
                send_to_char(&mut game.descriptors, ch, "Nah... You feel too relaxed to do that..\r\n");
            }
            POS_SITTING => {
                send_to_char(&mut game.descriptors, ch, "Maybe you should get on your feet first?\r\n");
            }
            POS_FIGHTING => {
                send_to_char(&mut game.descriptors, ch, "No way!  You're fighting for your life!\r\n");
            }
            _ => {}
        }
    } else if db.no_specials || !special(game, db,chars,  texts, objs, chid, cmd_idx as i32, line) {
        (cmd.command_pointer)(game, db,chars, texts,objs, chid, line, cmd_idx, cmd.subcmd);
    }
}

/**************************************************************************
 * Routines to handle aliasing                                             *
 **************************************************************************/

fn find_alias<'a, 'b>(alias_list: &'a Vec<AliasData>, alias: &'b str) -> Option<&'a AliasData> {
    alias_list.iter().find(|e| e.alias.as_ref() == alias)
}

/* The interface to the outside world: do_alias */
pub fn do_alias(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    if ch.is_npc() {
        return;
    }

    let mut repl = any_one_arg(argument, &mut arg).to_string();

    if arg.is_empty() {
        /* no argument specified -- list currently defined aliases */
        send_to_char(&mut game.descriptors, ch, "Currently defined aliases:\r\n");
        let ch = chars.get(chid);
        if ch.player_specials.aliases.len() == 0 {
            send_to_char(&mut game.descriptors, ch, " None.\r\n");
        } else {
            for a in &ch.player_specials.aliases {
                send_to_char(&mut game.descriptors, ch, format!("{:15} {}\r\n", a.alias, a.replacement).as_str());
            }
        }
    } else {
        /* otherwise, add or remove aliases */
        /* is this an alias we've already defined? */
        let a = ch
            .player_specials
            .aliases
            .iter()
            .position(|e| e.alias.as_ref() == &arg);
        if a.is_some() {
            let ch = chars.get_mut(chid);
            ch.player_specials.aliases.remove(a.unwrap());
        }
        let ch = chars.get(chid);
        /* if no replacement string is specified, assume we want to delete */
        if repl.is_empty() {
            if a.is_none() {
                send_to_char(&mut game.descriptors, ch, "No such alias.\r\n");
            } else {
                send_to_char(&mut game.descriptors, ch, "Alias deleted.\r\n");
            }
        } else {
            /* otherwise, either add or redefine an alias */
            if arg == "alias" {
                send_to_char(&mut game.descriptors, ch, "You can't alias 'alias'.\r\n");
                return;
            }
            delete_doubledollar(&mut repl);

            let mut a = AliasData {
                alias: Rc::from(arg.as_str()),
                replacement: Rc::from(repl),
                type_: 0,
            };

            if a.replacement.contains(ALIAS_SEP_CHAR) || a.replacement.contains(ALIAS_VAR_CHAR) {
                a.type_ = ALIAS_COMPLEX;
            } else {
                a.type_ = ALIAS_SIMPLE;
            }
            let ch = chars.get_mut(chid);
            ch.player_specials.aliases.push(a);
            send_to_char(&mut game.descriptors, ch, "Alias added.\r\n");
        }
    }
}

/*
 * Valid numeric replacements are only $1 .. $9 (makes parsing a little
 * easier, and it's not that much of a limitation anyway.)  Also valid
 * is "$*", which stands for the entire original line after the alias.
 * ";" is used to delimit commands.
 */
const NUM_TOKENS: i32 = 9;

fn perform_complex_alias(input_q: &mut LinkedList<TxtBlock>, orig: &str, a: &AliasData) {
    let mut num_of_tokens = 0;
    let mut tokens = [0 as usize; NUM_TOKENS as usize];

    /* First, parse the original string */
    let mut buf = String::new();
    let buf2 = orig.to_string();
    let mut temp = buf2.find(' ');

    while temp.is_some() && num_of_tokens < NUM_TOKENS {
        tokens[num_of_tokens as usize] = temp.unwrap();
        num_of_tokens += 1;
        temp = buf2.as_str()[temp.unwrap() + 1..].find(' ');
    }

    /* initialize */
    let mut num;
    /* now parse the alias */
    let mut temp = a.replacement.as_ref();
    while !temp.is_empty() {
        if temp.starts_with(ALIAS_SEP_CHAR) {
            write_to_q(&buf, input_q, true);
            buf.clear();
        } else if temp.starts_with(ALIAS_VAR_CHAR) {
            temp = &temp[1..];
            if {
                num = temp.chars().next().unwrap() as u32 - '1' as u32;
                num < num_of_tokens as u32 /*&& num >= 0*/
            } {
                buf.push_str(&buf2[tokens[num as usize]..]);
            } else if temp.starts_with(ALIAS_GLOB_CHAR) {
                buf.push_str(orig);
            } else if {
                buf.push(temp.chars().next().unwrap());
                temp.starts_with('$')
            } {
                /* redouble $ for act safety */
                buf.push('$');
            } else {
                buf.push(temp.chars().next().unwrap());
            }
        }

        temp = &temp[1..];
    }

    write_to_q(&buf, input_q, true);
}

/*
 * Given a character and a string, perform alias replacement on it.
 *
 * Return values:
 *   0: String was modified in place; call command_interpreter immediately.
 *   1: String was _not_ modified in place; rather, the expanded aliases
 *      have been placed at the front of the character's input queue.
 */
pub fn perform_alias(game: &mut Game, chars: &Depot<CharData>, d_id: DepotId, orig: &mut String) -> bool {
    let d = game.desc(d_id);
    /* Mobs don't have aliases. */
    let character = chars.get(d.character.unwrap());
    if character.is_npc() {
        return false;
    }
    /* bail out immediately if the guy doesn't have any aliases */
    if character.player_specials.aliases.len() == 0 {
        return false;
    }

    /* find the alias we're supposed to match */
    let mut first_arg = String::new();
    let ptr = any_one_arg(orig, &mut first_arg);

    /* bail out if it's null */
    if first_arg.is_empty() {
        return false;
    }
    let a;
    /* if the first arg is not an alias, return without doing anything */
    let dcps = &character.player_specials;
    if {
        a = find_alias(&dcps.aliases, &first_arg);
        a.is_none()
    } {
        return false;
    }
    let a = a.unwrap();
    if a.type_ == ALIAS_SIMPLE {
        orig.clear();
        orig.push_str(a.replacement.as_ref());
        return false;
    } else {
        let d = game.desc_mut(d_id);
        perform_complex_alias(&mut d.input, ptr, &a);
        return true;
    }
}

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
pub fn delete_doubledollar(text: &mut String) -> &String {
    *text = text.replace("$$", "$");
    text
}

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
    let mut argument = argument;
    loop {
        argument = argument.trim_start();
        first_arg.clear();

        let mut i = 0;
        for c in argument.chars() {
            if c.is_whitespace() {
                break;
            }
            first_arg.push(c.to_ascii_lowercase());
            i += 1;
        }

        argument = &argument[i..];
        if !fill_word(first_arg.as_str()) {
            break;
        }
    }

    argument
}

/*
 * one_word is like one_argument, except that words in quotes ("") are
 * considered one word.
 */
pub fn one_word<'a>(argument: &'a str, first_arg: &mut String) -> &'a str {
    let mut ret;
    loop {
        let mut argument = argument.trim_start();
        first_arg.clear();

        if argument.starts_with('\"') {
            argument = &argument[1..];

            while argument.len() != 0 && !argument.starts_with('\"') {
                first_arg.push(argument.chars().next().unwrap().to_ascii_lowercase());
                argument = &argument[1..];
            }
            argument = &argument[1..];
        } else {
            while argument.len() > 0 && !argument.chars().next().unwrap().is_whitespace() {
                first_arg.push(argument.chars().next().unwrap().to_ascii_lowercase());
                argument = &argument[1..];
            }
        }
        ret = argument;
        if !fill_word(first_arg.as_str()) {
            break;
        }
    }
    ret
}

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
pub fn find_command(command: &str) -> Option<usize> {
    CMD_INFO.iter().position(|e| e.command == command)
}

pub fn is_move(cmdnum: i32) -> bool {
    CMD_INFO[cmdnum as usize].command_pointer as usize == do_move as usize
}

pub fn special(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>,
    chid: DepotId,
    cmd: i32,
    arg: &str,
) -> bool {
    let ch = chars.get(chid);
    /* special in room? */
    if db.get_room_spec(ch.in_room()).is_some() {
        let f = db.get_room_spec(ch.in_room()).unwrap();
        if f(game, chars,db, texts,objs, chid, MeRef::None, cmd, arg) {
            return true;
        }
    }

    /* special in equipment list? */
    for j in 0..NUM_WEARS {
        let ch = chars.get(chid);
        if ch.get_eq(j).is_some() && db.get_obj_spec(objs.get(ch.get_eq(j).unwrap())).is_some() {
            let oid = ch.get_eq(j).unwrap();
            let obj = objs.get(oid);
            if db.get_obj_spec(obj).as_ref().unwrap()(
                game,
                chars,db,
                texts,objs,
                chid,
                MeRef::Obj(oid),
                cmd,
                arg,
            ) {
                return true;
            }
        }
    }

    /* special in inventory? */
    let ch = chars.get(chid);
    for i in ch.carrying.clone() {
        let obj = objs.get(i);
        if let Some(spec) = db.get_obj_spec(obj) {
            if spec(game, chars,db, texts,objs, chid, MeRef::Obj(i), cmd, arg) {
                return true;
            }
        }
    }

    /* special in mobile present? */
    let ch = chars.get(chid);
    for k_id in db.world[ch.in_room() as usize].peoples.clone() {
        let k = chars.get(k_id);
        if !k.mob_flagged(MOB_NOTDEADYET) {
            if db.get_mob_spec(k).is_some()
                && db.get_mob_spec(k).as_ref().unwrap()(
                    game,
                    chars,db,
                    texts,objs,
                    chid,
                    MeRef::Char(k_id),
                    cmd,
                    arg,
                )
            {
                return true;
            }
        }
    }
    let ch = chars.get(chid);
    for i in db.world[ch.in_room() as usize].contents.clone() {
        let obj = objs.get(i);
        if let Some(spec) = db.get_obj_spec(obj) {
            if spec(game, chars, db, texts,objs, chid, MeRef::Obj(i), cmd, arg) {
                return true;
            }
        }
    }
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
fn perform_dupe_check(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>,
    d_id: DepotId,
) -> bool {
    let mut target_id = None;
    let mut mode = 0;
    let id: i64;

    id = chars.get(game.desc(d_id).character.unwrap()).get_idnum();

    /*
     * Now that this descriptor has successfully logged in, disconnect all
     * other descriptors controlling a character with the same ID number.
     */
    for k_id in game.descriptor_list.clone() {
        if k_id == d_id {
            continue;
        }

        if game.desc(k_id).original.is_some()
            && chars.get(game.desc(k_id).original.unwrap()).get_idnum() == id
        {
            /* Original descriptor was switched, booting it and restoring normal body control. */
            game.desc_mut(d_id)
                .write_to_output("\r\nMultiple login detected -- disconnecting.\r\n");
            let k = game.desc_mut(k_id);
            k.set_state(ConClose);
            if target_id.is_none() {
                target_id = k.original;
                mode = UNSWITCH;
            }

            if k.character.is_some() {
                let id = k.character.unwrap();
                chars.get_mut(id).desc = None;
            }
            let k = game.desc_mut(k_id);
            k.character = None;
            k.original = None;
        } else if game.desc(k_id).character.is_some()
            && chars.get(game.desc(k_id).character.unwrap()).get_idnum() == id
            && game.desc(k_id).original.is_some()
        {
            /* Character taking over their own body, while an immortal was switched to it. */
            let chid = game.desc(k_id).character.unwrap();
            do_return(game, db, chars, texts, objs, chid, "", 0, 0);
        } else if game.desc(k_id).character.is_some()
            && chars.get(game.desc(k_id).character.unwrap()).get_idnum() == id
        {
            /* Character taking over their own body. */
            let k = game.desc_mut(k_id);
            if target_id.is_none() && k.state() == ConPlaying {
                k.write_to_output("\r\nThis body has been usurped!\r\n");
                target_id = Some(k.character.unwrap());
                mode = USURP;
            }

            chars.get_mut(k.character.unwrap()).desc = None;
            k.character = None;
            k.original = None;
            k.write_to_output("\r\nMultiple login detected -- disconnecting.\r\n");
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
    for &chid in &db.character_list.clone() {
        let ch = chars.get(chid);
        if ch.is_npc() {
            continue;
        }
        if ch.get_idnum() != id {
            continue;
        }

        /* ignore chars with descriptors (already handled by above step) */
        if ch.desc.is_some() {
            continue;
        }

        /* don't extract the target char we've found one already */
        if target_id.is_some() && chid == target_id.unwrap() {
            continue;
        }

        /* we don't already have a target and found a candidate for switching */
        if target_id.is_none() {
            target_id = Some(chid);
            mode = RECON;
            continue;
        }

        /* we've found a duplicate - blow him away, dumping his eq in limbo. */
        if ch.in_room != NOWHERE {
            db.char_from_room( objs,chars.get_mut(chid));
        }
        db.char_to_room(chars, objs,chid, 1);
        db.extract_char(chars, chid);
    }

    /* no target for switching into was found - allow login to continue */

    if target_id.is_none() {
        return false;
    }
    let target_id = target_id.unwrap();

    /* Okay, we've found a target.  Connect d to target. */
    let desc = game.desc_mut(d_id);
    let desc_char_id = desc.character.unwrap();
    db.free_char(&mut game.descriptors, chars, objs, desc_char_id);
    let desc = game.desc_mut(d_id);
    desc.character = Some(target_id);
    {
        let character_id = desc.character.unwrap();
        let character = chars.get_mut(character_id);
        character.desc = Some(d_id);
        desc.original = None;
        character.char_specials.timer = 0;
        character.remove_plr_flag(PLR_MAILING | PLR_WRITING);
        character.remove_aff_flags(AffectFlags::GROUP);
    }
    desc.set_state(ConPlaying);

    match mode {
        RECON => {
            desc.write_to_output("Reconnecting.\r\n");
            let chid = desc.character.unwrap();
            let ch = chars.get(chid);
            act(&mut game.descriptors, chars, 
                db,
                "$n has reconnected.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            let desc = game.desc_mut(d_id);
            let v2 = chars.get(desc.character.unwrap()).get_invis_lev() as i32;
            let msg = format!(
                "{} [{}] has reconnected.",
                chars.get(desc.character.unwrap()).get_name(),
                game.desc(d_id).host
            );
            game.mudlog(chars, NRM, max(LVL_IMMORT as i32, v2), true, msg.as_str());
        }
        USURP => {
            desc.write_to_output("You take over your own body, already in use!\r\n");
            let chid = desc.character.unwrap();
            let ch = chars.get(chid);
            act(&mut game.descriptors, chars, db, "$n suddenly keels over in pain, surrounded by a white aura...\r\n$n's body has been taken over by a new spirit!",
             true, Some(ch), None, None, TO_ROOM);
            let desc = game.desc_mut(d_id);
            let v2 = chars.get(desc.character.unwrap()).get_invis_lev() as i32;
            let msg = format!(
                "{} has re-logged in ... disconnecting old socket.",
                chars.get(desc.character.unwrap()).get_name()
            );

            game.mudlog(chars, NRM, max(LVL_IMMORT as i32, v2), true, msg.as_str());
        }
        UNSWITCH => {
            desc.write_to_output("Reconnecting to unswitched char.");
            let v2 = chars.get(desc.character.unwrap()).get_invis_lev() as i32;
            let msg = format!(
                "{} [{}] has reconnected.",
                chars.get(desc.character.unwrap()).get_name(),
                desc.host
            );
            game.mudlog(chars, NRM, max(LVL_IMMORT as i32, v2), true, msg.as_str());
        }
        _ => {}
    }
    return true;
}

/* deal with newcomers and other non-playing sockets */
pub fn nanny(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>,d_id: DepotId, arg: &str) {
    let arg = arg.trim();
    let desc = game.desc_mut(d_id);

    match desc.state() {
        ConGetName => {
            /* wait for input of name */
            if desc.character.is_none() {
                let mut ch = CharData::default();
                clear_char(&mut ch);
                ch.desc = Some(d_id);
                let chid = chars.push(ch);
                desc.character = Some(chid);
            }

            if arg.is_empty() {
                desc.set_state(ConClose);
            } else {
                let tmp_name = _parse_name(arg);

                if tmp_name.is_none()
                    || tmp_name.unwrap().len() < 2
                    || tmp_name.unwrap().len() > MAX_NAME_LENGTH
                    || !valid_name(game, chars, db, tmp_name.unwrap())
                    || fill_word(tmp_name.unwrap())
                    || reserved_word(tmp_name.unwrap())
                {
                    let desc = game.desc_mut(d_id);

                    desc.write_to_output("Invalid name, please try another.\r\nName: ");
                    return;
                }
                let desc = game.desc_mut(d_id);
                let character_id = desc.character.unwrap();
                let mut tmp_store = CharFileU::new();
                let player_i = db.load_char(tmp_name.unwrap(), &mut tmp_store);
                if player_i.is_some() {
                    let character = chars.get_mut(character_id);
                    store_to_char(texts, &tmp_store, character);
                    character.set_pfilepos(player_i.unwrap() as i32);

                    if character.prf_flagged(PLR_DELETED) {
                        /* We get a false positive from the original deleted character. */
                        desc.character = None;
                        db.free_char(&mut game.descriptors, chars, objs, character_id);
                        /* Check for multiple creations... */
                        if !valid_name(game, chars, db, tmp_name.unwrap()) {
                            let desc = game.desc_mut(d_id);

                            desc.write_to_output("Invalid name, please try another.\r\nName: ");
                            return;
                        }
                        let desc = game.desc_mut(d_id);

                        let mut new_char = CharData::default();
                        clear_char(&mut new_char);
                        new_char.desc = Some(d_id);
                        new_char.player.name = Rc::from(tmp_name.unwrap());
                        new_char.pfilepos = player_i.unwrap() as i32;
                        let new_char_id = chars.push(new_char);
                        desc.character = Some(new_char_id);
                        desc.write_to_output(
                            format!("Did I get that right, {} (Y/N)? ", tmp_name.unwrap()).as_str(),
                        );
                        desc.set_state(ConNameCnfrm);
                    } else {
                        /* undo it just in case they are set */
                        character.remove_plr_flag(PLR_WRITING | PLR_MAILING | PLR_CRYO);
                        character.remove_aff_flags(AffectFlags::GROUP);
                        desc.write_to_output("Password: ");
                        desc.echo_off();
                        desc.idle_tics = 0;
                        desc.set_state(ConPassword);
                    }
                } else {
                    /* player unknown -- make new character */

                    /* Check for multiple creations of a character. */
                    if !valid_name(game, chars, db, tmp_name.unwrap()) {
                        let desc = game.desc_mut(d_id);
                        desc.write_to_output("Invalid name, please try another.\r\nName: ");
                        return;
                    }
                    let desc = game.desc_mut(d_id);
                    let character_id = desc.character.unwrap();
                    let character = chars.get_mut(character_id);
                    character.player.name = Rc::from(tmp_name.unwrap());

                    desc.write_to_output(
                        format!("Did I get that right, {} (Y/N)? ", tmp_name.unwrap()).as_str(),
                    );
                    desc.set_state(ConNameCnfrm);
                }
            }
        }
        ConNameCnfrm => {
            /* wait for conf. of new name    */
            if arg.to_uppercase().starts_with('Y') {
                if isbanned(&db, &desc.host) >= BanType::New {
                    let msg = format!(
                        "Request for new char {} denied from [{}] (siteban)",
                        chars.get(desc.character.unwrap()).get_pc_name(),
                        desc.host
                    );
                    game.mudlog(chars, NRM, LVL_GOD as i32, true, msg.as_str());
                    let desc = game.desc_mut(d_id);

                    desc.write_to_output(
                        "Sorry, new characters are not allowed from your site!\r\n",
                    );
                    desc.set_state(ConClose);
                    return;
                }
                if db.circle_restrict != 0 {
                    desc.write_to_output("Sorry, new players can't be created at the moment.\r\n");
                    let msg = format!(
                        "Request for new char {} denied from [{}] (wizlock)",
                        chars.get(desc.character.unwrap()).get_pc_name(),
                        desc.host
                    );
                    game.mudlog(chars, NRM, LVL_GOD as i32, true, msg.as_str());
                    let desc = game.desc_mut(d_id);

                    desc.set_state(ConClose);
                    return;
                }

                let msg = format!(
                    "New character.\r\nGive me a password for {}: ",
                    chars.get(desc.character.unwrap()).get_pc_name()
                );
                desc.write_to_output(msg.as_str());
                desc.echo_off();
                desc.set_state(ConNewpasswd);
            } else if arg.starts_with('n') || arg.starts_with('N') {
                desc.write_to_output("Okay, what IS it, then? ");
                desc.set_state(ConGetName);
                let chid = desc.character.unwrap();
                db.free_char(&mut game.descriptors, chars, objs, chid );
            } else {
                desc.write_to_output("Please type Yes or No: ");
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

            desc.echo_on(); /* turn echo back on */

            /* New echo_on() eats the return on telnet. Extra space better than none. */
            desc.write_to_output("\r\n");
            let load_result: i32;

            if arg.is_empty() {
                desc.set_state(ConClose);
            } else {
                let matching_pwd: bool;
                {
                    let character_id = desc.character.unwrap();
                    let character = chars.get(character_id);
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
                        let msg = format!("Bad PW: {} [{}]", character.get_name(), desc.host);
                        game.mudlog(chars, BRF, LVL_GOD as i32, true, msg.as_str());
                        let character = chars.get_mut(character_id);
                        character.incr_bad_pws();
                        let desc = game.desc_mut(d_id);

                        let chid = desc.character.unwrap();
                        save_char(&mut game.descriptors, db, chars, texts, objs, chid);
                        let desc = game.desc_mut(d_id);

                        desc.bad_pws += 1;
                        if desc.bad_pws >= MAX_BAD_PWS {
                            /* 3 strikes and you're out. */
                            desc.write_to_output("Wrong password... disconnecting.\r\n");
                            desc.set_state(ConClose);
                        } else {
                            desc.write_to_output("Wrong password.\r\nPassword: ");
                            desc.echo_off();
                        }
                        return;
                    }

                    /* Password was correct. */
                    load_result = character.get_bad_pws() as i32;
                    let character = chars.get_mut(character_id);
                    character.reset_bad_pws();
                    desc.bad_pws = 0;
                    if isbanned(&db, &desc.host) == BanType::Select
                        && !chars.get(desc.character.unwrap()).plr_flagged(PLR_SITEOK)
                    {
                        desc.write_to_output(
                            "Sorry, this char has not been cleared for login from your site!\r\n",
                        );
                        desc.set_state(ConClose);
                        let msg = format!(
                            "Connection attempt for {} denied from {}",
                            chars.get(desc.character.unwrap()).get_name(),
                            desc.host
                        );
                        game.mudlog(chars, NRM, LVL_GOD as i32, true, msg.as_str());
                        return;
                    }
                    let desc = game.desc_mut(d_id);

                    if chars.get(desc.character.unwrap()).get_level() < db.circle_restrict {
                        desc.write_to_output(
                            "The game is temporarily restricted.. try again later.\r\n",
                        );
                        desc.set_state(ConClose);
                        let msg = format!(
                            "Request for login denied for {} [{}] (wizlock)",
                            chars.get(desc.character.unwrap()).get_name(),
                            desc.host
                        );
                        game.mudlog(chars, NRM, LVL_GOD as i32, true, msg.as_str());
                        return;
                    }
                }
                /* check and make sure no other copies of this player are logged in */
                if perform_dupe_check(game, db, chars, texts, objs, d_id) {
                    return;
                }
                let desc = game.desc_mut(d_id);

                let character_id = desc.character.unwrap();
                let character = chars.get(character_id);

                let level: u8;
                {
                    level = character.get_level();
                }
                if level >= LVL_IMMORT as u8 {
                    desc.write_to_output(db.imotd.as_ref());
                } else {
                    desc.write_to_output(db.motd.as_ref());
                }
                let character = chars.get(character_id);
                {
                    let msg = format!("{} [{}] has connected.", character.get_name(), desc.host);
                    let character = chars.get(character_id);
                    game.mudlog(chars,
                        BRF,
                        max(LVL_IMMORT as i32, character.get_invis_lev() as i32),
                        true,
                        msg.as_str(),
                    );
                }
                let desc = game.desc_mut(d_id);

                let character = chars.get(character_id);
                if load_result != 0 {
                    let color1: &str;
                    let color2: &str;
                    {
                        color1 = CCRED!(character, C_SPR);
                        color2 = CCNRM!(character, C_SPR);
                    }
                    desc.write_to_output(
                        format!("\r\n\r\n\007\007\007{}{} LOGIN FAILURE{} SINCE LAST SUCCESSFUL LOGIN.{}\r\n",
                                color1, load_result, if load_result > 1 { "S" } else { "" }, color2).as_str(),
                    );
                    let character = chars.get(character_id);
                    character.get_bad_pws();
                }
                desc.write_to_output("\r\n*** PRESS RETURN: ");
                desc.set_state(ConRmotd);
            }
        }
        ConNewpasswd | ConChpwdGetnew => {
            let character_id = desc.character.unwrap();
            let character = chars.get(character_id);
            if arg.is_empty()
                || arg.len() > MAX_PWD_LENGTH
                || arg.len() < 3
                || arg == character.get_pc_name().as_ref()
            {
                desc.write_to_output("\r\nIllegal password.\r\nPassword: ");
                return;
            }
            {
                let salt = character.get_pc_name().to_string();
                let mut tmp = [0; 16];
                pbkdf2::pbkdf2::<Hmac<Sha256>>(arg.as_bytes(), salt.as_bytes(), 4, &mut tmp)
                    .expect("Error while encrypting new password");
                let character = chars.get_mut(character_id);
                character.set_passwd(tmp);
            }
            desc.write_to_output("\r\nPlease retype password: ");
            if desc.state() == ConNewpasswd {
                desc.set_state(ConCnfpasswd);
            } else {
                desc.set_state(ConChpwdVrfy);
            }
        }
        ConCnfpasswd | ConChpwdVrfy => {
            let character_id = desc.character.unwrap();
            let character = chars.get(character_id);
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
                desc.write_to_output("\r\nPasswords don't match... start over.\r\nPassword: ");
                if desc.state() == ConCnfpasswd {
                    desc.set_state(ConNewpasswd);
                } else {
                    desc.set_state(ConChpwdGetnew);
                }
            }
            desc.echo_on();

            if desc.state() == ConCnfpasswd {
                desc.write_to_output("\r\nWhat is your sex (M/F)? ");
                desc.set_state(ConQsex);
            } else {
                desc.write_to_output(format!("\r\nDone.\r\n{}", MENU).as_str());
                desc.set_state(ConMenu);
            }
        }
        ConQsex => {
            let character_id = desc.character.unwrap();
            let character = chars.get_mut(character_id);
            /* query sex of new user         */
            match arg.chars().next().unwrap() {
                'm' | 'M' => {
                    character.player.sex = SEX_MALE;
                }
                'f' | 'F' => {
                    character.player.sex = SEX_FEMALE;
                }
                _ => {
                    desc.write_to_output("That is not a sex..\r\nWhat IS your sex? ");
                    return;
                }
            }

            desc.write_to_output(format!("{}\r\nClass: ", CLASS_MENU).as_str());
            desc.set_state(ConQclass);
        }
        ConQclass => {
            let character_id = desc.character.unwrap();

            let load_result = parse_class(arg.chars().next().unwrap());
            if load_result == CLASS_UNDEFINED {
                desc.write_to_output("\r\nThat's not a class.\r\nClass: ");
                return;
            } else {
                let character = chars.get_mut(character_id);
                character.set_class(load_result);
            }
            let character = chars.get(character_id);

            if character.get_pfilepos() < 0 {
                let name = character.get_pc_name().clone();
                let val = db.create_entry(name.as_ref());
                let character = chars.get_mut(character_id);
                character.set_pfilepos(val as i32);
            }

            /* Now GET_NAME() will work properly. */
            db.init_char(chars, texts, character_id);
            save_char(&mut game.descriptors, db, chars, texts, objs, character_id);
            let desc = game.desc_mut(d_id);

            desc.write_to_output(format!("{}\r\n*** PRESS RETURN: ", db.motd).as_str());
            desc.set_state(ConRmotd);

            {
                let character = chars.get(character_id);
                let msg = format!("{} [{}] new player.", character.get_pc_name(), desc.host);
                game.mudlog(chars, NRM, LVL_IMMORT as i32, true, msg.as_str());
            }
        }
        ConRmotd => {
            /* read CR after printing motd   */
            desc.write_to_output(MENU);
            desc.set_state(ConMenu);
        }
        ConMenu => {
            let load_result;
            /* get selection from main menu  */
            let character_id = desc.character.unwrap();
            let character = chars.get(character_id);
            match if arg.chars().last().is_some() {
                arg.chars().last().unwrap()
            } else {
                '\0'
            } {
                '0' => {
                    desc.write_to_output("Goodbye.\r\n");
                    desc.set_state(ConClose);
                }

                '1' => {
                    {
                        reset_char(chars.get_mut(character_id));
                        let character = chars.get_mut(character_id);
                        read_aliases(character);
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
                        let character = chars.get(character_id);
                        if load_room == NOWHERE {
                            if character.get_level() >= LVL_IMMORT as u8 {
                                load_room = db.r_immort_start_room;
                            } else {
                                load_room = db.r_mortal_start_room;
                            }
                        }
                        let character = chars.get(character_id);
                        if character.plr_flagged(PLR_FROZEN) {
                            load_room = db.r_frozen_start_room;
                        }

                        send_to_char(&mut game.descriptors, character, format!("{}", WELC_MESSG).as_str());
                        db.character_list.push(character.id());
                        db.char_to_room(chars, objs, character_id, load_room);
                        load_result = crash_load(game, chars, db, texts, objs,character_id);

                        /* Clear their load room if it's not persistant. */
                        let character = chars.get_mut(character_id);
                        if !character.plr_flagged(PLR_LOADROOM) {
                            character.set_loadroom(NOWHERE);
                        }
                        save_char(&mut game.descriptors, db, chars, texts, objs, character_id);
                        let character = chars.get(character_id);
                        act(&mut game.descriptors, chars, 
                            db,
                            "$n has entered the game.",
                            true,
                            Some(character),
                            None,
                            None,
                            TO_ROOM as i32,
                        );
                    }
                    let desc = game.desc_mut(d_id);
                    desc.set_state(ConPlaying);
                    let character = chars.get(character_id);
                    if character.get_level() == 0 {
                        do_start(game, chars, db, texts, objs, character_id);
                        let character = chars.get(character_id);
                        send_to_char(&mut game.descriptors, character, format!("{}", START_MESSG).as_str());
                        look_at_room(&mut game.descriptors, db, chars, texts, objs,character, false);
                    }
                    let desc = game.desc_mut(d_id);
                    if db
                        .mails
                        .has_mail(chars.get(desc.character.unwrap()).get_idnum())
                    {
                        let chid = desc.character.unwrap();
                        let ch = chars.get(chid);
                        send_to_char(&mut game.descriptors, ch, "You have mail waiting.\r\n");
                    }
                    let desc = game.desc_mut(d_id);
                    if load_result == 2 {
                        /* rented items lost */
                        let chid = desc.character.unwrap();
                        let ch = chars.get(chid);
                        send_to_char(&mut game.descriptors, ch, "\r\n\007You could not afford your rent!\r\nYour possesions have been donated to the Salvation Army!\r\n");
                    }
                    let desc = game.desc_mut(d_id);
                    desc.has_prompt = false;
                }

                '2' => {
                    let text = &mut texts.get_mut(character.player.description).text;
                    if text.is_empty() {
                        let mesg = format!("Old description:\r\n{}", text);
                        desc.write_to_output(&mesg);
                        text.clear();
                    }
                    desc.write_to_output( "Enter the new text you'd like others to see when they look at you.\r\nTerminate with a '@' on a new line.\r\n");
                    desc.str = Some(character.player.description);
                    desc.max_str = EXDSCR_LENGTH;
                    desc.set_state(ConExdesc);
                }
                '3' => {
                    let msg = db.background.as_ref();
                    page_string(&mut game.descriptors, chars,  d_id, msg, false);
                    let desc = game.desc_mut(d_id);
                    desc.set_state(ConRmotd);
                }
                '4' => {
                    desc.write_to_output("\r\nEnter your old password: ");
                    desc.echo_off();
                    desc.set_state(ConChpwdGetold);
                }
                '5' => {
                    desc.write_to_output("\r\nEnter your password for verification: ");
                    desc.echo_off();
                    desc.set_state(ConDelcnf1);
                }
                _ => {
                    desc.write_to_output(
                        format!("\r\nThat's not a menu choice!\r\n{}", MENU).as_str(),
                    );
                }
            }
        }

        ConChpwdGetold => {
            let matching_pwd: bool;
            {
                let character_id = desc.character.unwrap();
                let character = chars.get(character_id);
                let mut passwd2 = [0 as u8; 16];
                let salt = character.get_pc_name();
                let passwd = character.get_passwd();
                pbkdf2::pbkdf2::<Hmac<Sha256>>(arg.as_bytes(), salt.as_bytes(), 4, &mut passwd2)
                    .expect("Error while encrypting password");
                matching_pwd = passwd == passwd2;
            }

            if !matching_pwd {
                desc.echo_on();
                desc.write_to_output(format!("\r\nIncorrect password.\r\n{}", MENU).as_str());
                desc.set_state(ConMenu);
            } else {
                desc.write_to_output("\r\nEnter a new password: ");
                desc.set_state(ConChpwdGetnew);
            }
            return;
        }

        ConDelcnf1 => {
            desc.echo_on();
            let matching_pwd: bool;
            {
                let character_id = desc.character.unwrap();
                let character = chars.get(character_id);
                let mut passwd2 = [0 as u8; 16];
                let salt = character.get_pc_name();
                let passwd = character.get_passwd();
                pbkdf2::pbkdf2::<Hmac<Sha256>>(arg.as_bytes(), salt.as_bytes(), 4, &mut passwd2)
                    .expect("Error while encrypting password");
                matching_pwd = passwd == passwd2;
            }
            if !matching_pwd {
                desc.write_to_output(format!("\r\nIncorrect password.\r\n{}", MENU).as_str());
                desc.set_state(ConMenu);
            } else {
                desc.write_to_output(
                    "\r\nYOU ARE ABOUT TO DELETE THIS CHARACTER PERMANENTLY.\r\n\
                                ARE YOU ABSOLUTELY SURE?\r\n\r\n\
                                Please type \"yes\" to confirm: ",
                );
                desc.set_state(ConDelcnf2);
            }
        }

        ConDelcnf2 => {
            if arg == "yes" || arg == "YES" {
                let d_chid = desc.character.unwrap();
                let d_ch = chars.get(d_chid);
                if d_ch.plr_flagged(PLR_FROZEN) {
                    desc.write_to_output(
                        "You try to kill yourself, but the ice stops you.\r\n\
                                    Character not deleted.\r\n\r\n",
                    );
                    desc.set_state(ConClose);
                    return;
                }
                let d_ch = chars.get_mut(d_chid);
                if d_ch.get_level() < LVL_GRGOD as u8 {
                    d_ch.set_plr_flag_bit(PLR_DELETED);
                }
                save_char(&mut game.descriptors, db, chars, texts, objs, d_chid);
                let desc = game.desc_mut(d_id);
                let d_ch = chars.get(d_chid);
                crash_delete_file(&d_ch.get_name());
                delete_aliases(d_ch.get_name().as_ref());
                let txt = format!(
                    "Character '{}' deleted!\r\n\
                            Goodbye.\r\n",
                    d_ch.get_name()
                );
                desc.write_to_output(txt.as_str());
                let d_ch = chars.get(d_chid);
                let txt = format!(
                    "{} (lev {}) has self-deleted.",
                    d_ch.get_name(),
                    d_ch.get_level()
                );
                game.mudlog(chars, NRM, LVL_GOD as i32, true, txt.as_str());
                let desc = game.desc_mut(d_id);
                desc.set_state(ConClose);
                return;
            } else {
                desc.write_to_output(format!("\r\nCharacter not deleted.\r\n{}", MENU).as_str());
                desc.set_state(ConMenu);
            }
        }

        /*
         * It's possible, if enough pulses are missed, to kick someone off
         * while they are at the password prompt. We'll just defer to let
         * the game_loop() axe them.
         */
        ConClose => {}

        _ => {
            let char_name;
            if desc.character.is_some() {
                let character_id = desc.character.unwrap();
                let character = chars.get(character_id);
                char_name = String::from(character.get_name().as_ref());
            } else {
                char_name = "<unknown>".to_string();
            }
            error!(
                "SYSERR: Nanny: illegal state of con'ness ({:?}) for '{}'; closing connection.",
                desc.state(),
                char_name
            );
            desc.set_state(ConDisconnect); /* Safest to do. */
        }
    }
}
