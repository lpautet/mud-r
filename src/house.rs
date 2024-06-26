/* ************************************************************************
*   File: house.rs                                      Part of CircleMUD *
*  Usage: Handling of player houses                                       *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::{fs, mem, slice};

use log::{error, info};

use crate::constants::{DIRS, REV_DIR};
use crate::db::{DB, HCONTROL_FILE};
use crate::depot::DepotId;
use crate::interpreter::{half_chop, is_abbrev, one_argument, search_block};
use crate::objsave::{obj_from_store, obj_to_store};
use crate::structs::{
    CharData,  ObjFileElem, RoomRnum, RoomVnum, LVL_GRGOD, LVL_IMMORT, NOWHERE,
    NUM_OF_DIRS, ROOM_ATRIUM, ROOM_HOUSE, ROOM_HOUSE_CRASH, ROOM_PRIVATE,
};
use crate::util::{ctime, time_now, NRM};
use crate::Game;

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
    _spare0: i64,
    _spare1: i64,
    _spare2: i64,
    _spare3: i64,
    _spare4: i64,
    _spare5: i64,
    _spare6: i64,
    _spare7: i64,
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
            _spare0: 0,
            _spare1: 0,
            _spare2: 0,
            _spare3: 0,
            _spare4: 0,
            _spare5: 0,
            _spare6: 0,
            _spare7: 0,
        }
    }
}

fn toroom(db: &DB, room: usize, dir: usize) -> RoomRnum {
    if db.world[room].dir_option[dir].is_some() {
        db.world[room].dir_option[dir]
            .as_ref()
            .unwrap()
            .to_room
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
fn house_load(db: &mut DB, vnum: RoomVnum) -> bool {
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
            let newobjid = obj_from_store(db, &object, &mut i).clone().unwrap();
            db.obj_to_room(newobjid, rnum);
        }
    }

    true
}

/* Save all objects for a house (recursive; initial call must be followed
by a call to House_restore_weight)  Assumes file is open already. */
fn house_save(db: &mut DB, oids: Vec<DepotId>, fp: &mut File) -> bool {
    for oid in oids {
        for coid in db.obj(oid).contains.clone() {
            house_save(db, db.obj(coid).contains.clone(), fp);
        }
        let result = obj_to_store(db, db.obj(oid), fp, 0);
        if !result {
            return false;
        }
        if db.obj(oid).in_obj.is_some() {
            let tmp_id = db.obj(oid).in_obj.unwrap();
            let val = db.obj(tmp_id).get_obj_weight() - db.obj(oid).get_obj_weight();
            db.obj_mut(tmp_id).set_obj_weight(val);
        }
    }
    return true;
}

/* restore weight of containers after House_save has changed them for saving */
fn house_restore_weight(db: &mut DB, oids: Vec<DepotId>) {
    for oid in oids {
        for coid in db.obj(oid).contains.clone() {
            house_restore_weight(db, db.obj(coid).contains.clone());
        }

        if db.obj(oid).in_obj.is_some() {
            let val = db.obj(db.obj(oid).in_obj.unwrap()).get_obj_weight() + db.obj(oid).get_obj_weight();
            db.obj_mut(db.obj(oid).in_obj.unwrap()).set_obj_weight(
               val ,
            );
        }
    }
}

/* Save all objects in a house */
pub fn house_crashsave(db: &mut DB, vnum: RoomVnum) {
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
        db.world[rnum as usize].contents.clone(),
        &mut fp,
    ) {
        return;
    }

    house_restore_weight(db, db.world[rnum as usize].contents.clone());
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
// fn house_listrent(db: &DB, chid: DepotId, vnum: RoomVnum) {
//     let mut filename = String::new();
//     if !house_get_filename(vnum, &mut filename) {
//         return;
//     }
//     let fl;
//     if {
//         fl = OpenOptions::new().read(true).open(&filename);
//         fl.is_err()
//     } {
//         game.send_to_char(
//             ch,
//             format!("No objects on file for house #{}.\r\n", vnum).as_str(),
//         );
//         return;
//     }
//     let mut fl = fl.unwrap();
//
//     loop {
//         let mut object = ObjFileElem::new();
//         unsafe {
//             let object_slice = slice::from_raw_parts_mut(
//                 &mut object as *mut _ as *mut u8,
//                 mem::size_of::<ObjFileElem>(),
//             );
//             // `read_exact()` comes from `Read` impl for `&[u8]`
//             let r = fl.read_exact(object_slice);
//             if r.is_err() {
//                 let err = r.err().unwrap();
//                 if err.kind() == ErrorKind::UnexpectedEof {
//                     break;
//                 }
//                 return;
//             }
//         }
//         let mut i = -1;
//         let obj = obj_from_store(db, &object, &mut i);
//         if obj.is_some() {
//             let obj = obj.as_ref().unwrap();
//             game.send_to_char(
//                 ch,
//                 format!(
//                     " [{:5}] ({:5}au) {}\r\n",
//                     obj.item_number,
//                     obj.get_obj_rent(),
//                     obj.short_description
//                 )
//                 .as_str(),
//             );
//         }
//     }
// }

/******************************************************************
 *  Functions for house administration (creation, deletion, etc.  *
 *****************************************************************/

fn find_house(db: &DB, vnum: RoomVnum) -> Option<usize> {
    db.house_control
        .iter()
        .position(|hc| hc.vnum == vnum)
}

/* Save the house control information */
fn house_save_control(db: &mut DB) {
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
    for i in 0..db.num_of_houses {
        let slice;
        unsafe {
            slice = slice::from_raw_parts(
                &mut db.house_control[i] as *mut _ as *mut u8,
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
pub fn house_boot(db: &mut DB) {
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
    while db.num_of_houses < MAX_HOUSES {
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

            if db.get_name_by_id((&temp_house).owner).is_none() {
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

            db.house_control[db.num_of_houses] = temp_house;
            db.num_of_houses += 1;
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

pub fn hcontrol_list_houses(game: &mut Game, chid: DepotId) {
    if game.db.num_of_houses == 0 {
        game.send_to_char(chid, "No houses have been defined.\r\n");
        return;
    }
    game.send_to_char(
        chid,
        "Address  Atrium  Build Date  Guests  Owner        Last Paymt\r\n\
-------  ------  ----------  ------  ------------ ----------\r\n",
    );
    let house_control = game.db.house_control;
    for i in 0..game.db.num_of_houses {
        /* Avoid seeing <UNDEF> entries from self-deleted people. -gg 6/21/98 */
        let temp;
        if {
            temp = game.db.get_name_by_id(house_control[i].owner);
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

        game.send_to_char(
            chid,
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

        house_list_guests(game, chid, i, true);
    }
}

fn hcontrol_build_house(game: &mut Game, chid: DepotId, arg: &mut str) {
    if game.db.num_of_houses >= MAX_HOUSES {
        game.send_to_char(chid, "Max houses already defined.\r\n");
        return;
    }

    /* first arg: house's vnum */
    let mut arg1 = String::new();
    let arg2 = one_argument(arg, &mut arg1);
    let arg = arg2;
    if arg.is_empty() {
        game.send_to_char(chid, format!("{}", HCONTROL_FORMAT).as_str());
        return;
    }
    let virt_house = arg.parse::<i16>();
    if virt_house.is_err() {
        game.send_to_char(chid, "No such room exists.\r\n");
        return;
    }
    let virt_house = virt_house.unwrap();
    let real_house;
    if {
        real_house = game.db.real_room(virt_house);
        real_house == NOWHERE
    } {
        game.send_to_char(chid, "No such room exists.\r\n");
        return;
    }
    if find_house(&game.db, virt_house).is_some() {
        game.send_to_char(chid, "House already exists.\r\n");
        return;
    }

    /* second arg: direction of house's exit */
    let arg2 = one_argument(arg, &mut arg1);
    let arg = arg2;
    if arg1.is_empty() {
        game.send_to_char(chid, HCONTROL_FORMAT);
        return;
    }
    let exit_num;
    if {
        exit_num = search_block(&arg1, &DIRS, false);
        exit_num.is_none()
    } {
        game.send_to_char(
            chid,
            format!("'{}' is not a valid direction.\r\n", arg1).as_str(),
        );
        return;
    }
    let exit_num = exit_num.unwrap();
    if toroom(&game.db, real_house as usize, exit_num) == NOWHERE {
        game.send_to_char(
            chid,
            format!(
                "There is no exit {} from room {}.\r\n",
                DIRS[exit_num], virt_house
            )
            .as_str(),
        );
        return;
    }

    let real_atrium = toroom(&game.db, real_house as usize, exit_num);
    let virt_atrium = game.db.get_room_vnum(real_atrium);

    if toroom(&game.db, real_atrium as usize, REV_DIR[exit_num] as usize) != real_house {
        game.send_to_char(chid, "A house's exit must be a two-way door.\r\n");
        return;
    }

    /* third arg: player's name */
    one_argument(arg, &mut arg1);
    if arg1.is_empty() {
        game.send_to_char(chid, HCONTROL_FORMAT);
        return;
    }
    let owner;
    if {
        owner = game.db.get_id_by_name(&arg1);
        owner < 0
    } {
        game.send_to_char(chid, format!("Unknown player '{}'.\r\n", arg1).as_str());
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
        _spare0: 0,
        _spare1: 0,
        _spare2: 0,
        _spare3: 0,
        _spare4: 0,
        _spare5: 0,
        _spare6: 0,
        _spare7: 0,
    };

    game.db.house_control[game.db.num_of_houses] = temp_house;
    game.db.num_of_houses += 1;

    game.db.set_room_flags_bit(real_house, ROOM_HOUSE | ROOM_PRIVATE);
    game.db.set_room_flags_bit(real_atrium, ROOM_ATRIUM);
    house_crashsave(&mut game.db, virt_house);

    game.send_to_char(chid, "House built.  Mazel tov!\r\n");
    house_save_control(&mut game.db);
}

fn hcontrol_destroy_house(game: &mut Game, chid: DepotId, arg: &str) {
    if arg.is_empty() {
        game.send_to_char(chid, HCONTROL_FORMAT);
        return;
    }
    let argi = arg.parse::<i16>();
    let argi = if argi.is_ok() { argi.unwrap() } else { -1 };
    let i;
    if {
        i = find_house(&game.db, argi);
        i.is_none()
    } {
        game.send_to_char(chid, "Unknown house.\r\n");
        return;
    }
    let i = i.unwrap();
    let real_atrium;
    if {
        real_atrium = game.db.real_room(game.db.house_control[i].atrium);
        real_atrium == NOWHERE
    } {
        error!(
            "SYSERR: House {} had invalid atrium {}!",
            argi, game.db.house_control[i].atrium
        );
    } else {
        game.db.remove_room_flags_bit(real_atrium, ROOM_ATRIUM);
    }
    let real_house;
    if {
        real_house = game.db.real_room(game.db.house_control[i].vnum);
        real_house == NOWHERE
    } {
        error!(
            "SYSERR: House {} had invalid vnum {}!",
            argi, game.db.house_control[i].vnum
        );
    } else {
        game.db.remove_room_flags_bit(real_house, ROOM_HOUSE | ROOM_PRIVATE | ROOM_HOUSE_CRASH);
    }

    house_delete_file(game.db.house_control[i].vnum);

    for j in i..game.db.num_of_houses - 1 {
        game.db.house_control[j] = game.db.house_control[j + 1];
    }

    game.db.num_of_houses -= 1;

    game.send_to_char(chid, "House deleted.\r\n");
    house_save_control(&mut game.db);

    /*
     * Now, reset the ROOM_ATRIUM flag on all existing houses' atriums,
     * just in case the house we just deleted shared an atrium with another
     * house.  --JE 9/19/94
     */
    for i in 0..game.db.num_of_houses {
        let real_atrium;
        if {
            real_atrium = game.db.real_room(game.db.house_control[i].atrium);
            real_atrium != NOWHERE
        } {
            game.db.set_room_flags_bit(real_atrium, ROOM_ATRIUM);
        }
    }
}

fn hcontrol_pay_house(game: &mut Game, chid: DepotId, arg: &str) {
    let ch = game.db.ch(chid);

    let argi = arg.parse::<i16>();
    let argi = if argi.is_err() { -1 } else { argi.unwrap() };
    let i;
    if arg.is_empty() {
        game.send_to_char(chid, HCONTROL_FORMAT);
    } else if {
        i = find_house(&game.db, argi);
        i.is_none()
    } {
        game.send_to_char(chid, "Unknown house.\r\n");
    } else {
        game.mudlog(
            NRM,
            max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
            true,
            format!("Payment for house {} collected by {}.", arg, ch.get_name()).as_str(),
        );

        let i = i.unwrap();
        game.db.house_control[i].last_payment = time_now();
        house_save_control(&mut game.db);
        game.send_to_char(chid, "Payment recorded.\r\n");
    }
}

/* The hcontrol command itself, used by imms to create/destroy houses */
pub fn do_hcontrol(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut arg1, &mut arg2);

    if is_abbrev(&arg1, "build") {
        hcontrol_build_house( game, chid, &mut arg2);
    } else if is_abbrev(&arg1, "destroy") {
        hcontrol_destroy_house(game, chid, &arg2);
    } else if is_abbrev(&arg1, "pay") {
        hcontrol_pay_house(game, chid, &arg2);
    } else if is_abbrev(&arg1, "show") {
        hcontrol_list_houses(game, chid);
    } else {
        game.send_to_char(chid, HCONTROL_FORMAT);
    }
}

/* The house command, used by mortal house owners to assign guests */
pub fn do_house(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = game.db.ch(chid);

    let mut arg = String::new();
    one_argument(argument, &mut arg);
    let i;
    let id;
    if !game.db.room_flagged(ch.in_room(), ROOM_HOUSE) {
        game.send_to_char(chid, "You must be in your house to set guests.\r\n");
    } else if {
        i = find_house(&game.db, game.db.get_room_vnum(ch.in_room()));
        i.is_none()
    } {
        game.send_to_char(chid, "Um.. this house seems to be screwed up.\r\n");
    } else if ch.get_idnum() != game.db.house_control[i.unwrap()].owner {
        game.send_to_char(chid, "Only the primary owner can set guests.\r\n");
    } else if arg.is_empty() {
        house_list_guests(game, chid, i.unwrap(), false);
    } else if {
        id = game.db.get_id_by_name(&arg);
        id < 0
    } {
        game.send_to_char(chid, "No such player.\r\n");
    } else if id == ch.get_idnum() {
        game.send_to_char(chid, "It's your house!\r\n");
    } else {
        let i = i.unwrap();
        for j in 0..game.db.house_control[i as usize].num_of_guests {
            if game.db.house_control[i as usize].guests[j as usize] == id {
                for j in j..game.db.house_control[i as usize].num_of_guests {
                    game.db.house_control[i as usize].guests[j as usize] =
                        game.db.house_control[i as usize].guests[j as usize + 1];
                }
                game.db.house_control[i as usize].num_of_guests += 1;
                house_save_control(&mut game.db);
                game.send_to_char(chid, "Guest deleted.\r\n");
                return;
            }
        }

        if game.db.house_control[i as usize].num_of_guests == MAX_GUESTS as i32 {
            game.send_to_char(chid, "You have too many guests.\r\n");
            return;
        }
        game.db.house_control[i as usize].num_of_guests += 1;
        let j = game.db.house_control[i as usize].num_of_guests;
        game.db.house_control[i as usize].guests[j as usize] = id;
        house_save_control(&mut game.db);
        game.send_to_char(chid, "Guest added.\r\n");
    }
}

/* Misc. administrative functions */

/* crash-save all the houses */
pub fn house_save_all(db: &mut DB) {
    for i in 0..db.num_of_houses{
        let real_house = db.real_room(db.house_control[i].vnum);
        if real_house != NOWHERE {
            if db.room_flagged(real_house, ROOM_HOUSE_CRASH) {
                let room_vnum = db.house_control[i].vnum;
                house_crashsave(db, room_vnum);
            }
        }
    }
}

/* note: arg passed must be house vnum, so there. */
pub fn house_can_enter(db: &DB, ch: &CharData, house: RoomVnum) -> bool {
    let mut i = None;

    if ch.get_level() >= LVL_GRGOD as u8 || {
        i = find_house(db, house);
        i.is_none()
    } {
        return true;
    }
    let i = i.unwrap();
    match db.house_control[i].mode {
        HOUSE_PRIVATE => {
            if ch.get_idnum() == db.house_control[i].owner {
                return true;
            }
            for j in 0..db.house_control[i].num_of_guests as usize {
                if ch.get_idnum() == db.house_control[i].guests[j] {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

fn house_list_guests(game: &mut Game, chid: DepotId, i: usize, quiet: bool) {
    let house_control = game.db.house_control;
    if house_control[i].num_of_guests == 0 {
        if !quiet {
            game.send_to_char(chid, "  Guests: None\r\n");
        }
        return;
    }

    game.send_to_char(chid, "  Guests: ");
    let mut num_printed = 0;
    for j in 0..house_control[i].num_of_guests as usize {
        /* Avoid <UNDEF>. -gg 6/21/98 */
        let temp;
        if {
            temp = game.db.get_name_by_id(house_control[i].guests[j]);
            temp.is_none()
        } {
            continue;
        }
        let temp = temp.unwrap();
        num_printed += 1;
        game.send_to_char(
            chid,
            format!(
                "{}{} ",
                temp.chars().next().unwrap().to_uppercase(),
                &temp[1..]
            )
            .as_str(),
        );
    }

    if num_printed == 0 {
        game.send_to_char(chid, "all dead");
    }

    game.send_to_char(chid, "\r\n");
}
