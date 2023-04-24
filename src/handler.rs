/* ************************************************************************
*   File: handler.c                                     Part of CircleMUD *
*  Usage: internal funcs: moving and finding chars/objs                   *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::borrow::Borrow;
use std::cell::{Ref, RefCell};
use std::cmp::{max, min};
use std::process;
use std::rc::Rc;

use log::{error, info};

use crate::act_wizard::do_return;
use crate::class::invalid_class;
use crate::config::MENU;
use crate::db::DB;
use crate::interpreter::one_argument;
use crate::objsave::crash_delete_crashfile;
use crate::spells::{SAVING_BREATH, SAVING_PARA, SAVING_PETRI, SAVING_ROD, SAVING_SPELL};
use crate::structs::ConState::{ConClose, ConMenu};
use crate::structs::{
    AffectedType, CharData, ExtraDescrData, MobRnum, ObjData, ObjRnum, RoomRnum, APPLY_AC,
    APPLY_AGE, APPLY_CHA, APPLY_CHAR_HEIGHT, APPLY_CHAR_WEIGHT, APPLY_CLASS, APPLY_CON,
    APPLY_DAMROLL, APPLY_DEX, APPLY_EXP, APPLY_GOLD, APPLY_HIT, APPLY_HITROLL, APPLY_INT,
    APPLY_LEVEL, APPLY_MANA, APPLY_MOVE, APPLY_NONE, APPLY_SAVING_BREATH, APPLY_SAVING_PARA,
    APPLY_SAVING_PETRI, APPLY_SAVING_ROD, APPLY_SAVING_SPELL, APPLY_STR, APPLY_WIS, ITEM_ANTI_EVIL,
    ITEM_ANTI_GOOD, ITEM_ANTI_NEUTRAL, ITEM_ARMOR, ITEM_LIGHT, ITEM_MONEY, ITEM_WEAR_TAKE,
    LVL_GRGOD, MAX_OBJ_AFFECT, MOB_NOTDEADYET, NOTHING, NOWHERE, NUM_WEARS, PLR_CRASH,
    PLR_NOTDEADYET, ROOM_HOUSE, ROOM_HOUSE_CRASH, WEAR_BODY, WEAR_HEAD, WEAR_LEGS, WEAR_LIGHT,
};
use crate::util::{clone_vec, rand_number, SECS_PER_MUD_YEAR};
use crate::{is_set, send_to_char, write_to_output, Game, TO_CHAR, TO_ROOM};

pub const FIND_CHAR_ROOM: i32 = 1 << 0;
pub const FIND_CHAR_WORLD: i32 = 1 << 1;
pub const FIND_OBJ_INV: i32 = 1 << 2;
pub const FIND_OBJ_ROOM: i32 = 1 << 3;
pub const FIND_OBJ_WORLD: i32 = 1 << 4;
pub const FIND_OBJ_EQUIP: i32 = 1 << 5;

// /* local vars */
// int extractions_pending = 0;
//
// /* external vars */
// extern struct char_data *combat_list;
// extern const char *MENU;
//
// /* local functions */
// int apply_ac(struct char_data *ch, int eq_pos);
// void update_object(struct obj_data *obj, int use);
// void update_char_objects(struct char_data *ch);
//
// /* external functions */
// int invalid_class(struct char_data *ch, struct obj_data *obj);
// void remove_follower(struct char_data *ch);
// void clear_memory(struct char_data *ch);
// ACMD(do_return);

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
    info!("[DEBUG] {} namelist='{}'", txt, namelist);
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

fn affect_modify(ch: &CharData, loc: i8, _mod: i16, bitv: i64, add: bool) {
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
            ch.player.borrow_mut().time.birth -= _mod as u64 * SECS_PER_MUD_YEAR;
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
pub fn affect_total(ch: &CharData) {
    //struct affected_type *af;
    //int i, j;

    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some() {
            let eq = ch.get_eq(i).unwrap();
            for j in 0..MAX_OBJ_AFFECT {
                affect_modify(
                    ch,
                    eq.affected[j as usize].get().location as i8,
                    eq.affected[j as usize].get().modifier as i16,
                    eq.get_obj_affect(),
                    false,
                );
            }
        }
    }

    for af in ch.affected.borrow().iter() {
        affect_modify(
            ch,
            af.location as i8,
            af.modifier as i16,
            af.bitvector,
            false,
        );
    }

    *ch.aff_abils.borrow_mut() = *ch.real_abils.borrow_mut();

    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some() {
            let eq = ch.get_eq(i).unwrap();
            for j in 0..MAX_OBJ_AFFECT {
                affect_modify(
                    ch,
                    eq.affected[j as usize].get().location as i8,
                    eq.affected[j as usize].get().modifier as i16,
                    eq.get_obj_affect(),
                    true,
                )
            }
        }
    }

    for af in ch.affected.borrow().iter() {
        affect_modify(
            ch,
            af.location as i8,
            af.modifier as i16,
            af.bitvector,
            true,
        );
    }

    /* Make certain values are between 0..25, not < 0 and not > 25! */

    let mut i = if ch.is_npc() || ch.get_level() >= LVL_GRGOD as u8 {
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
            i = ch.get_add() + ((ch.get_str() - 18) * 10);
            ch.set_add(min(i, 100));
            ch.set_str(18);
        }
    }
}

/* Insert an affect_type in a char_data structure
Automatically sets apropriate bits and apply's */
pub fn affect_to_char(ch: &Rc<CharData>, af: &AffectedType) {
    ch.affected.borrow_mut().push(af.clone());

    affect_modify(
        ch,
        af.location as i8,
        af.modifier as i16,
        af.bitvector,
        true,
    );
    affect_total(ch);
}

/*
 * Remove an affected_type structure from a char (called when duration
 * reaches zero). Pointer *af must never be NIL!  Frees mem and calls
 * affect_location_apply
 */
pub fn affect_remove(ch: &Rc<CharData>, af: &AffectedType) {
    affect_modify(
        ch,
        af.location as i8,
        af.modifier as i16,
        af.bitvector,
        false,
    );
    affect_total(ch);
}

/* Call affect_remove with every spell of spelltype "skill" */
pub fn affect_from_char(ch: &Rc<CharData>, type_: i16) {
    ch.affected.borrow_mut().retain(|hjp| {
        if hjp._type == type_ {
            affect_remove(ch, hjp);
            false
        } else {
            true
        }
    });
}

/*
 * Return TRUE if a char is affected by a spell (SPELL_XXX),
 * FALSE indicates not affected.
 */
pub fn affected_by_spell(ch: &Rc<CharData>, type_: i16) -> bool {
    for hjp in ch.affected.borrow().iter() {
        if hjp._type == type_ {
            return true;
        }
    }

    false
}

pub fn affect_join(
    ch: &Rc<CharData>,
    af: &mut AffectedType,
    add_dur: bool,
    avg_dur: bool,
    add_mod: bool,
    avg_mod: bool,
) {
    ch.affected.borrow_mut().retain_mut(|hjp| {
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

            affect_remove(ch, hjp);
            false
        } else {
            true
        }
    });
    affect_to_char(ch, af);
}

impl DB {
    /* move a player out of a room */
    pub fn char_from_room(&self, ch: &Rc<CharData>) {
        if ch.in_room() == NOWHERE {
            error!("SYSERR: NULL character or NOWHERE in char_from_room");
            process::exit(1);
        }

        if ch.fighting().is_some() {
            self.stop_fighting(ch);
        }

        if ch.get_eq(WEAR_LIGHT as i8).is_some() {
            if ch.get_eq(WEAR_LIGHT as i8).as_ref().unwrap().get_obj_type() == ITEM_LIGHT {
                if ch.get_eq(WEAR_LIGHT as i8).as_ref().unwrap().get_obj_val(2) != 0 {
                    self.world.borrow()[ch.in_room() as usize]
                        .light
                        .set(self.world.borrow()[ch.in_room() as usize].light.get() - 1);
                }
            }
        }

        let w = self.world.borrow();
        let mut list = w[ch.in_room() as usize].peoples.borrow_mut();
        list.retain(|c_rch| !Rc::ptr_eq(c_rch, ch));
    }

    /* place a character in a room */
    pub(crate) fn char_to_room(&self, ch: Option<&Rc<CharData>>, room: RoomRnum) {
        if ch.is_none() && room == NOWHERE || room >= self.world.borrow().len() as i16 {
            error!(
                "SYSERR: Illegal value(s) passed to char_to_room. (Room: {}/{} Ch: {}",
                room,
                self.world.borrow().len(),
                'x'
            );
        } else {
            let ch = ch.unwrap();
            self.world.borrow()[room as usize]
                .peoples
                .borrow_mut()
                .push(ch.clone());
            // *ch.next_in_room.borrow_mut() =
            //     self.world.borrow()[room as usize].people.borrow().clone();
            // *self.world.borrow_mut()[room as usize].people.borrow_mut() = Some(ch.clone());
            ch.set_in_room(room);

            if ch.get_eq(WEAR_LIGHT as i8).is_some() {
                if ch.get_eq(WEAR_LIGHT as i8).as_ref().unwrap().get_obj_type() == ITEM_LIGHT {
                    if ch.get_eq(WEAR_LIGHT as i8).as_ref().unwrap().get_obj_val(2) != 0 {
                        self.world.borrow()[ch.in_room() as usize]
                            .light
                            .set(self.world.borrow()[ch.in_room() as usize].light.get() + 1);
                        /* Light ON */
                    }
                }
            }

            /* Stop fighting now, if we left. */
            if ch.fighting().is_some() && ch.in_room() != ch.fighting().as_ref().unwrap().in_room()
            {
                self.stop_fighting(ch.fighting().as_ref().unwrap());
                self.stop_fighting(ch);
            }
        }
    }

    /* give an object to a char   */
    pub fn obj_to_char(object: Option<&Rc<ObjData>>, ch: Option<&Rc<CharData>>) {
        if object.is_some() && ch.is_some() {
            let object = object.unwrap();
            let ch = ch.unwrap();
            ch.carrying.borrow_mut().push(object.clone());
            *object.carried_by.borrow_mut() = Some(ch.clone());
            object.as_ref().set_in_room(NOWHERE);

            ch.incr_is_carrying_w(object.get_obj_weight());
            ch.incr_is_carrying_n();

            /* set flag for crash-save system, but not on mobs! */
            if !ch.is_npc() {
                ch.set_plr_flag_bit(PLR_CRASH)
            }
        } else {
            error!("SYSERR: NULL obj  or char passed to obj_to_char.");
        }
    }
}
/* take an object from a char */
pub fn obj_from_char(object: Option<&Rc<ObjData>>) {
    if object.is_none() {
        error!("SYSERR: NULL object passed to obj_from_char.");
        return;
    }
    let object = object.unwrap();
    object
        .carried_by
        .borrow()
        .as_ref()
        .unwrap()
        .carrying
        .borrow_mut()
        .retain(|x| !Rc::ptr_eq(x, &object));

    /* set flag for crash-save system, but not on mobs! */
    if !object.carried_by.borrow().as_ref().unwrap().is_npc() {
        object
            .carried_by
            .borrow()
            .as_ref()
            .unwrap()
            .set_plr_flag_bit(PLR_CRASH);
    }

    object
        .carried_by
        .borrow()
        .as_ref()
        .unwrap()
        .incr_is_carrying_w(-object.get_obj_weight());
    object
        .carried_by
        .borrow()
        .as_ref()
        .unwrap()
        .decr_is_carrying_n();
    *object.carried_by.borrow_mut() = None;
}

/* Return the effect of a piece of armor in position eq_pos */
fn apply_ac(ch: &CharData, eq_pos: i16) -> i32 {
    if ch.get_eq(eq_pos as i8).is_none() {
        //core_dump();
        return 0;
    }
    if ch.get_eq(eq_pos as i8).unwrap().get_obj_type() != ITEM_ARMOR as u8 {
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
    factor * ch.get_eq(eq_pos as i8).unwrap().get_obj_val(0)
}

pub fn invalid_align(ch: &CharData, obj: &ObjData) -> bool {
    if obj.obj_flagged(ITEM_ANTI_EVIL) && ch.is_evil() {
        return true;
    };
    if obj.obj_flagged(ITEM_ANTI_GOOD) && ch.is_good() {
        return true;
    }
    if obj.obj_flagged(ITEM_ANTI_NEUTRAL) && ch.is_neutral() {
        return true;
    }
    false
}

impl DB {
    pub(crate) fn equip_char(&self, ch: Option<&Rc<CharData>>, obj: Option<&Rc<ObjData>>, pos: i8) {
        //int j;

        if pos < 0 || pos >= NUM_WEARS {
            //core_dump();
            return;
        }
        let ch = ch.unwrap();
        let obj = obj.unwrap();

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
        if invalid_align(ch.as_ref(), obj.as_ref()) || invalid_class(ch.as_ref(), obj.as_ref()) {
            self.act(
                "You are zapped by $p and instantly let go of it.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
            self.act(
                "$n is zapped by $p and instantly lets go of it.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_ROOM,
            );
            /* Changed to drop in inventory instead of the ground. */
            DB::obj_to_char(Some(obj), Some(ch));
            return;
        }

        ch.set_eq(pos, Some(obj.clone()));
        *obj.worn_by.borrow_mut() = Some(ch.clone());
        obj.worn_on.set(pos as i16);

        if obj.get_obj_type() == ITEM_ARMOR as u8 {
            ch.set_ac(ch.get_ac() - apply_ac(ch.as_ref(), pos as i16) as i16);
        }

        if ch.in_room() != NOWHERE {
            if pos == WEAR_LIGHT as i8 && obj.get_obj_type() == ITEM_LIGHT as u8 {
                if obj.get_obj_val(2) != 0 {
                    /* if light is ON */
                    self.world.borrow()[ch.in_room() as usize]
                        .light
                        .set(self.world.borrow()[ch.in_room() as usize].light.get() + 1);
                }
            }
        } else {
            error!(
                "SYSERR: IN_ROOM(ch) = NOWHERE when equipping char {}.",
                ch.get_name()
            );
        }

        for j in 0..MAX_OBJ_AFFECT {
            affect_modify(
                ch.as_ref(),
                obj.affected[j as usize].get().location as i8,
                obj.affected[j as usize].get().modifier as i16,
                obj.get_obj_affect(),
                true,
            );
        }

        affect_total(ch.as_ref());
    }

    pub fn unequip_char(&self, ch: &Rc<CharData>, pos: i8) -> Option<Rc<ObjData>> {
        if pos < 0 || pos > NUM_WEARS || ch.get_eq(pos).is_none() {
            //core_dump();
            return None;
        }

        let obj = ch.get_eq(pos).unwrap();
        *obj.worn_by.borrow_mut() = None;
        obj.worn_on.set(-1);

        if obj.get_obj_type() == ITEM_ARMOR as u8 {
            ch.set_ac(ch.get_ac() + apply_ac(ch.as_ref(), pos as i16) as i16);
        }

        if ch.in_room() != NOWHERE {
            if pos == WEAR_LIGHT as i8 && obj.get_obj_type() == ITEM_LIGHT as u8 {
                if obj.get_obj_val(2) != 0 {
                    self.world.borrow()[ch.in_room() as usize]
                        .light
                        .set(self.world.borrow()[ch.in_room() as usize].light.get() - 1);
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
            affect_modify(
                ch.as_ref(),
                obj.affected[j as usize].get().location as i8,
                obj.affected[j as usize].get().modifier as i16,
                obj.get_obj_affect(),
                false,
            );
        }

        affect_total(ch.as_ref());

        Some(obj.clone())
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

impl DB {
    /* Search a given list for an object number, and return a ptr to that obj */
    pub fn get_obj_in_list_num(&self, num: i16, list: &Vec<Rc<ObjData>>) -> Option<Rc<ObjData>> {
        for o in list {
            if o.get_obj_rnum() == num {
                return Some(o.clone());
            }
        }
        None
    }

    /* search the entire world for an object number, and return a pointer  */
    pub(crate) fn get_obj_num(&self, nr: ObjRnum) -> Option<Rc<ObjData>> {
        for o in self.object_list.borrow_mut().iter() {
            if o.get_obj_rnum() == nr {
                return Some(o.clone());
            }
        }
        None
    }
}

// /* search a room for a char, and return a pointer if found..  */
// struct char_data *get_char_room(char *name, int *number, RoomRnum room)
// {
// struct char_data *i;
// int num;
//
// if (!number) {
// number = &num;
// num = get_number(&name);
// }
//
// if (*number == 0)
// return (NULL);
//
// for (i = world[room].people; i && *number; i = i.next_in_room)
// if (isname(name, i.player.name))
// if (--(*number) == 0)
// return (i);
//
// return (NULL);
// }

impl DB {
    /* search all over the world for a char num, and return a pointer if found */
    pub fn get_char_num(&self, nr: MobRnum) -> Option<Rc<CharData>> {
        for i in self.character_list.borrow().iter() {
            if i.get_mob_rnum() == nr {
                return Some(i.clone());
            }
        }

        None
    }

    /* put an object in a room */
    pub fn obj_to_room(&self, object: Option<&Rc<ObjData>>, room: RoomRnum) {
        if object.is_none() || room == NOWHERE || room >= self.world.borrow().len() as i16 {
            error!(
                "SYSERR: Illegal value(s) passed to obj_to_room. (Room #{}/{}, {})",
                room,
                self.world.borrow().len(),
                object.is_some()
            );
        } else {
            let object = object.unwrap();
            object.as_ref().set_in_room(room);
            *object.carried_by.borrow_mut() = None;
            if self.room_flagged(room, ROOM_HOUSE) {
                self.set_room_flags_bit(room, ROOM_HOUSE_CRASH)
            }
            self.world.borrow()[room as usize]
                .contents
                .borrow_mut()
                .push(object.clone());
        }
    }

    /* Take an object from a room */
    pub fn obj_from_room(&self, object: Option<&Rc<ObjData>>) {
        // struct obj_data *temp;

        if object.is_none() {
            error!("SYSERR: NULL object  passed to obj_from_room");
            return;
        }
        let object = object.unwrap();

        if object.in_room() == NOWHERE {
            error!(
                "SYSERR: obj not in a room ({}) passed to obj_from_room",
                object.in_room(),
            );
            return;
        }

        self.world.borrow()[object.in_room() as usize]
            .contents
            .borrow_mut()
            .retain(|x| !Rc::ptr_eq(x, &object));

        if self.room_flagged(object.in_room(), ROOM_HOUSE) {
            self.set_room_flags_bit(object.in_room(), ROOM_HOUSE_CRASH);
        }
    }

    /* put an object in an object (quaint)  */
    pub fn obj_to_obj(&self, obj: Option<&Rc<ObjData>>, obj_to: Option<&Rc<ObjData>>) {
        if obj.is_none() || obj_to.is_none() {
            error!("SYSERR: None obj passed to obj_to_obj.");
            return;
        }
        let obj = obj.unwrap();
        let obj_to = obj_to.unwrap();
        if Rc::ptr_eq(&obj, &obj_to) {
            error!("SYSERR: same source and target  obj passed to obj_to_obj.");
            return;
        }

        obj_to.contains.borrow_mut().push(obj.clone());
        *obj.in_obj.borrow_mut() = Some(obj_to.clone());

        let mut tmp_obj = obj.clone();
        loop {
            if tmp_obj.in_obj.borrow().is_none() {
                break;
            }

            tmp_obj.set_obj_weight(obj.get_obj_weight());
            let n = tmp_obj.in_obj.borrow().as_ref().unwrap().clone();
            tmp_obj = n.clone();
        }

        /* top level object.  Subtract weight from inventory if necessary. */
        tmp_obj.incr_obj_weight(obj.get_obj_weight());
        if tmp_obj.carried_by.borrow().is_some() {
            tmp_obj
                .carried_by
                .borrow()
                .as_ref()
                .unwrap()
                .incr_is_carrying_w(obj.get_obj_weight());
        }
    }

    /* remove an object from an object */
    pub(crate) fn obj_from_obj(obj: &Rc<ObjData>) {
        if obj.in_obj.borrow().is_none() {
            error!("SYSERR:  trying to illegally extract obj from obj.");
            return;
        }
        {
            let oio = obj.in_obj.borrow();
            let obj_from = oio.as_ref().unwrap();
            obj_from
                .contains
                .borrow_mut()
                .retain(|o| !Rc::ptr_eq(o, &obj));

            /* Subtract weight from containers container */

            let oio = obj.in_obj.borrow();
            let mut temp = oio.as_ref().unwrap().clone();
            loop {
                if temp.in_obj.borrow().is_none() {
                    break;
                }
                temp.incr_obj_weight(-obj.get_obj_weight());
                let n = temp.in_obj.borrow().as_ref().unwrap().clone();
                temp = n;
            }

            /* Subtract weight from char that carries the object */
            temp.incr_obj_weight(-obj.get_obj_weight());

            if temp.carried_by.borrow().is_some() {
                temp.carried_by
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .incr_is_carrying_w(-obj.get_obj_weight());
            }
        }

        *obj.in_obj.borrow_mut() = None;
    }
}
/* Set all carried_by to point to new owner */
pub fn object_list_new_owner(obj: &Rc<ObjData>, ch: Option<Rc<CharData>>) {
    for o in obj.contains.borrow().iter() {
        object_list_new_owner(o, ch.clone());
        *o.carried_by.borrow_mut() = ch.clone();
    }
}

impl DB {
    /* Extract an object from the world */
    pub fn extract_obj(&self, obj: &Rc<ObjData>) {
        let tch = obj.worn_by.borrow().clone();
        if tch.is_some() {
            if Rc::ptr_eq(
                self.unequip_char(tch.as_ref().unwrap(), obj.worn_on.get() as i8)
                    .as_ref()
                    .unwrap(),
                &obj,
            ) {
                error!("SYSERR: Inconsistent worn_by and worn_on pointers!!");
            }
        }

        if obj.in_room() != NOWHERE {
            self.obj_from_room(Some(obj));
        } else if obj.carried_by.borrow().is_some() {
            obj_from_char(Some(obj));
        } else if obj.in_obj.borrow().is_some() {
            DB::obj_from_obj(&obj);
        }
        /* Get rid of the contents of the object, as well. */
        let mut old_object_list = vec![];
        for o in obj.contains.borrow().iter() {
            old_object_list.push(o.clone());
        }
        for o in old_object_list.iter() {
            self.extract_obj(o);
        }

        self.object_list
            .borrow_mut()
            .retain(|o| !Rc::ptr_eq(&obj, o));

        if obj.get_obj_rnum() != NOTHING {
            self.obj_index[obj.get_obj_rnum() as usize]
                .number
                .set(self.obj_index[obj.get_obj_rnum() as usize].number.get() - 1);
        }
        //free_obj(obj);
    }
}

fn update_object_list(list: &Vec<Rc<ObjData>>, _use: i32) {
    for obj in list {
        update_object(obj, _use);
    }
}

fn update_object(obj: &ObjData, _use: i32) {
    if obj.get_obj_timer() > 0 {
        obj.decr_obj_timer(_use);
    }
    update_object_list(obj.contains.borrow().as_ref(), _use);
}

impl DB {
    pub(crate) fn update_char_objects(&self, ch: &Rc<CharData>) {
        let i;
        if ch.get_eq(WEAR_LIGHT as i8).is_some() {
            if ch.get_eq(WEAR_LIGHT as i8).as_ref().unwrap().get_obj_type() == ITEM_LIGHT {
                if ch.get_eq(WEAR_LIGHT as i8).as_ref().unwrap().get_obj_val(2) > 0 {
                    ch.get_eq(WEAR_LIGHT as i8)
                        .as_ref()
                        .unwrap()
                        .decr_obj_val(2);
                    i = ch.get_eq(WEAR_LIGHT as i8).as_ref().unwrap().get_obj_val(2);
                    if i == 1 {
                        send_to_char(ch, "Your light begins to flicker and fade.\r\n");
                        self.act(
                            "$n's light begins to flicker and fade.",
                            false,
                            Some(ch),
                            None,
                            None,
                            TO_ROOM,
                        );
                    } else if i == 0 {
                        send_to_char(ch, "Your light sputters out and dies.\r\n");
                        self.act(
                            "$n's light sputters out and dies.",
                            false,
                            Some(ch),
                            None,
                            None,
                            TO_ROOM,
                        );
                        self.world.borrow()[ch.in_room() as usize]
                            .light
                            .set(self.world.borrow()[ch.in_room() as usize].light.get() - 1);
                    }
                }
            }
        }
        for i in 0..NUM_WEARS {
            if ch.get_eq(i).is_some() {
                update_object(ch.get_eq(i).as_ref().unwrap(), 2);
            }
        }

        if !ch.carrying.borrow().is_empty() {
            update_object_list(ch.carrying.borrow().as_ref(), 2);
        }
    }

    /* Extract a ch completely from the world, and leave his stuff behind */
    pub(crate) fn extract_char_final(&self, ch: &Rc<CharData>, game: &Game) {
        if ch.in_room() == NOWHERE {
            error!(
                "SYSERR: NOWHERE extracting char {}. ( extract_char_final)",
                ch.get_name()
            );
            process::exit(1);
        }

        /*
         * We're booting the character of someone who has switched so first we
         * need to stuff them back into their own body.  This will set ch.desc
         * we're checking below this loop to the proper value.
         */
        if !ch.is_npc() && ch.desc.borrow().is_none() {
            for d in game.descriptor_list.borrow().iter() {
                if d.original.borrow().is_some()
                    && Rc::ptr_eq(d.original.borrow().as_ref().unwrap(), ch)
                {
                    do_return(game, d.character.borrow().as_ref().unwrap(), "", 0, 0);
                    break;
                }
            }
        }

        if ch.desc.borrow().is_some() {
            /*
             * This time we're extracting the body someone has switched into
             * (not the body of someone switching as above) so we need to put
             * the switcher back to their own body.
             *
             * If this body is not possessed, the owner won't have a
             * body after the removal so dump them to the main menu.
             */
            if ch
                .desc
                .borrow()
                .as_ref()
                .unwrap()
                .original
                .borrow()
                .is_some()
            {
                do_return(game, ch, "", 0, 0);
            } else {
                /*
                 * Now we boot anybody trying to log in with the same character, to
                 * help guard against duping.  CON_DISCONNECT is used to close a
                 * descriptor without extracting the d.character associated with it,
                 * for being link-dead, so we want CON_CLOSE to clean everything up.
                 * If we're here, we know it's a player so no IS_NPC check required.
                 */
                for d in game.descriptor_list.borrow().iter() {
                    if Rc::ptr_eq(d, ch.desc.borrow().as_ref().unwrap()) {
                        continue;
                    }

                    if d.character.borrow().is_some()
                        && ch.get_idnum() == d.character.borrow().as_ref().unwrap().get_idnum()
                    {
                        d.set_state(ConClose);
                    }
                }
                ch.desc.borrow().as_ref().unwrap().set_state(ConMenu);

                write_to_output(ch.desc.borrow().as_ref().unwrap(), MENU);
            }
        }

        /* On with the character's assets... */

        if ch.followers.borrow().len() != 0 || ch.master.borrow().is_some() {
            self.die_follower(ch);
        }

        /* transfer objects to room, if any */
        for obj in clone_vec(&ch.carrying) {
            obj_from_char(Some(&obj));
            self.obj_to_room(Some(&obj), ch.in_room());
        }

        /* transfer equipment to room, if any */
        for i in 0..NUM_WEARS {
            if ch.get_eq(i).is_some() {
                self.obj_to_room(self.unequip_char(ch, i).as_ref(), ch.in_room())
            }
        }

        if ch.fighting().is_some() {
            self.stop_fighting(ch);
        }

        let mut old_combat_list = vec![];
        for c in self.combat_list.borrow().iter() {
            old_combat_list.push(c.clone());
        }
        for k in old_combat_list.iter() {
            if Rc::ptr_eq(k.fighting().as_ref().unwrap(), ch) {
                self.stop_fighting(k);
            }
        }
        /* we can't forget the hunters either... */
        for temp in self.character_list.borrow().iter() {
            if temp.char_specials.borrow().hunting.is_some()
                && Rc::ptr_eq(temp.char_specials.borrow().hunting.as_ref().unwrap(), ch)
            {
                temp.char_specials.borrow_mut().hunting = None;
            }
        }
        self.char_from_room(ch);

        if ch.is_npc() {
            if ch.get_mob_rnum() != NOTHING {
                self.mob_index[ch.get_mob_rnum() as usize]
                    .number
                    .set(self.mob_index[ch.get_mob_rnum() as usize].number.get() - 1);
            }
            ch.clear_memory()
        } else {
            self.save_char(ch);
            crash_delete_crashfile(ch);
        }

        /* If there's a descriptor, they're in the menu now. */
        // if (IS_NPC(ch) || !ch . desc)
        // free_char(ch);
    }

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
    pub fn extract_char(&self, ch: &Rc<CharData>) {
        if ch.is_npc() {
            ch.set_mob_flags_bit(MOB_NOTDEADYET);
        } else {
            ch.set_plr_flag_bit(PLR_NOTDEADYET);
        }

        let n = self.extractions_pending.get();
        self.extractions_pending.set(n + 1);
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
impl DB {
    pub fn extract_pending_chars(&self, main_globals: &Game) {
        // struct char_data * vict, * next_vict, * prev_vict;

        if self.extractions_pending.get() < 0 {
            error!(
                "SYSERR: Negative ({}) extractions pending.",
                self.extractions_pending.get()
            );
        }

        for vict in self.character_list.borrow().iter() {
            if vict.mob_flagged(MOB_NOTDEADYET) {
                vict.remove_mob_flags_bit(MOB_NOTDEADYET);
            } else if vict.plr_flagged(PLR_NOTDEADYET) {
                vict.remove_plr_flag(PLR_NOTDEADYET);
            } else {
                /* Last non-free'd character to continue chain from. */
                continue;
            }

            self.extract_char_final(vict, main_globals);
            self.extractions_pending
                .set(self.extractions_pending.get() - 1);
        }

        if self.extractions_pending.get() > 0 {
            error!(
                "SYSERR: Couldn't find {} extractions as counted.",
                self.extractions_pending.get()
            );
        }

        self.extractions_pending.set(0);
    }
}

/* ***********************************************************************
* Here follows high-level versions of some earlier routines, ie functions*
* which incorporate the actual player-data                               *.
*********************************************************************** */
impl DB {
    pub fn get_player_vis(
        &self,
        ch: &Rc<CharData>,
        name: &mut String,
        number: Option<&mut i32>,
        inroom: i32,
    ) -> Option<Rc<CharData>> {
        let mut num = 0;
        let mut t: &mut i32;
        if number.is_none() {
            num = get_number(name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let mut number = t;

        for i in self.character_list.borrow().iter() {
            if i.is_npc() {
                continue;
            }
            if inroom == FIND_CHAR_ROOM && ch.in_room() != i.in_room() {
                continue;
            }
            if i.player.borrow().name != *name {
                continue;
            }
            if !self.can_see(ch, i) {
                continue;
            }
            *number -= 1;
            if *number != 0 {
                continue;
            }
            return Some(i.clone());
        }
        return None;
    }

    pub fn get_char_room_vis(
        &self,
        ch: &Rc<CharData>,
        name: &mut String,
        number: Option<&mut i32>,
    ) -> Option<Rc<CharData>> {
        let mut num = 0;
        let mut t: &mut i32;
        if number.is_none() {
            num = get_number(name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let mut number = t;

        /* JE 7/18/94 :-) :-) */
        if name == "self" || name == "me" {
            return Some(ch.clone());
        }

        /* 0.<name> means PC with name */
        if *number == 0 {
            return self.get_player_vis(ch, name, None, FIND_CHAR_ROOM);
        }

        for i in self.world.borrow()[ch.in_room() as usize]
            .peoples
            .borrow()
            .iter()
        {
            if isname(name, i.player.borrow().name.as_str()) {
                if self.can_see(ch, i) {
                    *number -= 1;
                    if *number == 0 {
                        return Some(i.clone());
                    }
                }
            }
        }
        return None;
    }

    pub fn get_char_world_vis(
        &self,
        ch: &Rc<CharData>,
        name: &mut String,
        number: Option<&mut i32>,
    ) -> Option<Rc<CharData>> {
        let mut num = 0;
        let mut t: &mut i32;
        if number.is_none() {
            num = get_number(name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let mut number: &mut i32 = t;

        let i = self.get_char_room_vis(ch, name, Some(number));
        if i.is_some() {
            return i;
        }

        /* 0.<name> means PC with name */
        if *number == 0 {
            return self.get_player_vis(ch, name, None, 0);
        }

        for i in self.character_list.borrow().iter() {
            if ch.in_room() == i.in_room() {
                continue;
            }
            if !isname(name, i.player.borrow().name.as_str()) {
                continue;
            }
            if !self.can_see(ch, i) {
                continue;
            }
            *number -= 1;
            if *number != 0 {
                continue;
            }
            return Some(i.clone());
        }
        return None;
    }

    pub fn get_char_vis(
        &self,
        ch: &Rc<CharData>,
        name: &mut String,
        number: Option<&mut i32>,
        _where: i32,
    ) -> Option<Rc<CharData>> {
        return if _where == FIND_CHAR_ROOM {
            self.get_char_room_vis(ch, name, number)
        } else if _where == FIND_CHAR_WORLD {
            self.get_char_world_vis(ch, name, number)
        } else {
            None
        };
    }

    pub fn get_obj_in_list_vis(
        &self,
        ch: &Rc<CharData>,
        name: &str,
        number: Option<&mut i32>,
        list: Ref<Vec<Rc<ObjData>>>,
    ) -> Option<Rc<ObjData>> {
        let mut i: Option<&Rc<ObjData>> = None;
        let mut num = 0;
        let mut t: &mut i32;
        let mut name = name.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let mut number: &mut i32 = t;
        if *number == 0 {
            return None;
        }

        for i in list.iter() {
            if isname(&name, &i.name.borrow()) {
                if self.can_see_obj(ch, i) {
                    *number -= 1;
                    if *number == 0 {
                        return Some(i.clone());
                    }
                }
            }
        }

        None
    }

    /* search the entire world for an object, and return a pointer  */
    pub fn get_obj_vis(
        &self,
        ch: &Rc<CharData>,
        name: &str,
        number: Option<&mut i32>,
    ) -> Option<Rc<ObjData>> {
        let mut num = 0;
        let mut t: &mut i32;
        let mut name = name.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let mut number: &mut i32 = t;
        if *number == 0 {
            return None;
        }

        /* scan items carried */
        let i = self.get_obj_in_list_vis(ch, &name, Some(number), ch.carrying.borrow());
        if i.is_some() {
            return i.clone();
        }

        /* scan room */
        let i = self.get_obj_in_list_vis(
            ch,
            &name,
            Some(number),
            self.world.borrow()[ch.in_room() as usize].contents.borrow(),
        );
        if i.is_some() {
            return i;
        }

        /* ok.. no luck yet. scan the entire obj list   */
        for i in self.object_list.borrow().iter() {
            if isname(&name, &i.name.borrow()) {
                if self.can_see_obj(ch, i) {
                    *number -= 1;
                    if *number == 0 {
                        return Some(i.clone());
                    }
                }
            }
        }
        None
    }
}
impl DB {
    pub fn get_obj_in_equip_vis(
        &self,
        ch: &Rc<CharData>,
        arg: &str,
        number: Option<&mut i32>,
        equipment: &RefCell<[Option<Rc<ObjData>>]>,
    ) -> Option<Rc<ObjData>> {
        let mut num = 0;
        let mut t: &mut i32;
        let mut name = arg.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let mut number: &mut i32 = t;
        if *number == 0 {
            return None;
        }
        let equipment = equipment.borrow();
        for j in 0..NUM_WEARS as usize {
            if equipment[j].is_some()
                && self.can_see_obj(ch, equipment[j].as_ref().unwrap())
                && isname(&arg, &equipment[j].borrow().as_ref().unwrap().name.borrow())
            {
                *number -= 1;
                if *number == 0 {
                    return equipment[j].clone();
                }
            }
        }

        None
    }

    pub fn get_obj_pos_in_equip_vis(
        &self,
        ch: &Rc<CharData>,
        arg: &str,
        number: Option<&mut i32>,
        equipment: &RefCell<[Option<Rc<ObjData>>]>,
    ) -> Option<i8> {
        let equipment = equipment.borrow();
        let mut num = 0;
        let mut t: &mut i32;
        let mut name = arg.to_string();
        if number.is_none() {
            num = get_number(&mut name);
            t = &mut num;
        } else {
            t = number.unwrap();
        }
        let mut number: &mut i32 = t;
        if *number == 0 {
            return None;
        }

        for j in 0..NUM_WEARS as usize {
            if equipment[j].is_some()
                && self.can_see_obj(ch, equipment[j].as_ref().unwrap())
                && isname(arg, &equipment[j].as_ref().unwrap().name.borrow())
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
    pub fn create_money(&self, amount: i32) -> Option<Rc<ObjData>> {
        if amount <= 0 {
            error!("SYSERR: Try to create negative or 0 money. ({})", amount);
            return None;
        }
        let mut obj = ObjData::new();
        let mut new_descr = ExtraDescrData::new();

        if amount == 1 {
            obj.name = RefCell::from("coin gold".to_string());
            obj.short_description = "a gold coin".to_string();
            obj.description = "One miserable gold coin is lying here.".to_string();
            new_descr.keyword = "coin gold".to_string();
            new_descr.description = "It's just one miserable little gold coin.".to_string();
        } else {
            obj.name = RefCell::from("coins gold".to_string());
            obj.short_description = money_desc(amount).to_string();
            obj.description = format!("{} is lying here.", money_desc(amount));

            new_descr.keyword = "coins gold".to_string();
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
            new_descr.description = buf;
        }

        obj.set_obj_type(ITEM_MONEY);
        obj.set_obj_wear(ITEM_WEAR_TAKE);
        obj.set_obj_val(0, amount);
        obj.set_obj_cost(amount);
        obj.item_number = NOTHING;

        obj.ex_descriptions.push(new_descr);
        let ret = Rc::from(obj);
        self.object_list.borrow_mut().push(ret.clone());

        Some(ret)
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
    pub fn generic_find(
        &self,
        arg: &str,
        bitvector: i64,
        ch: &Rc<CharData>,
        tar_ch: &mut Option<Rc<CharData>>,
        tar_obj: &mut Option<Rc<ObjData>>,
    ) -> i32 {
        // int i, found, number;
        // char name_val[MAX_INPUT_LENGTH];
        // char * name = name_val;
        //
        // *tar_ch = NULL;
        // * tar_obj = NULL;
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
            *tar_ch = self.get_char_room_vis(ch, &mut name, Some(&mut number));

            if tar_ch.is_some() {
                return FIND_CHAR_ROOM;
            }
        }

        if is_set!(bitvector, FIND_CHAR_WORLD as i64) {
            *tar_ch = self.get_char_world_vis(ch, &mut name, Some(&mut number));
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
                    && isname(name.as_str(), &ch.get_eq(i).as_ref().unwrap().name.borrow())
                {
                    number -= 1;
                    if number == 0 {
                        *tar_obj = ch.get_eq(i);
                        found = true;
                    }
                }
            }
            if found {
                return FIND_OBJ_EQUIP;
            }
        }

        if is_set!(bitvector, FIND_OBJ_INV as i64) {
            *tar_obj = self.get_obj_in_list_vis(ch, &name, Some(&mut number), ch.carrying.borrow());
            if tar_obj.is_some() {
                return FIND_OBJ_INV;
            }
        }

        if is_set!(bitvector, FIND_OBJ_ROOM as i64) {
            *tar_obj = self.get_obj_in_list_vis(
                ch,
                &name,
                Some(&mut number),
                self.world.borrow()[ch.in_room() as usize].contents.borrow(),
            );
            if tar_obj.is_some() {
                return FIND_OBJ_ROOM;
            }
        }

        if is_set!(bitvector, FIND_OBJ_WORLD as i64) {
            *tar_obj = self.get_obj_vis(ch, &name, Some(&mut number));
            if tar_obj.is_some() {
                return FIND_OBJ_WORLD;
            }
        }
        0
    }
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
