/* ************************************************************************
*   File: magic.rs                                      Part of CircleMUD *
*  Usage: low-level functions for magic; spell template code              *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::cmp::{max, min};
use std::rc::Rc;

use log::error;

use crate::class::saving_throws;
use crate::config::{NOEFFECT, PK_ALLOWED};
use crate::db::{DB, VIRTUAL};
use crate::fight::update_pos;
use crate::handler::{affect_from_char, affect_join, affect_remove, affected_by_spell};
use crate::spells::{
    spell_recall, MAX_SPELLS, SPELL_ANIMATE_DEAD, SPELL_ARMOR, SPELL_BLESS, SPELL_BLINDNESS,
    SPELL_BURNING_HANDS, SPELL_CALL_LIGHTNING, SPELL_CHILL_TOUCH, SPELL_CLONE, SPELL_COLOR_SPRAY,
    SPELL_CREATE_FOOD, SPELL_CURE_BLIND, SPELL_CURE_CRITIC, SPELL_CURE_LIGHT, SPELL_CURSE,
    SPELL_DETECT_ALIGN, SPELL_DETECT_INVIS, SPELL_DETECT_MAGIC, SPELL_DISPEL_EVIL,
    SPELL_DISPEL_GOOD, SPELL_EARTHQUAKE, SPELL_ENERGY_DRAIN, SPELL_FIREBALL, SPELL_GROUP_ARMOR,
    SPELL_GROUP_HEAL, SPELL_GROUP_RECALL, SPELL_HARM, SPELL_HEAL, SPELL_INFRAVISION,
    SPELL_INVISIBLE, SPELL_LIGHTNING_BOLT, SPELL_MAGIC_MISSILE, SPELL_POISON, SPELL_PROT_FROM_EVIL,
    SPELL_REMOVE_CURSE, SPELL_REMOVE_POISON, SPELL_SANCTUARY, SPELL_SENSE_LIFE,
    SPELL_SHOCKING_GRASP, SPELL_SLEEP, SPELL_STRENGTH, SPELL_WATERWALK,
};
use crate::structs::{
    AffectedType, CharData, MobVnum, ObjData, AFF_BLIND, AFF_CHARM, AFF_CURSE, AFF_DETECT_ALIGN,
    AFF_DETECT_INVIS, AFF_DETECT_MAGIC, AFF_GROUP, AFF_INFRAVISION, AFF_INVISIBLE, AFF_POISON,
    AFF_PROTECT_EVIL, AFF_SANCTUARY, AFF_SENSE_LIFE, AFF_SLEEP, AFF_WATERWALK, APPLY_AC,
    APPLY_DAMROLL, APPLY_HITROLL, APPLY_NONE, APPLY_SAVING_SPELL, APPLY_STR, CLASS_WARRIOR,
    ITEM_BLESS, ITEM_DRINKCON, ITEM_FOOD, ITEM_FOUNTAIN, ITEM_INVISIBLE, ITEM_NODROP, ITEM_NOINVIS,
    ITEM_WEAPON, LVL_IMMORT, MOB_NOBLIND, MOB_NOSLEEP, POS_SLEEPING,
};
use crate::util::{add_follower, clone_vec, dice, rand_number};
use crate::{send_to_char, Game, TO_CHAR, TO_ROOM};

/*
 * Negative apply_saving_throw[] values make saving throws better!
 * Then, so do negative modifiers.  Though people may be used to
 * the reverse of that. It's due to the code modifying the target
 * saving throw instead of the random number of the character as
 * in some other systems.
 */
pub fn mag_savingthrow(ch: &Rc<CharData>, type_: i32, modifier: i32) -> bool {
    /* NPCs use warrior tables according to some book */
    let mut class_sav = CLASS_WARRIOR;

    if !ch.is_npc() {
        class_sav = ch.get_class();
    }

    let mut save = saving_throws(class_sav, type_, ch.get_level()) as i32;
    save += ch.get_save(type_) as i32;
    save += modifier;

    /* Throwing a 0 is always a failure. */
    if max(1, save) < rand_number(0, 99) as i32 {
        return true;
    }

    /* Oops, failed. Sorry. */
    false
}

/* affect_update: called from comm.c (causes spells to wear off) */
pub fn affect_update(db: &DB) {
    for i in db.character_list.borrow().iter() {
        let mut last_type_notification = -1;
        i.affected.borrow_mut().retain_mut(|af| {
            if af.duration >= 1 {
                af.duration -= 1;
                last_type_notification = -1;
                true
            } else if af.duration == -1 {
                /* No action */
                af.duration = -1; /* GODs only! unlimited */
                last_type_notification = -1;
                true
            } else {
                if af._type > 0 && af._type <= MAX_SPELLS as i16 {
                    if af._type != last_type_notification {
                        if db.spell_info[af._type as usize].wear_off_msg.is_some() {
                            send_to_char(
                                i,
                                format!(
                                    "{}\r\n",
                                    db.spell_info[af._type as usize].wear_off_msg.unwrap()
                                )
                                .as_str(),
                            );
                            last_type_notification = af._type;
                        }
                    }
                }
                affect_remove(i, af);
                false
            }
        });
    }
}

// /*
//  *  mag_materials:
//  *  Checks for up to 3 vnums (spell reagents) in the player's inventory.
//  *
//  * No spells implemented in Circle use mag_materials, but you can use
//  * it to implement your own spells which require ingredients (i.e., some
//  * heal spell which requires a rare herb or some such.)
//  */
// int mag_materials(struct char_data *ch, int item0, int item1, int item2,
// int extract, int verbose)
// {
// struct obj_data *tobj;
// struct obj_data *obj0 = NULL, *obj1 = NULL, *obj2 = NULL;
//
// for (tobj = ch->carrying; tobj; tobj = tobj->next_content) {
// if ((item0 > 0) && (GET_OBJ_VNUM(tobj) == item0)) {
// obj0 = tobj;
// item0 = -1;
// } else if ((item1 > 0) && (GET_OBJ_VNUM(tobj) == item1)) {
// obj1 = tobj;
// item1 = -1;
// } else if ((item2 > 0) && (GET_OBJ_VNUM(tobj) == item2)) {
// obj2 = tobj;
// item2 = -1;
// }
// }
// if ((item0 > 0) || (item1 > 0) || (item2 > 0)) {
// if (verbose) {
// switch (rand_number(0, 2)) {
// case 0:
// send_to_char(ch, "A wart sprouts on your nose.\r\n");
// break;
// case 1:
// send_to_char(ch, "Your hair falls out in clumps.\r\n");
// break;
// case 2:
// send_to_char(ch, "A huge corn develops on your big toe.\r\n");
// break;
// }
// }
// return (FALSE);
// }
// if (extract) {
// if (item0 < 0)
// extract_obj(obj0);
// if (item1 < 0)
// extract_obj(obj1);
// if (item2 < 0)
// extract_obj(obj2);
// }
// if (verbose) {
// send_to_char(ch, "A puff of smoke rises from your pack.\r\n");
// act("A puff of smoke rises from $n's pack.", TRUE, ch, NULL, NULL, TO_ROOM);
// }
// return (TRUE);
// }

/*
 * Every spell that does damage comes through here.  This calculates the
 * amount of damage, adds in any modifiers, determines what the saves are,
 * tests for save and calls damage().
 *
 * -1 = dead, otherwise the amount of damage done.
 */
pub fn mag_damage(
    game: &mut Game,
    level: i32,
    ch: &Rc<CharData>,
    victim: &Rc<CharData>,
    spellnum: i32,
    savetype: i32,
) -> i32 {
    let db = &game.db;
    let mut dam = 0;
    let mut victim = victim;

    match spellnum {
        /* Mostly mages */
        SPELL_MAGIC_MISSILE | SPELL_CHILL_TOUCH => {
            /* chill touch also has an affect */
            if ch.is_magic_user() {
                dam = dice(1, 8) + 1;
            } else {
                dam = dice(1, 6) + 1;
            }
        }
        SPELL_BURNING_HANDS => {
            if ch.is_magic_user() {
                dam = dice(3, 8) + 3;
            } else {
                dam = dice(3, 6) + 3;
            }
        }
        SPELL_SHOCKING_GRASP => {
            if ch.is_magic_user() {
                dam = dice(5, 8) + 5;
            } else {
                dam = dice(5, 6) + 5;
            }
        }
        SPELL_LIGHTNING_BOLT => {
            if ch.is_magic_user() {
                dam = dice(7, 8) + 7;
            } else {
                dam = dice(7, 6) + 7;
            }
        }
        SPELL_COLOR_SPRAY => {
            if ch.is_magic_user() {
                dam = dice(9, 8) + 9;
            } else {
                dam = dice(9, 6) + 9;
            }
        }
        SPELL_FIREBALL => {
            if ch.is_magic_user() {
                dam = dice(11, 8) + 11;
            } else {
                dam = dice(11, 6) + 11;
            }
        }

        /* Mostly clerics */
        SPELL_DISPEL_EVIL => {
            dam = dice(6, 8) + 6;
            if ch.is_evil() {
                victim = ch;
                dam = (ch.get_hit() - 1) as i32;
            } else if victim.is_good() {
                db.act(
                    "The gods protect $N.",
                    false,
                    Some(ch),
                    None,
                    Some(victim),
                    TO_CHAR,
                );
                return 0;
            }
        }
        SPELL_DISPEL_GOOD => {
            dam = dice(6, 8) + 6;
            if ch.is_good() {
                victim = ch;
                dam = (ch.get_hit() - 1) as i32;
            } else if victim.is_evil() {
                db.act(
                    "The gods protect $N.",
                    false,
                    Some(ch),
                    None,
                    Some(victim),
                    TO_CHAR,
                );
                return 0;
            }
        }

        SPELL_CALL_LIGHTNING => {
            dam = dice(7, 8) + 7;
        }

        SPELL_HARM => {
            dam = dice(8, 8) + 8;
        }

        SPELL_ENERGY_DRAIN => {
            if victim.get_level() <= 2 {
                dam = 100;
            } else {
                dam = dice(1, 10);
            }
        }

        /* Area spells */
        SPELL_EARTHQUAKE => {
            dam = dice(2, 8) + level;
        }
        _ => {}
    } /* switch(spellnum) */

    /* divide damage by two if victim makes his saving throw */
    if mag_savingthrow(victim, savetype, 0) {
        dam /= 2;
    }

    /* and finally, inflict the damage */
    return game.damage(ch, victim, dam, spellnum);
}

/*
 * Every spell that does an affect comes through here.  This determines
 * the effect, whether it is added or replacement, whether it is legal or
 * not, etc.
 *
 * affect_join(vict, aff, add_dur, avg_dur, add_mod, avg_mod)
 */

const MAX_SPELL_AFFECTS: i32 = 5; /* change if more needed */

pub fn mag_affects(
    db: &DB,
    level: i32,
    ch: &Rc<CharData>,
    victim: Option<&Rc<CharData>>,
    spellnum: i32,
    savetype: i32,
) {
    let mut victim = victim;
    let mut af = [AffectedType {
        _type: 0,
        duration: 0,
        modifier: 0,
        location: 0,
        bitvector: 0,
    }; MAX_SPELL_AFFECTS as usize];

    for i in 0..MAX_SPELL_AFFECTS as usize {
        af[i]._type = spellnum as i16;
        af[i].bitvector = 0;
        af[i].modifier = 0;
        af[i].location = APPLY_NONE as u8;
    }
    let mut accum_duration = false;
    let mut to_vict = "";
    let mut to_room = "";
    let mut accum_affect = false;
    match spellnum {
        SPELL_CHILL_TOUCH => {
            af[0].location = APPLY_STR as u8;
            if mag_savingthrow(victim.as_ref().unwrap(), savetype, 0) {
                af[0].duration = 1;
            } else {
                af[0].duration = 4;
            }
            af[0].modifier = -1;
            accum_duration = true;
            to_vict = "You feel your strength wither!";
        }

        SPELL_ARMOR => {
            af[0].location = APPLY_AC as u8;
            af[0].modifier = -20;
            af[0].duration = 24;
            accum_duration = true;
            to_vict = "You feel someone protecting you.";
        }
        SPELL_BLESS => {
            af[0].location = APPLY_HITROLL as u8;
            af[0].modifier = 2;
            af[0].duration = 6;

            af[1].location = APPLY_SAVING_SPELL as u8;
            af[1].modifier = -1;
            af[1].duration = 6;

            accum_duration = true;
            to_vict = "You feel righteous.";
        }
        SPELL_BLINDNESS => {
            if victim.as_ref().unwrap().mob_flagged(MOB_NOBLIND)
                || mag_savingthrow(victim.as_ref().unwrap(), savetype, 0)
            {
                send_to_char(ch, "You fail.\r\n");
                return;
            }

            af[0].location = APPLY_HITROLL as u8;
            af[0].modifier = -4;
            af[0].duration = 2;
            af[0].bitvector = AFF_BLIND;

            af[1].location = APPLY_AC as u8;
            af[1].modifier = 40;
            af[1].duration = 2;
            af[1].bitvector = AFF_BLIND;

            to_room = "$n seems to be blinded!";
            to_vict = "You have been blinded!";
        }
        SPELL_CURSE => {
            if mag_savingthrow(victim.as_ref().unwrap(), savetype, 0) {
                send_to_char(ch, NOEFFECT);
                return;
            }

            af[0].location = APPLY_HITROLL as u8;
            af[0].duration = (1 + (ch.get_level() / 2)) as i16;
            af[0].modifier = -1;
            af[0].bitvector = AFF_CURSE;

            af[1].location = APPLY_DAMROLL as u8;
            af[1].duration = (1 + (ch.get_level() / 2)) as i16;
            af[1].modifier = -1;
            af[1].bitvector = AFF_CURSE;

            accum_duration = true;
            accum_affect = true;
            to_room = "$n briefly glows red!";
            to_vict = "You feel very uncomfortable.";
        }
        SPELL_DETECT_ALIGN => {
            af[0].duration = 12 + level as i16;
            af[0].bitvector = AFF_DETECT_ALIGN;
            accum_duration = true;
            to_vict = "Your eyes tingle.";
        }
        SPELL_DETECT_INVIS => {
            af[0].duration = 12 + level as i16;
            af[0].bitvector = AFF_DETECT_INVIS;
            accum_duration = true;
            to_vict = "Your eyes tingle.";
        }
        SPELL_DETECT_MAGIC => {
            af[0].duration = 12 + level as i16;
            af[0].bitvector = AFF_DETECT_MAGIC;
            accum_duration = true;
            to_vict = "Your eyes tingle.";
        }
        SPELL_INFRAVISION => {
            af[0].duration = 12 + level as i16;
            af[0].bitvector = AFF_INFRAVISION;
            accum_duration = true;
            to_vict = "Your eyes glow red.";
            to_room = "$n's eyes glow red.";
        }

        SPELL_INVISIBLE => {
            if victim.is_none() {
                victim = Some(ch);
            }

            af[0].duration = 12 + (ch.get_level() as i16 / 4);
            af[0].modifier = -40;
            af[0].location = APPLY_AC as u8;
            af[0].bitvector = AFF_INVISIBLE;
            accum_duration = true;
            to_vict = "You vanish.";
            to_room = "$n slowly fades out of existence.";
        }
        SPELL_POISON => {
            if mag_savingthrow(victim.as_ref().unwrap(), savetype, 0) {
                send_to_char(ch, NOEFFECT);
                return;
            }

            af[0].location = APPLY_STR as u8;
            af[0].duration = ch.get_level() as i16;
            af[0].modifier = -2;
            af[0].bitvector = AFF_POISON;
            to_vict = "You feel very sick.";
            to_room = "$n gets violently ill!";
        }
        SPELL_PROT_FROM_EVIL => {
            af[0].duration = 24;
            af[0].bitvector = AFF_PROTECT_EVIL;
            accum_duration = true;
            to_vict = "You feel invulnerable!";
        }
        SPELL_SANCTUARY => {
            af[0].duration = 4;
            af[0].bitvector = AFF_SANCTUARY;

            accum_duration = true;
            to_vict = "A white aura momentarily surrounds you.";
            to_room = "$n is surrounded by a white aura.";
        }
        SPELL_SLEEP => {
            if !PK_ALLOWED && !ch.is_npc() && !victim.as_ref().unwrap().is_npc() {
                return;
            }
            if victim.as_ref().unwrap().mob_flagged(MOB_NOSLEEP) {
                return;
            }
            if mag_savingthrow(victim.as_ref().unwrap(), savetype, 0) {
                return;
            }

            af[0].duration = 4 + (ch.get_level() as i16 / 4);
            af[0].bitvector = AFF_SLEEP;

            if victim.as_ref().unwrap().get_pos() > POS_SLEEPING {
                send_to_char(
                    victim.as_ref().unwrap(),
                    "You feel very sleepy...  Zzzz......\r\n",
                );
                db.act("$n goes to sleep.", true, Some(victim.unwrap().as_ref()), None, None, TO_ROOM);
                victim.as_ref().unwrap().set_pos(POS_SLEEPING);
            }
        }
        SPELL_STRENGTH => {
            if victim.as_ref().unwrap().get_add() == 100 {
                return;
            }

            af[0].location = APPLY_STR as u8;
            af[0].duration = (ch.get_level() as i16 / 2) + 4;
            af[0].modifier = 1 + if level > 18 { 1 } else { 0 };
            accum_duration = true;
            accum_affect = true;
            to_vict = "You feel stronger!";
        }

        SPELL_SENSE_LIFE => {
            to_vict = "Your feel your awareness improve.";
            af[0].duration = ch.get_level() as i16;
            af[0].bitvector = AFF_SENSE_LIFE;
            accum_duration = true;
        }
        SPELL_WATERWALK => {
            af[0].duration = 24;
            af[0].bitvector = AFF_WATERWALK;
            accum_duration = true;
            to_vict = "You feel webbing between your toes.";
        }
        _ => {}
    }

    /*
     * If this is a mob that has this affect set in its mob file, do not
     * perform the affect.  This prevents people from un-sancting mobs
     * by sancting them and waiting for it to fade, for example.
     */
    if victim.as_ref().unwrap().is_npc()
        && !affected_by_spell(victim.as_ref().unwrap(), spellnum as i16)
    {
        for i in 0..MAX_SPELL_AFFECTS as usize {
            if victim.as_ref().unwrap().aff_flagged(af[i].bitvector) {
                send_to_char(ch, NOEFFECT);
                return;
            }
        }
    }

    /*
     * If the victim is already affected by this spell, and the spell does
     * not have an accumulative effect, then fail the spell.
     */
    if affected_by_spell(victim.as_ref().unwrap(), spellnum as i16)
        && !(accum_duration || accum_affect)
    {
        send_to_char(ch, NOEFFECT);
        return;
    }

    for i in 0..MAX_SPELL_AFFECTS as usize {
        if af[i].bitvector != 0 || af[i].location != APPLY_NONE as u8 {
            affect_join(
                victim.as_ref().unwrap(),
                &mut af[i],
                accum_duration,
                false,
                accum_affect,
                false,
            );
        }
    }

    if !to_vict.is_empty() {
        db.act(to_vict, false, if victim.is_none() { None} else {Some(victim.unwrap())}, None, Some(ch), TO_CHAR);
    }
    if !to_room.is_empty() {
        db.act(to_room, true, if victim.is_none() { None} else {Some(victim.unwrap())}, None, Some(ch), TO_ROOM);
    }
}
/*
 * This function is used to provide services to mag_groups.  This function
 * is the one you should change to add new group spells.
 */
fn perform_mag_groups(
    db: &DB,
    level: i32,
    ch: &Rc<CharData>,
    tch: &Rc<CharData>,
    spellnum: i32,
    savetype: i32,
) {
    match spellnum {
        SPELL_GROUP_HEAL => {
            mag_points(level, ch, Some(tch), SPELL_HEAL, savetype);
        }
        SPELL_GROUP_ARMOR => {
            mag_affects(db, level, ch, Some(tch), SPELL_ARMOR, savetype);
        }
        SPELL_GROUP_RECALL => {
            spell_recall(db, level, Some(ch), Some(tch), None);
        }
        _ => {}
    }
}

/*
 * Every spell that affects the group should run through here
 * perform_mag_groups contains the switch statement to send us to the right
 * magic.
 *
 * group spells affect everyone grouped with the caster who is in the room,
 * caster last.
 *
 * To add new group spells, you shouldn't have to change anything in
 * mag_groups -- just add a new case to perform_mag_groups.
 */
pub fn mag_groups(db: &DB, level: i32, ch: Option<&Rc<CharData>>, spellnum: i32, savetype: i32) {
    if ch.is_none() {
        return;
    }
    let ch = ch.unwrap();

    if !ch.aff_flagged(AFF_GROUP) {
        return;
    }
    let k;
    if ch.master.borrow().is_some() {
        k = ch.master.borrow().as_ref().unwrap().clone();
    } else {
        k = ch.clone();
    }
    for f in k.followers.borrow().iter() {
        let tch = f.follower.clone();
        if tch.in_room() != ch.in_room() {
            continue;
        }
        if !tch.aff_flagged(AFF_GROUP) {
            continue;
        }
        if Rc::ptr_eq(ch, &tch) {
            continue;
        }
        perform_mag_groups(db, level, ch, &tch, spellnum, savetype);
    }

    if !Rc::ptr_eq(&k, ch) && k.aff_flagged(AFF_GROUP) {
        perform_mag_groups(db, level, ch, &k, spellnum, savetype);
    }
    perform_mag_groups(db, level, ch, ch, spellnum, savetype);
}

/*
 * mass spells affect every creature in the room except the caster.
 *
 * No spells of this class currently implemented.
 */
pub fn mag_masses(db: &DB, _level: i32, ch: &Rc<CharData>, spellnum: i32, _savetype: i32) {
    for tch in db.world.borrow()[ch.in_room() as usize]
        .peoples
        .borrow()
        .iter()
    {
        if Rc::ptr_eq(tch, ch) {
            continue;
        }

        match spellnum {
            _ => {}
        }
    }
}

/*
 * Every spell that affects an area (room) runs through here.  These are
 * generally offensive spells.  This calls mag_damage to do the actual
 * damage -- all spells listed here must also have a case in mag_damage()
 * in order for them to work.
 *
 *  area spells have limited targets within the room.
 */
pub fn mag_areas(
    game: &mut Game,
    level: i32,
    ch: Option<&Rc<CharData>>,
    spellnum: i32,
    _savetype: i32,
) {
    let mut to_char = "";
    let mut to_room = "";

    if ch.is_none() {
        return;
    }
    let ch = ch.unwrap();
    /*
     * to add spells to this fn, just add the message here plus an entry
     * in mag_damage for the damaging part of the spell.
     */
    match spellnum {
        SPELL_EARTHQUAKE => {
            to_char = "You gesture and the earth begins to shake all around you!";
            to_room = "$n gracefully gestures and the earth begins to shake violently!";
        }
        _ => {}
    }

    if !to_char.is_empty() {
        game.db.act(to_char, false, Some(ch), None, None, TO_CHAR);
    }
    if !to_room.is_empty() {
        game.db.act(to_room, false, Some(ch), None, None, TO_ROOM);
    }
    let peoples = clone_vec(&game.db.world.borrow()[ch.in_room() as usize].peoples);
    for tch in peoples.iter() {
        /*
         * The skips: 1: the caster
         *            2: immortals
         *            3: if no pk on this mud, skips over all players
         *            4: pets (charmed NPCs)
         */

        if Rc::ptr_eq(ch, tch) {
            continue;
        }
        if !tch.is_npc() && tch.get_level() >= LVL_IMMORT as u8 {
            continue;
        }
        if !PK_ALLOWED && !ch.is_npc() && !tch.is_npc() {
            continue;
        }
        if !ch.is_npc() && tch.is_npc() && tch.aff_flagged(AFF_CHARM) {
            continue;
        }

        /* Doesn't matter if they die here so we don't check. -gg 6/24/98 */
        mag_damage(game, level, ch, tch, spellnum, 1);
    }
}

/*
 *  Every spell which summons/gates/conjours a mob comes through here.
 *
 *  None of these spells are currently implemented in CircleMUD; these
 *  were taken as examples from the JediMUD code.  Summons can be used
 *  for spells like clone, ariel servant, etc.
 *
 * 10/15/97 (gg) - Implemented Animate Dead and Clone.
 */

/*
 * These use act(), don't put the \r\n.
 */
const MAG_SUMMON_MSGS: [&str; 13] = [
    "\r\n",
    "$n makes a strange magical gesture; you feel a strong breeze!",
    "$n animates a corpse!",
    "$N appears from a cloud of thick blue smoke!",
    "$N appears from a cloud of thick green smoke!",
    "$N appears from a cloud of thick red smoke!",
    "$N disappears in a thick black cloud!",
    "As $n makes a strange magical gesture, you feel a strong breeze.",
    "As $n makes a strange magical gesture, you feel a searing heat.",
    "As $n makes a strange magical gesture, you feel a sudden chill.",
    "As $n makes a strange magical gesture, you feel the dust swirl.",
    "$n magically divides!",
    "$n animates a corpse!",
];

/*
 * Keep the \r\n because these use send_to_char.
 */
const MAG_SUMMON_FAIL_MSGS: [&str; 8] = [
    "\r\n",
    "There are no such creatures.\r\n",
    "Uh oh...\r\n",
    "Oh dear.\r\n",
    "Gosh durnit!\r\n",
    "The elements resist!\r\n",
    "You failed.\r\n",
    "There is no corpse!\r\n",
];

// /* Defined mobiles. */
const MOB_CLONE: i32 = 10;
const MOB_ZOMBIE: i32 = 11;

pub fn mag_summons(
    db: &DB,
    _level: i32,
    ch: Option<&Rc<CharData>>,
    obj: Option<&Rc<ObjData>>,
    spellnum: i32,
    _savetype: i32,
) {
    let pfail;
    let msg;
    let num = 1;
    let mut handle_corpse = false;
    let fmsg;
    let mob_num: MobVnum;

    if ch.is_none() {
        return;
    }
    let ch = ch.unwrap();

    match spellnum {
        SPELL_CLONE => {
            msg = 10;
            fmsg = rand_number(2, 6); /* Random fail message. */
            mob_num = MOB_CLONE as MobVnum;
            pfail = 50; /* 50% failure, should be based on something later. */
        }
        SPELL_ANIMATE_DEAD => {
            if obj.is_none() || !obj.unwrap().is_corpse() {
                db.act(
                    MAG_SUMMON_FAIL_MSGS[7],
                    false,
                    Some(ch),
                    None,
                    None,
                    TO_CHAR,
                );
                return;
            }
            handle_corpse = true;
            msg = 11;
            fmsg = rand_number(2, 6); /* Random fail message. */
            mob_num = MOB_ZOMBIE as MobVnum;
            pfail = 10; /* 10% failure, should vary in the future. */
        }

        _ => {
            return;
        }
    }

    if ch.aff_flagged(AFF_CHARM) {
        send_to_char(ch, "You are too giddy to have any followers!\r\n");
        return;
    }
    if rand_number(0, 101) < pfail {
        send_to_char(ch, MAG_SUMMON_FAIL_MSGS[fmsg as usize]);
        return;
    }
    for _ in 0..num {
        let mob;
        if {
            mob = db.read_mobile(mob_num, VIRTUAL);
            mob.is_none()
        } {
            send_to_char(
                ch,
                "You don't quite remember how to make that creature.\r\n",
            );
            return;
        }
        let mob = mob.as_ref().unwrap();
        db.char_to_room(mob, ch.in_room());
        mob.set_is_carrying_w(0);
        mob.set_is_carrying_n(0);
        mob.set_aff_flags_bits(AFF_CHARM);
        if spellnum == SPELL_CLONE {
            /* Don't mess up the prototype; use new string copies. */
            mob.player.borrow_mut().name = ch.get_name().to_string();
            mob.player.borrow_mut().short_descr = ch.get_name().to_string();
        }
        db.act(
            MAG_SUMMON_MSGS[msg],
            false,
            Some(ch),
            None,
            Some(mob),
            TO_ROOM,
        );
        add_follower(db, mob, ch);

        if handle_corpse {
            for tobj in clone_vec(&obj.as_ref().unwrap().contains) {
                DB::obj_from_obj(&tobj);
                DB::obj_to_char(&tobj, mob);
            }
            db.extract_obj(obj.as_ref().unwrap());
        }
    }
}

pub fn mag_points(
    level: i32,
    _ch: &Rc<CharData>,
    victim: Option<&Rc<CharData>>,
    spellnum: i32,
    _savetype: i32,
) {
    let healing;
    let move_ = 0;

    if victim.is_none() {
        return;
    }
    let victim = victim.unwrap();

    match spellnum {
        SPELL_CURE_LIGHT => {
            healing = dice(1, 8) + 1 + (level / 4);
            send_to_char(victim, "You feel better.\r\n");
        }
        SPELL_CURE_CRITIC => {
            healing = dice(3, 8) + 3 + (level / 4);
            send_to_char(victim, "You feel a lot better!\r\n");
        }
        SPELL_HEAL => {
            healing = 100 + dice(3, 8);
            send_to_char(victim, "A warm feeling floods your body.\r\n");
        }
        _ => {
            return;
        }
    }
    victim.set_hit(min(victim.get_max_hit(), victim.get_hit() + healing as i16));
    victim.set_move(min(victim.get_max_move(), victim.get_move() + move_));
    update_pos(victim);
}

pub fn mag_unaffects(
    db: &DB,
    _level: i32,
    ch: &Rc<CharData>,
    victim: &Rc<CharData>,
    spellnum: i32,
    _type_: i32,
) {
    let spell;
    let mut msg_not_affected = true;
    let to_vict;
    let mut to_room = "";

    match spellnum {
        SPELL_HEAL => {
            /*
             * Heal also restores health, so don't give the "no effect" message
             * if the target isn't afflicted by the 'blindness' spell.
             */
            msg_not_affected = false;
            to_vict = "Your vision returns!";
            to_room = "There's a momentary gleam in $n's eyes.";
            spell = SPELL_BLINDNESS;
        }
        /* fall-through */
        SPELL_CURE_BLIND => {
            spell = SPELL_BLINDNESS;
            to_vict = "Your vision returns!";
            to_room = "There's a momentary gleam in $n's eyes.";
        }
        SPELL_REMOVE_POISON => {
            spell = SPELL_POISON;
            to_vict = "A warm feeling runs through your body!";
            to_room = "$n looks better.";
        }
        SPELL_REMOVE_CURSE => {
            spell = SPELL_CURSE;
            to_vict = "You don't feel so unlucky.";
        }
        _ => {
            error!(
                "SYSERR: unknown spellnum {} passed to mag_unaffects.",
                spellnum
            );
            return;
        }
    }

    if !affected_by_spell(victim, spell as i16) {
        if msg_not_affected {
            send_to_char(ch, NOEFFECT);
        }
        return;
    }

    affect_from_char(victim, spell as i16);
    if !to_vict.is_empty() {
        db.act(to_vict, false, Some(victim), None, Some(ch), TO_CHAR);
    }
    if !to_room.is_empty() {
        db.act(to_room, false, Some(victim), None, Some(ch), TO_ROOM);
    }
}

pub fn mag_alter_objs(
    db: &DB,
    _level: i32,
    ch: &Rc<CharData>,
    obj: Option<&Rc<ObjData>>,
    spellnum: i32,
    _savetype: i32,
) {
    let mut to_char = "";
    let to_room = "";

    if obj.is_none() {
        return;
    }
    let obj = obj.unwrap();

    match spellnum {
        SPELL_BLESS => {
            if !obj.obj_flagged(ITEM_BLESS) && (obj.get_obj_weight() <= 5 * ch.get_level() as i32) {
                obj.set_obj_extra_bit(ITEM_BLESS);
                to_char = "$p glows briefly.";
            }
        }
        SPELL_CURSE => {
            if !obj.obj_flagged(ITEM_NODROP) {
                obj.set_obj_extra_bit(ITEM_NODROP);
                if obj.get_obj_type() == ITEM_WEAPON {
                    obj.decr_obj_val(2);
                }
                to_char = "$p briefly glows red.";
            }
        }
        SPELL_INVISIBLE => {
            if !obj.obj_flagged(ITEM_NOINVIS | ITEM_INVISIBLE) {
                obj.set_obj_extra_bit(ITEM_INVISIBLE);
                to_char = "$p vanishes.";
            }
        }
        SPELL_POISON => {
            if ((obj.get_obj_type() == ITEM_DRINKCON)
                || (obj.get_obj_type() == ITEM_FOUNTAIN)
                || (obj.get_obj_type() == ITEM_FOOD))
                && obj.get_obj_val(3) == 0
            {
                obj.set_obj_val(3, 1);
                to_char = "$p steams briefly.";
            }
        }
        SPELL_REMOVE_CURSE => {
            if obj.obj_flagged(ITEM_NODROP) {
                obj.remove_obj_extra_bit(ITEM_NODROP);
            }
            if obj.get_obj_type() == ITEM_WEAPON {
                obj.incr_obj_val(2);
                to_char = "$p briefly glows blue.";
            }
        }

        SPELL_REMOVE_POISON => {
            if (obj.get_obj_type() == ITEM_DRINKCON)
                || ((obj.get_obj_type() == ITEM_FOUNTAIN)
                    || (obj.get_obj_type() == ITEM_FOOD) && obj.get_obj_val(3) != 0)
            {
                obj.set_obj_val(3, 0);
                to_char = "$p steams briefly.";
            }
        }
        _ => {}
    }

    if to_char.is_empty() {
        send_to_char(ch, NOEFFECT);
    } else {
        db.act(to_char, true, Some(ch), Some(obj), None, TO_CHAR);
    }

    if !to_room.is_empty() {
        db.act(to_room, true, Some(ch), Some(obj), None, TO_ROOM);
    } else if !to_char.is_empty() {
        db.act(to_char, true, Some(ch), Some(obj), None, TO_ROOM);
    }
}

pub fn mag_creations(db: &DB, _level: i32, ch: Option<&Rc<CharData>>, spellnum: i32) {
    if ch.is_none() {
        return;
    }
    let ch = ch.unwrap();
    /* level = MAX(MIN(level, LVL_IMPL), 1); - Hm, not used. */
    let z;
    match spellnum {
        SPELL_CREATE_FOOD => {
            z = 10;
        }
        _ => {
            send_to_char(ch, "Spell unimplemented, it would seem.\r\n");
            return;
        }
    }
    let tobj = db.read_object(z, VIRTUAL);
    if tobj.is_none() {
        send_to_char(ch, "I seem to have goofed.\r\n");
        error!(
            "SYSERR: spell_creations, spell {}, obj {}: obj not found",
            spellnum, z
        );
        return;
    }
    let tobj = tobj.unwrap();
    DB::obj_to_char(&tobj, ch);
    db.act(
        "$n creates $p.",
        false,
        Some(ch),
        Some(tobj.as_ref()),
        None,
        TO_ROOM,
    );
    db.act(
        "You create $p.",
        false,
        Some(ch),
        Some(tobj.as_ref()),
        None,
        TO_CHAR,
    );
}
