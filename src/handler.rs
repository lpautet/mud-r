/* ************************************************************************
*   File: handler.rs                                    Part of CircleMUD *
*  Usage: internal funcs: moving and finding chars/objs                   *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use std::borrow::Borrow;
use std::cmp::{max, min};
use std::process;
use std::rc::Rc;

use log::error;

use crate::act_wizard::do_return;
use crate::class::invalid_class;
use crate::config::MENU;
use crate::db::DB;
use crate::depot::{Depot, DepotId, HasId};
use crate::interpreter::one_argument;
use crate::objsave::crash_delete_crashfile;
use crate::spells::{SAVING_BREATH, SAVING_PARA, SAVING_PETRI, SAVING_ROD, SAVING_SPELL};
use crate::structs::ConState::{ConClose, ConMenu};
use crate::structs::{
    AffectFlags, AffectedType, CharData, ExtraDescrData, ExtraFlags, MobRnum, ObjData, ObjRnum, RoomFlags, RoomRnum, WearFlags, APPLY_AC, APPLY_AGE, APPLY_CHA, APPLY_CHAR_HEIGHT, APPLY_CHAR_WEIGHT, APPLY_CLASS, APPLY_CON, APPLY_DAMROLL, APPLY_DEX, APPLY_EXP, APPLY_GOLD, APPLY_HIT, APPLY_HITROLL, APPLY_INT, APPLY_LEVEL, APPLY_MANA, APPLY_MOVE, APPLY_NONE, APPLY_SAVING_BREATH, APPLY_SAVING_PARA, APPLY_SAVING_PETRI, APPLY_SAVING_ROD, APPLY_SAVING_SPELL, APPLY_STR, APPLY_WIS, ITEM_ARMOR, ITEM_LIGHT, ITEM_MONEY, LVL_GRGOD, MAX_OBJ_AFFECT, MOB_NOTDEADYET, NOTHING, NOWHERE, NUM_WEARS, PLR_CRASH, PLR_NOTDEADYET, WEAR_BODY, WEAR_HEAD, WEAR_LEGS, WEAR_LIGHT
};
use crate::util::{can_see, can_see_obj, die_follower, rand_number, SECS_PER_MUD_YEAR};
use crate::{act, is_set, save_char, send_to_char, DescriptorData, Game, TextData, TO_CHAR, TO_ROOM};

pub const FIND_CHAR_ROOM: i32 = 1 << 0;
pub const FIND_CHAR_WORLD: i32 = 1 << 1;
pub const FIND_OBJ_INV: i32 = 1 << 2;
pub const FIND_OBJ_ROOM: i32 = 1 << 3;
pub const FIND_OBJ_WORLD: i32 = 1 << 4;
pub const FIND_OBJ_EQUIP: i32 = 1 << 5;

pub fn fname(namelist: &str) -> Rc<str> {
    let mut holder = String::new();
    for c in namelist.chars() {
        if !char::is_alphanumeric(c) {
            break;
        }
        holder.push(c);
    }
    return Rc::from(holder.as_str());
}

pub fn isname(txt: &str, namelist: &str) -> bool {
    //info!("[DEBUG] {} namelist='{}'", txt, namelist);
    let mut curname = namelist.to_string();
    loop {
        let mut skip = false;
        let mut p = '\0';
        for c in txt.chars() {
            if curname.is_empty() {
                return false;
            }

            p = curname.remove(0);
            if p == ' ' {
                skip = true;
                break;
            }
            if p.to_ascii_lowercase() != c.to_ascii_lowercase() {
                skip = true;
                break;
            }
        }
        if !skip {
            if curname.is_empty() {
                return true;
            }
            p = curname.remove(0);
            if !p.is_alphanumeric() {
                return true;
            }
        }

        while curname.len() > 0 && p.is_alphanumeric() {
            p = curname.remove(0);
        }
    }
}

fn affect_modify(ch: &mut CharData, loc: i8, _mod: i16, bitv: AffectFlags, add: bool) {
    let mut _mod = _mod;
    if add {
        ch.set_aff_flags(bitv);
    } else {
        ch.remove_aff_flags(bitv);
        _mod = -_mod;
    }

    match loc {
        APPLY_NONE => {}
        APPLY_STR => {
            ch.incr_str(_mod as i8);
        }
        APPLY_DEX => {
            ch.incr_dex(_mod as i8);
        }
        APPLY_INT => {
            ch.incr_int(_mod as i8);
        }
        APPLY_WIS => {
            ch.incr_wis(_mod as i8);
        }
        APPLY_CON => {
            ch.incr_con(_mod as i8);
        }
        APPLY_CHA => {
            ch.incr_cha(_mod as i8);
        }

        APPLY_CLASS => { /* ??? GET_CLASS(ch) += mod; */ }

        /*
         * My personal thoughts on these two would be to set the person to the
         * value of the apply.  That way you won't have to worry about people
         * making +1 level things to be imp (you restrict anything that gives
         * immortal level of course).  It also makes more sense to set someone
         * to a class rather than adding to the class number. -gg
         */
        APPLY_LEVEL => { /* ??? GET_LEVEL(ch) += mod; */ }

        APPLY_AGE => {
            ch.player.time.birth -= _mod as u64 * SECS_PER_MUD_YEAR;
        }

        APPLY_CHAR_WEIGHT => {
            ch.set_weight(ch.get_weight() + _mod as u8);
        }

        APPLY_CHAR_HEIGHT => {
            ch.set_height(ch.get_height() + _mod as u8);
        }

        APPLY_MANA => {
            ch.incr_max_mana(_mod as i16);
        }

        APPLY_HIT => {
            ch.incr_max_hit(_mod as i16);
        }

        APPLY_MOVE => {
            ch.incr_max_move(_mod as i16);
        }

        APPLY_GOLD => {}

        APPLY_EXP => {}

        APPLY_AC => {
            ch.set_ac(ch.get_ac() + _mod as i16);
        }

        APPLY_HITROLL => {
            ch.set_hitroll(ch.get_hitroll() + _mod as i8);
        }

        APPLY_DAMROLL => {
            ch.set_damroll(ch.get_damroll() + _mod as i8);
        }

        APPLY_SAVING_PARA => {
            ch.set_save(SAVING_PARA as usize, ch.get_save(SAVING_PARA) + _mod as i16);
        }
        APPLY_SAVING_ROD => {
            ch.set_save(SAVING_ROD as usize, ch.get_save(SAVING_ROD) + _mod as i16);
        }
        APPLY_SAVING_PETRI => {
            ch.set_save(
                SAVING_PETRI as usize,
                ch.get_save(SAVING_PETRI) + _mod as i16,
            );
        }

        APPLY_SAVING_BREATH => {
            ch.set_save(
                SAVING_BREATH as usize,
                ch.get_save(SAVING_BREATH) + _mod as i16,
            );
        }

        APPLY_SAVING_SPELL => {
            ch.set_save(SAVING_SPELL as usize, ch.get_save(SAVING_SPELL) + _mod);
        }

        _ => {
            error!(
                "SYSERR: Unknown apply adjust {} attempt (affect_modify).",
                loc
            );
        }
    } /* switch */
}

/* This updates a character by subtracting everything he is affected by */
/* restoring original abilities, and then affecting all again           */
pub fn affect_total(objs: &Depot<ObjData>, ch: &mut CharData) {
    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some() {
            for j in 0..MAX_OBJ_AFFECT {
                let eq = objs.get(ch.get_eq(i).unwrap());
                let loc = eq.affected[j as usize].location as i8;
                let mod_ = eq.affected[j as usize].modifier as i16;
                let bitv = eq.get_obj_affect();
                affect_modify(ch, loc, mod_, bitv, false);
            }
        }
    }
    for af in ch.affected.clone() {
        affect_modify(
            ch,
            af.location as i8,
            af.modifier as i16,
            af.bitvector,
            false,
        );
    }

    ch.aff_abils = ch.real_abils;

    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some() {
            for j in 0..MAX_OBJ_AFFECT {
                let eq = objs.get(ch.get_eq(i).unwrap());
                let loc = eq.affected[j as usize].location as i8;
                let mod_ = eq.affected[j as usize].modifier as i16;
                let bitv = eq.get_obj_affect();
                affect_modify(ch, loc, mod_, bitv, true)
            }
        }
    }
    for af in ch.affected.clone() {
        affect_modify(
            ch,
            af.location as i8,
            af.modifier as i16,
            af.bitvector,
            true,
        );
    }

    /* Make certain values are between 0..25, not < 0 and not > 25! */

    let i = if ch.is_npc() || ch.get_level() >= LVL_GRGOD as u8 {
        25
    } else {
        18
    };

    ch.set_dex(max(0, min(ch.get_dex(), i)));
    ch.set_int(max(0, min(ch.get_int(), i)));
    ch.set_wis(max(0, min(ch.get_wis(), i)));
    ch.set_con(max(0, min(ch.get_con(), i)));
    ch.set_cha(max(0, min(ch.get_cha(), i)));
    ch.set_str(max(0, ch.get_str()));

    if ch.is_npc() {
        ch.set_str(min(ch.get_str(), i));
    } else {
        if ch.get_str() > 18 {
            let i = ch.get_add() as i16 + ((ch.get_str() as i16 - 18) * 10);
            ch.set_add(min(i, 100) as i8);
            ch.set_str(18);
        }
    }
}

/* Insert an affect_type in a char_data structure
Automatically sets apropriate bits and apply's */
pub fn affect_to_char(objs: &Depot<ObjData>, ch: &mut CharData, af: AffectedType) {
    ch.affected.push(af);

    affect_modify(
        ch,
        af.location as i8,
        af.modifier as i16,
        af.bitvector,
        true,
    );
    affect_total(objs, ch);
}

/*
 * Remove an affected_type structure from a char (called when duration
 * reaches zero). Pointer *af must never be NIL!  Frees mem and calls
 * affect_location_apply
 */
pub fn affect_remove(objs: &Depot<ObjData>, ch: &mut CharData, af: AffectedType) {
    affect_modify(
        ch,
        af.location as i8,
        af.modifier as i16,
        af.bitvector,
        false,
    );
    affect_total(objs, ch);
}

/* Call affect_remove with every spell of spelltype "skill" */
pub fn affect_from_char(objs: &mut Depot<ObjData>, ch: &mut CharData, type_: i16) {
    let mut list = ch.affected.clone();
    list.retain(|hjp| {
        if hjp._type == type_ {
            affect_remove(objs, ch, *hjp);
            false
        } else {
            true
        }
    });
    ch.affected = list;
}

/*
 * Return TRUE if a char is affected by a spell (SPELL_XXX),
 * FALSE indicates not affected.
 */
pub fn affected_by_spell(ch: &CharData, type_: i16) -> bool {
    for hjp in ch.affected.iter() {
        if hjp._type == type_ {
            return true;
        }
    }

    false
}

pub fn affect_join(
    objs: &Depot<ObjData>,
    ch: &mut CharData,
    af: &AffectedType,
    add_dur: bool,
    avg_dur: bool,
    add_mod: bool,
    avg_mod: bool,
) {
    let mut af = *af;
    let mut list = ch.affected.clone();
    list.retain_mut(|hjp| {
        if (hjp._type == af._type) && (hjp.location == af.location) {
            if add_dur {
                af.duration += hjp.duration;
            }
            if avg_dur {
                af.duration /= 2;
            }

            if add_mod {
                af.modifier += hjp.modifier;
            }
            if avg_mod {
                af.modifier /= 2;
            }

            affect_remove(objs, ch, *hjp);
            false
        } else {
            true
        }
    });
    ch.affected = list;
    affect_to_char(objs, ch, af);
}
impl DB {
    /* move a player out of a room */
    pub fn char_from_room(&mut self, objs: &Depot<ObjData>, ch: &mut CharData) {
        if ch.in_room() == NOWHERE {
            error!("SYSERR: NULL character or NOWHERE in char_from_room");
            process::exit(1);
        }

        if ch.fighting_id().is_some() {
            self.stop_fighting(ch);
        }
        if ch.get_eq(WEAR_LIGHT as i8).is_some() {
            let light = objs.get(ch.get_eq(WEAR_LIGHT as i8).unwrap());
            if light.get_obj_type() == ITEM_LIGHT {
                if light.get_obj_val(2) != 0 {
                    let in_room = ch.in_room();
                    self.world[in_room as usize].light -= 1;
                }
            }
        }
        let in_room = ch.in_room();
        let list = &mut self.world[in_room as usize].peoples;
        list.retain(|c_rch| *c_rch != ch.id());
    }

    /* place a character in a room */
    pub(crate) fn char_to_room(
        &mut self,
        chars: &mut Depot<CharData>,
        objs: &Depot<ObjData>,
        chid: DepotId,
        room: RoomRnum,
    ) {
        if room == NOWHERE || room >= self.world.len() as i16 {
            error!(
                "SYSERR: Illegal value(s) passed to char_to_room. (Room: {}/{} Ch: {}",
                room,
                self.world.len(),
                'x'
            );
            return;
        }
        self.world[room as usize].peoples.push(chid);
        let ch = chars.get_mut(chid);
        ch.set_in_room(room);
        let ch = chars.get(chid);

        if ch.get_eq(WEAR_LIGHT as i8).is_some() {
            let light = objs.get(ch.get_eq(WEAR_LIGHT as i8).unwrap());
            if light.get_obj_type() == ITEM_LIGHT {
                if light.get_obj_val(2) != 0 {
                    let in_room = ch.in_room();
                    self.world[in_room as usize].light += 1; /* Light ON */
                }
            }
        }

        /* Stop fighting now, if we left. */
        let ch = chars.get(chid);
        if ch.fighting_id().is_some()
            && ch.in_room() != chars.get(ch.fighting_id().unwrap()).in_room()
        {
            self.stop_fighting(chars.get_mut(ch.fighting_id().unwrap()));
            self.stop_fighting(chars.get_mut(chid));
        }
    }
}
/* give an object to a char   */
pub fn obj_to_char(obj: &mut ObjData, ch: &mut CharData) {
    ch.carrying.push(obj.id());
    obj.carried_by = Some(ch.id());
    obj.set_in_room(NOWHERE);

    ch.incr_is_carrying_w(obj.get_obj_weight());
    ch.incr_is_carrying_n();

    /* set flag for crash-save system, but not on mobs! */
    if !ch.is_npc() {
        ch.set_plr_flag_bit(PLR_CRASH)
    }
}

/* take an object from a char */
pub fn obj_from_char(chars: &mut Depot<CharData>, obj: &mut ObjData) {
    let obj_weight = obj.get_obj_weight();
    let carried_by_id = obj.carried_by.unwrap();
    let carried_by_ch = chars.get_mut(carried_by_id);
    carried_by_ch.carrying.retain(|x| *x != obj.id());

    /* set flag for crash-save system, but not on mobs! */
    if !carried_by_ch.is_npc() {
        carried_by_ch.set_plr_flag_bit(PLR_CRASH);
    }

    carried_by_ch.incr_is_carrying_w(-obj_weight);
    carried_by_ch.decr_is_carrying_n();
    obj.carried_by = None;
}

/* Return the effect of a piece of armor in position eq_pos */
fn apply_ac(objs: &Depot<ObjData>, ch: &CharData, eq_pos: i16) -> i32 {
    let eq_id = ch.get_eq(eq_pos as i8);
    if eq_id.is_none() {
        panic!(
            "apply_ac cannot find eq at pos {} for {}",
            eq_pos,
            ch.get_name()
        );
    }

    let eq_id = eq_id.unwrap();
    let eq = objs.get(eq_id);

    if eq.get_obj_type() != ITEM_ARMOR as u8 {
        return 0;
    }

    let factor;

    match eq_pos {
        WEAR_BODY => {
            factor = 3;
        } /* 30% */
        WEAR_HEAD => {
            factor = 2;
        } /* 20% */
        WEAR_LEGS => {
            factor = 2;
        } /* 20% */
        _ => {
            factor = 1;
        } /* all others 10% */
    }
    factor * eq.get_obj_val(0)
}

pub fn invalid_align(ch: &CharData, obj: &ObjData) -> bool {
    if obj.obj_flagged(ExtraFlags::ANTI_EVIL) && ch.is_evil() {
        return true;
    };
    if obj.obj_flagged(ExtraFlags::ANTI_GOOD) && ch.is_good() {
        return true;
    }
    if obj.obj_flagged(ExtraFlags::ANTI_NEUTRAL) && ch.is_neutral() {
        return true;
    }
    false
}

    pub(crate) fn equip_char(
        descs: &mut Depot<DescriptorData>,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        objs: &mut Depot<ObjData>,
        chid: DepotId,
        oid: DepotId,
        pos: i8,
    ) {
        let ch = chars.get_mut(chid);

        if pos < 0 || pos >= NUM_WEARS {
            panic!("Invalid position in equip_char: {}", pos);
        }

        let obj = objs.get_mut(oid);

        if ch.get_eq(pos).is_some() {
            error!(
                "SYSERR: Char is already equipped: {}, {}",
                ch.get_name(),
                obj.short_description
            );
            return;
        }
        if obj.carried_by.borrow().is_some() {
            error!("SYSERR: EQUIP: Obj is carried_by when equip.");
            return;
        }
        if obj.in_room() != NOWHERE {
            error!("SYSERR: EQUIP: Obj is in_room when equip.");
            return;
        }

        if invalid_align(ch, obj) || invalid_class(ch, obj) {
            let ch = chars.get(chid);
            act(descs, 
                chars,
                db,
                "You are zapped by $p and instantly let go of it.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
            act(descs, 
                chars,
                db,
                "$n is zapped by $p and instantly lets go of it.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_ROOM,
            );
            /* Changed to drop in inventory instead of the ground. */
            let ch = chars.get_mut(chid);
            obj_to_char(obj, ch);
            return;
        }

        ch.set_eq(pos, Some(oid));
        obj.worn_by = Some(chid);
        obj.worn_on = pos as i16;

        if obj.get_obj_type() == ITEM_ARMOR as u8 {
            let armor = apply_ac(objs, ch, pos as i16);
            ch.set_ac(ch.get_ac() - armor as i16);
        }
        let obj = objs.get_mut(oid);
        if ch.in_room() != NOWHERE {
            if pos == WEAR_LIGHT as i8 && obj.get_obj_type() == ITEM_LIGHT as u8 {
                if obj.get_obj_val(2) != 0 {
                    /* if light is ON */
                    db.world[ch.in_room() as usize].light += 1;
                }
            }
        } else {
            error!(
                "SYSERR: IN_ROOM(ch) = NOWHERE when equipping char {}.",
                ch.get_name()
            );
        }

        for j in 0..MAX_OBJ_AFFECT {
            let loc = obj.affected[j as usize].location as i8;
            let mod_ = obj.affected[j as usize].modifier as i16;
            let bitv = obj.get_obj_affect();
            affect_modify(ch, loc, mod_, bitv, true);
        }

        affect_total(objs, ch);
    }

impl DB {
    pub fn unequip_char(
        &mut self,
        chars: &mut Depot<CharData>,
        objs: &mut Depot<ObjData>,
        chid: DepotId,
        pos: i8,
    ) -> Option<DepotId> {
        let ch = chars.get_mut(chid);
        if pos < 0 || pos > NUM_WEARS || ch.get_eq(pos).is_none() {
            //core_dump();
            return None;
        }

        let oid = ch.get_eq(pos).unwrap();
        let obj = objs.get_mut(oid);
        obj.worn_by = None;
        obj.worn_on = -1;
        if obj.get_obj_type() == ITEM_ARMOR as u8 {
            let armor = apply_ac(objs, ch, pos as i16);
            ch.set_ac(ch.get_ac() + armor as i16);
        }
        let obj = objs.get_mut(oid);
        if ch.in_room() != NOWHERE {
            if pos == WEAR_LIGHT as i8 && obj.get_obj_type() == ITEM_LIGHT as u8 {
                if obj.get_obj_val(2) != 0 {
                    let ch_in_room = ch.in_room();
                    self.world[ch_in_room as usize].light -= 1;
                }
            }
        } else {
            error!(
                "SYSERR: IN_ROOM(ch) = NOWHERE when unequipping char {}.",
                ch.get_name()
            );
        }
        ch.set_eq(pos, None);

        for j in 0..MAX_OBJ_AFFECT {
            let loc = obj.affected[j as usize].location as i8;
            let mod_ = obj.affected[j as usize].modifier as i16;
            let bitv = obj.get_obj_affect();
            affect_modify(ch, loc, mod_, bitv, false);
        }

        affect_total(objs, ch);

        Some(oid)
    }
}

pub fn get_number(name: &mut String) -> i32 {
    let ppos = name.find('.');
    if ppos.is_none() {
        return 1;
    }
    let ppos = ppos.unwrap();
    let number = name.split_off(ppos);
    let r = number.parse::<i32>();
    if r.is_err() {
        return 0;
    }
    r.unwrap()
}

/* Search a given list for an object number, and return a ptr to that obj */
pub fn get_obj_in_list_num<'a>(
    objs: &'a Depot<ObjData>,
    num: i16,
    list: &Vec<DepotId>,
) -> Option<&'a ObjData> {
    for o in list {
        let obj = objs.get(*o);
        if obj.get_obj_rnum() == num {
            return Some(obj);
        }
    }
    None
}
impl DB {
    /* search the entire world for an object number, and return a pointer  */
    pub(crate) fn get_obj_num<'a>(
        &self,
        objs: &'a Depot<ObjData>,
        nr: ObjRnum,
    ) -> Option<&'a ObjData> {
        for &oid in self.object_list.iter() {
            let o = objs.get(oid);
            if o.get_obj_rnum() == nr {
                return Some(o);
            }
        }
        None
    }

    /* search a room for a char, and return a pointer if found..  */
    pub fn get_char_room<'a>(
        &self,
        chars: &'a Depot<CharData>,
        name: &str,
        number: Option<&mut i32>,
        room: RoomRnum,
    ) -> Option<&'a CharData> {
        let mut name = name.to_string();
        let mut number = number;

        let mut num;

        if number.is_none() {
            num = get_number(&mut name);
            number = Some(&mut num);
        }

        let number = number.unwrap();
        if *number == 0 {
            return None;
        }

        for &i_id in &self.world[room as usize].peoples {
            let i = chars.get(i_id);
            if isname(&name, i.player.name.as_ref()) {
                *number -= 1;
                if *number == 0 {
                    return Some(i);
                }
            }
        }

        None
    }

    /* search all over the world for a char num, and return a pointer if found */
    pub fn get_char_num<'a>(
        &self,
        chars: &'a Depot<CharData>,
        nr: MobRnum,
    ) -> Option<&'a CharData> {
        for &i_id in &self.character_list {
            let i = chars.get(i_id);
            if i.get_mob_rnum() == nr {
                return Some(i);
            }
        }
        None
    }

    /* put an object in a room */
    pub fn obj_to_room(&mut self, obj: &mut ObjData, room: RoomRnum) {
        if room == NOWHERE || room >= self.world.len() as i16 {
            error!(
                "SYSERR: Illegal value(s) passed to obj_to_room. (Room #{}/{})",
                room,
                self.world.len()
            );
            return;
        }
        obj.set_in_room(room);
        obj.carried_by = None;

        if self.room_flagged(room, RoomFlags::HOUSE) {
            self.set_room_flags_bit(room, RoomFlags::HOUSE_CRASH)
        }
        self.world[room as usize].contents.push(obj.id());
    }

    /* Take an object from a room */
    pub fn obj_from_room(&mut self, obj: &ObjData) {
        let in_room = obj.in_room;
        if in_room == NOWHERE {
            error!(
                "SYSERR: obj not in a room ({}) passed to obj_from_room",
                in_room,
            );
            return;
        }

        self.world[in_room as usize]
            .contents
            .retain(|x| *x != obj.id());

        if self.room_flagged(in_room, RoomFlags::HOUSE) {
            self.set_room_flags_bit(in_room, RoomFlags::HOUSE_CRASH);
        }
    }
}
/* put an object in an object (quaint)  */
pub fn obj_to_obj(
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    oid: DepotId,
    oid_to: DepotId,
) {
    if oid == oid_to {
        error!("SYSERR: same source and target  obj passed to obj_to_obj.");
        return;
    }

    objs.get_mut(oid_to).contains.push(oid);
    objs.get_mut(oid).in_obj = Some(oid_to);
    let obj_weight = objs.get(oid).get_obj_weight();

    let mut tmp_oid = oid;
    loop {
        let tmp_obj = objs.get_mut(tmp_oid);
        if tmp_obj.in_obj.is_none() {
            break;
        }

        tmp_obj.set_obj_weight(obj_weight);
        tmp_oid = tmp_obj.in_obj.unwrap();
    }

    let tmp_obj = objs.get_mut(tmp_oid);
    /* top level object.  Subtract weight from inventory if necessary. */
    tmp_obj.incr_obj_weight(obj_weight);
    if tmp_obj.carried_by.is_some() {
        let carried_by_id = tmp_obj.carried_by.unwrap();
        chars.get_mut(carried_by_id).incr_is_carrying_w(obj_weight);
    }
}

/* remove an object from an object */
pub(crate) fn obj_from_obj(chars: &mut Depot<CharData>, objs: &mut Depot<ObjData>, oid: DepotId) {
    if objs.get(oid).in_obj.is_none() {
        error!("SYSERR:  trying to illegally extract obj from obj.");
        return;
    }
    let oid_from = objs.get(oid).in_obj.unwrap();
    let obj_weight = objs.get(oid).get_obj_weight();

    {
        let obj_from = objs.get_mut(oid_from);
        obj_from.contains.retain(|i| *i != oid);

        /* Subtract weight from containers container */

        let mut temp_id = objs.get(oid).in_obj.unwrap();
        loop {
            let tmp_obj = objs.get_mut(temp_id);

            if tmp_obj.in_obj.is_none() {
                break;
            }

            tmp_obj.incr_obj_weight(-obj_weight);
            temp_id = tmp_obj.in_obj.unwrap();
        }

        let temp = objs.get_mut(temp_id);
        /* Subtract weight from char that carries the object */
        temp.incr_obj_weight(-obj_weight);

        if temp.carried_by.is_some() {
            let carried_by_id = temp.carried_by.unwrap();
            chars.get_mut(carried_by_id).incr_is_carrying_w(-obj_weight);
        }
    }

    objs.get_mut(oid).in_obj = None;
}

/* Set all carried_by to point to new owner */
pub fn object_list_new_owner(
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    oid: DepotId,
    chid: Option<DepotId>,
) {
    for o in objs.get(oid).contains.clone() {
        object_list_new_owner(chars, objs, o, chid);
        objs.get_mut(oid).carried_by = chid;
    }
}
impl DB {
    /* Extract an object from the world */
    pub fn extract_obj(
        &mut self,
        chars: &mut Depot<CharData>,
        objs: &mut Depot<ObjData>,
        oid: DepotId,
    ) {
        let tch_id = &objs.get(oid).worn_by;
        if tch_id.is_some() {
            if self
                .unequip_char(chars, objs, tch_id.unwrap(), objs.get(oid).worn_on as i8)
                .unwrap()
                != oid
            {
                error!("SYSERR: Inconsistent worn_by and worn_on pointers!!");
            }
        }

        let obj = objs.get_mut(oid);
        if obj.in_room() != NOWHERE {
            self.obj_from_room(obj);
        } else if obj.carried_by.is_some() {
            obj_from_char(chars, obj);
        } else if obj.in_obj.is_some() {
            obj_from_obj(chars, objs, oid);
        }
        /* Get rid of the contents of the object, as well. */
        let obj = objs.get(oid);
        let mut old_object_list = vec![];
        for o in obj.contains.iter() {
            old_object_list.push(*o);
        }
        for o in old_object_list {
            self.extract_obj(chars, objs, o);
        }

        self.object_list.retain(|&i| i != oid);
        let obj = objs.get(oid);
        if obj.get_obj_rnum() != NOTHING {
            self.obj_index[obj.get_obj_rnum() as usize].number -= 1;
        }

        self.free_obj(objs, oid);
    }
}
fn update_object_list(objs: &mut Depot<ObjData>, list: Vec<DepotId>, _use: i32) {
    for oid in list {
        update_object(objs, oid, _use);
    }
}

fn update_object(objs: &mut Depot<ObjData>, oid: DepotId, _use: i32) {
    if objs.get(oid).get_obj_timer() > 0 {
        objs.get_mut(oid).decr_obj_timer(_use);
    }
    update_object_list(objs, objs.get(oid).contains.clone(), _use);
}

    pub(crate) fn update_char_objects(
        descs: &mut Depot<DescriptorData>,
        chars: &Depot<CharData>,
        objs: &mut Depot<ObjData>,
        db: &mut DB,
        chid: DepotId,
    ) {
        let ch = chars.get(chid);
        let i;
        let light_oid = ch.get_eq(WEAR_LIGHT as i8);

        if light_oid.is_some() {
            let light_oid = light_oid.unwrap();
            if objs.get(light_oid).get_obj_type() == ITEM_LIGHT {
                if objs.get(light_oid).get_obj_val(2) > 0 {
                    objs.get_mut(light_oid).decr_obj_val(2);
                    i = objs.get(light_oid).get_obj_val(2);
                    let ch = chars.get(chid);
                    if i == 1 {
                        send_to_char(descs, ch, "Your light begins to flicker and fade.\r\n");
                        act(descs, 
                            chars,
                            db,
                            "$n's light begins to flicker and fade.",
                            false,
                            Some(ch),
                            None,
                            None,
                            TO_ROOM,
                        );
                    } else if i == 0 {
                        send_to_char(descs, ch, "Your light sputters out and dies.\r\n");
                        act(descs, 
                            chars,
                            db,
                            "$n's light sputters out and dies.",
                            false,
                            Some(ch),
                            None,
                            None,
                            TO_ROOM,
                        );
                        let ch = chars.get(chid);
                        let in_room = ch.in_room();
                        db.world[in_room as usize].light -= 1;
                    }
                }
            }
        }
        for i in 0..NUM_WEARS {
            let ch = chars.get(chid);
            if ch.get_eq(i).is_some() {
                update_object(objs, ch.get_eq(i).unwrap(), 2);
            }
        }
        let ch = chars.get(chid);
        if !ch.carrying.is_empty() {
            update_object_list(objs, ch.carrying.clone(), 2);
        }
    }
impl Game {
    /* Extract a ch completely from the world, and leave his stuff behind */
    pub fn extract_char_final(
        &mut self,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        texts: &mut Depot<TextData>,
        objs: &mut Depot<ObjData>,
        chid: DepotId,
    ) {
        let ch = chars.get(chid);
        if ch.in_room() == NOWHERE {
            error!(
                "SYSERR: NOWHERE extracting char {}. (extract_char_final)",
                ch.get_name()
            );
            process::exit(1);
        }

        /*
         * We're booting the character of someone who has switched so first we
         * need to stuff them back into their own body.  This will set ch.desc
         * we're checking below this loop to the proper value.
         */
        if !ch.is_npc() && ch.desc.is_none() {
            for d_id in self.descriptor_list.clone() {
                let d = self.desc(d_id);
                if d.original.is_some() && d.original.unwrap() == chid {
                    let chid = d.character.unwrap();
                    do_return(self, db, chars, texts, objs, chid, "", 0, 0);
                    break;
                }
            }
        }
        let ch = chars.get(chid);
        if ch.desc.is_some() {
            /*
             * This time we're extracting the body someone has switched into
             * (not the body of someone switching as above) so we need to put
             * the switcher back to their own body.
             *
             * If this body is not possessed, the owner won't have a
             * body after the removal so dump them to the main menu.
             */
            if self.desc(ch.desc.unwrap()).original.borrow().is_some() {
                do_return(self, db, chars, texts, objs, chid, "", 0, 0);
            } else {
                /*
                 * Now we boot anybody trying to log in with the same character, to
                 * help guard against duping.  CON_DISCONNECT is used to close a
                 * descriptor without extracting the d.character associated with it,
                 * for being link-dead, so we want CON_CLOSE to clean everything up.
                 * If we're here, we know it's a player so no IS_NPC check required.
                 */
                for d_id in self.descriptor_list.clone() {
                    if d_id == ch.desc.unwrap() {
                        continue;
                    }
                    let d = self.desc(d_id);
                    if d.character.is_some()
                        && ch.get_idnum()
                            == chars
                                .get(d.character.unwrap())
                                .get_idnum()
                    {
                        self.desc_mut(d_id).set_state(ConClose);
                    }
                }
                let ch = chars.get(chid);
                let desc_id = ch.desc.unwrap();
                let desc = self.desc_mut(desc_id);
                desc.set_state(ConMenu);
                desc.write_to_output(MENU);
            }
        }

        /* On with the character's assets... */
        let ch = chars.get(chid);
        if ch.followers.len() != 0 || ch.master.is_some() {
            die_follower(&mut self.descriptors, chars, db, objs, chid);
        }

        /* transfer objects to room, if any */
        let ch = chars.get(chid);
        let ch_in_room = ch.in_room();
        for oid in ch.carrying.clone() {
            let obj = objs.get_mut(oid);
            obj_from_char(chars, obj);
            db.obj_to_room(obj, ch_in_room);
        }

        /* transfer equipment to room, if any */
        for i in 0..NUM_WEARS {
            let ch = chars.get(chid);
            if ch.get_eq(i).is_some() {
                let oid = db.unequip_char(chars, objs, chid, i).unwrap();
                let ch = chars.get(chid);
                let obj = objs.get_mut(oid);
                db.obj_to_room(obj, ch.in_room())
            }
        }
        let ch = chars.get_mut(chid);
        if ch.fighting_id().is_some() {
            db.stop_fighting(ch);
        }

        let mut old_combat_list = vec![];
        for &c in &db.combat_list {
            old_combat_list.push(c);
        }
        for k_id in old_combat_list.clone() {
            let k = chars.get_mut(k_id);
            if k.fighting_id().unwrap() == chid {
                db.stop_fighting(k);
            }
        }
        /* we can't forget the hunters either... */
        for &temp_id in &db.character_list {
            let temp = chars.get_mut(temp_id);
            if temp.char_specials.hunting_chid.is_some()
                && temp.char_specials.hunting_chid.unwrap() == chid
            {
                temp.char_specials.hunting_chid = None;
            }
        }
        let ch = chars.get_mut(chid);
        db.char_from_room(objs, ch);
        let ch = chars.get(chid);
        if ch.is_npc() {
            if ch.get_mob_rnum() != NOTHING {
                let rnum = ch.get_mob_rnum();
                db.mob_index[rnum as usize].number -= 1;
            }
            let ch = chars.get_mut(chid);
            ch.clear_memory()
        } else {
            save_char(&mut self.descriptors, db, chars, texts, objs, chid);
            let ch = chars.get(chid);
            crash_delete_crashfile(ch);
        }

        /* If there's a descriptor, they're in the menu now. */
        let ch = chars.get(chid);
        if ch.is_npc() || ch.desc.is_none() {
            db.free_char(&mut self.descriptors, chars, objs, chid)
        }
    }
}
impl DB {
    /*
     * Q: Why do we do this?
     * A: Because trying to iterate over the character
     *    list with 'ch = ch.next' does bad things if
     *    the current character happens to die. The
     *    trivial workaround of 'vict = next_vict'
     *    doesn't work if the _next_ person in the list
     *    gets killed, for example, by an area spell.
     *
     * Q: Why do we leave them on the character_list?
     * A: Because code doing 'vict = vict.next' would
     *    get really confused otherwise.
     */
    pub fn extract_char(&mut self, chars: &mut Depot<CharData>, chid: DepotId) {
        let ch = chars.get_mut(chid);
        if ch.is_npc() {
            ch.set_mob_flags_bit(MOB_NOTDEADYET);
        } else {
            ch.set_plr_flag_bit(PLR_NOTDEADYET);
        }

        self.extractions_pending += 1;
    }
}

/*
 * I'm not particularly pleased with the MOB/PLR
 * hoops that have to be jumped through but it
 * hardly calls for a completely new variable.
 * Ideally it would be its own list, but that
 * would change the '.next' pointer, potentially
 * confusing some code. Ugh. -gg 3/15/2001
 *
 * NOTE: This doesn't handle recursive extractions.
 */
impl Game {
    pub fn extract_pending_chars(
        &mut self,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        texts: &mut Depot<TextData>,
        objs: &mut Depot<ObjData>,
    ) {
        if db.extractions_pending < 0 {
            error!(
                "SYSERR: Negative ({}) extractions pending.",
                db.extractions_pending
            );
        }

        for &vict_id in &db.character_list.clone() {
            let vict = chars.get_mut(vict_id);
            if vict.mob_flagged(MOB_NOTDEADYET) {
                vict.remove_mob_flags_bit(MOB_NOTDEADYET);
            } else if vict.plr_flagged(PLR_NOTDEADYET) {
                vict.remove_plr_flag(PLR_NOTDEADYET);
            } else {
                /* Last non-free'd character to continue chain from. */
                continue;
            }

            self.extract_char_final(chars, db, texts, objs, vict_id);
            db.character_list.retain(|&i| i != vict_id);
            db.extractions_pending -= 1;
        }

        if db.extractions_pending > 0 {
            error!(
                "SYSERR: Couldn't find {} extractions as counted.",
                db.extractions_pending
            );
        }

        db.extractions_pending = 0;
    }
}

/* ***********************************************************************
* Here follows high-level versions of some earlier routines, ie functions*
* which incorporate the actual player-data                               *.
*********************************************************************** */
    pub fn get_player_vis<'a>(
        descs: &Depot<DescriptorData>,
        chars: &'a Depot<CharData>,
        db: &'a DB,
        ch: &CharData,
        name: &mut String,
        number: Option<&mut i32>,
        inroom: i32,
    ) -> Option<&'a CharData> {
        //let ch = chars.get(chid);
        let mut num;
        let t: &mut i32;
        if number.is_none() {
            num = get_number(name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let number = t;

        for &i_id in &db.character_list {
            let i = chars.get(i_id);
            if i.is_npc() {
                continue;
            }
            if inroom == FIND_CHAR_ROOM && ch.in_room() != i.in_room() {
                continue;
            }
            if i.player.name.as_ref() != name {
                continue;
            }
            if !can_see(descs, chars, db, ch, i) {
                continue;
            }
            *number -= 1;
            if *number != 0 {
                continue;
            }
            return Some(i);
        }
        return None;
    }

    pub fn get_char_room_vis<'a>(
        descs: &Depot<DescriptorData>,
        chars: &'a Depot<CharData>,
        db: &'a DB,
        ch: &'a CharData,
        name: &mut String,
        number: Option<&mut i32>,
    ) -> Option<&'a CharData> {
        //let ch = chars.get(chid);
        let mut num;
        let t: &mut i32;
        if number.is_none() {
            num = get_number(name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let number = t;

        /* JE 7/18/94 :-) :-) */
        if name == "self" || name == "me" {
            return Some(ch);
        }

        /* 0.<name> means PC with name */
        if *number == 0 {
            return get_player_vis(descs, chars, db, ch, name, None, FIND_CHAR_ROOM);
        }

        for i_id in db.world[ch.in_room() as usize].peoples.clone() {
            let i = chars.get(i_id);
            if isname(name, i.player.name.as_ref()) {
                if can_see(descs, chars, db, ch, i) {
                    *number -= 1;
                    if *number == 0 {
                        return Some(i);
                    }
                }
            }
        }
        return None;
    }

    pub fn get_char_world_vis<'a>(
        descs: &Depot<DescriptorData>,
        chars: &'a Depot<CharData>,
        db: &'a DB,
        ch: &'a CharData,
        name: &mut String,
        number: Option<&mut i32>,
    ) -> Option<&'a CharData> {
        let mut num;
        let t: &mut i32;
        if number.is_none() {
            num = get_number(name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let number: &mut i32 = t;

        let i = get_char_room_vis(descs, chars, db, ch, name, Some(number));
        if i.is_some() {
            return i;
        }

        /* 0.<name> means PC with name */
        if *number == 0 {
            return get_player_vis(descs, chars, db, ch, name, None, 0);
        }

        for &i_id in &db.character_list {
            let i = chars.get(i_id);
            if ch.in_room() == i.in_room() {
                continue;
            }
            if !isname(name, i.player.name.as_ref()) {
                continue;
            }
            if !can_see(descs, chars, db, ch, i) {
                continue;
            }
            *number -= 1;
            if *number != 0 {
                continue;
            }
            return Some(i);
        }
        return None;
    }

    pub fn get_char_vis<'a>(
        descs: &Depot<DescriptorData>,
        chars: &'a Depot<CharData>,
        db: &'a DB,
        ch: &'a CharData,
        name: &mut String,
        number: Option<&mut i32>,
        _where: i32,
    ) -> Option<&'a CharData> {
        return if _where == FIND_CHAR_ROOM {
            get_char_room_vis(descs, chars, db, ch, name, number)
        } else if _where == FIND_CHAR_WORLD {
            get_char_world_vis(descs, chars, db, ch, name, number)
        } else {
            None
        };
    }

    pub fn get_obj_in_list_vis<'a>(
        descs: &Depot<DescriptorData>,
        chars: &Depot<CharData>,
        db: &'a DB,
        objs: &'a Depot<ObjData>,
        ch: &'a CharData,
        name: &str,
        number: Option<&mut i32>,
        list: &Vec<DepotId>,
    ) -> Option<&'a ObjData> {
        let mut num;
        let t: &mut i32;
        let mut name = name.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let number: &mut i32 = t;
        if *number == 0 {
            return None;
        }

        for i in list.iter() {
            if isname(&name, objs.get(*i).name.as_ref()) {
                let obj = objs.get(*i);
                if can_see_obj(descs, chars, db, ch, obj) {
                    *number -= 1;
                    if *number == 0 {
                        return Some(obj);
                    }
                }
            }
        }

        None
    }

    pub fn get_obj_in_list_vis2<'a>(
        descs: &Depot<DescriptorData>,
        chars: &Depot<CharData>,
        db: &'a DB,
        objs: &'a Depot<ObjData>,
        ch: &'a CharData,
        name: &str,
        number: Option<&mut i32>,
        list: &Vec<DepotId>,
    ) -> Option<&'a ObjData> {
        let mut num;
        let t: &mut i32;
        let mut name = name.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let number: &mut i32 = t;
        if *number == 0 {
            return None;
        }

        for i in list.iter() {
            if isname(&name, objs.get(*i).name.as_ref()) {
                let obj = objs.get(*i);
                if can_see_obj(descs, chars, db, ch, obj) {
                    *number -= 1;
                    if *number == 0 {
                        return Some(obj);
                    }
                }
            }
        }

        None
    }

    /* search the entire world for an object, and return a pointer  */
    pub fn get_obj_vis<'a>(
        descs: &Depot<DescriptorData>,
        chars: &Depot<CharData>,
        db: &'a DB,
        objs: &'a Depot<ObjData>,
        ch: &'a CharData,
        name: &str,
        number: Option<&mut i32>,
    ) -> Option<&'a ObjData> {
        let mut num;
        let t: &mut i32;
        let mut name = name.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let number: &mut i32 = t;
        if *number == 0 {
            return None;
        }

        /* scan items carried */
        let i = get_obj_in_list_vis(descs, chars, db, objs, ch, &name, Some(number), &ch.carrying);
        if i.is_some() {
            return i;
        }

        /* scan room */
        let i = get_obj_in_list_vis2(descs, 
            chars,
            db,
            objs,
            ch,
            &name,
            Some(number),
            &db.world[ch.in_room() as usize].contents,
        );
        if i.is_some() {
            return i;
        }

        /* ok.. no luck yet. scan the entire obj list   */
        for &oid in db.object_list.iter() {
            let i = objs.get(oid);
            if isname(&name, &i.name.borrow()) {
                if can_see_obj(descs, chars, db, ch, i) {
                    *number -= 1;
                    if *number == 0 {
                        return Some(i);
                    }
                }
            }
        }
        None
    }

    pub fn get_obj_in_equip_vis<'a>(
        descs: &Depot<DescriptorData>,
        chars: &Depot<CharData>,
        db: &'a DB,
        objs: &'a Depot<ObjData>,
        ch: &'a CharData,
        arg: &str,
        number: Option<&mut i32>,
        equipment: &[Option<DepotId>],
    ) -> Option<&'a ObjData> {
        let mut num;
        let t: &mut i32;
        let mut name = arg.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let number: &mut i32 = t;
        if *number == 0 {
            return None;
        }
        let equipment = equipment;
        for j in 0..NUM_WEARS as usize {
            if equipment[j].is_some()
                && can_see_obj(descs, chars, db, ch, objs.get(equipment[j].unwrap()))
                && isname(&arg, objs.get(equipment[j].unwrap()).name.as_ref())
            {
                *number -= 1;
                if *number == 0 {
                    return equipment[j].map(|i| objs.get(i));
                }
            }
        }

        None
    }

    pub fn get_obj_pos_in_equip_vis(
        descs: &Depot<DescriptorData>,
        chars: &Depot<CharData>,
        db: &DB,
        objs: &Depot<ObjData>,
        ch: &CharData,
        arg: &str,
        number: Option<&mut i32>,
        equipment: &[Option<DepotId>],
    ) -> Option<i8> {
        let equipment = equipment;
        let mut num;
        let t: &mut i32;
        let mut name = arg.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let number: &mut i32 = t;
        if *number == 0 {
            return None;
        }

        for j in 0..NUM_WEARS as usize {
            if equipment[j].is_some()
                && can_see_obj(descs, chars, db, ch, objs.get(equipment[j].unwrap()))
                && isname(arg, objs.get(equipment[j].unwrap()).name.as_ref())
            {
                if {
                    *number -= 1;
                    *number == 0
                } {
                    return Some(j as i8);
                }
            }
        }

        return None;
    }


pub fn money_desc(amount: i32) -> &'static str {
    struct MyItem {
        limit: i32,
        description: &'static str,
    }

    const MONEY_TABLE: [MyItem; 14] = [
        MyItem {
            limit: 1,
            description: "a gold coin",
        },
        MyItem {
            limit: 10,
            description: "a tiny pile of gold coins",
        },
        MyItem {
            limit: 20,
            description: "a handful of gold coins",
        },
        MyItem {
            limit: 75,
            description: "a little pile of gold coins",
        },
        MyItem {
            limit: 200,
            description: "a small pile of gold coins",
        },
        MyItem {
            limit: 1000,
            description: "a pile of gold coins",
        },
        MyItem {
            limit: 5000,
            description: "a big pile of gold coins",
        },
        MyItem {
            limit: 10000,
            description: "a large heap of gold coins",
        },
        MyItem {
            limit: 20000,
            description: "a huge mound of gold coins",
        },
        MyItem {
            limit: 75000,
            description: "an enormous mound of gold coins",
        },
        MyItem {
            limit: 150000,
            description: "a small mountain of gold coins",
        },
        MyItem {
            limit: 250000,
            description: "a mountain of gold coins",
        },
        MyItem {
            limit: 500000,
            description: "a huge mountain of gold coins",
        },
        MyItem {
            limit: 1000000,
            description: "an enormous mountain of gold coins",
        },
    ];

    if amount <= 0 {
        error!("SYSERR: Try to create negative or 0 money ({}).", amount);
        return "";
    }

    for item in MONEY_TABLE {
        if amount < item.limit {
            return item.description;
        }
    }

    return "an absolutely colossal mountain of gold coins";
}

impl DB {
    pub fn create_money(&mut self, objs: &mut Depot<ObjData>, amount: i32) -> Option<DepotId> {
        if amount <= 0 {
            error!("SYSERR: Try to create negative or 0 money. ({})", amount);
            return None;
        }
        let mut obj = ObjData::default();
        let mut new_descr = ExtraDescrData::new();

        if amount == 1 {
            obj.name = Rc::from("coin gold");
            obj.short_description = Rc::from("a gold coin");
            obj.description = Rc::from("One miserable gold coin is lying here.");
            new_descr.keyword = Rc::from("coin gold");
            new_descr.description = Rc::from("It's just one miserable little gold coin.");
        } else {
            obj.name = Rc::from("coins gold");
            obj.short_description = Rc::from(money_desc(amount));
            obj.description = Rc::from(format!("{} is lying here.", money_desc(amount)).as_str());

            new_descr.keyword = Rc::from("coins gold");
            let buf;
            if amount < 10 {
                buf = format!("There are {} coins.", amount);
            } else if amount < 100 {
                buf = format!("There are about {} coins.", 10 * (amount / 10));
            } else if amount < 1000 {
                buf = format!("It looks to be about {} coins.", 100 * (amount / 100));
            } else if amount < 100000 {
                buf = format!(
                    "You guess there are, maybe, {} coins.",
                    1000 * ((amount / 1000) + rand_number(0, (amount / 1000) as u32) as i32)
                );
            } else {
                buf = format!("There are a LOT of coins."); /* strcpy: OK (is < 200) */
            }
            new_descr.description = Rc::from(buf.as_str());
        }

        obj.set_obj_type(ITEM_MONEY);
        obj.set_obj_wear(WearFlags::TAKE);
        obj.set_obj_val(0, amount);
        obj.set_obj_cost(amount);
        obj.item_number = NOTHING;

        obj.ex_descriptions.push(new_descr);
        let oid = objs.push(obj);
        self.object_list.push(oid);
        Some(oid)
    }
}
    /* Generic Find, designed to find any object/character
     *
     * Calling:
     *  *arg     is the pointer containing the string to be searched for.
     *           This string doesn't have to be a single word, the routine
     *           extracts the next word itself.
     *  bitv..   All those bits that you want to "search through".
     *           Bit found will be result of the function
     *  *ch      This is the person that is trying to "find"
     *  **tar_ch Will be NULL if no character was found, otherwise points
     * **tar_obj Will be NULL if no object was found, otherwise points
     *
     * The routine used to return a pointer to the next word in *arg (just
     * like the one_argument routine), but now it returns an integer that
     * describes what it filled in.
     */
    pub fn generic_find<'a>(
        descs: &Depot<DescriptorData>,
        chars: &'a Depot<CharData>,
        db: &'a DB,
        objs: &'a Depot<ObjData>,
        arg: &str,
        bitvector: i64,
        ch: &'a CharData,
        tar_ch: &mut Option<&'a CharData>,
        tar_obj: &mut Option<&'a ObjData>,
    ) -> i32 {
        let mut name = String::new();
        let mut found = false;

        one_argument(arg, &mut name);

        if name.is_empty() {
            return 0;
        }
        let mut number = get_number(&mut name);
        if number == 0 {
            return 0;
        }

        if is_set!(bitvector, FIND_CHAR_ROOM as i64) {
            /* Find person in room */
            *tar_ch = get_char_room_vis(descs, chars, db, ch, &mut name, Some(&mut number));

            if tar_ch.is_some() {
                return FIND_CHAR_ROOM;
            }
        }

        if is_set!(bitvector, FIND_CHAR_WORLD as i64) {
            *tar_ch = get_char_world_vis(descs, chars, db, ch, &mut name, Some(&mut number));
            if tar_ch.is_some() {
                return FIND_CHAR_WORLD;
            }
        }

        if is_set!(bitvector, FIND_OBJ_EQUIP as i64) {
            for i in 0..NUM_WEARS {
                if found {
                    break;
                }

                if ch.get_eq(i).is_some()
                    && isname(name.as_str(), objs.get(ch.get_eq(i).unwrap()).name.as_ref())
                {
                    number -= 1;
                    if number == 0 {
                        *tar_obj = Some(objs.get(ch.get_eq(i).unwrap()));
                        found = true;
                    }
                }
            }
            if found {
                return FIND_OBJ_EQUIP;
            }
        }

        if is_set!(bitvector, FIND_OBJ_INV as i64) {
            *tar_obj = get_obj_in_list_vis(descs,
                chars,
                db,
                objs,
                ch,
                &name,
                Some(&mut number),
                &ch.carrying,
            );
            if tar_obj.is_some() {
                return FIND_OBJ_INV;
            }
        }

        if is_set!(bitvector, FIND_OBJ_ROOM as i64) {
            *tar_obj = get_obj_in_list_vis2(descs,
                chars,
                db,
                objs,
                ch,
                &name,
                Some(&mut number),
                &db.world[ch.in_room() as usize].contents,
            );
            if tar_obj.is_some() {
                return FIND_OBJ_ROOM;
            }
        }

        if is_set!(bitvector, FIND_OBJ_WORLD as i64) {
            *tar_obj = get_obj_vis(descs, chars, db, objs, ch, &name, Some(&mut number));
            if tar_obj.is_some() {
                return FIND_OBJ_WORLD;
            }
        }
        0
    }


pub const FIND_INDIV: u8 = 0;
pub const FIND_ALL: u8 = 1;
pub const FIND_ALLDOT: u8 = 2;

/* a function to scan for "all" or "all.x" */
pub fn find_all_dots(arg: &str) -> u8 {
    if arg == "all" {
        return FIND_ALL;
    } else if arg.starts_with("all.") {
        return FIND_ALLDOT;
    } else {
        return FIND_INDIV;
    }
}
