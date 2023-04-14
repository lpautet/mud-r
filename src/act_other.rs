/* ************************************************************************
*   File: act.other.c                                   Part of CircleMUD *
*  Usage: Miscellaneous player-level commands                             *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::cmp::max;
use std::rc::Rc;

use log::error;

use crate::config::{FREE_RENT, OK};
use crate::fight::die;
use crate::interpreter::{
    one_argument, SCMD_AUTOEXIT, SCMD_BRIEF, SCMD_COMPACT, SCMD_DEAF, SCMD_HOLYLIGHT,
    SCMD_NOAUCTION, SCMD_NOGOSSIP, SCMD_NOGRATZ, SCMD_NOHASSLE, SCMD_NOREPEAT, SCMD_NOSUMMON,
    SCMD_NOTELL, SCMD_NOWIZ, SCMD_QUEST, SCMD_QUIT, SCMD_ROOMFLAGS, SCMD_SLOWNS, SCMD_TRACK,
};
use crate::objsave::crash_rentsave;
use crate::spec_procs::list_skills;
use crate::structs::{
    CharData, LVL_IMMORT, POS_FIGHTING, POS_STUNNED, PRF_AUTOEXIT, PRF_BRIEF, PRF_COMPACT,
    PRF_DEAF, PRF_DISPAUTO, PRF_DISPHP, PRF_DISPMANA, PRF_DISPMOVE, PRF_HOLYLIGHT, PRF_NOAUCT,
    PRF_NOGOSS, PRF_NOGRATZ, PRF_NOHASSLE, PRF_NOREPEAT, PRF_NOTELL, PRF_NOWIZ, PRF_QUEST,
    PRF_ROOMFLAGS, PRF_SUMMONABLE,
};
use crate::util::NRM;
use crate::{send_to_char, Game, TO_ROOM};

#[allow(unused_variables)]
pub fn do_quit(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    if ch.is_npc() || ch.desc.borrow().is_none() {
        return;
    }

    if subcmd != SCMD_QUIT && ch.get_level() < LVL_IMMORT as u8 {
        send_to_char(ch, "You have to type quit--no less, to quit!\r\n");
    } else if ch.get_pos() == POS_FIGHTING {
        send_to_char(ch, "No way!  You're fighting for your life!\r\n");
    } else if ch.get_pos() < POS_STUNNED {
        send_to_char(ch, "You die before your time...\r\n");
        die(ch, game);
    } else {
        game.db
            .act("$n has left the game.", true, Some(ch), None, None, TO_ROOM);
        game.mudlog(
            NRM,
            max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
            true,
            format!("{} has quit the game.", ch.get_name()).as_str(),
        );
        send_to_char(ch, "Goodbye, friend.. Come back soon!\r\n");

        /*  We used to check here for duping attempts, but we may as well
         *  do it right in extract_char(), since there is no check if a
         *  player rents out and it can leave them in an equally screwy
         *  situation.
         */

        if FREE_RENT {
            crash_rentsave(&game.db, ch, 0);
        }

        // TODO implement houses
        /* If someone is quitting in their house, let them load back here. */
        // if (!PLR_FLAGGED(ch, PLR_LOADROOM) && ROOM_FLAGGED(IN_ROOM(ch), ROOM_HOUSE))
        // GET_LOADROOM(ch) = GET_ROOM_VNUM(IN_ROOM(ch));

        game.db.extract_char(ch); /* Char is saved before extracting. */
    }
}

// ACMD(do_save)
// {
// if (IS_NPC(ch) || !ch->desc)
// return;
//
// /* Only tell the char we're saving if they actually typed "save" */
// if (cmd) {
// /*
//  * This prevents item duplication by two PC's using coordinated saves
//  * (or one PC with a house) and system crashes. Note that houses are
//  * still automatically saved without this enabled. This code assumes
//  * that guest immortals aren't trustworthy. If you've disabled guest
//  * immortal advances from mortality, you may want < instead of <=.
//  */
// if (AUTO_SAVE && GET_LEVEL(ch) <= LVL_IMMORT) {
// send_to_char(ch, "Saving aliases.\r\n");
// write_aliases(ch);
// return;
// }
// send_to_char(ch, "Saving %s and aliases.\r\n", GET_NAME(ch));
// }
//
// write_aliases(ch);
// save_char(ch);
// Crash_crashsave(ch);
// if (ROOM_FLAGGED(IN_ROOM(ch), ROOM_HOUSE_CRASH))
// House_crashsave(GET_ROOM_VNUM(IN_ROOM(ch)));
// }

/* generic function for commands which are normally overridden by
special procedures - i.e., shop commands, mail commands, etc. */
#[allow(unused_variables)]
pub fn do_not_here(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    send_to_char(ch, "Sorry, but you cannot do that here!\r\n");
}

// ACMD(do_sneak)
// {
// struct affected_type af;
// byte percent;
//
// if (IS_NPC(ch) || !GET_SKILL(ch, SKILL_SNEAK)) {
// send_to_char(ch, "You have no idea how to do that.\r\n");
// return;
// }
// send_to_char(ch, "Okay, you'll try to move silently for a while.\r\n");
// if (AFF_FLAGGED(ch, AFF_SNEAK))
// affect_from_char(ch, SKILL_SNEAK);
//
// percent = rand_number(1, 101);	/* 101% is a complete failure */
//
// if (percent > GET_SKILL(ch, SKILL_SNEAK) + dex_app_skill[GET_DEX(ch)].sneak)
// return;
//
// af.type = SKILL_SNEAK;
// af.duration = GET_LEVEL(ch);
// af.modifier = 0;
// af.location = APPLY_NONE;
// af.bitvector = AFF_SNEAK;
// affect_to_char(ch, &af);
// }
//
//
//
// ACMD(do_hide)
// {
// byte percent;
//
// if (IS_NPC(ch) || !GET_SKILL(ch, SKILL_HIDE)) {
// send_to_char(ch, "You have no idea how to do that.\r\n");
// return;
// }
//
// send_to_char(ch, "You attempt to hide yourself.\r\n");
//
// if (AFF_FLAGGED(ch, AFF_HIDE))
// REMOVE_BIT(AFF_FLAGS(ch), AFF_HIDE);
//
// percent = rand_number(1, 101);	/* 101% is a complete failure */
//
// if (percent > GET_SKILL(ch, SKILL_HIDE) + dex_app_skill[GET_DEX(ch)].hide)
// return;
//
// SET_BIT(AFF_FLAGS(ch), AFF_HIDE);
// }
//
//
//
//
// ACMD(do_steal)
// {
// struct char_data *vict;
// struct obj_data *obj;
// char vict_name[MAX_INPUT_LENGTH], obj_name[MAX_INPUT_LENGTH];
// int percent, gold, eq_pos, pcsteal = 0, ohoh = 0;
//
// if (IS_NPC(ch) || !GET_SKILL(ch, SKILL_STEAL)) {
// send_to_char(ch, "You have no idea how to do that.\r\n");
// return;
// }
// if (ROOM_FLAGGED(IN_ROOM(ch), ROOM_PEACEFUL)) {
// send_to_char(ch, "This room just has such a peaceful, easy feeling...\r\n");
// return;
// }
//
// two_arguments(argument, obj_name, vict_name);
//
// if (!(vict = get_char_vis(ch, vict_name, NULL, FIND_CHAR_ROOM))) {
// send_to_char(ch, "Steal what from who?\r\n");
// return;
// } else if (vict == ch) {
// send_to_char(ch, "Come on now, that's rather stupid!\r\n");
// return;
// }
//
// /* 101% is a complete failure */
// percent = rand_number(1, 101) - dex_app_skill[GET_DEX(ch)].p_pocket;
//
// if (GET_POS(vict) < POS_SLEEPING)
// percent = -1;		/* ALWAYS SUCCESS, unless heavy object. */
//
// if (!pt_allowed && !IS_NPC(vict))
// pcsteal = 1;
//
// if (!AWAKE(vict))	/* Easier to steal from sleeping people. */
// percent -= 50;
//
// /* NO NO With Imp's and Shopkeepers, and if player thieving is not allowed */
// if (GET_LEVEL(vict) >= LVL_IMMORT || pcsteal ||
// GET_MOB_SPEC(vict) == shop_keeper)
// percent = 101;		/* Failure */
//
// if (str_cmp(obj_name, "coins") && str_cmp(obj_name, "gold")) {
//
// if (!(obj = get_obj_in_list_vis(ch, obj_name, NULL, vict->carrying))) {
//
// for (eq_pos = 0; eq_pos < NUM_WEARS; eq_pos++)
// if (GET_EQ(vict, eq_pos) &&
// (isname(obj_name, GET_EQ(vict, eq_pos)->name)) &&
// CAN_SEE_OBJ(ch, GET_EQ(vict, eq_pos))) {
// obj = GET_EQ(vict, eq_pos);
// }
// }
// if (!obj) {
// act("$E hasn't got that item.", FALSE, ch, 0, vict, TO_CHAR);
// return;
// } else {			/* It is equipment */
// if ((GET_POS(vict) > POS_STUNNED)) {
// send_to_char(ch, "Steal the equipment now?  Impossible!\r\n");
// return;
// } else {
// act("You unequip $p and steal it.", FALSE, ch, obj, 0, TO_CHAR);
// act("$n steals $p from $N.", FALSE, ch, obj, vict, TO_NOTVICT);
// obj_to_char(unequip_char(vict, eq_pos), ch);
// }
// }
// } else {			/* obj found in inventory */
//
// percent += GET_OBJ_WEIGHT(obj);	/* Make heavy harder */
//
// if (percent > GET_SKILL(ch, SKILL_STEAL)) {
// ohoh = TRUE;
// send_to_char(ch, "Oops..\r\n");
// act("$n tried to steal something from you!", FALSE, ch, 0, vict, TO_VICT);
// act("$n tries to steal something from $N.", TRUE, ch, 0, vict, TO_NOTVICT);
// } else {			/* Steal the item */
// if (IS_CARRYING_N(ch) + 1 < CAN_CARRY_N(ch)) {
// if (IS_CARRYING_W(ch) + GET_OBJ_WEIGHT(obj) < CAN_CARRY_W(ch)) {
// obj_from_char(obj);
// obj_to_char(obj, ch);
// send_to_char(ch, "Got it!\r\n");
// }
// } else
// send_to_char(ch, "You cannot carry that much.\r\n");
// }
// }
// } else {			/* Steal some coins */
// if (AWAKE(vict) && (percent > GET_SKILL(ch, SKILL_STEAL))) {
// ohoh = TRUE;
// send_to_char(ch, "Oops..\r\n");
// act("You discover that $n has $s hands in your wallet.", FALSE, ch, 0, vict, TO_VICT);
// act("$n tries to steal gold from $N.", TRUE, ch, 0, vict, TO_NOTVICT);
// } else {
// /* Steal some gold coins */
// gold = (GET_GOLD(vict) * rand_number(1, 10)) / 100;
// gold = MIN(1782, gold);
// if (gold > 0) {
// GET_GOLD(ch) += gold;
// GET_GOLD(vict) -= gold;
// if (gold > 1)
// send_to_char(ch, "Bingo!  You got %d gold coins.\r\n", gold);
// else
// send_to_char(ch, "You manage to swipe a solitary gold coin.\r\n");
// } else {
// send_to_char(ch, "You couldn't get any gold...\r\n");
// }
// }
// }
//
// if (ohoh && IS_NPC(vict) && AWAKE(vict))
// hit(vict, ch, TYPE_UNDEFINED);
// }

#[allow(unused_variables)]
pub fn do_practice(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    if ch.is_npc() {
        return;
    }
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if !arg.is_empty() {
        send_to_char(ch, "You can only practice skills in your guild.\r\n");
    } else {
        list_skills(&game.db, ch);
    }
}

// ACMD(do_visible)
// {
// if (GET_LEVEL(ch) >= LVL_IMMORT) {
// perform_immort_vis(ch);
// return;
// }
//
// if AFF_FLAGGED(ch, AFF_INVISIBLE) {
// appear(ch);
// send_to_char(ch, "You break the spell of invisibility.\r\n");
// } else
// send_to_char(ch, "You are already visible.\r\n");
// }
//
//
//
// ACMD(do_title)
// {
// skip_spaces(&argument);
// delete_doubledollar(argument);
//
// if (IS_NPC(ch))
// send_to_char(ch, "Your title is fine... go away.\r\n");
// else if (PLR_FLAGGED(ch, PLR_NOTITLE))
// send_to_char(ch, "You can't title yourself -- you shouldn't have abused it!\r\n");
// else if (strstr(argument, "(") || strstr(argument, ")"))
// send_to_char(ch, "Titles can't contain the ( or ) characters.\r\n");
// else if (strlen(argument) > MAX_TITLE_LENGTH)
// send_to_char(ch, "Sorry, titles can't be longer than %d characters.\r\n", MAX_TITLE_LENGTH);
// else {
// set_title(ch, argument);
// send_to_char(ch, "Okay, you're now %s %s.\r\n", GET_NAME(ch), GET_TITLE(ch));
// }
// }
//
//
// int perform_group(struct char_data *ch, struct char_data *vict)
// {
// if (AFF_FLAGGED(vict, AFF_GROUP) || !CAN_SEE(ch, vict))
// return (0);
//
// SET_BIT(AFF_FLAGS(vict), AFF_GROUP);
// if (ch != vict)
// act("$N is now a member of your group.", FALSE, ch, 0, vict, TO_CHAR);
// act("You are now a member of $n's group.", FALSE, ch, 0, vict, TO_VICT);
// act("$N is now a member of $n's group.", FALSE, ch, 0, vict, TO_NOTVICT);
// return (1);
// }
//
//
// void print_group(struct char_data *ch)
// {
// struct char_data *k;
// struct follow_type *f;
//
// if (!AFF_FLAGGED(ch, AFF_GROUP))
// send_to_char(ch, "But you are not the member of a group!\r\n");
// else {
// char buf[MAX_STRING_LENGTH];
//
// send_to_char(ch, "Your group consists of:\r\n");
//
// k = (ch->master ? ch->master : ch);
//
// if (AFF_FLAGGED(k, AFF_GROUP)) {
// snprintf(buf, sizeof(buf), "     [%3dH %3dM %3dV] [%2d %s] $N (Head of group)",
// GET_HIT(k), GET_MANA(k), GET_MOVE(k), GET_LEVEL(k), CLASS_ABBR(k));
// act(buf, FALSE, ch, 0, k, TO_CHAR);
// }
//
// for (f = k->followers; f; f = f->next) {
// if (!AFF_FLAGGED(f->follower, AFF_GROUP))
// continue;
//
// snprintf(buf, sizeof(buf), "     [%3dH %3dM %3dV] [%2d %s] $N", GET_HIT(f->follower),
// GET_MANA(f->follower), GET_MOVE(f->follower),
// GET_LEVEL(f->follower), CLASS_ABBR(f->follower));
// act(buf, FALSE, ch, 0, f->follower, TO_CHAR);
// }
// }
// }
//
//
//
// ACMD(do_group)
// {
// char buf[MAX_STRING_LENGTH];
// struct char_data *vict;
// struct follow_type *f;
// int found;
//
// one_argument(argument, buf);
//
// if (!*buf) {
// print_group(ch);
// return;
// }
//
// if (ch->master) {
// act("You can not enroll group members without being head of a group.",
// FALSE, ch, 0, 0, TO_CHAR);
// return;
// }
//
// if (!str_cmp(buf, "all")) {
// perform_group(ch, ch);
// for (found = 0, f = ch->followers; f; f = f->next)
// found += perform_group(ch, f->follower);
// if (!found)
// send_to_char(ch, "Everyone following you is already in your group.\r\n");
// return;
// }
//
// if (!(vict = get_char_vis(ch, buf, NULL, FIND_CHAR_ROOM)))
// send_to_char(ch, "%s", NOPERSON);
// else if ((vict->master != ch) && (vict != ch))
// act("$N must follow you to enter your group.", FALSE, ch, 0, vict, TO_CHAR);
// else {
// if (!AFF_FLAGGED(vict, AFF_GROUP))
// perform_group(ch, vict);
// else {
// if (ch != vict)
// act("$N is no longer a member of your group.", FALSE, ch, 0, vict, TO_CHAR);
// act("You have been kicked out of $n's group!", FALSE, ch, 0, vict, TO_VICT);
// act("$N has been kicked out of $n's group!", FALSE, ch, 0, vict, TO_NOTVICT);
// REMOVE_BIT(AFF_FLAGS(vict), AFF_GROUP);
// }
// }
// }
//
//
//
// ACMD(do_ungroup)
// {
// char buf[MAX_INPUT_LENGTH];
// struct follow_type *f, *next_fol;
// struct char_data *tch;
//
// one_argument(argument, buf);
//
// if (!*buf) {
// if (ch->master || !(AFF_FLAGGED(ch, AFF_GROUP))) {
// send_to_char(ch, "But you lead no group!\r\n");
// return;
// }
//
// for (f = ch->followers; f; f = next_fol) {
// next_fol = f->next;
// if (AFF_FLAGGED(f->follower, AFF_GROUP)) {
// REMOVE_BIT(AFF_FLAGS(f->follower), AFF_GROUP);
// act("$N has disbanded the group.", TRUE, f->follower, NULL, ch, TO_CHAR);
// if (!AFF_FLAGGED(f->follower, AFF_CHARM))
// stop_follower(f->follower);
// }
// }
//
// REMOVE_BIT(AFF_FLAGS(ch), AFF_GROUP);
// send_to_char(ch, "You disband the group.\r\n");
// return;
// }
// if (!(tch = get_char_vis(ch, buf, NULL, FIND_CHAR_ROOM))) {
// send_to_char(ch, "There is no such person!\r\n");
// return;
// }
// if (tch->master != ch) {
// send_to_char(ch, "That person is not following you!\r\n");
// return;
// }
//
// if (!AFF_FLAGGED(tch, AFF_GROUP)) {
// send_to_char(ch, "That person isn't in your group.\r\n");
// return;
// }
//
// REMOVE_BIT(AFF_FLAGS(tch), AFF_GROUP);
//
// act("$N is no longer a member of your group.", FALSE, ch, 0, tch, TO_CHAR);
// act("You have been kicked out of $n's group!", FALSE, ch, 0, tch, TO_VICT);
// act("$N has been kicked out of $n's group!", FALSE, ch, 0, tch, TO_NOTVICT);
//
// if (!AFF_FLAGGED(tch, AFF_CHARM))
// stop_follower(tch);
// }
//
//
//
//
// ACMD(do_report)
// {
// char buf[MAX_STRING_LENGTH];
// struct char_data *k;
// struct follow_type *f;
//
// if (!AFF_FLAGGED(ch, AFF_GROUP)) {
// send_to_char(ch, "But you are not a member of any group!\r\n");
// return;
// }
//
// snprintf(buf, sizeof(buf), "$n reports: %d/%dH, %d/%dM, %d/%dV\r\n",
// GET_HIT(ch), GET_MAX_HIT(ch),
// GET_MANA(ch), GET_MAX_MANA(ch),
// GET_MOVE(ch), GET_MAX_MOVE(ch));
//
// k = (ch->master ? ch->master : ch);
//
// for (f = k->followers; f; f = f->next)
// if (AFF_FLAGGED(f->follower, AFF_GROUP) && f->follower != ch)
// act(buf, TRUE, ch, NULL, f->follower, TO_VICT);
//
// if (k != ch)
// act(buf, TRUE, ch, NULL, k, TO_VICT);
//
// send_to_char(ch, "You report to the group.\r\n");
// }
//
//
//
// ACMD(do_split)
// {
// char buf[MAX_INPUT_LENGTH];
// int amount, num, share, rest;
// size_t len;
// struct char_data *k;
// struct follow_type *f;
//
// if (IS_NPC(ch))
// return;
//
// one_argument(argument, buf);
//
// if (is_number(buf)) {
// amount = atoi(buf);
// if (amount <= 0) {
// send_to_char(ch, "Sorry, you can't do that.\r\n");
// return;
// }
// if (amount > GET_GOLD(ch)) {
// send_to_char(ch, "You don't seem to have that much gold to split.\r\n");
// return;
// }
// k = (ch->master ? ch->master : ch);
//
// if (AFF_FLAGGED(k, AFF_GROUP) && (IN_ROOM(k) == IN_ROOM(ch)))
// num = 1;
// else
// num = 0;
//
// for (f = k->followers; f; f = f->next)
// if (AFF_FLAGGED(f->follower, AFF_GROUP) &&
// (!IS_NPC(f->follower)) &&
// (IN_ROOM(f->follower) == IN_ROOM(ch)))
// num++;
//
// if (num && AFF_FLAGGED(ch, AFF_GROUP)) {
// share = amount / num;
// rest = amount % num;
// } else {
// send_to_char(ch, "With whom do you wish to share your gold?\r\n");
// return;
// }
//
// GET_GOLD(ch) -= share * (num - 1);
//
// /* Abusing signed/unsigned to make sizeof work. */
// len = snprintf(buf, sizeof(buf), "%s splits %d coins; you receive %d.\r\n",
// GET_NAME(ch), amount, share);
// if (rest && len < sizeof(buf)) {
// snprintf(buf + len, sizeof(buf) - len,
// "%d coin%s %s not splitable, so %s keeps the money.\r\n", rest,
// (rest == 1) ? "" : "s", (rest == 1) ? "was" : "were", GET_NAME(ch));
// }
// if (AFF_FLAGGED(k, AFF_GROUP) && IN_ROOM(k) == IN_ROOM(ch) &&
// !IS_NPC(k) && k != ch) {
// GET_GOLD(k) += share;
// send_to_char(k, "%s", buf);
// }
//
// for (f = k->followers; f; f = f->next) {
// if (AFF_FLAGGED(f->follower, AFF_GROUP) &&
// (!IS_NPC(f->follower)) &&
// (IN_ROOM(f->follower) == IN_ROOM(ch)) &&
// f->follower != ch) {
//
// GET_GOLD(f->follower) += share;
// send_to_char(f->follower, "%s", buf);
// }
// }
// send_to_char(ch, "You split %d coins among %d members -- %d coins each.\r\n",
// amount, num, share);
//
// if (rest) {
// send_to_char(ch, "%d coin%s %s not splitable, so you keep the money.\r\n",
// rest, (rest == 1) ? "" : "s", (rest == 1) ? "was" : "were");
// GET_GOLD(ch) += rest;
// }
// } else {
// send_to_char(ch, "How many coins do you wish to split with your group?\r\n");
// return;
// }
// }
//
//
//
// ACMD(do_use)
// {
// char buf[MAX_INPUT_LENGTH], arg[MAX_INPUT_LENGTH];
// struct obj_data *mag_item;
//
// half_chop(argument, arg, buf);
// if (!*arg) {
// send_to_char(ch, "What do you want to %s?\r\n", CMD_NAME);
// return;
// }
// mag_item = GET_EQ(ch, WEAR_HOLD);
//
// if (!mag_item || !isname(arg, mag_item->name)) {
// switch (subcmd) {
// SCMD_RECITE => {
// SCMD_QUAFF => {
// if (!(mag_item = get_obj_in_list_vis(ch, arg, NULL, ch->carrying))) {
// send_to_char(ch, "You don't seem to have %s %s.\r\n", AN(arg), arg);
// return;
// }
// }
// SCMD_USE => {
// send_to_char(ch, "You don't seem to be holding %s %s.\r\n", AN(arg), arg);
// return;
// default:
// log("SYSERR: Unknown subcmd %d passed to do_use.", subcmd);
// return;
// }
// }
// switch (subcmd) {
// SCMD_QUAFF => {
// if (GET_OBJ_TYPE(mag_item) != ITEM_POTION) {
// send_to_char(ch, "You can only quaff potions.\r\n");
// return;
// }
// }
// SCMD_RECITE => {
// if (GET_OBJ_TYPE(mag_item) != ITEM_SCROLL) {
// send_to_char(ch, "You can only recite scrolls.\r\n");
// return;
// }
// }
// SCMD_USE => {
// if ((GET_OBJ_TYPE(mag_item) != ITEM_WAND) &&
// (GET_OBJ_TYPE(mag_item) != ITEM_STAFF)) {
// send_to_char(ch, "You can't seem to figure out how to use it.\r\n");
// return;
// }
// }
// }
//
// mag_objectmagic(ch, mag_item, buf);
// }
//
//
//
// ACMD(do_wimpy)
// {
// char arg[MAX_INPUT_LENGTH];
// int wimp_lev;
//
// /* 'wimp_level' is a player_special. -gg 2/25/98 */
// if (IS_NPC(ch))
// return;
//
// one_argument(argument, arg);
//
// if (!*arg) {
// if (GET_WIMP_LEV(ch)) {
// send_to_char(ch, "Your current wimp level is %d hit points.\r\n", GET_WIMP_LEV(ch));
// return;
// } else {
// send_to_char(ch, "At the moment, you're not a wimp.  (sure, sure...)\r\n");
// return;
// }
// }
// if (isdigit(*arg)) {
// if ((wimp_lev = atoi(arg)) != 0) {
// if (wimp_lev < 0)
// send_to_char(ch, "Heh, heh, heh.. we are jolly funny today, eh?\r\n");
// else if (wimp_lev > GET_MAX_HIT(ch))
// send_to_char(ch, "That doesn't make much sense, now does it?\r\n");
// else if (wimp_lev > (GET_MAX_HIT(ch) / 2))
// send_to_char(ch, "You can't set your wimp level above half your hit points.\r\n");
// else {
// send_to_char(ch, "Okay, you'll wimp out if you drop below %d hit points.\r\n", wimp_lev);
// GET_WIMP_LEV(ch) = wimp_lev;
// }
// } else {
// send_to_char(ch, "Okay, you'll now tough out fights to the bitter end.\r\n");
// GET_WIMP_LEV(ch) = 0;
// }
// } else
// send_to_char(ch, "Specify at how many hit points you want to wimp out at.  (0 to disable)\r\n");
// }

#[allow(unused_variables)]
pub fn do_display(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    if ch.is_npc() {
        send_to_char(ch, "Monsters don't need displays.  Go away.\r\n");
        return;
    }
    let argument = argument.trim_start();

    if argument.len() == 0 {
        send_to_char(
            ch,
            "Usage: prompt { { H | M | V } | all | auto | none }\r\n",
        );
        return;
    }

    if argument == "auto" {
        ch.toggle_prf_flag_bits(PRF_DISPAUTO);
        send_to_char(
            ch,
            format!(
                "Auto prompt {}abled.\r\n",
                if ch.prf_flagged(PRF_DISPAUTO) {
                    "en"
                } else {
                    "dis"
                }
            )
            .as_str(),
        );
        return;
    }

    if argument == "on" || argument == "all" {
        ch.set_prf_flags_bits(PRF_DISPHP | PRF_DISPMANA | PRF_DISPMOVE);
    } else if argument == "off" || argument == "none" {
        ch.remove_prf_flags_bits(PRF_DISPHP | PRF_DISPMANA | PRF_DISPMOVE);
    } else {
        ch.remove_prf_flags_bits(PRF_DISPHP | PRF_DISPMANA | PRF_DISPMOVE);

        for c in argument.chars() {
            match c.to_ascii_lowercase() {
                'h' => {
                    ch.set_prf_flags_bits(PRF_DISPHP);
                }
                'm' => {
                    ch.set_prf_flags_bits(PRF_DISPMANA);
                }
                'v' => {
                    ch.set_prf_flags_bits(PRF_DISPMOVE);
                }
                _ => {
                    send_to_char(
                        ch,
                        "Usage: prompt { { H | M | V } | all | auto | none }\r\n",
                    );
                    return;
                }
            }
        }
    }

    send_to_char(ch, OK);
}

// ACMD(do_gen_write)
// {
// FILE *fl;
// char *tmp;
// const char *filename;
// struct stat fbuf;
// time_t ct;
//
// switch (subcmd) {
// SCMD_BUG => {
// filename = BUG_FILE;
// }
// SCMD_TYPO => {
// filename = TYPO_FILE;
// }
// SCMD_IDEA => {
// filename = IDEA_FILE;
// }
// default:
// return;
// }
//
// ct = time(0);
// tmp = asctime(localtime(&ct));
//
// if (IS_NPC(ch)) {
// send_to_char(ch, "Monsters can't have ideas - Go away.\r\n");
// return;
// }
//
// skip_spaces(&argument);
// delete_doubledollar(argument);
//
// if (!*argument) {
// send_to_char(ch, "That must be a mistake...\r\n");
// return;
// }
// mudlog(CMP, LVL_IMMORT, FALSE, "%s %s: %s", GET_NAME(ch), CMD_NAME, argument);
//
// if (stat(filename, &fbuf) < 0) {
// perror("SYSERR: Can't stat() file");
// return;
// }
// if (fbuf.st_size >= max_filesize) {
// send_to_char(ch, "Sorry, the file is full right now.. try again later.\r\n");
// return;
// }
// if (!(fl = fopen(filename, "a"))) {
// perror("SYSERR: do_gen_write");
// send_to_char(ch, "Could not open the file.  Sorry.\r\n");
// return;
// }
// fprintf(fl, "%-8s (%6.6s) [%5d] %s\n", GET_NAME(ch), (tmp + 4),
// GET_ROOM_VNUM(IN_ROOM(ch)), argument);
// fclose(fl);
// send_to_char(ch, "Okay.  Thanks!\r\n");
// }

const TOG_ON: usize = 1;
const TOG_OFF: usize = 0;

macro_rules! prf_tog_chk {
    ($ch:expr, $flag:expr) => {
        ($ch.toggle_prf_flag_bits($flag) & $flag) != 0
    };
}

#[allow(unused_variables)]
pub fn do_gen_tog(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    const TOG_MESSAGES: [[&str; 2]; 17] = [
        [
            "You are now safe from summoning by other players.\r\n",
            "You may now be summoned by other players.\r\n",
        ],
        ["Nohassle disabled.\r\n", "Nohassle enabled.\r\n"],
        ["Brief mode off.\r\n", "Brief mode on.\r\n"],
        ["Compact mode off.\r\n", "Compact mode on.\r\n"],
        [
            "You can now hear tells.\r\n",
            "You are now deaf to tells.\r\n",
        ],
        [
            "You can now hear auctions.\r\n",
            "You are now deaf to auctions.\r\n",
        ],
        [
            "You can now hear shouts.\r\n",
            "You are now deaf to shouts.\r\n",
        ],
        [
            "You can now hear gossip.\r\n",
            "You are now deaf to gossip.\r\n",
        ],
        [
            "You can now hear the congratulation messages.\r\n",
            "You are now deaf to the congratulation messages.\r\n",
        ],
        [
            "You can now hear the Wiz-channel.\r\n",
            "You are now deaf to the Wiz-channel.\r\n",
        ],
        [
            "You are no longer part of the Quest.\r\n",
            "Okay, you are part of the Quest!\r\n",
        ],
        [
            "You will no longer see the room flags.\r\n",
            "You will now see the room flags.\r\n",
        ],
        [
            "You will now have your communication repeated.\r\n",
            "You will no longer have your communication repeated.\r\n",
        ],
        ["HolyLight mode off.\r\n", "HolyLight mode on.\r\n"],
        [
            "Nameserver_is_slow changed to NO; IP addresses will now be resolved.\r\n",
            "Nameserver_is_slow changed to YES; sitenames will no longer be resolved.\r\n",
        ],
        ["Autoexits disabled.\r\n", "Autoexits enabled.\r\n"],
        [
            "Will no longer track through doors.\r\n",
            "Will now track through doors.\r\n",
        ],
    ];

    if ch.is_npc() {
        return;
    }
    let result;
    match subcmd {
        SCMD_NOSUMMON => {
            result = prf_tog_chk!(ch, PRF_SUMMONABLE);
        }
        SCMD_NOHASSLE => {
            result = prf_tog_chk!(ch, PRF_NOHASSLE);
        }
        SCMD_BRIEF => {
            result = prf_tog_chk!(ch, PRF_BRIEF);
        }
        SCMD_COMPACT => {
            result = prf_tog_chk!(ch, PRF_COMPACT);
        }
        SCMD_NOTELL => {
            result = prf_tog_chk!(ch, PRF_NOTELL);
        }
        SCMD_NOAUCTION => {
            result = prf_tog_chk!(ch, PRF_NOAUCT);
        }
        SCMD_DEAF => {
            result = prf_tog_chk!(ch, PRF_DEAF);
        }
        SCMD_NOGOSSIP => {
            result = prf_tog_chk!(ch, PRF_NOGOSS);
        }
        SCMD_NOGRATZ => {
            result = prf_tog_chk!(ch, PRF_NOGRATZ);
        }
        SCMD_NOWIZ => {
            result = prf_tog_chk!(ch, PRF_NOWIZ);
        }
        SCMD_QUEST => {
            result = prf_tog_chk!(ch, PRF_QUEST);
        }
        SCMD_ROOMFLAGS => {
            result = prf_tog_chk!(ch, PRF_ROOMFLAGS);
        }
        SCMD_NOREPEAT => {
            result = prf_tog_chk!(ch, PRF_NOREPEAT);
        }
        SCMD_HOLYLIGHT => {
            result = prf_tog_chk!(ch, PRF_HOLYLIGHT);
        }
        SCMD_SLOWNS => {
            result = {
                game.config
                    .nameserver_is_slow
                    .set(game.config.nameserver_is_slow.get());
                game.config.nameserver_is_slow.get()
            }
        }
        SCMD_AUTOEXIT => {
            result = prf_tog_chk!(ch, PRF_AUTOEXIT);
        }
        SCMD_TRACK => {
            result = {
                game.config
                    .track_through_doors
                    .set(!game.config.track_through_doors.get());
                game.config.track_through_doors.get()
            }
        }
        _ => {
            error!("SYSERR: Unknown subcmd {} in do_gen_toggle.", subcmd);
            return;
        }
    }

    if result {
        send_to_char(ch, TOG_MESSAGES[subcmd as usize][TOG_ON]);
    } else {
        send_to_char(ch, TOG_MESSAGES[subcmd as usize][TOG_OFF]);
    }

    return;
}
