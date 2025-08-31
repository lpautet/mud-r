/* ************************************************************************
*   File: spell_parser.rs                               Part of CircleMUD *
*  Usage: top-level magic routines; outside points of entry to magic sys. *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use std::cmp::{max, min};

use crate::depot::{Depot, DepotId, HasId};
use crate::fight::skill_message;
use crate::{act, perform_act, send_to_char, ObjData, TextData, VictimRef};
use log::error;

use crate::config::OK;
use crate::db::DB;
use crate::handler::{
    generic_find, get_char_vis, get_obj_in_list_vis, get_obj_in_list_vis2, get_obj_vis, isname,
    FindFlags,
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
use crate::structs::NUM_CLASSES;
use crate::structs::{
    AffectFlags, CharData, Class, ItemType, Position, RoomFlags, LVL_IMMORT, LVL_IMPL, NUM_WEARS,
    PULSE_VIOLENCE,
};
use crate::util::{has_spell_routine, rand_number};
use crate::{is_set, Game, TO_CHAR, TO_ROOM, TO_VICT};

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
    max(
        (sinfo.mana_max
            - (sinfo.mana_change
                * (ch.get_level() as i32 - sinfo.min_level[ch.get_class() as usize])))
            as i16,
        sinfo.mana_min as i16,
    )
}

#[allow(clippy::too_many_arguments)]
fn say_spell(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    objs: &Depot<ObjData>,
    chid: DepotId,
    spellnum: i32,
    tch_id: Option<DepotId>,
    tobj_id: Option<DepotId>,
) {
    let ch = chars.get(chid);
    let mut lbuf = String::new();
    let mut buf = String::new();
    lbuf.push_str(skill_name(db, spellnum));
    let mut ofs = 0;
    while ofs < lbuf.len() {
        let mut found = false;
        for syl in SYLS.iter() {
            if syl.org == &lbuf[ofs..] {
                buf.push_str(syl.news); /* strcat: BAD */
                ofs += syl.org.len();
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
    #[allow(clippy::unnecessary_unwrap)]
    if tch_id.is_some() && chars.get(tch_id.unwrap()).in_room() == ch.in_room() {
        if tch_id.unwrap() == chid {
            buf1.push_str(
                format!(
                    "$n closes $s eyes and utters the words, '{}'.",
                    skill_name(db, spellnum)
                )
                .as_str(),
            );
            buf2.push_str(format!("$n closes $s eyes and utters the words, '{}'.", buf).as_str());
        } else {
            buf1.push_str(
                format!(
                    "$n stares at $N and utters the words, '{}'.",
                    skill_name(db, spellnum)
                )
                .as_str(),
            );
            buf2.push_str(format!("$n stares at $N and utters the words, '{}'.", buf).as_str());
        }
    } else if tobj_id.is_some() && objs.get(tobj_id.unwrap()).in_room() == ch.in_room()
        || objs.get(tobj_id.unwrap()).carried_by.unwrap() == chid
    {
        buf1.push_str(
            format!(
                "$n stares at $p and utters the words, '{}'.",
                skill_name(db, spellnum)
            )
            .as_str(),
        );
        buf2.push_str(format!("$n stares at $p and utters the words, '{}'.", buf).as_str());
    } else {
        buf1.push_str(format!("$n utters the words, '{}'.", skill_name(db, spellnum)).as_str());
        buf2.push_str(format!("$n utters the words, '{}'.", buf).as_str());
    }

    for &i_id in &db.world[ch.in_room() as usize].peoples {
        if i_id == chid
            || (tch_id.is_some() && i_id == tch_id.unwrap())
            || chars.get(i_id).desc.is_none()
            || !chars.get(i_id).awake()
        {
            continue;
        }
        #[allow(clippy::unnecessary_unwrap)]
        let tch2_id = if tch_id.is_some() {
            Some(tch_id.unwrap())
        } else {
            None
        };
        let ch = chars.get(chid);
        let i = chars.get(i_id);
        let toobj = tobj_id.map(|id| objs.get(id));
        let tch2 = chars.get(tch2_id.unwrap());
        if ch.get_class() == chars.get(i_id).get_class() {
            perform_act(
                &mut game.descriptors,
                chars,
                db,
                &buf1,
                Some(ch),
                toobj,
                Some(VictimRef::Char(tch2)),
                i,
            );
        } else {
            perform_act(
                &mut game.descriptors,
                chars,
                db,
                &buf2,
                Some(ch),
                toobj,
                Some(VictimRef::Char(tch2)),
                i,
            );
        }
    }
    let ch = chars.get(chid);
    if tch_id.is_some()
        && tch_id.unwrap() != chid
        && chars.get(tch_id.unwrap()).in_room() == ch.in_room()
    {
        buf1.push_str(
            format!(
                "$n stares at you and utters the words, '{}'.",
                if ch.get_class() == chars.get(tch_id.unwrap()).get_class() {
                    skill_name(db, spellnum)
                } else {
                    &buf
                }
            )
            .as_str(),
        );
        #[allow(clippy::unnecessary_unwrap)]
        let tch2_id = tch_id.unwrap();
        let tch2 = chars.get(tch2_id);
        act(
            &mut game.descriptors,
            chars,
            db,
            &buf1,
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(tch2)),
            TO_VICT,
        );
    }
}

/*
 * This function should be used anytime you are not 100% sure that you have
 * a valid spell/skill number.  A typical for() loop would not need to use
 * this because you can guarantee > 0 and <= TOP_SPELL_DEFINE.
 */
pub fn skill_name(db: &DB, num: i32) -> &'static str {
    if num > 0 && num <= TOP_SPELL_DEFINE as i32 {
        db.spell_info[num as usize].name
    } else if num == -1 {
        "UNUSED"
    } else {
        "UNDEFINED"
    }
}

pub fn find_skill_num(db: &DB, name: &str) -> Option<i32> {
    let mut ok;
    for skindex in 1..(TOP_SPELL_DEFINE + 1) {
        if is_abbrev(name, db.spell_info[skindex].name) {
            return Some(skindex as i32);
        }

        ok = true;
        let tempbuf = db.spell_info[skindex].name;
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
#[allow(clippy::too_many_arguments)]
pub fn call_magic(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    caster_id: DepotId,
    cvict_id: Option<DepotId>,
    ovict: Option<DepotId>,
    spellnum: i32,
    level: u8,
    casttype: i32,
) -> i32 {
    let caster = chars.get(caster_id);
    if spellnum < 1 || spellnum > TOP_SPELL_DEFINE as i32 {
        return 0;
    }
    let sinfo_routines;
    let sinfo_violent;
    {
        let sinfo = &db.spell_info[spellnum as usize];
        sinfo_routines = sinfo.routines;
        sinfo_violent = sinfo.violent;
    }
    if db.room_flagged(chars.get(caster_id).in_room(), RoomFlags::NOMAGIC) {
        send_to_char(
            &mut game.descriptors,
            chars.get(caster_id),
            "Your magic fizzles out and dies.\r\n",
        );
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n's magic fizzles out and dies.",
            false,
            Some(caster),
            None,
            None,
            TO_ROOM,
        );
        return 0;
    }
    if db.room_flagged(chars.get(caster_id).in_room(), RoomFlags::PEACEFUL)
        && (sinfo_violent || is_set!(sinfo_routines, MAG_DAMAGE))
    {
        send_to_char(
            &mut game.descriptors,
            chars.get(caster_id),
            "A flash of white light fills the room, dispelling your violent magic!\r\n",
        );
        act(
            &mut game.descriptors,
            chars,
            db,
            "White light from no particular source suddenly fills the room, then vanishes.",
            false,
            Some(caster),
            None,
            None,
            TO_ROOM,
        );
        return 0;
    }
    let savetype =
    /* determine the type of saving throw */
    match casttype {
        CAST_STAFF | CAST_SCROLL | CAST_POTION | CAST_WAND => {
           SAVING_ROD
        }
        CAST_SPELL => {
           SAVING_SPELL
        }
        _ => {
           SAVING_BREATH
        }
    };

    if is_set!(sinfo_routines, MAG_DAMAGE)
        && mag_damage(
            game,
            chars,
            db,
            texts,
            objs,
            level,
            caster_id,
            cvict_id.unwrap(),
            spellnum,
            savetype,
        ) == -1
    {
        return -1; /* Successful and target died, don't cast again. */
    }
    if is_set!(sinfo_routines, MAG_AFFECTS) {
        mag_affects(
            game, chars, db, objs, level, caster_id, cvict_id, spellnum, savetype,
        );
    }

    if is_set!(sinfo_routines, MAG_UNAFFECTS) {
        mag_unaffects(
            game,
            chars,
            db,
            objs,
            level,
            caster_id,
            cvict_id.unwrap(),
            spellnum,
            savetype,
        );
    }

    if is_set!(sinfo_routines, MAG_POINTS) {
        mag_points(game, chars, level, caster_id, cvict_id, spellnum, savetype);
    }

    if is_set!(sinfo_routines, MAG_ALTER_OBJS) {
        mag_alter_objs(
            game, chars, db, objs, level, caster_id, ovict, spellnum, savetype,
        );
    }

    if is_set!(sinfo_routines, MAG_GROUPS) {
        mag_groups(
            game,
            chars,
            db,
            texts,
            objs,
            level,
            Some(caster_id),
            spellnum,
            savetype,
        );
    }
    if is_set!(sinfo_routines, MAG_MASSES) {
        mag_masses(chars, db, level, caster_id, spellnum, savetype);
    }

    if is_set!(sinfo_routines, MAG_AREAS) {
        mag_areas(
            game,
            chars,
            db,
            texts,
            objs,
            level,
            Some(caster_id),
            spellnum,
            savetype,
        );
    }

    if is_set!(sinfo_routines, MAG_SUMMONS) {
        mag_summons(
            game,
            chars,
            db,
            objs,
            level,
            Some(caster_id),
            ovict,
            spellnum,
            savetype,
        );
    }

    if is_set!(sinfo_routines, MAG_CREATIONS) {
        mag_creations(game, chars, db, objs, level, Some(caster_id), spellnum);
    }

    if is_set!(sinfo_routines, MAG_MANUAL) {
        match spellnum {
            SPELL_CHARM => spell_charm(
                game,
                chars,
                db,
                objs,
                level,
                Some(caster_id),
                cvict_id,
                ovict,
            ),
            SPELL_CREATE_WATER => spell_create_water(
                game,
                chars,
                db,
                objs,
                level,
                Some(caster_id),
                cvict_id,
                ovict,
            ),
            SPELL_DETECT_POISON => spell_detect_poison(
                game,
                chars,
                db,
                objs,
                level,
                Some(caster_id),
                cvict_id,
                ovict,
            ),
            SPELL_ENCHANT_WEAPON => spell_enchant_weapon(
                game,
                chars,
                db,
                objs,
                level,
                Some(caster_id),
                cvict_id,
                ovict,
            ),
            SPELL_IDENTIFY => spell_identify(
                game,
                chars,
                db,
                objs,
                level,
                Some(caster_id),
                cvict_id,
                ovict,
            ),
            SPELL_LOCATE_OBJECT => spell_locate_object(
                game,
                chars,
                db,
                objs,
                level,
                Some(caster_id),
                cvict_id,
                ovict,
            ),
            SPELL_SUMMON => spell_summon(
                game,
                chars,
                db,
                texts,
                objs,
                level,
                Some(caster_id),
                cvict_id,
                ovict,
            ),
            SPELL_WORD_OF_RECALL => {
                spell_recall(
                    game,
                    chars,
                    db,
                    texts,
                    objs,
                    level,
                    Some(caster_id),
                    cvict_id,
                    ovict,
                );
            }
            SPELL_TELEPORT => {
                spell_teleport(
                    game,
                    chars,
                    db,
                    texts,
                    objs,
                    level,
                    Some(caster_id),
                    cvict_id,
                    ovict,
                );
            }
            _ => {}
        }
    }

    1
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
#[allow(clippy::too_many_arguments)]
pub fn mag_objectmagic(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
    argument: &str,
) {
    let ch = chars.get(chid);
    let obj = objs.get(oid);
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let mut tch = None;
    let mut tobj = None;
    let k = generic_find(
        &game.descriptors,
        chars,
        db,
        objs,
        &arg,
        FindFlags::CHAR_ROOM | FindFlags::OBJ_INV | FindFlags::OBJ_ROOM | FindFlags::OBJ_EQUIP,
        ch,
        &mut tch,
        &mut tobj,
    );

    match obj.get_obj_type() {
        ItemType::Staff => {
            act(
                &mut game.descriptors,
                chars,
                db,
                "You tap $p three times on the ground.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
            if !texts.get(obj.action_description).text.is_empty() {
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    &texts.get(obj.action_description).text,
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            } else {
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "$n taps $p three times on the ground.",
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            }

            if obj.get_obj_val(2) <= 0 {
                send_to_char(&mut game.descriptors, ch, "It seems powerless.\r\n");
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "Nothing seems to happen.",
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            } else {
                let oid = obj.id();
                objs.get_mut(oid).decr_obj_val(2);
                let ch = chars.get_mut(chid);
                ch.set_wait_state(PULSE_VIOLENCE as i32);
                /* Level to cast spell at. */
                let obj = objs.get(oid);
                let k = if obj.get_obj_val(0) != 0 {
                    obj.get_obj_val(0) as u8
                } else {
                    DEFAULT_STAFF_LVL
                };

                /*
                 * Problem : Area/mass spells on staves can cause crashes.
                 * Solution: Remove the special nature of area/mass spells on staves.
                 * Problem : People like that behavior.
                 * Solution: We special case the area/mass spells here.
                 */
                let ch = chars.get(chid);
                if has_spell_routine(db, obj.get_obj_val(3), MAG_MASSES | MAG_AREAS) {
                    let mut i = db.world[ch.in_room() as usize].peoples.len();
                    while i > 0 {
                        i -= 1;
                        let obj = objs.get(oid);
                        let spellnum = obj.get_obj_val(i);
                        call_magic(
                            game, chars, db, texts, objs, chid, None, None, spellnum, k, CAST_STAFF,
                        );
                    }
                } else {
                    for tch_id in db.world[ch.in_room() as usize].peoples.clone() {
                        if chid != tch_id {
                            let obj = objs.get(oid);
                            let spellnum = obj.get_obj_val(3);
                            call_magic(
                                game,
                                chars,
                                db,
                                texts,
                                objs,
                                chid,
                                Some(tch_id),
                                None,
                                spellnum,
                                k,
                                CAST_STAFF,
                            );
                        }
                    }
                }
            }
        }
        ItemType::Wand => {
            if k == FindFlags::CHAR_ROOM {
                if tch.unwrap().id() == chid {
                    act(
                        &mut game.descriptors,
                        chars,
                        db,
                        "You point $p at yourself.",
                        false,
                        Some(ch),
                        Some(obj),
                        None,
                        TO_CHAR,
                    );
                    act(
                        &mut game.descriptors,
                        chars,
                        db,
                        "$n points $p at $mself.",
                        false,
                        Some(ch),
                        Some(obj),
                        None,
                        TO_ROOM,
                    );
                } else {
                    let tch = tch.unwrap();
                    act(
                        &mut game.descriptors,
                        chars,
                        db,
                        "You point $p at $N.",
                        false,
                        Some(ch),
                        Some(obj),
                        Some(VictimRef::Char(tch)),
                        TO_CHAR,
                    );
                    if !texts.get(obj.action_description).text.is_empty() {
                        act(
                            &mut game.descriptors,
                            chars,
                            db,
                            &texts.get(obj.action_description).text,
                            false,
                            Some(ch),
                            Some(obj),
                            Some(VictimRef::Char(tch)),
                            TO_ROOM,
                        );
                    } else {
                        act(
                            &mut game.descriptors,
                            chars,
                            db,
                            "$n points $p at $N.",
                            true,
                            Some(ch),
                            Some(obj),
                            Some(VictimRef::Char(tch)),
                            TO_ROOM,
                        );
                    }
                }
            } else if tobj.is_some() {
                let tobj = tobj.unwrap();
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "You point $p at $P.",
                    false,
                    Some(ch),
                    Some(obj),
                    Some(VictimRef::Obj(tobj)),
                    TO_CHAR,
                );
                if !texts.get(obj.action_description).text.is_empty() {
                    act(
                        &mut game.descriptors,
                        chars,
                        db,
                        &texts.get(obj.action_description).text,
                        false,
                        Some(ch),
                        Some(obj),
                        Some(VictimRef::Obj(tobj)),
                        TO_ROOM,
                    );
                } else {
                    act(
                        &mut game.descriptors,
                        chars,
                        db,
                        "$n points $p at $P.",
                        true,
                        Some(ch),
                        Some(obj),
                        Some(VictimRef::Obj(tobj)),
                        TO_ROOM,
                    );
                }
            } else if is_set!(
                db.spell_info[obj.get_obj_val(3) as usize].routines,
                MAG_AREAS | MAG_MASSES
            ) {
                /* Wands with area spells don't need to be pointed. */
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "You point $p outward.",
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_CHAR,
                );
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "$n points $p outward.",
                    true,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            } else {
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "At what should $p be pointed?",
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_CHAR,
                );
                return;
            }

            if obj.get_obj_val(2) <= 0 {
                send_to_char(&mut game.descriptors, ch, "It seems powerless.\r\n");
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "Nothing seems to happen.",
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
                return;
            }
            let tch_id = tch.map(|c| c.id());
            let tobj_id = tobj.map(|o| o.id());
            let oid = obj.id();
            objs.get_mut(oid).decr_obj_val(2);
            let ch = chars.get_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32);
            let obj = objs.get(oid);
            if obj.get_obj_val(0) != 0 {
                call_magic(
                    game,
                    chars,
                    db,
                    texts,
                    objs,
                    chid,
                    tch_id,
                    tobj_id,
                    obj.get_obj_val(3),
                    obj.get_obj_val(0) as u8,
                    CAST_WAND,
                );
            } else {
                call_magic(
                    game,
                    chars,
                    db,
                    texts,
                    objs,
                    chid,
                    tch_id,
                    tobj_id,
                    obj.get_obj_val(3),
                    DEFAULT_WAND_LVL,
                    CAST_WAND,
                );
            }
        }
        ItemType::Scroll => {
            if !arg.is_empty() {
                if k.is_empty() {
                    act(
                        &mut game.descriptors,
                        chars,
                        db,
                        "There is nothing to here to affect with $p.",
                        false,
                        Some(ch),
                        Some(obj),
                        None,
                        TO_CHAR,
                    );
                    return;
                }
            } else {
                tch = Some(ch);
            }

            act(
                &mut game.descriptors,
                chars,
                db,
                "You recite $p which dissolves.",
                true,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
            if !texts.get(obj.action_description).text.is_empty() {
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    &texts.get(obj.action_description).text,
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            } else {
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "$n recites $p.",
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            }
            let tch_id = tch.map(|c| c.id());
            let tobj_id = tobj.map(|o| o.id());
            let ch = chars.get_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32);
            for i in 1..3 {
                let obj = objs.get(oid);
                let spellnum = obj.get_obj_val(i);
                let level = obj.get_obj_val(0) as u8;
                if call_magic(
                    game,
                    chars,
                    db,
                    texts,
                    objs,
                    chid,
                    tch_id,
                    tobj_id,
                    spellnum,
                    level,
                    CAST_SCROLL,
                ) <= 0
                {
                    break;
                }
            }

            db.extract_obj(chars, objs, oid);
        }
        ItemType::Potion => {
            act(
                &mut game.descriptors,
                chars,
                db,
                "You quaff $p.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
            if !texts.get(obj.action_description).text.is_empty() {
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    &texts.get(obj.action_description).text,
                    false,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            } else {
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "$n quaffs $p.",
                    true,
                    Some(ch),
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            }
            let ch = chars.get_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32);
            for i in 1..3 {
                let obj = objs.get(oid);
                let spellnum = obj.get_obj_val(i);
                let level = obj.get_obj_val(0) as u8;
                if call_magic(
                    game,
                    chars,
                    db,
                    texts,
                    objs,
                    chid,
                    Some(chid),
                    None,
                    spellnum,
                    level,
                    CAST_POTION,
                ) <= 0
                {
                    break;
                }
            }

            db.extract_obj(chars, objs, oid);
        }
        _ => {
            error!(
                "SYSERR: Unknown object_type {} in mag_objectmagic.",
                obj.get_obj_type() as i32
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
#[allow(clippy::too_many_arguments)]
pub fn cast_spell(
    game: &mut Game,
    chars: &mut Depot<CharData>,
    db: &mut DB,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    tch_id: Option<DepotId>,
    tobj_id: Option<DepotId>,
    spellnum: i32,
) -> i32 {
    let ch = chars.get(chid);
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
            Position::Sleeping => {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "You dream about great magical powers.\r\n",
                );
            }
            Position::Resting => {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "You cannot concentrate while resting.\r\n",
                );
            }
            Position::Sitting => {
                send_to_char(&mut game.descriptors, ch, "You can't do this sitting!\r\n");
            }
            Position::Fighting => {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "Impossible!  You can't concentrate enough!\r\n",
                );
            }
            _ => {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "You can't do much of anything like this!\r\n",
                );
            }
        }
        return 0;
    }
    if ch.aff_flagged(AffectFlags::CHARM)
        && tch_id.is_some()
        && ch.master.unwrap() == tch_id.unwrap()
    {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are afraid you might hurt your master!\r\n",
        );
        return 0;
    }
    if (tch_id.is_none() || chid != tch_id.unwrap()) && is_set!(sinfo.targets, TAR_SELF_ONLY) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can only cast this spell upon yourself!\r\n",
        );
        return 0;
    }
    if tch_id.is_some() && chid == tch_id.unwrap() && is_set!(sinfo.targets, TAR_NOT_SELF) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You cannot cast this spell upon yourself!\r\n",
        );
        return 0;
    }
    if is_set!(sinfo.routines, MAG_GROUPS) && !ch.aff_flagged(AffectFlags::GROUP) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can't cast this spell if you're not in a group!\r\n",
        );
        return 0;
    }
    send_to_char(&mut game.descriptors, ch, OK);
    say_spell(game, chars, db, objs, chid, spellnum, tch_id, tobj_id);
    let ch = chars.get(chid);
    call_magic(
        game,
        chars,
        db,
        texts,
        objs,
        chid,
        tch_id,
        tobj_id,
        spellnum,
        ch.get_level(),
        CAST_SPELL,
    )
}

/*
 * do_cast is the entry point for PC-casted spells.  It parses the arguments,
 * determines the spell number and finds a target, throws the die to see if
 * the spell can be cast, checks for sufficient mana and subtracts it, and
 * passes control to cast_spell().
 */
#[allow(clippy::too_many_arguments)]
pub fn do_cast(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() {
        return;
    }

    /* get: blank, spell name, target name */
    let mut i = argument.splitn(3, '\'');

    if i.next().is_none() {
        send_to_char(&mut game.descriptors, ch, "Cast what where?\r\n");
        return;
    }
    let s = i.next();
    if s.is_none() {
        send_to_char(
            &mut game.descriptors,
            ch,
            "Spell names must be enclosed in the Holy Magic Symbols: '\r\n",
        );
        return;
    }
    let s = s.unwrap();
    let mut t = i.next();
    /* spellnum = search_block(s, spells, 0); */
    let spellnum = find_skill_num(db, s);

    if spellnum.is_none() || spellnum.unwrap() > MAX_SPELLS {
        send_to_char(&mut game.descriptors, ch, "Cast what?!?\r\n");
        return;
    }
    let spellnum = spellnum.unwrap();
    let sinfo = db.spell_info[spellnum as usize];
    if ch.get_level() < sinfo.min_level[ch.get_class() as usize] as u8 {
        send_to_char(&mut game.descriptors, ch, "You do not know that spell!\r\n");
        return;
    }
    if ch.get_skill(spellnum) == 0 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are unfamiliar with that spell.\r\n",
        );
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
    let mut tch = None;
    let mut tobj = None;
    if is_set!(sinfo.targets, TAR_IGNORE) {
        target = true;
    } else if !t.is_empty() {
        if !target && is_set!(sinfo.targets, TAR_CHAR_ROOM) && {
            tch = get_char_vis(
                &game.descriptors,
                chars,
                db,
                ch,
                &mut t,
                None,
                FindFlags::CHAR_ROOM,
            );
            tch.is_some()
        } {
            target = true;
        }
        if !target && is_set!(sinfo.targets, TAR_CHAR_WORLD) && {
            tch = get_char_vis(
                &game.descriptors,
                chars,
                db,
                ch,
                &mut t,
                None,
                FindFlags::CHAR_WORLD,
            );
            tch.is_some()
        } {
            target = true;
        }

        if !target && is_set!(sinfo.targets, TAR_OBJ_INV) && {
            tobj = get_obj_in_list_vis(
                &game.descriptors,
                chars,
                db,
                objs,
                ch,
                &t,
                None,
                &ch.carrying,
            );
            tobj.is_some()
        } {
            target = true;
        }

        if !target && is_set!(sinfo.targets, TAR_OBJ_EQUIP) {
            for i in 0..NUM_WEARS {
                if ch.get_eq(i).is_some()
                    && isname(&t, objs.get(ch.get_eq(i).unwrap()).name.as_ref())
                {
                    tobj = Some(objs.get(ch.get_eq(i).unwrap()));
                    target = true;
                }
            }
        }
        if !target && is_set!(sinfo.targets, TAR_OBJ_ROOM) && {
            tobj = get_obj_in_list_vis2(
                &game.descriptors,
                chars,
                db,
                objs,
                ch,
                &t,
                None,
                &db.world[ch.in_room as usize].contents,
            );
            tobj.is_some()
        } {
            target = true;
        }
        if !target && is_set!(sinfo.targets, TAR_OBJ_WORLD) && {
            tobj = get_obj_vis(&game.descriptors, chars, db, objs, ch, &t, None);
            tobj.is_some()
        } {
            target = true;
        }
    } else {
        /* if target string is empty */
        if !target && is_set!(sinfo.targets, TAR_FIGHT_SELF) && ch.fighting_id().is_some() {
            tch = Some(ch);
            target = true;
        }
        if !target && is_set!(sinfo.targets, TAR_FIGHT_VICT) && ch.fighting_id().is_some() {
            tch = Some(chars.get(ch.fighting_id().unwrap()));
            target = true;
        }
        /* if no target specified, and the spell isn't violent, default to self */
        if !target && is_set!(sinfo.targets, TAR_CHAR_ROOM) && !sinfo.violent {
            tch = Some(ch);
            target = true;
        }
        if !target {
            send_to_char(
                &mut game.descriptors,
                ch,
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

    let tch_id = tch.map(|c| c.id());
    if target && tch.unwrap().id() == chid && sinfo.violent {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You shouldn't cast that on yourself -- could be bad for your health!\r\n",
        );
        return;
    }
    if !target {
        send_to_char(
            &mut game.descriptors,
            ch,
            "Cannot find the target of your spell!\r\n",
        );
        return;
    }
    let mana = mag_manacost(ch, &sinfo);
    if mana > 0 && ch.get_mana() < mana && ch.get_level() < LVL_IMMORT {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You haven't the energy to cast that spell!\r\n",
        );
        return;
    }

    /* You throws the dice and you takes your chances.. 101% is total failure */
    if rand_number(0, 101) > ch.get_skill(spellnum) as u32 {
        let ch = chars.get_mut(chid);
        ch.set_wait_state(PULSE_VIOLENCE as i32);
        let ch = chars.get(chid);
        let tch = tch_id.map(|i| chars.get(i));
        if tch.is_none()
            || skill_message(
                &mut game.descriptors,
                chars,
                db,
                objs,
                0,
                ch,
                tch.unwrap(),
                spellnum,
            ) == 0
        {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You lost your concentration!\r\n",
            );
        }
        if mana > 0 {
            let ch = chars.get_mut(chid);
            ch.set_mana(max(0, min(ch.get_mana(), ch.get_mana() - (mana / 2))));
        }
        let tch = tch_id.map(|i| chars.get(i));
        if sinfo.violent && tch.is_some() && tch.unwrap().is_npc() {
            game.hit(
                chars,
                db,
                texts,
                objs,
                tch_id.unwrap(),
                chid,
                TYPE_UNDEFINED,
            );
        }
    } else {
        /* cast spell returns 1 on success; subtract mana & set waitstate */
        let tch_id = tch.map(|c| c.id());
        let tobj_id = tobj.map(|o| o.id());
        if cast_spell(
            game, chars, db, texts, objs, chid, tch_id, tobj_id, spellnum,
        ) != 0
        {
            let ch = chars.get_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32);
            if mana > 0 {
                ch.set_mana(max(0, min(ch.get_mana(), ch.get_mana() - mana)));
            }
        }
    }
}

pub fn spell_level(db: &mut DB, spell: i32, chclass: Class, level: i32) {
    let mut bad = false;

    if spell < 0 || spell > TOP_SPELL_DEFINE as i32 {
        error!(
            "SYSERR: attempting assign to illegal spellnum {}/{}",
            spell, TOP_SPELL_DEFINE
        );
        return;
    }

    if chclass == Class::Undefined {
        error!(
            "SYSERR: assigning '{}' to illegal class {}/{}.",
            skill_name(db, spell),
            chclass as i8,
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
#[allow(clippy::too_many_arguments)]
fn spello(
    db: &mut DB,
    spl: i32,
    name: &'static str,
    max_mana: i32,
    min_mana: i32,
    mana_change: i32,
    minpos: Position,
    targets: i32,
    violent: bool,
    routines: i32,
    wearoff: &'static str,
) {
    let spl = spl as usize;
    for i in 0..NUM_CLASSES {
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

// fn unused_spell(chars: &mut Depot<CharData>, db: &mut DB, spl: usize) {
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
    spello(db, skill, name, 0, 0, 0, Position::Dead, 0, false, 0, "");
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Fighting,
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
        Position::Fighting,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Fighting,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Standing,
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
        Position::Fighting,
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
        Position::Dead,
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
        Position::Sitting,
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
        Position::Sitting,
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
        Position::Sitting,
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
        Position::Sitting,
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
        Position::Sitting,
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
