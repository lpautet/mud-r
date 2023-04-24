/* ************************************************************************
*   File: objsave.c                                     Part of CircleMUD *
*  Usage: loading/saving player objects for rent and crash-save           *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, Write};
use std::rc::Rc;
use std::{mem, slice};

use log::error;

use crate::class::invalid_class;
use crate::config::MAX_OBJ_SAVE;
use crate::db::{DB, REAL, VIRTUAL};
use crate::handler::{invalid_align, obj_from_char};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    CharData, ObjAffectedType, ObjData, ObjFileElem, RentInfo, ITEM_CONTAINER, ITEM_KEY,
    ITEM_NORENT, ITEM_WEAPON, ITEM_WEAR_ABOUT, ITEM_WEAR_ARMS, ITEM_WEAR_BODY, ITEM_WEAR_FEET,
    ITEM_WEAR_FINGER, ITEM_WEAR_HANDS, ITEM_WEAR_HEAD, ITEM_WEAR_HOLD, ITEM_WEAR_LEGS,
    ITEM_WEAR_NECK, ITEM_WEAR_SHIELD, ITEM_WEAR_WAIST, ITEM_WEAR_WIELD, ITEM_WEAR_WRIST, LVL_GOD,
    LVL_IMMORT, MAX_OBJ_AFFECT, NOTHING, NUM_WEARS, PLR_CRASH, RENT_CRASH, RENT_CRYO, RENT_FORCED,
    RENT_RENTED, RENT_TIMEDOUT, WEAR_ABOUT, WEAR_ARMS, WEAR_BODY, WEAR_FEET, WEAR_FINGER_L,
    WEAR_FINGER_R, WEAR_HANDS, WEAR_HEAD, WEAR_HOLD, WEAR_LEGS, WEAR_LIGHT, WEAR_NECK_1,
    WEAR_NECK_2, WEAR_SHIELD, WEAR_WAIST, WEAR_WIELD, WEAR_WRIST_L, WEAR_WRIST_R,
};
use crate::util::{clone_vec, get_filename, time_now, BRF, CRASH_FILE, NRM, SECS_PER_REAL_DAY};
use crate::{send_to_char, Game};

// /* these factors should be unique integers */
// #define RENT_FACTOR 	1
// #define CRYO_FACTOR 	4
//
pub const LOC_INVENTORY: i32 = 0;
pub const MAX_BAG_ROWS: i32 = 5;

pub fn obj_from_store(db: &DB, object: &ObjFileElem, location: &mut i32) -> Option<Rc<ObjData>> {
    *location = 0;
    let itemnum = db.real_object(object.item_number);
    if itemnum == NOTHING {
        return None;
    }

    let mut obj = db.read_object(itemnum, REAL).unwrap();
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
                db.equip_char(Some(ch), Some(obj), j as i8);
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
        DB::obj_to_char(Some(obj), Some(ch));
    }
}

// int Crash_delete_file(char *name)
// {
// char filename[50];
// FILE *fl;
//
// if (!get_filename(filename, sizeof(filename), CRASH_FILE, name))
// return (0);
// if (!(fl = fopen(filename, "rb"))) {
// if (errno != ENOENT)	/* if it fails but NOT because of no file */
// log("SYSERR: deleting crash file {} (1): {}", filename, strerror(errno));
// return (0);
// }
// fclose(fl);
//
// /* if it fails, NOT because of no file */
// if (remove(filename) < 0 && errno != ENOENT)
// log("SYSERR: deleting crash file {} (2): {}", filename, strerror(errno));
//
// return (1);
// }
//
//
// int Crash_delete_crashfile(struct char_data *ch)
// {
// char filename[MAX_INPUT_LENGTH];
// struct rent_info rent;
// int numread;
// FILE *fl;
//
// if (!get_filename(filename, sizeof(filename), CRASH_FILE, GET_NAME(ch)))
// return (0);
// if (!(fl = fopen(filename, "rb"))) {
// if (errno != ENOENT)	/* if it fails, NOT because of no file */
// log("SYSERR: checking for crash file {} (3): {}", filename, strerror(errno));
// return (0);
// }
// numread = fread(&rent, sizeof(struct rent_info), 1, fl);
// fclose(fl);
//
// if (numread == 0)
// return (0);
//
// if (rent.rentcode == RENT_CRASH)
// Crash_delete_file(GET_NAME(ch));
//
// return (1);
// }
//
//
// int Crash_clean_file(char *name)
// {
// char filename[MAX_STRING_LENGTH];
// struct rent_info rent;
// int numread;
// FILE *fl;
//
// if (!get_filename(filename, sizeof(filename), CRASH_FILE, name))
// return (0);
// /*
//  * open for write so that permission problems will be flagged now, at boot
//  * time.
//  */
// if (!(fl = fopen(filename, "r+b"))) {
// if (errno != ENOENT)	/* if it fails, NOT because of no file */
// log("SYSERR: OPENING OBJECT FILE {} (4): {}", filename, strerror(errno));
// return (0);
// }
// numread = fread(&rent, sizeof(struct rent_info), 1, fl);
// fclose(fl);
//
// if (numread == 0)
// return (0);
//
// if ((rent.rentcode == RENT_CRASH) ||
// (rent.rentcode == RENT_FORCED) || (rent.rentcode == RENT_TIMEDOUT)) {
// if (rent.time < time(0) - (crash_file_timeout * SECS_PER_REAL_DAY)) {
// const char *filetype;
//
// Crash_delete_file(name);
// switch (rent.rentcode) {
// case RENT_CRASH:
// filetype = "crash";
// break;
// case RENT_FORCED:
// filetype = "forced rent";
// break;
// case RENT_TIMEDOUT:
// filetype = "idlesave";
// break;
// default:
// filetype = "UNKNOWN!";
// break;
// }
// log("    Deleting {}'s {} file.", name, filetype);
// return (1);
// }
// /* Must retrieve rented items w/in 30 days */
// } else if (rent.rentcode == RENT_RENTED)
// if (rent.time < time(0) - (rent_file_timeout * SECS_PER_REAL_DAY)) {
// Crash_delete_file(name);
// log("    Deleting {}'s rent file.", name);
// return (1);
// }
// return (0);
// }
//
//
// void update_obj_file(void)
// {
// int i;
//
// for (i = 0; i <= top_of_p_table; i++)
// if (*player_table[i].name)
// Crash_clean_file(player_table[i].name);
// }

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
            // object.item_number, GET_OBJ_RENT(obj),
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

fn crash_write_rentcode(ch: &Rc<CharData>, fl: &mut File, rent: &mut RentInfo) -> bool {
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
    //int cost, orig_rent_code, num_objs = 0, j;
    //float num_of_days;
    /* AutoEQ addition. */
    // struct obj_data *obj, *obj2, *cont_row[MAX_BAG_ROWS];
    // int location;
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
                        DB::obj_to_char(Some(obj2), Some(ch));
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
                        db.obj_to_obj(Some(&obj2), obj.as_ref());
                    }
                    db.equip_char(Some(ch), obj.as_ref(), (location - 1) as i8);
                } else {
                    /* Object isn't container, empty the list. */
                    for obj2 in cont_row[0].iter() {
                        DB::obj_to_char(Some(&obj2), Some(ch));
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
                        DB::obj_to_char(Some(&obj2), Some(ch));
                    }
                    cont_row[j].clear();
                }
                j -= 1;
            }
            if j == -location as usize && cont_row[j].len() != 0 {
                /* Content list exists. */
                if obj.as_ref().unwrap().get_obj_type() == ITEM_CONTAINER {
                    /* Take the item, fill it, and give it back. */
                    obj_from_char(obj.as_ref());
                    obj.as_ref().unwrap().contains.borrow_mut().clear();
                    for obj2 in cont_row[j].iter() {
                        db.obj_to_obj(Some(&obj2), obj.as_ref());
                    }
                    DB::obj_to_char(obj.as_ref(), Some(ch)); /* Add to inventory first. */
                } else {
                    /* Object isn't container, empty content list. */
                    for obj2 in cont_row[j].iter() {
                        DB::obj_to_char(Some(&obj2), Some(ch));
                    }
                    cont_row[j].clear();
                }
            }
            if location < 0 && location >= -MAX_BAG_ROWS {
                /*
                 * Let the object be part of the content list but put it at the
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
    rent.time = time_now() as i32;
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
    let mut result = false;
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
        if crash_is_unrentable(ch.get_eq(j).as_ref()) {
            DB::obj_to_char(db.unequip_char(ch, j).as_ref(), Some(ch));
        } else {
            crash_extract_norents(db, ch.get_eq(j).as_ref());
        }
    }
}

fn crash_extract_objs(db: &DB, obj: &Rc<ObjData>) {
    for o in obj.contains.borrow().iter() {
        crash_extract_objs(db, o);
    }
    db.extract_obj(obj);
}

fn crash_is_unrentable(obj: Option<&Rc<ObjData>>) -> bool {
    if obj.is_none() {
        return false;
    }

    let obj = obj.as_ref().unwrap();
    if obj.obj_flagged(ITEM_NORENT)
        || obj.get_obj_rent() < 0
        || obj.get_obj_rnum() == NOTHING
        || obj.get_obj_type() == ITEM_KEY
    {
        return true;
    }
    false
}

fn crash_extract_norents(db: &DB, obj: Option<&Rc<ObjData>>) {
    if obj.is_some() {
        for o in obj.as_ref().unwrap().contains.borrow().iter() {
            crash_extract_norents(db, Some(o));
        }

        if crash_is_unrentable(obj) {
            db.extract_obj(obj.as_ref().unwrap());
        }
    }
}

// void Crash_extract_expensive(struct obj_data *obj)
// {
// struct obj_data *tobj, *max;
//
// max = obj;
// for (tobj = obj; tobj; tobj = tobj->next_content)
// if (GET_OBJ_RENT(tobj) > GET_OBJ_RENT(max))
// max = tobj;
// extract_obj(max);
// }
//
//
//
// void Crash_calculate_rent(struct obj_data *obj, int *cost)
// {
// if (obj) {
// *cost += MAX(0, GET_OBJ_RENT(obj));
// Crash_calculate_rent(obj->contains, cost);
// Crash_calculate_rent(obj->next_content, cost);
// }
// }

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
        time: time_now() as i32,
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

// void Crash_idlesave(struct char_data *ch)
// {
// char buf[MAX_INPUT_LENGTH];
// struct rent_info rent;
// int j;
// int cost, cost_eq;
// FILE *fp;
//
// if (IS_NPC(ch))
// return;
//
// if (!get_filename(buf, sizeof(buf), CRASH_FILE, GET_NAME(ch)))
// return;
// if (!(fp = fopen(buf, "wb")))
// return;
//
// Crash_extract_norent_eq(ch);
// Crash_extract_norents(ch->carrying);
//
// cost = 0;
// Crash_calculate_rent(ch->carrying, &cost);
//
// cost_eq = 0;
// for (j = 0; j < NUM_WEARS; j++)
// Crash_calculate_rent(GET_EQ(ch, j), &cost_eq);
//
// cost += cost_eq;
// cost *= 2;			/* forcerent cost is 2x normal rent */
//
// if (cost > GET_GOLD(ch) + GET_BANK_GOLD(ch)) {
// for (j = 0; j < NUM_WEARS; j++)	/* Unequip players with low gold. */
// if (GET_EQ(ch, j))
// obj_to_char(unequip_char(ch, j), ch);
//
// while ((cost > GET_GOLD(ch) + GET_BANK_GOLD(ch)) && ch->carrying) {
// Crash_extract_expensive(ch->carrying);
// cost = 0;
// Crash_calculate_rent(ch->carrying, &cost);
// cost *= 2;
// }
// }
//
// if (ch->carrying == NULL) {
// for (j = 0; j < NUM_WEARS && GET_EQ(ch, j) == NULL; j++) /* Nothing */ ;
// if (j == NUM_WEARS) {	/* No equipment or inventory. */
// fclose(fp);
// Crash_delete_file(GET_NAME(ch));
// return;
// }
// }
// rent.net_cost_per_diem = cost;
//
// rent.rentcode = RENT_TIMEDOUT;
// rent.time = time(0);
// rent.gold = GET_GOLD(ch);
// rent.account = GET_BANK_GOLD(ch);
// if (!Crash_write_rentcode(ch, fp, &rent)) {
// fclose(fp);
// return;
// }
// for (j = 0; j < NUM_WEARS; j++) {
// if (GET_EQ(ch, j)) {
// if (!Crash_save(GET_EQ(ch, j), fp, j + 1)) {
// fclose(fp);
// return;
// }
// Crash_restore_weight(GET_EQ(ch, j));
// Crash_extract_objs(GET_EQ(ch, j));
// }
// }
// if (!Crash_save(ch->carrying, fp, 0)) {
// fclose(fp);
// return;
// }
// fclose(fp);
//
// Crash_extract_objs(ch->carrying);
// }

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
    for o in ch.carrying.borrow().iter() {
        crash_extract_norents(db, Some(o));
    }

    let mut rent = RentInfo {
        time: time_now() as i32,
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
            crash_extract_objs(db, ch.get_eq(j).as_ref().unwrap());
        }
    }

    for o in ch.carrying.borrow().iter() {
        if !crash_save(db, Some(o), &mut fp, 0) {
            return;
        }
    }
    for o in clone_vec(&ch.carrying).iter() {
        crash_extract_objs(db, o);
    }
}

// void Crash_cryosave(struct char_data *ch, int cost)
// {
// char buf[MAX_INPUT_LENGTH];
// struct rent_info rent;
// int j;
// FILE *fp;
//
// if (IS_NPC(ch))
// return;
//
// if (!get_filename(buf, sizeof(buf), CRASH_FILE, GET_NAME(ch)))
// return;
// if (!(fp = fopen(buf, "wb")))
// return;
//
// Crash_extract_norent_eq(ch);
// Crash_extract_norents(ch->carrying);
//
// GET_GOLD(ch) = MAX(0, GET_GOLD(ch) - cost);
//
// rent.rentcode = RENT_CRYO;
// rent.time = time(0);
// rent.gold = GET_GOLD(ch);
// rent.account = GET_BANK_GOLD(ch);
// rent.net_cost_per_diem = 0;
// if (!Crash_write_rentcode(ch, fp, &rent)) {
// fclose(fp);
// return;
// }
// for (j = 0; j < NUM_WEARS; j++)
// if (GET_EQ(ch, j)) {
// if (!Crash_save(GET_EQ(ch, j), fp, j + 1)) {
// fclose(fp);
// return;
// }
// Crash_restore_weight(GET_EQ(ch, j));
// Crash_extract_objs(GET_EQ(ch, j));
// }
// if (!Crash_save(ch->carrying, fp, 0)) {
// fclose(fp);
// return;
// }
// fclose(fp);
//
// Crash_extract_objs(ch->carrying);
// SET_BIT(PLR_FLAGS(ch), PLR_CRYO);
// }
//
//
// /* ************************************************************************
// * Routines used for the receptionist					  *
// ************************************************************************* */
//
// void Crash_rent_deadline(struct char_data *ch, struct char_data *recep,
// long cost)
// {
// char buf[256];
// long rent_deadline;
//
// if (!cost)
// return;
//
// rent_deadline = ((GET_GOLD(ch) + GET_BANK_GOLD(ch)) / cost);
// snprintf(buf, sizeof(buf), "$n tells you, 'You can rent for %ld day{} with the gold you have\r\n"
// "on hand and in the bank.'\r\n", rent_deadline, rent_deadline != 1 ? "s" : "");
// act(buf, FALSE, recep, 0, ch, TO_VICT);
// }
//
// int Crash_report_unrentables(struct char_data *ch, struct char_data *recep,
// struct obj_data *obj)
// {
// int has_norents = 0;
//
// if (obj) {
// if (Crash_is_unrentable(obj)) {
// char buf[128];
//
// has_norents = 1;
// snprintf(buf, sizeof(buf), "$n tells you, 'You cannot store {}.'", OBJS(obj, ch));
// act(buf, FALSE, recep, 0, ch, TO_VICT);
// }
// has_norents += Crash_report_unrentables(ch, recep, obj->contains);
// has_norents += Crash_report_unrentables(ch, recep, obj->next_content);
// }
// return (has_norents);
// }
//
//
//
// void Crash_report_rent(struct char_data *ch, struct char_data *recep,
// struct obj_data *obj, long *cost, long *nitems, int display, int factor)
// {
// if (obj) {
// if (!Crash_is_unrentable(obj)) {
// (*nitems)++;
// *cost += MAX(0, (GET_OBJ_RENT(obj) * factor));
// if (display) {
// char buf[256];
//
// snprintf(buf, sizeof(buf), "$n tells you, '%5d coins for {}..'", GET_OBJ_RENT(obj) * factor, OBJS(obj, ch));
// act(buf, FALSE, recep, 0, ch, TO_VICT);
// }
// }
// Crash_report_rent(ch, recep, obj->contains, cost, nitems, display, factor);
// Crash_report_rent(ch, recep, obj->next_content, cost, nitems, display, factor);
// }
// }
//
//
//
// int Crash_offer_rent(struct char_data *ch, struct char_data *recep,
// int display, int factor)
// {
// int i;
// long totalcost = 0, numitems = 0, norent;
//
// norent = Crash_report_unrentables(ch, recep, ch->carrying);
// for (i = 0; i < NUM_WEARS; i++)
// norent += Crash_report_unrentables(ch, recep, GET_EQ(ch, i));
//
// if (norent)
// return (0);
//
// totalcost = min_rent_cost * factor;
//
// Crash_report_rent(ch, recep, ch->carrying, &totalcost, &numitems, display, factor);
//
// for (i = 0; i < NUM_WEARS; i++)
// Crash_report_rent(ch, recep, GET_EQ(ch, i), &totalcost, &numitems, display, factor);
//
// if (!numitems) {
// act("$n tells you, 'But you are not carrying anything!  Just quit!'",
// FALSE, recep, 0, ch, TO_VICT);
// return (0);
// }
// if (numitems > MAX_OBJ_SAVE) {
// char buf[256];
//
// snprintf(buf, sizeof(buf), "$n tells you, 'Sorry, but I cannot store more than %d items.'", MAX_OBJ_SAVE);
// act(buf, FALSE, recep, 0, ch, TO_VICT);
// return (0);
// }
// if (display) {
// char buf[256];
//
// snprintf(buf, sizeof(buf), "$n tells you, 'Plus, my %d coin fee..'", min_rent_cost * factor);
// act(buf, FALSE, recep, 0, ch, TO_VICT);
//
// snprintf(buf, sizeof(buf), "$n tells you, 'For a total of %ld coins{}.'", totalcost, factor == RENT_FACTOR ? " per day" : "");
// act(buf, FALSE, recep, 0, ch, TO_VICT);
//
// if (totalcost > GET_GOLD(ch) + GET_BANK_GOLD(ch)) {
// act("$n tells you, '...which I see you can't afford.'", FALSE, recep, 0, ch, TO_VICT);
// return (0);
// } else if (factor == RENT_FACTOR)
// Crash_rent_deadline(ch, recep, totalcost);
// }
// return (totalcost);
// }
//
//
//
// int gen_receptionist(struct char_data *ch, struct char_data *recep,
// int cmd, char *arg, int mode)
// {
// int cost;
// const char *action_table[] = { "smile", "dance", "sigh", "blush", "burp",
// "cough", "fart", "twiddle", "yawn" };
//
// if (!cmd && !rand_number(0, 5)) {
// do_action(recep, NULL, find_command(action_table[rand_number(0, 8)]), 0);
// return (FALSE);
// }
//
// if (!ch->desc || IS_NPC(ch))
// return (FALSE);
//
// if (!CMD_IS("offer") && !CMD_IS("rent"))
// return (FALSE);
//
// if (!AWAKE(recep)) {
// send_to_char(ch, "{} is unable to talk to you...\r\n", HSSH(recep));
// return (TRUE);
// }
//
// if (!CAN_SEE(recep, ch)) {
// act("$n says, 'I don't deal with people I can't see!'", FALSE, recep, 0, 0, TO_ROOM);
// return (TRUE);
// }
//
// if (free_rent) {
// act("$n tells you, 'Rent is free here.  Just quit, and your objects will be saved!'",
// FALSE, recep, 0, ch, TO_VICT);
// return (1);
// }
//
// if (CMD_IS("rent")) {
// char buf[128];
//
// if (!(cost = Crash_offer_rent(ch, recep, FALSE, mode)))
// return (TRUE);
// if (mode == RENT_FACTOR)
// snprintf(buf, sizeof(buf), "$n tells you, 'Rent will cost you %d gold coins per day.'", cost);
// else if (mode == CRYO_FACTOR)
// snprintf(buf, sizeof(buf), "$n tells you, 'It will cost you %d gold coins to be frozen.'", cost);
// act(buf, FALSE, recep, 0, ch, TO_VICT);
//
// if (cost > GET_GOLD(ch) + GET_BANK_GOLD(ch)) {
// act("$n tells you, '...which I see you can't afford.'",
// FALSE, recep, 0, ch, TO_VICT);
// return (TRUE);
// }
// if (cost && (mode == RENT_FACTOR))
// Crash_rent_deadline(ch, recep, cost);
//
// if (mode == RENT_FACTOR) {
// act("$n stores your belongings and helps you into your private chamber.", FALSE, recep, 0, ch, TO_VICT);
// Crash_rentsave(ch, cost);
// mudlog(NRM, MAX(LVL_IMMORT, GET_INVIS_LEV(ch)), TRUE, "{} has rented (%d/day, %d tot.)",
// GET_NAME(ch), cost, GET_GOLD(ch) + GET_BANK_GOLD(ch));
// } else {			/* cryo */
// act("$n stores your belongings and helps you into your private chamber.\r\n"
// "A white mist appears in the room, chilling you to the bone...\r\n"
// "You begin to lose consciousness...",
// FALSE, recep, 0, ch, TO_VICT);
// Crash_cryosave(ch, cost);
// mudlog(NRM, MAX(LVL_IMMORT, GET_INVIS_LEV(ch)), TRUE, "{} has cryo-rented.", GET_NAME(ch));
// SET_BIT(PLR_FLAGS(ch), PLR_CRYO);
// }
//
// act("$n helps $N into $S private chamber.", FALSE, recep, 0, ch, TO_NOTVICT);
//
// GET_LOADROOM(ch) = GET_ROOM_VNUM(IN_ROOM(ch));
// extract_char(ch);	/* It saves. */
// } else {
// Crash_offer_rent(ch, recep, TRUE, mode);
// act("$N gives $n an offer.", FALSE, ch, 0, recep, TO_ROOM);
// }
// return (TRUE);
// }
//
//
// SPECIAL(receptionist)
// {
// return (gen_receptionist(ch, (struct char_data *)me, cmd, argument, RENT_FACTOR));
// }
//
//
// SPECIAL(cryogenicist)
// {
// return (gen_receptionist(ch, (struct char_data *)me, cmd, argument, CRYO_FACTOR));
// }

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
