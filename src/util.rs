/* ************************************************************************
*   File: utils.c                                       Part of CircleMUD *
*  Usage: various internal functions of a utility nature                  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */
#[macro_export]
macro_rules! is_set {
    ($flag:expr, $bit:expr) => {
        (($flag & $bit) != 0)
    };
}

#[macro_export]
macro_rules! mob_flags {
    ($ch:expr) => {
        ($ch.char_specials.saved.act)
    };
}

#[macro_export]
macro_rules! is_npc {
    ($ch:expr) => {{
        (is_set!(mob_flags!($ch), MOB_ISNPC))
    }};
}

#[macro_export]
macro_rules! prf_flagged {
    ($ch:expr,$flag:expr) => {
        (is_set!(prf_flags!($ch), ($flag)))
    };
}

#[macro_export]
macro_rules! prf_flags {
    ($ch:expr) => {
        (check_player_special!(($ch), ($ch).player_specials.saved.pref))
    };
}

/* TODO:
 * Accessing player specific data structures on a mobile is a very bad thing
 * to do.  Consider that changing these variables for a single mob will change
 * it for every other single mob in the game.  If we didn't specifically check
 * for it, 'wimpy' would be an extremely bad thing for a mob to do, as an
 * example.  If you really couldn't care less, change this to a '#if 0'.
 */
#[macro_export]
macro_rules! check_player_special {
    ($ch:expr,$var:expr) => {
        ($var)
    };
}

#[macro_export]
macro_rules! get_invis_lev {
    ($ch:expr) => {
        (check_player_special!(($ch), ($ch).player_specials.saved.invis_level))
    };
}

#[macro_export]
macro_rules! get_hit {
    ($ch:expr) => {
        (($ch).points.hit)
    };
}

#[macro_export]
macro_rules! get_mana {
    ($ch:expr) => {
        (($ch).points.mana)
    };
}

#[macro_export]
macro_rules! get_move {
    ($ch:expr) => {
        (($ch).points.movem)
    };
}

#[macro_export]
macro_rules! get_pc_name {
    ($ch:expr) => {
        (($ch).player.name)
    };
}

#[macro_export]
macro_rules! get_name {
    ($ch:expr) => {
        (if is_npc!($ch) {
            ($ch).player.short_descr
        } else {
            get_pc_name!($ch)
        })
    };
}

#[macro_export]
macro_rules! isnewl {
    ($ch:expr) => {
        ((ch) == '\n' as u8 || (ch) == '\r' as u8))
    }
}

/* external globals */
// extern struct time_data time_info;

/* local functions */
// struct time_info_data *real_time_passed(time_t t2, time_t t1);
// struct time_info_data *mud_time_passed(time_t t2, time_t t1);
// void prune_crlf(char *txt);

/* creates a random number in interval [from;to] */
// int rand_number(int from, int to)
// {
// /* error checking in case people call this incorrectly */
// if (from > to) {
// int tmp = from;
// from = to;
// to = tmp;
// log("SYSERR: rand_number() should be called with lowest, then highest. (%d, %d), not (%d, %d).", from, to, to, from);
// }
//
// /*
//  * This should always be of the form:
//  *
//  *	((float)(to - from + 1) * rand() / (float)(RAND_MAX + from) + from);
//  *
//  * if you are using rand() due to historical non-randomness of the
//  * lower bits in older implementations.  We always use circle_random()
//  * though, which shouldn't have that problem. Mean and standard
//  * deviation of both are identical (within the realm of statistical
//  * identity) if the rand() implementation is non-broken.
//  */
// return ((circle_random() % (to - from + 1)) + from);
// }

/* simulates dice roll */
// int dice(int num, int size)
// {
// int sum = 0;
//
// if (size <= 0 || num <= 0)
// return (0);
//
// while (num-- > 0)
// sum += rand_number(1, size);
//
// return (sum);
// }

/* Be wary of sign issues with this. */
// int MIN(int a, int b)
// {
// return (a < b ? a : b);
// }

/* Be wary of sign issues with this. */
// int MAX(int a, int b)
// {
// return (a > b ? a : b);
// }

// char *CAP(char *txt)
// {
// *txt = UPPER(*txt);
// return (txt);
// }

/*
 * Strips \r\n from end of string.
 */
pub fn prune_crlf(s: &mut String) {
    while s.ends_with('\n') || s.ends_with('\r') {
        s.pop();
    }
}

/* log a death trap hit */
// void log_death_trap(struct char_data *ch)
// {
// mudlog(BRF, LVL_IMMORT, TRUE, "%s hit death trap #%d (%s)", GET_NAME(ch), GET_ROOM_VNUM(IN_ROOM(ch)), world[IN_ROOM(ch)].name);
// }

// #[macro_export]
// macro_rules! foo {
//     ($base: expr, $($args:tt),*) => {{
//         Err(MyError::new(format!($base, $($args),*)))
//     }};
// }

/*
 * New variable argument log() function.  Works the same as the old for
 * previously written code but is very nice for new code.
 */
use std::fs::OpenOptions;
use std::io;
use std::path::Path;

/* So mudlog() can use the same function. */
// void basic_mud_log(const char *format, ...)
// {
// va_list args;
//
// va_start(args, format);
// basic_mud_vlog(format, args);
// va_end(args);
// }

/* the "touch" command, essentially. */
pub fn touch(path: &Path) -> io::Result<()> {
    match OpenOptions::new().create(true).write(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/*
 * mudlog -- log mud messages to a file & to online imm's syslogs
 * based on syslog by Fen Jul 3, 1992
 */
// void mudlog(int type, int level, int file, const char *str, ...)
// {
// char buf[MAX_STRING_LENGTH];
// struct descriptor_data *i;
// va_list args;
//
// if (str == NULL)
// return;	/* eh, oh well. */
//
// if (file) {
// va_start(args, str);
// basic_mud_vlog(str, args);
// va_end(args);
// }
//
// if (level < 0)
// return;
//
// strcpy(buf, "[ ");	/* strcpy: OK */
// va_start(args, str);
// vsnprintf(buf + 2, sizeof(buf) - 6, str, args);
// va_end(args);
// strcat(buf, " ]\r\n");	/* strcat: OK */
//
// for (i = descriptor_list; i; i = i->next) {
// if (STATE(i) != ConPlaying || IS_NPC(i->character)) /* switch */
// continue;
// if (GET_LEVEL(i->character) < level)
// continue;
// if (PLR_FLAGGED(i->character, PLR_WRITING))
// continue;
// if (type > (PRF_FLAGGED(i->character, PRF_LOG1) ? 1 : 0) + (PRF_FLAGGED(i->character, PRF_LOG2) ? 2 : 0))
// continue;
//
// send_to_char(i->character, "%s%s%s", CCGRN(i->character, C_NRM), buf, CCNRM(i->character, C_NRM));
// }
// }

/*
 * If you don't have a 'const' array, just cast it as such.  It's safer
 * to cast a non-const array as const than to cast a const one as non-const.
 * Doesn't really matter since this function doesn't change the array though.
 */
// size_t sprintbit(bitvector_t bitvector, const char *names[], char *result, size_t reslen)
// {
// size_t len = 0;
// int nlen;
// long nr;
//
// *result = '\0';
//
// for (nr = 0; bitvector && len < reslen; bitvector >>= 1) {
// if (IS_SET(bitvector, 1)) {
// nlen = snprintf(result + len, reslen - len, "%s ", *names[nr] != '\n' ? names[nr] : "UNDEFINED");
// if (len + nlen >= reslen || nlen < 0)
// break;
// len += nlen;
// }
//
// if (*names[nr] != '\n')
// nr++;
// }
//
// if (!*result)
// len = strlcpy(result, "NOBITS ", reslen);
//
// return (len);
// }

// size_t sprinttype(int type, const char *names[], char *result, size_t reslen)
// {
// int nr = 0;
//
// while (type && *names[nr] != '\n') {
// type--;
// nr++;
// }
//
// return strlcpy(result, *names[nr] != '\n' ? names[nr] : "UNDEFINED", reslen);
// }

/* Calculate the REAL time passed over the last t2-t1 centuries (secs) */
// struct time_info_data *real_time_passed(time_t t2, time_t t1)
// {
// long secs;
// static struct time_info_data now;
//
// secs = t2 - t1;
//
// now.hours = (secs / SECS_PER_REAL_HOUR) % 24;	/* 0..23 hours */
// secs -= SECS_PER_REAL_HOUR * now.hours;
//
// now.day = (secs / SECS_PER_REAL_DAY);	/* 0..34 days  */
// /* secs -= SECS_PER_REAL_DAY * now.day; - Not used. */
//
// now.month = -1;
// now.year = -1;
//
// return (&now);
// }

/* Calculate the MUD time passed over the last t2-t1 centuries (secs) */
// struct time_info_data *mud_time_passed(time_t t2, time_t t1)
// {
// long secs;
// static struct time_info_data now;
//
// secs = t2 - t1;
//
// now.hours = (secs / SECS_PER_MUD_HOUR) % 24;	/* 0..23 hours */
// secs -= SECS_PER_MUD_HOUR * now.hours;
//
// now.day = (secs / SECS_PER_MUD_DAY) % 35;	/* 0..34 days  */
// secs -= SECS_PER_MUD_DAY * now.day;
//
// now.month = (secs / SECS_PER_MUD_MONTH) % 17;	/* 0..16 months */
// secs -= SECS_PER_MUD_MONTH * now.month;
//
// now.year = (secs / SECS_PER_MUD_YEAR);	/* 0..XX? years */
//
// return (&now);
// }

// time_t mud_time_to_secs(struct time_info_data *now)
// {
// time_t when = 0;
//
// when += now->year  * SECS_PER_MUD_YEAR;
// when += now->month * SECS_PER_MUD_MONTH;
// when += now->day   * SECS_PER_MUD_DAY;
// when += now->hours * SECS_PER_MUD_HOUR;
//
// return (time(NULL) - when);
// }

// struct time_info_data *age(struct char_data *ch)
// {
// static struct time_info_data player_age;
//
// player_age = *mud_time_passed(time(0), ch->player.time.birth);
//
// player_age.year += 17;	/* All players start at 17 */
//
// return (&player_age);
// }

/* Check if making CH follow VICTIM will create an illegal */
/* Follow "Loop/circle"                                    */
// bool circle_follow(struct char_data *ch, struct char_data *victim)
// {
// struct char_data *k;
//
// for (k = victim; k; k = k->master) {
// if (k == ch)
// return (TRUE);
// }
//
// return (FALSE);
// }

/* Called when stop following persons, or stopping charm */
/* This will NOT do if a character quits/dies!!          */
// void stop_follower(struct char_data *ch)
// {
// struct follow_type *j, *k;
//
// if (ch->master == NULL) {
// core_dump();
// return;
// }
//
// if (AFF_FLAGGED(ch, AFF_CHARM)) {
// act("You realize that $N is a jerk!", FALSE, ch, 0, ch->master, TO_CHAR);
// act("$n realizes that $N is a jerk!", FALSE, ch, 0, ch->master, TO_NOTVICT);
// act("$n hates your guts!", FALSE, ch, 0, ch->master, TO_VICT);
// if (affected_by_spell(ch, SPELL_CHARM))
// affect_from_char(ch, SPELL_CHARM);
// } else {
// act("You stop following $N.", FALSE, ch, 0, ch->master, TO_CHAR);
// act("$n stops following $N.", TRUE, ch, 0, ch->master, TO_NOTVICT);
// act("$n stops following you.", TRUE, ch, 0, ch->master, TO_VICT);
// }
//
// if (ch->master->followers->follower == ch) {	/* Head of follower-list? */
// k = ch->master->followers;
// ch->master->followers = k->next;
// free(k);
// } else {			/* locate follower who is not head of list */
// for (k = ch->master->followers; k->next->follower != ch; k = k->next);
//
// j = k->next;
// k->next = j->next;
// free(j);
// }
//
// ch->master = NULL;
// REMOVE_BIT(AFF_FLAGS(ch), AFF_CHARM | AFF_GROUP);
// }

//
// int num_followers_charmed(struct char_data *ch)
// {
// struct follow_type *lackey;
// int total = 0;
//
// for (lackey = ch->followers; lackey; lackey = lackey->next)
// if (AFF_FLAGGED(lackey->follower, AFF_CHARM) && lackey->follower->master == ch)
// total++;
//
// return (total);
// }
//

/* Called when a character that follows/is followed dies */
// void die_follower(struct char_data *ch)
// {
// struct follow_type *j, *k;
//
// if (ch->master)
// stop_follower(ch);
//
// for (k = ch->followers; k; k = j) {
// j = k->next;
// stop_follower(k->follower);
// }
// }
//
//

/* Do NOT call this before having checked if a circle of followers */
/* will arise. CH will follow leader                               */
// void add_follower(struct char_data *ch, struct char_data *leader)
// {
// struct follow_type *k;
//
// if (ch->master) {
// core_dump();
// return;
// }
//
// ch->master = leader;
//
// CREATE(k, struct follow_type, 1);
//
// k->follower = ch;
// k->next = leader->followers;
// leader->followers = k;
//
// act("You now follow $N.", FALSE, ch, 0, leader, TO_CHAR);
// if (CAN_SEE(leader, ch))
// act("$n starts following you.", TRUE, ch, 0, leader, TO_VICT);
// act("$n starts to follow $N.", TRUE, ch, 0, leader, TO_NOTVICT);
// }

/*
 * get_line reads the next non-blank line off of the input stream.
 * The newline character is removed from the input.  Lines which begin
 * with '*' are considered to be comments.
 *
 * Returns the number of lines advanced in the file. Buffer given must
 * be at least READ_SIZE (256) characters large.
 */
// int get_line(FILE *fl, char *buf)
// {
// char temp[READ_SIZE];
// int lines = 0;
// int sl;
//
// do {
// if (!fgets(temp, READ_SIZE, fl))
// return (0);
// lines++;
// } while (*temp == '*' || *temp == '\n' || *temp == '\r');
//
// /* Last line of file doesn't always have a \n, but it should. */
// sl = strlen(temp);
// while (sl > 0 && (temp[sl - 1] == '\n' || temp[sl - 1] == '\r'))
// temp[--sl] = '\0';
//
// strcpy(buf, temp); /* strcpy: OK, if buf >= READ_SIZE (256) */
// return (lines);
// }

// int get_filename(char *filename, size_t fbufsize, int mode, const char *orig_name)
// {
// const char *prefix, *middle, *suffix;
// char name[PATH_MAX], *ptr;
//
// if (orig_name == NULL || *orig_name == '\0' || filename == NULL) {
// log("SYSERR: NULL pointer or empty string passed to get_filename(), %p or %p.",
// orig_name, filename);
// return (0);
// }
//
// switch (mode) {
// case CRASH_FILE:
// prefix = LIB_PLROBJS;
// suffix = SUF_OBJS;
// break;
// case ALIAS_FILE:
// prefix = LIB_PLRALIAS;
// suffix = SUF_ALIAS;
// break;
// case ETEXT_FILE:
// prefix = LIB_PLRTEXT;
// suffix = SUF_TEXT;
// break;
// default:
// return (0);
// }
//
// strlcpy(name, orig_name, sizeof(name));
// for (ptr = name; *ptr; ptr++)
// *ptr = LOWER(*ptr);
//
// switch (LOWER(*name)) {
// case 'a':  case 'b':  case 'c':  case 'd':  case 'e':
// middle = "A-E";
// break;
// case 'f':  case 'g':  case 'h':  case 'i':  case 'j':
// middle = "F-J";
// break;
// case 'k':  case 'l':  case 'm':  case 'n':  case 'o':
// middle = "K-O";
// break;
// case 'p':  case 'q':  case 'r':  case 's':  case 't':
// middle = "P-T";
// break;
// case 'u':  case 'v':  case 'w':  case 'x':  case 'y':  case 'z':
// middle = "U-Z";
// break;
// default:
// middle = "ZZZ";
// break;
// }
//
// snprintf(filename, fbufsize, "%s%s"SLASH"%s.%s", prefix, middle, name, suffix);
// return (1);
// }

//
// int num_pc_in_room(struct room_data *room)
// {
// int i = 0;
// struct char_data *ch;
//
// for (ch = room->people; ch != NULL; ch = ch->next_in_room)
// if (!IS_NPC(ch))
// i++;
//
// return (i);
// }

/*
 * This function (derived from basic fork(); abort(); idea by Erwin S.
 * Andreasen) causes your MUD to dump core (assuming you can) but
 * continue running.  The core dump will allow post-mortem debugging
 * that is less severe than assert();  Don't call this directly as
 * core_dump_unix() but as simply 'core_dump()' so that it will be
 * excluded from systems not supporting them. (e.g. Windows '95).
 *
 * You still want to call abort() or exit(1) for
 * non-recoverable errors, of course...
 *
 * XXX: Wonder if flushing streams includes sockets?
 */
// extern FILE *player_fl;
// void core_dump_real(const char *who, int line)
// {
// log("SYSERR: Assertion failed at %s:%d!", who, line);
//
// #if 0	/* By default, let's not litter. */
// #if defined(CIRCLE_UNIX)
// /* These would be duplicated otherwise...make very sure. */
// fflush(stdout);
// fflush(stderr);
// fflush(logfile);
// fflush(player_fl);
// /* Everything, just in case, for the systems that support it. */
// fflush(NULL);
//
// /*
//  * Kill the child so the debugger or script doesn't think the MUD
//  * crashed.  The 'autorun' script would otherwise run it again.
//  */
// if (fork() == 0)
// abort();
// #endif
// #endif
// }

/*
 * Rules (unless overridden by ROOM_DARK):
 *
 * Inside and City rooms are always lit.
 * Outside rooms are dark at sunset and night.
 */
// int room_is_dark(room_rnum room)
// {
// if (!VALID_ROOM_RNUM(room)) {
// log("room_is_dark: Invalid room rnum %d. (0-%d)", room, top_of_world);
// return (FALSE);
// }
//
// if (world[room].light)
// return (FALSE);
//
// if (ROOM_FLAGGED(room, ROOM_DARK))
// return (TRUE);
//
// if (SECT(room) == SECT_INSIDE || SECT(room) == SECT_CITY)
// return (FALSE);
//
// if (weather_info.sunlight == SUN_SET || weather_info.sunlight == SUN_DARK)
// return (TRUE);
//
// return (FALSE);
// }
