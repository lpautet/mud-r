/* ************************************************************************
*   File: objsave.rs                                    Part of CircleMUD *
*  Usage: loading/saving player objects for rent and crash-save           *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, Write};
use std::path::Path;
use std::{fs, mem, slice};

use crate::depot::{DepotId, HasId};
use crate::VictimRef;
use log::{error, info};

use crate::act_social::do_action;
use crate::class::invalid_class;
use crate::config::{
    CRASH_FILE_TIMEOUT, FREE_RENT, MAX_OBJ_SAVE, MIN_RENT_COST, RENT_FILE_TIMEOUT,
};
use crate::db::{DB, REAL, VIRTUAL};
use crate::handler::invalid_align;
use crate::interpreter::{cmd_is, find_command};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    CharData, MeRef, ObjAffectedType, ObjData, ObjFileElem, RentInfo, ITEM_CONTAINER, ITEM_KEY,
    ITEM_NORENT, ITEM_WEAPON, ITEM_WEAR_ABOUT, ITEM_WEAR_ARMS, ITEM_WEAR_BODY, ITEM_WEAR_FEET,
    ITEM_WEAR_FINGER, ITEM_WEAR_HANDS, ITEM_WEAR_HEAD, ITEM_WEAR_HOLD, ITEM_WEAR_LEGS,
    ITEM_WEAR_NECK, ITEM_WEAR_SHIELD, ITEM_WEAR_WAIST, ITEM_WEAR_WIELD, ITEM_WEAR_WRIST, LVL_GOD,
    LVL_IMMORT, MAX_OBJ_AFFECT, NOTHING, NUM_WEARS, PLR_CRASH, PLR_CRYO, RENT_CRASH, RENT_CRYO,
    RENT_FORCED, RENT_RENTED, RENT_TIMEDOUT, WEAR_ABOUT, WEAR_ARMS, WEAR_BODY, WEAR_FEET,
    WEAR_FINGER_L, WEAR_FINGER_R, WEAR_HANDS, WEAR_HEAD, WEAR_HOLD, WEAR_LEGS, WEAR_LIGHT,
    WEAR_NECK_1, WEAR_NECK_2, WEAR_SHIELD, WEAR_WAIST, WEAR_WIELD, WEAR_WRIST_L, WEAR_WRIST_R,
};
use crate::util::{
    get_filename, hssh, rand_number, time_now, BRF, CRASH_FILE, NRM, SECS_PER_REAL_DAY,
};
use crate::{Game, TO_NOTVICT, TO_ROOM, TO_VICT};

/* these factors should be unique integers */
const RENT_FACTOR: i32 = 1;
const CRYO_FACTOR: i32 = 4;

pub const LOC_INVENTORY: i32 = 0;
pub const MAX_BAG_ROWS: i32 = 5;

pub fn obj_from_store(db: &mut DB, object: &ObjFileElem, location: &mut i32) -> Option<DepotId> {
    *location = 0;
    let itemnum = db.real_object(object.item_number);
    if itemnum == NOTHING {
        return None;
    }

    let oid = db.read_object(itemnum, REAL).unwrap();
    let obj = db.obj_mut(oid);
    *location = object.location as i32;
    obj.set_obj_val(0, object.value[0]);
    obj.set_obj_val(1, object.value[1]);
    obj.set_obj_val(2, object.value[2]);
    obj.set_obj_val(3, object.value[3]);
    obj.set_obj_extra(object.extra_flags);
    obj.set_obj_weight(object.weight);
    obj.set_obj_timer(object.timer);
    obj.set_obj_affect(object.bitvector);

    for j in 0..MAX_OBJ_AFFECT as usize {
        obj.affected[j] = object.affected[j];
    }
    Some(obj.id())
}

pub fn obj_to_store(db: &DB, obj: &ObjData, fl: &mut File, location: i32) -> bool {
    let mut object = ObjFileElem {
        item_number: db.get_obj_vnum(obj),
        location: location as i16,
        value: [
            obj.get_obj_val(0),
            obj.get_obj_val(1),
            obj.get_obj_val(2),
            obj.get_obj_val(3),
        ],
        extra_flags: obj.get_obj_extra(),
        weight: obj.get_obj_weight(),
        timer: obj.get_obj_timer(),
        bitvector: obj.get_obj_affect(),
        affected: [ObjAffectedType {
            location: 0,
            modifier: 0,
        }; 6],
    };
    for i in 0..6 {
        object.affected[i] = obj.affected[i];
    }

    let record_size = mem::size_of::<ObjFileElem>();
    let slice;
    unsafe {
        slice = slice::from_raw_parts(&mut object as *mut _ as *mut u8, record_size);
    }
    let r = fl.write_all(slice);
    if r.is_err() {
        error!("SYSERR: error writing object in Obj_to_store");
        return false;
    }
    true
}

/*
 * AutoEQ by Burkhard Knopf <burkhard.knopf@informatik.tu-clausthal.de>
 */
fn auto_equip(game: &mut Game, db: &mut DB, chid: DepotId, oid: DepotId, location: i32) {
    let ch = db.ch(chid);

    let mut location = location;
    /* Lots of checks... */
    let mut j = 0;
    if location > 0 {
        /* Was wearing it. */
        j = location - 1;
        let obj = db.obj(oid);
        match j as i16 {
            WEAR_LIGHT => {}
            WEAR_FINGER_R | WEAR_FINGER_L => {
                if !obj.can_wear(ITEM_WEAR_FINGER) {
                    /* not fitting :( */
                    location = LOC_INVENTORY;
                }
            }
            WEAR_NECK_1 | WEAR_NECK_2 => {
                if !obj.can_wear(ITEM_WEAR_NECK) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_BODY => {
                if !obj.can_wear(ITEM_WEAR_BODY) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_HEAD => {
                if !obj.can_wear(ITEM_WEAR_HEAD) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_LEGS => {
                if !obj.can_wear(ITEM_WEAR_LEGS) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_FEET => {
                if !obj.can_wear(ITEM_WEAR_FEET) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_HANDS => {
                if !obj.can_wear(ITEM_WEAR_HANDS) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_ARMS => {
                if !obj.can_wear(ITEM_WEAR_ARMS) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_SHIELD => {
                if !obj.can_wear(ITEM_WEAR_SHIELD) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_ABOUT => {
                if !obj.can_wear(ITEM_WEAR_ABOUT) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_WAIST => {
                if !obj.can_wear(ITEM_WEAR_WAIST) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_WRIST_R | WEAR_WRIST_L => {
                if !obj.can_wear(ITEM_WEAR_WRIST) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_WIELD => {
                if !obj.can_wear(ITEM_WEAR_WIELD) {
                    location = LOC_INVENTORY;
                }
            }
            WEAR_HOLD => {
                if obj.can_wear(ITEM_WEAR_HOLD) {
                } else if ch.is_warrior()
                    && obj.can_wear(ITEM_WEAR_WIELD)
                    && obj.get_obj_type() == ITEM_WEAPON
                {
                } else {
                    location = LOC_INVENTORY;
                }
            }
            _ => {
                location = LOC_INVENTORY;
            }
        }
    }

    if location > 0 {
        /* Wearable. */
        if ch.get_eq(j as i8).is_none() {
            /*
             * Check the characters's alignment to prevent them from being
             * zapped through the auto-equipping.
             */
            let obj = db.obj(oid);
            if invalid_align(ch, obj) || invalid_class(ch, obj) {
                location = LOC_INVENTORY;
            } else {
                game.equip_char(db, chid, oid, j as i8);
            }
        } else {
            /* Oops, saved a player with double equipment? */
            game.mudlog(
                db,
                BRF,
                LVL_IMMORT as i32,
                true,
                format!(
                    "SYSERR: autoeq: '{}' already equipped in position {}.",
                    ch.get_name(),
                    location
                )
                .as_str(),
            );
            location = LOC_INVENTORY;
        }
    }

    if location <= 0 {
        /* Inventory */
        db.obj_to_char(oid, chid);
    }
}

pub fn crash_delete_file(name: &str) -> bool {
    let mut filename = String::new();

    if !get_filename(&mut filename, CRASH_FILE, name) {
        return false;
    }
    {
        let fl = OpenOptions::new().read(true).open(&filename);
        if fl.is_err() {
            let err = fl.err().unwrap();
            if err.kind() != ErrorKind::NotFound {
                /* if it fails but NOT because of no file */
                error!("SYSERR: deleting crash file {} (1): {}", &filename, err);
            }
            return false;
        }
    }
    let r = fs::remove_file(Path::new(&filename));
    /* if it fails, NOT because of no file */
    if r.is_err() {
        let err = r.err().unwrap();
        if err.kind() != ErrorKind::NotFound {
            error!("SYSERR: deleting crash file {} (2): {}", filename, err);
        }
    }

    return true;
}

pub fn crash_delete_crashfile(ch: &CharData) -> bool {
    let mut filename = String::new();

    if !get_filename(&mut filename, CRASH_FILE, ch.get_name().as_ref()) {
        return false;
    }
    let fl = OpenOptions::new().read(true).open(&filename);
    if fl.is_err() {
        let err = fl.err().unwrap();
        if err.kind() != ErrorKind::NotFound {
            /* if it fails, NOT because of no file */
            error!("SYSERR: checking for crash file {} (3): {}", &filename, err);
        }
        return false;
    }
    let mut rent_info = RentInfo::new();
    let slice;
    unsafe {
        slice = slice::from_raw_parts_mut(
            &mut rent_info as *mut _ as *mut u8,
            mem::size_of::<RentInfo>(),
        );
    }
    let r = fl.unwrap().read_exact(slice);

    if r.is_err() {
        return false;
    }

    if rent_info.rentcode == RENT_CRASH {
        crash_delete_file(ch.get_name().as_ref());
    }

    return true;
}

fn crash_clean_file(name: &str) -> bool {
    let mut filename = String::new();
    let mut rent = RentInfo::new();

    if !get_filename(&mut filename, CRASH_FILE, name) {
        return false;
    }
    /*
     * open for write so that permission problems will be flagged now, at boot
     * time.
     */
    let fl = OpenOptions::new().read(true).open(&filename);
    if fl.is_err() {
        let err = fl.err().unwrap();
        if err.kind() != ErrorKind::NotFound {
            /* if it fails, NOT because of no file */
            error!("SYSERR: OPENING OBJECT FILE {} (4): {}", &filename, err);
        }
        return false;
    }

    let slice;
    unsafe {
        slice =
            slice::from_raw_parts_mut(&mut rent as *mut _ as *mut u8, mem::size_of::<RentInfo>());
    }

    let r = fl.unwrap().read_exact(slice);

    if r.is_err() {
        return false;
    }

    if rent.rentcode == RENT_CRASH || rent.rentcode == RENT_FORCED || rent.rentcode == RENT_TIMEDOUT
    {
        if rent.time < time_now() as i64 - (CRASH_FILE_TIMEOUT as i64 * SECS_PER_REAL_DAY as i64) {
            crash_delete_file(name);
            let filetype;
            match rent.rentcode {
                RENT_CRASH => {
                    filetype = "crash";
                }
                RENT_FORCED => {
                    filetype = "forced rent";
                }
                RENT_TIMEDOUT => {
                    filetype = "idlesave";
                }
                _ => {
                    filetype = "UNKNOWN!";
                }
            }
            info!("    Deleting {}'s {} file.", name, filetype);
            return true;
        }
        /* Must retrieve rented items w/in 30 days */
    } else if rent.rentcode == RENT_RENTED {
        if rent.time < (time_now() as i64 - (RENT_FILE_TIMEOUT as i64 * SECS_PER_REAL_DAY as i64)) {
            crash_delete_file(name);
            info!("    Deleting {}'s rent file.", name);
            return true;
        }
    }
    false
}

pub fn update_obj_file(db: &DB) {
    for i in 0..db.player_table.len() {
        if !db.player_table[i].name.is_empty() {
            crash_clean_file(&db.player_table[i].name);
        }
    }
}

pub fn crash_listrent(game: &mut Game, db: &mut DB, chid: DepotId, name: &str) {
    let ch = db.ch(chid);
    let mut filename = String::new();
    if !get_filename(&mut filename, CRASH_FILE, name) {
        return;
    }
    let fl = OpenOptions::new().read(true).open(&filename);
    if fl.is_err() {
        game.send_to_char(ch, format!("{} has no rent file.\r\n", name).as_str());
        return;
    }
    let mut rent = RentInfo::new();
    let mut fl = fl.unwrap();
    let slice;
    unsafe {
        slice =
            slice::from_raw_parts_mut(&mut rent as *mut _ as *mut u8, mem::size_of::<RentInfo>());
    }
    let r = fl.read_exact(slice);

    /* Oops, can't get the data, punt. */
    if r.is_err() {
        game.send_to_char(ch, "Error reading rent information.\r\n");
        return;
    }

    game.send_to_char(ch, format!("{}\r\n", filename).as_str());
    match rent.rentcode {
        RENT_RENTED => {
            game.send_to_char(ch, "Rent\r\n");
        }
        RENT_CRASH => {
            game.send_to_char(ch, "Crash\r\n");
        }
        RENT_CRYO => {
            game.send_to_char(ch, "Cryo\r\n");
        }
        RENT_TIMEDOUT | RENT_FORCED => {
            game.send_to_char(ch, "TimedOut\r\n");
        }
        _ => {
            game.send_to_char(ch, "Undef\r\n");
        }
    }

    loop {
        let mut object = ObjFileElem::new();
        let slice;
        unsafe {
            slice = slice::from_raw_parts_mut(
                &mut object as *mut _ as *mut u8,
                mem::size_of::<ObjFileElem>(),
            );
        }
        let r = fl.read_exact(slice);

        if r.is_err() {
            return;
        }

        if db.real_object(object.item_number) != NOTHING {
            let oid = db.read_object(object.item_number, VIRTUAL);
            let obj = db.obj(oid.unwrap());
            // #if USE_AUTOEQ
            // game.send_to_char(ch, " [%5d] (%5dau) <%2d> %-20s\r\n",
            // object.item_number, obj.get_obj_rent(),
            // object.location, obj->short_description);
            // #else
            let oin = object.item_number;
            let ch = db.ch(chid);
            game.send_to_char(
                ch,
                format!(
                    " [{:5}] ({:5}au) {:20}\r\n",
                    oin,
                    obj.get_obj_rent(),
                    obj.short_description
                )
                .as_str(),
            );
            // #endif
            db.extract_obj( oid.unwrap());
        }
    }
}

fn crash_write_rentcode(_chid: DepotId, fl: &mut File, rent: &mut RentInfo) -> bool {
    let record_size = mem::size_of::<RentInfo>();

    let rent_slice;
    unsafe {
        rent_slice = slice::from_raw_parts(rent as *mut _ as *mut u8, record_size);
    }
    let r = fl.write_all(rent_slice);

    if r.is_err() {
        error!("SYSERR: writing rent code {}", r.err().unwrap());
        return false;
    }
    true
}

impl RentInfo {
    fn new() -> RentInfo {
        RentInfo {
            time: 0,
            rentcode: 0,
            net_cost_per_diem: 0,
            gold: 0,
            account: 0,
            nitems: 0,
            spare0: 0,
            spare1: 0,
            spare2: 0,
            spare3: 0,
            spare4: 0,
            spare5: 0,
            spare6: 0,
            spare7: 0,
        }
    }
}

impl ObjFileElem {
    pub fn new() -> ObjFileElem {
        ObjFileElem {
            item_number: 0,
            location: 0,
            value: [0; 4],
            extra_flags: 0,
            weight: 0,
            timer: 0,
            bitvector: 0,
            affected: [ObjAffectedType {
                location: 0,
                modifier: 0,
            }; MAX_OBJ_AFFECT as usize],
        }
    }
}

/*
 * Return values:
 *  0 - successful load, keep char in rent room.
 *  1 - load failure or load of crash items -- put char in temple.
 *  2 - rented equipment lost (no $)
 */
pub fn crash_load(game: &mut Game, db: &mut DB, chid: DepotId) -> i32 {
    let ch = db.ch(chid);

    /* AutoEQ addition. */
    let mut location = 0;
    let mut j;
    let mut cont_row: [Vec<DepotId>; MAX_BAG_ROWS as usize] =
        [vec![], vec![], vec![], vec![], vec![]];

    let mut filename = String::new();
    if !get_filename(&mut filename, CRASH_FILE, &ch.get_name()) {
        return 1;
    }
    let fl = OpenOptions::new().read(true).write(true).open(&filename);
    if fl.is_err() {
        let err = fl.err().unwrap();
        if err.kind() != ErrorKind::NotFound {
            error!("SYSERR: READING OBJECT FILE {} (5) {}", filename, err);
            game.send_to_char(ch,
                         "\r\n********************* NOTICE *********************\r\nThere was a problem loading your objects from disk.\r\nContact a God for assistance.\r\n");
        }
        let ch = db.ch(chid);
        game.mudlog(
            db,
            NRM,
            max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
            true,
            format!("{} entering game with no equipment.", ch.get_name()).as_str(),
        );
        return 1;
    }
    let mut fl = fl.unwrap();
    let mut rent = RentInfo::new();
    let slice;
    unsafe {
        slice =
            slice::from_raw_parts_mut(&mut rent as *mut _ as *mut u8, mem::size_of::<RentInfo>());
    }
    let r = fl.read_exact(slice);

    if r.is_err() {
        error!(
            "SYSERR: Crash_load: {}'s rent file was empty!",
            ch.get_name()
        );
        return 1;
    }
    if rent.rentcode == RENT_RENTED || rent.rentcode == RENT_TIMEDOUT {
        let num_of_days = (time_now() - rent.time as u64) / SECS_PER_REAL_DAY;
        let cost = rent.net_cost_per_diem * num_of_days as i32;
        if cost > ch.get_gold() + ch.get_bank_gold() {
            game.mudlog(
                db,
                BRF,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "{} entering game, rented equipment lost (no $).",
                    ch.get_name()
                )
                .as_str(),
            );
            crash_crashsave(db, chid);
            return 2;
        } else {
            let ch = db.ch_mut(chid);
            ch.set_bank_gold(ch.get_bank_gold() - max(cost - ch.get_gold(), 0));
            ch.set_gold(max(ch.get_gold() - cost, 0));
            game.save_char(db, chid);
        }
    }
    let mut num_objs = 0;
    let orig_rent_code = rent.rentcode;
    let ch = db.ch(chid);
    match orig_rent_code {
        RENT_RENTED => {
            game.mudlog(
                db,
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!("{} un-renting and entering game.", ch.get_name()).as_str(),
            );
        }
        RENT_CRASH => {
            game.mudlog(
                db,
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "{} retrieving crash-saved items and entering game.",
                    ch.get_name()
                )
                .as_str(),
            );
        }
        RENT_CRYO => {
            game.mudlog(
                db,
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!("{} un-cryo'ing and entering game.", ch.get_name()).as_str(),
            );
        }
        RENT_FORCED | RENT_TIMEDOUT => {
            game.mudlog(
                db,
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "{} retrieving force-saved items and entering game.",
                    ch.get_name()
                )
                .as_str(),
            );
        }
        _ => {
            let rc = rent.rentcode;
            game.mudlog(
                db,
                BRF,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "SYSERR: {} entering game with undefined rent code {}.",
                    ch.get_name(),
                    rc
                )
                .as_str(),
            );
        }
    }

    loop {
        let mut object = ObjFileElem::new();

        let slice;
        unsafe {
            slice = slice::from_raw_parts_mut(
                &mut object as *mut _ as *mut u8,
                mem::size_of::<ObjFileElem>(),
            );
        }
        let r = fl.read_exact(slice);
        if r.is_err() {
            let err = r.err().unwrap();
            if err.kind() == ErrorKind::UnexpectedEof {
                break;
            }
            error!("SYSERR: Reading crash file: Crash_load");
            return 1;
        }

        num_objs += 1;
        let oid = obj_from_store(db, &object, &mut location);
        if oid.is_none() {
            continue;
        }
        let mut oid = oid.unwrap();

        auto_equip(game, db, chid, oid, location);
        /*
         * What to do with a new loaded item:
         *
         * If there's a list with location less than 1 below this, then its
         * container has disappeared from the file so we put the list back into
         * the character's inventory. (Equipped items are 0 here.)
         *
         * If there's a list of contents with location of 1 below this, then we
         * check if it is a container:
         *   - Yes: Get it from the character, fill it, and give it back so we
         *          have the correct weight.
         *   -  No: The container is missing so we put everything back into the
         *          character's inventory.
         *
         * For items with negative location, we check if there is already a list
         * of contents with the same location.  If so, we put it there and if not,
         * we start a new list.
         *
         * Since location for contents is < 0, the list indices are switched to
         * non-negative.
         *
         * This looks ugly, but it works.
         */
        if location > 0 {
            /* Equipped */
            j = (MAX_BAG_ROWS - 1) as usize;
            loop {
                if j == 0 {
                    break;
                }
                if cont_row[j].len() != 0 {
                    /* No container, back to inventory. */
                    for obj2 in cont_row[j].iter() {
                        db.obj_to_char(*obj2, chid);
                    }
                    cont_row[j].clear();
                }
                j -= 1;
            }
            if cont_row[0].len() != 0 {
                /* Content list existing. */
                if db.obj(oid).get_obj_type() == ITEM_CONTAINER {
                    /* Remove object, fill it, equip again. */
                    oid = db.unequip_char( chid, (location - 1) as i8).unwrap();
                    db.obj_mut(oid).contains.clear(); /* Should be empty anyway, but just in case. */
                    for oid2 in cont_row[0].iter() {
                        db.obj_to_obj(*oid2, oid);
                    }
                    game.equip_char(db, chid, oid, (location - 1) as i8);
                } else {
                    /* Object isn't container, empty the list. */
                    for oid2 in cont_row[0].iter() {
                        db.obj_to_char(*oid2, chid);
                    }
                    cont_row[0].clear();
                }
            }
        } else {
            /* location <= 0 */
            j = MAX_BAG_ROWS as usize - 1;
            loop {
                if j == -location as usize {
                    break;
                }
                if cont_row[j].len() != 0 {
                    /* No container, back to inventory. */
                    for obj2 in cont_row[j].iter() {
                        db.obj_to_char(*obj2, chid);
                    }
                    cont_row[j].clear();
                }
                j -= 1;
            }
            if j == -location as usize && cont_row[j].len() != 0 {
                /* Content list exists. */
                if db.obj(oid).get_obj_type() == ITEM_CONTAINER {
                    /* Take the item, fill it, and give it back. */
                    db.obj_from_char(oid);
                    db.obj_mut(oid).contains.clear();
                    for oid2 in cont_row[j].iter() {
                        db.obj_to_obj(*oid2, oid);
                    }
                    db.obj_to_char(oid, chid); /* Add to inventory first. */
                } else {
                    /* Object isn't container, empty content list. */
                    for oid2 in cont_row[j].iter() {
                        db.obj_to_char(*oid2, chid);
                    }
                    cont_row[j].clear();
                }
            }
            if location < 0 && location >= -MAX_BAG_ROWS {
                /*
                 * TODO Let the object be part of the content list but put it at the
                 * list's end.  Thus having the items in the same order as before
                 * the character rented.
                 */
                // obj_from_char(obj.as_ref());
                // if (obj2 = cont_row[-location - 1]) != NULL {
                //     while (obj2 -> next_content)
                //     obj2 = obj2 -> next_content;
                //     obj2 -> next_content = obj;
                // } else
                // cont_row[-location - 1] = obj;
            }
        }
    }

    /* Little hoarding check. -gg 3/1/98 */
    let ch = db.ch(chid);
    game.mudlog(
        db,
        NRM,
        max(ch.get_invis_lev() as i32, LVL_GOD as i32),
        true,
        format!(
            "{} (level {}) has {} object{} (max {}).",
            ch.get_name(),
            ch.get_level(),
            num_objs,
            if num_objs != 1 { "s" } else { "" },
            MAX_OBJ_SAVE
        )
        .as_str(),
    );

    /* turn this into a crash file by re-writing the control block */
    rent.rentcode = RENT_CRASH;
    rent.time = time_now() as i64;
    fl.rewind().expect("Cannot unwrap file");
    crash_write_rentcode(chid, &mut fl, &mut rent);

    return if (orig_rent_code == RENT_RENTED) || (orig_rent_code == RENT_CRYO) {
        0
    } else {
        1
    };
}

fn crash_save(db: &mut DB, oid: Option<DepotId>, fp: &mut File, location: i32) -> bool {
    let location = location;
    let result;
    if oid.is_some() {
        result = obj_to_store(db, db.obj(oid.unwrap()), fp, location);
        for o in db.obj(oid.unwrap()).contains.clone() {
            crash_save(db, Some(o), fp, location - 1);
        }

        let mut toid = oid.unwrap();

        loop {
            if db.obj(toid).in_obj.is_none() {
                break;
            }

            let obj_weight = db.obj(oid.unwrap()).get_obj_weight();
            db.obj_mut(db.obj(toid).in_obj.unwrap())
                .incr_obj_weight(-obj_weight);

            toid = db.obj(toid).in_obj.unwrap();
        }

        if !result {
            return false;
        }
    }
    true
}

fn crash_restore_weight(db: &mut DB, oid: DepotId) {
    for o in db.obj(oid).contains.clone() {
        crash_restore_weight(db, o);
    }
    if db.obj(oid).in_obj.is_some() {
        let obj_weight = db.obj(oid).get_obj_weight();
        db.obj_mut(db.obj(oid).in_obj.unwrap())
            .incr_obj_weight(obj_weight);
    }
}

/*
 * Get !RENT items from equipment to inventory and
 * extract !RENT out of worn containers.
 */
fn crash_extract_norent_eq(game: &mut Game, db: &mut DB, chid: DepotId) {
    for j in 0..NUM_WEARS {
        let ch = db.ch(chid);
        if ch.get_eq(j).is_none() {
            continue;
        }
        if crash_is_unrentable(db.obj(ch.get_eq(j).unwrap())) {
            let eqid = db.unequip_char( chid, j).unwrap();
            db.obj_to_char(eqid, chid);
        } else {
            crash_extract_norents(game, db, ch.get_eq(j).unwrap());
        }
    }
}

fn crash_extract_objs(game: &mut Game, db: &mut DB, oid: Option<DepotId>) {
    let oid = oid.unwrap();
    for o in db.obj(oid).contains.clone() {
        crash_extract_objs(game, db, Some(o));
    }
    db.extract_obj( oid);
}

fn crash_is_unrentable(obj: &ObjData) -> bool {
    if obj.obj_flagged(ITEM_NORENT)
        || obj.get_obj_rent() < 0
        || obj.get_obj_rnum() == NOTHING
        || obj.get_obj_type() == ITEM_KEY
    {
        return true;
    }
    false
}

fn crash_extract_norents(game: &mut Game, db: &mut DB, oid: DepotId) {
    for o in db.obj(oid).contains.clone() {
        crash_extract_norents(game, db, o);
    }

    if crash_is_unrentable(db.obj(oid)) {
        db.extract_obj( oid);
    }
}

fn crash_extract_expensive(db: &mut DB, oids: Vec<DepotId>) {
    if oids.len() == 0 {
        return;
    }
    let mut max = oids[0];

    for tobjid in oids {
        if db.obj(tobjid).get_obj_rent() > db.obj(max).get_obj_rent() {
            max = tobjid;
        }
    }

    db.extract_obj( max);
}

fn crash_calculate_rent(db: &DB, oid: Option<DepotId>, cost: &mut i32) {
    if oid.is_some() {
        let oid = oid.unwrap();
        *cost += max(0, db.obj(oid).get_obj_rent());
        for o in db.obj(oid).contains.iter() {
            crash_calculate_rent(db, Some(*o), cost);
        }
    }
}

pub fn crash_crashsave(db: &mut DB, chid: DepotId) {
    let ch = db.ch(chid);
    if ch.is_npc() {
        return;
    }

    let mut buf = String::new();
    if !get_filename(&mut buf, CRASH_FILE, &ch.get_name()) {
        return;
    }
    let mut fp = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&buf)
        .expect("Cannot open rent crash file");

    let mut rent = RentInfo {
        time: time_now() as i64,
        rentcode: RENT_CRASH,
        net_cost_per_diem: 0,
        gold: 0,
        account: 0,
        nitems: 0,
        spare0: 0,
        spare1: 0,
        spare2: 0,
        spare3: 0,
        spare4: 0,
        spare5: 0,
        spare6: 0,
        spare7: 0,
    };

    if !crash_write_rentcode(chid, &mut fp, &mut rent) {
        return;
    }

    for j in 0..NUM_WEARS as usize {
        let ch = db.ch(chid);
        if ch.get_eq(j as i8).is_some() {
            if !crash_save(db, ch.get_eq(j as i8), &mut fp, (j + 1) as i32) {
                return;
            }
            let ch = db.ch(chid);
            crash_restore_weight(db, ch.get_eq(j as i8).unwrap());
        }
    }
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        if !crash_save(db, Some(o), &mut fp, 0) {
            return;
        }
    }
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        crash_restore_weight(db, o);
    }
    let ch = db.ch_mut(chid);
    ch.remove_plr_flag(PLR_CRASH);
}

pub fn crash_idlesave(game: &mut Game, db: &mut DB, chid: DepotId) {
    let ch = db.ch(chid);
    let mut rent = RentInfo::new();

    if ch.is_npc() {
        return;
    }
    let mut buf = String::new();
    if !get_filename(&mut buf, CRASH_FILE, ch.get_name().as_ref()) {
        return;
    }
    let fp = OpenOptions::new().create(true).write(true).open(buf);
    if fp.is_err() {
        return;
    }

    crash_extract_norent_eq(game, db, chid);
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        crash_extract_norents(game, db, o);
    }

    let mut cost = 0;
    let ch = db.ch(chid);
    for o in ch.carrying.iter() {
        crash_calculate_rent(&db, Some(*o), &mut cost);
    }

    let mut cost_eq = 0;
    for j in 0..NUM_WEARS {
        crash_calculate_rent(db, ch.get_eq(j), &mut cost_eq);
    }

    cost += cost_eq;
    cost *= 2; /* forcerent cost is 2x normal rent */

    if cost > ch.get_gold() + ch.get_bank_gold() {
        for j in 0..NUM_WEARS {
            /* Unequip players with low gold. */
            let ch = db.ch(chid);
            if ch.get_eq(j).is_some() {
                let eqid = db.unequip_char( chid, j).unwrap();
                db.obj_to_char(eqid, chid);
            }
        }

        while {
            let ch = db.ch(chid);
            (cost > ch.get_gold() + ch.get_bank_gold()) && ch.carrying.len() != 0
        } {
            let ch = db.ch(chid);
            crash_extract_expensive(db, ch.carrying.clone());
            cost = 0;
            let ch = db.ch(chid);
            for o in ch.carrying.iter() {
                crash_calculate_rent(&db, Some(*o), &mut cost);
            }
            cost *= 2;
        }
    }
    let ch = db.ch(chid);
    if ch.carrying.len() == 0 {
        let mut found = false;
        for j in 0..NUM_WEARS {
            if ch.get_eq(j).is_some() {
                found = true;
                break;
            }
        }

        if !found {
            /* No equipment or inventory. */

            crash_delete_file(ch.get_name().as_ref());
            return;
        }
    }
    rent.net_cost_per_diem = cost;

    rent.rentcode = RENT_TIMEDOUT;
    rent.time = time_now() as i64;
    rent.gold = ch.get_gold();
    rent.account = ch.get_bank_gold();
    let mut fp = fp.unwrap();
    if !crash_write_rentcode(chid, &mut fp, &mut rent) {
        return;
    }
    for j in 0..NUM_WEARS {
        let ch = db.ch(chid);
        if ch.get_eq(j).is_some() {
            let ch = db.ch(chid);
            let oid = ch.get_eq(j);
            if !crash_save(db, oid, &mut fp, (j + 1) as i32) {
                return;
            }
            let ch = db.ch(chid);
            let oid = ch.get_eq(j).unwrap();
            crash_restore_weight(db, oid);
            let ch = db.ch(chid);
            let oid = ch.get_eq(j);
            crash_extract_objs(game, db, oid);
        }
    }
    let mut location = 0;
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        if !crash_save(db, Some(o), &mut fp, location) {
            return;
        }
        location += 1;
    }
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        crash_extract_objs(game, db, Some(o));
    }
}

pub fn crash_rentsave(game: &mut Game, db: &mut DB, chid: DepotId, cost: i32) {
    let ch = db.ch(chid);
    if ch.is_npc() {
        return;
    }
    let mut buf = String::new();
    if !get_filename(&mut buf, CRASH_FILE, &ch.get_name()) {
        return;
    }
    let fpo = OpenOptions::new().write(true).create(true).open(buf);
    if fpo.is_err() {
        return;
    }
    let mut fp = fpo.unwrap();

    crash_extract_norent_eq(game, db, chid);
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        crash_extract_norents(game, db, o);
    }
    let ch = db.ch(chid);
    let mut rent = RentInfo {
        time: time_now() as i64,
        rentcode: RENT_RENTED,
        net_cost_per_diem: cost,
        gold: ch.get_gold(),
        account: ch.get_bank_gold(),
        nitems: 0,
        spare0: 0,
        spare1: 0,
        spare2: 0,
        spare3: 0,
        spare4: 0,
        spare5: 0,
        spare6: 0,
        spare7: 0,
    };

    if !crash_write_rentcode(chid, &mut fp, &mut rent) {
        return;
    }

    for j in 0..NUM_WEARS {
        let ch = db.ch(chid);
        if ch.get_eq(j).is_some() {
            let oid = ch.get_eq(j);
            if !crash_save(db, oid, &mut fp, (j + 1) as i32) {
                return;
            }
            let ch = db.ch(chid);
            let oid = ch.get_eq(j).unwrap();
            crash_restore_weight(db, oid);
            let ch = db.ch(chid);
            crash_extract_objs(game, db, ch.get_eq(j));
        }
    }
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        if !crash_save(db, Some(o), &mut fp, 0) {
            return;
        }
    }
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        crash_extract_objs(game, db, Some(o));
    }
}

fn crash_cryosave(game: &mut Game, db: &mut DB, chid: DepotId, cost: i32) {
    let ch = db.ch(chid);

    let mut buf = String::new();
    let mut rent = RentInfo::new();

    if ch.is_npc() {
        return;
    }

    if !get_filename(&mut buf, CRASH_FILE, ch.get_name().as_ref()) {
        return;
    }
    let fp = OpenOptions::new().create(true).write(true).open(buf);
    if fp.is_err() {
        return;
    }
    let mut fp = fp.unwrap();

    crash_extract_norent_eq(game, db, chid);
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        crash_extract_norents(game, db, o);
    }
    let ch = db.ch_mut(chid);
    ch.set_gold(max(0, ch.get_gold() - cost));

    rent.rentcode = RENT_CRYO;
    rent.time = time_now() as i64;
    rent.gold = ch.get_gold();
    rent.account = ch.get_bank_gold();
    rent.net_cost_per_diem = 0;
    if !crash_write_rentcode(chid, &mut fp, &mut rent) {
        return;
    }
    for j in 0..NUM_WEARS {
        let ch = db.ch(chid);
        if ch.get_eq(j).is_some() {
            let oid = ch.get_eq(j);
            if !crash_save(db, oid, &mut fp, (j + 1) as i32) {
                return;
            }
            let ch = db.ch(chid);
            let oid = ch.get_eq(j).unwrap();
            crash_restore_weight(db, oid);
            let ch = db.ch(chid);
            crash_extract_objs(game, db, ch.get_eq(j));
        }
    }
    let mut j = 0;
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        if !crash_save(db, Some(o), &mut fp, j) {
            return;
        }
        j += 1;
    }
    let ch = db.ch(chid);
    for o in ch.carrying.clone() {
        crash_extract_objs(game, db, Some(o));
    }
    let ch = db.ch_mut(chid);
    ch.set_plr_flag_bit(PLR_CRYO);
}

/* ************************************************************************
* Routines used for the receptionist					  *
************************************************************************* */

fn crash_rent_deadline(game: &mut Game, db: &mut DB, chid: DepotId, recep_id: DepotId, cost: i32) {
    let recep = db.ch(recep_id);
    let ch = db.ch(chid);
    if cost == 0 {
        return;
    }

    let rent_deadline = (ch.get_gold() + ch.get_bank_gold()) / cost;
    let buf = format!(
        "$n tells you, 'You can rent for {} day{} with the gold you have\r\n\
on hand and in the bank.'\r\n",
        rent_deadline,
        if rent_deadline != 1 { "s" } else { "" }
    );
    game.act(
        db,
        &buf,
        false,
        Some(recep),
        None,
        Some(VictimRef::Char(ch)),
        TO_VICT,
    );
}

fn crash_report_unrentables(
    game: &mut Game,
    db: &DB,
    chid: DepotId,
    recep_id: DepotId,
    oid: DepotId,
) -> i32 {
    let ch = db.ch(chid);
    let recep = db.ch(recep_id);
    let mut has_norents = 0;

    if crash_is_unrentable(db.obj(oid)) {
        has_norents = 1;
        let buf = format!(
            "$n tells you, 'You cannot store {}.'",
            game.objs(db, db.obj(oid), ch)
        );
        game.act(
            db,
            &buf,
            false,
            Some(recep),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
    }
    for o in db.obj(oid).contains.clone() {
        has_norents += crash_report_unrentables(game, db, chid, recep_id, o);
    }

    has_norents
}

fn crash_report_rent(
    game: &mut Game,
    db: &DB,
    chid: DepotId,
    recep_id: DepotId,
    oid: DepotId,
    cost: &mut i32,
    nitems: &mut i64,
    display: bool,
    factor: i32,
) {
    let ch = db.ch(chid);
    let recep = db.ch(recep_id);
    if !crash_is_unrentable(db.obj(oid)) {
        *nitems += 1;
        *cost += max(0, db.obj(oid).get_obj_rent() * factor);
        if display {
            let buf = format!(
                "$n tells you, '{:5} coins for {}..'",
                db.obj(oid).get_obj_rent() * factor,
                game.objs(db, db.obj(oid), ch)
            );
            game.act(
                db,
                &buf,
                false,
                Some(recep),
                None,
                Some(VictimRef::Char(ch)),
                TO_VICT,
            );
        }
    }
    for &o in &db.obj(oid).contains {
        crash_report_rent(game, db, chid, recep_id, o, cost, nitems, display, factor);
    }
}

fn crash_offer_rent(
    game: &mut Game,
    db: &mut DB,
    chid: DepotId,
    recep_id: DepotId,
    display: bool,
    factor: i32,
) -> i32 {
    let recep = db.ch(recep_id);
    let ch = db.ch(chid);
    let mut numitems = 0;

    let mut norent = 0;
    for &o in &ch.carrying {
        norent += crash_report_unrentables(game, db, chid, recep_id, o);
    }
    for i in 0..NUM_WEARS {
        let ch = db.ch(chid);
        let eqi = ch.get_eq(i);
        if eqi.is_none() {
            continue;
        }
        norent += crash_report_unrentables(game, db, chid, recep_id, eqi.unwrap());
    }
    if norent != 0 {
        return 0;
    }

    let mut totalcost = MIN_RENT_COST * factor;
    let ch = db.ch(chid);
    for &o in &ch.carrying {
        crash_report_rent(
            game,
            db,
            chid,
            recep_id,
            o,
            &mut totalcost,
            &mut numitems,
            display,
            factor,
        );
    }
    for i in 0..NUM_WEARS {
        let ch = db.ch(chid);
        let eqi = ch.get_eq(i);
        if eqi.is_none() {
            continue;
        }
        crash_report_rent(
            game,
            db,
            chid,
            recep_id,
            ch.get_eq(i).unwrap(),
            &mut totalcost,
            &mut numitems,
            display,
            factor,
        );
    }

    if numitems == 0 {
        game.act(
            db,
            "$n tells you, 'But you are not carrying anything!  Just quit!'",
            false,
            Some(recep),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        return 0;
    }
    if numitems > MAX_OBJ_SAVE as i64 {
        let buf = format!(
            "$n tells you, 'Sorry, but I cannot store more than {} items.'",
            MAX_OBJ_SAVE
        );
        game.act(
            db,
            &buf,
            false,
            Some(recep),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        return 0;
    }
    if display {
        let buf = format!(
            "$n tells you, 'Plus, my {} coin fee..'",
            MIN_RENT_COST * factor
        );
        game.act(
            db,
            &buf,
            false,
            Some(recep),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );

        let buf = format!(
            "$n tells you, 'For a total of {} coins{}.'",
            totalcost,
            if factor == RENT_FACTOR {
                " per day"
            } else {
                ""
            }
        );
        game.act(
            db,
            &buf,
            false,
            Some(recep),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );

        let ch = db.ch(chid);
        if totalcost > ch.get_gold() + ch.get_bank_gold() {
            game.act(
                db,
                "$n tells you, '...which I see you can't afford.'",
                false,
                Some(recep),
                None,
                Some(VictimRef::Char(ch)),
                TO_VICT,
            );
            return 0;
        } else if factor == RENT_FACTOR {
            crash_rent_deadline(game, db, chid, recep_id, totalcost);
        }
    }
    totalcost
}

fn gen_receptionist(
    game: &mut Game,
    db: &mut DB,
    chid: DepotId,
    recep_id: DepotId,
    cmd: i32,
    _arg: &str,
    mode: i32,
) -> bool {
    let ch = db.ch(chid);
    let recep = db.ch(recep_id);
    const ACTION_TABLE: [&str; 9] = [
        "smile", "dance", "sigh", "blush", "burp", "cough", "fart", "twiddle", "yawn",
    ];

    if cmd == 0 && rand_number(0, 5) == 0 {
        do_action(
            game,
            db,
            recep_id,
            "",
            find_command(ACTION_TABLE[rand_number(0, 8) as usize]).unwrap(),
            0,
        );
        return false;
    }

    if ch.desc.is_none() || ch.is_npc() {
        return false;
    }

    if !cmd_is(cmd, "offer") && !cmd_is(cmd, "rent") {
        return false;
    }

    if recep.awake() {
        game.send_to_char(
            ch,
            format!("{} is unable to talk to you...\r\n", hssh(recep)).as_str(),
        );
        return true;
    }

    if !game.can_see(db, recep, ch) {
        game.act(
            db,
            "$n says, 'I don't deal with people I can't see!'",
            false,
            Some(recep),
            None,
            None,
            TO_ROOM,
        );
        return true;
    }

    if FREE_RENT {
        game.act(
            db,
            "$n tells you, 'Rent is free here.  Just quit, and your objects will be saved!'",
            false,
            Some(recep),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        return true;
    }

    if cmd_is(cmd, "rent") {
        let cost = crash_offer_rent(game, db, chid, recep_id, false, mode);
        if cost == 0 {
            return true;
        }
        let mut buf = String::new();
        if mode == RENT_FACTOR {
            buf = format!(
                "$n tells you, 'Rent will cost you {} gold coins per day.'",
                cost
            );
        } else if mode == CRYO_FACTOR {
            buf = format!(
                "$n tells you, 'It will cost you {} gold coins to be frozen.'",
                cost
            );
        }
        let recep = db.ch(recep_id);
        let ch = db.ch(chid);
        game.act(
            db,
            &buf,
            false,
            Some(recep),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        let ch = db.ch(chid);
        if cost > ch.get_gold() + ch.get_bank_gold() {
            game.act(
                db,
                "$n tells you, '...which I see you can't afford.'",
                false,
                Some(recep),
                None,
                Some(VictimRef::Char(ch)),
                TO_VICT,
            );
            return true;
        }
        if cost != 0 && (mode == RENT_FACTOR) {
            crash_rent_deadline(game, db, chid, recep_id, cost);
        }

        if mode == RENT_FACTOR {
            let recep = db.ch(recep_id);
            let ch = db.ch(chid);
            game.act(
                db,
                "$n stores your belongings and helps you into your private chamber.",
                false,
                Some(recep),
                None,
                Some(VictimRef::Char(ch)),
                TO_VICT,
            );
            crash_rentsave(game, db, chid, cost);
            let ch = db.ch(chid);
            game.mudlog(
                db,
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "{} has rented ({}/day, {} tot.)",
                    ch.get_name(),
                    cost,
                    ch.get_gold() + ch.get_bank_gold()
                )
                .as_str(),
            );
        } else {
            /* cryo */
            let ch = db.ch(chid);
            let recep = db.ch(recep_id);
            game.act(
                db,
                "$n stores your belongings and helps you into your private chamber.\r\n\
A white mist appears in the room, chilling you to the bone...\r\n\
You begin to lose consciousness...",
                false,
                Some(recep),
                None,
                Some(VictimRef::Char(ch)),
                TO_VICT,
            );
            crash_cryosave(game, db, chid, cost);
            let ch = db.ch(chid);
            game.mudlog(
                db,
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!("{} has cryo-rented.", ch.get_name()).as_str(),
            );
            let ch = db.ch_mut(chid);
            ch.set_plr_flag_bit(PLR_CRYO);
        }
        let ch = db.ch(chid);
        let recep = db.ch(recep_id);
        game.act(
            db,
            "$n helps $N into $S private chamber.",
            false,
            Some(recep),
            None,
            Some(VictimRef::Char(ch)),
            TO_NOTVICT,
        );
        let ch = db.ch(chid);
        let val = db.get_room_vnum(ch.in_room());
        let ch = db.ch_mut(chid);
        ch.set_loadroom(val);
        db.extract_char(chid); /* It saves. */
    } else {
        crash_offer_rent(game, db, chid, recep_id, true, mode);
        let recep = db.ch(recep_id);
        let ch = db.ch(chid);
        game.act(
            db,
            "$N gives $n an offer.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(recep)),
            TO_ROOM,
        );
    }
    true
}

pub fn receptionist(
    game: &mut Game,
    db: &mut DB,
    chid: DepotId,
    me: MeRef,
    cmd: i32,
    argument: &str,
) -> bool {
    match me {
        MeRef::Char(recep) => gen_receptionist(game, db, chid, recep, cmd, argument, RENT_FACTOR),
        _ => panic!("Unexpected MeRef type in receptionist"),
    }
}

pub fn cryogenicist(
    game: &mut Game,
    db: &mut DB,
    chid: DepotId,
    me: MeRef,
    cmd: i32,
    argument: &str,
) -> bool {
    match me {
        MeRef::Char(recep) => gen_receptionist(game, db, chid, recep, cmd, argument, CRYO_FACTOR),
        _ => panic!("Unexpected MeRef type in cryogenicist"),
    }
}

pub fn crash_save_all(game: &mut Game, db: &mut DB) {
    for d in game.descriptor_list.iter() {
        if d.state() == ConPlaying && !db.ch(d.character.unwrap()).is_npc() {
            if db.ch(d.character.unwrap()).plr_flagged(PLR_CRASH) {
                crash_crashsave(db, d.character.unwrap());
                db.ch_mut(d.character.unwrap()).remove_plr_flag(PLR_CRASH);
            }
        }
    }
}
