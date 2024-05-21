/* ************************************************************************
*   File: objsave.rs                                    Part of CircleMUD *
*  Usage: loading/saving player objects for rent and crash-save           *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::any::Any;
use std::cell::RefCell;
use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, Write};
use std::path::Path;
use std::rc::Rc;
use std::{fs, mem, slice};

use log::{error, info};

use crate::act_social::do_action;
use crate::class::invalid_class;
use crate::config::{
    CRASH_FILE_TIMEOUT, FREE_RENT, MAX_OBJ_SAVE, MIN_RENT_COST, RENT_FILE_TIMEOUT,
};
use crate::db::{DB, REAL, VIRTUAL};
use crate::handler::{invalid_align, obj_from_char};
use crate::interpreter::{cmd_is, find_command};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    CharData, ObjAffectedType, ObjData, ObjFileElem, RentInfo, ITEM_CONTAINER, ITEM_KEY,
    ITEM_NORENT, ITEM_WEAPON, ITEM_WEAR_ABOUT, ITEM_WEAR_ARMS, ITEM_WEAR_BODY, ITEM_WEAR_FEET,
    ITEM_WEAR_FINGER, ITEM_WEAR_HANDS, ITEM_WEAR_HEAD, ITEM_WEAR_HOLD, ITEM_WEAR_LEGS,
    ITEM_WEAR_NECK, ITEM_WEAR_SHIELD, ITEM_WEAR_WAIST, ITEM_WEAR_WIELD, ITEM_WEAR_WRIST, LVL_GOD,
    LVL_IMMORT, MAX_OBJ_AFFECT, NOTHING, NUM_WEARS, PLR_CRASH, PLR_CRYO, RENT_CRASH, RENT_CRYO,
    RENT_FORCED, RENT_RENTED, RENT_TIMEDOUT, WEAR_ABOUT, WEAR_ARMS, WEAR_BODY, WEAR_FEET,
    WEAR_FINGER_L, WEAR_FINGER_R, WEAR_HANDS, WEAR_HEAD, WEAR_HOLD, WEAR_LEGS, WEAR_LIGHT,
    WEAR_NECK_1, WEAR_NECK_2, WEAR_SHIELD, WEAR_WAIST, WEAR_WIELD, WEAR_WRIST_L, WEAR_WRIST_R,
};
use crate::util::{
    clone_vec, get_filename, hssh, rand_number, time_now, BRF, CRASH_FILE, NRM, SECS_PER_REAL_DAY,
};
use crate::{send_to_char, Game, TO_NOTVICT, TO_ROOM, TO_VICT};

/* these factors should be unique integers */
const RENT_FACTOR: i32 = 1;
const CRYO_FACTOR: i32 = 4;

pub const LOC_INVENTORY: i32 = 0;
pub const MAX_BAG_ROWS: i32 = 5;

pub fn obj_from_store(db: &DB, object: &ObjFileElem, location: &mut i32) -> Option<Rc<ObjData>> {
    *location = 0;
    let itemnum = db.real_object(object.item_number);
    if itemnum == NOTHING {
        return None;
    }

    let obj = db.read_object(itemnum, REAL).unwrap();
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
        obj.affected[j].set(object.affected[j]);
    }
    Some(obj)
}

pub fn obj_to_store(db: &DB, obj: &Rc<ObjData>, fl: &mut File, location: i32) -> bool {
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
        object.affected[i] = obj.affected[i].get();
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
fn auto_equip(game: &Game, ch: &Rc<CharData>, obj: &Rc<ObjData>, location: i32) {
    let mut location = location;
    let db = &game.db;
    /* Lots of checks... */
    let mut j = 0;
    if location > 0 {
        /* Was wearing it. */
        j = location - 1;
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
            if invalid_align(ch, obj) || invalid_class(ch, obj) {
                location = LOC_INVENTORY;
            } else {
                db.equip_char(ch, obj, j as i8);
            }
        } else {
            /* Oops, saved a player with double equipment? */
            game.mudlog(
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
        DB::obj_to_char(obj, ch);
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

pub fn crash_delete_crashfile(ch: &Rc<CharData>) -> bool {
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
    for i in 0..db.player_table.borrow().len() {
        if !db.player_table.borrow()[i].name.is_empty() {
            crash_clean_file(&db.player_table.borrow()[i].name);
        }
    }
}

pub fn crash_listrent(db: &DB, ch: &Rc<CharData>, name: &str) {
    let mut filename = String::new();
    if !get_filename(&mut filename, CRASH_FILE, name) {
        return;
    }
    let fl = OpenOptions::new().read(true).open(&filename);
    if fl.is_err() {
        send_to_char(ch, format!("{} has no rent file.\r\n", name).as_str());
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
        send_to_char(ch, "Error reading rent information.\r\n");
        return;
    }

    send_to_char(ch, format!("{}\r\n", filename).as_str());
    match rent.rentcode {
        RENT_RENTED => {
            send_to_char(ch, "Rent\r\n");
        }
        RENT_CRASH => {
            send_to_char(ch, "Crash\r\n");
        }
        RENT_CRYO => {
            send_to_char(ch, "Cryo\r\n");
        }
        RENT_TIMEDOUT | RENT_FORCED => {
            send_to_char(ch, "TimedOut\r\n");
        }
        _ => {
            send_to_char(ch, "Undef\r\n");
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
            let obj = db.read_object(object.item_number, VIRTUAL);
            // #if USE_AUTOEQ
            // send_to_char(ch, " [%5d] (%5dau) <%2d> %-20s\r\n",
            // object.item_number, obj.get_obj_rent(),
            // object.location, obj->short_description);
            // #else
            let oin = object.item_number;
            send_to_char(
                ch,
                format!(
                    " [{:5}] ({:5}au) {:20}\r\n",
                    oin,
                    obj.as_ref().unwrap().get_obj_rent(),
                    obj.as_ref().unwrap().short_description
                )
                .as_str(),
            );
            // #endif
            db.extract_obj(obj.as_ref().unwrap());
        }
    }
}

fn crash_write_rentcode(_ch: &Rc<CharData>, fl: &mut File, rent: &mut RentInfo) -> bool {
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
pub fn crash_load(game: &Game, ch: &Rc<CharData>) -> i32 {
    let db = &game.db;
    /* AutoEQ addition. */
    let mut location = 0;
    let mut j;
    let mut cont_row: [Vec<Rc<ObjData>>; MAX_BAG_ROWS as usize] =
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
            send_to_char(ch,
                         "\r\n********************* NOTICE *********************\r\nThere was a problem loading your objects from disk.\r\nContact a God for assistance.\r\n");
        }
        game.mudlog(
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
                BRF,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!(
                    "{} entering game, rented equipment lost (no $).",
                    ch.get_name()
                )
                .as_str(),
            );
            crash_crashsave(db, ch);
            return 2;
        } else {
            ch.set_bank_gold(ch.get_bank_gold() - max(cost - ch.get_gold(), 0));
            ch.set_gold(max(ch.get_gold() - cost, 0));
            db.save_char(ch);
        }
    }
    let mut num_objs = 0;
    let orig_rent_code = rent.rentcode;
    match orig_rent_code {
        RENT_RENTED => {
            game.mudlog(
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!("{} un-renting and entering game.", ch.get_name()).as_str(),
            );
        }
        RENT_CRASH => {
            game.mudlog(
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
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!("{} un-cryo'ing and entering game.", ch.get_name()).as_str(),
            );
        }
        RENT_FORCED | RENT_TIMEDOUT => {
            game.mudlog(
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
        let mut obj = obj_from_store(db, &object, &mut location);
        if obj.is_none() {
            continue;
        }

        auto_equip(game, ch, obj.as_ref().unwrap(), location);
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
                        DB::obj_to_char(obj2, ch);
                    }
                    cont_row[j].clear();
                }
                j -= 1;
            }
            if cont_row[0].len() != 0 {
                /* Content list existing. */
                if obj.as_ref().unwrap().get_obj_type() == ITEM_CONTAINER {
                    /* Remove object, fill it, equip again. */
                    obj = db.unequip_char(ch, (location - 1) as i8);
                    obj.as_ref().unwrap().contains.borrow_mut().clear(); /* Should be empty anyway, but just in case. */
                    for obj2 in cont_row[0].iter() {
                        db.obj_to_obj(&obj2, obj.as_ref().unwrap());
                    }
                    db.equip_char(ch, obj.as_ref().unwrap(), (location - 1) as i8);
                } else {
                    /* Object isn't container, empty the list. */
                    for obj2 in cont_row[0].iter() {
                        DB::obj_to_char(&obj2, ch);
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
                        DB::obj_to_char(&obj2, ch);
                    }
                    cont_row[j].clear();
                }
                j -= 1;
            }
            if j == -location as usize && cont_row[j].len() != 0 {
                /* Content list exists. */
                if obj.as_ref().unwrap().get_obj_type() == ITEM_CONTAINER {
                    /* Take the item, fill it, and give it back. */
                    obj_from_char(obj.as_ref().unwrap());
                    obj.as_ref().unwrap().contains.borrow_mut().clear();
                    for obj2 in cont_row[j].iter() {
                        db.obj_to_obj(&obj2, obj.as_ref().unwrap());
                    }
                    DB::obj_to_char(obj.as_ref().unwrap(), ch); /* Add to inventory first. */
                } else {
                    /* Object isn't container, empty content list. */
                    for obj2 in cont_row[j].iter() {
                        DB::obj_to_char(&obj2, ch);
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
    game.mudlog(
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
    crash_write_rentcode(ch, &mut fl, &mut rent);

    return if (orig_rent_code == RENT_RENTED) || (orig_rent_code == RENT_CRYO) {
        0
    } else {
        1
    };
}

fn crash_save(db: &DB, obj: Option<&Rc<ObjData>>, fp: &mut File, location: i32) -> bool {
    let location = location;
    let result;
    if obj.is_some() {
        result = obj_to_store(db, obj.as_ref().unwrap(), fp, location);
        for o in obj.as_ref().unwrap().contains.borrow().iter() {
            crash_save(db, Some(o), fp, location - 1);
        }

        let mut t = obj.unwrap().clone();

        loop {
            if t.in_obj.borrow().is_none() {
                break;
            }

            t.in_obj
                .borrow()
                .as_ref()
                .unwrap()
                .incr_obj_weight(-obj.as_ref().unwrap().get_obj_weight());
            let k;
            {
                let y = t.in_obj.borrow();
                k = y.as_ref().unwrap().clone();
            }
            t = k;
        }

        if !result {
            return false;
        }
    }
    true
}

fn crash_restore_weight(obj: &Rc<ObjData>) {
    for o in obj.contains.borrow().iter() {
        crash_restore_weight(o);
    }
    if obj.in_obj.borrow().is_some() {
        obj.in_obj
            .borrow_mut()
            .as_mut()
            .unwrap()
            .incr_obj_weight(obj.get_obj_weight());
    }
}

/*
 * Get !RENT items from equipment to inventory and
 * extract !RENT out of worn containers.
 */
fn crash_extract_norent_eq(db: &DB, ch: &Rc<CharData>) {
    for j in 0..NUM_WEARS {
        if ch.get_eq(j).is_none() {
            continue;
        }
        if crash_is_unrentable(&ch.get_eq(j).unwrap()) {
            DB::obj_to_char(db.unequip_char(ch, j).as_ref().unwrap(), ch);
        } else {
            crash_extract_norents(db, &ch.get_eq(j).unwrap());
        }
    }
}

fn crash_extract_objs(db: &DB, obj: Option<&Rc<ObjData>>) {
    let obj = obj.unwrap();
    for o in obj.contains.borrow().iter() {
        crash_extract_objs(db, Some(o));
    }
    db.extract_obj(obj);
}

fn crash_is_unrentable(obj: &Rc<ObjData>) -> bool {
    if obj.obj_flagged(ITEM_NORENT)
        || obj.get_obj_rent() < 0
        || obj.get_obj_rnum() == NOTHING
        || obj.get_obj_type() == ITEM_KEY
    {
        return true;
    }
    false
}

fn crash_extract_norents(db: &DB, obj: &Rc<ObjData>) {
    for o in clone_vec(&obj.contains) {
        crash_extract_norents(db, &o);
    }

    if crash_is_unrentable(obj) {
        db.extract_obj(obj);
    }
}

fn crash_extract_expensive(db: &DB, objs: &RefCell<Vec<Rc<ObjData>>>) {
    if objs.borrow().len() == 0 {
        return;
    }
    let mut max = objs.borrow()[0].clone();

    for tobj in objs.borrow().iter() {
        if tobj.get_obj_rent() > max.get_obj_rent() {
            max = tobj.clone();
        }
    }

    db.extract_obj(&max);
}

fn crash_calculate_rent(obj: Option<&Rc<ObjData>>, cost: &mut i32) {
    if obj.is_some() {
        let obj = obj.unwrap();
        *cost += max(0, obj.get_obj_rent());
        for o in obj.contains.borrow().iter() {
            crash_calculate_rent(Some(o), cost);
        }
    }
}

pub fn crash_crashsave(db: &DB, ch: &Rc<CharData>) {
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

    if !crash_write_rentcode(ch, &mut fp, &mut rent) {
        return;
    }

    for j in 0..NUM_WEARS as usize {
        if ch.get_eq(j as i8).is_some() {
            if !crash_save(db, ch.get_eq(j as i8).as_ref(), &mut fp, (j + 1) as i32) {
                return;
            }
            crash_restore_weight(ch.get_eq(j as i8).as_ref().unwrap());
        }
    }

    for o in ch.carrying.borrow().iter() {
        if !crash_save(db, Some(o), &mut fp, 0) {
            return;
        }
    }

    for o in ch.carrying.borrow().iter() {
        crash_restore_weight(o);
    }

    ch.remove_plr_flag(PLR_CRASH);
}

pub fn crash_idlesave(db: &DB, ch: &Rc<CharData>) {
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

    crash_extract_norent_eq(db, ch);
    for o in ch.carrying.borrow().iter() {
        crash_extract_norents(db, o);
    }

    let mut cost = 0;
    for o in ch.carrying.borrow().iter() {
        crash_calculate_rent(Some(o), &mut cost);
    }

    let mut cost_eq = 0;
    for j in 0..NUM_WEARS {
        crash_calculate_rent(ch.get_eq(j).as_ref(), &mut cost_eq);
    }

    cost += cost_eq;
    cost *= 2; /* forcerent cost is 2x normal rent */

    if cost > ch.get_gold() + ch.get_bank_gold() {
        for j in 0..NUM_WEARS {
            /* Unequip players with low gold. */
            if ch.get_eq(j).is_some() {
                DB::obj_to_char(db.unequip_char(ch, j).as_ref().unwrap(), ch);
            }
        }

        while (cost > ch.get_gold() + ch.get_bank_gold()) && ch.carrying.borrow().len() != 0 {
            crash_extract_expensive(db, &ch.carrying);
            cost = 0;
            for o in ch.carrying.borrow().iter() {
                crash_calculate_rent(Some(o), &mut cost);
            }
            cost *= 2;
        }
    }

    if ch.carrying.borrow().len() == 0 {
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
    if !crash_write_rentcode(ch, &mut fp, &mut rent) {
        return;
    }
    for j in 0..NUM_WEARS {
        if ch.get_eq(j).is_some() {
            if !crash_save(db, ch.get_eq(j).as_ref(), &mut fp, (j + 1) as i32) {
                return;
            }
            crash_restore_weight(ch.get_eq(j).as_ref().unwrap());
            crash_extract_objs(db, ch.get_eq(j).as_ref());
        }
    }
    let mut location = 0;
    for o in ch.carrying.borrow().iter() {
        if !crash_save(db, Some(o), &mut fp, location) {
            return;
        }
        location += 1;
    }

    for o in ch.carrying.borrow().iter() {
        crash_extract_objs(db, Some(o));
    }
}

pub fn crash_rentsave(db: &DB, ch: &Rc<CharData>, cost: i32) {
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

    crash_extract_norent_eq(db, ch);
    for o in clone_vec(&ch.carrying) {
        crash_extract_norents(db, &o);
    }

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

    if !crash_write_rentcode(ch, &mut fp, &mut rent) {
        return;
    }

    for j in 0..NUM_WEARS {
        if ch.get_eq(j).is_some() {
            if !crash_save(db, ch.get_eq(j).as_ref(), &mut fp, (j + 1) as i32) {
                return;
            }

            crash_restore_weight(ch.get_eq(j).as_ref().unwrap());
            crash_extract_objs(db, ch.get_eq(j).as_ref());
        }
    }

    for o in ch.carrying.borrow().iter() {
        if !crash_save(db, Some(o), &mut fp, 0) {
            return;
        }
    }
    for o in clone_vec(&ch.carrying).iter() {
        crash_extract_objs(db, Some(o));
    }
}

fn crash_cryosave(db: &DB, ch: &Rc<CharData>, cost: i32) {
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

    crash_extract_norent_eq(db, ch);
    for o in ch.carrying.borrow().iter() {
        crash_extract_norents(db, o);
    }

    ch.set_gold(max(0, ch.get_gold() - cost));

    rent.rentcode = RENT_CRYO;
    rent.time = time_now() as i64;
    rent.gold = ch.get_gold();
    rent.account = ch.get_bank_gold();
    rent.net_cost_per_diem = 0;
    if !crash_write_rentcode(ch, &mut fp, &mut rent) {
        return;
    }
    for j in 0..NUM_WEARS {
        if ch.get_eq(j).is_some() {
            if !crash_save(db, ch.get_eq(j).as_ref(), &mut fp, (j + 1) as i32) {
                return;
            }
            crash_restore_weight(ch.get_eq(j).as_ref().unwrap());
            crash_extract_objs(db, ch.get_eq(j).as_ref());
        }
    }
    let mut j = 0;
    for o in ch.carrying.borrow().iter() {
        if !crash_save(db, Some(o), &mut fp, j) {
            return;
        }
        j += 1;
    }

    for o in ch.carrying.borrow().iter() {
        crash_extract_objs(db, Some(o));
    }
    ch.set_plr_flag_bit(PLR_CRYO);
}

/* ************************************************************************
* Routines used for the receptionist					  *
************************************************************************* */

fn crash_rent_deadline(db: &DB, ch: &Rc<CharData>, recep: &Rc<CharData>, cost: i32) {
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
    db.act(&buf, false, Some(recep), None, Some(ch), TO_VICT);
}

fn crash_report_unrentables(
    db: &DB,
    ch: &Rc<CharData>,
    recep: &Rc<CharData>,
    obj: &Rc<ObjData>,
) -> i32 {
    let mut has_norents = 0;

    if crash_is_unrentable(obj) {
        has_norents = 1;
        let buf = format!("$n tells you, 'You cannot store {}.'", db.objs(obj, ch));
        db.act(&buf, false, Some(recep), None, Some(ch), TO_VICT);
    }
    for o in obj.contains.borrow().iter() {
        has_norents += crash_report_unrentables(db, ch, recep, o);
    }

    has_norents
}

fn crash_report_rent(
    db: &DB,
    ch: &Rc<CharData>,
    recep: &Rc<CharData>,
    obj: &Rc<ObjData>,
    cost: &mut i32,
    nitems: &mut i64,
    display: bool,
    factor: i32,
) {
    if !crash_is_unrentable(obj) {
        *nitems += 1;
        *cost += max(0, obj.get_obj_rent() * factor);
        if display {
            let buf = format!(
                "$n tells you, '{:5} coins for {}..'",
                obj.get_obj_rent() * factor,
                db.objs(obj, ch)
            );
            db.act(&buf, false, Some(recep), None, Some(ch), TO_VICT);
        }
    }
    for o in obj.contains.borrow().iter() {
        crash_report_rent(db, ch, recep, o, cost, nitems, display, factor);
    }
}

fn crash_offer_rent(
    db: &DB,
    ch: &Rc<CharData>,
    recep: &Rc<CharData>,
    display: bool,
    factor: i32,
) -> i32 {
    let mut numitems = 0;

    let mut norent = 0;
    for o in ch.carrying.borrow().iter() {
        norent += crash_report_unrentables(db, ch, recep, o);
    }
    for i in 0..NUM_WEARS {
        let eqi: Option<Rc<ObjData>> = ch.get_eq(i);
        if eqi.is_none() {
            continue;
        }
        norent += crash_report_unrentables(db, ch, recep, &eqi.unwrap());
    }
    if norent != 0 {
        return 0;
    }

    let mut totalcost = MIN_RENT_COST * factor;

    for o in ch.carrying.borrow().iter() {
        crash_report_rent(
            db,
            ch,
            recep,
            o,
            &mut totalcost,
            &mut numitems,
            display,
            factor,
        );
    }
    for i in 0..NUM_WEARS {
        let eqi: Option<Rc<ObjData>> = ch.get_eq(i);
        if eqi.is_none() {
            continue;
        }
        crash_report_rent(
            db,
            ch,
            recep,
            &ch.get_eq(i).unwrap(),
            &mut totalcost,
            &mut numitems,
            display,
            factor,
        );
    }

    if numitems == 0 {
        db.act(
            "$n tells you, 'But you are not carrying anything!  Just quit!'",
            false,
            Some(recep),
            None,
            Some(ch),
            TO_VICT,
        );
        return 0;
    }
    if numitems > MAX_OBJ_SAVE as i64 {
        let buf = format!(
            "$n tells you, 'Sorry, but I cannot store more than {} items.'",
            MAX_OBJ_SAVE
        );
        db.act(&buf, false, Some(recep), None, Some(ch), TO_VICT);
        return 0;
    }
    if display {
        let buf = format!(
            "$n tells you, 'Plus, my {} coin fee..'",
            MIN_RENT_COST * factor
        );
        db.act(&buf, false, Some(recep), None, Some(ch), TO_VICT);

        let buf = format!(
            "$n tells you, 'For a total of {} coins{}.'",
            totalcost,
            if factor == RENT_FACTOR {
                " per day"
            } else {
                ""
            }
        );
        db.act(&buf, false, Some(recep), None, Some(ch), TO_VICT);

        if totalcost > ch.get_gold() + ch.get_bank_gold() {
            db.act(
                "$n tells you, '...which I see you can't afford.'",
                false,
                Some(recep),
                None,
                Some(ch),
                TO_VICT,
            );
            return 0;
        } else if factor == RENT_FACTOR {
            crash_rent_deadline(db, ch, recep, totalcost);
        }
    }
    totalcost
}

fn gen_receptionist(
    game: &mut Game,
    ch: &Rc<CharData>,
    recep: &Rc<CharData>,
    cmd: i32,
    _arg: &str,
    mode: i32,
) -> bool {
    let db = &game.db;
    const ACTION_TABLE: [&str; 9] = [
        "smile", "dance", "sigh", "blush", "burp", "cough", "fart", "twiddle", "yawn",
    ];

    if cmd == 0 && rand_number(0, 5) == 0 {
        do_action(
            game,
            recep,
            "",
            find_command(ACTION_TABLE[rand_number(0, 8) as usize]).unwrap(),
            0,
        );
        return false;
    }

    if ch.desc.borrow().is_none() || ch.is_npc() {
        return false;
    }

    if !cmd_is(cmd, "offer") && !cmd_is(cmd, "rent") {
        return false;
    }

    if recep.awake() {
        send_to_char(
            ch,
            format!("{} is unable to talk to you...\r\n", hssh(recep)).as_str(),
        );
        return true;
    }

    if !db.can_see(recep, ch) {
        db.act(
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
        db.act(
            "$n tells you, 'Rent is free here.  Just quit, and your objects will be saved!'",
            false,
            Some(recep),
            None,
            Some(ch),
            TO_VICT,
        );
        return true;
    }

    if cmd_is(cmd, "rent") {
        let cost = crash_offer_rent(db, ch, recep, false, mode);
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
        db.act(&buf, false, Some(recep), None, Some(ch), TO_VICT);

        if cost > ch.get_gold() + ch.get_bank_gold() {
            db.act(
                "$n tells you, '...which I see you can't afford.'",
                false,
                Some(recep),
                None,
                Some(ch),
                TO_VICT,
            );
            return true;
        }
        if cost != 0 && (mode == RENT_FACTOR) {
            crash_rent_deadline(db, ch, recep, cost);
        }

        if mode == RENT_FACTOR {
            db.act(
                "$n stores your belongings and helps you into your private chamber.",
                false,
                Some(recep),
                None,
                Some(ch),
                TO_VICT,
            );
            crash_rentsave(db, ch, cost);
            game.mudlog(
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
            db.act(
                "$n stores your belongings and helps you into your private chamber.\r\n\
A white mist appears in the room, chilling you to the bone...\r\n\
You begin to lose consciousness...",
                false,
                Some(recep),
                None,
                Some(ch),
                TO_VICT,
            );
            crash_cryosave(db, ch, cost);
            game.mudlog(
                NRM,
                max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
                true,
                format!("{} has cryo-rented.", ch.get_name()).as_str(),
            );
            ch.set_plr_flag_bit(PLR_CRYO);
        }

        db.act(
            "$n helps $N into $S private chamber.",
            false,
            Some(recep),
            None,
            Some(ch),
            TO_NOTVICT,
        );

        ch.set_loadroom(db.get_room_vnum(ch.in_room()));
        db.extract_char(ch); /* It saves. */
    } else {
        crash_offer_rent(db, ch, recep, true, mode);
        db.act(
            "$N gives $n an offer.",
            false,
            Some(ch),
            None,
            Some(recep),
            TO_ROOM,
        );
    }
    true
}

pub fn receptionist(
    game: &mut Game,
    ch: &Rc<CharData>,
    me: &dyn Any,
    cmd: i32,
    argument: &str,
) -> bool {
    return gen_receptionist(
        game,
        ch,
        me.downcast_ref::<Rc<CharData>>().unwrap(),
        cmd,
        argument,
        RENT_FACTOR,
    );
}

pub fn cryogenicist(
    game: &mut Game,
    ch: &Rc<CharData>,
    me: &dyn Any,
    cmd: i32,
    argument: &str,
) -> bool {
    return gen_receptionist(
        game,
        ch,
        me.downcast_ref::<Rc<CharData>>().unwrap(),
        cmd,
        argument,
        CRYO_FACTOR,
    );
}

pub fn crash_save_all(game: &Game) {
    for d in game.descriptor_list.borrow().iter() {
        if d.state() == ConPlaying && !d.character.borrow().as_ref().unwrap().is_npc() {
            if d.character
                .borrow()
                .as_ref()
                .unwrap()
                .plr_flagged(PLR_CRASH)
            {
                crash_crashsave(&game.db, d.character.borrow().as_ref().unwrap());
                d.character
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .remove_plr_flag(PLR_CRASH);
            }
        }
    }
}
