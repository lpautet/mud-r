/* ************************************************************************
*   File: spell_parser.rs                               Part of CircleMUD *
*  Usage: top-level magic routines; outside points of entry to magic sys. *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::cell::RefCell;
use std::cmp::{max, min};

use log::error;
use crate::depot::DepotId;
use crate::VictimRef;

use crate::config::OK;
use crate::db::DB;
use crate::handler::{
    isname, FIND_CHAR_ROOM, FIND_CHAR_WORLD, FIND_OBJ_EQUIP, FIND_OBJ_INV, FIND_OBJ_ROOM,
};
use crate::interpreter::{any_one_arg, is_abbrev, one_argument};
use crate::magic::{
    mag_affects, mag_alter_objs, mag_areas, mag_creations, mag_damage, mag_groups, mag_masses,
    mag_points, mag_summons, mag_unaffects,
};
use crate::spells::{
    spell_charm, spell_create_water, spell_detect_poison, spell_enchant_weapon, spell_identify,
    spell_locate_object, spell_recall, spell_summon, spell_teleport, SpellInfoType, CAST_POTION,
    CAST_SCROLL, CAST_SPELL, CAST_STAFF, CAST_WAND, DEFAULT_STAFF_LVL, DEFAULT_WAND_LVL,
    MAG_AFFECTS, MAG_ALTER_OBJS, MAG_AREAS, MAG_CREATIONS, MAG_DAMAGE, MAG_GROUPS, MAG_MANUAL,
    MAG_MASSES, MAG_POINTS, MAG_SUMMONS, MAG_UNAFFECTS, MAX_SPELLS, SAVING_BREATH, SAVING_ROD,
    SAVING_SPELL, SKILL_BACKSTAB, SKILL_BASH, SKILL_HIDE, SKILL_KICK, SKILL_PICK_LOCK,
    SKILL_RESCUE, SKILL_SNEAK, SKILL_STEAL, SKILL_TRACK, SPELL_ACID_BREATH, SPELL_ANIMATE_DEAD,
    SPELL_ARMOR, SPELL_BLESS, SPELL_BLINDNESS, SPELL_BURNING_HANDS, SPELL_CALL_LIGHTNING,
    SPELL_CHARM, SPELL_CHILL_TOUCH, SPELL_CLONE, SPELL_COLOR_SPRAY, SPELL_CONTROL_WEATHER,
    SPELL_CREATE_FOOD, SPELL_CREATE_WATER, SPELL_CURE_BLIND, SPELL_CURE_CRITIC, SPELL_CURE_LIGHT,
    SPELL_CURSE, SPELL_DETECT_ALIGN, SPELL_DETECT_INVIS, SPELL_DETECT_MAGIC, SPELL_DETECT_POISON,
    SPELL_DISPEL_EVIL, SPELL_DISPEL_GOOD, SPELL_EARTHQUAKE, SPELL_ENCHANT_WEAPON,
    SPELL_ENERGY_DRAIN, SPELL_FIREBALL, SPELL_FIRE_BREATH, SPELL_FROST_BREATH, SPELL_GAS_BREATH,
    SPELL_GROUP_ARMOR, SPELL_GROUP_HEAL, SPELL_HARM, SPELL_HEAL, SPELL_IDENTIFY, SPELL_INFRAVISION,
    SPELL_INVISIBLE, SPELL_LIGHTNING_BOLT, SPELL_LIGHTNING_BREATH, SPELL_LOCATE_OBJECT,
    SPELL_MAGIC_MISSILE, SPELL_POISON, SPELL_PROT_FROM_EVIL, SPELL_REMOVE_CURSE,
    SPELL_REMOVE_POISON, SPELL_SANCTUARY, SPELL_SENSE_LIFE, SPELL_SHOCKING_GRASP, SPELL_SLEEP,
    SPELL_STRENGTH, SPELL_SUMMON, SPELL_TELEPORT, SPELL_WATERWALK, SPELL_WORD_OF_RECALL,
    TAR_CHAR_ROOM, TAR_CHAR_WORLD, TAR_FIGHT_SELF, TAR_FIGHT_VICT, TAR_IGNORE, TAR_NOT_SELF,
    TAR_OBJ_EQUIP, TAR_OBJ_INV, TAR_OBJ_ROOM, TAR_OBJ_WORLD, TAR_SELF_ONLY, TOP_SPELL_DEFINE,
    TYPE_UNDEFINED,
};
use crate::structs::{
    CharData, AFF_CHARM, AFF_GROUP, ITEM_POTION, ITEM_SCROLL, ITEM_STAFF, ITEM_WAND,
    LVL_IMMORT, LVL_IMPL, NUM_WEARS, POS_FIGHTING, POS_RESTING, POS_SITTING, POS_SLEEPING,
    PULSE_VIOLENCE, ROOM_NOMAGIC, ROOM_PEACEFUL,
};
use crate::structs::{NUM_CLASSES, POS_STANDING};
use crate::util::{ clone_vec2, has_spell_routine, rand_number};
use crate::{is_set,  Game, TO_CHAR, TO_ROOM, TO_VICT};

/*
 * This arrangement is pretty stupid, but the number of skills is limited by
 * the playerfile.  We can arbitrarily increase the number of skills by
 * increasing the space in the playerfile. Meanwhile, 200 should provide
 * ample slots for skills.
 */

struct Syllable {
    org: &'static str,
    news: &'static str,
}

const SYLS: [Syllable; 55] = [
    Syllable {
        org: " ",
        news: " ",
    },
    Syllable {
        org: "ar",
        news: "abra",
    },
    Syllable {
        org: "ate",
        news: "i",
    },
    Syllable {
        org: "cau",
        news: "kada",
    },
    Syllable {
        org: "blind",
        news: "nose",
    },
    Syllable {
        org: "bur",
        news: "mosa",
    },
    Syllable {
        org: "cu",
        news: "judi",
    },
    Syllable {
        org: "de",
        news: "oculo",
    },
    Syllable {
        org: "dis",
        news: "mar",
    },
    Syllable {
        org: "ect",
        news: "kamina",
    },
    Syllable {
        org: "en",
        news: "uns",
    },
    Syllable {
        org: "gro",
        news: "cra",
    },
    Syllable {
        org: "light",
        news: "dies",
    },
    Syllable {
        org: "lo",
        news: "hi",
    },
    Syllable {
        org: "magi",
        news: "kari",
    },
    Syllable {
        org: "mon",
        news: "bar",
    },
    Syllable {
        org: "mor",
        news: "zak",
    },
    Syllable {
        org: "move",
        news: "sido",
    },
    Syllable {
        org: "ness",
        news: "lacri",
    },
    Syllable {
        org: "ning",
        news: "illa",
    },
    Syllable {
        org: "per",
        news: "duda",
    },
    Syllable {
        org: "ra",
        news: "gru",
    },
    Syllable {
        org: "re",
        news: "candus",
    },
    Syllable {
        org: "son",
        news: "sabru",
    },
    Syllable {
        org: "tect",
        news: "infra",
    },
    Syllable {
        org: "tri",
        news: "cula",
    },
    Syllable {
        org: "ven",
        news: "nofo",
    },
    Syllable {
        org: "word of",
        news: "inset",
    },
    Syllable {
        org: "a",
        news: "i",
    },
    Syllable {
        org: "b",
        news: "v",
    },
    Syllable {
        org: "c",
        news: "q",
    },
    Syllable {
        org: "d",
        news: "m",
    },
    Syllable {
        org: "e",
        news: "o",
    },
    Syllable {
        org: "f",
        news: "y",
    },
    Syllable {
        org: "g",
        news: "t",
    },
    Syllable {
        org: "h",
        news: "p",
    },
    Syllable {
        org: "i",
        news: "u",
    },
    Syllable {
        org: "j",
        news: "y",
    },
    Syllable {
        org: "k",
        news: "t",
    },
    Syllable {
        org: "l",
        news: "r",
    },
    Syllable {
        org: "m",
        news: "w",
    },
    Syllable {
        org: "n",
        news: "b",
    },
    Syllable {
        org: "o",
        news: "a",
    },
    Syllable {
        org: "p",
        news: "s",
    },
    Syllable {
        org: "q",
        news: "d",
    },
    Syllable {
        org: "r",
        news: "f",
    },
    Syllable {
        org: "s",
        news: "g",
    },
    Syllable {
        org: "t",
        news: "h",
    },
    Syllable {
        org: "u",
        news: "e",
    },
    Syllable {
        org: "v",
        news: "z",
    },
    Syllable {
        org: "w",
        news: "x",
    },
    Syllable {
        org: "x",
        news: "n",
    },
    Syllable {
        org: "y",
        news: "l",
    },
    Syllable {
        org: "z",
        news: "k",
    },
    Syllable { org: "", news: "" },
];

pub const UNUSED_SPELLNAME: &str = "!UNUSED!"; /* So we can get &UNUSED_SPELLNAME */

fn mag_manacost(ch: &CharData, sinfo: &SpellInfoType) -> i16 {
    return max(
        (sinfo.mana_max
            - (sinfo.mana_change
                * (ch.get_level() as i32 - sinfo.min_level[ch.get_class() as usize])))
            as i16,
        sinfo.mana_min as i16,
    );
}

fn say_spell(
    game: &mut Game,
    chid: DepotId,
    spellnum: i32,
    tch_id: Option<DepotId>,
    tobj_id: Option<DepotId>,
) {
    let ch = game.db.ch(chid);
    let mut lbuf = String::new();
    let mut buf = String::new();
    lbuf.push_str(skill_name(&game.db, spellnum));
    let mut ofs = 0;
    while ofs < lbuf.len() {
        let mut found = false;
        for j in 0..(SYLS.len() - 1) {
            if SYLS[j].org == &lbuf[ofs..] {
                buf.push_str(SYLS[j].news); /* strcat: BAD */
                ofs += SYLS[j].org.len();
                found = true;
                break;
            }
        }
        /* i.e., we didn't find a match in SYLS[] */
        if !found {
            error!("No entry in Syllable table for substring of '{}'", lbuf);
            ofs += 1;
        }
    }
    let mut buf1 = String::new();
    let mut buf2 = String::new();
    if tch_id.is_some() && game.db.ch(tch_id.unwrap()).in_room() == ch.in_room() {
        if tch_id.unwrap() == chid {
            buf1.push_str(
                format!(
                    "$n closes $s eyes and utters the words, '{}'.",
                    skill_name(&game.db, spellnum)
                )
                .as_str(),
            );
            buf2.push_str(format!("$n closes $s eyes and utters the words, '{}'.", buf).as_str());
        } else {
            buf1.push_str(
                format!(
                    "$n stares at $N and utters the words, '{}'.",
                    skill_name(&game.db, spellnum)
                )
                .as_str(),
            );
            buf2.push_str(format!("$n stares at $N and utters the words, '{}'.", buf).as_str());
        }
    } else if tobj_id.is_some() && game.db.obj(tobj_id.unwrap()).in_room() == ch.in_room()
        || 
            game.db.obj(tobj_id.unwrap()).carried_by.unwrap() ==
            chid
    {
        buf1.push_str(
            format!(
                "$n stares at $p and utters the words, '{}'.",
                skill_name(&game.db, spellnum)
            )
            .as_str(),
        );
        buf2.push_str(format!("$n stares at $p and utters the words, '{}'.", buf).as_str());
    } else {
        buf1.push_str(format!("$n utters the words, '{}'.", skill_name(&game.db, spellnum)).as_str());
        buf2.push_str(format!("$n utters the words, '{}'.", buf).as_str());
    }

    let list = clone_vec2(&game.db.world[ch.in_room() as usize].peoples);
    for i in list {
        if i == chid
            || (tch_id.is_some() && i == tch_id.unwrap())
            || game.db.ch(i).desc.is_none()
            || !game.db.ch(i).awake()
        {
            continue;
        }
        let tch2 = if tch_id.is_some() {
            Some(tch_id.unwrap())
        } else {
            None
        };
        let ch = game.db.ch(chid);
        if ch.get_class() == game.db.ch(i).get_class() {
            game.perform_act(&buf1, Some(chid), tobj_id, Some(VictimRef::Char(tch2.unwrap())), i);
        } else {
            game.perform_act(&buf2, Some(chid), tobj_id, Some(VictimRef::Char(tch2.unwrap())), i);
        }
    }
    let ch = game.db.ch(chid);
    if tch_id.is_some()
        && tch_id.unwrap() != chid
        && game.db.ch(tch_id.unwrap()).in_room() == ch.in_room()
    {
        buf1.push_str(
            format!(
                "$n stares at you and utters the words, '{}'.",
                if ch.get_class() == game.db.ch(tch_id.unwrap()).get_class() {
                    skill_name(&game.db, spellnum)
                } else {
                    &buf
                }
            )
            .as_str(),
        );
        let tch2 = tch_id.unwrap();
        game.act(&buf1, false, Some(chid), None, Some(VictimRef::Char(tch2)), TO_VICT);
    }
}

/*
 * This function should be used anytime you are not 100% sure that you have
 * a valid spell/skill number.  A typical for() loop would not need to use
 * this because you can guarantee > 0 and <= TOP_SPELL_DEFINE.
 */
pub fn skill_name(db: &DB, num: i32) -> &'static str {
    return if num > 0 && num <= TOP_SPELL_DEFINE as i32 {
        db.spell_info[num as usize].name
    } else if num == -1 {
        "UNUSED"
    } else {
        "UNDEFINED"
    };
}

pub fn find_skill_num(db: &DB, name: &str) -> Option<i32> {
    let mut ok;
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

/*
 * This function is the very heart of the entire magic system.  All
 * invocations of all types of magic -- objects, spoken and unspoken PC
 * and NPC spells, the works -- all come through this function eventually.
 * This is also the entry point for non-spoken or unrestricted spells.
 * Spellnum 0 is legal but silently ignored here, to make callers simpler.
 */
pub fn call_magic(
    game: &mut Game,
    caster_id: DepotId,
    cvict_id: Option<DepotId>,
    ovict: Option<DepotId>,
    spellnum: i32,
    level: i32,
    casttype: i32,
) -> i32 {
    if spellnum < 1 || spellnum > TOP_SPELL_DEFINE as i32 {
        return 0;
    }
    let sinfo_routines;
    let sinfo_violent;
    {
        let sinfo = &game.db.spell_info[spellnum as usize];
        sinfo_routines = sinfo.routines;
        sinfo_violent = sinfo.violent;
    }
    if game.db.room_flagged(game.db.ch(caster_id).in_room(), ROOM_NOMAGIC) {
        game.send_to_char(caster_id, "Your magic fizzles out and dies.\r\n");
        game.act(
            "$n's magic fizzles out and dies.",
            false,
            Some(caster_id),
            None,
            None,
            TO_ROOM,
        );
        return 0;
    }
    if game.db.room_flagged(game.db.ch(caster_id).in_room(), ROOM_PEACEFUL)
        && (sinfo_violent || is_set!(sinfo_routines, MAG_DAMAGE))
    {
        game.send_to_char(
            caster_id,
            "A flash of white light fills the room, dispelling your violent magic!\r\n",
        );
        game.act(
            "White light from no particular source suddenly fills the room, then vanishes.",
            false,
            Some(caster_id),
            None,
            None,
            TO_ROOM,
        );
        return 0;
    }
    let savetype;
    /* determine the type of saving throw */
    match casttype {
        CAST_STAFF | CAST_SCROLL | CAST_POTION | CAST_WAND => {
            savetype = SAVING_ROD;
        }
        CAST_SPELL => {
            savetype = SAVING_SPELL;
        }
        _ => {
            savetype = SAVING_BREATH;
        }
    }

    if is_set!(sinfo_routines, MAG_DAMAGE) {
        if mag_damage(
            game,
            level,
            caster_id,
            cvict_id.unwrap(),
            spellnum,
            savetype,
        ) == -1
        {
            return -1; /* Successful and target died, don't cast again. */
        }
    }
    if is_set!(sinfo_routines, MAG_AFFECTS) {
        mag_affects(game, level, caster_id, cvict_id, spellnum, savetype);
    }

    if is_set!(sinfo_routines, MAG_UNAFFECTS) {
        mag_unaffects(
            game,
            level,
            caster_id,
            cvict_id.unwrap(),
            spellnum,
            savetype,
        );
    }

    if is_set!(sinfo_routines, MAG_POINTS) {
        mag_points(game, level, caster_id, cvict_id, spellnum, savetype);
    }

    if is_set!(sinfo_routines, MAG_ALTER_OBJS) {
        mag_alter_objs(game, level, caster_id, ovict, spellnum, savetype);
    }

    if is_set!(sinfo_routines, MAG_GROUPS) {
        mag_groups(game, level, Some(caster_id), spellnum, savetype);
    }
    if is_set!(sinfo_routines, MAG_MASSES) {
        mag_masses(&game.db, level, caster_id, spellnum, savetype);
    }

    if is_set!(sinfo_routines, MAG_AREAS) {
        mag_areas(game, level, Some(caster_id), spellnum, savetype);
    }

    if is_set!(sinfo_routines, MAG_SUMMONS) {
        mag_summons(game, level, Some(caster_id), ovict, spellnum, savetype);
    }

    if is_set!(sinfo_routines, MAG_CREATIONS) {
        mag_creations(game, level, Some(caster_id), spellnum);
    }

    if is_set!(sinfo_routines, MAG_MANUAL) {
        match spellnum {
            SPELL_CHARM => spell_charm(game, level, Some(caster_id), cvict_id, ovict),
            SPELL_CREATE_WATER => spell_create_water(game, level, Some(caster_id), cvict_id, ovict),
            SPELL_DETECT_POISON => spell_detect_poison(game, level, Some(caster_id), cvict_id, ovict),
            SPELL_ENCHANT_WEAPON => {
                spell_enchant_weapon(game, level, Some(caster_id), cvict_id, ovict)
            }
            SPELL_IDENTIFY => spell_identify(game, level, Some(caster_id), cvict_id, ovict),
            SPELL_LOCATE_OBJECT => spell_locate_object(game, level, Some(caster_id), cvict_id, ovict),
            SPELL_SUMMON => spell_summon(game, level, Some(caster_id), cvict_id, ovict),
            SPELL_WORD_OF_RECALL => {
                spell_recall(game, level, Some(caster_id), cvict_id, ovict);
            }
            SPELL_TELEPORT => {
                spell_teleport(game, level, Some(caster_id), cvict_id, ovict);
            }
            _ => {}
        }
    }

    return 1;
}

/*
 * mag_objectmagic: This is the entry-point for all magic items.  This should
 * only be called by the 'quaff', 'use', 'recite', etc. routines.
 *
 * For reference, object values 0-3:
 * staff  - [0]	level	[1] max charges	[2] num charges	[3] spell num
 * wand   - [0]	level	[1] max charges	[2] num charges	[3] spell num
 * scroll - [0]	level	[1] spell num	[2] spell num	[3] spell num
 * potion - [0] level	[1] spell num	[2] spell num	[3] spell num
 *
 * Staves and wands will default to level 14 if the level is not specified;
 * the DikuMUD format did not specify staff and wand levels in the world
 * files (this is a CircleMUD enhancement).
 */
pub fn mag_objectmagic(game: &mut Game, chid: DepotId, oid: DepotId, argument: &str) {
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let mut tch_id = None;
    let mut tobjid = None;
    let k = game.generic_find(
        &arg,
        (FIND_CHAR_ROOM | FIND_OBJ_INV | FIND_OBJ_ROOM | FIND_OBJ_EQUIP) as i64,
        chid,
        &mut tch_id,
        &mut tobjid,
    );

    match game.db.obj(oid).get_obj_type() {
        ITEM_STAFF => {
            game.act(
                "You tap $p three times on the ground.",
                false,
                Some(chid),
                Some(oid),
                None,
                TO_CHAR,
            );
            if !game.db.obj(oid).action_description.borrow().is_empty() {
                let str = game.db.obj(oid).action_description.borrow().clone();
                game.act(
                    str.as_str(),
                    false,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_ROOM,
                );
            } else {
                game.act(
                    "$n taps $p three times on the ground.",
                    false,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_ROOM,
                );
            }

            if game.db.obj(oid).get_obj_val(2) <= 0 {
                game.send_to_char(chid, "It seems powerless.\r\n");
                game.act(
                    "Nothing seems to happen.",
                    false,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_ROOM,
                );
            } else {
                game.db.obj_mut(oid).decr_obj_val(2);
                let ch = game.db.ch_mut(chid);
                ch.set_wait_state(PULSE_VIOLENCE as i32);
                /* Level to cast spell at. */
                let k = if game.db.obj(oid).get_obj_val(0) != 0 {
                    game.db.obj(oid).get_obj_val(0)
                } else {
                    DEFAULT_STAFF_LVL
                };

                /*
                 * Problem : Area/mass spells on staves can cause crashes.
                 * Solution: Remove the special nature of area/mass spells on staves.
                 * Problem : People like that behavior.
                 * Solution: We special case the area/mass spells here.
                 */
                let ch = game.db.ch(chid);
                if has_spell_routine(&game.db, game.db.obj(oid).get_obj_val(3), MAG_MASSES | MAG_AREAS) {
                    let mut i = game.db.world[ch.in_room() as usize]
                        .peoples
                        .len();
                    while i > 0 {
                        i -= 1;
                        call_magic(game, chid, None, None, game.db.obj(oid).get_obj_val(3), k, CAST_STAFF);
                    }
                } else {
                    let peoples_in_room =
                        clone_vec2(&game.db.world[ch.in_room() as usize].peoples);
                    for tch_id in peoples_in_room {
                        if chid != tch_id {
                            call_magic(
                                game,
                                chid,
                                Some(tch_id),
                                None,
                                game.db.obj(oid).get_obj_val(3),
                                k,
                                CAST_STAFF,
                            );
                        }
                    }
                }
            }
        }
        ITEM_WAND => {
            if k == FIND_CHAR_ROOM {
                if tch_id.unwrap() == chid {
                    game.act(
                        "You point $p at yourself.",
                        false,
                        Some(chid),
                        Some(oid),
                        None,
                        TO_CHAR,
                    );
                    game.act(
                        "$n points $p at $mself.",
                        false,
                        Some(chid),
                        Some(oid),
                        None,
                        TO_ROOM,
                    );
                } else {
                    game.act(
                        "You point $p at $N.",
                        false,
                        Some(chid),
                        Some(oid),
                        Some(VictimRef::Char(tch_id.unwrap())),
                        TO_CHAR,
                    );
                    if !RefCell::borrow(&game.db.obj(oid).action_description).is_empty() {
                        let str = game.db.obj(oid).action_description.borrow().clone();
                        game.act(
                            str.as_str(),
                            false,
                            Some(chid),
                            Some(oid),
                            Some(VictimRef::Char(tch_id.unwrap())),
                            TO_ROOM,
                        );
                    } else {
                        game.act(
                            "$n points $p at $N.",
                            true,
                            Some(chid),
                            Some(oid),
                            Some(VictimRef::Char(tch_id.unwrap())),
                            TO_ROOM,
                        );
                    }
                }
            } else if tobjid.is_some() {
                game.act(
                    "You point $p at $P.",
                    false,
                    Some(chid),
                    Some(oid),
                    Some(VictimRef::Obj(tobjid.unwrap())),
                    TO_CHAR,
                );
                if !RefCell::borrow(&game.db.obj(oid).action_description).is_empty() {
                    let str = game.db.obj(oid).action_description.borrow().clone();
                    game.act(
                        str.as_str(),
                        false,
                        Some(chid),
                        Some(oid),
                        Some(VictimRef::Obj(tobjid.unwrap())),
                        TO_ROOM,
                    );
                } else {
                    game.act(
                        "$n points $p at $P.",
                        true,
                        Some(chid),
                        Some(oid),
                        Some(VictimRef::Obj(tobjid.unwrap())),
                        TO_ROOM,
                    );
                }
            } else if is_set!(
                game.db.spell_info[game.db.obj(oid).get_obj_val(3) as usize].routines,
                MAG_AREAS | MAG_MASSES
            ) {
                /* Wands with area spells don't need to be pointed. */
                game.act(
                    "You point $p outward.",
                    false,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_CHAR,
                );
                game.act(
                    "$n points $p outward.",
                    true,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_ROOM,
                );
            } else {
                game.act(
                    "At what should $p be pointed?",
                    false,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_CHAR,
                );
                return;
            }

            if game.db.obj(oid).get_obj_val(2) <= 0 {
                game.send_to_char(chid, "It seems powerless.\r\n");
                game.act(
                    "Nothing seems to happen.",
                    false,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_ROOM,
                );
                return;
            }
            game.db.obj_mut(oid).decr_obj_val(2);
            let ch = game.db.ch_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32);
            if game.db.obj(oid).get_obj_val(0) != 0 {
                call_magic(
                    game,
                    chid,
                    tch_id,
                    tobjid,
                    game.db.obj(oid).get_obj_val(3),
                    game.db.obj(oid).get_obj_val(0),
                    CAST_WAND,
                );
            } else {
                call_magic(
                    game,
                    chid,
                    tch_id,
                    tobjid,
                    game.db.obj(oid).get_obj_val(3),
                    DEFAULT_WAND_LVL,
                    CAST_WAND,
                );
            }
        }
        ITEM_SCROLL => {
            if !arg.is_empty() {
                if k == 0 {
                    game.act(
                        "There is nothing to here to affect with $p.",
                        false,
                        Some(chid),
                        Some(oid),
                        None,
                        TO_CHAR,
                    );
                    return;
                }
            } else {
                tch_id = Some(chid);
            }

            game.act(
                "You recite $p which dissolves.",
                true,
                Some(chid),
                Some(oid),
                None,
                TO_CHAR,
            );
            if !RefCell::borrow(&game.db.obj(oid).action_description).is_empty() {
                let str = game.db.obj(oid).action_description.borrow().clone();
                game.act(
                    str.as_str(),
                    false,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_ROOM,
                );
            } else {
                game
                    .act("$n recites $p.", false, Some(chid), Some(oid), None, TO_ROOM);
            }
            let ch = game.db.ch_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32);
            for i in 1..3 {
                if call_magic(
                    game,
                    chid,
                    tch_id,
                    tobjid,
                    game.db.obj(oid).get_obj_val(i),
                    game.db.obj(oid).get_obj_val(0),
                    CAST_SCROLL,
                ) <= 0
                {
                    break;
                }
            }

            game.extract_obj(oid);
        }
        ITEM_POTION => {
            game
                .act("You quaff $p.", false, Some(chid), Some(oid), None, TO_CHAR);
            if !RefCell::borrow(&game.db.obj(oid).action_description).is_empty() {
                let str = game.db.obj(oid).action_description.borrow().clone();
                game.act(
                    str.as_str(),
                    false,
                    Some(chid),
                    Some(oid),
                    None,
                    TO_ROOM,
                );
            } else {
                game
                    .act("$n quaffs $p.", true, Some(chid), Some(oid), None, TO_ROOM);
            }
            let ch = game.db.ch_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32);
            for i in 1..3 {
                if call_magic(
                    game,
                    chid,
                    Some(chid),
                    None,
                    game.db.obj(oid).get_obj_val(i),
                    game.db.obj(oid).get_obj_val(0),
                    CAST_POTION,
                ) <= 0
                {
                    break;
                }
            }

            game.extract_obj(oid);
        }
        _ => {
            error!(
                "SYSERR: Unknown object_type {} in mag_objectmagic.",
                game.db.obj(oid).get_obj_type()
            );
        }
    }
}

/*
 * cast_spell is used generically to cast any spoken spell, assuming we
 * already have the target char/obj and spell number.  It checks all
 * restrictions, etc., prints the words, etc.
 *
 * Entry point for NPC casts.  Recommended entry point for spells cast
 * by NPCs via specprocs.
 */
pub fn cast_spell(
    game: &mut Game,
    chid: DepotId,
    tch_id: Option<DepotId>,
    tobj_id: Option<DepotId>,
    spellnum: i32,
) -> i32 {
    let ch = game.db.ch(chid);
    let db = &game.db;
    if spellnum < 0 || spellnum > TOP_SPELL_DEFINE as i32 {
        error!(
            "SYSERR: cast_spell trying to call spellnum {}/{}.",
            spellnum, TOP_SPELL_DEFINE
        );
        return 0;
    }
    let sinfo = db.spell_info[spellnum as usize];
    if ch.get_pos() < sinfo.min_position {
        match ch.get_pos() {
            POS_SLEEPING => {
                game.send_to_char(chid, "You dream about great magical powers.\r\n");
            }
            POS_RESTING => {
                game.send_to_char(chid, "You cannot concentrate while resting.\r\n");
            }
            POS_SITTING => {
                game.send_to_char(chid, "You can't do this sitting!\r\n");
            }
            POS_FIGHTING => {
                game.send_to_char(chid, "Impossible!  You can't concentrate enough!\r\n");
            }
            _ => {
                game.send_to_char(chid, "You can't do much of anything like this!\r\n");
            }
        }
        return 0;
    }
    if ch.aff_flagged(AFF_CHARM)
        && tch_id.is_some()
        && ch.master.unwrap() == tch_id.unwrap()
    {
        game.send_to_char(chid, "You are afraid you might hurt your master!\r\n");
        return 0;
    }
    if (tch_id.is_none() || chid != tch_id.unwrap()) && is_set!(sinfo.targets, TAR_SELF_ONLY) {
        game.send_to_char(chid, "You can only cast this spell upon yourself!\r\n");
        return 0;
    }
    if tch_id.is_some() && chid == tch_id.unwrap() && is_set!(sinfo.targets, TAR_NOT_SELF) {
        game.send_to_char(chid, "You cannot cast this spell upon yourself!\r\n");
        return 0;
    }
    if is_set!(sinfo.routines, MAG_GROUPS) && !ch.aff_flagged(AFF_GROUP) {
        game.send_to_char(
            chid,
            "You can't cast this spell if you're not in a group!\r\n",
        );
        return 0;
    }
    game.send_to_char(chid, OK);
    say_spell(game, chid, spellnum, tch_id, tobj_id);
    let ch = game.db.ch(chid);
    return call_magic(
        game,
        chid,
        tch_id,
        tobj_id,
        spellnum,
        ch.get_level() as i32,
        CAST_SPELL,
    );
}

/*
 * do_cast is the entry point for PC-casted spells.  It parses the arguments,
 * determines the spell number and finds a target, throws the die to see if
 * the spell can be cast, checks for sufficient mana and subtracts it, and
 * passes control to cast_spell().
 */
pub fn do_cast(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);
    if ch.is_npc() {
        return;
    }

    /* get: blank, spell name, target name */
    let mut i = argument.splitn(3, '\'');

    if i.next().is_none() {
        game.send_to_char(chid, "Cast what where?\r\n");
        return;
    }
    let s = i.next();
    if s.is_none() {
        game.send_to_char(
            chid,
            "Spell names must be enclosed in the Holy Magic Symbols: '\r\n",
        );
        return;
    }
    let s = s.unwrap();
    let mut t = i.next();
    let db = &game.db;
    /* spellnum = search_block(s, spells, 0); */
    let spellnum = find_skill_num(db, s);

    if spellnum.is_none() || spellnum.unwrap() > MAX_SPELLS {
        game.send_to_char(chid, "Cast what?!?\r\n");
        return;
    }
    let spellnum = spellnum.unwrap();
    let sinfo = db.spell_info[spellnum as usize];
    if ch.get_level() < sinfo.min_level[ch.get_class() as usize] as u8 {
        game.send_to_char(chid, "You do not know that spell!\r\n");
        return;
    }
    if ch.get_skill(spellnum) == 0 {
        game.send_to_char(chid, "You are unfamiliar with that spell.\r\n");
        return;
    }
    let arg;
    /* Find the target */
    let mut nt;
    if t.is_some() {
        arg = t.unwrap();
        nt = String::new();
        one_argument(arg, &mut nt);
        t = Some(&nt);
    }
    let mut t = t.unwrap().to_string();
    let mut target = false;
    let mut tch_id= None;
    let mut tobj_id= None;
    if is_set!(sinfo.targets, TAR_IGNORE) {
        target = true;
    } else if !t.is_empty() {
        if !target && is_set!(sinfo.targets, TAR_CHAR_ROOM) {
            if {
                tch_id = game.get_char_vis(chid, &mut t, None, FIND_CHAR_ROOM);
                tch_id.is_some()
            } {
                target = true;
            }
        }
        if !target && is_set!(sinfo.targets, TAR_CHAR_WORLD) {
            if {
                tch_id = game.get_char_vis(chid, &mut t, None, FIND_CHAR_WORLD);
                tch_id.is_some()
            } {
                target = true;
            }
        }

        if !target && is_set!(sinfo.targets, TAR_OBJ_INV) {
            if {
                tobj_id = game.get_obj_in_list_vis(ch, &t, None, &ch.carrying);
                tobj_id.is_some()
            } {
                target = true;
            }
        }

        if !target && is_set!(sinfo.targets, TAR_OBJ_EQUIP) {
            for i in 0..NUM_WEARS {
                if ch.get_eq(i).is_some() && isname(&t, game.db.obj(ch.get_eq(i).unwrap()).name.as_ref()) {
                    tobj_id = Some(ch.get_eq(i).unwrap());
                    target = true;
                }
            }
        }
        if !target && is_set!(sinfo.targets, TAR_OBJ_ROOM) {
            if {
                tobj_id = game.get_obj_in_list_vis2(
                    ch,
                    &t,
                    None,
                    &db.world[ch.in_room as usize]
                        .contents
                );
                tobj_id.is_some()
            } {
                target = true;
            }
        }
        if !target && is_set!(sinfo.targets, TAR_OBJ_WORLD) {
            if {
                tobj_id = game.get_obj_vis(ch, &t, None);
                tobj_id.is_some()
            } {
                target = true;
            }
        }
    } else {
        /* if target string is empty */
        if !target && is_set!(sinfo.targets, TAR_FIGHT_SELF) {
            if ch.fighting_id().is_some() {
                tch_id = Some(chid);
                target = true;
            }
        }
        if !target && is_set!(sinfo.targets, TAR_FIGHT_VICT) {
            if ch.fighting_id().is_some() {
                tch_id = ch.fighting_id();
                target = true;
            }
        }
        /* if no target specified, and the spell isn't violent, default to self */
        if !target && is_set!(sinfo.targets, TAR_CHAR_ROOM) && !sinfo.violent {
            tch_id = Some(chid);
            target = true;
        }
        if !target {
            game.send_to_char(
                chid,
                format!(
                    "Upon {} should the spell be cast?\r\n",
                    if is_set!(
                        sinfo.targets,
                        TAR_OBJ_ROOM | TAR_OBJ_INV | TAR_OBJ_WORLD | TAR_OBJ_EQUIP
                    ) {
                        "what"
                    } else {
                        "who"
                    }
                )
                .as_str(),
            );
            return;
        }
    }

    if target && tch_id.unwrap() == chid && sinfo.violent {
        game.send_to_char(
            chid,
            "You shouldn't cast that on yourself -- could be bad for your health!\r\n",
        );
        return;
    }
    if !target {
        game.send_to_char(chid, "Cannot find the target of your spell!\r\n");
        return;
    }
    let mana = mag_manacost(ch, &sinfo);
    if mana > 0 && ch.get_mana() < mana && ch.get_level() < LVL_IMMORT as u8 {
        game.send_to_char(chid, "You haven't the energy to cast that spell!\r\n");
        return;
    }

    /* You throws the dice and you takes your chances.. 101% is total failure */
    if rand_number(0, 101) > ch.get_skill(spellnum) as u32 {
        let ch = game.db.ch_mut(chid);
        ch.set_wait_state(PULSE_VIOLENCE as i32);
        if tch_id.is_none() || game.skill_message(0, chid, tch_id.unwrap(), spellnum) == 0 {
            game.send_to_char(chid, "You lost your concentration!\r\n");
        }
        if mana > 0 {
            let ch = game.db.ch_mut(chid);
            ch.set_mana(max(0, min(ch.get_mana(), ch.get_mana() - (mana / 2))));
        }
        if sinfo.violent && tch_id.is_some() && game.db.ch(tch_id.unwrap()).is_npc() {
            game.hit(tch_id.unwrap(), chid, TYPE_UNDEFINED);
        }
    } else {
        /* cast spell returns 1 on success; subtract mana & set waitstate */
        if cast_spell(game, chid, tch_id, tobj_id, spellnum) != 0 {
            let ch = game.db.ch_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32);
            if mana > 0 {
                ch.set_mana(max(0, min(ch.get_mana(), ch.get_mana() - mana)));
            }
        }
    }
}

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

// fn unused_spell(db: &mut DB, spl: usize) {
//     for i in 0..NUM_CLASSES as usize {
//         db.spell_info[spl].min_level[i] = (LVL_IMPL + 1) as i32;
//         db.spell_info[spl].mana_max = 0;
//         db.spell_info[spl].mana_min = 0;
//         db.spell_info[spl].mana_change = 0;
//         db.spell_info[spl].min_position = 0;
//         db.spell_info[spl].targets = 0;
//         db.spell_info[spl].violent = false;
//         db.spell_info[spl].routines = 0;
//         db.spell_info[spl].name = UNUSED_SPELLNAME;
//     }
// }

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
 * violent :  true or false, depending on if this is considered a violent
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
    /* TODO Do not change the loop below. */
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
