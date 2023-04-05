/* ************************************************************************
*   File: spell_parser.c                                Part of CircleMUD *
*  Usage: top-level magic routines; outside points of entry to magic sys. *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::rc::Rc;

use log::error;

use crate::db::DB;
use crate::interpreter::{any_one_arg, is_abbrev};
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
use crate::structs::{LVL_IMMORT, LVL_IMPL, POS_FIGHTING, POS_SITTING};
use crate::structs::{NUM_CLASSES, POS_STANDING};

// #define SINFO spell_info[spellnum]
//
// /* local globals */
// struct spell_info_type spell_info[TOP_SPELL_DEFINE + 1];
//
// /* local functions */
// void say_spell(struct char_data *ch, int spellnum, struct char_data *tch, struct obj_data *tobj);
// void spello(int spl, const char *name, int max_mana, int min_mana, int mana_change, int minpos, int targets, int violent, int routines, const char *wearoff);
// int mag_manacost(struct char_data *ch, int spellnum);
// ACMD(do_cast);
// void unused_spell(int spl);
// void mag_assign_spells(void);
//
// /*
//  * This arrangement is pretty stupid, but the number of skills is limited by
//  * the playerfile.  We can arbitrarily increase the number of skills by
//  * increasing the space in the playerfile. Meanwhile, 200 should provide
//  * ample slots for skills.
//  */
//
// struct syllable {
//     const char *org;
//     const char *news;
// };
//
//
// struct syllable syls[] = {
// {" ", " "},
// {"ar", "abra"},
// {"ate", "i"},
// {"cau", "kada"},
// {"blind", "nose"},
// {"bur", "mosa"},
// {"cu", "judi"},
// {"de", "oculo"},
// {"dis", "mar"},
// {"ect", "kamina"},
// {"en", "uns"},
// {"gro", "cra"},
// {"light", "dies"},
// {"lo", "hi"},
// {"magi", "kari"},
// {"mon", "bar"},
// {"mor", "zak"},
// {"move", "sido"},
// {"ness", "lacri"},
// {"ning", "illa"},
// {"per", "duda"},
// {"ra", "gru"},
// {"re", "candus"},
// {"son", "sabru"},
// {"tect", "infra"},
// {"tri", "cula"},
// {"ven", "nofo"},
// {"word of", "inset"},
// {"a", "i"}, {"b", "v"}, {"c", "q"}, {"d", "m"}, {"e", "o"}, {"f", "y"}, {"g", "t"},
// {"h", "p"}, {"i", "u"}, {"j", "y"}, {"k", "t"}, {"l", "r"}, {"m", "w"}, {"n", "b"},
// {"o", "a"}, {"p", "s"}, {"q", "d"}, {"r", "f"}, {"s", "g"}, {"t", "h"}, {"u", "e"},
// {"v", "z"}, {"w", "x"}, {"x", "n"}, {"y", "l"}, {"z", "k"}, {"", ""}
// };
//
pub const UNUSED_SPELLNAME: &str = "!UNUSED!"; /* So we can get &UNUSED_SPELLNAME */
//
// int mag_manacost(struct char_data *ch, int spellnum)
// {
// return MAX(SINFO.mana_max - (SINFO.mana_change *
// (GET_LEVEL(ch) - SINFO.min_level[(int) GET_CLASS(ch)])),
// SINFO.mana_min);
// }
//
//
// void say_spell(struct char_data *ch, int spellnum, struct char_data *tch,
// struct obj_data *tobj)
// {
// char lbuf[256], buf[256], buf1[256], buf2[256];	/* FIXME */
// const char *format;
//
// struct char_data *i;
// int j, ofs = 0;
//
// *buf = '\0';
// strlcpy(lbuf, skill_name(spellnum), sizeof(lbuf));
//
// while (lbuf[ofs]) {
// for (j = 0; *(syls[j].org); j++) {
// if (!strncmp(syls[j].org, lbuf + ofs, strlen(syls[j].org))) {
// strcat(buf, syls[j].news);	/* strcat: BAD */
// ofs += strlen(syls[j].org);
// break;
// }
// }
// /* i.e., we didn't find a match in syls[] */
// if (!*syls[j].org) {
// log("No entry in syllable table for substring of '%s'", lbuf);
// ofs++;
// }
// }
//
// if (tch != NULL && IN_ROOM(tch) == IN_ROOM(ch)) {
// if (tch == ch)
// format = "$n closes $s eyes and utters the words, '%s'.";
// else
// format = "$n stares at $N and utters the words, '%s'.";
// } else if (tobj != NULL &&
// ((IN_ROOM(tobj) == IN_ROOM(ch)) || (tobj->carried_by == ch)))
// format = "$n stares at $p and utters the words, '%s'.";
// else
// format = "$n utters the words, '%s'.";
//
// snprintf(buf1, sizeof(buf1), format, skill_name(spellnum));
// snprintf(buf2, sizeof(buf2), format, buf);
//
// for (i = world[IN_ROOM(ch)].people; i; i = i->next_in_room) {
// if (i == ch || i == tch || !i->desc || !AWAKE(i))
// continue;
// if (GET_CLASS(ch) == GET_CLASS(i))
// perform_act(buf1, ch, tobj, tch, i);
// else
// perform_act(buf2, ch, tobj, tch, i);
// }
//
// if (tch != NULL && tch != ch && IN_ROOM(tch) == IN_ROOM(ch)) {
// snprintf(buf1, sizeof(buf1), "$n stares at you and utters the words, '%s'.",
// GET_CLASS(ch) == GET_CLASS(tch) ? skill_name(spellnum) : buf);
// act(buf1, false, ch, NULL, tch, TO_VICT);
// }
// }

/*
 * This function should be used anytime you are not 100% sure that you have
 * a valid spell/skill number.  A typical for() loop would not need to use
 * this because you can guarantee > 0 and <= TOP_SPELL_DEFINE.
 */
fn skill_name(db: &DB, num: i32) -> &'static str {
    return if num > 0 && num <= TOP_SPELL_DEFINE as i32 {
        db.spell_info[num as usize].name
    } else if num == -1 {
        "UNUSED"
    } else {
        "UNDEFINED"
    };
}

pub fn find_skill_num(db: &DB, name: &str) -> Option<i32> {
    let mut ok = false;
    for skindex in 1..(TOP_SPELL_DEFINE + 1) {
        if is_abbrev(name, &db.spell_info[skindex].name) {
            return Some(skindex as i32);
        }

        ok = true;
        let tempbuf = db.spell_info[skindex].name.as_ref();
        let mut first = String::new();
        let mut first2 = String::new();
        let mut temp = any_one_arg(tempbuf, &mut first);
        let mut temp2 = any_one_arg(name, &mut first2);
        while !first.is_empty() && !first2.is_empty() && ok {
            if !is_abbrev(&first2, &first) {
                ok = false;
                continue;
            }
            temp = any_one_arg(temp, &mut first);
            temp2 = any_one_arg(temp2, &mut first2);
        }

        if ok && first2.is_empty() {
            return Some(skindex as i32);
        }
    }

    None
}

// /*
//  * This function is the very heart of the entire magic system.  All
//  * invocations of all types of magic -- objects, spoken and unspoken PC
//  * and NPC spells, the works -- all come through this function eventually.
//  * This is also the entry point for non-spoken or unrestricted spells.
//  * Spellnum 0 is legal but silently ignored here, to make callers simpler.
//  */
// int call_magic(struct char_data *caster, struct char_data *cvict,
// struct obj_data *ovict, int spellnum, int level, int casttype)
// {
// int savetype;
//
// if (spellnum < 1 || spellnum > TOP_SPELL_DEFINE)
// return (0);
//
// if (ROOM_FLAGGED(IN_ROOM(caster), ROOM_NOMAGIC)) {
// send_to_char(caster, "Your magic fizzles out and dies.\r\n");
// act("$n's magic fizzles out and dies.", false, caster, 0, 0, TO_ROOM);
// return (0);
// }
// if (ROOM_FLAGGED(IN_ROOM(caster), ROOM_PEACEFUL) &&
// (SINFO.violent || IS_SET(SINFO.routines, MAG_DAMAGE))) {
// send_to_char(caster, "A flash of white light fills the room, dispelling your violent magic!\r\n");
// act("White light from no particular source suddenly fills the room, then vanishes.", false, caster, 0, 0, TO_ROOM);
// return (0);
// }
// /* determine the type of saving throw */
// switch (casttype) {
// case CAST_STAFF:
// case CAST_SCROLL:
// case CAST_POTION:
// case CAST_WAND:
// savetype = SAVING_ROD;
// break;
// case CAST_SPELL:
// savetype = SAVING_SPELL;
// break;
// default:
// savetype = SAVING_BREATH;
// break;
// }
//
//
// if (IS_SET(SINFO.routines, MAG_DAMAGE))
// if (mag_damage(level, caster, cvict, spellnum, savetype) == -1)
// return (-1);	/* Successful and target died, don't cast again. */
//
// if (IS_SET(SINFO.routines, MAG_AFFECTS))
// mag_affects(level, caster, cvict, spellnum, savetype);
//
// if (IS_SET(SINFO.routines, MAG_UNAFFECTS))
// mag_unaffects(level, caster, cvict, spellnum, savetype);
//
// if (IS_SET(SINFO.routines, MAG_POINTS))
// mag_points(level, caster, cvict, spellnum, savetype);
//
// if (IS_SET(SINFO.routines, MAG_ALTER_OBJS))
// mag_alter_objs(level, caster, ovict, spellnum, savetype);
//
// if (IS_SET(SINFO.routines, MAG_GROUPS))
// mag_groups(level, caster, spellnum, savetype);
//
// if (IS_SET(SINFO.routines, MAG_MASSES))
// mag_masses(level, caster, spellnum, savetype);
//
// if (IS_SET(SINFO.routines, MAG_AREAS))
// mag_areas(level, caster, spellnum, savetype);
//
// if (IS_SET(SINFO.routines, MAG_SUMMONS))
// mag_summons(level, caster, ovict, spellnum, savetype);
//
// if (IS_SET(SINFO.routines, MAG_CREATIONS))
// mag_creations(level, caster, spellnum);
//
// if (IS_SET(SINFO.routines, MAG_MANUAL))
// switch (spellnum) {
// case SPELL_CHARM:		MANUAL_SPELL(spell_charm); break;
// case SPELL_CREATE_WATER:	MANUAL_SPELL(spell_create_water); break;
// case SPELL_DETECT_POISON:	MANUAL_SPELL(spell_detect_poison); break;
// case SPELL_ENCHANT_WEAPON:  MANUAL_SPELL(spell_enchant_weapon); break;
// case SPELL_IDENTIFY:	MANUAL_SPELL(spell_identify); break;
// case SPELL_LOCATE_OBJECT:   MANUAL_SPELL(spell_locate_object); break;
// case SPELL_SUMMON:		MANUAL_SPELL(spell_summon); break;
// case SPELL_WORD_OF_RECALL:  MANUAL_SPELL(spell_recall); break;
// case SPELL_TELEPORT:	MANUAL_SPELL(spell_teleport); break;
// }
//
// return (1);
// }
//
// /*
//  * mag_objectmagic: This is the entry-point for all magic items.  This should
//  * only be called by the 'quaff', 'use', 'recite', etc. routines.
//  *
//  * For reference, object values 0-3:
//  * staff  - [0]	level	[1] max charges	[2] num charges	[3] spell num
//  * wand   - [0]	level	[1] max charges	[2] num charges	[3] spell num
//  * scroll - [0]	level	[1] spell num	[2] spell num	[3] spell num
//  * potion - [0] level	[1] spell num	[2] spell num	[3] spell num
//  *
//  * Staves and wands will default to level 14 if the level is not specified;
//  * the DikuMUD format did not specify staff and wand levels in the world
//  * files (this is a CircleMUD enhancement).
//  */
// void mag_objectmagic(struct char_data *ch, struct obj_data *obj,
// char *argument)
// {
// char arg[MAX_INPUT_LENGTH];
// int i, k;
// struct char_data *tch = NULL, *next_tch;
// struct obj_data *tobj = NULL;
//
// one_argument(argument, arg);
//
// k = generic_find(arg, FIND_CHAR_ROOM | FIND_OBJ_INV | FIND_OBJ_ROOM |
// FIND_OBJ_EQUIP, ch, &tch, &tobj);
//
// switch (GET_OBJ_TYPE(obj)) {
// case ITEM_STAFF:
// act("You tap $p three times on the ground.", false, ch, obj, 0, TO_CHAR);
// if (obj->action_description)
// act(obj->action_description, false, ch, obj, 0, TO_ROOM);
// else
// act("$n taps $p three times on the ground.", false, ch, obj, 0, TO_ROOM);
//
// if (GET_OBJ_VAL(obj, 2) <= 0) {
// send_to_char(ch, "It seems powerless.\r\n");
// act("Nothing seems to happen.", false, ch, obj, 0, TO_ROOM);
// } else {
// GET_OBJ_VAL(obj, 2)--;
// WAIT_STATE(ch, PULSE_VIOLENCE);
// /* Level to cast spell at. */
// k = GET_OBJ_VAL(obj, 0) ? GET_OBJ_VAL(obj, 0) : DEFAULT_STAFF_LVL;
//
// /*
//  * Problem : Area/mass spells on staves can cause crashes.
//  * Solution: Remove the special nature of area/mass spells on staves.
//  * Problem : People like that behavior.
//  * Solution: We special case the area/mass spells here.
//  */
// if (HAS_SPELL_ROUTINE(GET_OBJ_VAL(obj, 3), MAG_MASSES | MAG_AREAS)) {
// for (i = 0, tch = world[IN_ROOM(ch)].people; tch; tch = tch->next_in_room)
// i++;
// while (i-- > 0)
// call_magic(ch, NULL, NULL, GET_OBJ_VAL(obj, 3), k, CAST_STAFF);
// } else {
// for (tch = world[IN_ROOM(ch)].people; tch; tch = next_tch) {
// next_tch = tch->next_in_room;
// if (ch != tch)
// call_magic(ch, tch, NULL, GET_OBJ_VAL(obj, 3), k, CAST_STAFF);
// }
// }
// }
// break;
// case ITEM_WAND:
// if (k == FIND_CHAR_ROOM) {
// if (tch == ch) {
// act("You point $p at yourself.", false, ch, obj, 0, TO_CHAR);
// act("$n points $p at $mself.", false, ch, obj, 0, TO_ROOM);
// } else {
// act("You point $p at $N.", false, ch, obj, tch, TO_CHAR);
// if (obj->action_description)
// act(obj->action_description, false, ch, obj, tch, TO_ROOM);
// else
// act("$n points $p at $N.", TRUE, ch, obj, tch, TO_ROOM);
// }
// } else if (tobj != NULL) {
// act("You point $p at $P.", false, ch, obj, tobj, TO_CHAR);
// if (obj->action_description)
// act(obj->action_description, false, ch, obj, tobj, TO_ROOM);
// else
// act("$n points $p at $P.", TRUE, ch, obj, tobj, TO_ROOM);
// } else if (IS_SET(spell_info[GET_OBJ_VAL(obj, 3)].routines, MAG_AREAS | MAG_MASSES)) {
// /* Wands with area spells don't need to be pointed. */
// act("You point $p outward.", false, ch, obj, NULL, TO_CHAR);
// act("$n points $p outward.", TRUE, ch, obj, NULL, TO_ROOM);
// } else {
// act("At what should $p be pointed?", false, ch, obj, NULL, TO_CHAR);
// return;
// }
//
// if (GET_OBJ_VAL(obj, 2) <= 0) {
// send_to_char(ch, "It seems powerless.\r\n");
// act("Nothing seems to happen.", false, ch, obj, 0, TO_ROOM);
// return;
// }
// GET_OBJ_VAL(obj, 2)--;
// WAIT_STATE(ch, PULSE_VIOLENCE);
// if (GET_OBJ_VAL(obj, 0))
// call_magic(ch, tch, tobj, GET_OBJ_VAL(obj, 3),
// GET_OBJ_VAL(obj, 0), CAST_WAND);
// else
// call_magic(ch, tch, tobj, GET_OBJ_VAL(obj, 3),
// DEFAULT_WAND_LVL, CAST_WAND);
// break;
// case ITEM_SCROLL:
// if (*arg) {
// if (!k) {
// act("There is nothing to here to affect with $p.", false,
// ch, obj, NULL, TO_CHAR);
// return;
// }
// } else
// tch = ch;
//
// act("You recite $p which dissolves.", TRUE, ch, obj, 0, TO_CHAR);
// if (obj->action_description)
// act(obj->action_description, false, ch, obj, NULL, TO_ROOM);
// else
// act("$n recites $p.", false, ch, obj, NULL, TO_ROOM);
//
// WAIT_STATE(ch, PULSE_VIOLENCE);
// for (i = 1; i <= 3; i++)
// if (call_magic(ch, tch, tobj, GET_OBJ_VAL(obj, i),
// GET_OBJ_VAL(obj, 0), CAST_SCROLL) <= 0)
// break;
//
// if (obj != NULL)
// extract_obj(obj);
// break;
// case ITEM_POTION:
// tch = ch;
// act("You quaff $p.", false, ch, obj, NULL, TO_CHAR);
// if (obj->action_description)
// act(obj->action_description, false, ch, obj, NULL, TO_ROOM);
// else
// act("$n quaffs $p.", TRUE, ch, obj, NULL, TO_ROOM);
//
// WAIT_STATE(ch, PULSE_VIOLENCE);
// for (i = 1; i <= 3; i++)
// if (call_magic(ch, ch, NULL, GET_OBJ_VAL(obj, i),
// GET_OBJ_VAL(obj, 0), CAST_POTION) <= 0)
// break;
//
// if (obj != NULL)
// extract_obj(obj);
// break;
// default:
// log("SYSERR: Unknown object_type %d in mag_objectmagic.",
// GET_OBJ_TYPE(obj));
// break;
// }
// }
//
//
// /*
//  * cast_spell is used generically to cast any spoken spell, assuming we
//  * already have the target char/obj and spell number.  It checks all
//  * restrictions, etc., prints the words, etc.
//  *
//  * Entry point for NPC casts.  Recommended entry point for spells cast
//  * by NPCs via specprocs.
//  */
// int cast_spell(struct char_data *ch, struct char_data *tch,
// struct obj_data *tobj, int spellnum)
// {
// if (spellnum < 0 || spellnum > TOP_SPELL_DEFINE) {
// log("SYSERR: cast_spell trying to call spellnum %d/%d.", spellnum,
// TOP_SPELL_DEFINE);
// return (0);
// }
//
// if (GET_POS(ch) < SINFO.min_position) {
// switch (GET_POS(ch)) {
// case POS_SLEEPING:
// send_to_char(ch, "You dream about great magical powers.\r\n");
// break;
// case POS_RESTING:
// send_to_char(ch, "You cannot concentrate while resting.\r\n");
// break;
// case POS_SITTING:
// send_to_char(ch, "You can't do this sitting!\r\n");
// break;
// case POS_FIGHTING:
// send_to_char(ch, "Impossible!  You can't concentrate enough!\r\n");
// break;
// default:
// send_to_char(ch, "You can't do much of anything like this!\r\n");
// break;
// }
// return (0);
// }
// if (AFF_FLAGGED(ch, AFF_CHARM) && (ch->master == tch)) {
// send_to_char(ch, "You are afraid you might hurt your master!\r\n");
// return (0);
// }
// if ((tch != ch) && IS_SET(SINFO.targets, TAR_SELF_ONLY)) {
// send_to_char(ch, "You can only cast this spell upon yourself!\r\n");
// return (0);
// }
// if ((tch == ch) && IS_SET(SINFO.targets, TAR_NOT_SELF)) {
// send_to_char(ch, "You cannot cast this spell upon yourself!\r\n");
// return (0);
// }
// if (IS_SET(SINFO.routines, MAG_GROUPS) && !AFF_FLAGGED(ch, AFF_GROUP)) {
// send_to_char(ch, "You can't cast this spell if you're not in a group!\r\n");
// return (0);
// }
// send_to_char(ch, "%s", OK);
// say_spell(ch, spellnum, tch, tobj);
//
// return (call_magic(ch, tch, tobj, spellnum, GET_LEVEL(ch), CAST_SPELL));
// }
//
//
// /*
//  * do_cast is the entry point for PC-casted spells.  It parses the arguments,
//  * determines the spell number and finds a target, throws the die to see if
//  * the spell can be cast, checks for sufficient mana and subtracts it, and
//  * passes control to cast_spell().
//  */
// ACMD(do_cast)
// {
// struct char_data *tch = NULL;
// struct obj_data *tobj = NULL;
// char *s, *t;
// int mana, spellnum, i, target = 0;
//
// if (IS_NPC(ch))
// return;
//
// /* get: blank, spell name, target name */
// s = strtok(argument, "'");
//
// if (s == NULL) {
// send_to_char(ch, "Cast what where?\r\n");
// return;
// }
// s = strtok(NULL, "'");
// if (s == NULL) {
// send_to_char(ch, "Spell names must be enclosed in the Holy Magic Symbols: '\r\n");
// return;
// }
// t = strtok(NULL, "\0");
//
// /* spellnum = search_block(s, spells, 0); */
// spellnum = find_skill_num(s);
//
// if ((spellnum < 1) || (spellnum > MAX_SPELLS)) {
// send_to_char(ch, "Cast what?!?\r\n");
// return;
// }
// if (GET_LEVEL(ch) < SINFO.min_level[(int) GET_CLASS(ch)]) {
// send_to_char(ch, "You do not know that spell!\r\n");
// return;
// }
// if (GET_SKILL(ch, spellnum) == 0) {
// send_to_char(ch, "You are unfamiliar with that spell.\r\n");
// return;
// }
// /* Find the target */
// if (t != NULL) {
// char arg[MAX_INPUT_LENGTH];
//
// strlcpy(arg, t, sizeof(arg));
// one_argument(arg, t);
// skip_spaces(&t);
// }
// if (IS_SET(SINFO.targets, TAR_IGNORE)) {
// target = TRUE;
// } else if (t != NULL && *t) {
// if (!target && (IS_SET(SINFO.targets, TAR_CHAR_ROOM))) {
// if ((tch = get_char_vis(ch, t, NULL, FIND_CHAR_ROOM)) != NULL)
// target = TRUE;
// }
// if (!target && IS_SET(SINFO.targets, TAR_CHAR_WORLD))
// if ((tch = get_char_vis(ch, t, NULL, FIND_CHAR_WORLD)) != NULL)
// target = TRUE;
//
// if (!target && IS_SET(SINFO.targets, TAR_OBJ_INV))
// if ((tobj = get_obj_in_list_vis(ch, t, NULL, ch->carrying)) != NULL)
// target = TRUE;
//
// if (!target && IS_SET(SINFO.targets, TAR_OBJ_EQUIP)) {
// for (i = 0; !target && i < NUM_WEARS; i++)
// if (GET_EQ(ch, i) && isname(t, GET_EQ(ch, i)->name)) {
// tobj = GET_EQ(ch, i);
// target = TRUE;
// }
// }
// if (!target && IS_SET(SINFO.targets, TAR_OBJ_ROOM))
// if ((tobj = get_obj_in_list_vis(ch, t, NULL, world[IN_ROOM(ch)].contents)) != NULL)
// target = TRUE;
//
// if (!target && IS_SET(SINFO.targets, TAR_OBJ_WORLD))
// if ((tobj = get_obj_vis(ch, t, NULL)) != NULL)
// target = TRUE;
//
// } else {			/* if target string is empty */
// if (!target && IS_SET(SINFO.targets, TAR_FIGHT_SELF))
// if (FIGHTING(ch) != NULL) {
// tch = ch;
// target = TRUE;
// }
// if (!target && IS_SET(SINFO.targets, TAR_FIGHT_VICT))
// if (FIGHTING(ch) != NULL) {
// tch = FIGHTING(ch);
// target = TRUE;
// }
// /* if no target specified, and the spell isn't violent, default to self */
// if (!target && IS_SET(SINFO.targets, TAR_CHAR_ROOM) &&
// !SINFO.violent) {
// tch = ch;
// target = TRUE;
// }
// if (!target) {
// send_to_char(ch, "Upon %s should the spell be cast?\r\n",
// IS_SET(SINFO.targets, TAR_OBJ_ROOM | TAR_OBJ_INV | TAR_OBJ_WORLD | TAR_OBJ_EQUIP) ? "what" : "who");
// return;
// }
// }
//
// if (target && (tch == ch) && SINFO.violent) {
// send_to_char(ch, "You shouldn't cast that on yourself -- could be bad for your health!\r\n");
// return;
// }
// if (!target) {
// send_to_char(ch, "Cannot find the target of your spell!\r\n");
// return;
// }
// mana = mag_manacost(ch, spellnum);
// if ((mana > 0) && (GET_MANA(ch) < mana) && (GET_LEVEL(ch) < LVL_IMMORT)) {
// send_to_char(ch, "You haven't the energy to cast that spell!\r\n");
// return;
// }
//
// /* You throws the dice and you takes your chances.. 101% is total failure */
// if (rand_number(0, 101) > GET_SKILL(ch, spellnum)) {
// WAIT_STATE(ch, PULSE_VIOLENCE);
// if (!tch || !skill_message(0, ch, tch, spellnum))
// send_to_char(ch, "You lost your concentration!\r\n");
// if (mana > 0)
// GET_MANA(ch) = MAX(0, MIN(GET_MAX_MANA(ch), GET_MANA(ch) - (mana / 2)));
// if (SINFO.violent && tch && IS_NPC(tch))
// hit(tch, ch, TYPE_UNDEFINED);
// } else { /* cast spell returns 1 on success; subtract mana & set waitstate */
// if (cast_spell(ch, tch, tobj, spellnum)) {
// WAIT_STATE(ch, PULSE_VIOLENCE);
// if (mana > 0)
// GET_MANA(ch) = MAX(0, MIN(GET_MAX_MANA(ch), GET_MANA(ch) - mana));
// }
// }
// }
//

pub fn spell_level(db: &mut DB, spell: i32, chclass: i8, level: i32) {
    let mut bad = false;

    if spell < 0 || spell > TOP_SPELL_DEFINE as i32 {
        error!(
            "SYSERR: attempting assign to illegal spellnum {}/{}",
            spell, TOP_SPELL_DEFINE
        );
        return;
    }

    if chclass < 0 || chclass >= NUM_CLASSES as i8 {
        error!(
            "SYSERR: assigning '{}' to illegal class {}/{}.",
            skill_name(db, spell),
            chclass,
            NUM_CLASSES - 1
        );
        bad = true;
    }

    if level < 1 || level > LVL_IMPL as i32 {
        error!(
            "SYSERR: assigning '{}' to illegal level {}/{}.",
            skill_name(db, spell),
            level,
            LVL_IMPL
        );
        bad = true;
    }

    if !bad {
        db.spell_info[spell as usize].min_level[chclass as usize] = level;
    }
}

/* Assign the spells on boot up */
fn spello(
    db: &mut DB,
    spl: i32,
    name: &'static str,
    max_mana: i32,
    min_mana: i32,
    mana_change: i32,
    minpos: u8,
    targets: i32,
    violent: bool,
    routines: i32,
    wearoff: &'static str,
) {
    let spl = spl as usize;
    for i in 0..NUM_CLASSES as usize {
        db.spell_info[spl].min_level[i] = LVL_IMMORT as i32;
    }

    db.spell_info[spl].mana_max = max_mana;
    db.spell_info[spl].mana_min = min_mana;
    db.spell_info[spl].mana_change = mana_change;
    db.spell_info[spl].min_position = minpos;
    db.spell_info[spl].targets = targets;
    db.spell_info[spl].violent = violent;
    db.spell_info[spl].routines = routines;
    db.spell_info[spl].name = name;
    db.spell_info[spl].wear_off_msg = if wearoff.is_empty() {
        None
    } else {
        Some(wearoff)
    };
}

fn unused_spell(db: &mut DB, spl: usize) {
    for i in 0..NUM_CLASSES as usize {
        db.spell_info[spl].min_level[i] = (LVL_IMPL + 1) as i32;
        db.spell_info[spl].mana_max = 0;
        db.spell_info[spl].mana_min = 0;
        db.spell_info[spl].mana_change = 0;
        db.spell_info[spl].min_position = 0;
        db.spell_info[spl].targets = 0;
        db.spell_info[spl].violent = false;
        db.spell_info[spl].routines = 0;
        db.spell_info[spl].name = UNUSED_SPELLNAME;
    }
}

fn skillo(db: &mut DB, skill: i32, name: &'static str) {
    spello(db, skill, name, 0, 0, 0, 0, 0, false, 0, "");
}

/*
 * Arguments for spello calls:
 *
 * spellnum, maxmana, minmana, manachng, minpos, targets, violent?, routines.
 *
 * spellnum:  Number of the spell.  Usually the symbolic name as defined in
 * spells.h (such as SPELL_HEAL).
 *
 * spellname: The name of the spell.
 *
 * maxmana :  The maximum mana this spell will take (i.e., the mana it
 * will take when the player first gets the spell).
 *
 * minmana :  The minimum mana this spell will take, no matter how high
 * level the caster is.
 *
 * manachng:  The change in mana for the spell from level to level.  This
 * number should be positive, but represents the reduction in mana cost as
 * the caster's level increases.
 *
 * minpos  :  Minimum position the caster must be in for the spell to work
 * (usually fighting or standing). targets :  A "list" of the valid targets
 * for the spell, joined with bitwise OR ('|').
 *
 * violent :  TRUE or false, depending on if this is considered a violent
 * spell and should not be cast in PEACEFUL rooms or on yourself.  Should be
 * set on any spell that inflicts damage, is considered aggressive (i.e.
 * charm, curse), or is otherwise nasty.
 *
 * routines:  A list of magic routines which are associated with this spell
 * if the spell uses spell templates.  Also joined with bitwise OR ('|').
 *
 * See the CircleMUD documentation for a more detailed description of these
 * fields.
 */

/*
 * NOTE: SPELL LEVELS ARE NO LONGER ASSIGNED HERE AS OF Circle 3.0 bpl9.
 * In order to make this cleaner, as well as to make adding new classes
 * much easier, spell levels are now assigned in class.c.  You only need
 * a spello() call to define a new spell; to decide who gets to use a spell
 * or skill, look in class.c.  -JE 5 Feb 1996
 */

pub fn mag_assign_spells(db: &mut DB) {
    /* Do not change the loop below. */
    // for i in 0..(TOP_SPELL_DEFINE as usize + 1) {
    //     unused_spell(i);
    // }
    /* Do not change the loop above. */

    spello(
        db,
        SPELL_ANIMATE_DEAD,
        "animate dead",
        35,
        10,
        3,
        POS_STANDING,
        TAR_OBJ_ROOM,
        false,
        MAG_SUMMONS,
        "",
    );

    spello(
        db,
        SPELL_ARMOR,
        "armor",
        30,
        15,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM,
        false,
        MAG_AFFECTS,
        "You feel less protected.",
    );

    spello(
        db,
        SPELL_BLESS,
        "bless",
        35,
        5,
        3,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_OBJ_INV,
        false,
        MAG_AFFECTS | MAG_ALTER_OBJS,
        "You feel less righteous.",
    );

    spello(
        db,
        SPELL_BLINDNESS,
        "blindness",
        35,
        25,
        1,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_NOT_SELF,
        false,
        MAG_AFFECTS,
        "You feel a cloak of blindness dissolve.",
    );

    spello(
        db,
        SPELL_BURNING_HANDS,
        "burning hands",
        30,
        10,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_CALL_LIGHTNING,
        "call lightning",
        40,
        25,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_CHARM,
        "charm person",
        75,
        50,
        2,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_NOT_SELF,
        true,
        MAG_MANUAL,
        "You feel more self-confident.",
    );

    spello(
        db,
        SPELL_CHILL_TOUCH,
        "chill touch",
        30,
        10,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE | MAG_AFFECTS,
        "You feel your strength return.",
    );

    spello(
        db,
        SPELL_CLONE,
        "clone",
        80,
        65,
        5,
        POS_STANDING,
        TAR_SELF_ONLY,
        false,
        MAG_SUMMONS,
        "",
    );

    spello(
        db,
        SPELL_COLOR_SPRAY,
        "color spray",
        30,
        15,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_CONTROL_WEATHER,
        "control weather",
        75,
        25,
        5,
        POS_STANDING,
        TAR_IGNORE,
        false,
        MAG_MANUAL,
        "",
    );

    spello(
        db,
        SPELL_CREATE_FOOD,
        "create food",
        30,
        5,
        4,
        POS_STANDING,
        TAR_IGNORE,
        false,
        MAG_CREATIONS,
        "",
    );

    spello(
        db,
        SPELL_CREATE_WATER,
        "create water",
        30,
        5,
        4,
        POS_STANDING,
        TAR_OBJ_INV | TAR_OBJ_EQUIP,
        false,
        MAG_MANUAL,
        "",
    );

    spello(
        db,
        SPELL_CURE_BLIND,
        "cure blind",
        30,
        5,
        2,
        POS_STANDING,
        TAR_CHAR_ROOM,
        false,
        MAG_UNAFFECTS,
        "",
    );

    spello(
        db,
        SPELL_CURE_CRITIC,
        "cure critic",
        30,
        10,
        2,
        POS_FIGHTING,
        TAR_CHAR_ROOM,
        false,
        MAG_POINTS,
        "",
    );

    spello(
        db,
        SPELL_CURE_LIGHT,
        "cure light",
        30,
        10,
        2,
        POS_FIGHTING,
        TAR_CHAR_ROOM,
        false,
        MAG_POINTS,
        "",
    );

    spello(
        db,
        SPELL_CURSE,
        "curse",
        80,
        50,
        2,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_OBJ_INV,
        true,
        MAG_AFFECTS | MAG_ALTER_OBJS,
        "You feel more optimistic.",
    );

    spello(
        db,
        SPELL_DETECT_ALIGN,
        "detect alignment",
        20,
        10,
        2,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_SELF_ONLY,
        false,
        MAG_AFFECTS,
        "You feel less aware.",
    );

    spello(
        db,
        SPELL_DETECT_INVIS,
        "detect invisibility",
        20,
        10,
        2,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_SELF_ONLY,
        false,
        MAG_AFFECTS,
        "Your eyes stop tingling.",
    );

    spello(
        db,
        SPELL_DETECT_MAGIC,
        "detect magic",
        20,
        10,
        2,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_SELF_ONLY,
        false,
        MAG_AFFECTS,
        "The detect magic wears off.",
    );

    spello(
        db,
        SPELL_DETECT_POISON,
        "detect poison",
        15,
        5,
        1,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_OBJ_INV | TAR_OBJ_ROOM,
        false,
        MAG_MANUAL,
        "The detect poison wears off.",
    );

    spello(
        db,
        SPELL_DISPEL_EVIL,
        "dispel evil",
        40,
        25,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_DISPEL_GOOD,
        "dispel good",
        40,
        25,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_EARTHQUAKE,
        "earthquake",
        40,
        25,
        3,
        POS_FIGHTING,
        TAR_IGNORE,
        true,
        MAG_AREAS,
        "",
    );

    spello(
        db,
        SPELL_ENCHANT_WEAPON,
        "enchant weapon",
        150,
        100,
        10,
        POS_STANDING,
        TAR_OBJ_INV,
        false,
        MAG_MANUAL,
        "",
    );

    spello(
        db,
        SPELL_ENERGY_DRAIN,
        "energy drain",
        40,
        25,
        1,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE | MAG_MANUAL,
        "",
    );

    spello(
        db,
        SPELL_GROUP_ARMOR,
        "group armor",
        50,
        30,
        2,
        POS_STANDING,
        TAR_IGNORE,
        false,
        MAG_GROUPS,
        "",
    );

    spello(
        db,
        SPELL_FIREBALL,
        "fireball",
        40,
        30,
        2,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_GROUP_HEAL,
        "group heal",
        80,
        60,
        5,
        POS_STANDING,
        TAR_IGNORE,
        false,
        MAG_GROUPS,
        "",
    );

    spello(
        db,
        SPELL_HARM,
        "harm",
        75,
        45,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_HEAL,
        "heal",
        60,
        40,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM,
        false,
        MAG_POINTS | MAG_UNAFFECTS,
        "",
    );

    spello(
        db,
        SPELL_INFRAVISION,
        "infravision",
        25,
        10,
        1,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_SELF_ONLY,
        false,
        MAG_AFFECTS,
        "Your night vision seems to fade.",
    );

    spello(
        db,
        SPELL_INVISIBLE,
        "invisibility",
        35,
        25,
        1,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_OBJ_INV | TAR_OBJ_ROOM,
        false,
        MAG_AFFECTS | MAG_ALTER_OBJS,
        "You feel yourself exposed.",
    );

    spello(
        db,
        SPELL_LIGHTNING_BOLT,
        "lightning bolt",
        30,
        15,
        1,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_LOCATE_OBJECT,
        "locate object",
        25,
        20,
        1,
        POS_STANDING,
        TAR_OBJ_WORLD,
        false,
        MAG_MANUAL,
        "",
    );

    spello(
        db,
        SPELL_MAGIC_MISSILE,
        "magic missile",
        25,
        10,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_POISON,
        "poison",
        50,
        20,
        3,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_NOT_SELF | TAR_OBJ_INV,
        true,
        MAG_AFFECTS | MAG_ALTER_OBJS,
        "You feel less sick.",
    );

    spello(
        db,
        SPELL_PROT_FROM_EVIL,
        "protection from evil",
        40,
        10,
        3,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_SELF_ONLY,
        false,
        MAG_AFFECTS,
        "You feel less protected.",
    );

    spello(
        db,
        SPELL_REMOVE_CURSE,
        "remove curse",
        45,
        25,
        5,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_OBJ_INV | TAR_OBJ_EQUIP,
        false,
        MAG_UNAFFECTS | MAG_ALTER_OBJS,
        "",
    );

    spello(
        db,
        SPELL_REMOVE_POISON,
        "remove poison",
        40,
        8,
        4,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_OBJ_INV | TAR_OBJ_ROOM,
        false,
        MAG_UNAFFECTS | MAG_ALTER_OBJS,
        "",
    );

    spello(
        db,
        SPELL_SANCTUARY,
        "sanctuary",
        110,
        85,
        5,
        POS_STANDING,
        TAR_CHAR_ROOM,
        false,
        MAG_AFFECTS,
        "The white aura around your body fades.",
    );

    spello(
        db,
        SPELL_SENSE_LIFE,
        "sense life",
        20,
        10,
        2,
        POS_STANDING,
        TAR_CHAR_ROOM | TAR_SELF_ONLY,
        false,
        MAG_AFFECTS,
        "You feel less aware of your surroundings.",
    );

    spello(
        db,
        SPELL_SHOCKING_GRASP,
        "shocking grasp",
        30,
        15,
        3,
        POS_FIGHTING,
        TAR_CHAR_ROOM | TAR_FIGHT_VICT,
        true,
        MAG_DAMAGE,
        "",
    );

    spello(
        db,
        SPELL_SLEEP,
        "sleep",
        40,
        25,
        5,
        POS_STANDING,
        TAR_CHAR_ROOM,
        true,
        MAG_AFFECTS,
        "You feel less tired.",
    );

    spello(
        db,
        SPELL_STRENGTH,
        "strength",
        35,
        30,
        1,
        POS_STANDING,
        TAR_CHAR_ROOM,
        false,
        MAG_AFFECTS,
        "You feel weaker.",
    );

    spello(
        db,
        SPELL_SUMMON,
        "summon",
        75,
        50,
        3,
        POS_STANDING,
        TAR_CHAR_WORLD | TAR_NOT_SELF,
        false,
        MAG_MANUAL,
        "",
    );

    spello(
        db,
        SPELL_TELEPORT,
        "teleport",
        75,
        50,
        3,
        POS_STANDING,
        TAR_CHAR_ROOM,
        false,
        MAG_MANUAL,
        "",
    );

    spello(
        db,
        SPELL_WATERWALK,
        "waterwalk",
        40,
        20,
        2,
        POS_STANDING,
        TAR_CHAR_ROOM,
        false,
        MAG_AFFECTS,
        "Your feet seem less buoyant.",
    );

    spello(
        db,
        SPELL_WORD_OF_RECALL,
        "word of recall",
        20,
        10,
        2,
        POS_FIGHTING,
        TAR_CHAR_ROOM,
        false,
        MAG_MANUAL,
        "",
    );

    /* NON-castable spells should appear below here. */

    spello(
        db,
        SPELL_IDENTIFY,
        "identify",
        0,
        0,
        0,
        0,
        TAR_CHAR_ROOM | TAR_OBJ_INV | TAR_OBJ_ROOM,
        false,
        MAG_MANUAL,
        "",
    );

    /*
     * These spells are currently not used, not implemented, and not castable.
     * Values for the 'breath' spells are filled in assuming a dragon's breath.
     */

    spello(
        db,
        SPELL_FIRE_BREATH,
        "fire breath",
        0,
        0,
        0,
        POS_SITTING,
        TAR_IGNORE,
        true,
        0,
        "",
    );

    spello(
        db,
        SPELL_GAS_BREATH,
        "gas breath",
        0,
        0,
        0,
        POS_SITTING,
        TAR_IGNORE,
        true,
        0,
        "",
    );

    spello(
        db,
        SPELL_FROST_BREATH,
        "frost breath",
        0,
        0,
        0,
        POS_SITTING,
        TAR_IGNORE,
        true,
        0,
        "",
    );

    spello(
        db,
        SPELL_ACID_BREATH,
        "acid breath",
        0,
        0,
        0,
        POS_SITTING,
        TAR_IGNORE,
        true,
        0,
        "",
    );

    spello(
        db,
        SPELL_LIGHTNING_BREATH,
        "lightning breath",
        0,
        0,
        0,
        POS_SITTING,
        TAR_IGNORE,
        true,
        0,
        "",
    );

    /*
     * Declaration of skills - this actually doesn't do anything except
     * set it up so that immortals can use these skills by default.  The
     * min level to use the skill for other classes is set up in class.c.
     */

    skillo(db, SKILL_BACKSTAB, "backstab");
    skillo(db, SKILL_BASH, "bash");
    skillo(db, SKILL_HIDE, "hide");
    skillo(db, SKILL_KICK, "kick");
    skillo(db, SKILL_PICK_LOCK, "pick lock");
    skillo(db, SKILL_RESCUE, "rescue");
    skillo(db, SKILL_SNEAK, "sneak");
    skillo(db, SKILL_STEAL, "steal");
    skillo(db, SKILL_TRACK, "track");
}
