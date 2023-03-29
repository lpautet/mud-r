/* ************************************************************************
*   File: act.movement.c                                Part of CircleMUD *
*  Usage: movement commands, door handling, & sleep/rest/etc state        *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use crate::config::TUNNEL_SIZE;
use crate::constants::{DIRS, MOVEMENT_LOSS};
use crate::db::DB;
use crate::handler::fname;
use crate::structs::{
    CharData, AFF_CHARM, AFF_SNEAK, EX_CLOSED, LVL_GRGOD, LVL_IMMORT, NOWHERE, NUM_OF_DIRS,
    ROOM_GODROOM, ROOM_TUNNEL, SECT_WATER_NOSWIM,
};
use crate::util::num_pc_in_room;
use crate::{send_to_char, MainGlobals, TO_ROOM};
use std::borrow::Borrow;
use std::rc::Rc;

// ACMD(do_gen_door);
// ACMD(do_enter);
// ACMD(do_leave);
// ACMD(do_stand);
// ACMD(do_sit);
// ACMD(do_rest);
// ACMD(do_sleep);
// ACMD(do_wake);
// ACMD(do_follow);
//
//
// /* simple function to determine if char can walk on water */
// int has_boat(struct char_data *ch)
// {
// struct obj_data *obj;
// int i;
//
// /*
//   if (ROOM_IDENTITY(IN_ROOM(ch)) == DEAD_SEA)
//     return (1);
// */
//
// if (GET_LEVEL(ch) > LVL_IMMORT)
// return (1);
//
// if (AFF_FLAGGED(ch, AFF_WATERWALK))
// return (1);
//
// /* non-wearable boats in inventory will do it */
// for (obj = ch->carrying; obj; obj = obj->next_content)
// if (GET_OBJ_TYPE(obj) == ITEM_BOAT && (find_eq_pos(ch, obj, NULL) < 0))
// return (1);
//
// /* and any boat you're wearing will do it too */
// for (i = 0; i < NUM_WEARS; i++)
// if (GET_EQ(ch, i) && GET_OBJ_TYPE(GET_EQ(ch, i)) == ITEM_BOAT)
// return (1);
//
// return (0);
// }

/* do_simple_move assumes
 *    1. That there is no master and no followers.
 *    2. That the direction exists.
 *
 *   Returns :
 *   1 : If succes.
 *   0 : If fail
 */
pub fn perform_move(db: &DB, rch: &Rc<CharData>, dir: i32, need_specials_check: i32) -> i32 {
    let ch = rch.as_ref();
    //struct follow_type *k, *next;

    if
    /* ch == NULL || */
    dir < 0 || dir >= NUM_OF_DIRS as i32 || ch.fighting().is_some() {
        return 0;
    } else if db.exit(ch, dir as usize).is_none()
        || db.exit(ch, dir as usize).as_ref().unwrap().to_room.get() == NOWHERE
    {
        send_to_char(ch, "Alas, you cannot go that way...\r\n");
    } else if db
        .exit(ch, dir as usize)
        .as_ref()
        .unwrap()
        .exit_flagged(EX_CLOSED)
    {
        if !db
            .exit(ch, dir as usize)
            .as_ref()
            .unwrap()
            .keyword
            .is_empty()
        {
            send_to_char(
                ch,
                format!(
                    "The {} seems to be closed.\r\n",
                    fname(db.exit(ch, dir as usize).as_ref().unwrap().keyword.as_str())
                )
                .as_str(),
            );
        } else {
            send_to_char(ch, "It seems to be closed.\r\n");
        }
    } else {
        if ch.followers.borrow().is_empty() {
            return do_simple_move(db, rch, dir, need_specials_check);
        }

        // TODO implement follower
        // was_in = IN_ROOM(ch);
        // if (!do_simple_move(ch, dir, need_specials_check))
        // return (0);
        //
        // for (k = ch->followers; k; k = next) {
        //     next = k -> next;
        //     if ((IN_ROOM(k->follower) == was_in) &&
        //         (GET_POS(k->follower) >= POS_STANDING)) {
        //         act("You follow $N.\r\n", FALSE, k->follower, 0, ch, TO_CHAR);
        //         perform_move(k->follower, dir, 1);
        //     }
        // }
        // return (1);
    }
    return 0;
}

pub fn do_simple_move(db: &DB, ch: &Rc<CharData>, dir: i32, need_specials_check: i32) -> i32 {
    //char throwaway[MAX_INPUT_LENGTH] = ""; /* Functions assume writable. */
    let was_in;
    let need_movement;

    /*
     * Check for special routines (North is 1 in command list, but 0 here) Note
     * -- only check if following; this avoids 'double spec-proc' bug
     */
    // TODO implement spec proc
    // if (need_specials_check && special(ch, dir + 1, throwaway))
    // return (0);

    /* charmed? */
    if ch.aff_flagged(AFF_CHARM)
        && ch.master.borrow().is_some()
        && ch.in_room() == ch.master.borrow().as_ref().unwrap().in_room()
    {
        send_to_char(ch, "The thought of leaving your master makes you weep.\r\n");
        db.act(
            "$n bursts into tears.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        return 0;
    }

    /* if this room or the one we're going to needs a boat, check for one */
    if (db.sect(ch.in_room()) == SECT_WATER_NOSWIM)
        || (db.sect(db.exit(ch, dir as usize).as_ref().unwrap().to_room.get()) == SECT_WATER_NOSWIM)
    {
        // TODO implement has_boat
        // if (!has_boat(ch)) {
        send_to_char(ch, "You need a boat to go there.\r\n");
        return 0;
        // }
    }

    /* move points needed is avg. move loss for src and destination sect type */
    need_movement = (MOVEMENT_LOSS[db.sect(ch.in_room()) as usize]
        + MOVEMENT_LOSS
            [db.sect(db.exit(ch, dir as usize).as_ref().unwrap().to_room.get()) as usize])
        / 2;

    if ch.get_move() < need_movement as i16 && !ch.is_npc() {
        if need_specials_check != 0 && ch.master.borrow().is_some() {
            send_to_char(ch, "You are too exhausted to follow.\r\n");
        } else {
            send_to_char(ch, "You are too exhausted.\r\n");
        }

        return 0;
    }
    // TODO implement houses
    // if db.room_flagged(ch.in_room(), ROOM_ATRIUM) {
    //     if (!House_can_enter(ch, GET_ROOM_VNUM(EXIT(ch, dir)->to_room))) {
    //         send_to_char(ch, "That's private property -- no trespassing!\r\n");
    //         return (0);
    //     }
    // }
    if db.room_flagged(
        db.exit(ch, dir as usize).as_ref().unwrap().to_room.get(),
        ROOM_TUNNEL,
    ) && num_pc_in_room(
        db.world.borrow()[db.exit(ch, dir as usize).as_ref().unwrap().to_room.get() as usize]
            .borrow(),
    ) >= TUNNEL_SIZE
    {
        if TUNNEL_SIZE > 1 {
            send_to_char(ch, "There isn't enough room for you to go there!\r\n");
        } else {
            send_to_char(
                ch,
                "There isn't enough room there for more than one person!\r\n",
            );
        }
        return 0;
    }
    /* Mortals and low level gods cannot enter greater god rooms. */
    if db.room_flagged(
        db.exit(ch, dir as usize).as_ref().unwrap().to_room.get(),
        ROOM_GODROOM,
    ) && ch.get_level() < LVL_GRGOD as u8
    {
        send_to_char(ch, "You aren't godly enough to use that room!\r\n");
        return 0;
    }

    /* Now we know we're allow to go into the room. */
    if ch.get_level() < LVL_IMMORT as u8 && !ch.is_npc() {
        ch.incr_move(-need_movement as i16);
    }

    if !ch.aff_flagged(AFF_SNEAK) {
        let buf2 = format!("$n leaves {}.", DIRS[dir as usize]);
        db.act(buf2.as_str(), true, Some(ch), None, None, TO_ROOM);
    }
    was_in = ch.in_room();
    db.char_from_room(ch);
    db.char_to_room(
        Some(ch),
        db.world.borrow()[was_in as usize].dir_option[dir as usize]
            .as_ref()
            .unwrap()
            .to_room
            .get(),
    );

    if !ch.aff_flagged(AFF_SNEAK) {
        db.act("$n has arrived.", true, Some(ch), None, None, TO_ROOM);
    }

    if ch.desc.borrow().is_some() {
        db.look_at_room(ch, false);
    }

    // if (ROOM_FLAGGED(IN_ROOM(ch), ROOM_DEATH) && GET_LEVEL(ch) < LVL_IMMORT) {
    //     log_death_trap(ch);
    //     death_cry(ch);
    //     extract_char(ch);
    //     return (0);
    // }
    return 1;
}
#[allow(unused_variables)]
pub fn do_move(game: &MainGlobals, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    /*
     * This is basically a mapping of cmd numbers to perform_move indices.
     * It cannot be done in perform_move because perform_move is called
     * by other functions which do not require the remapping.
     */
    let db = &game.db;
    perform_move(db, ch, subcmd - 1, 0);
}
//
//
// int find_door(struct char_data *ch, const char *type, char *dir, const char *cmdname)
// {
// int door;
//
// if (*dir) {			/* a direction was specified */
// if ((door = search_block(dir, dirs, FALSE)) == -1) {	/* Partial Match */
// send_to_char(ch, "That's not a direction.\r\n");
// return (-1);
// }
// if (EXIT(ch, door)) {	/* Braces added according to indent. -gg */
// if (EXIT(ch, door)->keyword) {
// if (isname(type, EXIT(ch, door)->keyword))
// return (door);
// else {
// send_to_char(ch, "I see no %s there.\r\n", type);
// return (-1);
// }
// } else
// return (door);
// } else {
// send_to_char(ch, "I really don't see how you can %s anything there.\r\n", cmdname);
// return (-1);
// }
// } else {			/* try to locate the keyword */
// if (!*type) {
// send_to_char(ch, "What is it you want to %s?\r\n", cmdname);
// return (-1);
// }
// for (door = 0; door < NUM_OF_DIRS; door++)
// if (EXIT(ch, door))
// if (EXIT(ch, door)->keyword)
// if (isname(type, EXIT(ch, door)->keyword))
// return (door);
//
// send_to_char(ch, "There doesn't seem to be %s %s here.\r\n", AN(type), type);
// return (-1);
// }
// }
//
//
// int has_key(struct char_data *ch, obj_vnum key)
// {
// struct obj_data *o;
//
// for (o = ch->carrying; o; o = o->next_content)
// if (GET_OBJ_VNUM(o) == key)
// return (1);
//
// if (GET_EQ(ch, WEAR_HOLD))
// if (GET_OBJ_VNUM(GET_EQ(ch, WEAR_HOLD)) == key)
// return (1);
//
// return (0);
// }
//
//
//
// #define NEED_OPEN	(1 << 0)
// #define NEED_CLOSED	(1 << 1)
// #define NEED_UNLOCKED	(1 << 2)
// #define NEED_LOCKED	(1 << 3)
//
// const char *cmd_door[] =
// {
// "open",
// "close",
// "unlock",
// "lock",
// "pick"
// };
//
// const int flags_door[] =
// {
// NEED_CLOSED | NEED_UNLOCKED,
// NEED_OPEN,
// NEED_CLOSED | NEED_LOCKED,
// NEED_CLOSED | NEED_UNLOCKED,
// NEED_CLOSED | NEED_LOCKED
// };
//
//
// #define EXITN(room, door)		(world[room].dir_option[door])
// #define OPEN_DOOR(room, obj, door)	((obj) ?\
// (REMOVE_BIT(GET_OBJ_VAL(obj, 1), CONT_CLOSED)) :\
// (REMOVE_BIT(EXITN(room, door)->exit_info, EX_CLOSED)))
// #define CLOSE_DOOR(room, obj, door)	((obj) ?\
// (SET_BIT(GET_OBJ_VAL(obj, 1), CONT_CLOSED)) :\
// (SET_BIT(EXITN(room, door)->exit_info, EX_CLOSED)))
// #define LOCK_DOOR(room, obj, door)	((obj) ?\
// (SET_BIT(GET_OBJ_VAL(obj, 1), CONT_LOCKED)) :\
// (SET_BIT(EXITN(room, door)->exit_info, EX_LOCKED)))
// #define UNLOCK_DOOR(room, obj, door)	((obj) ?\
// (REMOVE_BIT(GET_OBJ_VAL(obj, 1), CONT_LOCKED)) :\
// (REMOVE_BIT(EXITN(room, door)->exit_info, EX_LOCKED)))
// #define TOGGLE_LOCK(room, obj, door)	((obj) ?\
// (TOGGLE_BIT(GET_OBJ_VAL(obj, 1), CONT_LOCKED)) :\
// (TOGGLE_BIT(EXITN(room, door)->exit_info, EX_LOCKED)))
//
// void do_doorcmd(struct char_data *ch, struct obj_data *obj, int door, int scmd)
// {
// char buf[MAX_STRING_LENGTH];
// size_t len;
// room_rnum other_room = NOWHERE;
// struct room_direction_data *back = NULL;
//
// len = snprintf(buf, sizeof(buf), "$n %ss ", cmd_door[scmd]);
// if (!obj && ((other_room = EXIT(ch, door)->to_room) != NOWHERE))
// if ((back = world[other_room].dir_option[rev_dir[door]]) != NULL)
// if (back->to_room != IN_ROOM(ch))
// back = NULL;
//
// switch (scmd) {
// case SCMD_OPEN:
// OPEN_DOOR(IN_ROOM(ch), obj, door);
// if (back)
// OPEN_DOOR(other_room, obj, rev_dir[door]);
// send_to_char(ch, "%s", OK);
// break;
//
// case SCMD_CLOSE:
// CLOSE_DOOR(IN_ROOM(ch), obj, door);
// if (back)
// CLOSE_DOOR(other_room, obj, rev_dir[door]);
// send_to_char(ch, "%s", OK);
// break;
//
// case SCMD_LOCK:
// LOCK_DOOR(IN_ROOM(ch), obj, door);
// if (back)
// LOCK_DOOR(other_room, obj, rev_dir[door]);
// send_to_char(ch, "*Click*\r\n");
// break;
//
// case SCMD_UNLOCK:
// UNLOCK_DOOR(IN_ROOM(ch), obj, door);
// if (back)
// UNLOCK_DOOR(other_room, obj, rev_dir[door]);
// send_to_char(ch, "*Click*\r\n");
// break;
//
// case SCMD_PICK:
// TOGGLE_LOCK(IN_ROOM(ch), obj, door);
// if (back)
// TOGGLE_LOCK(other_room, obj, rev_dir[door]);
// send_to_char(ch, "The lock quickly yields to your skills.\r\n");
// len = strlcpy(buf, "$n skillfully picks the lock on ", sizeof(buf));
// break;
// }
//
// /* Notify the room. */
// if (len < sizeof(buf))
// snprintf(buf + len, sizeof(buf) - len, "%s%s.",
// obj ? "" : "the ", obj ? "$p" : EXIT(ch, door)->keyword ? "$F" : "door");
// if (!obj || IN_ROOM(obj) != NOWHERE)
// act(buf, FALSE, ch, obj, obj ? 0 : EXIT(ch, door)->keyword, TO_ROOM);
//
// /* Notify the other room */
// if (back && (scmd == SCMD_OPEN || scmd == SCMD_CLOSE))
// send_to_room(EXIT(ch, door)->to_room, "The %s is %s%s from the other side.",
// back->keyword ? fname(back->keyword) : "door", cmd_door[scmd],
// scmd == SCMD_CLOSE ? "d" : "ed");
// }
//
//
// int ok_pick(struct char_data *ch, obj_vnum keynum, int pickproof, int scmd)
// {
// int percent, skill_lvl;
//
// if (scmd != SCMD_PICK)
// return (1);
//
// percent = rand_number(1, 101);
// skill_lvl = GET_SKILL(ch, SKILL_PICK_LOCK) + dex_app_skill[GET_DEX(ch)].p_locks;
//
// if (keynum == NOTHING)
// send_to_char(ch, "Odd - you can't seem to find a keyhole.\r\n");
// else if (pickproof)
// send_to_char(ch, "It resists your attempts to pick it.\r\n");
// else if (percent > skill_lvl)
// send_to_char(ch, "You failed to pick the lock.\r\n");
// else
// return (1);
//
// return (0);
// }
//
//
// #define DOOR_IS_OPENABLE(ch, obj, door)	((obj) ? \
// ((GET_OBJ_TYPE(obj) == ITEM_CONTAINER) && \
// OBJVAL_FLAGGED(obj, CONT_CLOSEABLE)) :\
// (EXIT_FLAGGED(EXIT(ch, door), EX_ISDOOR)))
// #define DOOR_IS_OPEN(ch, obj, door)	((obj) ? \
// (!OBJVAL_FLAGGED(obj, CONT_CLOSED)) :\
// (!EXIT_FLAGGED(EXIT(ch, door), EX_CLOSED)))
// #define DOOR_IS_UNLOCKED(ch, obj, door)	((obj) ? \
// (!OBJVAL_FLAGGED(obj, CONT_LOCKED)) :\
// (!EXIT_FLAGGED(EXIT(ch, door), EX_LOCKED)))
// #define DOOR_IS_PICKPROOF(ch, obj, door) ((obj) ? \
// (OBJVAL_FLAGGED(obj, CONT_PICKPROOF)) : \
// (EXIT_FLAGGED(EXIT(ch, door), EX_PICKPROOF)))
//
// #define DOOR_IS_CLOSED(ch, obj, door)	(!(DOOR_IS_OPEN(ch, obj, door)))
// #define DOOR_IS_LOCKED(ch, obj, door)	(!(DOOR_IS_UNLOCKED(ch, obj, door)))
// #define DOOR_KEY(ch, obj, door)		((obj) ? (GET_OBJ_VAL(obj, 2)) : \
// (EXIT(ch, door)->key))
//
// ACMD(do_gen_door)
// {
// int door = -1;
// obj_vnum keynum;
// char type[MAX_INPUT_LENGTH], dir[MAX_INPUT_LENGTH];
// struct obj_data *obj = NULL;
// struct char_data *victim = NULL;
//
// skip_spaces(&argument);
// if (!*argument) {
// send_to_char(ch, "%c%s what?\r\n", UPPER(*cmd_door[subcmd]), cmd_door[subcmd] + 1);
// return;
// }
// two_arguments(argument, type, dir);
// if (!generic_find(type, FIND_OBJ_INV | FIND_OBJ_ROOM, ch, &victim, &obj))
// door = find_door(ch, type, dir, cmd_door[subcmd]);
//
// if ((obj) || (door >= 0)) {
// keynum = DOOR_KEY(ch, obj, door);
// if (!(DOOR_IS_OPENABLE(ch, obj, door)))
// act("You can't $F that!", FALSE, ch, 0, cmd_door[subcmd], TO_CHAR);
// else if (!DOOR_IS_OPEN(ch, obj, door) &&
// IS_SET(flags_door[subcmd], NEED_OPEN))
// send_to_char(ch, "But it's already closed!\r\n");
// else if (!DOOR_IS_CLOSED(ch, obj, door) &&
// IS_SET(flags_door[subcmd], NEED_CLOSED))
// send_to_char(ch, "But it's currently open!\r\n");
// else if (!(DOOR_IS_LOCKED(ch, obj, door)) &&
// IS_SET(flags_door[subcmd], NEED_LOCKED))
// send_to_char(ch, "Oh.. it wasn't locked, after all..\r\n");
// else if (!(DOOR_IS_UNLOCKED(ch, obj, door)) &&
// IS_SET(flags_door[subcmd], NEED_UNLOCKED))
// send_to_char(ch, "It seems to be locked.\r\n");
// else if (!has_key(ch, keynum) && (GET_LEVEL(ch) < LVL_GOD) &&
// ((subcmd == SCMD_LOCK) || (subcmd == SCMD_UNLOCK)))
// send_to_char(ch, "You don't seem to have the proper key.\r\n");
// else if (ok_pick(ch, keynum, DOOR_IS_PICKPROOF(ch, obj, door), subcmd))
// do_doorcmd(ch, obj, door, subcmd);
// }
// return;
// }
//
//
//
// ACMD(do_enter)
// {
// char buf[MAX_INPUT_LENGTH];
// int door;
//
// one_argument(argument, buf);
//
// if (*buf) {			/* an argument was supplied, search for door
// 				 * keyword */
// for (door = 0; door < NUM_OF_DIRS; door++)
// if (EXIT(ch, door))
// if (EXIT(ch, door)->keyword)
// if (!str_cmp(EXIT(ch, door)->keyword, buf)) {
// perform_move(ch, door, 1);
// return;
// }
// send_to_char(ch, "There is no %s here.\r\n", buf);
// } else if (ROOM_FLAGGED(IN_ROOM(ch), ROOM_INDOORS))
// send_to_char(ch, "You are already indoors.\r\n");
// else {
// /* try to locate an entrance */
// for (door = 0; door < NUM_OF_DIRS; door++)
// if (EXIT(ch, door))
// if (EXIT(ch, door)->to_room != NOWHERE)
// if (!EXIT_FLAGGED(EXIT(ch, door), EX_CLOSED) &&
// ROOM_FLAGGED(EXIT(ch, door)->to_room, ROOM_INDOORS)) {
// perform_move(ch, door, 1);
// return;
// }
// send_to_char(ch, "You can't seem to find anything to enter.\r\n");
// }
// }
//
//
// ACMD(do_leave)
// {
// int door;
//
// if (OUTSIDE(ch))
// send_to_char(ch, "You are outside.. where do you want to go?\r\n");
// else {
// for (door = 0; door < NUM_OF_DIRS; door++)
// if (EXIT(ch, door))
// if (EXIT(ch, door)->to_room != NOWHERE)
// if (!EXIT_FLAGGED(EXIT(ch, door), EX_CLOSED) &&
// !ROOM_FLAGGED(EXIT(ch, door)->to_room, ROOM_INDOORS)) {
// perform_move(ch, door, 1);
// return;
// }
// send_to_char(ch, "I see no obvious exits to the outside.\r\n");
// }
// }
//
//
// ACMD(do_stand)
// {
// switch (GET_POS(ch)) {
// case POS_STANDING:
// send_to_char(ch, "You are already standing.\r\n");
// break;
// case POS_SITTING:
// send_to_char(ch, "You stand up.\r\n");
// act("$n clambers to $s feet.", TRUE, ch, 0, 0, TO_ROOM);
// /* Will be sitting after a successful bash and may still be fighting. */
// GET_POS(ch) = FIGHTING(ch) ? POS_FIGHTING : POS_STANDING;
// break;
// case POS_RESTING:
// send_to_char(ch, "You stop resting, and stand up.\r\n");
// act("$n stops resting, and clambers on $s feet.", TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_STANDING;
// break;
// case POS_SLEEPING:
// send_to_char(ch, "You have to wake up first!\r\n");
// break;
// case POS_FIGHTING:
// send_to_char(ch, "Do you not consider fighting as standing?\r\n");
// break;
// default:
// send_to_char(ch, "You stop floating around, and put your feet on the ground.\r\n");
// act("$n stops floating around, and puts $s feet on the ground.",
// TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_STANDING;
// break;
// }
// }
//
//
// ACMD(do_sit)
// {
// switch (GET_POS(ch)) {
// case POS_STANDING:
// send_to_char(ch, "You sit down.\r\n");
// act("$n sits down.", FALSE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_SITTING;
// break;
// case POS_SITTING:
// send_to_char(ch, "You're sitting already.\r\n");
// break;
// case POS_RESTING:
// send_to_char(ch, "You stop resting, and sit up.\r\n");
// act("$n stops resting.", TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_SITTING;
// break;
// case POS_SLEEPING:
// send_to_char(ch, "You have to wake up first.\r\n");
// break;
// case POS_FIGHTING:
// send_to_char(ch, "Sit down while fighting? Are you MAD?\r\n");
// break;
// default:
// send_to_char(ch, "You stop floating around, and sit down.\r\n");
// act("$n stops floating around, and sits down.", TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_SITTING;
// break;
// }
// }
//
//
// ACMD(do_rest)
// {
// switch (GET_POS(ch)) {
// case POS_STANDING:
// send_to_char(ch, "You sit down and rest your tired bones.\r\n");
// act("$n sits down and rests.", TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_RESTING;
// break;
// case POS_SITTING:
// send_to_char(ch, "You rest your tired bones.\r\n");
// act("$n rests.", TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_RESTING;
// break;
// case POS_RESTING:
// send_to_char(ch, "You are already resting.\r\n");
// break;
// case POS_SLEEPING:
// send_to_char(ch, "You have to wake up first.\r\n");
// break;
// case POS_FIGHTING:
// send_to_char(ch, "Rest while fighting?  Are you MAD?\r\n");
// break;
// default:
// send_to_char(ch, "You stop floating around, and stop to rest your tired bones.\r\n");
// act("$n stops floating around, and rests.", FALSE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_SITTING;
// break;
// }
// }
//
//
// ACMD(do_sleep)
// {
// switch (GET_POS(ch)) {
// case POS_STANDING:
// case POS_SITTING:
// case POS_RESTING:
// send_to_char(ch, "You go to sleep.\r\n");
// act("$n lies down and falls asleep.", TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_SLEEPING;
// break;
// case POS_SLEEPING:
// send_to_char(ch, "You are already sound asleep.\r\n");
// break;
// case POS_FIGHTING:
// send_to_char(ch, "Sleep while fighting?  Are you MAD?\r\n");
// break;
// default:
// send_to_char(ch, "You stop floating around, and lie down to sleep.\r\n");
// act("$n stops floating around, and lie down to sleep.",
// TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_SLEEPING;
// break;
// }
// }
//
//
// ACMD(do_wake)
// {
// char arg[MAX_INPUT_LENGTH];
// struct char_data *vict;
// int self = 0;
//
// one_argument(argument, arg);
// if (*arg) {
// if (GET_POS(ch) == POS_SLEEPING)
// send_to_char(ch, "Maybe you should wake yourself up first.\r\n");
// else if ((vict = get_char_vis(ch, arg, NULL, FIND_CHAR_ROOM)) == NULL)
// send_to_char(ch, "%s", NOPERSON);
// else if (vict == ch)
// self = 1;
// else if (AWAKE(vict))
// act("$E is already awake.", FALSE, ch, 0, vict, TO_CHAR);
// else if (AFF_FLAGGED(vict, AFF_SLEEP))
// act("You can't wake $M up!", FALSE, ch, 0, vict, TO_CHAR);
// else if (GET_POS(vict) < POS_SLEEPING)
// act("$E's in pretty bad shape!", FALSE, ch, 0, vict, TO_CHAR);
// else {
// act("You wake $M up.", FALSE, ch, 0, vict, TO_CHAR);
// act("You are awakened by $n.", FALSE, ch, 0, vict, TO_VICT | TO_SLEEP);
// GET_POS(vict) = POS_SITTING;
// }
// if (!self)
// return;
// }
// if (AFF_FLAGGED(ch, AFF_SLEEP))
// send_to_char(ch, "You can't wake up!\r\n");
// else if (GET_POS(ch) > POS_SLEEPING)
// send_to_char(ch, "You are already awake...\r\n");
// else {
// send_to_char(ch, "You awaken, and sit up.\r\n");
// act("$n awakens.", TRUE, ch, 0, 0, TO_ROOM);
// GET_POS(ch) = POS_SITTING;
// }
// }
//
//
// ACMD(do_follow)
// {
// char buf[MAX_INPUT_LENGTH];
// struct char_data *leader;
//
// one_argument(argument, buf);
//
// if (*buf) {
// if (!(leader = get_char_vis(ch, buf, NULL, FIND_CHAR_ROOM))) {
// send_to_char(ch, "%s", NOPERSON);
// return;
// }
// } else {
// send_to_char(ch, "Whom do you wish to follow?\r\n");
// return;
// }
//
// if (ch->master == leader) {
// act("You are already following $M.", FALSE, ch, 0, leader, TO_CHAR);
// return;
// }
// if (AFF_FLAGGED(ch, AFF_CHARM) && (ch->master)) {
// act("But you only feel like following $N!", FALSE, ch, 0, ch->master, TO_CHAR);
// } else {			/* Not Charmed follow person */
// if (leader == ch) {
// if (!ch->master) {
// send_to_char(ch, "You are already following yourself.\r\n");
// return;
// }
// stop_follower(ch);
// } else {
// if (circle_follow(ch, leader)) {
// send_to_char(ch, "Sorry, but following in loops is not allowed.\r\n");
// return;
// }
// if (ch->master)
// stop_follower(ch);
// REMOVE_BIT(AFF_FLAGS(ch), AFF_GROUP);
// add_follower(ch, leader);
// }
// }
// }
