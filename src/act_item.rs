/* ************************************************************************
*   File: act.item.rs                                   Part of CircleMUD *
*  Usage: object handling routines -- get/drop and container handling     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use std::cmp::{max, min};
use std::rc::Rc;

use crate::depot::{Depot, DepotId, HasId};
use crate::{CharData, ObjData, TextData, VictimRef};
use log::error;

use crate::config::{DONATION_ROOM_1, NOPERSON, OK};
use crate::constants::{DRINKNAMES, DRINKS, DRINK_AFF, STR_APP};
use crate::db::DB;
use crate::handler::{
    find_all_dots, isname, money_desc, FIND_ALL, FIND_ALLDOT, FIND_CHAR_ROOM, FIND_INDIV,
    FIND_OBJ_INV, FIND_OBJ_ROOM,
};
use crate::interpreter::{
    is_number, one_argument, search_block, two_arguments, SCMD_DONATE, SCMD_DRINK, SCMD_DROP,
    SCMD_EAT, SCMD_FILL, SCMD_JUNK, SCMD_POUR, SCMD_SIP, SCMD_TASTE,
};
use crate::spells::SPELL_POISON;
use crate::structs::{
    AffectedType, RoomRnum, AFF_POISON, APPLY_NONE, CONT_CLOSED, DRUNK, FULL, ITEM_CONTAINER,
    ITEM_DRINKCON, ITEM_FOOD, ITEM_FOUNTAIN, ITEM_LIGHT, ITEM_MONEY, ITEM_NODONATE, ITEM_NODROP,
    ITEM_POTION, ITEM_SCROLL, ITEM_STAFF, ITEM_WAND, ITEM_WEAR_ABOUT, ITEM_WEAR_ARMS,
    ITEM_WEAR_BODY, ITEM_WEAR_FEET, ITEM_WEAR_FINGER, ITEM_WEAR_HANDS, ITEM_WEAR_HEAD,
    ITEM_WEAR_HOLD, ITEM_WEAR_LEGS, ITEM_WEAR_NECK, ITEM_WEAR_SHIELD, ITEM_WEAR_TAKE,
    ITEM_WEAR_WAIST, ITEM_WEAR_WIELD, ITEM_WEAR_WRIST, LVL_GOD, LVL_IMMORT, NOWHERE, NUM_WEARS,
    PULSE_VIOLENCE, THIRST, WEAR_ABOUT, WEAR_ARMS, WEAR_BODY, WEAR_FEET, WEAR_FINGER_R, WEAR_HANDS,
    WEAR_HEAD, WEAR_HOLD, WEAR_LEGS, WEAR_LIGHT, WEAR_NECK_1, WEAR_SHIELD, WEAR_WAIST, WEAR_WIELD,
    WEAR_WRIST_R,
};
use crate::util::rand_number;
use crate::{an, Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

fn perform_put(game: &mut Game, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId, oid: DepotId, cid: DepotId) {
    let ch = db.ch(chid);
    let obj = objs.get(oid);
    let cobj = objs.get(cid);

    if objs.get(cid).get_obj_weight() + objs.get(oid).get_obj_weight() > objs.get(cid).get_obj_val(0) {
        game.act(
            db,
            "$p won't fit in $P.",
            false,
            Some(ch),
            Some(obj),
            Some(VictimRef::Obj(cobj)),
            TO_CHAR,
        );
    } else if objs.get(oid).obj_flagged(ITEM_NODROP) && objs.get(cid).in_room() != NOWHERE {
        game.act(
            db,
            "You can't get $p out of your hand.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
    } else {
        db.obj_from_char(objs,oid);
        db.obj_to_obj(objs,oid, cid);
        let ch = db.ch(chid);
        let obj = objs.get(oid);
        let cobj = objs.get(cid);
        game.act(
            db,
            "$n puts $p in $P.",
            true,
            Some(ch),
            Some(obj),
            Some(VictimRef::Obj(cobj)),
            TO_ROOM,
        );

        /* Yes, I realize this is strange until we have auto-equip on rent. -gg */
        if objs.get(oid).obj_flagged(ITEM_NODROP) && !objs.get(cid).obj_flagged(ITEM_NODROP) {
            objs.get_mut(cid).set_obj_extra_bit(ITEM_NODROP);
            let ch = db.ch(chid);
            let obj = objs.get(oid);
            let cobj = objs.get(cid);
            game.act(
                db,
                "You get a strange feeling as you put $p in $P.",
                false,
                Some(ch),
                Some(obj),
                Some(VictimRef::Obj(cobj)),
                TO_CHAR,
            );
        } else {
            game.act(
                db,
                "You put $p in $P.",
                false,
                Some(ch),
                Some(obj),
                Some(VictimRef::Obj(cobj)),
                TO_CHAR,
            );
        }
    }
}

/* The following put modes are supported by the code below:

    1) put <object> <container>
    2) put all.<object> <container>
    3) put all <container>

    <container> must be in inventory or on ground.
    all objects to be put into container must be in inventory.
*/
pub fn do_put(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut found = false;
    let mut howmany = 1;
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut arg3 = String::new();
    let theobj;
    let thecont;
    one_argument(two_arguments(argument, &mut arg1, &mut arg2), &mut arg3); /* three_arguments */

    if !arg3.is_empty() && is_number(&arg1) {
        howmany = arg1.parse::<i32>().unwrap();
        theobj = arg2;
        thecont = arg3;
    } else {
        theobj = arg1;
        thecont = arg2;
    }
    let obj_dotmode = find_all_dots(&theobj);
    let cont_dotmode = find_all_dots(&thecont);
    let mut tmp_char = None;
    let mut c = None;
    let mut obj;

    if theobj.is_empty() {
        game.send_to_char(ch, "Put what in what?\r\n");
    } else if cont_dotmode != FIND_INDIV {
        game.send_to_char(
            ch,
            "You can only put things into one container at a time.\r\n",
        );
    } else if thecont.is_empty() {
        game.send_to_char(
            ch,
            format!(
                "What do you want to put {} in?\r\n",
                if obj_dotmode == FIND_INDIV {
                    "it"
                } else {
                    "them"
                }
            )
            .as_str(),
        );
    } else {
        game.generic_find(
            db,objs,
            &thecont,
            (FIND_OBJ_INV | FIND_OBJ_ROOM) as i64,
            ch,
            &mut tmp_char,
            &mut c,
        );
        if c.is_none() {
            game.send_to_char(
                ch,
                format!("You don't see {} {} here.\r\n", an!(thecont), thecont).as_str(),
            );
        } else if c.unwrap().get_obj_type() != ITEM_CONTAINER {
            game.act(
                db,
                "$p is not a container.",
                false,
                Some(ch),
                Some(c.unwrap()),
                None,
                TO_CHAR,
            );
        } else if c.unwrap().objval_flagged(CONT_CLOSED) {
            game.send_to_char(ch, "You'd better open it first!\r\n");
        } else {
            if obj_dotmode == FIND_INDIV {
                /* put <obj> <container> */

                if {
                    obj = game.get_obj_in_list_vis(db, objs,ch, &theobj, None, ch.carrying.as_ref());
                    obj.is_none()
                } {
                    game.send_to_char(
                        ch,
                        format!("You aren't carrying {} {}.\r\n", an!(theobj), theobj).as_str(),
                    );
                } else if obj.unwrap().id() == c.unwrap().id() && howmany == 1 {
                    game.send_to_char(ch, "You attempt to fold it into itself, but fail.\r\n");
                } else {
                    let c_id = c.unwrap().id();
                    while obj.is_some() && howmany != 0 {
                        let oid = obj.unwrap().id();
                        if oid != c_id {
                            howmany -= 1;
                            perform_put(game, db, objs,chid, oid, c_id);
                        }
                        let ch = db.ch(chid);
                        obj = game.get_obj_in_list_vis(db, objs,ch, &theobj, None, ch.carrying.as_ref());
                    }
                }
            } else {
                let c_id = c.unwrap().id();
                for oid in ch.carrying.clone() {
                    if oid != c_id
                        && (obj_dotmode == FIND_ALL || isname(&theobj, &objs.get(oid).name.as_ref()))
                    {
                        found = true;
                        perform_put(game, db, objs,chid, oid, c_id);
                    }
                }
                let ch = db.ch(chid);
                if !found {
                    if obj_dotmode == FIND_ALL {
                        game.send_to_char(ch, "You don't seem to have anything to put in it.\r\n");
                    } else {
                        game.send_to_char(
                            ch,
                            format!("You don't seem to have any {}s.\r\n", theobj).as_str(),
                        );
                    }
                }
            }
        }
    }
}

fn can_take_obj(game: &mut Game, db: &DB,objs: & Depot<ObjData>,  chid: DepotId, oid: DepotId) -> bool {
    let ch = db.ch(chid);
    let obj = objs.get(oid);
    if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
        game.act(
            db,
            "$p: you can't carry that many items.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return false;
    } else if (ch.is_carrying_w() + objs.get(oid).get_obj_weight()) > ch.can_carry_w() as i32 {
        game.act(
            db,
            "$p: you can't carry that much weight.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return false;
    } else if !objs.get(oid).can_wear(ITEM_WEAR_TAKE) {
        game.act(
            db,
            "$p: you can't take that!",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return false;
    }
    true
}

fn get_check_money(game: &mut Game, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId, oid: DepotId) {
    let value = objs.get(oid).get_obj_val(0);

    if objs.get(oid).get_obj_type() != ITEM_MONEY || value <= 0 {
        return;
    }

    db.extract_obj( objs,oid);
    let ch = db.ch_mut(chid);
    ch.set_gold(ch.get_gold() + value);

    if value == 1 {
        game.send_to_char(ch, "There was 1 coin.\r\n");
    } else {
        game.send_to_char(ch, format!("There were {} coins.\r\n", value).as_str());
    }
}

fn perform_get_from_container(
    game: &mut Game,
    db: &mut DB,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    oid: DepotId,
    cid: DepotId,
    mode: i32,
) {
    if mode == FIND_OBJ_INV || can_take_obj(game, db, objs,chid, oid) {
        let ch = db.ch(chid);
        let obj = objs.get(oid);
        if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
            game.act(
                db,
                "$p: you can't hold any more items.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
        } else {
            db.obj_from_obj(objs,oid);
            db.obj_to_char(objs,oid, chid);
            let ch = db.ch(chid);
            let obj = objs.get(oid);
            let cobj = objs.get(cid);
            game.act(
                db,
                "You get $p from $P.",
                false,
                Some(ch),
                Some(obj),
                Some(VictimRef::Obj(cobj)),
                TO_CHAR,
            );
            game.act(
                db,
                "$n gets $p from $P.",
                true,
                Some(ch),
                Some(obj),
                Some(VictimRef::Obj(cobj)),
                TO_ROOM,
            );
            get_check_money(game, db, objs,chid, oid);
        }
    }
}

fn get_from_container(
    game: &mut Game,
    db: &mut DB,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    cid: DepotId,
    arg: &str,
    mode: i32,
    howmany: i32,
) {
    let ch = db.ch(chid);
    let cobj = objs.get(cid);
    let mut found = false;

    let mut howmany = howmany;
    let obj_dotmode = find_all_dots(arg);

    if cobj.obj_flagged(CONT_CLOSED) {
        game.act(
            db,
            "$p is closed.",
            false,
            Some(ch),
            Some(cobj),
            None,
            TO_CHAR,
        );
    } else if obj_dotmode == FIND_INDIV {
        let mut obj = game.get_obj_in_list_vis(db, objs,ch, arg, None, &cobj.contains);
        if obj.is_none() {
            let buf = format!("There doesn't seem to be {} {} in $p.", an!(arg), arg);
            game.act(db, &buf, false, Some(ch), Some(cobj), None, TO_CHAR);
        } else {
            while obj.is_some() && howmany != 0 {
                let oid = obj.unwrap().id();
                howmany -= 1;
                perform_get_from_container(game, db, objs,chid, oid, cid, mode);
                let ch = db.ch(chid);
                let cobj = objs.get(cid);
                obj = game.get_obj_in_list_vis(db, objs,ch, arg, None, &cobj.contains);
            }
        }
    } else {
        if obj_dotmode == FIND_ALLDOT && arg.is_empty() {
            game.send_to_char(ch, "Get all of what?\r\n");
            return;
        }
        for oid in cobj.contains.clone() {
            let ch = db.ch(chid);
            if game.can_see_obj(db, ch, objs.get(oid))
                && (obj_dotmode == FIND_ALL || isname(arg, &objs.get(oid).name))
            {
                found = true;
                perform_get_from_container(game, db,objs, chid, oid, cid, mode);
            }
        }
        let ch = db.ch(chid);
        let cobj = objs.get(cid);
        if !found {
            if obj_dotmode == FIND_ALL {
                game.act(
                    db,
                    "$p seems to be empty.",
                    false,
                    Some(ch),
                    Some(cobj),
                    None,
                    TO_CHAR,
                );
            } else {
                let buf = format!("You can't seem to find any {}s in $p.", arg);
                game.act(db, &buf, false, Some(ch), Some(cobj), None, TO_CHAR);
            }
        }
    }
}

fn perform_get_from_room(game: &mut Game, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId, oid: DepotId) -> bool {
    if can_take_obj(game, db,objs, chid, oid) {
        db.obj_from_room(objs,oid);
        db.obj_to_char(objs,oid, chid);
        let ch = db.ch(chid);
        let obj = objs.get(oid);
        game.act(
            db,
            "You get $p.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        game.act(
            db,
            "$n gets $p.",
            true,
            Some(ch),
            Some(obj),
            None,
            TO_ROOM,
        );
        get_check_money(game, db, objs,chid, oid);
        return true;
    }
    return false;
}

fn get_from_room(game: &mut Game, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId, arg: &str, howmany: i32) {
    let ch = db.ch(chid);
    let mut found = false;
    let mut howmany = howmany;
    let dotmode = find_all_dots(arg);

    if dotmode == FIND_INDIV {
        let mut obj =
            game.get_obj_in_list_vis2(db, objs,ch, arg, None, &db.world[ch.in_room() as usize].contents);
        if obj.is_none() {
            game.send_to_char(
                ch,
                format!("You don't see {} {} here.\r\n", an!(arg), arg).as_str(),
            );
        } else {
            while obj.is_some() {
                if howmany == 0 {
                    break;
                }
                howmany -= 1;
                let oid = obj.unwrap().id();
                perform_get_from_room(game, db, objs,chid, oid);
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis2(
                    db,
                    objs,ch,
                    arg,
                    None,
                    &db.world[ch.in_room() as usize].contents,
                );
            }
        }
    } else {
        if dotmode == FIND_ALLDOT && arg.is_empty() {
            game.send_to_char(ch, "Get all of what?\r\n");
            return;
        }
        for oid in db.world[ch.in_room() as usize].contents.clone() {
            let ch = db.ch(chid);
            if game.can_see_obj(db, ch, objs.get(oid))
                && (dotmode == FIND_ALL || isname(arg, &objs.get(oid).name))
            {
                found = true;
                perform_get_from_room(game, db, objs,chid, oid);
            }
        }
        let ch = db.ch(chid);
        if !found {
            if dotmode == FIND_ALL {
                game.send_to_char(ch, "There doesn't seem to be anything here.\r\n");
            } else {
                game.send_to_char(ch, format!("You don't see any {}s here.\r\n", arg).as_str());
            }
        }
    }
}

pub fn do_get(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut arg3 = String::new();
    let mut tmp_char = None;
    let mut c = None;

    let mut found = false;
    one_argument(two_arguments(argument, &mut arg1, &mut arg2), &mut arg3); /* three_arguments */

    if arg1.is_empty() {
        game.send_to_char(ch, "Get what?\r\n");
    } else if arg2.is_empty() {
        get_from_room(game, db, objs, chid, &arg1, 1);
    } else if is_number(&arg1) && arg3.is_empty() {
        get_from_room(game, db, objs,chid, &arg2, arg1.parse::<i32>().unwrap());
    } else {
        let mut amount = 1;
        if is_number(&arg1) {
            amount = arg1.parse::<i32>().unwrap();
            arg1 = arg2; /* strcpy: OK (sizeof: arg1 == arg2) */
            arg2 = arg3; /* strcpy: OK (sizeof: arg2 == arg3) */
        }
        let cont_dotmode = find_all_dots(&arg2);
        if cont_dotmode == FIND_INDIV {
            let mode = game.generic_find(
                db,objs,
                &arg2,
                (FIND_OBJ_INV | FIND_OBJ_ROOM) as i64,
                ch,
                &mut tmp_char,
                &mut c,
            );
            if c.is_none() {
                game.send_to_char(
                    ch,
                    format!("You don't have {} {}.\r\n", an!(&arg2), &arg2).as_str(),
                );
            } else if c.unwrap().get_obj_type() != ITEM_CONTAINER {
                game.act(
                    db,
                    "$p is not a container.",
                    false,
                    Some(ch),
                    Some(c.unwrap()),
                    None,
                    TO_CHAR,
                );
            } else {
                get_from_container(game, db,objs, chid, c.unwrap().id(), &arg1, mode, amount);
            }
        } else {
            if cont_dotmode == FIND_ALLDOT && arg2.is_empty() {
                game.send_to_char(ch, "Get from all of what?\r\n");
                return;
            }
            let list = ch.carrying.clone();
            for contid in list {
                let ch = db.ch(chid);
                let contobj = objs.get(contid);
                if game.can_see_obj(db, ch, contobj)
                    && (cont_dotmode == FIND_ALL || isname(&arg2, &contobj.name))
                {
                    if contobj.get_obj_type() == ITEM_CONTAINER {
                        found = true;
                        get_from_container(game, db,objs, chid, contid, &arg1, FIND_OBJ_INV, amount);
                    } else if cont_dotmode == FIND_ALLDOT {
                        found = true;
                        game.act(
                            db,
                            "$p is not a container.",
                            false,
                            Some(ch),
                            Some(contobj),
                            None,
                            TO_CHAR,
                        );
                    }
                }
            }
            let ch = db.ch(chid);
            for contid in db.world[ch.in_room() as usize].contents.clone() {
                let ch = db.ch(chid);
                let cont_obj = objs.get(contid);
                if game.can_see_obj(db, ch, cont_obj)
                    && (cont_dotmode == FIND_ALL || isname(&arg2, &cont_obj.name))
                {
                    if cont_obj.get_obj_type() == ITEM_CONTAINER {
                        get_from_container(game, db, objs,chid, contid, &arg1, FIND_OBJ_ROOM, amount);
                        found = true;
                    } else if cont_dotmode == FIND_ALLDOT {
                        game.act(
                            db,
                            "$p is not a container.",
                            false,
                            Some(ch),
                            Some(cont_obj),
                            None,
                            TO_CHAR,
                        );
                        found = true;
                    }
                }
            }
            if !found {
                if cont_dotmode == FIND_ALL {
                    let ch = db.ch(chid);
                    game.send_to_char(ch, "You can't seem to find any containers.\r\n");
                } else {
                    let ch = db.ch(chid);
                    game.send_to_char(
                        ch,
                        format!("You can't seem to find any {}s here.\r\n", &arg2).as_str(),
                    );
                }
            }
        }
    }
}

fn perform_drop_gold(
    game: &mut Game,
    db: &mut DB,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    amount: i32,
    mode: u8,
    rdr: RoomRnum,
) {
    let ch = db.ch(chid);
    if amount <= 0 {
        game.send_to_char(ch, "Heh heh heh.. we are jolly funny today, eh?\r\n");
    } else if ch.get_gold() < amount {
        game.send_to_char(ch, "You don't have that many coins!\r\n");
    } else {
        if mode != SCMD_JUNK as u8 {
            let ch = db.ch_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32); /* to prevent coin-bombing */

            let oid = db.create_money(objs,amount).unwrap();
            if mode == SCMD_DONATE as u8 {
                let ch = db.ch(chid);
                game.send_to_char(
                    ch,
                    "You throw some gold into the air where it disappears in a puff of smoke!\r\n",
                );
                game.act(
                    db,
                    "$n throws some gold into the air where it disappears in a puff of smoke!",
                    false,
                    Some(ch),
                    None,
                    None,
                    TO_ROOM,
                );
                db.obj_to_room(objs,oid, rdr);
                let obj = objs.get(oid);
                game.act(
                    db,
                    "$p suddenly appears in a puff of orange smoke!",
                    false,
                    None,
                    Some(obj),
                    None,
                    TO_ROOM,
                );
            } else {
                let buf = format!("$n drops {}.", money_desc(amount));
                let ch = db.ch(chid);
                game.act(db, &buf, true, Some(ch), None, None, TO_ROOM);
                game.send_to_char(ch, "You drop some gold.\r\n");
                db.obj_to_room(objs,oid, ch.in_room());
            }
        } else {
            let buf = format!(
                "$n drops {} which disappears in a puff of smoke!",
                money_desc(amount)
            );
            game.act(db, &buf, false, Some(ch), None, None, TO_ROOM);

            game.send_to_char(
                ch,
                "You drop some gold which disappears in a puff of smoke!\r\n",
            );
        }
        let ch = db.ch_mut(chid);
        ch.set_gold(ch.get_gold() - amount);
    }
}

macro_rules! vanish {
    ($mode:expr) => {
        (if $mode == SCMD_DONATE as u8 || $mode == SCMD_JUNK as u8 {
            "  It vanishes in a puff of smoke!"
        } else {
            ""
        })
    };
}

fn perform_drop(
    game: &mut Game,
    db: &mut DB,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    oid: DepotId,
    mut mode: u8,
    sname: &str,
    rdr: RoomRnum,
) -> i32 {
    let ch = db.ch(chid);
    let obj = objs.get(oid);
    if obj.obj_flagged(ITEM_NODROP) {
        let buf = format!("You can't {} $p, it must be CURSED!", sname);
        game.act(db, &buf, false, Some(ch), Some(obj), None, TO_CHAR);
        return 0;
    }

    let buf = format!("You {} $p.{}", sname, vanish!(mode));
    game.act(db, &buf, false, Some(ch), Some(obj), None, TO_CHAR);

    let buf = format!("$n {}s $p.{}", sname, vanish!(mode));
    game.act(db, &buf, true, Some(ch), Some(obj), None, TO_ROOM);

    db.obj_from_char(objs,oid);
    let obj = objs.get(oid);
    if (mode == SCMD_DONATE as u8) && obj.obj_flagged(ITEM_NODONATE) {
        mode = SCMD_JUNK as u8;
    }

    let ch = db.ch(chid);
    match mode {
        SCMD_DROP => {
            db.obj_to_room(objs,oid, ch.in_room());
        }

        SCMD_DONATE => {
            db.obj_to_room(objs,oid, rdr);
            let obj = objs.get(oid);
            game.act(
                db,
                "$p suddenly appears in a puff a smoke!",
                false,
                None,
                Some(obj),
                None,
                TO_ROOM,
            );
            return 0;
        }
        SCMD_JUNK => {
            let value = max(1, min(200, objs.get(oid).get_obj_cost() / 16));
            db.extract_obj( objs,oid);
            return value;
        }
        _ => {
            error!(
                "SYSERR: Incorrect argument {} passed to perform_drop.",
                mode
            );
        }
    }
    0
}

pub fn do_drop(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = db.ch(chid);
    let sname;
    let mut mode = SCMD_DROP;
    let mut rdr = 0;
    match subcmd as u8 {
        SCMD_JUNK => {
            sname = "junk";
            mode = SCMD_JUNK;
        }
        SCMD_DONATE => {
            sname = "donate";
            mode = SCMD_DONATE;
            match rand_number(0, 2) {
                0 => {
                    mode = SCMD_JUNK;
                }
                1 | 2 => {
                    rdr = db.real_room(DONATION_ROOM_1);
                }
                /*    case 3: RDR = real_room(donation_room_2); break;
                      case 4: RDR = real_room(donation_room_3); break;
                */
                _ => {}
            }
            if rdr == NOWHERE {
                game.send_to_char(ch, "Sorry, you can't donate anything right now.\r\n");
                return;
            }
        }
        _ => {
            sname = "drop";
        }
    }

    let mut arg = String::new();
    let argument = one_argument(argument, &mut arg);
    let mut obj;
    let mut amount = 0;
    let dotmode;

    if arg.is_empty() {
        game.send_to_char(ch, format!("What do you want to {}?\r\n", sname).as_str());
        return;
    } else if is_number(&arg) {
        let mut multi = arg.parse::<i32>().unwrap();
        one_argument(argument, &mut arg);
        if arg == "coins" || arg == "coin" {
            perform_drop_gold(game, db, objs,chid, multi, mode, rdr);
        } else if multi <= 0 {
            game.send_to_char(ch, "Yeah, that makes sense.\r\n");
        } else if arg.is_empty() {
            game.send_to_char(
                ch,
                format!("What do you want to {} {} of?\r\n", sname, multi).as_str(),
            );
        } else if {
            obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
            obj.is_none()
        } {
            game.send_to_char(
                ch,
                format!("You don't seem to have any {}s.\r\n", arg).as_str(),
            );
        } else {
            loop {
                amount += perform_drop(game, db, objs,chid, obj.unwrap().id(), mode, sname, rdr);
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
                multi -= 1;
                if multi == 0 {
                    break;
                }
            }
        }
    } else {
        dotmode = find_all_dots(&arg);

        /* Can't junk or donate all */
        if (dotmode == FIND_ALL) && (subcmd == SCMD_JUNK as i32 || subcmd == SCMD_DONATE as i32) {
            if subcmd == SCMD_JUNK as i32 {
                game.send_to_char(ch, "Go to the dump if you want to junk EVERYTHING!\r\n");
            } else {
                game.send_to_char(
                    ch,
                    "Go do the donation room if you want to donate EVERYTHING!\r\n",
                );
                return;
            }
        }
        if dotmode == FIND_ALL {
            let ch = db.ch(chid);
            if ch.carrying.is_empty() {
                game.send_to_char(ch, "You don't seem to be carrying anything.\r\n");
            } else {
                let list = ch.carrying.clone();
                for oid in list {
                    amount += perform_drop(game, db, objs,chid, oid, mode, sname, rdr);
                }
            }
        } else if dotmode == FIND_ALLDOT {
            if arg.is_empty() {
                game.send_to_char(
                    ch,
                    format!("What do you want to {} all of?\r\n", sname).as_str(),
                );
                return;
            }
            if {
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
                obj.is_none()
            } {
                game.send_to_char(
                    ch,
                    format!("You don't seem to have any {}s.\r\n", arg).as_str(),
                );
            }

            while obj.is_some() {
                amount += perform_drop(game, db,objs, chid, obj.unwrap().id(), mode, sname, rdr);
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
            }
        } else {
            if {
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis(db,objs, ch, &arg, None, &ch.carrying);
                obj.is_none()
            } {
                game.send_to_char(
                    ch,
                    format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
                );
            } else {
                amount += perform_drop(game, db, objs,chid, obj.unwrap().id(), mode, sname, rdr);
            }
        }
    }

    if amount != 0 && subcmd == SCMD_JUNK as i32 {
        let ch = db.ch(chid);
        game.send_to_char(ch, "You have been rewarded by the gods!\r\n");
        game.act(
            db,
            "$n has been rewarded by the gods!",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let ch = db.ch_mut(chid);
        ch.set_gold(ch.get_gold() + amount);
    }
}

fn perform_give(game: &mut Game, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId, vict_id: DepotId, oid: DepotId) {
    let ch = db.ch(chid);
    let vict = db.ch(vict_id);
    let obj = objs.get(oid);
    if obj.obj_flagged(ITEM_NODROP) {
        game.act(
            db,
            "You can't let go of $p!!  Yeech!",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return;
    }
    if vict.is_carrying_n() >= vict.can_carry_n() as u8 {
        game.act(
            db,
            "$N seems to have $S hands full.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR,
        );
        return;
    }
    if obj.get_obj_weight() + vict.is_carrying_w() > vict.can_carry_w() as i32 {
        game.act(
            db,
            "$E can't carry that much weight.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR,
        );
        return;
    }
    db.obj_from_char(objs,oid);
    db.obj_to_char(objs,oid, vict_id);
    let vict = db.ch(vict_id);
    let ch = db.ch(chid);
    let obj = objs.get(oid);
    game.act(
        db,
        "You give $p to $N.",
        false,
        Some(ch),
        Some(obj),
        Some(VictimRef::Char(vict)),
        TO_CHAR,
    );
    game.act(
        db,
        "$n gives you $p.",
        false,
        Some(ch),
        Some(obj),
        Some(VictimRef::Char(vict)),
        TO_VICT,
    );
    game.act(
        db,
        "$n gives $p to $N.",
        true,
        Some(ch),
        Some(obj),
        Some(VictimRef::Char(vict)),
        TO_NOTVICT,
    );
}

/* utility function for give */
fn give_find_vict<'a>(game: &mut Game, db: &'a DB, ch: &'a CharData, arg: &str) -> Option<&'a CharData> {
    let vict;
    let mut arg = arg.trim_start().to_string();

    if arg.is_empty() {
        game.send_to_char(ch, "To who?\r\n");
    } else if {
        vict = game.get_char_vis(db, ch, &mut arg, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        game.send_to_char(ch, NOPERSON);
    } else if vict.unwrap().id() == ch.id() {
        game.send_to_char(ch, "What's the point of that?\r\n");
    } else {
        return vict;
    }

    None
}

fn perform_give_gold(game: &mut Game, db: &mut DB, chid: DepotId, vict_id: DepotId, amount: i32) {
    let ch = db.ch(chid);
    let vict = db.ch(vict_id);
    let mut buf;

    if amount <= 0 {
        game.send_to_char(ch, "Heh heh heh ... we are jolly funny today, eh?\r\n");
        return;
    }
    if ch.get_gold() < amount && (ch.is_npc() || (ch.get_level() < LVL_GOD as u8)) {
        game.send_to_char(ch, "You don't have that many coins!\r\n");
        return;
    }
    game.send_to_char(ch, OK);

    buf = format!(
        "$n gives you {} gold coin{}.",
        amount,
        if amount == 1 { "" } else { "s" }
    );
    game.act(
        db,
        &buf,
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_VICT,
    );

    buf = format!("$n gives {} to $N.", money_desc(amount));
    game.act(
        db,
        &buf,
        true,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_NOTVICT,
    );
    let ch = db.ch(chid);

    if ch.is_npc() || ch.get_level() < LVL_GOD as u8 {
        let ch = db.ch_mut(chid);
        ch.set_gold(ch.get_gold() - amount);
    }
    let vict = db.ch_mut(vict_id);
    vict.set_gold(vict.get_gold() + amount);
}

pub fn do_give(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg = String::new();

    let mut argument = one_argument(argument, &mut arg);
    let mut amount;
    let mut vict = None;
    let mut obj = None;

    if arg.is_empty() {
        game.send_to_char(ch, "Give what to who?\r\n");
    } else if is_number(&arg) {
        amount = arg.parse::<i32>().unwrap();
        argument = one_argument(argument, &mut arg);
        if arg == "coins" || arg == "coin" {
            one_argument(argument, &mut arg);
            if {
                vict = give_find_vict(game, db, ch, &arg);
                vict.is_some()
            } {
                perform_give_gold(game, db, chid, vict.unwrap().id(), amount);
                return;
            } else if arg.is_empty() {
                /* Give multiple code. */
                let ch = db.ch(chid);
                game.send_to_char(
                    ch,
                    format!("What do you want to give {} of?\r\n", amount).as_str(),
                );
            } else if {
                vict = give_find_vict(game, db, ch, argument);
                vict.is_none()
            } {
                return;
            } else if {
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
                obj.is_none()
            } {
            }
            let ch = db.ch(chid);
            game.send_to_char(
                ch,
                format!("You don't seem to have any {}s.\r\n", arg).as_str(),
            );
        } else {
            let vict_id = vict.unwrap().id();
            while obj.is_some() && amount != 0 {
                amount -= 1;
                perform_give(game, db,objs, chid, vict_id, obj.unwrap().id());
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
            }
        }
    } else {
        let mut buf1 = String::new();
        one_argument(argument, &mut buf1);
        if {
            vict = give_find_vict(game, db, ch, &buf1);
            vict.is_none()
        } {
            return;
        }
        let dotmode = find_all_dots(&arg);
        if dotmode == FIND_INDIV {
            if {
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
                obj.is_none()
            } {
                let ch = db.ch(chid);
                game.send_to_char(
                    ch,
                    format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
                );
            } else {
                perform_give(game, db,objs,chid, vict.unwrap().id(), obj.unwrap().id());
            }
        } else {
            if dotmode == FIND_ALLDOT && arg.is_empty() {
                let ch = db.ch(chid);
                game.send_to_char(ch, "All of what?\r\n");
                return;
            }
            let ch = db.ch(chid);
            if ch.carrying.len() == 0 {
                game.send_to_char(ch, "You don't seem to be holding anything.\r\n");
            } else {
                let list = ch.carrying.clone();
                let vict_id = vict.unwrap().id();
                for oid in list {
                    let ch = db.ch(chid);
                    let obj = objs.get(oid);
                    if game.can_see_obj(db, ch, obj)
                        && (dotmode == FIND_ALL || isname(&arg, &obj.name))
                    {
                        perform_give(game, db, objs,chid, vict_id, oid);
                    }
                }
            }
        }
    }
}

pub fn weight_change_object(db: &mut DB,objs: &mut Depot<ObjData>,  oid: DepotId, weight: i32) {
    let tmp_ch;
    let tmp_obj;
    if objs.get(oid).in_room() != NOWHERE {
        objs.get_mut(oid).incr_obj_weight(weight);
    } else if {
        tmp_ch = objs.get(oid).carried_by.clone();
        tmp_ch.is_some()
    } {
        db.obj_from_char(objs,oid);
        objs.get_mut(oid).incr_obj_weight(weight);
        db.obj_to_char(objs,oid, tmp_ch.unwrap());
    } else if {
        tmp_obj = objs.get(oid).in_obj;
        tmp_obj.is_some()
    } {
        db.obj_from_obj(objs,oid);
        objs.get_mut(oid).incr_obj_weight(weight);
        db.obj_to_obj(objs,oid, tmp_obj.unwrap());
    } else {
        error!("SYSERR: Unknown attempt to subtract weight from an object.");
    }
}

pub fn name_from_drinkcon( objs: &mut Depot<ObjData>, oid: Option<DepotId>) {
    if oid.is_none()
        || objs.get(oid.unwrap()).get_obj_type() != ITEM_DRINKCON
            && objs.get(oid.unwrap()).get_obj_type() != ITEM_FOUNTAIN
    {
        return;
    }
    let oid = oid.unwrap();

    let liqname = DRINKNAMES[objs.get(oid).get_obj_val(2) as usize];
    if !isname(liqname, &objs.get(oid).name) {
        error!(
            "SYSERR: Can't remove liquid '{}' from '{}' ({}) item.",
            liqname,
            objs.get(oid).name,
            objs.get(oid).item_number
        );
        return;
    }

    let mut new_name = String::new();
    let next = "";
    let bname = &objs.get(oid).name;
    let mut cur_name = bname.as_ref();
    while cur_name.len() != 0 {
        if cur_name.starts_with(' ') {
            cur_name = &cur_name[1..];
        }
        let i = cur_name.find(' ');
        let cpylen;
        if i.is_some() {
            cpylen = i.unwrap();
        } else {
            cpylen = cur_name.len();
        }

        if cur_name.starts_with(liqname) {
            cur_name = next;
            continue;
        }

        if new_name.len() != 0 {
            new_name.push(' ');
        } else {
            new_name.push_str(&cur_name[0..cpylen])
        }
        cur_name = next;
    }

    objs.get_mut(oid).name = Rc::from(new_name.as_str());
}

pub fn name_to_drinkcon(objs: &mut Depot<ObjData>,  oid: Option<DepotId>, type_: i32) {
    let mut new_name = String::new();
    if oid.is_none()
        || objs.get(oid.unwrap()).get_obj_type() != ITEM_DRINKCON
            && objs.get(oid.unwrap()).get_obj_type() != ITEM_FOUNTAIN
    {
        return;
    }
    new_name.push_str(
        format!(
            "{} {}",
            objs.get(oid.unwrap()).name.as_ref(),
            DRINKNAMES[type_ as usize]
        )
        .as_str(),
    );

    objs.get_mut(oid.unwrap()).name = Rc::from(new_name.as_str());
}

pub fn do_drink(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg = String::new();

    one_argument(argument, &mut arg);

    if ch.is_npc() {
        /* Cannot use ) on mobs. */
        return;
    }

    if arg.len() == 0 {
        game.send_to_char(ch, "Drink from what?\r\n");
        return;
    }
    let mut tobj;
    let mut on_ground = false;
    if {
        tobj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
        tobj.is_none()
    } {
        if {
            tobj = game.get_obj_in_list_vis2(
                db,objs,
                ch,
                &arg,
                None,
                &db.world[ch.in_room() as usize].contents,
            );
            tobj.is_none()
        } {
            game.send_to_char(ch, "You can't find it!\r\n");
            return;
        } else {
            on_ground = true;
        }
    }
    let to_obj = tobj.unwrap();
    if to_obj.get_obj_type() != ITEM_DRINKCON && to_obj.get_obj_type() != ITEM_FOUNTAIN
    {
        game.send_to_char(ch, "You can't drink from that!\r\n");
        return;
    }
    if on_ground && to_obj.get_obj_type() == ITEM_DRINKCON {
        game.send_to_char(ch, "You have to be holding that to drink from it.\r\n");
        return;
    }
    if ch.get_cond(DRUNK) > 10 && ch.get_cond(THIRST) > 0 {
        /* The pig is drunk */
        game.send_to_char(ch, "You can't seem to get close enough to your mouth.\r\n");
        game.act(
            db,
            "$n tries to drink but misses $s mouth!",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        return;
    }
    if ch.get_cond(FULL) > 20 && ch.get_cond(THIRST) > 0 {
        game.send_to_char(ch, "Your stomach can't contain anymore!\r\n");
        return;
    }
    if to_obj.get_obj_val(1) == 0 {
        game.send_to_char(ch, "It's empty.\r\n");
        return;
    }
    let mut amount;
    if subcmd == SCMD_DRINK {
        let buf = format!(
            "$n DRINKS {} from $p.",
            DRINKS[to_obj.get_obj_val(2) as usize]
        );
        game.act(db, &buf, true, Some(ch), Some(to_obj), None, TO_ROOM);

        game.send_to_char(
            ch,
            format!(
                "You drink the {}.\r\n",
                DRINKS[to_obj.get_obj_val(2) as usize]
            )
            .as_str(),
        );
        let ch = db.ch(chid);
        if DRINK_AFF[to_obj.get_obj_val(2) as usize][DRUNK as usize] > 0 {
            amount = (25 - ch.get_cond(THIRST)) as i32
                / DRINK_AFF[to_obj.get_obj_val(2) as usize][DRUNK as usize];
        } else {
            amount = rand_number(3, 10) as i32;
        }
    } else {
        game.act(
            db,
            "$n sips from $p.",
            true,
            Some(ch),
            Some(to_obj),
            None,
            TO_ROOM,
        );
        game.send_to_char(
            ch,
            format!(
                "It tastes like {}.\r\n",
                DRINKS[to_obj.get_obj_val(2) as usize]
            )
            .as_str(),
        );
        amount = 1;
    }

    amount = min(amount, to_obj.get_obj_val(1));

    /* You can't subtract more than the object weighs */
    let weight = min(amount, to_obj.get_obj_weight());
    let toid = to_obj.id();
    weight_change_object(db, objs,toid, -weight as i32); /* Subtract amount */
    let to_obj = objs.get(toid);
    game.gain_condition(
        db,
        chid,
        DRUNK,
        DRINK_AFF[to_obj.get_obj_val(2) as usize][DRUNK as usize] * amount / 4,
    );
    let to_obj = objs.get(toid);
    game.gain_condition(
        db,
        chid,
        FULL,
        DRINK_AFF[to_obj.get_obj_val(2) as usize][FULL as usize] * amount / 4,
    );
    let to_obj = objs.get(toid);
    game.gain_condition(
        db,
        chid,
        THIRST,
        DRINK_AFF[to_obj.get_obj_val(2) as usize][THIRST as usize] * amount / 4,
    );
    let ch = db.ch(chid);

    if ch.get_cond(DRUNK) > 10 {
        game.send_to_char(ch, "You feel drunk.\r\n");
    }
    let ch = db.ch(chid);

    if ch.get_cond(THIRST) > 20 {
        game.send_to_char(ch, "You don't feel thirsty any more.\r\n");
    }
    let ch = db.ch(chid);

    if ch.get_cond(FULL) > 20 {
        game.send_to_char(ch, "You are full.\r\n");
    }
    let to_obj = objs.get(toid);
    if to_obj.get_obj_val(3) != 0 {
        /* The crap was poisoned ! */
        game.send_to_char(ch, "Oops, it tasted rather strange!\r\n");
        game.act(
            db,
            "$n chokes and utters some strange sounds.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let mut af = AffectedType {
            _type: SPELL_POISON as i16,
            duration: (amount * 3) as i16,
            modifier: 0,
            location: APPLY_NONE as u8,
            bitvector: AFF_POISON,
        };
        db.affect_join(objs,chid, &mut af, false, false, false, false);
    }
    /* empty the container, and no longer poison. */
    let to_obj = objs.get(toid);
    let v = to_obj.get_obj_val(1) - amount;
    let to_obj = objs.get_mut(toid);
    to_obj.set_obj_val(1, v);

    if to_obj.get_obj_val(1) == 0 {
        /* The last bit */
        name_from_drinkcon( objs,Some(toid));
        let to_obj = objs.get_mut(toid);
        to_obj.set_obj_val(2, 0);
        to_obj.set_obj_val(3, 0);
    }
    return;
}

pub fn do_eat(
    game: &mut Game,
    db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if ch.is_npc() {
        /* Cannot use ) on mobs. */
        return;
    }

    if arg.len() == 0 {
        game.send_to_char(ch, "Eat what?\r\n");
        return;
    }
    let food;
    if {
        food = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
        food.is_none()
    } {
        game.send_to_char(
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
        return;
    }
    let food = food.unwrap();
    if subcmd == SCMD_TASTE
        && (food.get_obj_type() == ITEM_DRINKCON
            || food.get_obj_type() == ITEM_FOUNTAIN)
    {
        do_drink(game, db, texts,objs,chid, argument, 0, SCMD_SIP);
        return;
    }
    if (food.get_obj_type() != ITEM_FOOD) && (ch.get_level() < LVL_GOD as u8) {
        game.send_to_char(ch, "You can't eat THAT!\r\n");
        return;
    }
    if ch.get_cond(FULL) > 20 {
        /* Stomach full */
        game.send_to_char(ch, "You are too full to eat more!\r\n");
        return;
    }
    if subcmd == SCMD_EAT {
        game.act(
            db,
            "You eat $p.",
            false,
            Some(ch),
            Some(food),
            None,
            TO_CHAR,
        );
        game.act(
            db,
            "$n eats $p.",
            true,
            Some(ch),
            Some(food),
            None,
            TO_ROOM,
        );
    } else {
        game.act(
            db,
            "You nibble a little bit of $p.",
            false,
            Some(ch),
            Some(food),
            None,
            TO_CHAR,
        );
        game.act(
            db,
            "$n tastes a little bit of $p.",
            true,
            Some(ch),
            Some(food),
            None,
            TO_ROOM,
        );
    }

    let amount = if subcmd == SCMD_EAT {
        food.get_obj_val(0)
    } else {
        1
    };
    let food_id = food.id();
    game.gain_condition(db, chid, FULL, amount);
    let ch = db.ch(chid);

    if ch.get_cond(FULL) > 20 {
        game.send_to_char(ch, "You are full.\r\n");
    }
    let ch = db.ch(chid);
    let food = objs.get(food_id);
    if food.get_obj_val(3) != 0 && (ch.get_level() < LVL_IMMORT as u8) {
        /* The crap was poisoned ! */
        game.send_to_char(ch, "Oops, that tasted rather strange!\r\n");
        game.act(
            db,
            "$n coughs and utters some strange sounds.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );

        let mut af = AffectedType {
            _type: SPELL_POISON as i16,
            duration: (amount * 2) as i16,
            modifier: 0,
            location: APPLY_NONE as u8,
            bitvector: AFF_POISON,
        };
        db.affect_join(objs,chid, &mut af, false, false, false, false);
    }
    if subcmd == SCMD_EAT {
        db.extract_obj( objs,food_id);
    } else {
        if {
            objs.get_mut(food_id).decr_obj_val(1);
            let food = objs.get_mut(food_id);
            food.get_obj_val(0) == 0
        } {
            let ch = db.ch(chid);
            game.send_to_char(ch, "There's nothing left now.\r\n");
            db.extract_obj(objs, food_id);
        }
    }
}

pub fn do_pour(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut from_obj = None;
    let mut to_obj = None;
    let mut amount;

    two_arguments(argument, &mut arg1, &mut arg2);

    if subcmd == SCMD_POUR {
        if arg1.is_empty() {
            /* No arguments */
            game.send_to_char(ch, "From what do you want to pour?\r\n");
            return;
        }
        if {
            from_obj = game.get_obj_in_list_vis(db, objs,ch, &arg1, None, &ch.carrying);
            from_obj.is_none()
        } {
            game.send_to_char(ch, "You can't find it!\r\n");
            return;
        }
        let from_obj = from_obj.unwrap();
        if from_obj.get_obj_type() != ITEM_DRINKCON {
            game.send_to_char(ch, "You can't pour from that!\r\n");
            return;
        }
    }
    if subcmd == SCMD_FILL {
        if arg1.is_empty() {
            /* no arguments */
            game.send_to_char(
                ch,
                "What do you want to fill?  And what are you filling it from?\r\n",
            );
            return;
        }
        if {
            to_obj = game.get_obj_in_list_vis(db, objs,ch, &arg1, None, &ch.carrying);
            to_obj.is_none()
        } {
            game.send_to_char(ch, "You can't find it!\r\n");
            return;
        }
        let to_obj = to_obj.unwrap();
        if to_obj.get_obj_type() != ITEM_DRINKCON {
            game.act(
                db,
                "You can't fill $p!",
                false,
                Some(ch),
                Some(to_obj),
                None,
                TO_CHAR,
            );
            return;
        }
        if arg2.is_empty() {
            /* no 2nd argument */
            game.act(
                db,
                "What do you want to fill $p from?",
                false,
                Some(ch),
                Some(to_obj),
                None,
                TO_CHAR,
            );
            return;
        }
        if {
            from_obj = game.get_obj_in_list_vis2(
                db,objs,
                ch,
                &arg2,
                None,
                &db.world[ch.in_room() as usize].contents,
            );
            from_obj.is_none()
        } {
            game.send_to_char(
                ch,
                format!("There doesn't seem to be {} {} here.\r\n", an!(arg2), arg2).as_str(),
            );
            return;
        }
        let from_obj = from_obj.unwrap();
        if from_obj.get_obj_type() != ITEM_FOUNTAIN {
            game.act(
                db,
                "You can't fill something from $p.",
                false,
                Some(ch),
                Some(from_obj),
                None,
                TO_CHAR,
            );
            return;
        }
    }
    let from_obj = from_obj.unwrap();

    if from_obj.get_obj_val(1) == 0 {
        game.act(
            db,
            "The $p is empty.",
            false,
            Some(ch),
            Some(from_obj),
            None,
            TO_CHAR,
        );
        return;
    }
    if subcmd == SCMD_POUR {
        /* pour */
        if arg2.is_empty() {
            game.send_to_char(ch, "Where do you want it?  Out or in what?\r\n");
            return;
        }
        if arg2 == "out" {
            game.act(
                db,
                "$n empties $p.",
                true,
                Some(ch),
                Some(from_obj),
                None,
                TO_ROOM,
            );
            game.act(
                db,
                "You empty $p.",
                false,
                Some(ch),
                Some(from_obj),
                None,
                TO_CHAR,
            );
            let from_obj_id = from_obj.id();
            weight_change_object(db, objs,from_obj_id, -from_obj.get_obj_val(1)); /* Empty */

            name_from_drinkcon(objs, Some(from_obj_id));
            let from_obj = objs.get_mut(from_obj_id);
            from_obj.set_obj_val(1, 0);
            from_obj.set_obj_val(2, 0);
            from_obj.set_obj_val(3, 0);

            return;
        }
        if {
            to_obj = game.get_obj_in_list_vis(db, objs,ch, &arg2, None, &ch.carrying);
            to_obj.is_none()
        } {
            game.send_to_char(ch, "You can't find it!\r\n");
            return;
        }
        let to_obj = to_obj.unwrap();
        if (to_obj.get_obj_type() != ITEM_DRINKCON)
            && (to_obj.get_obj_type() != ITEM_FOUNTAIN)
        {
            game.send_to_char(ch, "You can't pour anything into that.\r\n");
            return;
        }
    }

    let to_obj = to_obj.unwrap();
    if to_obj.id() == from_obj.id() {
        game.send_to_char(ch, "A most unproductive effort.\r\n");
        return;
    }

    if (to_obj.get_obj_val(1) != 0)
        && (to_obj.get_obj_val(2) != to_obj.get_obj_val(2))
    {
        game.send_to_char(ch, "There is already another liquid in it!\r\n");
        return;
    }
    if !(to_obj.get_obj_val(1) < to_obj.get_obj_val(0)) {
        game.send_to_char(ch, "There is no room for more.\r\n");
        return;
    }
    if subcmd == SCMD_POUR {
        game.send_to_char(
            ch,
            format!(
                "You pour the {} into the {}.",
                DRINKS[to_obj.get_obj_val(2) as usize],
                arg2
            )
            .as_str(),
        );
    }

    if subcmd == SCMD_FILL {
        game.act(
            db,
            "You gently fill $p from $P.",
            false,
            Some(ch),
            Some(to_obj),
            Some(VictimRef::Obj(from_obj)),
            TO_CHAR,
        );
        game.act(
            db,
            "$n gently fills $p from $P.",
            true,
            Some(ch),
            Some(to_obj),
            Some(VictimRef::Obj(from_obj)),
            TO_ROOM,
        );
    }
    let to_obj_id = to_obj.id();
    let from_obj_id = from_obj.id();
    /* New alias */
    if to_obj.get_obj_val(1) == 0 {
        let _type = to_obj.get_obj_val(2);
        name_to_drinkcon( objs,Some(to_obj_id), _type);
    }
    /* First same type liq. */
    let v = objs.get(from_obj_id).get_obj_val(2);
    objs.get_mut(to_obj_id).set_obj_val(2, v);

    /* Then how much to pour */
    let v = objs.get(from_obj_id).get_obj_val(1) - {
        amount = objs.get(to_obj_id).get_obj_val(0) - objs.get(to_obj_id).get_obj_val(1);
        amount
    };
    objs.get_mut(from_obj_id).set_obj_val(1, v);
    let v = objs.get(to_obj_id).get_obj_val(0);
    objs.get_mut(to_obj_id).set_obj_val(1, v);

    if objs.get(from_obj_id).get_obj_val(1) < 0 {
        /* There was too little */
        let v = objs.get(to_obj_id).get_obj_val(1) + objs.get(from_obj_id).get_obj_val(1);
        objs.get_mut(to_obj_id).set_obj_val(1, v);
        amount += objs.get(from_obj_id).get_obj_val(1);
        name_from_drinkcon(objs,Some(from_obj_id));
        objs.get_mut(from_obj_id).set_obj_val(1, 0);
        objs.get_mut(from_obj_id).set_obj_val(2, 0);
        objs.get_mut(from_obj_id).set_obj_val(3, 0);
    }
    /* Then the poison boogie */
    let v = if objs.get(to_obj_id).get_obj_val(3) != 0 || objs.get(from_obj_id).get_obj_val(3) != 0 {
        1
    } else {
        0
    };
    objs.get_mut(to_obj_id).set_obj_val(3, v);

    /* And the weight boogie */
    weight_change_object(db, objs,from_obj_id, -amount);
    weight_change_object(db, objs,to_obj_id, amount); /* Add weight */
}

fn wear_message(game: &mut Game, db: &mut DB,objs: & Depot<ObjData>,  chid: DepotId, oid: DepotId, _where: i32) {
    let ch = db.ch(chid);
    let obj = objs.get(oid);
    const WEAR_MESSAGES: [[&str; 2]; 18] = [
        ["$n lights $p and holds it.", "You light $p and hold it."],
        [
            "$n slides $p on to $s right ring finger.",
            "You slide $p on to your right ring finger.",
        ],
        [
            "$n slides $p on to $s left ring finger.",
            "You slide $p on to your left ring finger.",
        ],
        [
            "$n wears $p around $s neck.",
            "You wear $p around your neck.",
        ],
        [
            "$n wears $p around $s neck.",
            "You wear $p around your neck.",
        ],
        ["$n wears $p on $s body.", "You wear $p on your body."],
        ["$n wears $p on $s head.", "You wear $p on your head."],
        ["$n puts $p on $s legs.", "You put $p on your legs."],
        ["$n wears $p on $s feet.", "You wear $p on your feet."],
        ["$n puts $p on $s hands.", "You put $p on your hands."],
        ["$n wears $p on $s arms.", "You wear $p on your arms."],
        [
            "$n straps $p around $s arm as a shield.",
            "You start to use $p as a shield.",
        ],
        [
            "$n wears $p about $s body.",
            "You wear $p around your body.",
        ],
        [
            "$n wears $p around $s waist.",
            "You wear $p around your waist.",
        ],
        [
            "$n puts $p on around $s right wrist.",
            "You put $p on around your right wrist.",
        ],
        [
            "$n puts $p on around $s left wrist.",
            "You put $p on around your left wrist.",
        ],
        ["$n wields $p.", "You wield $p."],
        ["$n grabs $p.", "You grab $p."],
    ];

    game.act(
        db,
        WEAR_MESSAGES[_where as usize][0],
        true,
        Some(ch),
        Some(obj),
        None,
        TO_ROOM,
    );
    game.act(
        db,
        WEAR_MESSAGES[_where as usize][1],
        false,
        Some(ch),
        Some(obj),
        None,
        TO_CHAR,
    );
}

fn perform_wear(game: &mut Game, db: &mut DB, objs: &mut Depot<ObjData>, chid: DepotId, oid: DepotId, _where: i32) {
    /*
     * ITEM_WEAR_TAKE is used for objects that do not require special bits
     * to be put into that position (e.g. you can hold any object, not just
     * an object with a HOLD bit.)
     */
    let ch = db.ch(chid);
    let obj = objs.get(oid);
    let mut _where = _where;
    const WEAR_BITVECTORS: [i32; 18] = [
        ITEM_WEAR_TAKE,
        ITEM_WEAR_FINGER,
        ITEM_WEAR_FINGER,
        ITEM_WEAR_NECK,
        ITEM_WEAR_NECK,
        ITEM_WEAR_BODY,
        ITEM_WEAR_HEAD,
        ITEM_WEAR_LEGS,
        ITEM_WEAR_FEET,
        ITEM_WEAR_HANDS,
        ITEM_WEAR_ARMS,
        ITEM_WEAR_SHIELD,
        ITEM_WEAR_ABOUT,
        ITEM_WEAR_WAIST,
        ITEM_WEAR_WRIST,
        ITEM_WEAR_WRIST,
        ITEM_WEAR_WIELD,
        ITEM_WEAR_TAKE,
    ];

    const ALREADY_WEARING: [&str; 18] = [
        "You're already using a light.\r\n",
        "YOU SHOULD NEVER SEE THIS MESSAGE.  PLEASE REPORT.\r\n",
        "You're already wearing something on both of your ring fingers.\r\n",
        "YOU SHOULD NEVER SEE THIS MESSAGE.  PLEASE REPORT.\r\n",
        "You can't wear anything else around your neck.\r\n",
        "You're already wearing something on your body.\r\n",
        "You're already wearing something on your head.\r\n",
        "You're already wearing something on your legs.\r\n",
        "You're already wearing something on your feet.\r\n",
        "You're already wearing something on your hands.\r\n",
        "You're already wearing something on your arms.\r\n",
        "You're already using a shield.\r\n",
        "You're already wearing something about your body.\r\n",
        "You already have something around your waist.\r\n",
        "YOU SHOULD NEVER SEE THIS MESSAGE.  PLEASE REPORT.\r\n",
        "You're already wearing something around both of your wrists.\r\n",
        "You're already wielding a weapon.\r\n",
        "You're already holding something.\r\n",
    ];

    /* first, make sure that the wear position is valid. */
    if !obj.can_wear(WEAR_BITVECTORS[_where as usize]) {
        game.act(
            db,
            "You can't wear $p there.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return;
    }
    /* for neck, finger, and wrist, try pos 2 if pos 1 is already full */
    if (_where == WEAR_FINGER_R as i32)
        || (_where == WEAR_NECK_1 as i32)
        || (_where == WEAR_WRIST_R as i32)
    {
        if ch.get_eq(_where as i8).is_some() {
            _where += 1;
        }
    }

    if ch.get_eq(_where as i8).is_some() {
        game.send_to_char(ch, ALREADY_WEARING[_where as usize]);
        return;
    }
    wear_message(game, db, objs,chid, oid, _where);
    db.obj_from_char(objs,oid);
    game.equip_char(db, objs,chid, oid, _where as i8);
}

pub fn find_eq_pos(game: &mut Game, ch: &CharData, obj: &ObjData, arg: &str) -> i16 {
    let mut _where = -1;

    const KEYWORDS: [&str; 19] = [
        "!RESERVED!",
        "finger",
        "!RESERVED!",
        "neck",
        "!RESERVED!",
        "body",
        "head",
        "legs",
        "feet",
        "hands",
        "arms",
        "shield",
        "about",
        "waist",
        "wrist",
        "!RESERVED!",
        "!RESERVED!",
        "!RESERVED!",
        "\n",
    ];
    let _where_o;
    if arg.is_empty() {
        if obj.can_wear(ITEM_WEAR_FINGER) {
            _where = WEAR_FINGER_R;
        }
        if obj.can_wear(ITEM_WEAR_NECK) {
            _where = WEAR_NECK_1;
        }
        if obj.can_wear(ITEM_WEAR_BODY) {
            _where = WEAR_BODY;
        }
        if obj.can_wear(ITEM_WEAR_HEAD) {
            _where = WEAR_HEAD;
        }
        if obj.can_wear(ITEM_WEAR_LEGS) {
            _where = WEAR_LEGS;
        }
        if obj.can_wear(ITEM_WEAR_FEET) {
            _where = WEAR_FEET;
        }
        if obj.can_wear(ITEM_WEAR_HANDS) {
            _where = WEAR_HANDS;
        }
        if obj.can_wear(ITEM_WEAR_ARMS) {
            _where = WEAR_ARMS;
        }
        if obj.can_wear(ITEM_WEAR_SHIELD) {
            _where = WEAR_SHIELD;
        }
        if obj.can_wear(ITEM_WEAR_ABOUT) {
            _where = WEAR_ABOUT;
        }
        if obj.can_wear(ITEM_WEAR_WAIST) {
            _where = WEAR_WAIST;
        }
        if obj.can_wear(ITEM_WEAR_WRIST) {
            _where = WEAR_WRIST_R;
        }
    } else if {
        _where_o = search_block(arg, &KEYWORDS, false);
        _where_o.is_none()
    } {
        game.send_to_char(
            ch,
            format!("'{}'?  What part of your body is THAT?\r\n", arg).as_str(),
        );
    } else {
        _where = _where_o.unwrap() as i16;
    }

    _where
}

pub fn do_wear(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg1 = String::new();
    let mut arg2 = String::new();

    two_arguments(argument, &mut arg1, &mut arg2);

    if arg1.is_empty() {
        game.send_to_char(ch, "Wear what?\r\n");
        return;
    }
    let dotmode = find_all_dots(&arg1);

    if !arg2.is_empty() && dotmode != FIND_INDIV {
        game.send_to_char(
            ch,
            "You can't specify the same body location for more than one item!\r\n",
        );
        return;
    }
    let mut _where = -1;
    let mut items_worn = 0;
    if dotmode == FIND_ALL {
        for oid in ch.carrying.clone() {
            let ch = db.ch(chid);
            let obj = objs.get(oid);
            if game.can_see_obj(db, ch, obj) && {
                _where = find_eq_pos(game, ch, obj, "");
                _where >= 0
            } {
                items_worn += 1;
                perform_wear(game, db, objs,chid, oid, _where as i32);
            }
        }
        if items_worn == 0 {
            let ch = db.ch(chid);
            game.send_to_char(ch, "You don't seem to have anything wearable.\r\n");
        }
    } else if dotmode == FIND_ALLDOT {
        if arg1.is_empty() {
            game.send_to_char(ch, "Wear all of what?\r\n");
            return;
        }
        let mut obj;
        if {
            obj = game.get_obj_in_list_vis(db, objs,ch, &arg1, None, &ch.carrying);
            obj.is_none()
        } {
            game.send_to_char(
                ch,
                format!("You don't seem to have any {}s.\r\n", arg1).as_str(),
            );
        } else {
            while obj.is_some() {
                let ch = db.ch(chid);
                if {
                    _where = find_eq_pos(game,  ch, obj.unwrap(), "");
                    _where >= 0
                } {
                    perform_wear(game, db, objs,chid, obj.unwrap().id(), _where as i32);
                } else {
                    game.act(
                        db,
                        "You can't wear $p.",
                        false,
                        Some(ch),
                        obj,
                        None,
                        TO_CHAR,
                    );
                }
                let ch = db.ch(chid);
                obj = game.get_obj_in_list_vis(db, objs,ch, &arg1, None, &ch.carrying);
            }
        }
    } else {
        let obj;
        if {
            obj = game.get_obj_in_list_vis(db, objs,ch, &arg1, None, &ch.carrying);
            obj.is_none()
        } {
            game.send_to_char(
                ch,
                format!("You don't seem to have {} {}.\r\n", an!(arg1), arg1).as_str(),
            );
        } else {
            if {
                _where = find_eq_pos(game,  ch, obj.unwrap(), &arg2);
                _where >= 0
            } {
                perform_wear(game, db, objs,chid, obj.unwrap().id(), _where as i32);
            } else if arg2.is_empty() {
                game.act(
                    db,
                    "You can't wear $p.",
                    false,
                    Some(ch),
                    obj,
                    None,
                    TO_CHAR,
                );
            }
        }
    }
}

pub fn do_wield(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg = String::new();

    let obj;
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        game.send_to_char(ch, "Wield what?\r\n");
    } else if {
        obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
        obj.is_none()
    } {
        game.send_to_char(
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
    } else {
        let obj = obj.unwrap();
        if !obj.can_wear(ITEM_WEAR_WIELD) {
            game.send_to_char(ch, "You can't wield that.\r\n");
        } else if obj.get_obj_weight() > STR_APP[ch.strength_apply_index()].wield_w as i32 {
            game.send_to_char(ch, "It's too heavy for you to use.\r\n");
        } else {
            perform_wear(game, db, objs,chid, obj.id(), WEAR_WIELD as i32);
        }
    }
}

pub fn do_grab(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg = String::new();
    let obj;
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        game.send_to_char(ch, "Hold what?\r\n");
    } else if {
        obj = game.get_obj_in_list_vis(db, objs,ch, &arg, None, &ch.carrying);
        obj.is_none()
    } {
        game.send_to_char(
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
    } else {
        let obj = obj.unwrap();

        if obj.get_obj_type() == ITEM_LIGHT {
            perform_wear(game, db, objs,chid, obj.id(), WEAR_LIGHT as i32);
        } else {
            if !obj.can_wear(ITEM_WEAR_HOLD)
                && obj.get_obj_type() != ITEM_WAND
                && obj.get_obj_type() != ITEM_STAFF
                && obj.get_obj_type() != ITEM_SCROLL
                && obj.get_obj_type() != ITEM_POTION
            {
                game.send_to_char(ch, "You can't hold that.\r\n");
            } else {
                perform_wear(game, db, objs,chid, obj.id(), WEAR_HOLD as i32);
            }
        }
    }
}

fn perform_remove(game: &mut Game, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId, pos: i8) {
    let ch = db.ch(chid);
    let oid;

    if {
        oid = ch.get_eq(pos as i8);
        oid.is_none()
    } {
        error!("SYSERR: perform_remove: bad pos {} passed.", pos);
    } else if objs.get(oid.unwrap()).obj_flagged(ITEM_NODROP) {
        let oid = oid.unwrap();
        let obj = objs.get(oid);
        game.act(
            db,
            "You can't remove $p, it must be CURSED!",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
    } else if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
        let oid = oid.unwrap();
        let obj = objs.get(oid);
        game.act(
            db,
            "$p: you can't carry that many items!",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
    } else {
        let oid = oid.unwrap();
        let eqid = db.unequip_char( objs,chid, pos).unwrap();
        db.obj_to_char(objs,eqid, chid);
        let ch = db.ch(chid);
        let obj = objs.get(oid);
        game.act(
            db,
            "You stop using $p.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        game.act(
            db,
            "$n stops using $p.",
            true,
            Some(ch),
            Some(obj),
            None,
            TO_ROOM,
        );
    }
}

pub fn do_remove(
    game: &mut Game,
    db: &mut DB,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = db.ch(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        game.send_to_char(ch, "Remove what?\r\n");
        return;
    }
    let dotmode = find_all_dots(&arg);

    let mut found = false;
    let i;
    if dotmode == FIND_ALL {
        for i in 0..NUM_WEARS {
            let ch = db.ch(chid);
            if ch.get_eq(i).is_some() {
                perform_remove(game, db, objs,chid, i);
                found = true;
            }
        }
        if !found {
            let ch = db.ch(chid);
            game.send_to_char(ch, "You're not using anything.\r\n");
        }
    } else if dotmode == FIND_ALLDOT {
        if arg.is_empty() {
            game.send_to_char(ch, "Remove all of what?\r\n");
        } else {
            found = false;
            for i in 0..NUM_WEARS {
                let ch = db.ch(chid);
                if ch.get_eq(i).is_some()
                    && game.can_see_obj(db, ch, objs.get(ch.get_eq(i).unwrap()))
                    && isname(&arg, objs.get(ch.get_eq(i).unwrap()).name.as_ref())
                {
                    perform_remove(game, db, objs,chid, i);
                    found = true;
                }
            }
            if !found {
                let ch = db.ch(chid);
                game.send_to_char(
                    ch,
                    format!("You don't seem to be using any {}s.\r\n", arg).as_str(),
                );
            }
        }
    } else {
        if {
            i = game.get_obj_pos_in_equip_vis(db, objs,ch, &arg, None, &ch.equipment);
            i.is_none()
        } {
            game.send_to_char(
                ch,
                format!("You don't seem to be using {} {}.\r\n", an!(arg), arg).as_str(),
            );
        } else {
            perform_remove(game, db,objs, chid, i.unwrap());
        }
    }
}
