/* ************************************************************************
*   File: utils.rs                                      Part of CircleMUD *
*  Usage: various internal functions of a utility nature                  *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

/* defines for mudlog() */
use std::borrow::Borrow;
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{TimeZone, Utc};
use log::{error, info};
// struct time_info_data *real_time_passed(time_t t2, time_t t1);
// struct time_info_data *mud_time_passed(time_t t2, time_t t1);
// void prune_crlf(char *txt);
use rand::Rng;

use crate::class::CLASS_ABBREVS;
use crate::constants::STR_APP;
use crate::db::{DB, LIB_PLRALIAS, LIB_PLROBJS, LIB_PLRTEXT, SUF_ALIAS, SUF_OBJS, SUF_TEXT};
use crate::handler::{affect_from_char, affected_by_spell, fname};
use crate::screen::{C_NRM, KGRN, KNRM, KNUL};
use crate::spells::SPELL_CHARM;
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    CharData, ConState, FollowType, MobVnum, ObjData, RoomData, RoomDirectionData, Special,
    AFF_BLIND, AFF_DETECT_INVIS, AFF_HIDE, AFF_INFRAVISION, AFF_INVISIBLE, AFF_SENSE_LIFE,
    CLASS_CLERIC, CLASS_MAGIC_USER, CLASS_THIEF, CLASS_WARRIOR, LVL_IMMORT, MOB_ISNPC, NOWHERE,
    PLR_WRITING, POS_SLEEPING, PRF_COLOR_1, PRF_COLOR_2, PRF_HOLYLIGHT, PRF_LOG1, PRF_LOG2,
    ROOM_DARK, SECT_CITY, SECT_INSIDE, SEX_MALE, SUN_DARK, SUN_SET,
};
use crate::structs::{
    MobRnum, ObjVnum, RoomRnum, RoomVnum, TimeInfoData, AFF_CHARM, AFF_GROUP, EX_CLOSED,
    ITEM_CONTAINER, ITEM_INVISIBLE, ITEM_WEAR_TAKE, NOBODY, NOTHING, ROOM_INDOORS,
};
use crate::{
    _clrlevel, clr, send_to_char, DescriptorData, Game, CCGRN, CCNRM, TO_CHAR, TO_NOTVICT, TO_VICT,
};

// pub const OFF: u8 = 0;
pub const BRF: u8 = 1;
pub const NRM: u8 = 2;
pub const CMP: u8 = 3;

/* get_filename() */
pub const CRASH_FILE: i32 = 0;
pub const ETEXT_FILE: i32 = 1;
pub const ALIAS_FILE: i32 = 2;

/* breadth-first searching */
pub const BFS_ERROR: i32 = -1;
pub const BFS_ALREADY_THERE: i32 = -2;
pub const BFS_NO_PATH: i32 = -3;

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

impl CharData {
    pub fn is_npc(&self) -> bool {
        return is_set!(self.char_specials.borrow().saved.act, MOB_ISNPC);
    }
    pub fn memory(&self) -> &RefCell<Vec<i64>> {
        &self.mob_specials.memory
    }
}

impl DB {
    pub fn is_mob(&self, ch: &CharData) -> bool {
        ch.is_npc()
            && ch.get_mob_rnum() != NOBODY
            && ch.get_mob_rnum() < self.mob_protos.len() as i16
    }
    pub fn get_mob_spec(&self, ch: &CharData) -> Option<Special> {
        if self.is_mob(ch) {
            self.mob_index[ch.nr as usize].func
        } else {
            None
        }
    }
}

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
    pub fn remove_prf_flags_bits(&self, flag: i64) {
        self.player_specials.borrow_mut().saved.pref &= !flag;
    }
    pub fn toggle_prf_flag_bits(&self, flag: i64) -> i64 {
        self.player_specials.borrow_mut().saved.pref ^= flag;
        self.player_specials.borrow().saved.pref
    }
    pub fn toggle_plr_flag_bits(&self, flag: i64) -> i64 {
        self.char_specials.borrow_mut().saved.act ^= flag;
        self.char_specials.borrow().saved.act
    }
    pub fn plr_tog_chk(&self, flag: i64) -> i64 {
        self.toggle_plr_flag_bits(flag) & flag
    }
    pub fn prf_tog_chk(&self, flag: i64) -> i64 {
        self.toggle_prf_flag_bits(flag) & flag
    }
}

/* TOO
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
pub use check_player_special;

impl CharData {
    pub fn poofin(&self) -> Rc<str> {
        self.player_specials.borrow().poofin.clone()
    }
    pub fn poofout(&self) -> Rc<str> {
        self.player_specials.borrow().poofout.clone()
    }
    pub fn get_last_tell(&self) -> i64 {
        self.player_specials.borrow().last_tell
    }
    pub fn set_last_tell(&self, val: i64) {
        self.player_specials.borrow_mut().last_tell = val;
    }
    pub fn get_invis_lev(&self) -> i16 {
        check_player_special!(self, self.player_specials.borrow().saved.invis_level)
    }
    pub fn get_wimp_lev(&self) -> i32 {
        self.player_specials.borrow().saved.wimp_level
    }
    pub fn set_wimp_lev(&self, val: i32) {
        self.player_specials.borrow_mut().saved.wimp_level = val;
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
    pub fn decr_hit(&self, val: i16) {
        self.points.borrow_mut().hit -= val;
    }
}

impl DB {
    pub fn get_room_vnum(&self, rnum: RoomVnum) -> i16 {
        if self.valid_room_rnum(rnum) {
            self.world[rnum as usize].number
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

impl CharData {
    pub fn get_pc_name(&self) -> Rc<str> {
        return Rc::from(self.player.borrow().name.as_str());
    }
    pub fn get_name(&self) -> Rc<str> {
        if self.is_npc() {
            Rc::from(self.player.borrow().short_descr.as_str())
        } else {
            self.get_pc_name()
        }
    }

    pub fn has_title(&self) -> bool {
        self.player.borrow().title.is_some()
    }
    pub fn get_title(&self) -> Rc<str> {
        if self.player.borrow().title.is_none() {
            return Rc::from("");
        }
        Rc::from(self.player.borrow().title.as_ref().unwrap().as_str())
    }
    pub fn set_title(&self, val: Option<String>) {
        self.player.borrow_mut().title = val;
    }
}

#[macro_export]
macro_rules! an {
    ($string:expr) => {
        if "aeiouAEIOU".contains($string.chars().next().unwrap()) {
            "an"
        } else {
            "a"
        }
    };
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
        self.pfilepos.get()
    }
    pub fn set_pfilepos(&self, val: i32) {
        self.pfilepos.set(val);
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
    pub fn set_bank_gold(&self, val: i32) {
        self.points.borrow_mut().bank_gold = val
    }
    pub fn get_bank_gold(&self) -> i32 {
        self.points.borrow().bank_gold
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
    pub fn in_room(&self) -> RoomRnum {
        self.in_room.get()
    }
    pub fn set_in_room(&self, val: RoomRnum) {
        self.in_room.set(val);
    }
    pub fn get_was_in(&self) -> RoomRnum {
        self.was_in_room.get()
    }
    pub fn get_age(&self) -> i16 {
        age(self).year
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
    pub fn incr_cond(&self, i: i32, val: i16) {
        self.player_specials.borrow_mut().saved.conditions[i as usize] += val;
    }
    pub fn get_loadroom(&self) -> RoomVnum {
        self.player_specials.borrow().saved.load_room
    }
    pub fn set_loadroom(&self, val: RoomVnum) {
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
    pub fn get_skill(&self, i: i32) -> i8 {
        self.player_specials.borrow().saved.skills[i as usize]
    }
    pub fn set_skill(&self, i: i32, pct: i8) {
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
        self.char_specials.borrow().fighting.as_ref().map( |f| f.clone())
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
    pub fn get_save(&self, i: i32) -> i16 {
        self.char_specials.borrow().saved.apply_saving_throw[i as usize]
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
    pub fn set_aff_flags_bits(&self, val: i64) {
        self.char_specials.borrow_mut().saved.affected_by |= val;
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

pub fn has_spell_routine(db: &DB, spl: i32, flag: i32) -> bool {
    is_set!(db.spell_info[spl as usize].routines, flag)
}

impl CharData {
    pub fn class_abbr(&self) -> &'static str {
        if self.is_npc() {
            "--"
        } else {
            CLASS_ABBREVS[self.get_class() as usize]
        }
    }
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
    pub fn set_is_carrying_w(&self, val: i32) {
        self.char_specials.borrow_mut().carry_weight = val;
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
    pub fn set_is_carrying_n(&self, val: u8) {
        self.char_specials.borrow_mut().carry_items = val;
    }
    pub fn get_freeze_lev(&self) -> i8 {
        self.player_specials.borrow().saved.freeze_level
    }
    pub fn set_freeze_lev(&self, val: i8) {
        self.player_specials.borrow_mut().saved.freeze_level = val;
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
        self.obj_flags.extra_flags.get()
    }
    pub fn set_obj_extra(&self, val: i32) {
        self.obj_flags.extra_flags.set(val);
    }
    pub fn set_obj_extra_bit(&self, val: i32) {
        self.obj_flags
            .extra_flags
            .set(self.obj_flags.extra_flags.get() | val);
    }
    pub fn remove_obj_extra_bit(&self, val: i32) {
        self.obj_flags
            .extra_flags
            .set(self.obj_flags.extra_flags.get() & !val);
    }
    pub fn get_obj_wear(&self) -> i32 {
        self.obj_flags.wear_flags
    }
    pub fn set_obj_wear(&mut self, val: i32) {
        self.obj_flags.wear_flags = val;
    }
    pub fn get_obj_val(&self, val: usize) -> i32 {
        self.obj_flags.value[val].get()
    }
    pub fn set_obj_val(&self, val: usize, v: i32) {
        self.obj_flags.value[val].set(v);
    }
    pub fn decr_obj_val(&self, val: usize) {
        self.obj_flags.value[val].set(self.obj_flags.value[val].get() - 1);
    }
    pub fn incr_obj_val(&self, val: usize) {
        self.obj_flags.value[val].set(self.obj_flags.value[val].get() + 1);
    }
    pub fn obj_flagged(&self, flag: i32) -> bool {
        is_set!(self.get_obj_extra(), flag)
    }
    pub fn objval_flagged(&self, flag: i32) -> bool {
        is_set!(self.get_obj_val(1), flag)
    }
    pub fn remove_objval_bit(&self, val: i32, flag: i32) {
        self.obj_flags.value[val as usize].set(self.obj_flags.value[val as usize].get() & !flag)
    }
    pub fn set_objval_bit(&self, val: i32, flag: i32) {
        self.obj_flags.value[val as usize].set(self.obj_flags.value[val as usize].get() | flag)
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
    pub fn get_obj_rnum(&self) -> ObjVnum {
        self.item_number
    }
    pub fn get_obj_affect(&self) -> i64 {
        self.obj_flags.bitvector.get()
    }
    pub fn set_obj_affect(&self, val: i64) {
        self.obj_flags.bitvector.set(val);
    }
    pub fn set_in_room(&self, val: RoomRnum) {
        self.in_room.set(val);
    }
    pub fn is_corpse(&self) -> bool {
        self.get_obj_type() == ITEM_CONTAINER && self.get_obj_val(3) == 1
    }
    pub fn get_obj_timer(&self) -> i32 {
        self.obj_flags.timer.get()
    }
    pub fn set_obj_timer(&self, val: i32) {
        self.obj_flags.timer.set(val);
    }
    pub fn decr_obj_timer(&self, val: i32) {
        self.obj_flags.timer.set(self.obj_flags.timer.get() - val);
    }
}

pub fn clone_vec<A: Clone>(from: &RefCell<Vec<A>>) -> Vec<A> {
    let mut ret = vec![];
    for e in from.borrow().iter() {
        ret.push(e.clone());
    }
    ret
}

pub fn clone_vec2<A: Clone>(from: &Vec<A>) -> Vec<A> {
    let mut ret = vec![];
    for e in from.iter() {
        ret.push(e.clone());
    }
    ret
}

impl ObjData {
    pub fn objwear_flagged(&self, flag: i32) -> bool {
        is_set!(self.get_obj_wear(), flag)
    }
    pub fn can_wear(&self, part: i32) -> bool {
        self.objwear_flagged(part)
    }
}

impl CharData {
    pub fn can_carry_obj(&self, obj: &ObjData) -> bool {
        (self.is_carrying_w() + obj.get_obj_weight()) <= self.can_carry_w() as i32
            && (self.is_carrying_n() + 1) <= self.can_carry_n() as u8
    }
    pub fn can_carry_w(&self) -> i16 {
        STR_APP[self.strength_apply_index()].carry_w
    }
    pub fn can_carry_n(&self) -> i32 {
        (5 + self.get_dex() as i32 >> 1) + (self.get_level() as i32 >> 1)
    }
    pub fn strength_apply_index(&self) -> usize {
        (if self.get_add() == 0 || self.get_str() != 18 {
            self.get_str()
        } else if self.get_add() <= 50 {
            26
        } else if self.get_add() <= 75 {
            27
        } else if self.get_add() <= 90 {
            28
        } else if self.get_add() <= 99 {
            29
        } else {
            30
        }) as usize
    }
}

impl DB {
    pub fn valid_room_rnum(&self, rnum: RoomRnum) -> bool {
        rnum != NOWHERE && rnum <= self.world.len() as i16
    }
    pub fn get_room_spec(&self, rnum: RoomRnum) -> Option<Special> {
        if self.valid_room_rnum(rnum) {
            self.world[rnum as usize].func.borrow().clone()
        } else {
            None
        }
    }
    pub fn outside(&self, ch: &CharData) -> bool {
        !self.room_flagged(ch.in_room(), ROOM_INDOORS)
    }
    pub fn can_get_obj(&self, ch: &CharData, obj: &ObjData) -> bool {
        obj.can_wear(ITEM_WEAR_TAKE) && ch.can_carry_obj(obj) && self.can_see_obj(ch, obj)
    }
    pub fn can_go(&self, ch: &CharData, door: usize) -> bool {
        self.exit(ch, door).is_some()
            && self.exit(ch, door).as_ref().unwrap().to_room != NOWHERE
            && !is_set!(
                self.exit(ch, door).as_ref().unwrap().exit_info,
                EX_CLOSED
            )
    }

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
    pub fn get_mob_vnum(&self, mob: &CharData) -> MobVnum {
        if self.is_mob(mob) {
            self.mob_index[mob.get_mob_rnum() as usize].vnum
        } else {
            NOBODY
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
    std::ptr::eq(sub, obj)
}

impl ObjData {
    pub fn in_room(&self) -> RoomRnum {
        self.in_room.get()
    }
}

impl DB {
    pub fn pers(&self, ch: &CharData, vict: &CharData) -> Rc<str> {
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
            fname(obj.name.borrow().as_str())
        } else {
            Rc::from("something")
        }
    }
    pub fn get_obj_spec(&self, obj: &Rc<ObjData>) -> Option<Special> {
        if self.valid_obj_rnum(obj) {
            self.obj_index[obj.get_obj_rnum() as usize].func
        } else {
            None
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

// pub fn ana(obj: &ObjData) -> &str {
//     if "aeiouAEIOU".contains(obj.name.borrow().chars().next().unwrap()) {
//         "An"
//     } else {
//         "A"
//     }
// }

pub fn sana(obj: &ObjData) -> &str {
    if "aeiouAEIOU".contains(obj.name.borrow().chars().next().unwrap()) {
        "an"
    } else {
        "a"
    }
}

impl RoomDirectionData {
    pub fn exit_flagged(&self, flag: i16) -> bool {
        is_set!(self.exit_info, flag)
    }
    pub fn remove_exit_info_bit(&mut self, flag: i32) {
        self.exit_info &= !flag as i16;
    }
    pub fn set_exit_info_bit(&mut self, flag: i32) {
        self.exit_info |= !flag as i16;
    }
}

impl DB {
    pub fn exit(&self, ch: &CharData, door: usize) -> Option<RoomDirectionData> {
        self.world[ch.in_room() as usize].dir_option[door].clone()
    }
    pub fn room_flags(&self, loc: RoomRnum) -> i32 {
        self.world[loc as usize].room_flags
    }
    pub fn room_flagged(&self, loc: RoomRnum, flag: i64) -> bool {
        is_set!(self.room_flags(loc), flag as i32)
    }
    pub fn set_room_flags_bit(&mut self, loc: RoomRnum, flags: i64) {
        let flags = self.room_flags(loc) | flags as i32;
        self.world[loc as usize].room_flags = flags;
    }
    pub fn remove_room_flags_bit(&mut self, loc: RoomRnum, flags: i64) {
        let flags = self.room_flags(loc) & !flags as i32;
        self.world[loc as usize].room_flags = flags;
    }
    pub fn sect(&self, loc: RoomRnum) -> i32 {
        if self.valid_room_rnum(loc) {
            self.world[loc as usize].sector_type
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
// pub const SECS_PER_REAL_YEAR: u64 = 365 * SECS_PER_REAL_DAY;

pub fn ctime(t: u64) -> String {
    let date_time = Utc.timestamp_millis_opt(t as i64 * 1000).unwrap();
    date_time.to_rfc2822()
}

pub fn time_now() -> u64 {
    return SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}

/* creates a random number in interval [from;to] */
pub fn rand_number(from: u32, to: u32) -> u32 {
    /* error checking in case people call this incorrectly */
    let mut from = from;
    let mut to = to;
    if from > to {
        let tmp = from;
        from = to;
        to = tmp;
        error!("SYSERR: rand_number() should be called with lowest, then highest. ({}, {}), not ({}, {}).", from, to, to, from);
    }

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

/*
 * Strips \r\n from end of string.
 */
pub fn prune_crlf(text: &mut Rc<str>) {
    let mut s = text.to_string();
    while s.ends_with('\n') || s.ends_with('\r') {
        s.pop();
    }
    *text = Rc::from(s.as_str());
}

/* log a death trap hit */
pub fn log_death_trap(game: &Game, ch: &CharData) {
    game.mudlog(
        BRF,
        LVL_IMMORT as i32,
        true,
        format!(
            "{} hit death trap #{} ({})",
            ch.get_name(),
            game.db.get_room_vnum(ch.in_room()),
            game.db.world[ch.in_room() as usize].name
        )
        .as_str(),
    );
}

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
impl Game {
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

        for d in self.descriptor_list.iter() {
            if d.state() != ConPlaying {
                /* switch */
                continue;
            }
            let ohc = d.character.borrow();
            let character = ohc.as_ref().unwrap();
            if character.is_npc() {
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
    let mut nr = 0;
    let mut bitvector = bitvector;
    loop {
        if bitvector == 0 {
            break;
        }
        if is_set!(bitvector, 1) {
            let s = if nr < (names.len() - 1) {
                format!("{} ", names[nr])
            } else {
                "UNDEFINED ".to_string()
            };
            result.push_str(s.as_str());
        }
        nr += 1;
        bitvector >>= 1;
    }
    if result.len() == 0 {
        result.push_str("NOBITS");
    }

    return result.len();
}

pub fn sprinttype(_type: i32, names: &[&str], result: &mut String) {
    let mut nr = 0;
    let mut _type = _type;
    while _type != 0 && names[nr] != "\n" {
        _type -= 1;
        nr += 1;
    }

    while _type != 0 && names[nr] != "\n" {
        _type -= 1;
        nr += 1;
    }

    result.push_str(if names[nr] != "\n" {
        names[nr]
    } else {
        "UNDEFINED"
    });
}

/* Calculate the REAL time passed over the last t2-t1 centuries (secs) */
pub fn real_time_passed(t2: u64, t1: u64) -> TimeInfoData {
    let mut secs = t2 - t1;
    let mut now = TimeInfoData {
        hours: ((secs / SECS_PER_REAL_HOUR) % 24) as i32,
        /* 0..23 hours */
        day: 0,
        month: 0,
        year: 0,
    };
    secs -= SECS_PER_REAL_HOUR * now.hours as u64;

    now.day = (secs / SECS_PER_REAL_DAY) as i32; /* 0..34 days  */
    /* secs -= SECS_PER_REAL_DAY * now.day; - Not used. */

    now.month = -1;
    now.year = -1;

    now
}

/* Calculate the MUD time passed over the last t2-t1 centuries (secs) */
pub fn mud_time_passed(t2: u64, t1: u64) -> TimeInfoData {
    let mut now = TimeInfoData {
        hours: 0,
        day: 0,
        month: 0,
        year: 0,
    };
    let mut secs = t2 - t1;

    now.hours = ((secs / SECS_PER_MUD_HOUR) % 24) as i32; /* 0..23 hours */
    secs -= SECS_PER_MUD_HOUR * now.hours as u64;

    now.day = ((secs / SECS_PER_MUD_DAY) % 35) as i32; /* 0..34 days  */
    secs -= SECS_PER_MUD_DAY * now.day as u64;

    now.month = ((secs / SECS_PER_MUD_MONTH) % 17) as i32; /* 0..16 months */
    secs -= SECS_PER_MUD_MONTH * now.month as u64;

    now.year = (secs / SECS_PER_MUD_YEAR) as i16; /* 0..XX? years */

    now
}

pub fn mud_time_to_secs(now: &TimeInfoData) -> u64 {
    let mut when: u64 = 0;

    when += now.year as u64 * SECS_PER_MUD_YEAR;
    when += now.month as u64 * SECS_PER_MUD_MONTH;
    when += now.day as u64 * SECS_PER_MUD_DAY;
    when += now.hours as u64 * SECS_PER_MUD_HOUR;

    time_now() - when
}

pub fn age(ch: &CharData) -> TimeInfoData {
    let mut player_age = mud_time_passed(time_now(), ch.player.borrow().time.birth);

    player_age.year += 17; /* All players start at 17 */

    player_age
}

/* Check if making CH follow VICTIM will create an illegal */
/* Follow "Loop/circle"                                    */
pub fn circle_follow(ch: &Rc<CharData>, victim: Option<&Rc<CharData>>) -> bool {
    if victim.is_none() {
        return false;
    }
    let mut k = victim.unwrap().clone();
    loop {
        if Rc::ptr_eq(&k, ch) {
            return true;
        }

        let t;
        {
            if k.master.borrow().is_none() {
                break;
            } else {
                t = k.master.borrow().as_ref().unwrap().clone();
            }
        }
        k = t;
    }
    false
}

/* Called when stop following persons, or stopping charm */
/* This will NOT do if a character quits/dies!!          */
impl DB {
    pub fn stop_follower(&self, ch: &Rc<CharData>) {
        if ch.master.borrow().is_none() {
            return;
        }

        if ch.aff_flagged(AFF_CHARM) {
            self.act(
                "You realize that $N is a jerk!",
                false,
                Some(ch),
                None,
                Some(ch.master.borrow().as_ref().unwrap()),
                TO_CHAR,
            );
            self.act(
                "$n realizes that $N is a jerk!",
                false,
                Some(ch),
                None,
                Some(ch.master.borrow().as_ref().unwrap()),
                TO_NOTVICT,
            );
            self.act(
                "$n hates your guts!",
                false,
                Some(ch),
                None,
                Some(ch.master.borrow().as_ref().unwrap()),
                TO_VICT,
            );
            if affected_by_spell(ch, SPELL_CHARM as i16) {
                affect_from_char(ch, SPELL_CHARM as i16);
            }
        } else {
            self.act(
                "You stop following $N.",
                false,
                Some(ch),
                None,
                Some(ch.master.borrow().as_ref().unwrap()),
                TO_CHAR,
            );
            self.act(
                "$n stops following $N.",
                true,
                Some(ch),
                None,
                Some(ch.master.borrow().as_ref().unwrap()),
                TO_NOTVICT,
            );
            self.act(
                "$n stops following you.",
                true,
                Some(ch),
                None,
                Some(ch.master.borrow().as_ref().unwrap()),
                TO_VICT,
            );
        }

        ch.master
            .borrow()
            .as_ref()
            .unwrap()
            .followers
            .borrow_mut()
            .retain(|c| Rc::ptr_eq(&c.follower, ch));

        *ch.master.borrow_mut() = None;
        ch.remove_aff_flags(AFF_CHARM | AFF_GROUP);
    }
}

pub fn num_followers_charmed(ch: &Rc<CharData>) -> i32 {
    let mut total = 0;

    for lackey in ch.followers.borrow().iter() {
        if lackey.follower.aff_flagged(AFF_CHARM)
            && Rc::ptr_eq(lackey.follower.master.borrow().as_ref().unwrap(), ch)
        {
            total += 1;
        }
    }
    total
}

impl DB {
    /* Called when a character that follows/is followed dies */
    pub fn die_follower(&self, ch: &Rc<CharData>) {
        if ch.master.borrow().is_some() {
            self.stop_follower(ch);
        }

        for k in ch.followers.borrow().iter() {
            self.stop_follower(&k.follower);
        }
    }
}

/* Do NOT call this before having checked if a circle of followers */
/* will arise. CH will follow leader                               */
pub fn add_follower(db: &DB, ch: &Rc<CharData>, leader: &Rc<CharData>) {
    if ch.master.borrow().is_some() {
        // core_dump();
        return;
    }

    *ch.master.borrow_mut() = Some(leader.clone());

    let k = FollowType {
        follower: ch.clone(),
    };
    leader.followers.borrow_mut().push(k);

    db.act(
        "You now follow $N.",
        false,
        Some(ch),
        None,
        Some(leader),
        TO_CHAR,
    );
    if db.can_see(leader, ch) {
        db.act(
            "$n starts following you.",
            true,
            Some(ch),
            None,
            Some(leader),
            TO_VICT,
        );
    }
    db.act(
        "$n starts to follow $N.",
        true,
        Some(ch),
        None,
        Some(leader),
        TO_NOTVICT,
    );
}

/*
 * get_line reads the next non-blank line off of the input stream.
 * The newline character is removed from the input.  Lines which begin
 * with '*' are considered to be comments.
 *
 * Returns the number of lines advanced in the file. Buffer given must
 * be at least READ_SIZE (256) characters large.
 */
pub fn get_line(reader: &mut BufReader<File>, buf: &mut String) -> i32 {
    let mut lines = 0;
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

pub fn get_filename(filename: &mut String, mode: i32, orig_name: &str) -> bool {
    if orig_name.is_empty() {
        error!(
            "SYSERR:  empty string passed to get_filename(), {} .",
            orig_name
        );
        return false;
    }
    let prefix;
    let suffix;

    match mode {
        CRASH_FILE => {
            prefix = LIB_PLROBJS;
            suffix = SUF_OBJS;
        }
        ALIAS_FILE => {
            prefix = LIB_PLRALIAS;
            suffix = SUF_ALIAS;
        }
        ETEXT_FILE => {
            prefix = LIB_PLRTEXT;
            suffix = SUF_TEXT;
        }
        _ => {
            return false;
        }
    }

    let name = orig_name.to_lowercase();
    let middle;

    match name.chars().next().unwrap() {
        'a' | 'b' | 'c' | 'd' | 'e' => {
            middle = "A-E";
        }

        'f' | 'g' | 'h' | 'i' | 'j' => {
            middle = "F-J";
        }

        'k' | 'l' | 'm' | 'n' | 'o' => {
            middle = "K-O";
        }
        'p' | 'q' | 'r' | 's' | 't' => {
            middle = "P-T";
        }
        'u' | 'v' | 'w' | 'X' | 'y' | 'z' => {
            middle = "U-Z";
        }
        _ => {
            middle = "ZZZ";
        }
    }

    *filename = format!("{}{}/{}.{}", prefix, middle, name, suffix);
    true
}

pub fn num_pc_in_room(room: &RoomData) -> i32 {
    room.peoples.len() as i32
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
    pub fn is_light(&self, room: RoomRnum) -> bool {
        !self.is_dark(room)
    }
    pub fn is_dark(&self, room: RoomRnum) -> bool {
        if !self.valid_room_rnum(room) {
            error!(
                "room_is_dark: Invalid room rnum {}. (0-{})",
                room,
                self.world.len()
            );
            return false;
        }

        if self.world[room as usize].light != 0 {
            return false;
        }

        if self.room_flagged(room, ROOM_DARK) {
            return true;
        }

        if self.sect(room) == SECT_INSIDE || self.sect(room) == SECT_CITY {
            return false;
        }

        if self.weather_info.get().sunlight == SUN_SET
            || self.weather_info.get().sunlight == SUN_DARK
        {
            return true;
        }

        return false;
    }
}
