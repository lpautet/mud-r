/* ************************************************************************
*   File: utils.c                                       Part of CircleMUD *
*  Usage: various internal functions of a utility nature                  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

/* defines for mudlog() */
pub const OFF: u8 = 0;
pub const BRF: u8 = 1;
pub const NRM: u8 = 2;
pub const CMP: u8 = 3;

#[macro_export]
macro_rules! is_set {
    ($flag:expr, $bit:expr) => {
        (($flag & $bit) != 0)
    };
}

#[macro_export]
macro_rules! set_bit {
    ($var:expr, $bit:expr) => {
        (($var) |= ($bit))
    };
}

#[macro_export]
macro_rules! remove_bit {
    ($var:expr, $bit:expr) => {
        (($var) &= !($bit))
    };
}

#[macro_export]
macro_rules! toggle_bit {
    ($var:expr, $bit:expr) => {
        (($var) ^= ($bit))
    };
}

// #[macro_export]
// macro_rules! is_npc {
//     ($ch:expr) => {{
//         (is_set!(mob_flags!($ch), MOB_ISNPC))
//     }};
// }
impl CharData {
    pub fn is_npc(&self) -> bool {
        return is_set!(self.char_specials.borrow().saved.act, MOB_ISNPC);
    }
}

// #[macro_export]
// macro_rules! prf_flagged {
//     ($ch:expr,$flag:expr) => {
//         (is_set!(prf_flags!($ch), ($flag)))
//     };
// }
impl CharData {
    pub fn prf_flagged(&self, flag: i64) -> bool {
        return is_set!(self.prf_flags(), flag);
    }
    pub fn prf_flags(&self) -> i64 {
        check_player_special!(self, self.player_specials.borrow().saved.pref)
    }
    pub fn set_prf_flags_bits(&self, flag: i64) {
        self.player_specials.borrow_mut().saved.pref |= flag;
    }
}

// #[macro_export]
// macro_rules! prf_flags {
//     ($ch:expr) => {
//         (check_player_special!(($ch), RefCell::borrow(&(($ch).player_specials)).saved.pref))
//     };
// }

/* TODO:
 * Accessing player specific data structures on a mobile is a very bad thing
 * to do.  Consider that changing these variables for a single mob will change
 * it for every other single mob in the game.  If we didn't specifically check
 * for it, 'wimpy' would be an extremely bad thing for a mob to do, as an
 * example.  If you really couldn't care less, change this to a '#if 0'.
 */
#[macro_export]
macro_rules! check_player_special {
    ($ch:expr,$var:expr) => {
        ($var)
    };
}
use crate::structs::{obj_vnum, room_rnum, MobRnum, RoomRnum, ITEM_INVISIBLE, NOTHING};
pub use check_player_special;
use std::borrow::Borrow;

impl CharData {
    pub fn get_invis_lev(&self) -> i16 {
        check_player_special!(self, self.player_specials.borrow().saved.invis_level)
    }
    pub fn set_invis_lev(&self, val: i16) {
        self.player_specials.borrow_mut().saved.invis_level = val;
    }
    pub fn get_hit(&self) -> i16 {
        self.points.borrow().hit
    }
    pub fn get_mana(&self) -> i16 {
        self.points.borrow().mana
    }
    pub fn get_move(&self) -> i16 {
        self.points.borrow().movem
    }
    pub fn incr_move(&self, val: i16) {
        self.points.borrow_mut().movem += val;
    }
    pub fn set_move(&self, val: i16) {
        self.points.borrow_mut().movem = val;
    }
    pub fn set_mana(&self, val: i16) {
        self.points.borrow_mut().mana = val;
    }
    pub fn set_hit(&self, val: i16) {
        self.points.borrow_mut().hit = val;
    }
}

impl DB {
    pub fn valid_room_rnum(&self, rnum: room_rnum) -> bool {
        rnum != NOWHERE && rnum < self.world.borrow().len() as i16
    }
    pub fn get_room_vnum(&self, rnum: room_vnum) -> i16 {
        if self.valid_room_rnum(rnum) {
            self.world.borrow()[rnum as usize].number
        } else {
            NOWHERE
        }
    }
}

#[macro_export]
macro_rules! get_room_spec {
    ($db:expr, $room:expr) => {
        (if valid_room_rnum!($room) {
            (RefCell::borrow(($db).world.get($rnum).unwrap()).func)
        } else {
            None
        })
    };
}

// #[macro_export]
// macro_rules! get_pc_name {
//     ($ch:expr) => {
//         (($ch).player.name.as_str())
//     };
// }

impl CharData {
    pub fn get_pc_name(&self) -> Rc<str> {
        return Rc::from(self.player.borrow().name.as_str());
    }
}

// #[macro_export]
// macro_rules! get_name {
//     ($ch:expr) => {
//         (if is_npc!($ch) {
//             ($ch).player.short_descr.as_str()
//         } else {
//             get_pc_name!($ch)
//         })
//     };
// }

impl CharData {
    pub fn get_name(&self) -> Rc<str> {
        if self.is_npc() {
            Rc::from(self.player.borrow().short_descr.as_str())
        } else {
            self.get_pc_name()
        }
    }
    pub fn get_title(&self) -> Rc<str> {
        Rc::from(self.player.borrow().title.as_ref().unwrap().as_str())
    }
    pub fn set_title(&self, val: Option<String>) {
        self.player.borrow_mut().title = val;
    }
}

#[macro_export]
macro_rules! is_print {
    ($c:expr) => {
        (($c) > 31 && ($c) != 127)
    };
}

#[macro_export]
macro_rules! isnewl {
    ($ch:expr) => {
        (($ch) == '\n' || ($ch) == '\r')
    };
}

// #[macro_export]
// macro_rules! get_wait_state {
//     ($ch:expr) => {
//         (($ch).wait)
//     };
// }
impl DescriptorData {
    pub fn state(&self) -> ConState {
        self.connected.get()
    }
    pub fn set_state(&self, val: ConState) {
        self.connected.set(val);
    }
}

impl CharData {
    pub(crate) fn get_wait_state(&self) -> i32 {
        return self.wait.get();
    }
    pub(crate) fn decr_wait_state(&self, val: i32) {
        self.wait.set(self.wait.get() - val);
    }
    pub(crate) fn set_wait_state(&self, val: i32) {
        self.wait.set(val);
    }
    pub fn get_class(&self) -> i8 {
        self.player.borrow().chclass
    }
    pub fn set_class(&self, val: i8) {
        self.player.borrow_mut().chclass = val;
    }
    pub fn get_pfilepos(&self) -> i32 {
        *self.pfilepos.borrow()
    }
    pub fn set_pfilepos(&self, val: i32) {
        *self.pfilepos.borrow_mut() = val;
    }
    pub fn get_level(&self) -> u8 {
        self.player.borrow().level
    }
    pub fn set_level(&self, val: u8) {
        self.player.borrow_mut().level = val;
    }
    pub fn get_passwd(&self) -> [u8; 16] {
        self.player.borrow().passwd
    }
    pub fn set_passwd(&self, val: [u8; 16]) {
        self.player.borrow_mut().passwd = val;
    }
    pub fn get_exp(&self) -> i32 {
        self.points.borrow().exp
    }
    pub fn set_exp(&self, val: i32) {
        self.points.borrow_mut().exp = val;
    }
    pub fn set_gold(&self, val: i32) {
        self.points.borrow_mut().gold = val
    }
    pub fn get_gold(&self) -> i32 {
        self.points.borrow().gold
    }
    pub fn get_max_move(&self) -> i16 {
        self.points.borrow().max_move
    }
    pub fn get_max_mana(&self) -> i16 {
        self.points.borrow().max_mana
    }
    pub fn get_hitroll(&self) -> i8 {
        self.points.borrow().hitroll
    }
    pub fn set_damroll(&self, val: i8) {
        self.points.borrow_mut().damroll = val;
    }
    pub fn get_damroll(&self) -> i8 {
        self.points.borrow().damroll
    }
    pub fn set_hitroll(&self, val: i8) {
        self.points.borrow_mut().hitroll = val;
    }
    pub fn get_max_hit(&self) -> i16 {
        self.points.borrow().max_hit
    }
    pub fn set_max_hit(&self, val: i16) {
        self.points.borrow_mut().max_hit = val;
    }
    pub fn incr_max_hit(&self, val: i16) {
        self.points.borrow_mut().max_hit += val;
    }
    pub fn set_max_mana(&self, val: i16) {
        self.points.borrow_mut().max_mana = val;
    }
    pub fn incr_max_mana(&self, val: i16) {
        self.points.borrow_mut().max_mana += val;
    }
    pub fn set_max_move(&self, val: i16) {
        self.points.borrow_mut().max_move = val;
    }
    pub fn incr_max_move(&self, val: i16) {
        self.points.borrow_mut().max_move += val;
    }
    pub fn get_home(&self) -> i16 {
        self.player.borrow().hometown
    }
    pub fn set_home(&self, val: i16) {
        self.player.borrow_mut().hometown = val;
    }
    pub fn get_ac(&self) -> i16 {
        self.points.borrow().armor
    }
    pub fn set_ac(&self, val: i16) {
        self.points.borrow_mut().armor = val;
    }
    pub fn in_room(&self) -> room_rnum {
        self.in_room.get()
    }
    pub fn set_in_room(&self, val: room_rnum) {
        self.in_room.set(val);
    }
    pub fn get_was_in(&self) -> room_rnum {
        self.was_in_room.get()
    }
    pub fn set_was_in(&self, val: RoomRnum) {
        self.was_in_room.set(val)
    }
}

#[macro_export]
macro_rules! get_age {
    ($ch:expr) => {
        (($ch).year)
    };
}

#[macro_export]
macro_rules! get_talk {
    ($ch:expr, $i:expr) => {
        (check_player_special!(
            ($ch),
            RefCell::borrow(&($ch).player_specials).saved.talks[($i)]
        ))
    };
}

// #[macro_export]
// macro_rules! get_talk_mut {
//     ($ch:expr, $i:expr) => {
//         (check_player_special!(
//             ($ch),
//             RefCell::borrow_mut(&($ch).player_specials).saved.talks[($i)]
//         ))
//     };
// }

impl CharData {
    pub fn get_talk_mut(&self, i: usize) -> bool {
        check_player_special!(self, self.player_specials.borrow().saved.talks[i])
    }
    pub fn set_talk_mut(&self, i: usize, val: bool) {
        self.player_specials.borrow_mut().saved.talks[i] = val;
    }
    pub fn get_mob_rnum(&self) -> MobRnum {
        self.nr
    }
    pub fn set_mob_rnum(&mut self, val: MobRnum) {
        self.nr = val;
    }
    pub fn get_cond(&self, i: i32) -> i16 {
        self.player_specials.borrow().saved.conditions[i as usize]
    }
    pub fn set_cond(&self, i: i32, val: i16) {
        self.player_specials.borrow_mut().saved.conditions[i as usize] = val;
    }
    pub fn get_loadroom(&self) -> room_vnum {
        self.player_specials.borrow().saved.load_room
    }
    pub fn set_loadroom(&self, val: room_vnum) {
        self.player_specials.borrow_mut().saved.load_room = val;
    }
    pub fn get_practices(&self) -> i32 {
        self.player_specials.borrow().saved.spells_to_learn
    }
    pub fn set_practices(&self, val: i32) {
        self.player_specials.borrow_mut().saved.spells_to_learn = val;
    }
    pub fn incr_practices(&self, val: i32) {
        self.player_specials.borrow_mut().saved.spells_to_learn += val;
    }
    pub fn get_bad_pws(&self) -> u8 {
        self.player_specials.borrow().saved.bad_pws
    }
    pub fn reset_bad_pws(&self) {
        self.player_specials.borrow_mut().saved.bad_pws = 0;
    }
    pub fn incr_bad_pws(&self) {
        self.player_specials.borrow_mut().saved.bad_pws += 1;
    }
}

#[macro_export]
macro_rules! get_last_tell {
    ($ch:expr) => {
        (check_player_special!(($ch), RefCell::borrow(&($ch).player_specials).last_tell))
    };
}

#[macro_export]
macro_rules! get_last_tell_mut {
    ($ch:expr) => {
        (check_player_special!(($ch), RefCell::borrow_mut(&($ch).player_specials).last_tell))
    };
}

#[macro_export]
macro_rules! set_skill {
    ($ch:expr, $i:expr, $pct:expr) => {{
        check_player_special!(
            ($ch),
            RefCell::borrow_mut(&($ch).player_specials).saved.skills[$i as usize]
        ) = $pct;
    }};
}

impl CharData {
    pub fn set_skill(&self, i: usize, pct: i8) {
        self.player_specials.borrow_mut().saved.skills[i as usize] = pct;
    }
    pub fn get_sex(&self) -> u8 {
        self.player.borrow().sex
    }
    pub fn set_sex(&self, val: u8) {
        self.player.borrow_mut().sex = val;
    }
    pub fn get_str(&self) -> i8 {
        self.aff_abils.borrow().str
    }
    pub fn set_str(&self, val: i8) {
        self.aff_abils.borrow_mut().str = val;
    }
    pub fn incr_str(&self, val: i8) {
        self.aff_abils.borrow_mut().str += val;
    }
    pub fn incr_dex(&self, val: i8) {
        self.aff_abils.borrow_mut().dex += val;
    }
    pub fn incr_int(&self, val: i8) {
        self.aff_abils.borrow_mut().intel += val;
    }
    pub fn incr_wis(&self, val: i8) {
        self.aff_abils.borrow_mut().wis += val;
    }
    pub fn incr_con(&self, val: i8) {
        self.aff_abils.borrow_mut().con += val;
    }
    pub fn incr_cha(&self, val: i8) {
        self.aff_abils.borrow_mut().cha += val;
    }
    pub fn get_add(&self) -> i8 {
        self.aff_abils.borrow().str_add
    }
    pub fn set_add(&self, val: i8) {
        self.aff_abils.borrow_mut().str_add = val;
    }
    pub fn get_dex(&self) -> i8 {
        self.aff_abils.borrow().dex
    }
    pub fn set_dex(&self, val: i8) {
        self.aff_abils.borrow_mut().dex = val;
    }
    pub fn get_int(&self) -> i8 {
        self.aff_abils.borrow().intel
    }
    pub fn set_int(&self, val: i8) {
        self.aff_abils.borrow_mut().intel = val;
    }
    pub fn get_wis(&self) -> i8 {
        self.aff_abils.borrow().wis
    }
    pub fn set_wis(&self, val: i8) {
        self.aff_abils.borrow_mut().wis = val;
    }
    pub fn get_con(&self) -> i8 {
        self.aff_abils.borrow().con
    }
    pub fn set_con(&self, val: i8) {
        self.aff_abils.borrow_mut().con = val;
    }
    pub fn get_cha(&self) -> i8 {
        self.aff_abils.borrow().cha
    }
    pub fn set_cha(&self, val: i8) {
        self.aff_abils.borrow_mut().cha = val;
    }
    pub fn get_pos(&self) -> u8 {
        self.char_specials.borrow().position
    }
    pub fn set_pos(&self, val: u8) {
        self.char_specials.borrow_mut().position = val;
    }
    pub fn get_idnum(&self) -> i64 {
        self.char_specials.borrow().saved.idnum
    }
    pub fn set_idnum(&self, val: i64) {
        self.char_specials.borrow_mut().saved.idnum = val;
    }
    pub fn fighting(&self) -> Option<Rc<CharData>> {
        if self.char_specials.borrow().fighting.is_none() {
            None
        } else {
            Some(
                self.char_specials
                    .borrow()
                    .fighting
                    .as_ref()
                    .unwrap()
                    .clone(),
            )
        }
    }
    pub fn set_fighting(&self, val: Option<Rc<CharData>>) {
        self.char_specials.borrow_mut().fighting = val;
    }
    pub fn get_alignment(&self) -> i32 {
        self.char_specials.borrow().saved.alignment
    }
    pub fn set_alignment(&self, val: i32) {
        self.char_specials.borrow_mut().saved.alignment = val;
    }
    pub fn aff_flagged(&self, flag: i64) -> bool {
        is_set!(self.aff_flags(), flag)
    }
    pub fn get_weight(&self) -> u8 {
        self.player.borrow().weight
    }
    pub fn set_weight(&self, val: u8) {
        self.player.borrow_mut().weight = val;
    }
    pub fn get_height(&self) -> u8 {
        self.player.borrow().height
    }
    pub fn set_height(&self, val: u8) {
        self.player.borrow_mut().height = val;
    }
    pub fn get_save(&self, i: usize) -> i16 {
        self.char_specials.borrow().saved.apply_saving_throw[i]
    }
    pub fn set_save(&self, i: usize, val: i16) {
        self.char_specials.borrow_mut().saved.apply_saving_throw[i] = val;
    }
    pub fn plr_flagged(&self, flag: i64) -> bool {
        !self.is_npc() && is_set!(self.plr_flags(), flag)
    }
    pub fn mob_flagged(&self, flag: i64) -> bool {
        self.is_npc() && is_set!(self.mob_flags(), flag)
    }
    pub fn plr_flags(&self) -> i64 {
        self.char_specials.borrow().saved.act
    }
    pub fn remove_plr_flag(&self, flag: i64) {
        self.char_specials.borrow_mut().saved.act &= !flag;
    }
    pub fn set_plr_flag_bit(&self, flag: i64) {
        self.char_specials.borrow_mut().saved.act |= flag;
    }
    pub fn mob_flags(&self) -> i64 {
        self.char_specials.borrow().saved.act
    }
    pub fn remove_mob_flags_bit(&self, flag: i64) {
        self.char_specials.borrow_mut().saved.act &= !flag;
    }
    pub fn set_mob_flags(&self, flags: i64) {
        self.char_specials.borrow_mut().saved.act = flags;
    }
    pub fn set_mob_flags_bit(&self, flag: i64) {
        self.char_specials.borrow_mut().saved.act |= flag;
    }

    pub fn get_default_pos(&self) -> u8 {
        self.mob_specials.default_pos
    }
    pub fn set_default_pos(&mut self, val: u8) {
        self.mob_specials.default_pos = val;
    }
}

#[macro_export]
macro_rules! mob_flags {
    ($ch:expr) => {
        (($ch).char_specials.saved.act)
    };
}

impl CharData {
    pub fn aff_flags(&self) -> i64 {
        self.char_specials.borrow().saved.affected_by
    }
    pub fn set_aff_flags(&self, val: i64) {
        self.char_specials.borrow_mut().saved.affected_by = val;
    }
    pub fn remove_aff_flags(&self, val: i64) {
        self.char_specials.borrow_mut().saved.affected_by &= !val;
    }
    pub fn awake(&self) -> bool {
        self.get_pos() > POS_SLEEPING
    }
    pub fn can_see_in_dark(&self) -> bool {
        self.aff_flagged(AFF_INFRAVISION) || (!self.is_npc() && self.prf_flagged(PRF_HOLYLIGHT))
    }
}

#[macro_export]
macro_rules! room_flags {
    ($loc:expr) => {
        (world[($loc)].room_flags)
    };
}
#[macro_export]
macro_rules! spell_routines {
    ($spl:expr) => {
        (spell_infos[spl].routines)
    };
}

#[macro_export]
macro_rules! class_abbr {
    ($ch:expr) => {
        (if is_npc!($ch) {
            "--"
        } else {
            class_abbrevs[get_class($ch) as usize]
        })
    };
}

impl CharData {
    pub fn is_magic_user(&self) -> bool {
        self.is_npc() && self.get_class() == CLASS_MAGIC_USER
    }
    pub fn is_cleric(&self) -> bool {
        self.is_npc() && self.get_class() == CLASS_CLERIC
    }
    pub fn is_thief(&self) -> bool {
        self.is_npc() && self.get_class() == CLASS_THIEF
    }
    pub fn is_warrior(&self) -> bool {
        self.is_npc() && self.get_class() == CLASS_WARRIOR
    }
    pub fn get_real_level(&self) -> u8 {
        if self.desc.borrow().is_some()
            && self
                .desc
                .borrow()
                .as_ref()
                .unwrap()
                .original
                .borrow()
                .is_some()
        {
            self.desc
                .borrow()
                .as_ref()
                .unwrap()
                .original
                .borrow()
                .as_ref()
                .unwrap()
                .get_level()
        } else {
            self.get_level()
        }
    }
    pub fn get_eq(&self, pos: i8) -> Option<Rc<ObjData>> {
        self.equipment.borrow()[pos as usize].clone()
    }
    pub fn set_eq(&self, pos: i8, val: Option<Rc<ObjData>>) {
        self.equipment.borrow_mut()[pos as usize] = val;
    }
    pub fn is_good(&self) -> bool {
        self.get_alignment() >= 350
    }
    pub fn is_evil(&self) -> bool {
        self.get_alignment() <= -350
    }
    pub fn is_neutral(&self) -> bool {
        !self.is_good() && !self.is_evil()
    }
    pub fn is_carrying_w(&self) -> i32 {
        self.char_specials.borrow().carry_weight
    }
    pub fn incr_is_carrying_w(&self, val: i32) {
        self.char_specials.borrow_mut().carry_weight += val;
    }
    pub fn is_carrying_n(&self) -> u8 {
        self.char_specials.borrow().carry_items
    }
    pub fn incr_is_carrying_n(&self) {
        self.char_specials.borrow_mut().carry_weight += 1;
    }
    pub fn decr_is_carrying_n(&self) {
        self.char_specials.borrow_mut().carry_weight -= 1;
    }
}

impl ObjData {
    pub fn get_obj_type(&self) -> u8 {
        self.obj_flags.type_flag
    }
    pub fn set_obj_type(&mut self, val: u8) {
        self.obj_flags.type_flag = val;
    }

    pub fn get_obj_extra(&self) -> i32 {
        self.obj_flags.extra_flags
    }
    pub fn set_obj_extra(&mut self, val: i32) {
        self.obj_flags.extra_flags = val;
    }
    pub fn get_obj_wear(&self) -> i32 {
        self.obj_flags.wear_flags
    }
    pub fn set_obj_wear(&mut self, val: i32) {
        self.obj_flags.wear_flags = val;
    }
    pub fn get_obj_val(&self, val: usize) -> i32 {
        self.obj_flags.value[val]
    }
    pub fn set_obj_val(&mut self, val: usize, v: i32) {
        self.obj_flags.value[val] = v;
    }
    pub fn obj_flagged(&self, flag: i32) -> bool {
        is_set!(self.get_obj_extra(), flag)
    }
    pub fn get_obj_weight(&self) -> i32 {
        self.obj_flags.weight.get()
    }
    pub fn set_obj_weight(&self, val: i32) {
        self.obj_flags.weight.set(val);
    }
    pub fn incr_obj_weight(&self, val: i32) {
        self.obj_flags.weight.set(val + self.get_obj_weight());
    }
    pub fn get_obj_cost(&self) -> i32 {
        self.obj_flags.cost
    }
    pub fn set_obj_cost(&mut self, val: i32) {
        self.obj_flags.cost = val;
    }
    pub fn get_obj_rent(&self) -> i32 {
        self.obj_flags.cost_per_day
    }
    pub fn set_obj_rent(&mut self, val: i32) {
        self.obj_flags.cost_per_day = val;
    }
    pub fn get_obj_rnum(&self) -> obj_vnum {
        self.item_number
    }
    pub fn get_obj_affect(&self) -> i64 {
        self.obj_flags.bitvector
    }
    pub fn set_in_room(&self, val: RoomRnum) {
        self.in_room.set(val);
    }
}

impl DB {
    pub fn valid_obj_rnum(&self, obj: &ObjData) -> bool {
        obj.get_obj_rnum() < self.obj_index.len() as i16 && obj.get_obj_rnum() != NOTHING
    }
    pub fn get_obj_vnum(&self, obj: &ObjData) -> i16 {
        if self.valid_obj_rnum(obj) {
            self.obj_index[obj.get_obj_rnum() as usize].vnum
        } else {
            NOTHING
        }
    }
}

/* Various macros building up to CAN_SEE */

impl DB {
    pub fn light_ok(&self, sub: &CharData) -> bool {
        !sub.aff_flagged(AFF_BLIND) && self.is_light(sub.in_room())
            || sub.aff_flagged(AFF_INFRAVISION)
    }
}

pub fn invis_ok(sub: &CharData, obj: &CharData) -> bool {
    (!obj.aff_flagged(AFF_INVISIBLE) || sub.aff_flagged(AFF_DETECT_INVIS))
        && (!obj.aff_flagged(AFF_HIDE) || sub.aff_flagged(AFF_SENSE_LIFE))
}

pub fn invis_ok_obj(sub: &CharData, obj: &ObjData) -> bool {
    !obj.obj_flagged(ITEM_INVISIBLE) || sub.aff_flagged(AFF_DETECT_INVIS)
}

impl DB {
    pub fn mort_can_see(&self, sub: &CharData, obj: &CharData) -> bool {
        self.light_ok(sub) && invis_ok(sub, obj)
    }
    pub fn can_see_obj_carrier(&self, sub: &CharData, obj: &ObjData) -> bool {
        (obj.carried_by.borrow().is_none()
            || self.can_see(sub, obj.carried_by.borrow().as_ref().unwrap().borrow()))
            && (obj.worn_by.borrow().is_none()
                || self.can_see(sub, obj.worn_by.borrow().as_ref().unwrap().borrow()))
    }
    pub fn mort_can_see_obj(&self, sub: &CharData, obj: &ObjData) -> bool {
        self.light_ok(sub) && invis_ok_obj(sub, obj) && self.can_see_obj_carrier(sub, obj)
    }
    pub fn imm_can_see(&self, sub: &CharData, obj: &CharData) -> bool {
        self.mort_can_see(sub, obj) || (!sub.is_npc() && sub.prf_flagged(PRF_HOLYLIGHT))
    }
    pub fn can_see(&self, sub: &CharData, obj: &CharData) -> bool {
        self_(sub, obj)
            || ((sub.get_real_level()
                >= (if obj.is_npc() {
                    0
                } else {
                    obj.get_invis_lev() as u8
                }))
                && self.imm_can_see(sub, obj))
    }
}

pub fn self_(sub: &CharData, obj: &CharData) -> bool {
    sub as *const _ == obj as *const _
}

impl ObjData {
    pub fn in_room(&self) -> room_rnum {
        self.in_room.get()
    }
}

impl DB {
    pub fn pers<'a>(&self, ch: &'a CharData, vict: &CharData) -> Rc<str> {
        if self.can_see(vict, ch) {
            ch.get_name()
        } else {
            Rc::from("someone")
        }
    }
    pub fn can_see_obj(&self, sub: &CharData, obj: &ObjData) -> bool {
        self.mort_can_see_obj(sub, obj) || !sub.is_npc() && sub.prf_flagged(PRF_HOLYLIGHT)
    }

    pub fn objs<'a>(&self, obj: &'a ObjData, vict: &CharData) -> &'a str {
        if self.can_see_obj(vict, obj) {
            obj.short_description.as_str()
        } else {
            "something"
        }
    }

    pub fn objn(&self, obj: &ObjData, vict: &CharData) -> Rc<str> {
        if self.can_see_obj(vict, obj) {
            fname(obj.name.as_str())
        } else {
            Rc::from("something")
        }
    }
}

pub fn hmhr(ch: &CharData) -> &str {
    if ch.get_sex() != 0 {
        if ch.get_sex() == SEX_MALE {
            "him"
        } else {
            "her"
        }
    } else {
        "it"
    }
}

pub fn hshr(ch: &CharData) -> &str {
    if ch.get_sex() != 0 {
        if ch.get_sex() == SEX_MALE {
            "his"
        } else {
            "her"
        }
    } else {
        "its"
    }
}

pub fn hssh(ch: &CharData) -> &str {
    if ch.get_sex() != 0 {
        if ch.get_sex() == SEX_MALE {
            "he"
        } else {
            "she"
        }
    } else {
        "it"
    }
}

pub fn ana(obj: &ObjData) -> &str {
    if "aeiouAEIOU".contains(obj.name.chars().next().unwrap()) {
        "An"
    } else {
        "A"
    }
}

pub fn sana(obj: &ObjData) -> &str {
    if "aeiouAEIOU".contains(obj.name.chars().next().unwrap()) {
        "an"
    } else {
        "a"
    }
}

impl RoomDirectionData {
    pub fn exit_flagged(&self, flag: i16) -> bool {
        is_set!(self.exit_info.get(), flag)
    }
    pub fn remove_exit_info_bit(&self, flag: i32) {
        self.exit_info.set(self.exit_info.get() & !flag as i16);
    }
    pub fn set_exit_info_bit(&self, flag: i32) {
        self.exit_info.set(self.exit_info.get() | !flag as i16);
    }
}

impl DB {
    pub fn exit(&self, ch: &CharData, door: usize) -> Option<Rc<RoomDirectionData>> {
        self.world.borrow()[ch.in_room() as usize].dir_option[door as usize].clone()
    }
    pub fn room_flags(&self, loc: room_rnum) -> i32 {
        self.world.borrow()[loc as usize].room_flags.get()
    }
    pub fn room_flagged(&self, loc: room_rnum, flag: i64) -> bool {
        is_set!(self.room_flags(loc), flag as i32)
    }
    pub fn set_room_flags_bit(&self, loc: RoomRnum, flags: i64) {
        let flags = self.room_flags(loc) | flags as i32;
        self.world.borrow()[loc as usize].room_flags.set(flags);
    }
    pub fn sect(&self, loc: room_rnum) -> i32 {
        if self.valid_room_rnum(loc) {
            self.world.borrow()[loc as usize].sector_type
        } else {
            SECT_INSIDE
        }
    }
}

/* mud-life time */
pub const SECS_PER_MUD_HOUR: u64 = 75;
pub const SECS_PER_MUD_DAY: u64 = 24 * SECS_PER_MUD_HOUR;
pub const SECS_PER_MUD_MONTH: u64 = 35 * SECS_PER_MUD_DAY;
pub const SECS_PER_MUD_YEAR: u64 = 17 * SECS_PER_MUD_MONTH;

/* real-life time (remember Real Life?) */
pub const SECS_PER_REAL_MIN: u64 = 60;
pub const SECS_PER_REAL_HOUR: u64 = 60 * SECS_PER_REAL_MIN;
pub const SECS_PER_REAL_DAY: u64 = 24 * SECS_PER_REAL_HOUR;
pub const SECS_PER_REAL_YEAR: u64 = 365 * SECS_PER_REAL_DAY;

pub fn time_now() -> u64 {
    return SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}

/* external globals */
// extern struct time_data time_info;

/* local functions */
// struct time_info_data *real_time_passed(time_t t2, time_t t1);
// struct time_info_data *mud_time_passed(time_t t2, time_t t1);
// void prune_crlf(char *txt);
use rand::Rng;
/* creates a random number in interval [from;to] */
pub fn rand_number(from: u32, to: u32) -> u32 {
    /* error checking in case people call this incorrectly */
    // if from > to {
    // let  tmp = from;
    // from = to;
    // to = tmp;
    // log("SYSERR: rand_number() should be called with lowest, then highest. (%d, %d), not (%d, %d).", from, to, to, from);
    // }

    /*
     * This should always be of the form:
     *
     *	((float)(to - from + 1) * rand() / (float)(RAND_MAX + from) + from);
     *
     * if you are using rand() due to historical non-randomness of the
     * lower bits in older implementations.  We always use circle_random()
     * though, which shouldn't have that problem. Mean and standard
     * deviation of both are identical (within the realm of statistical
     * identity) if the rand() implementation is non-broken.
     */
    //return (circle_random() % (to - from + 1)) + from;
    return rand::thread_rng().gen_range(from..to + 1);
}

/* simulates dice roll */
pub fn dice(num: i32, size: i32) -> i32 {
    let mut sum: i32 = 0;
    let mut num = num;
    if size <= 0 || num <= 0 {
        return 0;
    }

    while num > 0 {
        num -= 1;
        sum += rand_number(1, size as u32) as i32;
    }

    return sum;
}

/* Be wary of sign issues with this. */
// int MIN(int a, int b)
// {
// return (a < b ? a : b);
// }

/* Be wary of sign issues with this. */
// int MAX(int a, int b)
// {
// return (a > b ? a : b);
// }

// char *CAP(char *txt)
// {
// *txt = UPPER(*txt);
// return (txt);
// }

/*
 * Strips \r\n from end of string.
 */
pub fn prune_crlf(s: &mut String) {
    while s.ends_with('\n') || s.ends_with('\r') {
        s.pop();
    }
}

/* log a death trap hit */
// void log_death_trap(struct char_data *ch)
// {
// mudlog(BRF, LVL_IMMORT, TRUE, "%s hit death trap #%d (%s)", GET_NAME(ch), GET_ROOM_VNUM(IN_ROOM(ch)), world[IN_ROOM(ch)].name);
// }

/*
 * New variable argument log() function.  Works the same as the old for
 * previously written code but is very nice for new code.
 */
// impl MainGlobals {
//     fn basic_mud_vlog(&self, msg: &str) {
//         time_t
//         ct = time(0);
//         char * time_s = asctime(localtime(&ct));
//
//         if (logfile == NULL) {
//             puts("SYSERR: Using log() before stream was initialized!");
//             return;
//         }
//
//         if (format == NULL)
//         format = "SYSERR: log() received a NULL format.";
//
//         time_s[strlen(time_s) - 1] = '\0';
//
//         fprintf(logfile, "%-15.15s :: ", time_s + 4);
//         vfprintf(logfile, format, args);
//         fputc('\n', logfile);
//         fflush(logfile);
//     }
// }

/*
 * New variable argument log() function.  Works the same as the old for
 * previously written code but is very nice for new code.
 */
use crate::db::DB;
use crate::handler::fname;
use crate::screen::{C_NRM, KGRN, KNRM, KNUL};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    room_vnum, CharData, ConState, ObjData, RoomData, RoomDirectionData, AFF_BLIND,
    AFF_DETECT_INVIS, AFF_HIDE, AFF_INFRAVISION, AFF_INVISIBLE, AFF_SENSE_LIFE, CLASS_CLERIC,
    CLASS_MAGIC_USER, CLASS_THIEF, CLASS_WARRIOR, MOB_ISNPC, NOWHERE, PLR_WRITING, POS_SLEEPING,
    PRF_COLOR_1, PRF_COLOR_2, PRF_HOLYLIGHT, PRF_LOG1, PRF_LOG2, ROOM_DARK, SECT_CITY, SECT_INSIDE,
    SEX_MALE,
};
use crate::{clr, send_to_char, DescriptorData, MainGlobals, _clrlevel, CCGRN, CCNRM};
use log::{error, info};
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

/* So mudlog() can use the same function. */
// pub fn basic_mud_log(msg: &str) {
//     basic_mud_vlog(msg);
// }

/* the "touch" command, essentially. */
pub fn touch(path: &Path) -> io::Result<()> {
    match OpenOptions::new().create(true).write(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/*
 * mudlog -- log mud messages to a file & to online imm's syslogs
 * based on syslog by Fen Jul 3, 1992
 */
impl MainGlobals {
    pub(crate) fn mudlog(&self, _type: u8, level: i32, file: bool, msg: &str) {
        if msg == "" {
            return;
        }
        if file {
            info!("{}", msg);
            //basic_mud_vlog(msg);
        }

        if level < 0 {
            return;
        }

        let buf = format!("[ {} ]", msg);

        for d in self.descriptor_list.borrow().iter() {
            let ohc = d.character.borrow();
            let character = ohc.as_ref().unwrap();
            if d.state() != ConPlaying || character.is_npc() {
                /* switch */
                continue;
            }
            if character.get_level() < level as u8 {
                continue;
            }
            if character.plr_flagged(PLR_WRITING) {
                continue;
            }
            let x = if _type > u8::from(character.prf_flagged(PRF_LOG1)) {
                1
            } else {
                0
            };
            let x = x + if character.prf_flagged(PRF_LOG2) {
                2
            } else {
                0
            };
            if x != 0 {
                continue;
            }
            // if  type > prf_flagged!(character, PRF_LOG1)?
            // 1: 0) + (PRF_FLAGGED(i->character, PRF_LOG2)?
            // 2: 0))
            // continue;
            send_to_char(
                &character,
                format!(
                    "{}{}{}",
                    CCGRN!(character, C_NRM),
                    buf,
                    CCNRM!(character, C_NRM)
                )
                .as_str(),
            );
        }
    }
}

/*
 * If you don't have a 'const' array, just cast it as such.  It's safer
 * to cast a non-const array as const than to cast a const one as non-const.
 * Doesn't really matter since this function doesn't change the array though.
 */
pub fn sprintbit(bitvector: i64, names: &[&str], result: &mut String) -> usize {
    // size_t len = 0;
    // int nlen;
    // long nr;

    let mut nr = 0;
    let mut bitvector = bitvector;
    loop {
        if bitvector == 0 {
            break;
        }
        if is_set!(bitvector, 1) {
            result.push_str(if nr < (names.len() - 1) {
                names[nr]
            } else {
                "UNDEFINED"
            });
        }
        bitvector >>= 1;
    }
    if result.len() == 0 {
        result.push_str("NOBITS");
    }

    return result.len();
}

// size_t sprinttype(int type, const char *names[], char *result, size_t reslen)
// {
// int nr = 0;
//
// while (type && *names[nr] != '\n') {
// type--;
// nr++;
// }
//
// return strlcpy(result, *names[nr] != '\n' ? names[nr] : "UNDEFINED", reslen);
// }

/* Calculate the REAL time passed over the last t2-t1 centuries (secs) */
// struct time_info_data *real_time_passed(time_t t2, time_t t1)
// {
// long secs;
// static struct time_info_data now;
//
// secs = t2 - t1;
//
// now.hours = (secs / SECS_PER_REAL_HOUR) % 24;	/* 0..23 hours */
// secs -= SECS_PER_REAL_HOUR * now.hours;
//
// now.day = (secs / SECS_PER_REAL_DAY);	/* 0..34 days  */
// /* secs -= SECS_PER_REAL_DAY * now.day; - Not used. */
//
// now.month = -1;
// now.year = -1;
//
// return (&now);
// }

/* Calculate the MUD time passed over the last t2-t1 centuries (secs) */
// struct time_info_data *mud_time_passed(time_t t2, time_t t1)
// {
// long secs;
// static struct time_info_data now;
//
// secs = t2 - t1;
//
// now.hours = (secs / SECS_PER_MUD_HOUR) % 24;	/* 0..23 hours */
// secs -= SECS_PER_MUD_HOUR * now.hours;
//
// now.day = (secs / SECS_PER_MUD_DAY) % 35;	/* 0..34 days  */
// secs -= SECS_PER_MUD_DAY * now.day;
//
// now.month = (secs / SECS_PER_MUD_MONTH) % 17;	/* 0..16 months */
// secs -= SECS_PER_MUD_MONTH * now.month;
//
// now.year = (secs / SECS_PER_MUD_YEAR);	/* 0..XX? years */
//
// return (&now);
// }

// time_t mud_time_to_secs(struct time_info_data *now)
// {
// time_t when = 0;
//
// when += now->year  * SECS_PER_MUD_YEAR;
// when += now->month * SECS_PER_MUD_MONTH;
// when += now->day   * SECS_PER_MUD_DAY;
// when += now->hours * SECS_PER_MUD_HOUR;
//
// return (time(NULL) - when);
// }

// struct time_info_data *age(struct char_data *ch)
// {
// static struct time_info_data player_age;
//
// player_age = *mud_time_passed(time(0), ch->player.time.birth);
//
// player_age.year += 17;	/* All players start at 17 */
//
// return (&player_age);
// }

/* Check if making CH follow VICTIM will create an illegal */
/* Follow "Loop/circle"                                    */
// bool circle_follow(struct char_data *ch, struct char_data *victim)
// {
// struct char_data *k;
//
// for (k = victim; k; k = k->master) {
// if (k == ch)
// return (TRUE);
// }
//
// return (FALSE);
// }

/* Called when stop following persons, or stopping charm */
/* This will NOT do if a character quits/dies!!          */
// void stop_follower(struct char_data *ch)
// {
// struct follow_type *j, *k;
//
// if (ch->master == NULL) {
// core_dump();
// return;
// }
//
// if (AFF_FLAGGED(ch, AFF_CHARM)) {
// act("You realize that $N is a jerk!", FALSE, ch, 0, ch->master, TO_CHAR);
// act("$n realizes that $N is a jerk!", FALSE, ch, 0, ch->master, TO_NOTVICT);
// act("$n hates your guts!", FALSE, ch, 0, ch->master, TO_VICT);
// if (affected_by_spell(ch, SPELL_CHARM))
// affect_from_char(ch, SPELL_CHARM);
// } else {
// act("You stop following $N.", FALSE, ch, 0, ch->master, TO_CHAR);
// act("$n stops following $N.", TRUE, ch, 0, ch->master, TO_NOTVICT);
// act("$n stops following you.", TRUE, ch, 0, ch->master, TO_VICT);
// }
//
// if (ch->master->followers->follower == ch) {	/* Head of follower-list? */
// k = ch->master->followers;
// ch->master->followers = k->next;
// free(k);
// } else {			/* locate follower who is not head of list */
// for (k = ch->master->followers; k->next->follower != ch; k = k->next);
//
// j = k->next;
// k->next = j->next;
// free(j);
// }
//
// ch->master = NULL;
// REMOVE_BIT(AFF_FLAGS(ch), AFF_CHARM | AFF_GROUP);
// }

//
// int num_followers_charmed(struct char_data *ch)
// {
// struct follow_type *lackey;
// int total = 0;
//
// for (lackey = ch->followers; lackey; lackey = lackey->next)
// if (AFF_FLAGGED(lackey->follower, AFF_CHARM) && lackey->follower->master == ch)
// total++;
//
// return (total);
// }
//

/* Called when a character that follows/is followed dies */
// void die_follower(struct char_data *ch)
// {
// struct follow_type *j, *k;
//
// if (ch->master)
// stop_follower(ch);
//
// for (k = ch->followers; k; k = j) {
// j = k->next;
// stop_follower(k->follower);
// }
// }
//
//

/* Do NOT call this before having checked if a circle of followers */
/* will arise. CH will follow leader                               */
// void add_follower(struct char_data *ch, struct char_data *leader)
// {
// struct follow_type *k;
//
// if (ch->master) {
// core_dump();
// return;
// }
//
// ch->master = leader;
//
// CREATE(k, struct follow_type, 1);
//
// k->follower = ch;
// k->next = leader->followers;
// leader->followers = k;
//
// act("You now follow $N.", FALSE, ch, 0, leader, TO_CHAR);
// if (CAN_SEE(leader, ch))
// act("$n starts following you.", TRUE, ch, 0, leader, TO_VICT);
// act("$n starts to follow $N.", TRUE, ch, 0, leader, TO_NOTVICT);
// }

/*
 * get_line reads the next non-blank line off of the input stream.
 * The newline character is removed from the input.  Lines which begin
 * with '*' are considered to be comments.
 *
 * Returns the number of lines advanced in the file. Buffer given must
 * be at least READ_SIZE (256) characters large.
 */
pub fn get_line(reader: &mut BufReader<File>, buf: &mut String) -> i32 {
    //char temp[READ_SIZE];
    let mut lines = 0;
    //let sl: i32;
    let mut temp = String::new();

    loop {
        temp.clear();
        let r = reader.read_line(&mut temp);
        if !r.is_ok() {
            return 0;
        }
        temp = temp.trim_end().to_string();
        lines += 1;
        if temp.starts_with('*') || temp.starts_with('\n') || temp.starts_with('\r') {
            continue;
        }
        break;
    }

    /* Last line of file doesn't always have a \n, but it should. */
    buf.clear();
    buf.push_str(temp.trim_end());
    return lines;
}

// int get_filename(char *filename, size_t fbufsize, int mode, const char *orig_name)
// {
// const char *prefix, *middle, *suffix;
// char name[PATH_MAX], *ptr;
//
// if (orig_name == NULL || *orig_name == '\0' || filename == NULL) {
// log("SYSERR: NULL pointer or empty string passed to get_filename(), %p or %p.",
// orig_name, filename);
// return (0);
// }
//
// switch (mode) {
// case CRASH_FILE:
// prefix = LIB_PLROBJS;
// suffix = SUF_OBJS;
// break;
// case ALIAS_FILE:
// prefix = LIB_PLRALIAS;
// suffix = SUF_ALIAS;
// break;
// case ETEXT_FILE:
// prefix = LIB_PLRTEXT;
// suffix = SUF_TEXT;
// break;
// default:
// return (0);
// }
//
// strlcpy(name, orig_name, sizeof(name));
// for (ptr = name; *ptr; ptr++)
// *ptr = LOWER(*ptr);
//
// switch (LOWER(*name)) {
// case 'a':  case 'b':  case 'c':  case 'd':  case 'e':
// middle = "A-E";
// break;
// case 'f':  case 'g':  case 'h':  case 'i':  case 'j':
// middle = "F-J";
// break;
// case 'k':  case 'l':  case 'm':  case 'n':  case 'o':
// middle = "K-O";
// break;
// case 'p':  case 'q':  case 'r':  case 's':  case 't':
// middle = "P-T";
// break;
// case 'u':  case 'v':  case 'w':  case 'X':  case 'y':  case 'z':
// middle = "U-Z";
// break;
// default:
// middle = "ZZZ";
// break;
// }
//
// snprintf(filename, fbufsize, "%s%s"SLASH"%s.%s", prefix, middle, name, suffix);
// return (1);
// }

pub fn num_pc_in_room(room: &RoomData) -> i32 {
    room.peoples.borrow().len() as i32
}

/*
 * This function (derived from basic fork(); abort(); idea by Erwin S.
 * Andreasen) causes your MUD to dump core (assuming you can) but
 * continue running.  The core dump will allow post-mortem debugging
 * that is less severe than assert();  Don't call this directly as
 * core_dump_unix() but as simply 'core_dump()' so that it will be
 * excluded from systems not supporting them. (e.g. Windows '95).
 *
 * You still want to call abort() or exit(1) for
 * non-recoverable errors, of course...
 *
 * XXX: Wonder if flushing streams includes sockets?
 */
// extern FILE *player_fl;
// void core_dump_real(const char *who, int line)
// {
// log("SYSERR: Assertion failed at %s:%d!", who, line);
//
// #if 0	/* By default, let's not litter. */
// #if defined(CIRCLE_UNIX)
// /* These would be duplicated otherwise...make very sure. */
// fflush(stdout);
// fflush(stderr);
// fflush(logfile);
// fflush(player_fl);
// /* Everything, just in case, for the systems that support it. */
// fflush(NULL);
//
// /*
//  * Kill the child so the debugger or script doesn't think the MUD
//  * crashed.  The 'autorun' script would otherwise run it again.
//  */
// if (fork() == 0)
// abort();
// #endif
// #endif
// }

/*
 * Rules (unless overridden by ROOM_DARK):
 *
 * Inside and City rooms are always lit.
 * Outside rooms are dark at sunset and night.
 */
impl DB {
    pub fn is_light(&self, room: room_rnum) -> bool {
        !self.is_dark(room)
    }
    pub fn is_dark(&self, room: room_rnum) -> bool {
        if !self.valid_room_rnum(room) {
            error!(
                "room_is_dark: Invalid room rnum {}. (0-{})",
                room,
                self.world.borrow().len()
            );
            return false;
        }

        if self.world.borrow()[room as usize].light.get() != 0 {
            return false;
        }

        if self.room_flagged(room, ROOM_DARK) {
            return true;
        }

        if self.sect(room) == SECT_INSIDE || self.sect(room) == SECT_CITY {
            return false;
        }

        // if (weather_info.sunlight == SUN_SET | | weather_info.sunlight == SUN_DARK)
        // return (TRUE);

        return false;
    }
}
