/* ************************************************************************
*   File: act.comm.c                                    Part of CircleMUD *
*  Usage: Player-level communication commands                             *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::rc::Rc;

use crate::config::{NOPERSON, OK};
use crate::db::DB;
use crate::handler::FIND_CHAR_WORLD;
use crate::interpreter::{delete_doubledollar, half_chop};
use crate::screen::{C_CMP, C_NRM, KNRM, KNUL, KRED};
use crate::structs::{
    CharData, AFF_GROUP, LVL_IMMORT, NOBODY, PLR_WRITING, PRF_COLOR_1, PRF_COLOR_2, PRF_NOREPEAT,
    PRF_NOTELL, ROOM_SOUNDPROOF,
};
use crate::{
    _clrlevel, clr, send_to_char, Game, CCNRM, CCRED, TO_CHAR, TO_ROOM, TO_SLEEP, TO_VICT,
};

#[allow(unused_variables)]
pub fn do_say(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let mut argument = argument.trim_start().to_string();

    if argument.is_empty() {
        send_to_char(ch, "Yes, but WHAT do you want to say?\r\n");
    } else {
        let buf = format!("$n says, '{}'", argument);
        let db = &game.db;
        db.act(&buf, false, Some(ch), None, None, TO_ROOM);

        if !ch.is_npc() && ch.prf_flagged(PRF_NOREPEAT) {
            send_to_char(ch, OK);
        } else {
            delete_doubledollar(&mut argument);
            send_to_char(ch, format!("You say, '{}'\r\n", argument).as_str());
        }
    }
}

#[allow(unused_variables)]
pub fn do_gsay(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let argument = argument.trim_start();

    if !ch.aff_flagged(AFF_GROUP) {
        send_to_char(ch, "But you are not the member of a group!\r\n");
        return;
    }
    if argument.is_empty() {
        send_to_char(ch, "Yes, but WHAT do you want to group-say?\r\n");
    } else {
        let k;
        if ch.master.borrow().is_some() {
            k = ch.master.borrow().as_ref().unwrap().clone();
        } else {
            k = ch.clone();
        }

        let buf = format!("$n tells the group, '{}'", argument);
        let db = &game.db;
        if k.aff_flagged(AFF_GROUP) && !Rc::ptr_eq(&k, ch) {
            db.act(&buf, false, Some(ch), None, Some(&k), TO_VICT | TO_SLEEP);
        }
        for f in k.followers.borrow().iter() {
            if f.follower.aff_flagged(AFF_GROUP) && !Rc::ptr_eq(&f.follower, ch) {
                db.act(
                    &buf,
                    false,
                    Some(ch),
                    None,
                    Some(&f.follower),
                    TO_VICT | TO_SLEEP,
                );
            }
        }
        if ch.prf_flagged(PRF_NOREPEAT) {
            send_to_char(ch, OK);
        } else {
            send_to_char(
                ch,
                format!("You tell the group, '{}'\r\n", argument).as_str(),
            );
        }
    }
}

fn perform_tell(db: &DB, ch: &Rc<CharData>, vict: &Rc<CharData>, arg: &str) {
    send_to_char(vict, CCRED!(vict, C_NRM));
    let buf = format!("$n tells you, '{}'", arg);
    db.act(&buf, false, Some(ch), None, Some(vict), TO_VICT | TO_SLEEP);
    send_to_char(vict, CCNRM!(vict, C_NRM));

    if !ch.is_npc() && ch.prf_flagged(PRF_NOREPEAT) {
        send_to_char(ch, OK);
    } else {
        send_to_char(ch, CCRED!(ch, C_CMP));
        let buf = format!("You tell $N, '{}'", arg);
        db.act(&buf, false, Some(ch), None, Some(vict), TO_CHAR | TO_SLEEP);
        send_to_char(ch, CCNRM!(ch, C_CMP));
    }

    if !vict.is_npc() && !ch.is_npc() {
        vict.set_last_tell(ch.get_idnum());
    }
}

fn is_tell_ok(db: &DB, ch: &Rc<CharData>, vict: &Rc<CharData>) -> bool {
    if Rc::ptr_eq(ch, vict) {
        send_to_char(ch, "You try to tell yourself something.\r\n");
    } else if !ch.is_npc() && ch.prf_flagged(PRF_NOTELL) {
        send_to_char(
            ch,
            "You can't tell other people while you have notell on.\r\n",
        );
    } else if db.room_flagged(ch.in_room(), ROOM_SOUNDPROOF) {
        send_to_char(ch, "The walls seem to absorb your words.\r\n");
    } else if !vict.is_npc() && vict.desc.borrow().is_none() {
        /* linkless */
        db.act(
            "$E's linkless at the moment.",
            false,
            Some(ch),
            None,
            Some(vict),
            TO_CHAR | TO_SLEEP,
        );
    } else if vict.plr_flagged(PLR_WRITING) {
        db.act(
            "$E's writing a message right now; try again later.",
            false,
            Some(ch),
            None,
            Some(vict),
            TO_CHAR | TO_SLEEP,
        );
    } else if (!vict.is_npc() && vict.prf_flagged(PRF_NOTELL))
        || db.room_flagged(vict.in_room(), ROOM_SOUNDPROOF)
    {
        db.act(
            "$E can't hear you.",
            false,
            Some(ch),
            None,
            Some(vict),
            TO_CHAR | TO_SLEEP,
        );
    } else {
        return true;
    }

    return false;
}

/*
 * Yes, do_tell probably could be combined with whisper and ask, but
 * called frequently, and should IMHO be kept as tight as possible.
 */
#[allow(unused_variables)]
pub fn do_tell(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let mut buf = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();
    half_chop(&mut argument, &mut buf, &mut buf2);
    let mut vict = None;
    let db = &game.db;
    if buf.is_empty() || buf2.is_empty() {
        send_to_char(ch, "Who do you wish to tell what??\r\n");
    } else if ch.get_level() < LVL_IMMORT as u8 && {
        vict = db.get_player_vis(ch, &mut buf, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(ch, NOPERSON);
    } else if ch.get_level() >= LVL_IMMORT as u8 && {
        vict = db.get_char_vis(ch, &mut buf, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(ch, NOPERSON);
    } else if is_tell_ok(db, ch, vict.as_ref().unwrap()) {
        perform_tell(db, ch, vict.as_ref().unwrap(), &buf2);
    }
}

#[allow(unused_variables)]
pub fn do_reply(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    if ch.is_npc() {
        return;
    }

    let argument = argument.trim_start();

    if ch.get_last_tell() == NOBODY as i64 {
        send_to_char(ch, "You have nobody to reply to!\r\n");
    } else if argument.is_empty() {
        send_to_char(ch, "What is your reply?\r\n");
    } else {
        /*
         * Make sure the person you're replying to is still playing by searching
         * for them.  Note, now last tell is stored as player IDnum instead of
         * a pointer, which is much better because it's safer, plus will still
         * work if someone logs out and back in again.
         */

        /*
         * XXX: A descriptor list based search would be faster although
         *      we could not find link dead people.  Not that they can
         *      hear tells anyway. :) -gg 2/24/98
         */
        let db = &game.db;
        let ch_list = db.character_list.borrow();
        let tch = ch_list
            .iter()
            .find(|c| !c.is_npc() && c.get_idnum() == ch.get_last_tell());

        if tch.is_none() {
            send_to_char(ch, "They are no longer playing.\r\n");
        } else if is_tell_ok(db, ch, tch.as_ref().unwrap()) {
            perform_tell(db, ch, tch.as_ref().unwrap(), argument);
        }
    }
}

// ACMD(do_spec_comm)
// {
// char buf[MAX_INPUT_LENGTH], buf2[MAX_INPUT_LENGTH];
// struct char_data *vict;
// const char *action_sing, *action_plur, *action_others;
//
// switch (subcmd) {
// case SCMD_WHISPER:
// action_sing = "whisper to";
// action_plur = "whispers to";
// action_others = "$n whispers something to $N.";
// break;
//
// case SCMD_ASK:
// action_sing = "ask";
// action_plur = "asks";
// action_others = "$n asks $N a question.";
// break;
//
// default:
// action_sing = "oops";
// action_plur = "oopses";
// action_others = "$n is tongue-tied trying to speak with $N.";
// break;
// }
//
// half_chop(argument, buf, buf2);
//
// if (!*buf || !*buf2)
// send_to_char(ch, "Whom do you want to %s.. and what??\r\n", action_sing);
// else if (!(vict = get_char_vis(ch, buf, None, FIND_CHAR_ROOM)))
// send_to_char(ch, "%s", NOPERSON);
// else if (vict == ch)
// send_to_char(ch, "You can't get your mouth close enough to your ear...\r\n");
// else {
// char buf1[MAX_STRING_LENGTH];
//
// snprintf(buf1, sizeof(buf1), "$n %s you, '%s'", action_plur, buf2);
// act(buf1, false, ch, 0, vict, TO_VICT);
//
// if (PRF_FLAGGED(ch, PRF_NOREPEAT))
// send_to_char(ch, "%s", OK);
// else
// send_to_char(ch, "You %s %s, '%s'\r\n", action_sing, GET_NAME(vict), buf2);
// act(action_others, false, ch, 0, vict, TO_NOTVICT);
// }
// }
//
//
// /*
//  * buf1, buf2 = MAX_OBJECT_NAME_LENGTH
//  *	(if it existed)
//  */
// ACMD(do_write)
// {
// struct obj_data *paper, *pen = None;
// char *papername, *penname;
// char buf1[MAX_STRING_LENGTH], buf2[MAX_STRING_LENGTH];
//
// papername = buf1;
// penname = buf2;
//
// two_arguments(argument, papername, penname);
//
// if (!ch->desc)
// return;
//
// if (!*papername) {		/* nothing was delivered */
// send_to_char(ch, "Write?  With what?  ON what?  What are you trying to do?!?\r\n");
// return;
// }
// if (*penname) {		/* there were two arguments */
// if (!(paper = get_obj_in_list_vis(ch, papername, None, ch->carrying))) {
// send_to_char(ch, "You have no %s.\r\n", papername);
// return;
// }
// if (!(pen = get_obj_in_list_vis(ch, penname, None, ch->carrying))) {
// send_to_char(ch, "You have no %s.\r\n", penname);
// return;
// }
// } else {		/* there was one arg.. let's see what we can find */
// if (!(paper = get_obj_in_list_vis(ch, papername, None, ch->carrying))) {
// send_to_char(ch, "There is no %s in your inventory.\r\n", papername);
// return;
// }
// if (GET_OBJ_TYPE(paper) == ITEM_PEN) {	/* oops, a pen.. */
// pen = paper;
// paper = None;
// } else if (GET_OBJ_TYPE(paper) != ITEM_NOTE) {
// send_to_char(ch, "That thing has nothing to do with writing.\r\n");
// return;
// }
// /* One object was found.. now for the other one. */
// if (!GET_EQ(ch, WEAR_HOLD)) {
// send_to_char(ch, "You can't write with %s %s alone.\r\n", AN(papername), papername);
// return;
// }
// if (!CAN_SEE_OBJ(ch, GET_EQ(ch, WEAR_HOLD))) {
// send_to_char(ch, "The stuff in your hand is invisible!  Yeech!!\r\n");
// return;
// }
// if (pen)
// paper = GET_EQ(ch, WEAR_HOLD);
// else
// pen = GET_EQ(ch, WEAR_HOLD);
// }
//
//
// /* ok.. now let's see what kind of stuff we've found */
// if (GET_OBJ_TYPE(pen) != ITEM_PEN)
// act("$p is no good for writing with.", false, ch, pen, 0, TO_CHAR);
// else if (GET_OBJ_TYPE(paper) != ITEM_NOTE)
// act("You can't write on $p.", false, ch, paper, 0, TO_CHAR);
// else if (paper->action_description)
// send_to_char(ch, "There's something written on it already.\r\n");
// else {
// /* we can write - hooray! */
// send_to_char(ch, "Write your note.  End with '@' on a new line.\r\n");
// act("$n begins to jot down a note.", true, ch, 0, 0, TO_ROOM);
// string_write(ch->desc, &paper->action_description, MAX_NOTE_LENGTH, 0, None);
// }
// }
//
//
//
// ACMD(do_page)
// {
// struct descriptor_data *d;
// struct char_data *vict;
// char buf2[MAX_INPUT_LENGTH], arg[MAX_INPUT_LENGTH];
//
// half_chop(argument, arg, buf2);
//
// if (IS_NPC(ch))
// send_to_char(ch, "Monsters can't page.. go away.\r\n");
// else if (!*arg)
// send_to_char(ch, "Whom do you wish to page?\r\n");
// else {
// char buf[MAX_STRING_LENGTH];
//
// snprintf(buf, sizeof(buf), "\007\007*$n* %s", buf2);
// if (!str_cmp(arg, "all")) {
// if (GET_LEVEL(ch) > LVL_GOD) {
// for (d = descriptor_list; d; d = d->next)
// if (STATE(d) == CON_PLAYING && d->character)
// act(buf, false, ch, 0, d->character, TO_VICT);
// } else
// send_to_char(ch, "You will never be godly enough to do that!\r\n");
// return;
// }
// if ((vict = get_char_vis(ch, arg, None, FIND_CHAR_WORLD)) != None) {
// act(buf, false, ch, 0, vict, TO_VICT);
// if (PRF_FLAGGED(ch, PRF_NOREPEAT))
// send_to_char(ch, "%s", OK);
// else
// act(buf, false, ch, 0, vict, TO_CHAR);
// } else
// send_to_char(ch, "There is no such person in the game!\r\n");
// }
// }
//
//
// /**********************************************************************
//  * generalized communication func, originally by Fred C. Merkel (Torg) *
//   *********************************************************************/
//
// ACMD(do_gen_comm)
// {
// struct descriptor_data *i;
// char color_on[24];
// char buf1[MAX_INPUT_LENGTH];
//
// /* Array of flags which must _not_ be set in order for comm to be heard */
// int channels[] = {
// 0,
// PRF_DEAF,
// PRF_NOGOSS,
// PRF_NOAUCT,
// PRF_NOGRATZ,
// 0
// };
//
// /*
//  * com_msgs: [0] Message if you can't perform the action because of noshout
//  *           [1] name of the action
//  *           [2] message if you're not on the channel
//  *           [3] a color string.
//  */
// const char *com_msgs[][4] = {
// {"You cannot holler!!\r\n",
// "holler",
// "",
// KYEL},
//
// {"You cannot shout!!\r\n",
// "shout",
// "Turn off your noshout flag first!\r\n",
// KYEL},
//
// {"You cannot gossip!!\r\n",
// "gossip",
// "You aren't even on the channel!\r\n",
// KYEL},
//
// {"You cannot auction!!\r\n",
// "auction",
// "You aren't even on the channel!\r\n",
// KMAG},
//
// {"You cannot congratulate!\r\n",
// "congrat",
// "You aren't even on the channel!\r\n",
// KGRN}
// };
//
// /* to keep pets, etc from being ordered to shout */
// if (!ch->desc)
// return;
//
// if (PLR_FLAGGED(ch, PLR_NOSHOUT)) {
// send_to_char(ch, "%s", com_msgs[subcmd][0]);
// return;
// }
// if (ROOM_FLAGGED(IN_ROOM(ch), ROOM_SOUNDPROOF)) {
// send_to_char(ch, "The walls seem to absorb your words.\r\n");
// return;
// }
// /* level_can_shout defined in config.c */
// if (GET_LEVEL(ch) < level_can_shout) {
// send_to_char(ch, "You must be at least level %d before you can %s.\r\n", level_can_shout, com_msgs[subcmd][1]);
// return;
// }
// /* make sure the char is on the channel */
// if (PRF_FLAGGED(ch, channels[subcmd])) {
// send_to_char(ch, "%s", com_msgs[subcmd][2]);
// return;
// }
// /* skip leading spaces */
// skip_spaces(&argument);
//
// /* make sure that there is something there to say! */
// if (!*argument) {
// send_to_char(ch, "Yes, %s, fine, %s we must, but WHAT???\r\n", com_msgs[subcmd][1], com_msgs[subcmd][1]);
// return;
// }
// if (subcmd == SCMD_HOLLER) {
// if (GET_MOVE(ch) < holler_move_cost) {
// send_to_char(ch, "You're too exhausted to holler.\r\n");
// return;
// } else
// GET_MOVE(ch) -= holler_move_cost;
// }
// /* set up the color on code */
// strlcpy(color_on, com_msgs[subcmd][3], sizeof(color_on));
//
// /* first, set up strings to be given to the communicator */
// if (PRF_FLAGGED(ch, PRF_NOREPEAT))
// send_to_char(ch, "%s", OK);
// else
// send_to_char(ch, "%sYou %s, '%s'%s\r\n", COLOR_LEV(ch) >= C_CMP ? color_on : "", com_msgs[subcmd][1], argument, CCNRM(ch, C_CMP));
//
// snprintf(buf1, sizeof(buf1), "$n %ss, '%s'", com_msgs[subcmd][1], argument);
//
// /* now send all the strings out */
// for (i = descriptor_list; i; i = i->next) {
// if (STATE(i) == CON_PLAYING && i != ch->desc && i->character &&
// !PRF_FLAGGED(i->character, channels[subcmd]) &&
// !PLR_FLAGGED(i->character, PLR_WRITING) &&
// !ROOM_FLAGGED(IN_ROOM(i->character), ROOM_SOUNDPROOF)) {
//
// if (subcmd == SCMD_SHOUT &&
// ((world[IN_ROOM(ch)].zone != world[IN_ROOM(i->character)].zone) ||
// !AWAKE(i->character)))
// continue;
//
// if (COLOR_LEV(i->character) >= C_NRM)
// send_to_char(i->character, "%s", color_on);
// act(buf1, false, ch, 0, i->character, TO_VICT | TO_SLEEP);
// if (COLOR_LEV(i->character) >= C_NRM)
// send_to_char(i->character, "%s", KNRM);
// }
// }
// }
//
//
// ACMD(do_qcomm)
// {
// if (!PRF_FLAGGED(ch, PRF_QUEST)) {
// send_to_char(ch, "You aren't even part of the quest!\r\n");
// return;
// }
// skip_spaces(&argument);
//
// if (!*argument)
// send_to_char(ch, "%c%s?  Yes, fine, %s we must, but WHAT??\r\n", UPPER(*CMD_NAME), CMD_NAME + 1, CMD_NAME);
// else {
// char buf[MAX_STRING_LENGTH];
// struct descriptor_data *i;
//
// if (PRF_FLAGGED(ch, PRF_NOREPEAT))
// send_to_char(ch, "%s", OK);
// else if (subcmd == SCMD_QSAY) {
// snprintf(buf, sizeof(buf), "You quest-say, '%s'", argument);
// act(buf, false, ch, 0, argument, TO_CHAR);
// } else
// act(argument, false, ch, 0, argument, TO_CHAR);
//
// if (subcmd == SCMD_QSAY)
// snprintf(buf, sizeof(buf), "$n quest-says, '%s'", argument);
// else
// strlcpy(buf, argument, sizeof(buf));
//
// for (i = descriptor_list; i; i = i->next)
// if (STATE(i) == CON_PLAYING && i != ch->desc && PRF_FLAGGED(i->character, PRF_QUEST))
// act(buf, 0, ch, 0, i->character, TO_VICT | TO_SLEEP);
// }
// }
