/* ************************************************************************
*   File: handler.c                                     Part of CircleMUD *
*  Usage: internal funcs: moving and finding chars/objs                   *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use crate::db::DB;
use crate::structs::{
    CharData, ObjData, ObjRnum, RoomRnum, APPLY_AC, APPLY_AGE, APPLY_CHA, APPLY_CHAR_HEIGHT,
    APPLY_CHAR_WEIGHT, APPLY_CLASS, APPLY_CON, APPLY_DAMROLL, APPLY_DEX, APPLY_EXP, APPLY_GOLD,
    APPLY_HIT, APPLY_HITROLL, APPLY_INT, APPLY_LEVEL, APPLY_MANA, APPLY_MOVE, APPLY_NONE,
    APPLY_SAVING_BREATH, APPLY_SAVING_PARA, APPLY_SAVING_PETRI, APPLY_SAVING_ROD,
    APPLY_SAVING_SPELL, APPLY_STR, APPLY_WIS, ITEM_ANTI_EVIL, ITEM_ANTI_GOOD, ITEM_ANTI_NEUTRAL,
    ITEM_ARMOR, ITEM_LIGHT, LVL_GRGOD, MAX_OBJ_AFFECT, MOB_NOTDEADYET, NOTHING, NOWHERE, NUM_WEARS,
    PLR_CRASH, PLR_NOTDEADYET, ROOM_HOUSE, ROOM_HOUSE_CRASH, WEAR_BODY, WEAR_HEAD, WEAR_LEGS,
    WEAR_LIGHT,
};
use log::error;
use std::cmp::{max, min};
use std::process;
use std::rc::Rc;

use crate::class::invalid_class;
use crate::config::MENU;
use crate::spells::{SAVING_BREATH, SAVING_PARA, SAVING_PETRI, SAVING_ROD, SAVING_SPELL};
use crate::structs::ConState::{ConClose, ConMenu};
use crate::util::SECS_PER_MUD_YEAR;
use crate::{send_to_char, write_to_output, MainGlobals, TO_CHAR, TO_ROOM};

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

// int isname(const char *str, const char *namelist)
// {
// const char *curname, *curstr;
//
// curname = namelist;
// for (;;) {
// for (curstr = str;; curstr++, curname++) {
// if (!*curstr && !isalpha(*curname))
// return (1);
//
// if (!*curname)
// return (0);
//
// if (!*curstr || *curname == ' ')
// break;
//
// if (LOWER(*curstr) != LOWER(*curname))
// break;
// }
//
// /* skip to next name */
//
// for (; isalpha(*curname); curname++);
// if (!*curname)
// return (0);
// curname++;			/* first char of new name */
// }
// }

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
            ch.set_save(
                SAVING_PARA as usize,
                ch.get_save(SAVING_PARA as usize) + _mod as i16,
            );
        }
        APPLY_SAVING_ROD => {
            ch.set_save(
                SAVING_ROD as usize,
                ch.get_save(SAVING_ROD as usize) + _mod as i16,
            );
        }
        APPLY_SAVING_PETRI => {
            ch.set_save(
                SAVING_PETRI as usize,
                ch.get_save(SAVING_PETRI as usize) + _mod as i16,
            );
        }

        APPLY_SAVING_BREATH => {
            ch.set_save(
                SAVING_BREATH as usize,
                ch.get_save(SAVING_BREATH as usize) + _mod as i16,
            );
        }

        APPLY_SAVING_SPELL => {
            ch.set_save(
                SAVING_SPELL as usize,
                ch.get_save(SAVING_SPELL as usize) + _mod,
            );
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
fn affect_total(ch: &CharData) {
    //struct affected_type *af;
    //int i, j;

    for i in 0..NUM_WEARS {
        if ch.get_eq(i).is_some() {
            let eq = ch.get_eq(i).unwrap();
            for j in 0..MAX_OBJ_AFFECT {
                affect_modify(
                    ch,
                    eq.affected[j as usize].location as i8,
                    eq.affected[j as usize].modifier as i16,
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
                    eq.affected[j as usize].location as i8,
                    eq.affected[j as usize].modifier as i16,
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

// /* Insert an affect_type in a char_data structure
//    Automatically sets apropriate bits and apply's */
// void affect_to_char(struct char_data *ch, struct affected_type *af)
// {
// struct affected_type *affected_alloc;
//
// CREATE(affected_alloc, struct affected_type, 1);
//
// *affected_alloc = *af;
// affected_alloc->next = ch->affected;
// ch->affected = affected_alloc;
//
// affect_modify(ch, af->location, af->modifier, af->bitvector, TRUE);
// affect_total(ch);
// }
//
//
//
// /*
//  * Remove an affected_type structure from a char (called when duration
//  * reaches zero). Pointer *af must never be NIL!  Frees mem and calls
//  * affect_location_apply
//  */
// void affect_remove(struct char_data *ch, struct affected_type *af)
// {
// struct affected_type *temp;
//
// if (ch->affected == NULL) {
// core_dump();
// return;
// }
//
// affect_modify(ch, af->location, af->modifier, af->bitvector, FALSE);
// REMOVE_FROM_LIST(af, ch->affected, next);
// free(af);
// affect_total(ch);
// }
//
//
//
// /* Call affect_remove with every spell of spelltype "skill" */
// void affect_from_char(struct char_data *ch, int type)
// {
// struct affected_type *hjp, *next;
//
// for (hjp = ch->affected; hjp; hjp = next) {
// next = hjp->next;
// if (hjp->type == type)
// affect_remove(ch, hjp);
// }
// }
//
//
//
// /*
//  * Return TRUE if a char is affected by a spell (SPELL_XXX),
//  * FALSE indicates not affected.
//  */
// bool affected_by_spell(struct char_data *ch, int type)
// {
// struct affected_type *hjp;
//
// for (hjp = ch->affected; hjp; hjp = hjp->next)
// if (hjp->type == type)
// return (TRUE);
//
// return (FALSE);
// }
//
//
//
// void affect_join(struct char_data *ch, struct affected_type *af,
// bool add_dur, bool avg_dur, bool add_mod, bool avg_mod)
// {
// struct affected_type *hjp, *next;
// bool found = FALSE;
//
// for (hjp = ch->affected; !found && hjp; hjp = next) {
// next = hjp->next;
//
// if ((hjp->type == af->type) && (hjp->location == af->location)) {
// if (add_dur)
// af->duration += hjp->duration;
// if (avg_dur)
// af->duration /= 2;
//
// if (add_mod)
// af->modifier += hjp->modifier;
// if (avg_mod)
// af->modifier /= 2;
//
// affect_remove(ch, hjp);
// affect_to_char(ch, af);
// found = TRUE;
// }
// }
// if (!found)
// affect_to_char(ch, af);
// }
impl DB {
    /* move a player out of a room */
    pub fn char_from_room(&self, rch: Rc<CharData>) {
        //struct char_data *temp;
        let ch = rch.as_ref();

        if ch.in_room() == NOWHERE {
            error!("SYSERR: NULL character or NOWHERE in char_from_room");
            process::exit(1);
        }

        // TODO implement fighting
        // if ch.fighting().is_some {
        //     stop_fighting(ch);
        // }

        // TODO implement objects
        // if (GET_EQ(ch, WEAR_LIGHT) != NULL)
        // if (GET_OBJ_TYPE(GET_EQ(ch, WEAR_LIGHT)) == ITEM_LIGHT)
        // if (GET_OBJ_VAL(GET_EQ(ch, WEAR_LIGHT), 2))	/* Light is ON */
        // world[IN_ROOM(ch)].light--;

        let w = self.world.borrow();
        let mut list = w[ch.in_room() as usize].peoples.borrow_mut();
        list.retain(|c_rch| !Rc::ptr_eq(c_rch, &rch));
    }

    /* place a character in a room */
    pub(crate) fn char_to_room(&self, ch: Option<Rc<CharData>>, room: RoomRnum) {
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

            // if (GET_EQ(ch, WEAR_LIGHT))
            // if (GET_OBJ_TYPE(GET_EQ(ch, WEAR_LIGHT)) == ITEM_LIGHT)
            // if (GET_OBJ_VAL(GET_EQ(ch, WEAR_LIGHT), 2))    /* Light ON */
            self.world.borrow()[room as usize]
                .light
                .replace(self.world.borrow()[room as usize].light.take() + 1);

            /* Stop fighting now, if we left. */
            // if (FIGHTING(ch) && IN_ROOM(ch) != IN_ROOM(FIGHTING(ch))) {
            //     stop_fighting(FIGHTING(ch));
            //     stop_fighting(ch);
            // }
        }
    }

    /* give an object to a char   */
    pub fn obj_to_char(object: Option<Rc<ObjData>>, ch: Option<Rc<CharData>>) {
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
pub fn obj_from_char(object: Option<Rc<ObjData>>) {
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

fn invalid_align(ch: &CharData, obj: &ObjData) -> bool {
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
    pub(crate) fn equip_char(&self, ch: Option<Rc<CharData>>, obj: Option<Rc<ObjData>>, pos: i8) {
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
                Some(ch.clone()),
                Some(obj.as_ref()),
                None,
                TO_CHAR,
            );
            self.act(
                "$n is zapped by $p and instantly lets go of it.",
                false,
                Some(ch.clone()),
                Some(obj.as_ref()),
                None,
                TO_ROOM,
            );
            /* Changed to drop in inventory instead of the ground. */
            DB::obj_to_char(Some(obj.clone()), Some(ch.clone()));
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
                obj.affected[j as usize].location as i8,
                obj.affected[j as usize].modifier as i16,
                obj.get_obj_affect(),
                true,
            );
        }

        affect_total(ch.as_ref());
    }

    pub fn unequip_char(&self, ch: Rc<CharData>, pos: i8) -> Option<Rc<ObjData>> {
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
                obj.affected[j as usize].location as i8,
                obj.affected[j as usize].modifier as i16,
                obj.get_obj_affect(),
                false,
            );
        }

        affect_total(ch.as_ref());

        Some(obj.clone())
    }
}

// int get_number(char **name)
// {
// int i;
// char *ppos;
// char number[MAX_INPUT_LENGTH];
//
// *number = '\0';
//
// if ((ppos = strchr(*name, '.')) != NULL) {
// *ppos++ = '\0';
// strlcpy(number, *name, sizeof(number));
// strcpy(*name, ppos);	/* strcpy: OK (always smaller) */
//
// for (i = 0; *(number + i); i++)
// if (!isdigit(*(number + i)))
// return (0);
//
// return (atoi(number));
// }
// return (1);
// }

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
// for (i = world[room].people; i && *number; i = i->next_in_room)
// if (isname(name, i->player.name))
// if (--(*number) == 0)
// return (i);
//
// return (NULL);
// }
//
//
//
// /* search all over the world for a char num, and return a pointer if found */
// struct char_data *get_char_num(mob_rnum nr)
// {
// struct char_data *i;
//
// for (i = character_list; i; i = i->next)
// if (GET_MOB_RNUM(i) == nr)
// return (i);
//
// return (NULL);
// }

impl DB {
    /* put an object in a room */
    pub fn obj_to_room(&self, object: Option<Rc<ObjData>>, room: RoomRnum) {
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
                .push(object);
        }
    }

    /* Take an object from a room */
    pub fn obj_from_room(&self, object: Option<Rc<ObjData>>) {
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
    pub fn obj_to_obj(&self, obj: Option<Rc<ObjData>>, obj_to: Option<Rc<ObjData>>) {
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
    pub(crate) fn obj_from_obj(obj: Rc<ObjData>) {
        if obj.in_obj.borrow().is_none() {
            error!("SYSERR:  trying to illegally extract obj from obj.");
            return;
        }
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

        *obj.in_obj.borrow_mut() = None;
    }
}
// /* Set all carried_by to point to new owner */
// void object_list_new_owner(struct obj_data *list, struct char_data *ch)
// {
// if (list) {
// object_list_new_owner(list->contains, ch);
// object_list_new_owner(list->next_content, ch);
// list->carried_by = ch;
// }
// }

impl DB {
    /* Extract an object from the world */
    pub fn extract_obj(&self, obj: Rc<ObjData>) {
        if obj.worn_by.borrow().is_some() {
            if Rc::ptr_eq(
                self.unequip_char(
                    obj.worn_by.borrow().as_ref().unwrap().clone(),
                    obj.worn_on.get() as i8,
                )
                .as_ref()
                .unwrap(),
                &obj,
            ) {
                error!("SYSERR: Inconsistent worn_by and worn_on pointers!!");
            }
        }

        if obj.in_room() != NOWHERE {
            self.obj_from_room(Some(obj.clone()));
        } else if obj.carried_by.borrow().is_some() {
            obj_from_char(Some(obj.clone()));
        } else if obj.in_obj.borrow().is_some() {
            DB::obj_from_obj(obj.clone());
        }
        /* Get rid of the contents of the object, as well. */
        for o in obj.contains.borrow().iter() {
            self.extract_obj(o.clone());
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
                            Some(ch.clone()),
                            None,
                            None,
                            TO_ROOM,
                        );
                    } else if i == 0 {
                        send_to_char(ch, "Your light sputters out and dies.\r\n");
                        self.act(
                            "$n's light sputters out and dies.",
                            false,
                            Some(ch.clone()),
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
    fn extract_char_final(&self, ch: &Rc<CharData>, main_globals: &MainGlobals) {
        if ch.in_room() == NOWHERE {
            error!(
                "SYSERR: NOWHERE extracting char {}. ( extract_char_final)",
                ch.get_name()
            );
            process::exit(1);
        }

        /*
         * We're booting the character of someone who has switched so first we
         * need to stuff them back into their own body.  This will set ch->desc
         * we're checking below this loop to the proper value.
         */
        if !ch.is_npc() && ch.desc.borrow().is_none() {
            // TODO implement do_return
            // for (d = descriptor_list; d; d = d->next)
            // if (d -> original == ch) {
            //     do_return(d->character, NULL, 0, 0);
            //     break;
            // }
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
                // TODO implement do_return
                //do_return(ch, NULL, 0, 0);
            } else {
                /*
                 * Now we boot anybody trying to log in with the same character, to
                 * help guard against duping.  CON_DISCONNECT is used to close a
                 * descriptor without extracting the d->character associated with it,
                 * for being link-dead, so we want CON_CLOSE to clean everything up.
                 * If we're here, we know it's a player so no IS_NPC check required.
                 */
                for d in main_globals.descriptor_list.borrow().iter() {
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
        for obj in ch.carrying.borrow().iter() {
            obj_from_char(Some(obj.clone()));
            self.obj_to_room(Some(obj.clone()), ch.in_room());
        }

        /* transfer equipment to room, if any */
        for i in 0..NUM_WEARS {
            if ch.get_eq(i).is_some() {
                self.obj_to_room(self.unequip_char(ch.clone(), i), ch.in_room())
            }
        }

        // TODO implement fighting
        // if (FIGHTING(ch))
        // stop_fighting(ch);

        // for (k = combat_list; k; k = temp) {
        //     temp = k -> next_fighting;
        //     if (FIGHTING(k) == ch)
        //     stop_fighting(k);
        // }
        /* we can't forget the hunters either... */
        // TODO implement hunting
        // for (temp = character_list; temp; temp = temp->next)
        // if (HUNTING(temp) == ch)
        // HUNTING(temp) = NULL;

        self.char_from_room(ch.clone());

        if ch.is_npc() {
            if ch.get_mob_rnum() != NOTHING {
                self.mob_index[ch.get_mob_rnum() as usize]
                    .number
                    .set(self.mob_index[ch.get_mob_rnum() as usize].number.get() - 1);
            }
            ch.clear_memory()
        } else {
            self.save_char(ch);
            // TODO implement crash delete
            // Crash_delete_crashfile(ch);
        }

        /* If there's a descriptor, they're in the menu now. */
        // if (IS_NPC(ch) || !ch -> desc)
        // free_char(ch);
    }

    /*
     * Q: Why do we do this?
     * A: Because trying to iterate over the character
     *    list with 'ch = ch->next' does bad things if
     *    the current character happens to die. The
     *    trivial workaround of 'vict = next_vict'
     *    doesn't work if the _next_ person in the list
     *    gets killed, for example, by an area spell.
     *
     * Q: Why do we leave them on the character_list?
     * A: Because code doing 'vict = vict->next' would
     *    get really confused otherwise.
     */
    pub fn extract_char(&self, ch: Rc<CharData>) {
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
 * would change the '->next' pointer, potentially
 * confusing some code. Ugh. -gg 3/15/2001
 *
 * NOTE: This doesn't handle recursive extractions.
 */
impl DB {
    pub fn extract_pending_chars(&self, main_globals: &MainGlobals) {
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

// /* ***********************************************************************
// * Here follows high-level versions of some earlier routines, ie functions*
// * which incorporate the actual player-data                               *.
// *********************************************************************** */
//
//
// struct char_data *get_player_vis(struct char_data *ch, char *name, int *number, int inroom)
// {
// struct char_data *i;
// int num;
//
// if (!number) {
// number = &num;
// num = get_number(&name);
// }
//
// for (i = character_list; i; i = i->next) {
// if (IS_NPC(i))
// continue;
// if (inroom == FIND_CHAR_ROOM && IN_ROOM(i) != IN_ROOM(ch))
// continue;
// if (str_cmp(i->player.name, name)) /* If not same, continue */
// continue;
// if (!CAN_SEE(ch, i))
// continue;
// if (--(*number) != 0)
// continue;
// return (i);
// }
//
// return (NULL);
// }
//
//
// struct char_data *get_char_room_vis(struct char_data *ch, char *name, int *number)
// {
// struct char_data *i;
// int num;
//
// if (!number) {
// number = &num;
// num = get_number(&name);
// }
//
// /* JE 7/18/94 :-) :-) */
// if (!str_cmp(name, "self") || !str_cmp(name, "me"))
// return (ch);
//
// /* 0.<name> means PC with name */
// if (*number == 0)
// return (get_player_vis(ch, name, NULL, FIND_CHAR_ROOM));
//
// for (i = world[IN_ROOM(ch)].people; i && *number; i = i->next_in_room)
// if (isname(name, i->player.name))
// if (CAN_SEE(ch, i))
// if (--(*number) == 0)
// return (i);
//
// return (NULL);
// }
//
//
// struct char_data *get_char_world_vis(struct char_data *ch, char *name, int *number)
// {
// struct char_data *i;
// int num;
//
// if (!number) {
// number = &num;
// num = get_number(&name);
// }
//
// if ((i = get_char_room_vis(ch, name, number)) != NULL)
// return (i);
//
// if (*number == 0)
// return get_player_vis(ch, name, NULL, 0);
//
// for (i = character_list; i && *number; i = i->next) {
// if (IN_ROOM(ch) == IN_ROOM(i))
// continue;
// if (!isname(name, i->player.name))
// continue;
// if (!CAN_SEE(ch, i))
// continue;
// if (--(*number) != 0)
// continue;
//
// return (i);
// }
// return (NULL);
// }
//
//
// struct char_data *get_char_vis(struct char_data *ch, char *name, int *number, int where)
// {
// if (where == FIND_CHAR_ROOM)
// return get_char_room_vis(ch, name, number);
// else if (where == FIND_CHAR_WORLD)
// return get_char_world_vis(ch, name, number);
// else
// return (NULL);
// }
//
//
// struct obj_data *get_obj_in_list_vis(struct char_data *ch, char *name, int *number, struct obj_data *list)
// {
// struct obj_data *i;
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
// for (i = list; i && *number; i = i->next_content)
// if (isname(name, i->name))
// if (CAN_SEE_OBJ(ch, i))
// if (--(*number) == 0)
// return (i);
//
// return (NULL);
// }
//
//
// /* search the entire world for an object, and return a pointer  */
// struct obj_data *get_obj_vis(struct char_data *ch, char *name, int *number)
// {
// struct obj_data *i;
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
// /* scan items carried */
// if ((i = get_obj_in_list_vis(ch, name, number, ch->carrying)) != NULL)
// return (i);
//
// /* scan room */
// if ((i = get_obj_in_list_vis(ch, name, number, world[IN_ROOM(ch)].contents)) != NULL)
// return (i);
//
// /* ok.. no luck yet. scan the entire obj list   */
// for (i = object_list; i && *number; i = i->next)
// if (isname(name, i->name))
// if (CAN_SEE_OBJ(ch, i))
// if (--(*number) == 0)
// return (i);
//
// return (NULL);
// }
//
//
// struct obj_data *get_obj_in_equip_vis(struct char_data *ch, char *arg, int *number, struct obj_data *equipment[])
// {
// int j, num;
//
// if (!number) {
// number = &num;
// num = get_number(&arg);
// }
//
// if (*number == 0)
// return (NULL);
//
// for (j = 0; j < NUM_WEARS; j++)
// if (equipment[j] && CAN_SEE_OBJ(ch, equipment[j]) && isname(arg, equipment[j]->name))
// if (--(*number) == 0)
// return (equipment[j]);
//
// return (NULL);
// }
//
//
// int get_obj_pos_in_equip_vis(struct char_data *ch, char *arg, int *number, struct obj_data *equipment[])
// {
// int j, num;
//
// if (!number) {
// number = &num;
// num = get_number(&arg);
// }
//
// if (*number == 0)
// return (-1);
//
// for (j = 0; j < NUM_WEARS; j++)
// if (equipment[j] && CAN_SEE_OBJ(ch, equipment[j]) && isname(arg, equipment[j]->name))
// if (--(*number) == 0)
// return (j);
//
// return (-1);
// }
//
//
// const char *money_desc(int amount)
// {
// int cnt;
// struct {
// int limit;
// const char *description;
// } money_table[] = {
// {          1, "a gold coin"				},
// {         10, "a tiny pile of gold coins"		},
// {         20, "a handful of gold coins"		},
// {         75, "a little pile of gold coins"		},
// {        200, "a small pile of gold coins"		},
// {       1000, "a pile of gold coins"		},
// {       5000, "a big pile of gold coins"		},
// {      10000, "a large heap of gold coins"		},
// {      20000, "a huge mound of gold coins"		},
// {      75000, "an enormous mound of gold coins"	},
// {     150000, "a small mountain of gold coins"	},
// {     250000, "a mountain of gold coins"		},
// {     500000, "a huge mountain of gold coins"	},
// {    1000000, "an enormous mountain of gold coins"	},
// {          0, NULL					},
// };
//
// if (amount <= 0) {
// log("SYSERR: Try to create negative or 0 money (%d).", amount);
// return (NULL);
// }
//
// for (cnt = 0; money_table[cnt].limit; cnt++)
// if (amount <= money_table[cnt].limit)
// return (money_table[cnt].description);
//
// return ("an absolutely colossal mountain of gold coins");
// }
//
//
// struct obj_data *create_money(int amount)
// {
// struct obj_data *obj;
// struct extra_descr_data *new_descr;
// char buf[200];
//
// if (amount <= 0) {
// log("SYSERR: Try to create negative or 0 money. (%d)", amount);
// return (NULL);
// }
// obj = create_obj();
// CREATE(new_descr, struct extra_descr_data, 1);
//
// if (amount == 1) {
// obj->name = strdup("coin gold");
// obj->short_description = strdup("a gold coin");
// obj->description = strdup("One miserable gold coin is lying here.");
// new_descr->keyword = strdup("coin gold");
// new_descr->description = strdup("It's just one miserable little gold coin.");
// } else {
// obj->name = strdup("coins gold");
// obj->short_description = strdup(money_desc(amount));
// snprintf(buf, sizeof(buf), "%s is lying here.", money_desc(amount));
// obj->description = strdup(CAP(buf));
//
// new_descr->keyword = strdup("coins gold");
// if (amount < 10)
// snprintf(buf, sizeof(buf), "There are %d coins.", amount);
// else if (amount < 100)
// snprintf(buf, sizeof(buf), "There are about %d coins.", 10 * (amount / 10));
// else if (amount < 1000)
// snprintf(buf, sizeof(buf), "It looks to be about %d coins.", 100 * (amount / 100));
// else if (amount < 100000)
// snprintf(buf, sizeof(buf), "You guess there are, maybe, %d coins.",
// 1000 * ((amount / 1000) + rand_number(0, (amount / 1000))));
// else
// strcpy(buf, "There are a LOT of coins.");	/* strcpy: OK (is < 200) */
// new_descr->description = strdup(buf);
// }
//
// new_descr->next = NULL;
// obj->ex_description = new_descr;
//
// GET_OBJ_TYPE(obj) = ITEM_MONEY;
// GET_OBJ_WEAR(obj) = ITEM_WEAR_TAKE;
// GET_OBJ_VAL(obj, 0) = amount;
// GET_OBJ_COST(obj) = amount;
// obj->item_number = NOTHING;
//
// return (obj);
// }
//
//
// /* Generic Find, designed to find any object/character
//  *
//  * Calling:
//  *  *arg     is the pointer containing the string to be searched for.
//  *           This string doesn't have to be a single word, the routine
//  *           extracts the next word itself.
//  *  bitv..   All those bits that you want to "search through".
//  *           Bit found will be result of the function
//  *  *ch      This is the person that is trying to "find"
//  *  **tar_ch Will be NULL if no character was found, otherwise points
//  * **tar_obj Will be NULL if no object was found, otherwise points
//  *
//  * The routine used to return a pointer to the next word in *arg (just
//  * like the one_argument routine), but now it returns an integer that
//  * describes what it filled in.
//  */
// int generic_find(char *arg, bitvector_t bitvector, struct char_data *ch,
// struct char_data **tar_ch, struct obj_data **tar_obj)
// {
// int i, found, number;
// char name_val[MAX_INPUT_LENGTH];
// char *name = name_val;
//
// *tar_ch = NULL;
// *tar_obj = NULL;
//
// one_argument(arg, name);
//
// if (!*name)
// return (0);
// if (!(number = get_number(&name)))
// return (0);
//
// if (IS_SET(bitvector, FIND_CHAR_ROOM)) {	/* Find person in room */
// if ((*tar_ch = get_char_room_vis(ch, name, &number)) != NULL)
// return (FIND_CHAR_ROOM);
// }
//
// if (IS_SET(bitvector, FIND_CHAR_WORLD)) {
// if ((*tar_ch = get_char_world_vis(ch, name, &number)) != NULL)
// return (FIND_CHAR_WORLD);
// }
//
// if (IS_SET(bitvector, FIND_OBJ_EQUIP)) {
// for (found = FALSE, i = 0; i < NUM_WEARS && !found; i++)
// if (GET_EQ(ch, i) && isname(name, GET_EQ(ch, i)->name) && --number == 0) {
// *tar_obj = GET_EQ(ch, i);
// found = TRUE;
// }
// if (found)
// return (FIND_OBJ_EQUIP);
// }
//
// if (IS_SET(bitvector, FIND_OBJ_INV)) {
// if ((*tar_obj = get_obj_in_list_vis(ch, name, &number, ch->carrying)) != NULL)
// return (FIND_OBJ_INV);
// }
//
// if (IS_SET(bitvector, FIND_OBJ_ROOM)) {
// if ((*tar_obj = get_obj_in_list_vis(ch, name, &number, world[IN_ROOM(ch)].contents)) != NULL)
// return (FIND_OBJ_ROOM);
// }
//
// if (IS_SET(bitvector, FIND_OBJ_WORLD)) {
// if ((*tar_obj = get_obj_vis(ch, name, &number)))
// return (FIND_OBJ_WORLD);
// }
//
// return (0);
// }
//
//
// /* a function to scan for "all" or "all.x" */
// int find_all_dots(char *arg)
// {
// if (!strcmp(arg, "all"))
// return (FIND_ALL);
// else if (!strncmp(arg, "all.", 4)) {
// strcpy(arg, arg + 4);	/* strcpy: OK (always less) */
// return (FIND_ALLDOT);
// } else
// return (FIND_INDIV);
// }
