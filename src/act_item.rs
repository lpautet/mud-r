/* ************************************************************************
*   File: act.item.rs                                   Part of CircleMUD *
*  Usage: object handling routines -- get/drop and container handling     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 - 2025 Laurent Pautet                     *
************************************************************************ */

use std::cmp::min;
use std::rc::Rc;

use crate::depot::{Depot, DepotId, HasId};
use crate::limits::gain_condition;
use crate::{act, send_to_char, CharData, DescriptorData, ObjData, TextData, VictimRef};
use log::error;

use crate::config::{DONATION_ROOM_1, NOPERSON, OK};
use crate::constants::{DRINKNAMES, DRINKS, DRINK_AFF, STR_APP};
use crate::db::DB;
use crate::handler::{
    affect_join, equip_char, find_all_dots, generic_find, get_char_vis, get_obj_in_list_vis,
    get_obj_in_list_vis2, get_obj_pos_in_equip_vis, isname, money_desc, obj_from_char,
    obj_from_obj, obj_to_char, obj_to_obj, FindAllType, FindFlags,
};
use crate::interpreter::{
    is_number, one_argument, search_block, two_arguments, SCMD_DONATE, SCMD_DRINK, SCMD_DROP,
    SCMD_EAT, SCMD_FILL, SCMD_JUNK, SCMD_POUR, SCMD_SIP, SCMD_TASTE,
};
use crate::spells::SPELL_POISON;
use crate::structs::{
    AffectFlags, AffectedType, ApplyType, ExtraFlags, ItemType, RoomRnum, WearFlags, CONT_CLOSED,
    DRUNK, FULL, LVL_GOD, LVL_IMMORT, NOWHERE, NUM_WEARS, PULSE_VIOLENCE, THIRST, WEAR_ABOUT,
    WEAR_ARMS, WEAR_BODY, WEAR_FEET, WEAR_FINGER_R, WEAR_HANDS, WEAR_HEAD, WEAR_HOLD, WEAR_LEGS,
    WEAR_LIGHT, WEAR_NECK_1, WEAR_SHIELD, WEAR_WAIST, WEAR_WIELD, WEAR_WRIST_R,
};
use crate::util::{can_see_obj, rand_number};
use crate::{an, Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

fn perform_put(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
    cid: DepotId,
) {
    let ch = chars.get(chid);
    let obj = objs.get(oid);
    let cobj = objs.get(cid);

    if cobj.get_obj_weight() + obj.get_obj_weight() > cobj.get_obj_val(0) {
        act(
            descs,
            chars,
            db,
            "$p won't fit in $P.",
            false,
            Some(ch),
            Some(obj),
            Some(VictimRef::Obj(cobj)),
            TO_CHAR,
        );
    } else if obj.obj_flagged(ExtraFlags::NODROP) && cobj.in_room() != NOWHERE {
        act(
            descs,
            chars,
            db,
            "You can't get $p out of your hand.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
    } else {
        let obj = objs.get_mut(oid);
        obj_from_char(chars, obj);
        obj_to_obj(chars, objs, oid, cid);
        let ch = chars.get(chid);
        let obj = objs.get(oid);
        let cobj = objs.get(cid);
        act(
            descs,
            chars,
            db,
            "$n puts $p in $P.",
            true,
            Some(ch),
            Some(obj),
            Some(VictimRef::Obj(cobj)),
            TO_ROOM,
        );

        /* Yes, I realize this is strange until we have auto-equip on rent. -gg */
        if obj.obj_flagged(ExtraFlags::NODROP) && !cobj.obj_flagged(ExtraFlags::NODROP) {
            objs.get_mut(cid).set_obj_extra_bit(ExtraFlags::NODROP);
            let ch = chars.get(chid);
            let obj = objs.get(oid);
            let cobj = objs.get(cid);
            act(
                descs,
                chars,
                db,
                "You get a strange feeling as you put $p in $P.",
                false,
                Some(ch),
                Some(obj),
                Some(VictimRef::Obj(cobj)),
                TO_CHAR,
            );
        } else {
            act(
                descs,
                chars,
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
#[allow(clippy::too_many_arguments)]
pub fn do_put(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut found = false;
    let mut howmany = 1;
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut arg3 = String::new();
    let theobj;
    let thecont;
    /* three_arguments */
    one_argument(two_arguments(argument, &mut arg1, &mut arg2), &mut arg3);

    if !arg3.is_empty() {
        if let Ok(num) = arg1.parse::<i32>() {
            howmany = num;
            theobj = arg2;
            thecont = arg3;
        } else {
            theobj = arg1;
            thecont = arg2;
        }
    } else {
        theobj = arg1;
        thecont = arg2;
    }
    let obj_dotmode = find_all_dots(&theobj);
    let cont_dotmode = find_all_dots(&thecont);
    let mut tmp_char = None;
    let mut c = None;
    let obj;

    if theobj.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Put what in what?\r\n");
    } else if cont_dotmode != FindAllType::Individual {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can only put things into one container at a time.\r\n",
        );
    } else if thecont.is_empty() {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!(
                "What do you want to put {} in?\r\n",
                if obj_dotmode == FindAllType::Individual {
                    "it"
                } else {
                    "them"
                }
            )
            .as_str(),
        );
    } else {
        generic_find(
            &game.descriptors,
            chars,
            db,
            objs,
            &thecont,
            FindFlags::OBJ_INV | FindFlags::OBJ_ROOM,
            ch,
            &mut tmp_char,
            &mut c,
        );
        match c {
            None => {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    format!("You don't see {} {} here.\r\n", an!(thecont), thecont).as_str(),
                );
            }
            Some(c) if c.get_obj_type() != ItemType::Container => {
                act(
                    &mut game.descriptors,
                    chars,
                    db,
                    "$p is not a container.",
                    false,
                    Some(ch),
                    Some(c),
                    None,
                    TO_CHAR,
                );
            }
            Some(c) if c.objval_flagged(CONT_CLOSED) => {
                send_to_char(&mut game.descriptors, ch, "You'd better open it first!\r\n");
            }
            Some(c) if obj_dotmode == FindAllType::Individual => {
                /* put <obj> <container> */
                obj = get_obj_in_list_vis(
                    &game.descriptors,
                    chars,
                    db,
                    objs,
                    ch,
                    &theobj,
                    None,
                    ch.carrying.as_ref(),
                );
                match obj {
                    None => {
                        send_to_char(
                            &mut game.descriptors,
                            ch,
                            format!("You aren't carrying {} {}.\r\n", an!(theobj), theobj).as_str(),
                        );
                    }
                    Some(obj) if obj.id == c.id() && howmany == 1 => {
                        send_to_char(
                            &mut game.descriptors,
                            ch,
                            "You attempt to fold it into itself, but fail.\r\n",
                        );
                    }
                    Some(obj) => {
                        let mut cur_obj_id = obj.id();
                        let c_id = c.id();
                        while howmany != 0 {
                            if cur_obj_id != c_id {
                                howmany -= 1;
                                perform_put(
                                    &mut game.descriptors,
                                    db,
                                    chars,
                                    objs,
                                    chid,
                                    cur_obj_id,
                                    c_id,
                                );
                            }
                            let ch = chars.get(chid);
                            if let Some(obj_found) = get_obj_in_list_vis(
                                &game.descriptors,
                                chars,
                                db,
                                objs,
                                ch,
                                &theobj,
                                None,
                                ch.carrying.as_ref(),
                            ) {
                                cur_obj_id = obj_found.id();
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
            Some(c) => {
                let c_id = c.id();
                for oid in ch.carrying.clone() {
                    if oid != c_id
                        && (obj_dotmode == FindAllType::All
                            || isname(&theobj, objs.get(oid).name.as_ref()))
                    {
                        found = true;
                        perform_put(&mut game.descriptors, db, chars, objs, chid, oid, c_id);
                    }
                }
                let ch = chars.get(chid);
                if !found {
                    if obj_dotmode == FindAllType::All {
                        send_to_char(
                            &mut game.descriptors,
                            ch,
                            "You don't seem to have anything to put in it.\r\n",
                        );
                    } else {
                        send_to_char(
                            &mut game.descriptors,
                            ch,
                            format!("You don't seem to have any {}s.\r\n", theobj).as_str(),
                        );
                    }
                }
            }
        }
    }
}

fn can_take_obj(
    descs: &mut Depot<DescriptorData>,
    chars: &Depot<CharData>,
    db: &DB,
    objs: &Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
) -> bool {
    let ch = chars.get(chid);
    let obj = objs.get(oid);
    if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
        act(
            descs,
            chars,
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
        act(
            descs,
            chars,
            db,
            "$p: you can't carry that much weight.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return false;
    } else if !objs.get(oid).can_wear(WearFlags::TAKE) {
        act(
            descs,
            chars,
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

fn get_check_money(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
) {
    let value = objs.get(oid).get_obj_val(0);

    if objs.get(oid).get_obj_type() != ItemType::Money || value <= 0 {
        return;
    }

    db.extract_obj(chars, objs, oid);
    let ch = chars.get_mut(chid);
    ch.set_gold(ch.get_gold() + value);

    if value == 1 {
        send_to_char(descs, ch, "There was 1 coin.\r\n");
    } else {
        send_to_char(
            descs,
            ch,
            format!("There were {} coins.\r\n", value).as_str(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn perform_get_from_container(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
    cid: DepotId,
    mode: FindFlags,
) {
    if mode == FindFlags::OBJ_INV || can_take_obj(descs, chars, db, objs, chid, oid) {
        let ch = chars.get(chid);
        let obj = objs.get(oid);
        if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
            act(
                descs,
                chars,
                db,
                "$p: you can't hold any more items.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
        } else {
            obj_from_obj(chars, objs, oid);
            let ch = chars.get_mut(chid);
            let obj = objs.get_mut(oid);
            obj_to_char(obj, ch);
            let ch = chars.get(chid);
            let obj = objs.get(oid);
            let cobj = objs.get(cid);
            act(
                descs,
                chars,
                db,
                "You get $p from $P.",
                false,
                Some(ch),
                Some(obj),
                Some(VictimRef::Obj(cobj)),
                TO_CHAR,
            );
            act(
                descs,
                chars,
                db,
                "$n gets $p from $P.",
                true,
                Some(ch),
                Some(obj),
                Some(VictimRef::Obj(cobj)),
                TO_ROOM,
            );
            get_check_money(descs, db, chars, objs, chid, oid);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn get_from_container(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    cid: DepotId,
    arg: &str,
    mode: FindFlags,
    howmany: i32,
) {
    let ch = chars.get(chid);
    let cobj = objs.get(cid);
    let mut found = false;

    let mut howmany = howmany;
    let obj_dotmode = find_all_dots(arg);

    if cobj.objval_flagged(CONT_CLOSED) {
        act(
            descs,
            chars,
            db,
            "$p is closed.",
            false,
            Some(ch),
            Some(cobj),
            None,
            TO_CHAR,
        );
    } else if obj_dotmode == FindAllType::Individual {
        let mut obj = get_obj_in_list_vis(descs, chars, db, objs, ch, arg, None, &cobj.contains);
        if obj.is_none() {
            let buf = format!("There doesn't seem to be {} {} in $p.", an!(arg), arg);
            act(
                descs,
                chars,
                db,
                &buf,
                false,
                Some(ch),
                Some(cobj),
                None,
                TO_CHAR,
            );
        } else {
            while howmany != 0 {
                if let Some(the_obj) = obj {
                    let oid = the_obj.id();
                    howmany -= 1;
                    perform_get_from_container(descs, db, chars, objs, chid, oid, cid, mode);
                    let ch = chars.get(chid);
                    let cobj = objs.get(cid);
                    obj =
                        get_obj_in_list_vis(descs, chars, db, objs, ch, arg, None, &cobj.contains);
                } else {
                    break;
                }
            }
        }
    } else {
        if obj_dotmode == FindAllType::AllDot && arg.is_empty() {
            send_to_char(descs, ch, "Get all of what?\r\n");
            return;
        }
        for oid in cobj.contains.clone() {
            let ch = chars.get(chid);
            if can_see_obj(descs, chars, db, ch, objs.get(oid))
                && (obj_dotmode == FindAllType::All || isname(arg, &objs.get(oid).name))
            {
                found = true;
                perform_get_from_container(descs, db, chars, objs, chid, oid, cid, mode);
            }
        }
        let ch = chars.get(chid);
        let cobj = objs.get(cid);
        if !found {
            if obj_dotmode == FindAllType::All {
                act(
                    descs,
                    chars,
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
                act(
                    descs,
                    chars,
                    db,
                    &buf,
                    false,
                    Some(ch),
                    Some(cobj),
                    None,
                    TO_CHAR,
                );
            }
        }
    }
}

fn perform_get_from_room(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
) -> bool {
    if can_take_obj(descs, chars, db, objs, chid, oid) {
        let obj = objs.get_mut(oid);
        db.obj_from_room(obj);
        let ch = chars.get_mut(chid);
        obj_to_char(obj, ch);
        let ch = chars.get(chid);
        act(
            descs,
            chars,
            db,
            "You get $p.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        act(
            descs,
            chars,
            db,
            "$n gets $p.",
            true,
            Some(ch),
            Some(obj),
            None,
            TO_ROOM,
        );
        get_check_money(descs, db, chars, objs, chid, oid);
        return true;
    }
    false
}

fn get_from_room(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    arg: &str,
    howmany: i32,
) {
    let ch = chars.get(chid);
    let mut found = false;
    let mut howmany = howmany;
    let dotmode = find_all_dots(arg);

    if dotmode == FindAllType::Individual {
        let mut obj = get_obj_in_list_vis2(
            descs,
            chars,
            db,
            objs,
            ch,
            arg,
            None,
            &db.world[ch.in_room() as usize].contents,
        );
        if obj.is_none() {
            send_to_char(
                descs,
                ch,
                format!("You don't see {} {} here.\r\n", an!(arg), arg).as_str(),
            );
        } else {
            loop {
                if howmany == 0 {
                    break;
                }
                if let Some(the_obj) = obj {
                    howmany -= 1;
                    let oid = the_obj.id();
                    perform_get_from_room(descs, db, chars, objs, chid, oid);
                    let ch = chars.get(chid);
                    obj = get_obj_in_list_vis2(
                        descs,
                        chars,
                        db,
                        objs,
                        ch,
                        arg,
                        None,
                        &db.world[ch.in_room() as usize].contents,
                    );
                } else {
                    break;
                }
            }
        }
    } else {
        if dotmode == FindAllType::AllDot && arg.is_empty() {
            send_to_char(descs, ch, "Get all of what?\r\n");
            return;
        }
        for oid in db.world[ch.in_room() as usize].contents.clone() {
            let ch = chars.get(chid);
            if can_see_obj(descs, chars, db, ch, objs.get(oid))
                && (dotmode == FindAllType::All || isname(arg, &objs.get(oid).name))
            {
                found = true;
                perform_get_from_room(descs, db, chars, objs, chid, oid);
            }
        }
        let ch = chars.get(chid);
        if !found {
            if dotmode == FindAllType::All {
                send_to_char(descs, ch, "There doesn't seem to be anything here.\r\n");
            } else {
                send_to_char(
                    descs,
                    ch,
                    format!("You don't see any {}s here.\r\n", arg).as_str(),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_get(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut arg3 = String::new();
    let mut tmp_char = None;
    let mut c = None;

    let mut found = false;
    one_argument(two_arguments(argument, &mut arg1, &mut arg2), &mut arg3); /* three_arguments */

    if arg1.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Get what?\r\n");
    } else if arg2.is_empty() {
        get_from_room(&mut game.descriptors, db, chars, objs, chid, &arg1, 1);
    } else if is_number(&arg1) && arg3.is_empty() {
        get_from_room(
            &mut game.descriptors,
            db,
            chars,
            objs,
            chid,
            &arg2,
            arg1.parse::<i32>()
                .unwrap_or_else(|e| panic!("Invalid number: {}", e)),
        );
    } else {
        let mut amount = 1;
        if let Ok(num) = arg1.parse::<i32>() {
            amount = num;
            arg1 = arg2; /* strcpy: OK (sizeof: arg1 == arg2) */
            arg2 = arg3; /* strcpy: OK (sizeof: arg2 == arg3) */
        }
        let cont_dotmode = find_all_dots(&arg2);
        if cont_dotmode == FindAllType::Individual {
            let mode = generic_find(
                &game.descriptors,
                chars,
                db,
                objs,
                &arg2,
                FindFlags::OBJ_INV | FindFlags::OBJ_ROOM,
                ch,
                &mut tmp_char,
                &mut c,
            );
            match c {
                None => {
                    send_to_char(
                        &mut game.descriptors,
                        ch,
                        format!("You don't have {} {}.\r\n", an!(&arg2), &arg2).as_str(),
                    );
                }
                Some(c) if c.get_obj_type() != ItemType::Container => {
                    act(
                        &mut game.descriptors,
                        chars,
                        db,
                        "$p is not a container.",
                        false,
                        Some(ch),
                        Some(c),
                        None,
                        TO_CHAR,
                    );
                }
                Some(c) => {
                    get_from_container(
                        &mut game.descriptors,
                        db,
                        chars,
                        objs,
                        chid,
                        c.id(),
                        &arg1,
                        mode,
                        amount,
                    );
                }
            }
        } else {
            if cont_dotmode == FindAllType::AllDot && arg2.is_empty() {
                send_to_char(&mut game.descriptors, ch, "Get from all of what?\r\n");
                return;
            }
            let list = ch.carrying.clone();
            for contid in list {
                let ch = chars.get(chid);
                let contobj = objs.get(contid);
                if can_see_obj(&game.descriptors, chars, db, ch, contobj)
                    && (cont_dotmode == FindAllType::All || isname(&arg2, &contobj.name))
                {
                    if contobj.get_obj_type() == ItemType::Container {
                        found = true;
                        get_from_container(
                            &mut game.descriptors,
                            db,
                            chars,
                            objs,
                            chid,
                            contid,
                            &arg1,
                            FindFlags::OBJ_INV,
                            amount,
                        );
                    } else if cont_dotmode == FindAllType::AllDot {
                        found = true;
                        act(
                            &mut game.descriptors,
                            chars,
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
            let ch = chars.get(chid);
            for contid in db.world[ch.in_room() as usize].contents.clone() {
                let ch = chars.get(chid);
                let cont_obj = objs.get(contid);
                if can_see_obj(&game.descriptors, chars, db, ch, cont_obj)
                    && (cont_dotmode == FindAllType::All || isname(&arg2, &cont_obj.name))
                {
                    if cont_obj.get_obj_type() == ItemType::Container {
                        get_from_container(
                            &mut game.descriptors,
                            db,
                            chars,
                            objs,
                            chid,
                            contid,
                            &arg1,
                            FindFlags::OBJ_ROOM,
                            amount,
                        );
                        found = true;
                    } else if cont_dotmode == FindAllType::AllDot {
                        act(
                            &mut game.descriptors,
                            chars,
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
                if cont_dotmode == FindAllType::All {
                    let ch = chars.get(chid);
                    send_to_char(
                        &mut game.descriptors,
                        ch,
                        "You can't seem to find any containers.\r\n",
                    );
                } else {
                    let ch = chars.get(chid);
                    send_to_char(
                        &mut game.descriptors,
                        ch,
                        format!("You can't seem to find any {}s here.\r\n", &arg2).as_str(),
                    );
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn perform_drop_gold(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    amount: i32,
    mode: u8,
    rdr: RoomRnum,
) {
    let ch = chars.get(chid);
    if amount <= 0 {
        send_to_char(descs, ch, "Heh heh heh.. we are jolly funny today, eh?\r\n");
    } else if ch.get_gold() < amount {
        send_to_char(descs, ch, "You don't have that many coins!\r\n");
    } else {
        if mode != SCMD_JUNK {
            let ch = chars.get_mut(chid);
            ch.set_wait_state(PULSE_VIOLENCE as i32); /* to prevent coin-bombing */

            let oid = db
                .create_money(objs, amount)
                .unwrap_or_else(|| panic!("Failed to create money"));
            if mode == SCMD_DONATE {
                let ch = chars.get(chid);
                send_to_char(
                    descs,
                    ch,
                    "You throw some gold into the air where it disappears in a puff of smoke!\r\n",
                );
                act(
                    descs,
                    chars,
                    db,
                    "$n throws some gold into the air where it disappears in a puff of smoke!",
                    false,
                    Some(ch),
                    None,
                    None,
                    TO_ROOM,
                );
                let obj = objs.get_mut(oid);
                db.obj_to_room(obj, rdr);
                let obj = objs.get(oid);
                act(
                    descs,
                    chars,
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
                let ch = chars.get(chid);
                act(descs, chars, db, &buf, true, Some(ch), None, None, TO_ROOM);
                send_to_char(descs, ch, "You drop some gold.\r\n");
                let obj = objs.get_mut(oid);
                db.obj_to_room(obj, ch.in_room());
            }
        } else {
            let buf = format!(
                "$n drops {} which disappears in a puff of smoke!",
                money_desc(amount)
            );
            act(descs, chars, db, &buf, false, Some(ch), None, None, TO_ROOM);

            send_to_char(
                descs,
                ch,
                "You drop some gold which disappears in a puff of smoke!\r\n",
            );
        }
        let ch = chars.get_mut(chid);
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

#[allow(clippy::too_many_arguments)]
fn perform_drop(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
    mut mode: u8,
    sname: &str,
    rdr: RoomRnum,
) -> i32 {
    let ch = chars.get(chid);
    let obj = objs.get_mut(oid);
    if obj.obj_flagged(ExtraFlags::NODROP) {
        let buf = format!("You can't {} $p, it must be CURSED!", sname);
        act(
            descs,
            chars,
            db,
            &buf,
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return 0;
    }

    let buf = format!("You {} $p.{}", sname, vanish!(mode));
    act(
        descs,
        chars,
        db,
        &buf,
        false,
        Some(ch),
        Some(obj),
        None,
        TO_CHAR,
    );

    let buf = format!("$n {}s $p.{}", sname, vanish!(mode));
    act(
        descs,
        chars,
        db,
        &buf,
        true,
        Some(ch),
        Some(obj),
        None,
        TO_ROOM,
    );

    obj_from_char(chars, obj);
    if (mode == SCMD_DONATE) && obj.obj_flagged(ExtraFlags::NODONATE) {
        mode = SCMD_JUNK;
    }

    let ch = chars.get(chid);
    match mode {
        SCMD_DROP => {
            db.obj_to_room(obj, ch.in_room());
        }

        SCMD_DONATE => {
            db.obj_to_room(obj, rdr);
            act(
                descs,
                chars,
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
            let value = (obj.get_obj_cost() / 16).clamp(1, 200);
            db.extract_obj(chars, objs, oid);
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

#[allow(clippy::too_many_arguments)]
pub fn do_drop(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
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
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "Sorry, you can't donate anything right now.\r\n",
                );
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

    #[allow(clippy::blocks_in_conditions)]
    if arg.is_empty() {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("What do you want to {}?\r\n", sname).as_str(),
        );
        return;
    } else if let Ok(mut multi) = arg.parse::<i32>() {
        one_argument(argument, &mut arg);
        if arg == "coins" || arg == "coin" {
            perform_drop_gold(
                &mut game.descriptors,
                db,
                chars,
                objs,
                chid,
                multi,
                mode,
                rdr,
            );
        } else if multi <= 0 {
            send_to_char(&mut game.descriptors, ch, "Yeah, that makes sense.\r\n");
        } else if arg.is_empty() {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!("What do you want to {} {} of?\r\n", sname, multi).as_str(),
            );
        } else {
            let mut obj = get_obj_in_list_vis(
                &game.descriptors,
                chars,
                db,
                objs,
                ch,
                &arg,
                None,
                &ch.carrying,
            );
            if obj.is_none() {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    format!("You don't seem to have any {}s.\r\n", arg).as_str(),
                );
            }
            while let Some(the_obj) = obj {
                amount += perform_drop(
                    &mut game.descriptors,
                    db,
                    chars,
                    objs,
                    chid,
                    the_obj.id(),
                    mode,
                    sname,
                    rdr,
                );
                let ch = chars.get(chid);
                obj = get_obj_in_list_vis(
                    &game.descriptors,
                    chars,
                    db,
                    objs,
                    ch,
                    &arg,
                    None,
                    &ch.carrying,
                );
                multi -= 1;
                if multi == 0 {
                    break;
                }
            }
        }
    } else {
        dotmode = find_all_dots(&arg);

        /* Can't junk or donate all */
        if (dotmode == FindAllType::All)
            && (subcmd == SCMD_JUNK as i32 || subcmd == SCMD_DONATE as i32)
        {
            if subcmd == SCMD_JUNK as i32 {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "Go to the dump if you want to junk EVERYTHING!\r\n",
                );
            } else {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "Go do the donation room if you want to donate EVERYTHING!\r\n",
                );
                return;
            }
        }
        if dotmode == FindAllType::All {
            let ch = chars.get(chid);
            if ch.carrying.is_empty() {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    "You don't seem to be carrying anything.\r\n",
                );
            } else {
                let list = ch.carrying.clone();
                for oid in list {
                    amount += perform_drop(
                        &mut game.descriptors,
                        db,
                        chars,
                        objs,
                        chid,
                        oid,
                        mode,
                        sname,
                        rdr,
                    );
                }
            }
        } else if dotmode == FindAllType::AllDot {
            if arg.is_empty() {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    format!("What do you want to {} all of?\r\n", sname).as_str(),
                );
                return;
            }
            if {
                let ch = chars.get(chid);
                obj = get_obj_in_list_vis(
                    &game.descriptors,
                    chars,
                    db,
                    objs,
                    ch,
                    &arg,
                    None,
                    &ch.carrying,
                );
                obj.is_none()
            } {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    format!("You don't seem to have any {}s.\r\n", arg).as_str(),
                );
            }

            while let Some(the_obj) = obj {
                amount += perform_drop(
                    &mut game.descriptors,
                    db,
                    chars,
                    objs,
                    chid,
                    the_obj.id(),
                    mode,
                    sname,
                    rdr,
                );
                let ch = chars.get(chid);
                obj = get_obj_in_list_vis(
                    &game.descriptors,
                    chars,
                    db,
                    objs,
                    ch,
                    &arg,
                    None,
                    &ch.carrying,
                );
            }
        } else {
            let ch = chars.get(chid);
            if let Some(obj) = get_obj_in_list_vis(
                &game.descriptors,
                chars,
                db,
                objs,
                ch,
                &arg,
                None,
                &ch.carrying,
            ) {
                amount += perform_drop(
                    &mut game.descriptors,
                    db,
                    chars,
                    objs,
                    chid,
                    obj.id(),
                    mode,
                    sname,
                    rdr,
                );
            } else {
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
                );
            }
        }
    }

    if amount != 0 && subcmd == SCMD_JUNK as i32 {
        let ch = chars.get(chid);
        send_to_char(
            &mut game.descriptors,
            ch,
            "You have been rewarded by the gods!\r\n",
        );
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n has been rewarded by the gods!",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let ch = chars.get_mut(chid);
        ch.set_gold(ch.get_gold() + amount);
    }
}

fn perform_give(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    vict_id: DepotId,
    oid: DepotId,
) {
    let ch = chars.get(chid);
    let vict = chars.get(vict_id);
    let obj = objs.get_mut(oid);
    if obj.obj_flagged(ExtraFlags::NODROP) {
        act(
            descs,
            chars,
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
        act(
            descs,
            chars,
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
        act(
            descs,
            chars,
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
    obj_from_char(chars, obj);
    let vict = chars.get_mut(vict_id);
    obj_to_char(obj, vict);
    let ch = chars.get(chid);
    let obj = objs.get(oid);
    let vict = chars.get(vict_id);
    act(
        descs,
        chars,
        db,
        "You give $p to $N.",
        false,
        Some(ch),
        Some(obj),
        Some(VictimRef::Char(vict)),
        TO_CHAR,
    );
    act(
        descs,
        chars,
        db,
        "$n gives you $p.",
        false,
        Some(ch),
        Some(obj),
        Some(VictimRef::Char(vict)),
        TO_VICT,
    );
    act(
        descs,
        chars,
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
fn give_find_vict<'a>(
    descs: &mut Depot<DescriptorData>,
    db: &'a DB,
    chars: &'a Depot<CharData>,
    ch: &'a CharData,
    arg: &str,
) -> Option<&'a CharData> {
    let mut arg = arg.trim_start().to_string();

    if arg.is_empty() {
        send_to_char(descs, ch, "To who?\r\n");
    } else {
        match get_char_vis(descs, chars, db, ch, &mut arg, None, FindFlags::CHAR_ROOM) {
            None => {
                send_to_char(descs, ch, NOPERSON);
            }
            Some(vict) if vict.id() == ch.id() => {
                send_to_char(descs, ch, "What's the point of that?\r\n");
            }
            Some(vict) => {
                return Some(vict);
            }
        }
    }

    None
}

fn perform_give_gold(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    chid: DepotId,
    vict_id: DepotId,
    amount: i32,
) {
    let ch = chars.get(chid);
    let vict = chars.get(vict_id);
    let mut buf;

    if amount <= 0 {
        send_to_char(
            descs,
            ch,
            "Heh heh heh ... we are jolly funny today, eh?\r\n",
        );
        return;
    }
    if ch.get_gold() < amount && (ch.is_npc() || (ch.get_level() < LVL_GOD)) {
        send_to_char(descs, ch, "You don't have that many coins!\r\n");
        return;
    }
    send_to_char(descs, ch, OK);

    buf = format!(
        "$n gives you {} gold coin{}.",
        amount,
        if amount == 1 { "" } else { "s" }
    );
    act(
        descs,
        chars,
        db,
        &buf,
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_VICT,
    );

    buf = format!("$n gives {} to $N.", money_desc(amount));
    act(
        descs,
        chars,
        db,
        &buf,
        true,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_NOTVICT,
    );
    let ch = chars.get(chid);

    if ch.is_npc() || ch.get_level() < LVL_GOD {
        let ch = chars.get_mut(chid);
        ch.set_gold(ch.get_gold() - amount);
    }
    let vict = chars.get_mut(vict_id);
    vict.set_gold(vict.get_gold() + amount);
}

#[allow(clippy::too_many_arguments)]
pub fn do_give(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    let mut argument = one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Give what to who?\r\n");
    } else if let Ok(mut amount) = arg.parse::<i32>() {
        argument = one_argument(argument, &mut arg);
        if arg == "coins" || arg == "coin" {
            one_argument(argument, &mut arg);
            if let Some(vict) = give_find_vict(&mut game.descriptors, db, chars, ch, &arg) {
                perform_give_gold(&mut game.descriptors, db, chars, chid, vict.id(), amount);
            } else if arg.is_empty() {
                /* Give multiple code. */
                let ch = chars.get(chid);
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    format!("What do you want to give {} of?\r\n", amount).as_str(),
                );
            } else if let Some(vict) =
                give_find_vict(&mut game.descriptors, db, chars, ch, argument)
            {
                let ch = chars.get(chid);
                if let Some(mut obj) = get_obj_in_list_vis(
                    &game.descriptors,
                    chars,
                    db,
                    objs,
                    ch,
                    &arg,
                    None,
                    &ch.carrying,
                ) {
                    let vict_id = vict.id();
                    while amount != 0 {
                        amount -= 1;
                        perform_give(
                            &mut game.descriptors,
                            db,
                            chars,
                            objs,
                            chid,
                            vict_id,
                            obj.id(),
                        );
                        let ch = chars.get(chid);
                        if let Some(found_obj) = get_obj_in_list_vis(
                            &game.descriptors,
                            chars,
                            db,
                            objs,
                            ch,
                            &arg,
                            None,
                            &ch.carrying,
                        ) {
                            obj = found_obj;
                        } else {
                            break;
                        }
                    }
                } else {
                    //let ch = chars.get(chid);
                    send_to_char(
                        &mut game.descriptors,
                        ch,
                        format!("You don't seem to have any {}s.\r\n", arg).as_str(),
                    );
                }
            }
        }
    } else {
        let mut buf1 = String::new();
        one_argument(argument, &mut buf1);
        if let Some(vict) = give_find_vict(&mut game.descriptors, db, chars, ch, &buf1) {
            let dotmode = find_all_dots(&arg);
            if dotmode == FindAllType::Individual {
                let ch = chars.get(chid);

                if let Some(obj) = get_obj_in_list_vis(
                    &game.descriptors,
                    chars,
                    db,
                    objs,
                    ch,
                    &arg,
                    None,
                    &ch.carrying,
                ) {
                    perform_give(
                        &mut game.descriptors,
                        db,
                        chars,
                        objs,
                        chid,
                        vict.id(),
                        obj.id(),
                    );
                } else {
                    //       let ch = chars.get(chid);
                    send_to_char(
                        &mut game.descriptors,
                        ch,
                        format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
                    );
                }
            } else {
                if dotmode == FindAllType::AllDot && arg.is_empty() {
                    let ch = chars.get(chid);
                    send_to_char(&mut game.descriptors, ch, "All of what?\r\n");
                    return;
                }
                let ch = chars.get(chid);
                if ch.carrying.is_empty() {
                    send_to_char(
                        &mut game.descriptors,
                        ch,
                        "You don't seem to be holding anything.\r\n",
                    );
                } else {
                    let list = ch.carrying.clone();
                    let vict_id = vict.id();
                    for oid in list {
                        let ch = chars.get(chid);
                        let obj = objs.get(oid);
                        if can_see_obj(&game.descriptors, chars, db, ch, obj)
                            && (dotmode == FindAllType::All || isname(&arg, &obj.name))
                        {
                            perform_give(
                                &mut game.descriptors,
                                db,
                                chars,
                                objs,
                                chid,
                                vict_id,
                                oid,
                            );
                        }
                    }
                }
            }
        }
    }
}

pub fn weight_change_object(
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    oid: DepotId,
    weight: i32,
) {
    let obj = objs.get_mut(oid);
    if obj.in_room() != NOWHERE {
        obj.incr_obj_weight(weight);
    } else if let Some(tmp_ch_id) = obj.carried_by {
        obj_from_char(chars, obj);
        obj.incr_obj_weight(weight);
        obj_to_char(obj, chars.get_mut(tmp_ch_id));
    } else if let Some(tmp_obj_id) = objs.get(oid).in_obj {
        obj_from_obj(chars, objs, oid);
        objs.get_mut(oid).incr_obj_weight(weight);
        obj_to_obj(chars, objs, oid, tmp_obj_id);
    } else {
        error!("SYSERR: Unknown attempt to subtract weight from an object.");
    }
}

pub fn name_from_drinkcon(objs: &mut Depot<ObjData>, oid: Option<DepotId>) {
    if let Some(oid) = oid {
        if objs.get(oid).get_obj_type() != ItemType::Drinkcon
            && objs.get(oid).get_obj_type() != ItemType::Fountain
        {
            return;
        }

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
        while !cur_name.is_empty() {
            if cur_name.starts_with(' ') {
                cur_name = &cur_name[1..];
            }
            let i = cur_name.find(' ');
            let cpylen = i.unwrap_or(cur_name.len());

            if cur_name.starts_with(liqname) {
                cur_name = next;
                continue;
            }

            if !new_name.is_empty() {
                new_name.push(' ');
            } else {
                new_name.push_str(&cur_name[0..cpylen])
            }
            cur_name = next;
        }

        objs.get_mut(oid).name = Rc::from(new_name.as_str());
    }
}

pub fn name_to_drinkcon(objs: &mut Depot<ObjData>, oid: Option<DepotId>, type_: i32) {
    let mut new_name = String::new();
    if let Some(oid) = oid {
        if objs.get(oid).get_obj_type() != ItemType::Drinkcon
            && objs.get(oid).get_obj_type() != ItemType::Fountain
        {
            return;
        }
        new_name.push_str(
            format!(
                "{} {}",
                objs.get(oid).name.as_ref(),
                DRINKNAMES[type_ as usize]
            )
            .as_str(),
        );

        objs.get_mut(oid).name = Rc::from(new_name.as_str());
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_drink(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    one_argument(argument, &mut arg);

    if ch.is_npc() {
        /* Cannot use ) on mobs. */
        return;
    }

    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Drink from what?\r\n");
        return;
    }
    let mut on_ground = false;
    let to_obj: &ObjData;
    if let Some(found_obj) = get_obj_in_list_vis(
        &game.descriptors,
        chars,
        db,
        objs,
        ch,
        &arg,
        None,
        &ch.carrying,
    ) {
        to_obj = found_obj;
    } else if let Some(found_obj) = get_obj_in_list_vis2(
        &game.descriptors,
        chars,
        db,
        objs,
        ch,
        &arg,
        None,
        &db.world[ch.in_room() as usize].contents,
    ) {
        to_obj = found_obj;
        on_ground = true;
    } else {
        send_to_char(&mut game.descriptors, ch, "You can't find it!\r\n");
        return;
    }
    if to_obj.get_obj_type() != ItemType::Drinkcon && to_obj.get_obj_type() != ItemType::Fountain {
        send_to_char(&mut game.descriptors, ch, "You can't drink from that!\r\n");
        return;
    }
    if on_ground && to_obj.get_obj_type() == ItemType::Drinkcon {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You have to be holding that to drink from it.\r\n",
        );
        return;
    }
    if ch.get_cond(DRUNK) > 10 && ch.get_cond(THIRST) > 0 {
        /* The pig is drunk */
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can't seem to get close enough to your mouth.\r\n",
        );
        act(
            &mut game.descriptors,
            chars,
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
        send_to_char(
            &mut game.descriptors,
            ch,
            "Your stomach can't contain anymore!\r\n",
        );
        return;
    }
    if to_obj.get_obj_val(1) == 0 {
        send_to_char(&mut game.descriptors, ch, "It's empty.\r\n");
        return;
    }
    let mut amount;
    if subcmd == SCMD_DRINK {
        let buf = format!(
            "$n DRINKS {} from $p.",
            DRINKS[to_obj.get_obj_val(2) as usize]
        );
        act(
            &mut game.descriptors,
            chars,
            db,
            &buf,
            true,
            Some(ch),
            Some(to_obj),
            None,
            TO_ROOM,
        );

        send_to_char(
            &mut game.descriptors,
            ch,
            format!(
                "You drink the {}.\r\n",
                DRINKS[to_obj.get_obj_val(2) as usize]
            )
            .as_str(),
        );
        let ch = chars.get(chid);
        if DRINK_AFF[to_obj.get_obj_val(2) as usize][DRUNK] > 0 {
            amount = (25 - ch.get_cond(THIRST)) as i32
                / DRINK_AFF[to_obj.get_obj_val(2) as usize][DRUNK];
        } else {
            amount = rand_number(3, 10) as i32;
        }
    } else {
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n sips from $p.",
            true,
            Some(ch),
            Some(to_obj),
            None,
            TO_ROOM,
        );
        send_to_char(
            &mut game.descriptors,
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
    weight_change_object(chars, objs, toid, -weight); /* Subtract amount */
    let to_obj = objs.get(toid);
    let ch = chars.get_mut(chid);
    gain_condition(
        &mut game.descriptors,
        ch,
        DRUNK,
        DRINK_AFF[to_obj.get_obj_val(2) as usize][DRUNK] * amount / 4,
    );
    let to_obj = objs.get(toid);
    gain_condition(
        &mut game.descriptors,
        ch,
        FULL,
        DRINK_AFF[to_obj.get_obj_val(2) as usize][FULL] * amount / 4,
    );
    let to_obj = objs.get(toid);
    gain_condition(
        &mut game.descriptors,
        ch,
        THIRST,
        DRINK_AFF[to_obj.get_obj_val(2) as usize][THIRST] * amount / 4,
    );
    let ch = chars.get(chid);

    if ch.get_cond(DRUNK) > 10 {
        send_to_char(&mut game.descriptors, ch, "You feel drunk.\r\n");
    }
    let ch = chars.get(chid);

    if ch.get_cond(THIRST) > 20 {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You don't feel thirsty any more.\r\n",
        );
    }
    let ch = chars.get(chid);

    if ch.get_cond(FULL) > 20 {
        send_to_char(&mut game.descriptors, ch, "You are full.\r\n");
    }
    let to_obj = objs.get(toid);
    if to_obj.get_obj_val(3) != 0 {
        /* The crap was poisoned ! */
        send_to_char(
            &mut game.descriptors,
            ch,
            "Oops, it tasted rather strange!\r\n",
        );
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n chokes and utters some strange sounds.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let af = AffectedType {
            _type: SPELL_POISON as i16,
            duration: (amount * 3) as i16,
            modifier: 0,
            location: ApplyType::None,
            bitvector: AffectFlags::POISON,
        };
        let ch = chars.get_mut(chid);
        affect_join(objs, ch, &af, false, false, false, false);
    }
    /* empty the container, and no longer poison. */
    let to_obj = objs.get(toid);
    let v = to_obj.get_obj_val(1) - amount;
    let to_obj = objs.get_mut(toid);
    to_obj.set_obj_val(1, v);

    if to_obj.get_obj_val(1) == 0 {
        /* The last bit */
        name_from_drinkcon(objs, Some(toid));
        let to_obj = objs.get_mut(toid);
        to_obj.set_obj_val(2, 0);
        to_obj.set_obj_val(3, 0);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_eat(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if ch.is_npc() {
        /* Cannot use ) on mobs. */
        return;
    }

    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Eat what?\r\n");
        return;
    }
    let food;
    if let Some(found) = get_obj_in_list_vis(
        &game.descriptors,
        chars,
        db,
        objs,
        ch,
        &arg,
        None,
        &ch.carrying,
    ) {
        food = found;
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
        return;
    }
    if subcmd == SCMD_TASTE
        && (food.get_obj_type() == ItemType::Drinkcon || food.get_obj_type() == ItemType::Fountain)
    {
        do_drink(game, db, chars, texts, objs, chid, argument, 0, SCMD_SIP);
        return;
    }
    if (food.get_obj_type() != ItemType::Food) && (ch.get_level() < LVL_GOD) {
        send_to_char(&mut game.descriptors, ch, "You can't eat THAT!\r\n");
        return;
    }
    if ch.get_cond(FULL) > 20 {
        /* Stomach full */
        send_to_char(
            &mut game.descriptors,
            ch,
            "You are too full to eat more!\r\n",
        );
        return;
    }
    if subcmd == SCMD_EAT {
        act(
            &mut game.descriptors,
            chars,
            db,
            "You eat $p.",
            false,
            Some(ch),
            Some(food),
            None,
            TO_CHAR,
        );
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n eats $p.",
            true,
            Some(ch),
            Some(food),
            None,
            TO_ROOM,
        );
    } else {
        act(
            &mut game.descriptors,
            chars,
            db,
            "You nibble a little bit of $p.",
            false,
            Some(ch),
            Some(food),
            None,
            TO_CHAR,
        );
        act(
            &mut game.descriptors,
            chars,
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
    let ch = chars.get_mut(chid);
    gain_condition(&mut game.descriptors, ch, FULL, amount);
    if ch.get_cond(FULL) > 20 {
        send_to_char(&mut game.descriptors, ch, "You are full.\r\n");
    }
    let ch = chars.get(chid);
    let food = objs.get(food_id);
    if food.get_obj_val(3) != 0 && (ch.get_level() < LVL_IMMORT) {
        /* The crap was poisoned ! */
        send_to_char(
            &mut game.descriptors,
            ch,
            "Oops, that tasted rather strange!\r\n",
        );
        act(
            &mut game.descriptors,
            chars,
            db,
            "$n coughs and utters some strange sounds.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );

        let af = AffectedType {
            _type: SPELL_POISON as i16,
            duration: (amount * 2) as i16,
            modifier: 0,
            location: ApplyType::None,
            bitvector: AffectFlags::POISON,
        };
        let ch = chars.get_mut(chid);
        affect_join(objs, ch, &af, false, false, false, false);
    }
    #[allow(clippy::blocks_in_conditions)]
    if subcmd == SCMD_EAT {
        db.extract_obj(chars, objs, food_id);
    } else if {
        objs.get_mut(food_id).decr_obj_val(1);
        let food = objs.get_mut(food_id);
        food.get_obj_val(0) == 0
    } {
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, "There's nothing left now.\r\n");
        db.extract_obj(chars, objs, food_id);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_pour(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let from_obj;
    let mut to_obj;
    let mut amount;

    two_arguments(argument, &mut arg1, &mut arg2);

    if subcmd == SCMD_POUR {
        if arg1.is_empty() {
            /* No arguments */
            send_to_char(
                &mut game.descriptors,
                ch,
                "From what do you want to pour?\r\n",
            );
            return;
        }
        if let Some(got_obj) = get_obj_in_list_vis(
            &game.descriptors,
            chars,
            db,
            objs,
            ch,
            &arg1,
            None,
            &ch.carrying,
        ) {
            from_obj = got_obj;
        } else {
            send_to_char(&mut game.descriptors, ch, "You can't find it!\r\n");
            return;
        }
        if from_obj.get_obj_type() != ItemType::Drinkcon {
            send_to_char(&mut game.descriptors, ch, "You can't pour from that!\r\n");
            return;
        }
    } else if subcmd == SCMD_FILL {
        if arg1.is_empty() {
            /* no arguments */
            send_to_char(
                &mut game.descriptors,
                ch,
                "What do you want to fill?  And what are you filling it from?\r\n",
            );
            return;
        }
        if let Some(got_obj) = get_obj_in_list_vis(
            &game.descriptors,
            chars,
            db,
            objs,
            ch,
            &arg1,
            None,
            &ch.carrying,
        ) {
            to_obj = got_obj;
        } else {
            send_to_char(&mut game.descriptors, ch, "You can't find it!\r\n");
            return;
        }
        if to_obj.get_obj_type() != ItemType::Drinkcon {
            act(
                &mut game.descriptors,
                chars,
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
            act(
                &mut game.descriptors,
                chars,
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
        if let Some(got_obj) = get_obj_in_list_vis2(
            &game.descriptors,
            chars,
            db,
            objs,
            ch,
            &arg2,
            None,
            &db.world[ch.in_room() as usize].contents,
        ) {
            from_obj = got_obj;
        } else {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!("There doesn't seem to be {} {} here.\r\n", an!(arg2), arg2).as_str(),
            );
            return;
        }
        if from_obj.get_obj_type() != ItemType::Fountain {
            act(
                &mut game.descriptors,
                chars,
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
    } else {
        error!("SYSERR: do_pour: invalid subcmd {}", subcmd);
        return;
    }

    if from_obj.get_obj_val(1) == 0 {
        act(
            &mut game.descriptors,
            chars,
            db,
            "The $p is empty.",
            false,
            Some(ch),
            Some(from_obj),
            None,
            TO_CHAR,
        );
        return;
    } else if subcmd == SCMD_POUR {
        /* pour */
        if arg2.is_empty() {
            send_to_char(
                &mut game.descriptors,
                ch,
                "Where do you want it?  Out or in what?\r\n",
            );
            return;
        }
        if arg2 == "out" {
            act(
                &mut game.descriptors,
                chars,
                db,
                "$n empties $p.",
                true,
                Some(ch),
                Some(from_obj),
                None,
                TO_ROOM,
            );
            act(
                &mut game.descriptors,
                chars,
                db,
                "You empty $p.",
                false,
                Some(ch),
                Some(from_obj),
                None,
                TO_CHAR,
            );
            let from_obj_id = from_obj.id();
            weight_change_object(chars, objs, from_obj_id, -from_obj.get_obj_val(1)); /* Empty */

            name_from_drinkcon(objs, Some(from_obj_id));
            let from_obj = objs.get_mut(from_obj_id);
            from_obj.set_obj_val(1, 0);
            from_obj.set_obj_val(2, 0);
            from_obj.set_obj_val(3, 0);

            return;
        }
        if let Some(got_obj) = get_obj_in_list_vis(
            &game.descriptors,
            chars,
            db,
            objs,
            ch,
            &arg2,
            None,
            &ch.carrying,
        ) {
            to_obj = got_obj;
        } else {
            send_to_char(&mut game.descriptors, ch, "You can't find it!\r\n");
            return;
        }
        if (to_obj.get_obj_type() != ItemType::Drinkcon)
            && (to_obj.get_obj_type() != ItemType::Fountain)
        {
            send_to_char(
                &mut game.descriptors,
                ch,
                "You can't pour anything into that.\r\n",
            );
            return;
        }
    } else {
        panic!("SYSERR: do_pour: invalid subcmd, but why ? {}", subcmd);
    }

    if to_obj.id() == from_obj.id() {
        send_to_char(&mut game.descriptors, ch, "A most unproductive effort.\r\n");
        return;
    }

    if (to_obj.get_obj_val(1) != 0) && (to_obj.get_obj_val(2) != to_obj.get_obj_val(2)) {
        send_to_char(
            &mut game.descriptors,
            ch,
            "There is already another liquid in it!\r\n",
        );
        return;
    }
    if to_obj.get_obj_val(1) >= to_obj.get_obj_val(0) {
        send_to_char(&mut game.descriptors, ch, "There is no room for more.\r\n");
        return;
    }
    if subcmd == SCMD_POUR {
        send_to_char(
            &mut game.descriptors,
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
        act(
            &mut game.descriptors,
            chars,
            db,
            "You gently fill $p from $P.",
            false,
            Some(ch),
            Some(to_obj),
            Some(VictimRef::Obj(from_obj)),
            TO_CHAR,
        );
        act(
            &mut game.descriptors,
            chars,
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
        name_to_drinkcon(objs, Some(to_obj_id), _type);
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
        name_from_drinkcon(objs, Some(from_obj_id));
        objs.get_mut(from_obj_id).set_obj_val(1, 0);
        objs.get_mut(from_obj_id).set_obj_val(2, 0);
        objs.get_mut(from_obj_id).set_obj_val(3, 0);
    }
    /* Then the poison boogie */
    let v = if objs.get(to_obj_id).get_obj_val(3) != 0 || objs.get(from_obj_id).get_obj_val(3) != 0
    {
        1
    } else {
        0
    };
    objs.get_mut(to_obj_id).set_obj_val(3, v);

    /* And the weight boogie */
    weight_change_object(chars, objs, from_obj_id, -amount);
    weight_change_object(chars, objs, to_obj_id, amount); /* Add weight */
}

fn wear_message(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
    _where: usize,
) {
    let ch = chars.get(chid);
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

    act(
        descs,
        chars,
        db,
        WEAR_MESSAGES[_where][0],
        true,
        Some(ch),
        Some(obj),
        None,
        TO_ROOM,
    );
    act(
        descs,
        chars,
        db,
        WEAR_MESSAGES[_where][1],
        false,
        Some(ch),
        Some(obj),
        None,
        TO_CHAR,
    );
}

fn perform_wear(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    oid: DepotId,
    _where: usize,
) {
    /*
     * ITEM_WEAR_TAKE is used for objects that do not require special bits
     * to be put into that position (e.g. you can hold any object, not just
     * an object with a HOLD bit.)
     */
    let ch = chars.get(chid);
    let obj = objs.get_mut(oid);
    let mut _where = _where;
    const WEAR_BITVECTORS: [WearFlags; 18] = [
        WearFlags::TAKE,
        WearFlags::FINGER,
        WearFlags::FINGER,
        WearFlags::NECK,
        WearFlags::NECK,
        WearFlags::BODY,
        WearFlags::HEAD,
        WearFlags::LEGS,
        WearFlags::FEET,
        WearFlags::HANDS,
        WearFlags::ARMS,
        WearFlags::SHIELD,
        WearFlags::ABOUT,
        WearFlags::WAIST,
        WearFlags::WRIST,
        WearFlags::WRIST,
        WearFlags::WIELD,
        WearFlags::TAKE,
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
    if !obj.can_wear(WEAR_BITVECTORS[_where]) {
        act(
            descs,
            chars,
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
    if ((_where == WEAR_FINGER_R) || (_where == WEAR_NECK_1) || (_where == WEAR_WRIST_R))
        && ch.get_eq(_where).is_some()
    {
        _where += 1;
    }

    if ch.get_eq(_where).is_some() {
        send_to_char(descs, ch, ALREADY_WEARING[_where]);
        return;
    }
    wear_message(descs, db, chars, objs, chid, oid, _where);
    let obj = objs.get_mut(oid);
    obj_from_char(chars, obj);
    equip_char(descs, chars, db, objs, chid, oid, _where);
}

pub fn find_eq_pos(
    descs: &mut Depot<DescriptorData>,
    ch: &CharData,
    obj: &ObjData,
    arg: &str,
) -> i16 {
    let mut _where: i16 = -1; // TODO change return type to Option<i16>

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
    if arg.is_empty() {
        if obj.can_wear(WearFlags::FINGER) {
            _where = WEAR_FINGER_R as i16;
        }
        if obj.can_wear(WearFlags::NECK) {
            _where = WEAR_NECK_1 as i16;
        }
        if obj.can_wear(WearFlags::BODY) {
            _where = WEAR_BODY as i16;
        }
        if obj.can_wear(WearFlags::HEAD) {
            _where = WEAR_HEAD as i16;
        }
        if obj.can_wear(WearFlags::LEGS) {
            _where = WEAR_LEGS as i16;
        }
        if obj.can_wear(WearFlags::FEET) {
            _where = WEAR_FEET as i16;
        }
        if obj.can_wear(WearFlags::HANDS) {
            _where = WEAR_HANDS as i16;
        }
        if obj.can_wear(WearFlags::ARMS) {
            _where = WEAR_ARMS as i16;
        }
        if obj.can_wear(WearFlags::SHIELD) {
            _where = WEAR_SHIELD as i16;
        }
        if obj.can_wear(WearFlags::ABOUT) {
            _where = WEAR_ABOUT as i16;
        }
        if obj.can_wear(WearFlags::WAIST) {
            _where = WEAR_WAIST as i16;
        }
        if obj.can_wear(WearFlags::WRIST) {
            _where = WEAR_WRIST_R as i16;
        }
    } else if let Some(found) = search_block(arg, &KEYWORDS, false) {
        _where = found as i16;
    } else {
        send_to_char(
            descs,
            ch,
            format!("'{}'?  What part of your body is THAT?\r\n", arg).as_str(),
        );
    }

    _where
}

#[allow(clippy::too_many_arguments)]
pub fn do_wear(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg1 = String::new();
    let mut arg2 = String::new();

    two_arguments(argument, &mut arg1, &mut arg2);

    if arg1.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Wear what?\r\n");
        return;
    }
    let dotmode = find_all_dots(&arg1);

    if !arg2.is_empty() && dotmode != FindAllType::Individual {
        send_to_char(
            &mut game.descriptors,
            ch,
            "You can't specify the same body location for more than one item!\r\n",
        );
        return;
    }
    let mut _where = -1;
    let mut items_worn = 0;
    if dotmode == FindAllType::All {
        for oid in ch.carrying.clone() {
            let ch = chars.get(chid);
            let obj = objs.get(oid);
            if can_see_obj(&game.descriptors, chars, db, ch, obj) && {
                _where = find_eq_pos(&mut game.descriptors, ch, obj, "");
                _where >= 0
            } {
                items_worn += 1;
                perform_wear(
                    &mut game.descriptors,
                    db,
                    chars,
                    objs,
                    chid,
                    oid,
                    _where as usize,
                );
            }
        }
        if items_worn == 0 {
            let ch = chars.get(chid);
            send_to_char(
                &mut game.descriptors,
                ch,
                "You don't seem to have anything wearable.\r\n",
            );
        }
    } else if dotmode == FindAllType::AllDot {
        if arg1.is_empty() {
            send_to_char(&mut game.descriptors, ch, "Wear all of what?\r\n");
            return;
        }
        let mut obj;
        let res = {
            obj = get_obj_in_list_vis(
                &game.descriptors,
                chars,
                db,
                objs,
                ch,
                &arg1,
                None,
                &ch.carrying,
            );
            obj.is_none()
        };
        if res {
            send_to_char(
                &mut game.descriptors,
                ch,
                format!("You don't seem to have any {}s.\r\n", arg1).as_str(),
            );
        } else {
            while let Some(the_obj) = obj {
                let ch = chars.get(chid);
                let res = {
                    _where = find_eq_pos(&mut game.descriptors, ch, the_obj, "");
                    _where >= 0
                };
                if res {
                    perform_wear(
                        &mut game.descriptors,
                        db,
                        chars,
                        objs,
                        chid,
                        the_obj.id(),
                        _where as usize,
                    );
                } else {
                    act(
                        &mut game.descriptors,
                        chars,
                        db,
                        "You can't wear $p.",
                        false,
                        Some(ch),
                        obj,
                        None,
                        TO_CHAR,
                    );
                }
                let ch = chars.get(chid);
                obj = get_obj_in_list_vis(
                    &game.descriptors,
                    chars,
                    db,
                    objs,
                    ch,
                    &arg1,
                    None,
                    &ch.carrying,
                );
            }
        }
    } else if let Some(obj) = get_obj_in_list_vis(
        &game.descriptors,
        chars,
        db,
        objs,
        ch,
        &arg1,
        None,
        &ch.carrying,
    ) {
        _where = find_eq_pos(&mut game.descriptors, ch, obj, &arg2);
        if _where >= 0 {
            perform_wear(
                &mut game.descriptors,
                db,
                chars,
                objs,
                chid,
                obj.id(),
                _where as usize,
            );
        } else if arg2.is_empty() {
            act(
                &mut game.descriptors,
                chars,
                db,
                "You can't wear $p.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
        }
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg1), arg1).as_str(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_wield(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Wield what?\r\n");
    } else if let Some(obj) = get_obj_in_list_vis(
        &game.descriptors,
        chars,
        db,
        objs,
        ch,
        &arg,
        None,
        &ch.carrying,
    ) {
        if !obj.can_wear(WearFlags::WIELD) {
            send_to_char(&mut game.descriptors, ch, "You can't wield that.\r\n");
        } else if obj.get_obj_weight() > STR_APP[ch.strength_apply_index()].wield_w as i32 {
            send_to_char(
                &mut game.descriptors,
                ch,
                "It's too heavy for you to use.\r\n",
            );
        } else {
            perform_wear(
                &mut game.descriptors,
                db,
                chars,
                objs,
                chid,
                obj.id(),
                WEAR_WIELD,
            );
        }
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_grab(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    #[allow(clippy::blocks_in_conditions)]
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Hold what?\r\n");
    } else if let Some(obj) = get_obj_in_list_vis(
        &game.descriptors,
        chars,
        db,
        objs,
        ch,
        &arg,
        None,
        &ch.carrying,
    ) {
        if obj.get_obj_type() == ItemType::Light {
            perform_wear(
                &mut game.descriptors,
                db,
                chars,
                objs,
                chid,
                obj.id(),
                WEAR_LIGHT,
            );
        } else if !obj.can_wear(WearFlags::HOLD)
            && obj.get_obj_type() != ItemType::Wand
            && obj.get_obj_type() != ItemType::Staff
            && obj.get_obj_type() != ItemType::Scroll
            && obj.get_obj_type() != ItemType::Potion
        {
            send_to_char(&mut game.descriptors, ch, "You can't hold that.\r\n");
        } else {
            perform_wear(
                &mut game.descriptors,
                db,
                chars,
                objs,
                chid,
                obj.id(),
                WEAR_HOLD,
            );
        }
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
    }
}

fn perform_remove(
    descs: &mut Depot<DescriptorData>,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    pos: usize,
) {
    let ch = chars.get(chid);

    if let Some(oid) = ch.get_eq(pos) {
        if objs.get(oid).obj_flagged(ExtraFlags::NODROP) {
            let obj = objs.get(oid);
            act(
                descs,
                chars,
                db,
                "You can't remove $p, it must be CURSED!",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
        } else if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
            let obj = objs.get(oid);
            act(
                descs,
                chars,
                db,
                "$p: you can't carry that many items!",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
        } else if let Some(eqid) = db.unequip_char(chars, objs, chid, pos) {
            let eq = objs.get_mut(eqid);
            let ch = chars.get_mut(chid);
            obj_to_char(eq, ch);
            let ch = chars.get(chid);
            let obj = objs.get(oid);
            act(
                descs,
                chars,
                db,
                "You stop using $p.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
            act(
                descs,
                chars,
                db,
                "$n stops using $p.",
                true,
                Some(ch),
                Some(obj),
                None,
                TO_ROOM,
            );
        } else {
            error!("SYSERR: unequip_char failed for pos {} !", pos);
        }
    } else {
        error!("SYSERR: perform_remove: bad pos {} passed.", pos);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn do_remove(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Remove what?\r\n");
        return;
    }
    let dotmode = find_all_dots(&arg);

    let mut found = false;
    if dotmode == FindAllType::All {
        for i in 0..NUM_WEARS {
            let ch = chars.get(chid);
            if ch.get_eq(i).is_some() {
                perform_remove(&mut game.descriptors, db, chars, objs, chid, i);
                found = true;
            }
        }
        if !found {
            let ch = chars.get(chid);
            send_to_char(&mut game.descriptors, ch, "You're not using anything.\r\n");
        }
    } else if dotmode == FindAllType::AllDot {
        if arg.is_empty() {
            send_to_char(&mut game.descriptors, ch, "Remove all of what?\r\n");
        } else {
            found = false;
            for i in 0..NUM_WEARS {
                let ch = chars.get(chid);
                if let Some(eqid) = ch.get_eq(i) {
                    if can_see_obj(&game.descriptors, chars, db, ch, objs.get(eqid))
                        && isname(&arg, objs.get(eqid).name.as_ref())
                    {
                        perform_remove(&mut game.descriptors, db, chars, objs, chid, i);
                        found = true;
                    }
                }
            }
            if !found {
                let ch = chars.get(chid);
                send_to_char(
                    &mut game.descriptors,
                    ch,
                    format!("You don't seem to be using any {}s.\r\n", arg).as_str(),
                );
            }
        }
    } else if let Some(i) = get_obj_pos_in_equip_vis(
        &game.descriptors,
        chars,
        db,
        objs,
        ch,
        &arg,
        None,
        &ch.equipment,
    ) {
        perform_remove(&mut game.descriptors, db, chars, objs, chid, i);
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            format!("You don't seem to be using {} {}.\r\n", an!(arg), arg).as_str(),
        );
    }
}
