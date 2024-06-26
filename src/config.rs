/* ************************************************************************
*   File: config.rs                                     Part of CircleMUD *
*  Usage: Configuration of various aspects of CircleMUD operation         *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

/*
 * Below are several constants which you can change to alter certain aspects
 * of the way CircleMUD acts.  Since this is a .c file, all you have to do
 * to change one of the constants (assuming you keep your object files around)
 * is change the constant in this file and type 'make'.  Make will recompile
 * this file and relink; you don't have to wait for the whole thing to
 * recompile as you do if you change a header file.
 *
 * I realize that it would be slightly more efficient to have lots of
 * #defines strewn about, so that, for example, the autowiz code isn't
 * compiled at all if you don't want to use autowiz.  However, the actual
 * code for the various options is quite small, as is the computational time
 * in checking the option you've selected at run-time, so I've decided the
 * convenience of having all your options in this one file outweighs the
 * efficency of doing it the other way.
 *
 */

/****************************************************************************/
/****************************************************************************/

/* GAME PLAY OPTIONS */
/*
* pk_allowed sets the tone of the entire game.  If pk_allowed is set to
* NO, then players will not be allowed to kill, summon, charm, or sleep
* other players, as well as a variety of other "asshole player" protections.
* However, if you decide you want to have an all-out knock-down drag-out
* PK Mud, just set pk_allowed to YES - and anything goes.
*/
use crate::structs::{RoomRnum, LVL_GOD};

pub const PK_ALLOWED: bool = false;

/* is playerthieving allowed? */
pub const PT_ALLOWED: bool = false;

/* minimum level a player must be to shout/holler/gossip/auction */
pub const LEVEL_CAN_SHOUT: i32 = 1;

/* number of movement points it costs to holler */
pub const HOLLER_MOVE_COST: i32 = 20;

/*  how many people can get into a tunnel?  The default is two, but there
*  is also an alternate message in the case of one person being allowed.
*/
pub const TUNNEL_SIZE: i32 = 2;

/* exp change limits */
pub const MAX_EXP_GAIN: i32 = 100000; /* max gainable per kill */
pub const MAX_EXP_LOSS: i32 = 500000; /* max losable per death */
/* number of tics (usually 75 seconds) before PC/NPC corpses decompose */
pub const MAX_NPC_CORPSE_TIME: i32 = 5;
pub const MAX_PC_CORPSE_TIME: i32 = 10;

/* How many ticks before a player is sent to the void or idle-rented. */
pub const IDLE_VOID: i32 = 8;
pub const IDLE_RENT_TIME: i32 = 48;

/* This level and up is immune to idling, LVL_IMPL+1 will disable it. */
pub const IDLE_MAX_LEVEL: i16 = LVL_GOD;

/* should items in death traps automatically be junked? */
pub const DTS_ARE_DUMPS: bool = true;

/*
* Whether you want items that immortals load to appear on the ground or not.
* It is most likely best to set this to 'YES' so that something else doesn't
* grab the item before the immortal does, but that also means people will be
* able to carry around things like boards.  That's not necessarily a bad
* thing, but this will be left at a default of 'NO' for historic reasons.
*/
pub const LOAD_INTO_INVENTORY: bool = false;

/* "okay" etc. */
pub const OK: &str = "Okay.\r\n";
pub const NOPERSON: &str = "No-one by that name here.\r\n";
pub const NOEFFECT: &str = "Nothing seems to happen.\r\n";

/*
* If you want mortals to level up to immortal once they have enough
* experience, then set this to 0.  This is the stock behaviour for
* CircleMUD because it was the stock DikuMud behaviour.  Subtracting
* this from LVL_IMMORT gives the top level that people can advance to
* in gain_exp() in limits.c
* For example, to stop people from advancing to LVL_IMMORT, simply set
* IMMORT_LEVEL_OK to 1.
*/
pub const IMMORT_LEVEL_OK: i16 = 0;

/****************************************************************************/
/****************************************************************************/

/* RENT/CRASHSAVE OPTIONS */

/*
* Should the MUD allow you to 'rent' for free?  (i.e. if you just quit,
* your objects are saved at no cost, as in Merc-type MUDs.)
*/
pub const FREE_RENT: bool = true;

/* maximum number of items players are allowed to rent */
pub const MAX_OBJ_SAVE: i32 = 30;

/* receptionist's surcharge on top of item costs */
pub const MIN_RENT_COST: i32 = 100;

/*
* Should the game automatically save people?  (i.e., save player data
* every 4 kills (on average), and Crash-save as defined below.  This
* option has an added meaning past bpl13.  If AUTO_SAVE is YES, then
* the 'save' command will be disabled to prevent item duplication via
* game crashes.
*/
pub const AUTO_SAVE: bool = true;

/*
* if AUTO_SAVE (above) is yes, how often (in minutes) should the MUD
* Crash-save people's objects?   Also, this number indicates how often
* the MUD will Crash-save players' houses.
*/
pub const AUTOSAVE_TIME: i32 = 5;

/* Lifetime of crashfiles and forced-rent (idlesave) files in days */
pub const CRASH_FILE_TIMEOUT: i32 = 10;

/* Lifetime of normal rent files in days */
pub const RENT_FILE_TIMEOUT: i32 = 30;

/****************************************************************************/
/****************************************************************************/

/* ROOM NUMBERS */

/* virtual number of room that mortals should enter at */
pub const MORTAL_START_ROOM: RoomRnum = 3001;

/* virtual number of room that immorts should enter at by default */
pub const IMMORT_START_ROOM: RoomRnum = 1204;

/* virtual number of room that frozen players should enter at */
pub const FROZEN_START_ROOM: RoomRnum = 1202;

/*
* virtual numbers of donation rooms.  note: you must change code in
* do_drop of act.item.c if you change the number of non-NOWHERE
* donation rooms.
*/
pub const DONATION_ROOM_1: RoomRnum = 3063;

/****************************************************************************/
/****************************************************************************/

/* GAME OPERATION OPTIONS */

/*
* This is the default port on which the game should run if no port is
* given on the command-line.  NOTE WELL: If you're using the
* 'autorun' script, the port number there will override this setting.
* Change the PORT= line in autorun instead of (or in addition to)
* changing this.
*/
pub const DFLT_PORT: u16 = 4000;

/*
* IP address to which the MUD should bind.  This is only useful if
* you're running Circle on a host that host more than one IP interface,
* and you only want to bind to *one* of them instead of all of them.
* Setting this to NULL (the default) causes Circle to bind to all
* interfaces on the host.  Otherwise, specify a numeric IP address in
* dotted quad format, and Circle will only bind to that IP address.  (Of
* course, that IP address must be one of your host's interfaces, or it
* won't work.)
*/
pub const DFLT_IP: Option<&str> = None; /* bind to all interfaces */
/* pub const DFLT_IP :Option<&str> = Some("192.168.1.1");  -- bind only to one interface */

/* default directory to use as data directory */
pub const DFLT_DIR: &str = "lib";

/*
* What file to log messages to (ex: "log/syslog").  Setting this to NULL
* means you want to log to stderr, which was the default in earlier
* versions of Circle.  If you specify a file, you don't get messages to
* the screen. (Hint: Try 'tail -f' if you have a UNIX machine.)
*/
pub const LOGNAME: Option<&str> = None;
/* const char *LOGNAME = "log/syslog";  -- useful for Windows users */

/* maximum number of players allowed before game starts to turn people away */
pub const MAX_PLAYING: i32 = 300;

/* maximum size of bug, typo and idea files in bytes (to prevent bombing) */
pub const MAX_FILESIZE: i32 = 50000;

/* maximum number of password attempts before disconnection */
pub const MAX_BAD_PWS: u8 = 3;

/*
* Rationale for enabling this, as explained by naved@bird.taponline.com.
*
* Usually, when you select ban a site, it is because one or two people are
* causing troubles while there are still many people from that site who you
* want to still log on.  Right now if I want to add a new select ban, I need
* to first add the ban, then SITEOK all the players from that site except for
* the one or two who I don't want logging on.  Wouldn't it be more convenient
* to just have to remove the SITEOK flags from those people I want to ban
* rather than what is currently done?
*/
pub const SITEOK_EVERYONE: bool = true;

pub struct Config {
    /*
     * Some nameservers are very slow and cause the game to lag terribly every
     * time someone logs in.  The lag is caused by the gethostbyaddr() function
     * which is responsible for resolving numeric IP addresses to alphabetic names.
     * Sometimes, nameservers can be so slow that the incredible lag caused by
     * gethostbyaddr() isn't worth the luxury of having names instead of numbers
     * for players' sitenames.
     *
     * If your nameserver is fast, set the variable below to NO.  If your
     * nameserver is slow, of it you would simply prefer to have numbers
     * instead of names for some other reason, set the variable to YES.
     *
     * You can experiment with the setting of NAMESERVER_IS_SLOW on-line using
     * the SLOWNS command from within the MUD.
     */
    pub nameserver_is_slow: bool,

    /*
     * You can define or not define TRACK_THOUGH_DOORS, depending on whether
     * or not you want track to find paths which lead through closed or
     * hidden doors. A setting of 'NO' means to not go through the doors
     * while 'YES' will pass through doors to find the target.
     */
    pub track_through_doors: bool,
}

pub const MENU: &str = "
Welcome to CircleMUD!
0) Exit from CircleMUD.
1) Enter the game.
2) Enter description.
3) Read the background story.
4) Change password.
5) Delete this character.

   Make your choice: ";

pub const WELC_MESSG: &str = "
Welcome to the land of CircleMUD!  May your visit here be... Interesting.
\r\n";

pub const START_MESSG: &str =
    "Welcome.  This is your new CircleMUD character!  You can now earn gold,
gain experience, find weapons and equipment, and much more -- while
meeting people from around the world!\r\n";

/****************************************************************************/
/****************************************************************************/

/* AUTOWIZ OPTIONS */

/*
* Should the game automatically create a new wizlist/immlist every time
* someone immorts, or is promoted to a higher (or lower) god level?
* NOTE: this only works under UNIX systems.
*/
// int use_autowiz = YES;

/* If yes, what is the lowest level which should be on the wizlist?  (All
immort levels below the level you specify will go on the immlist instead.) */
// int min_wizlist_lev = LVL_GOD;
