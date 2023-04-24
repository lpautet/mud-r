/* ************************************************************************
*   File: house.c                                       Part of CircleMUD *
*  Usage: Handling of player houses                                       *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::rc::Rc;
use std::{fs, mem, slice};

use log::{error, info};

use crate::constants::{DIRS, REV_DIR};
use crate::db::{get_id_by_name, get_name_by_id, DB, HCONTROL_FILE};
use crate::interpreter::{half_chop, is_abbrev, one_argument, search_block};
use crate::objsave::{obj_from_store, obj_to_store};
use crate::structs::{
    CharData, ObjData, ObjFileElem, RoomRnum, RoomVnum, LVL_GRGOD, LVL_IMMORT, NOWHERE,
    NUM_OF_DIRS, ROOM_ATRIUM, ROOM_HOUSE, ROOM_HOUSE_CRASH, ROOM_PRIVATE,
};
use crate::util::{ctime, time_now, NRM};
use crate::{send_to_char, Game};

pub const MAX_HOUSES: usize = 100;
pub const MAX_GUESTS: usize = 10;

pub const HOUSE_PRIVATE: i32 = 0;

#[derive(Clone, Copy)]
pub struct HouseControlRec {
    vnum: RoomVnum,
    /* vnum of this house		*/
    atrium: RoomVnum,
    /* vnum of atrium		*/
    exit_num: i16,
    /* direction of house's exit	*/
    built_on: u64,
    /* date this house was built	*/
    mode: i32,
    /* mode of ownership		*/
    owner: i64,
    /* idnum of house's owner	*/
    num_of_guests: i32,
    /* how many guests for house	*/
    guests: [i64; MAX_GUESTS],
    /* idnums of house's guests	*/
    last_payment: u64,
    /* date of last house payment   */
    spare0: i64,
    spare1: i64,
    spare2: i64,
    spare3: i64,
    spare4: i64,
    spare5: i64,
    spare6: i64,
    spare7: i64,
}

impl HouseControlRec {
    pub(crate) fn new() -> HouseControlRec {
        HouseControlRec {
            vnum: 0,
            atrium: 0,
            exit_num: 0,
            built_on: 0,
            mode: 0,
            owner: 0,
            num_of_guests: 0,
            guests: [0; MAX_GUESTS],
            last_payment: 0,
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

fn toroom(db: &DB, room: usize, dir: usize) -> RoomRnum {
    if db.world.borrow()[room].dir_option[dir].is_some() {
        db.world.borrow()[room].dir_option[dir]
            .as_ref()
            .unwrap()
            .to_room
            .get()
    } else {
        NOWHERE
    }
}

/* First, the basics: finding the filename; loading/saving objects */

/* Return a filename given a house vnum */
fn house_get_filename(vnum: RoomVnum, filename: &mut String) -> bool {
    if vnum == NOWHERE {
        return false;
    }

    *filename = format!("house/{}.house", vnum);
    return true;
}

/* Load all objects for a house */
fn house_load(db: &DB, vnum: RoomVnum) -> bool {
    let rnum;
    if {
        rnum = db.real_room(vnum);
        rnum == NOWHERE
    } {
        return false;
    }
    let mut filename = String::new();
    if !house_get_filename(vnum, &mut filename) {
        return false;
    }
    let fl;
    if {
        fl = OpenOptions::new().read(true).open(&filename);
        fl.is_err()
    } {
        /* no file found */
        return false;
    }
    let mut fl = fl.unwrap();

    loop {
        let mut object = ObjFileElem::new();
        unsafe {
            let obj_elem_slice = slice::from_raw_parts_mut(
                &mut object as *mut _ as *mut u8,
                mem::size_of::<ObjFileElem>(),
            );
            // `read_exact()` comes from `Read` impl for `&[u8]`
            let r = fl.read_exact(obj_elem_slice);
            if r.is_err() {
                let err = r.err().unwrap();
                if err.kind() == ErrorKind::UnexpectedEof {
                    break;
                }
                error!("[SYSERR] Error while reading house object file {err}",);
                return false;
            }
            let mut i = -1;
            db.obj_to_room(obj_from_store(db, &object, &mut i).as_ref(), rnum);
        }
    }

    true
}

/* Save all objects for a house (recursive; initial call must be followed
by a call to House_restore_weight)  Assumes file is open already. */
fn house_save(db: &DB, objects: &Vec<Rc<ObjData>>, fp: &mut File) -> bool {
    for obj in objects {
        for cobj in obj.contains.borrow().iter() {
            house_save(db, &cobj.contains.borrow(), fp);
        }
        let result = obj_to_store(db, obj, fp, 0);
        if !result {
            return false;
        }
        if obj.in_obj.borrow().is_some() {
            let oio = obj.in_obj.borrow();
            let tmp = oio.as_ref().unwrap();
            tmp.set_obj_weight(tmp.get_obj_weight() - obj.get_obj_weight());
        }
    }
    return true;
}

/* restore weight of containers after House_save has changed them for saving */
fn house_restore_weight(objects: &Vec<Rc<ObjData>>) {
    for obj in objects {
        for cobj in obj.contains.borrow().iter() {
            house_restore_weight(&cobj.contains.borrow());
        }

        if obj.in_obj.borrow().is_some() {
            obj.in_obj.borrow().as_ref().unwrap().set_obj_weight(
                obj.in_obj.borrow().as_ref().unwrap().get_obj_weight() + obj.get_obj_weight(),
            );
        }
    }
}

/* Save all objects in a house */
pub fn house_crashsave(db: &DB, vnum: RoomVnum) {
    let rnum;
    if {
        rnum = db.real_room(vnum);
        rnum == NOWHERE
    } {
        return;
    }
    let mut buf = String::new();
    if !house_get_filename(vnum, &mut buf) {
        return;
    }
    let fp;
    if {
        fp = OpenOptions::new().write(true).create(true).open(&buf);
        fp.is_err()
    } {
        error!("SYSERR: Error saving house file {}", fp.err().unwrap());
        return;
    }
    let mut fp = fp.unwrap();
    if !house_save(
        db,
        &db.world.borrow()[rnum as usize].contents.borrow(),
        &mut fp,
    ) {
        return;
    }

    house_restore_weight(&db.world.borrow()[rnum as usize].contents.borrow());
    db.remove_room_flags_bit(rnum, ROOM_HOUSE_CRASH);
}

/* Delete a house save file */
fn house_delete_file(vnum: RoomVnum) {
    let mut filename = "".to_string();
    if !house_get_filename(vnum, &mut filename) {
        return;
    }
    let fl;
    if {
        fl = OpenOptions::new().read(true).open(&filename);
        fl.is_err()
    } {
        let err = fl.err().unwrap();
        if err.kind() != ErrorKind::NotFound {
            error!("SYSERR: Error deleting house file #{}. (1): {}", vnum, err);
            return;
        }
    }
    let r;
    if {
        r = fs::remove_file(&filename);
        r.is_err()
    } {
        error!(
            "SYSERR: Error deleting house file #{}. (2): {}",
            vnum,
            r.err().unwrap()
        );
    }
}

/* List all objects in a house file */
fn house_listrent(db: &DB, ch: &Rc<CharData>, vnum: RoomVnum) {
    let mut filename = String::new();
    if !house_get_filename(vnum, &mut filename) {
        return;
    }
    let fl;
    if {
        fl = OpenOptions::new().read(true).open(&filename);
        fl.is_err()
    } {
        send_to_char(
            ch,
            format!("No objects on file for house #{}.\r\n", vnum).as_str(),
        );
        return;
    }
    let mut fl = fl.unwrap();

    loop {
        let mut object = ObjFileElem::new();
        unsafe {
            let object_slice = slice::from_raw_parts_mut(
                &mut object as *mut _ as *mut u8,
                mem::size_of::<ObjFileElem>(),
            );
            // `read_exact()` comes from `Read` impl for `&[u8]`
            let r = fl.read_exact(object_slice);
            if r.is_err() {
                let err = r.err().unwrap();
                if err.kind() == ErrorKind::UnexpectedEof {
                    break;
                }
                return;
            }
        }
        let mut i = -1;
        let obj = obj_from_store(db, &object, &mut i);
        if obj.is_some() {
            let obj = obj.as_ref().unwrap();
            send_to_char(
                ch,
                format!(
                    " [{:5}] ({:5}au) {}\r\n",
                    obj.item_number,
                    obj.get_obj_rent(),
                    obj.short_description
                )
                .as_str(),
            );
        }
    }
}

/******************************************************************
 *  Functions for house administration (creation, deletion, etc.  *
 *****************************************************************/

fn find_house(db: &DB, vnum: RoomVnum) -> Option<usize> {
    db.house_control
        .borrow()
        .iter()
        .position(|hc| hc.vnum == vnum)
}

/* Save the house control information */
fn house_save_control(db: &DB) {
    let fl;

    if {
        fl = OpenOptions::new()
            .create(true)
            .write(true)
            .open(HCONTROL_FILE);
        fl.is_err()
    } {
        error!(
            "SYSERR: Unable to open house control file. {}",
            fl.err().unwrap()
        );
        return;
    }
    let mut fl = fl.unwrap();
    for i in 0..db.num_of_houses.get() {
        let slice;
        unsafe {
            slice = slice::from_raw_parts(
                &mut db.house_control.borrow_mut()[i] as *mut _ as *mut u8,
                mem::size_of::<HouseControlRec>(),
            );
        }
        let r = fl.write_all(slice);
        if r.is_err() {
            error!("{}", r.err().unwrap());
            return;
        }
    }
}

/* call from boot_db - will load control recs, load objs, set atrium bits */
/* should do sanity checks on vnums & remove invalid records */
pub fn house_boot(db: &DB) {
    let mut temp_house = HouseControlRec::new();

    let fl;
    if {
        fl = OpenOptions::new().read(true).open(HCONTROL_FILE);
        fl.is_err()
    } {
        let err = fl.err().unwrap();
        if err.kind() == ErrorKind::NotFound {
            info!("   House control file '{}' does not exist.", HCONTROL_FILE);
        } else {
            error!("SYSERR: {} {} ", HCONTROL_FILE, err);
        }
        return;
    }
    let mut fl = fl.unwrap();
    while db.num_of_houses.get() < MAX_HOUSES {
        unsafe {
            let hc_slice = slice::from_raw_parts_mut(
                &mut temp_house as *mut _ as *mut u8,
                mem::size_of::<HouseControlRec>(),
            );
            // `read_exact()` comes from `Read` impl for `&[u8]`
            let r = fl.read_exact(hc_slice);
            if r.is_err() {
                let err = r.err().unwrap();
                if err.kind() == ErrorKind::UnexpectedEof {
                    break;
                }
                return;
            }

            if get_name_by_id(db, (&temp_house).owner).is_none() {
                continue; /* owner no longer exists -- skip */
            }
            let real_house;
            if {
                real_house = db.real_room((&temp_house).vnum);
                real_house == NOWHERE
            } {
                continue; /* this vnum doesn't exist -- skip */
            }

            if find_house(db, (&temp_house).vnum).is_some() {
                continue; /* this vnum is already a house -- skip */
            }
            let real_atrium;
            if {
                real_atrium = db.real_room((&temp_house).atrium);
                real_atrium == NOWHERE
            } {
                continue; /* house doesn't have an atrium -- skip */
            }

            if temp_house.exit_num < 0 || temp_house.exit_num >= NUM_OF_DIRS as i16 {
                continue; /* invalid exit num -- skip */
            }

            if toroom(db, real_house as usize, (&temp_house).exit_num as usize) != real_atrium {
                continue; /* exit num mismatch -- skip */
            }

            db.house_control.borrow_mut()[db.num_of_houses.get()] = temp_house;
            db.num_of_houses.set(db.num_of_houses.get() + 1);
            db.set_room_flags_bit(real_house, ROOM_HOUSE | ROOM_PRIVATE);
            db.set_room_flags_bit(real_atrium, ROOM_ATRIUM);

            house_load(db, temp_house.vnum);
        }
    }

    house_save_control(db);
}

/* "House Control" functions */

const HCONTROL_FORMAT: &str =
    "Usage: hcontrol build <house vnum> <exit direction> <player name>\r\n\
       hcontrol destroy <house vnum>\r\n\
       hcontrol pay <house vnum>\r\n\
       hcontrol show\r\n";

fn hcontrol_list_houses(db: &DB, ch: &Rc<CharData>) {
    if db.num_of_houses.get() == 0 {
        send_to_char(ch, "No houses have been defined.\r\n");
        return;
    }
    send_to_char(
        ch,
        "Address  Atrium  Build Date  Guests  Owner        Last Paymt\r\n\
-------  ------  ----------  ------  ------------ ----------\r\n",
    );
    let house_control = db.house_control.borrow();
    for i in 0..db.num_of_houses.get() {
        /* Avoid seeing <UNDEF> entries from self-deleted people. -gg 6/21/98 */
        let temp;
        if {
            temp = get_name_by_id(db, house_control[i].owner);
            temp.is_none()
        } {
            continue;
        }
        let built_on;
        if house_control[i].built_on != 0 {
            built_on = ctime(house_control[i].built_on);
        } else {
            built_on = "Unknown".to_string();
        }

        let last_pay;
        if house_control[i].last_payment != 0 {
            last_pay = ctime(house_control[i].last_payment);
        } else {
            last_pay = "None".to_string();
        }

        /* Now we need a copy of the owner's name to capitalize. -gg 6/21/98 */

        send_to_char(
            ch,
            format!(
                "{:7} {:7}  {:10}    {:2}    {:12} {}\r\n",
                house_control[i].vnum,
                house_control[i].atrium,
                built_on,
                house_control[i].num_of_guests,
                temp.unwrap().to_lowercase(),
                last_pay
            )
            .as_str(),
        );

        house_list_guests(db, ch, i, true);
    }
}

fn hcontrol_build_house(db: &DB, ch: &Rc<CharData>, arg: &mut str) {
    if db.num_of_houses.get() >= MAX_HOUSES {
        send_to_char(ch, "Max houses already defined.\r\n");
        return;
    }

    /* first arg: house's vnum */
    let mut arg1 = String::new();
    let arg2 = one_argument(arg, &mut arg1);
    let arg = arg2;
    if arg.is_empty() {
        send_to_char(ch, format!("{}", HCONTROL_FORMAT).as_str());
        return;
    }
    let virt_house = arg.parse::<i16>();
    if virt_house.is_err() {
        send_to_char(ch, "No such room exists.\r\n");
        return;
    }
    let virt_house = virt_house.unwrap();
    let real_house;
    if {
        real_house = db.real_room(virt_house);
        real_house == NOWHERE
    } {
        send_to_char(ch, "No such room exists.\r\n");
        return;
    }
    if find_house(db, virt_house).is_some() {
        send_to_char(ch, "House already exists.\r\n");
        return;
    }

    /* second arg: direction of house's exit */
    let arg2 = one_argument(arg, &mut arg1);
    let arg = arg2;
    if arg1.is_empty() {
        send_to_char(ch, HCONTROL_FORMAT);
        return;
    }
    let exit_num;
    if {
        exit_num = search_block(&arg1, &DIRS, false);
        exit_num.is_none()
    } {
        send_to_char(
            ch,
            format!("'{}' is not a valid direction.\r\n", arg1).as_str(),
        );
        return;
    }
    let exit_num = exit_num.unwrap();
    if toroom(db, real_house as usize, exit_num) == NOWHERE {
        send_to_char(
            ch,
            format!(
                "There is no exit {} from room {}.\r\n",
                DIRS[exit_num], virt_house
            )
            .as_str(),
        );
        return;
    }

    let real_atrium = toroom(db, real_house as usize, exit_num);
    let virt_atrium = db.get_room_vnum(real_atrium);

    if toroom(db, real_atrium as usize, REV_DIR[exit_num] as usize) != real_house {
        send_to_char(ch, "A house's exit must be a two-way door.\r\n");
        return;
    }

    /* third arg: player's name */
    one_argument(arg, &mut arg1);
    if arg1.is_empty() {
        send_to_char(ch, HCONTROL_FORMAT);
        return;
    }
    let owner;
    if {
        owner = get_id_by_name(db, &arg1);
        owner < 0
    } {
        send_to_char(ch, format!("Unknown player '{}'.\r\n", arg1).as_str());
        return;
    }
    let temp_house = HouseControlRec {
        vnum: virt_house,
        atrium: virt_atrium,
        exit_num: exit_num as i16,
        built_on: time_now(),
        mode: HOUSE_PRIVATE,
        owner,
        num_of_guests: 0,
        guests: [0; MAX_GUESTS],
        last_payment: 0,
        spare0: 0,
        spare1: 0,
        spare2: 0,
        spare3: 0,
        spare4: 0,
        spare5: 0,
        spare6: 0,
        spare7: 0,
    };

    db.house_control.borrow_mut()[db.num_of_houses.get()] = temp_house;
    db.num_of_houses.set(db.num_of_houses.get() + 1);

    db.set_room_flags_bit(real_house, ROOM_HOUSE | ROOM_PRIVATE);
    db.set_room_flags_bit(real_atrium, ROOM_ATRIUM);
    house_crashsave(db, virt_house);

    send_to_char(ch, "House built.  Mazel tov!\r\n");
    house_save_control(db);
}

fn hcontrol_destroy_house(db: &DB, ch: &Rc<CharData>, arg: &str) {
    let house_control = db.house_control.borrow_mut();
    if arg.is_empty() {
        send_to_char(ch, HCONTROL_FORMAT);
        return;
    }
    let argi = arg.parse::<i16>();
    let argi = if argi.is_ok() { argi.unwrap() } else { -1 };
    let i;
    if {
        i = find_house(db, argi);
        i.is_none()
    } {
        send_to_char(ch, "Unknown house.\r\n");
        return;
    }
    let i = i.unwrap();
    let real_atrium;
    if {
        real_atrium = db.real_room(house_control[i].atrium);
        real_atrium == NOWHERE
    } {
        error!(
            "SYSERR: House {} had invalid atrium {}!",
            argi, house_control[i].atrium
        );
    } else {
        db.remove_room_flags_bit(real_atrium, ROOM_ATRIUM);
    }
    let real_house;
    if {
        real_house = db.real_room(house_control[i].vnum);
        real_house == NOWHERE
    } {
        error!(
            "SYSERR: House {} had invalid vnum {}!",
            argi, house_control[i].vnum
        );
    } else {
        db.remove_room_flags_bit(real_house, ROOM_HOUSE | ROOM_PRIVATE | ROOM_HOUSE_CRASH);
    }

    house_delete_file(house_control[i].vnum);

    for j in i..db.num_of_houses.get() - 1 {
        db.house_control.borrow_mut()[j] = db.house_control.borrow()[j + 1];
    }

    db.num_of_houses.set(db.num_of_houses.get() - 1);

    send_to_char(ch, "House deleted.\r\n");
    house_save_control(db);

    /*
     * Now, reset the ROOM_ATRIUM flag on all existing houses' atriums,
     * just in case the house we just deleted shared an atrium with another
     * house.  --JE 9/19/94
     */
    for i in 0..db.num_of_houses.get() {
        let real_atrium;
        if {
            real_atrium = db.real_room(house_control[i].atrium);
            real_atrium != NOWHERE
        } {
            db.set_room_flags_bit(real_atrium, ROOM_ATRIUM);
        }
    }
}

fn hcontrol_pay_house(game: &Game, ch: &Rc<CharData>, arg: &str) {
    let db = &game.db;
    let argi = arg.parse::<i16>();
    let argi = if argi.is_err() { -1 } else { argi.unwrap() };
    let i;
    if arg.is_empty() {
        send_to_char(ch, HCONTROL_FORMAT);
    } else if {
        i = find_house(db, argi);
        i.is_none()
    } {
        send_to_char(ch, "Unknown house.\r\n");
    } else {
        game.mudlog(
            NRM,
            max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
            true,
            format!("Payment for house {} collected by {}.", arg, ch.get_name()).as_str(),
        );

        let i = i.unwrap();
        db.house_control.borrow_mut()[i].last_payment = time_now();
        house_save_control(db);
        send_to_char(ch, "Payment recorded.\r\n");
    }
}

/* The hcontrol command itself, used by imms to create/destroy houses */
#[allow(unused_variables)]
pub fn do_hcontrol(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let db = &game.db;
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut arg1, &mut arg2);

    if is_abbrev(&arg1, "build") {
        hcontrol_build_house(db, ch, &mut arg2);
    } else if is_abbrev(&arg1, "destroy") {
        hcontrol_destroy_house(db, ch, &arg2);
    } else if is_abbrev(&arg1, "pay") {
        hcontrol_pay_house(game, ch, &arg2);
    } else if is_abbrev(&arg1, "show") {
        hcontrol_list_houses(db, ch);
    } else {
        send_to_char(ch, HCONTROL_FORMAT);
    }
}

/* The house command, used by mortal house owners to assign guests */
#[allow(unused_variables)]
pub fn do_house(game: &Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let mut arg = String::new();
    one_argument(argument, &mut arg);
    let db = &game.db;
    let i;
    let id;
    if !db.room_flagged(ch.in_room(), ROOM_HOUSE) {
        send_to_char(ch, "You must be in your house to set guests.\r\n");
    } else if {
        i = find_house(db, db.get_room_vnum(ch.in_room()));
        i.is_none()
    } {
        send_to_char(ch, "Um.. this house seems to be screwed up.\r\n");
    } else if ch.get_idnum() != db.house_control.borrow()[i.unwrap()].owner {
        send_to_char(ch, "Only the primary owner can set guests.\r\n");
    } else if arg.is_empty() {
        house_list_guests(db, ch, i.unwrap(), false);
    } else if {
        id = get_id_by_name(db, &arg);
        id < 0
    } {
        send_to_char(ch, "No such player.\r\n");
    } else if id == ch.get_idnum() {
        send_to_char(ch, "It's your house!\r\n");
    } else {
        let i = i.unwrap();
        for j in 0..db.house_control.borrow()[i as usize].num_of_guests {
            if db.house_control.borrow()[i as usize].guests[j as usize] == id {
                for j in j..db.house_control.borrow()[i as usize].num_of_guests {
                    db.house_control.borrow_mut()[i as usize].guests[j as usize] =
                        db.house_control.borrow()[i as usize].guests[j as usize + 1];
                }
                db.house_control.borrow_mut()[i as usize].num_of_guests += 1;
                house_save_control(db);
                send_to_char(ch, "Guest deleted.\r\n");
                return;
            }
        }

        if db.house_control.borrow()[i as usize].num_of_guests == MAX_GUESTS as i32 {
            send_to_char(ch, "You have too many guests.\r\n");
            return;
        }
        db.house_control.borrow_mut()[i as usize].num_of_guests += 1;
        let j = db.house_control.borrow_mut()[i as usize].num_of_guests;
        db.house_control.borrow_mut()[i as usize].guests[j as usize] = id;
        house_save_control(db);
        send_to_char(ch, "Guest added.\r\n");
    }
}

/* Misc. administrative functions */

/* crash-save all the houses */
pub fn house_save_all(db: &DB) {
    for i in 0..db.num_of_houses.get() {
        let real_house = db.real_room(db.house_control.borrow()[i].vnum);
        if real_house != NOWHERE {
            if db.room_flagged(real_house, ROOM_HOUSE_CRASH) {
                house_crashsave(db, db.house_control.borrow()[i].vnum);
            }
        }
    }
}

/* note: arg passed must be house vnum, so there. */
pub fn house_can_enter(db: &DB, ch: &Rc<CharData>, house: RoomVnum) -> bool {
    let mut i = None;

    if ch.get_level() >= LVL_GRGOD as u8 || {
        i = find_house(db, house);
        i.is_none()
    } {
        return true;
    }
    let i = i.unwrap();
    match db.house_control.borrow()[i].mode {
        HOUSE_PRIVATE => {
            if ch.get_idnum() == db.house_control.borrow()[i].owner {
                return true;
            }
            for j in 0..db.house_control.borrow()[i].num_of_guests as usize {
                if ch.get_idnum() == db.house_control.borrow()[i].guests[j] {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

fn house_list_guests(db: &DB, ch: &Rc<CharData>, i: usize, quiet: bool) {
    let house_control = db.house_control.borrow();
    if house_control[i].num_of_guests == 0 {
        if !quiet {
            send_to_char(ch, "  Guests: None\r\n");
        }
        return;
    }

    send_to_char(ch, "  Guests: ");
    let mut num_printed = 0;
    for j in 0..house_control[i].num_of_guests as usize {
        /* Avoid <UNDEF>. -gg 6/21/98 */
        let temp;
        if {
            temp = get_name_by_id(db, house_control[i].guests[j]);
            temp.is_none()
        } {
            continue;
        }
        let temp = temp.unwrap();
        num_printed += 1;
        send_to_char(
            ch,
            format!(
                "{}{} ",
                temp.chars().next().unwrap().to_uppercase(),
                &temp[1..]
            )
            .as_str(),
        );
    }

    if num_printed == 0 {
        send_to_char(ch, "all dead");
    }

    send_to_char(ch, "\r\n");
}
