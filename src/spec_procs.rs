/* ************************************************************************
*   File: spec_procs.c                                  Part of CircleMUD *
*  Usage: implementation of special procedures for mobiles/objects/rooms  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */
// /* local functions */
// void sort_spells(void);
// int compare_spells(const void *x, const void *y);
// const char *how_good(int percent);
// void list_skills(struct char_data *ch);
// SPECIAL(guild);
// SPECIAL(dump);
// SPECIAL(mayor);
// void npc_steal(struct char_data *ch, struct char_data *victim);
// SPECIAL(snake);
// SPECIAL(thief);
// SPECIAL(magic_user);
// SPECIAL(guild_guard);
// SPECIAL(puff);
// SPECIAL(fido);
// SPECIAL(janitor);
// SPECIAL(cityguard);
// SPECIAL(pet_shops);
// SPECIAL(bank);

/* ********************************************************************
*  Special procedures for mobiles                                     *
******************************************************************** */

//
// int compare_spells(const void *x, const void *y)
// {
// int	a = *(const int *)x,
// b = *(const int *)y;
//
// return strcmp(spell_info[a].name, spell_info[b].name);
// }

use std::any::Any;
use std::cmp::{max, min};
use std::rc::Rc;

use crate::class::{GUILD_INFO, PRAC_PARAMS};
use crate::constants::INT_APP;
use crate::db::DB;
use crate::interpreter::{cmd_is, is_move};
use crate::modify::page_string;
use crate::spell_parser::find_skill_num;
use crate::structs::{CharData, AFF_BLIND, LVL_IMMORT, MAX_SKILLS, NOWHERE};
use crate::{send_to_char, MainGlobals, TO_ROOM};

pub fn sort_spells(db: &mut DB) {
    /* initialize array, avoiding reserved. */
    for a in 1..(MAX_SKILLS + 1) {
        db.spell_sort_info[a] = a as i32;
    }

    db.spell_sort_info
        .sort_by_key(|s| db.spell_info[*s as usize].name);
}

fn how_good(percent: i8) -> &'static str {
    if percent < 0 {
        return " error)";
    };
    if percent == 0 {
        return " (not learned)";
    };
    if percent <= 10 {
        return " (awful)";
    }
    if percent <= 20 {
        return " (bad)";
    }
    if percent <= 40 {
        return " (poor)";
    }
    if percent <= 55 {
        return " (average)";
    }
    if percent <= 70 {
        return " (fair)";
    }
    if percent <= 80 {
        return " (good)";
    }
    if percent <= 85 {
        return " (very good)";
    }

    " (superb)"
}

const PRAC_TYPES: [&str; 2] = ["spell", "skill"];

const LEARNED_LEVEL: usize = 0; /* % known which is considered "learned" */
const MAX_PER_PRAC: usize = 1; /* max percent gain in skill per practice */
const MIN_PER_PRAC: usize = 2; /* min percent gain in skill per practice */
const PRAC_TYPE: usize = 3; /* should it say 'spell' or 'skill'?	 */

fn learned(ch: &Rc<CharData>) -> i8 {
    PRAC_PARAMS[LEARNED_LEVEL][ch.get_class() as usize] as i8
}

fn mingain(ch: &Rc<CharData>) -> i32 {
    PRAC_PARAMS[MIN_PER_PRAC][ch.get_class() as usize]
}

fn maxgain(ch: &Rc<CharData>) -> i32 {
    PRAC_PARAMS[MAX_PER_PRAC][ch.get_class() as usize]
}

fn splskl(ch: &Rc<CharData>) -> &str {
    PRAC_TYPES[PRAC_PARAMS[PRAC_TYPE][ch.get_class() as usize] as usize]
}

pub fn list_skills(db: &DB, ch: &Rc<CharData>) {
    if ch.get_practices() == 0 {
        send_to_char(ch, "You have no practice sessions remaining.\r\n");
        return;
    }

    let mut buf = format!(
        "You have {} practice session{} remaining.\r\nYou know of the following {}s:\r\n",
        ch.get_practices(),
        if ch.get_practices() == 1 { "" } else { "s" },
        splskl(ch)
    );

    for sortpos in 1..(MAX_SKILLS + 1) as usize {
        let i = db.spell_sort_info[sortpos] as usize;
        if ch.get_level() >= db.spell_info[i].min_level[ch.get_class() as usize] as u8 {
            buf.push_str(
                format!(
                    "{:20} {}\r\n",
                    db.spell_info[i].name,
                    how_good(ch.get_skill(i as i32))
                )
                .as_str(),
            );
        }
    }

    page_string(ch.desc.borrow().as_ref(), &buf, true);
}

pub fn guild(
    game: &MainGlobals,
    ch: &Rc<CharData>,
    me: &dyn Any,
    cmd: i32,
    argument: &str,
) -> bool {
    let db = &game.db;
    if ch.is_npc() || !cmd_is(cmd, "practice") {
        return false;
    }

    let argument = argument.trim();

    if argument.is_empty() {
        list_skills(db, ch);
        return true;
    }

    if ch.get_practices() <= 0 {
        send_to_char(ch, "You do not seem to be able to practice now.\r\n");
        return true;
    }

    let skill_num = find_skill_num(db, argument);

    if skill_num.is_none()
        || ch.get_level()
            < db.spell_info[skill_num.unwrap() as usize].min_level[ch.get_class() as usize] as u8
    {
        send_to_char(
            ch,
            format!("You do not know of that {}.\r\n", splskl(ch)).as_str(),
        );
        return true;
    }
    if ch.get_skill(skill_num.unwrap()) >= learned(ch) {
        send_to_char(ch, "You are already learned in that area.\r\n");
        return true;
    }
    send_to_char(ch, "You practice for a while...\r\n");
    ch.set_practices(ch.get_practices() - 1);

    let mut percent = ch.get_skill(skill_num.unwrap());
    percent += min(
        maxgain(ch),
        max(mingain(ch), INT_APP[ch.get_int() as usize].learn as i32),
    ) as i8;

    ch.set_skill(skill_num.unwrap(), min(learned(ch), percent));

    if ch.get_skill(skill_num.unwrap()) >= learned(ch) {
        send_to_char(ch, "You are now learned in that area.\r\n");
    }

    true
}

// SPECIAL(dump)
// {
// struct obj_data *k;
// int value = 0;
//
// for (k = world[IN_ROOM(ch)].contents; k; k = world[IN_ROOM(ch)].contents) {
// act("$p vanishes in a puff of smoke!", FALSE, 0, k, 0, TO_ROOM);
// extract_obj(k);
// }
//
// if (!CMD_IS("drop"))
// return (FALSE);
//
// do_drop(ch, argument, cmd, SCMD_DROP);
//
// for (k = world[IN_ROOM(ch)].contents; k; k = world[IN_ROOM(ch)].contents) {
// act("$p vanishes in a puff of smoke!", FALSE, 0, k, 0, TO_ROOM);
// value += MAX(1, MIN(50, GET_OBJ_COST(k) / 10));
// extract_obj(k);
// }
//
// if (value) {
// send_to_char(ch, "You are awarded for outstanding performance.\r\n");
// act("$n has been awarded for being a good citizen.", TRUE, ch, 0, 0, TO_ROOM);
//
// if (GET_LEVEL(ch) < 3)
// gain_exp(ch, value);
// else
// GET_GOLD(ch) += value;
// }
// return (TRUE);
// }
//
//
// SPECIAL(mayor)
// {
// char actbuf[MAX_INPUT_LENGTH];
//
// const char open_path[] =
// "W3a3003b33000c111d0d111Oe333333Oe22c222112212111a1S.";
// const char close_path[] =
// "W3a3003b33000c111d0d111CE333333CE22c222112212111a1S.";
//
// static const char *path = NULL;
// static int path_index;
// static bool move = FALSE;
//
// if (!move) {
// if (time_info.hours == 6) {
// move = TRUE;
// path = open_path;
// path_index = 0;
// } else if (time_info.hours == 20) {
// move = TRUE;
// path = close_path;
// path_index = 0;
// }
// }
// if (cmd || !move || (GET_POS(ch) < POS_SLEEPING) ||
// (GET_POS(ch) == POS_FIGHTING))
// return (FALSE);
//
// switch (path[path_index]) {
// case '0':
// case '1':
// case '2':
// case '3':
// perform_move(ch, path[path_index] - '0', 1);
// break;
//
// case 'W':
// GET_POS(ch) = POS_STANDING;
// act("$n awakens and groans loudly.", FALSE, ch, 0, 0, TO_ROOM);
// break;
//
// case 'S':
// GET_POS(ch) = POS_SLEEPING;
// act("$n lies down and instantly falls asleep.", FALSE, ch, 0, 0, TO_ROOM);
// break;
//
// case 'a':
// act("$n says 'Hello Honey!'", FALSE, ch, 0, 0, TO_ROOM);
// act("$n smirks.", FALSE, ch, 0, 0, TO_ROOM);
// break;
//
// case 'b':
// act("$n says 'What a view!  I must get something done about that dump!'",
// FALSE, ch, 0, 0, TO_ROOM);
// break;
//
// case 'c':
// act("$n says 'Vandals!  Youngsters nowadays have no respect for anything!'",
// FALSE, ch, 0, 0, TO_ROOM);
// break;
//
// case 'd':
// act("$n says 'Good day, citizens!'", FALSE, ch, 0, 0, TO_ROOM);
// break;
//
// case 'e':
// act("$n says 'I hereby declare the bazaar open!'", FALSE, ch, 0, 0, TO_ROOM);
// break;
//
// case 'E':
// act("$n says 'I hereby declare Midgaard closed!'", FALSE, ch, 0, 0, TO_ROOM);
// break;
//
// case 'O':
// do_gen_door(ch, strcpy(actbuf, "gate"), 0, SCMD_UNLOCK);	/* strcpy: OK */
// do_gen_door(ch, strcpy(actbuf, "gate"), 0, SCMD_OPEN);	/* strcpy: OK */
// break;
//
// case 'C':
// do_gen_door(ch, strcpy(actbuf, "gate"), 0, SCMD_CLOSE);	/* strcpy: OK */
// do_gen_door(ch, strcpy(actbuf, "gate"), 0, SCMD_LOCK);	/* strcpy: OK */
// break;
//
// case '.':
// move = FALSE;
// break;
//
// }
//
// path_index++;
// return (FALSE);
// }
//
//
// /* ********************************************************************
// *  General special procedures for mobiles                             *
// ******************************************************************** */
//
//
// void npc_steal(struct char_data *ch, struct char_data *victim)
// {
// int gold;
//
// if (IS_NPC(victim))
// return;
// if (GET_LEVEL(victim) >= LVL_IMMORT)
// return;
// if (!CAN_SEE(ch, victim))
// return;
//
// if (AWAKE(victim) && (rand_number(0, GET_LEVEL(ch)) == 0)) {
// act("You discover that $n has $s hands in your wallet.", FALSE, ch, 0, victim, TO_VICT);
// act("$n tries to steal gold from $N.", TRUE, ch, 0, victim, TO_NOTVICT);
// } else {
// /* Steal some gold coins */
// gold = (GET_GOLD(victim) * rand_number(1, 10)) / 100;
// if (gold > 0) {
// GET_GOLD(ch) += gold;
// GET_GOLD(victim) -= gold;
// }
// }
// }
//
//
// /*
//  * Quite lethal to low-level characters.
//  */
// SPECIAL(snake)
// {
// if (cmd || GET_POS(ch) != POS_FIGHTING || !FIGHTING(ch))
// return (FALSE);
//
// if (IN_ROOM(FIGHTING(ch)) != IN_ROOM(ch) || rand_number(0, GET_LEVEL(ch)) != 0)
// return (FALSE);
//
// act("$n bites $N!", 1, ch, 0, FIGHTING(ch), TO_NOTVICT);
// act("$n bites you!", 1, ch, 0, FIGHTING(ch), TO_VICT);
// call_magic(ch, FIGHTING(ch), 0, SPELL_POISON, GET_LEVEL(ch), CAST_SPELL);
// return (TRUE);
// }
//
//
// SPECIAL(thief)
// {
// struct char_data *cons;
//
// if (cmd || GET_POS(ch) != POS_STANDING)
// return (FALSE);
//
// for (cons = world[IN_ROOM(ch)].people; cons; cons = cons->next_in_room)
// if (!IS_NPC(cons) && GET_LEVEL(cons) < LVL_IMMORT && !rand_number(0, 4)) {
// npc_steal(ch, cons);
// return (TRUE);
// }
//
// return (FALSE);
// }
//
//
// SPECIAL(magic_user)
// {
// struct char_data *vict;
//
// if (cmd || GET_POS(ch) != POS_FIGHTING)
// return (FALSE);
//
// /* pseudo-randomly choose someone in the room who is fighting me */
// for (vict = world[IN_ROOM(ch)].people; vict; vict = vict->next_in_room)
// if (FIGHTING(vict) == ch && !rand_number(0, 4))
// break;
//
// /* if I didn't pick any of those, then just slam the guy I'm fighting */
// if (vict == NULL && IN_ROOM(FIGHTING(ch)) == IN_ROOM(ch))
// vict = FIGHTING(ch);
//
// /* Hm...didn't pick anyone...I'll wait a round. */
// if (vict == NULL)
// return (TRUE);
//
// if (GET_LEVEL(ch) > 13 && rand_number(0, 10) == 0)
// cast_spell(ch, vict, NULL, SPELL_POISON);
//
// if (GET_LEVEL(ch) > 7 && rand_number(0, 8) == 0)
// cast_spell(ch, vict, NULL, SPELL_BLINDNESS);
//
// if (GET_LEVEL(ch) > 12 && rand_number(0, 12) == 0) {
// if (IS_EVIL(ch))
// cast_spell(ch, vict, NULL, SPELL_ENERGY_DRAIN);
// else if (IS_GOOD(ch))
// cast_spell(ch, vict, NULL, SPELL_DISPEL_EVIL);
// }
//
// if (rand_number(0, 4))
// return (TRUE);
//
// switch (GET_LEVEL(ch)) {
// case 4:
// case 5:
// cast_spell(ch, vict, NULL, SPELL_MAGIC_MISSILE);
// break;
// case 6:
// case 7:
// cast_spell(ch, vict, NULL, SPELL_CHILL_TOUCH);
// break;
// case 8:
// case 9:
// cast_spell(ch, vict, NULL, SPELL_BURNING_HANDS);
// break;
// case 10:
// case 11:
// cast_spell(ch, vict, NULL, SPELL_SHOCKING_GRASP);
// break;
// case 12:
// case 13:
// cast_spell(ch, vict, NULL, SPELL_LIGHTNING_BOLT);
// break;
// case 14:
// case 15:
// case 16:
// case 17:
// cast_spell(ch, vict, NULL, SPELL_COLOR_SPRAY);
// break;
// default:
// cast_spell(ch, vict, NULL, SPELL_FIREBALL);
// break;
// }
// return (TRUE);
//
// }

/* ********************************************************************
*  Special procedures for mobiles                                      *
******************************************************************** */

pub fn guild_guard(
    game: &MainGlobals,
    ch: &Rc<CharData>,
    me: &dyn Any,
    cmd: i32,
    argument: &str,
) -> bool {
    let guard = me.downcast_ref::<Rc<CharData>>().unwrap();
    let buf = "The guard humiliates you, and blocks your way.\r\n";
    let buf2 = "The guard humiliates $n, and blocks $s way.";

    if !is_move(cmd) || guard.aff_flagged(AFF_BLIND) {
        return false;
    }

    if ch.get_level() >= LVL_IMMORT as u8 {
        return false;
    }

    for gi in GUILD_INFO {
        if gi.guild_room == NOWHERE {
            break;
        }
        /* Wrong guild or not trying to enter. */
        if game.db.get_room_vnum(ch.in_room()) != gi.guild_room || cmd != gi.direction {
            continue;
        }
        /* Allow the people of the guild through. */
        if !ch.is_npc() && ch.get_class() == gi.pc_class {
            continue;
        }
        send_to_char(ch, buf);
        game.db.act(buf2, false, Some(ch), None, None, TO_ROOM);

        return true;
    }
    false
}

// SPECIAL(puff)
// {
// char actbuf[MAX_INPUT_LENGTH];
//
// if (cmd)
// return (FALSE);
//
// switch (rand_number(0, 60)) {
// case 0:
// do_say(ch, strcpy(actbuf, "My god!  It's full of stars!"), 0, 0);	/* strcpy: OK */
// return (TRUE);
// case 1:
// do_say(ch, strcpy(actbuf, "How'd all those fish get up here?"), 0, 0);	/* strcpy: OK */
// return (TRUE);
// case 2:
// do_say(ch, strcpy(actbuf, "I'm a very female dragon."), 0, 0);	/* strcpy: OK */
// return (TRUE);
// case 3:
// do_say(ch, strcpy(actbuf, "I've got a peaceful, easy feeling."), 0, 0);	/* strcpy: OK */
// return (TRUE);
// default:
// return (FALSE);
// }
// }
//
//
//
// SPECIAL(fido)
// {
// struct obj_data *i, *temp, *next_obj;
//
// if (cmd || !AWAKE(ch))
// return (FALSE);
//
// for (i = world[IN_ROOM(ch)].contents; i; i = i->next_content) {
// if (!IS_CORPSE(i))
// continue;
//
// act("$n savagely devours a corpse.", FALSE, ch, 0, 0, TO_ROOM);
// for (temp = i->contains; temp; temp = next_obj) {
// next_obj = temp->next_content;
// obj_from_obj(temp);
// obj_to_room(temp, IN_ROOM(ch));
// }
// extract_obj(i);
// return (TRUE);
// }
//
// return (FALSE);
// }
//
//
//
// SPECIAL(janitor)
// {
// struct obj_data *i;
//
// if (cmd || !AWAKE(ch))
// return (FALSE);
//
// for (i = world[IN_ROOM(ch)].contents; i; i = i->next_content) {
// if (!CAN_WEAR(i, ITEM_WEAR_TAKE))
// continue;
// if (GET_OBJ_TYPE(i) != ITEM_DRINKCON && GET_OBJ_COST(i) >= 15)
// continue;
// act("$n picks up some trash.", FALSE, ch, 0, 0, TO_ROOM);
// obj_from_room(i);
// obj_to_char(i, ch);
// return (TRUE);
// }
//
// return (FALSE);
// }
//
//
// SPECIAL(cityguard)
// {
// struct char_data *tch, *evil, *spittle;
// int max_evil, min_cha;
//
// if (cmd || !AWAKE(ch) || FIGHTING(ch))
// return (FALSE);
//
// max_evil = 1000;
// min_cha = 6;
// spittle = evil = NULL;
//
// for (tch = world[IN_ROOM(ch)].people; tch; tch = tch->next_in_room) {
// if (!CAN_SEE(ch, tch))
// continue;
//
// if (!IS_NPC(tch) && PLR_FLAGGED(tch, PLR_KILLER)) {
// act("$n screams 'HEY!!!  You're one of those PLAYER KILLERS!!!!!!'", FALSE, ch, 0, 0, TO_ROOM);
// hit(ch, tch, TYPE_UNDEFINED);
// return (TRUE);
// }
//
// if (!IS_NPC(tch) && PLR_FLAGGED(tch, PLR_THIEF)) {
// act("$n screams 'HEY!!!  You're one of those PLAYER THIEVES!!!!!!'", FALSE, ch, 0, 0, TO_ROOM);
// hit(ch, tch, TYPE_UNDEFINED);
// return (TRUE);
// }
//
// if (FIGHTING(tch) && GET_ALIGNMENT(tch) < max_evil && (IS_NPC(tch) || IS_NPC(FIGHTING(tch)))) {
// max_evil = GET_ALIGNMENT(tch);
// evil = tch;
// }
//
// if (GET_CHA(tch) < min_cha) {
// spittle = tch;
// min_cha = GET_CHA(tch);
// }
// }
//
// if (evil && GET_ALIGNMENT(FIGHTING(evil)) >= 0) {
// act("$n screams 'PROTECT THE INNOCENT!  BANZAI!  CHARGE!  ARARARAGGGHH!'", FALSE, ch, 0, 0, TO_ROOM);
// hit(ch, evil, TYPE_UNDEFINED);
// return (TRUE);
// }
//
// /* Reward the socially inept. */
// if (spittle && !rand_number(0, 9)) {
// static int spit_social;
//
// if (!spit_social)
// spit_social = find_command("spit");
//
// if (spit_social > 0) {
// char spitbuf[MAX_NAME_LENGTH + 1];
//
// strncpy(spitbuf, GET_NAME(spittle), sizeof(spitbuf));	/* strncpy: OK */
// spitbuf[sizeof(spitbuf) - 1] = '\0';
//
// do_action(ch, spitbuf, spit_social, 0);
// return (TRUE);
// }
// }
//
// return (FALSE);
// }
//
//
// #define PET_PRICE(pet) (GET_LEVEL(pet) * 300)
//
// SPECIAL(pet_shops)
// {
// char buf[MAX_STRING_LENGTH], pet_name[256];
// room_rnum pet_room;
// struct char_data *pet;
//
// /* Gross. */
// pet_room = IN_ROOM(ch) + 1;
//
// if (CMD_IS("list")) {
// send_to_char(ch, "Available pets are:\r\n");
// for (pet = world[pet_room].people; pet; pet = pet->next_in_room) {
// /* No, you can't have the Implementor as a pet if he's in there. */
// if (!IS_NPC(pet))
// continue;
// send_to_char(ch, "%8d - %s\r\n", PET_PRICE(pet), GET_NAME(pet));
// }
// return (TRUE);
// } else if (CMD_IS("buy")) {
//
// two_arguments(argument, buf, pet_name);
//
// if (!(pet = get_char_room(buf, NULL, pet_room)) || !IS_NPC(pet)) {
// send_to_char(ch, "There is no such pet!\r\n");
// return (TRUE);
// }
// if (GET_GOLD(ch) < PET_PRICE(pet)) {
// send_to_char(ch, "You don't have enough gold!\r\n");
// return (TRUE);
// }
// GET_GOLD(ch) -= PET_PRICE(pet);
//
// pet = read_mobile(GET_MOB_RNUM(pet), REAL);
// GET_EXP(pet) = 0;
// SET_BIT(AFF_FLAGS(pet), AFF_CHARM);
//
// if (*pet_name) {
// snprintf(buf, sizeof(buf), "%s %s", pet->player.name, pet_name);
// /* free(pet->player.name); don't free the prototype! */
// pet->player.name = strdup(buf);
//
// snprintf(buf, sizeof(buf), "%sA small sign on a chain around the neck says 'My name is %s'\r\n",
// pet->player.description, pet_name);
// /* free(pet->player.description); don't free the prototype! */
// pet->player.description = strdup(buf);
// }
// char_to_room(pet, IN_ROOM(ch));
// add_follower(pet, ch);
//
// /* Be certain that pets can't get/carry/use/wield/wear items */
// IS_CARRYING_W(pet) = 1000;
// IS_CARRYING_N(pet) = 100;
//
// send_to_char(ch, "May you enjoy your pet.\r\n");
// act("$n buys $N as a pet.", FALSE, ch, 0, pet, TO_ROOM);
//
// return (TRUE);
// }
//
// /* All commands except list and buy */
// return (FALSE);
// }
//
//
//
// /* ********************************************************************
// *  Special procedures for objects                                     *
// ******************************************************************** */
//
//
// SPECIAL(bank)
// {
// int amount;
//
// if (CMD_IS("balance")) {
// if (GET_BANK_GOLD(ch) > 0)
// send_to_char(ch, "Your current balance is %d coins.\r\n", GET_BANK_GOLD(ch));
// else
// send_to_char(ch, "You currently have no money deposited.\r\n");
// return (TRUE);
// } else if (CMD_IS("deposit")) {
// if ((amount = atoi(argument)) <= 0) {
// send_to_char(ch, "How much do you want to deposit?\r\n");
// return (TRUE);
// }
// if (GET_GOLD(ch) < amount) {
// send_to_char(ch, "You don't have that many coins!\r\n");
// return (TRUE);
// }
// GET_GOLD(ch) -= amount;
// GET_BANK_GOLD(ch) += amount;
// send_to_char(ch, "You deposit %d coins.\r\n", amount);
// act("$n makes a bank transaction.", TRUE, ch, 0, FALSE, TO_ROOM);
// return (TRUE);
// } else if (CMD_IS("withdraw")) {
// if ((amount = atoi(argument)) <= 0) {
// send_to_char(ch, "How much do you want to withdraw?\r\n");
// return (TRUE);
// }
// if (GET_BANK_GOLD(ch) < amount) {
// send_to_char(ch, "You don't have that many coins deposited!\r\n");
// return (TRUE);
// }
// GET_GOLD(ch) += amount;
// GET_BANK_GOLD(ch) -= amount;
// send_to_char(ch, "You withdraw %d coins.\r\n", amount);
// act("$n makes a bank transaction.", TRUE, ch, 0, FALSE, TO_ROOM);
// return (TRUE);
// } else
// return (FALSE);
// }
//
