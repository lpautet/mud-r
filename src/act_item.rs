/* ************************************************************************
*   File: act.item.rs                                   Part of CircleMUD *
*  Usage: object handling routines -- get/drop and container handling     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::cmp::{max, min};
use std::rc::Rc;

use log::error;

use crate::config::{DONATION_ROOM_1, NOPERSON, OK};
use crate::constants::{DRINKNAMES, DRINKS, DRINK_AFF, STR_APP};
use crate::db::DB;
use crate::handler::{
    affect_join, find_all_dots, isname, money_desc, obj_from_char, FIND_ALL, FIND_ALLDOT,
    FIND_CHAR_ROOM, FIND_INDIV, FIND_OBJ_INV, FIND_OBJ_ROOM,
};
use crate::interpreter::{
    is_number, one_argument, search_block, two_arguments, SCMD_DONATE, SCMD_DRINK, SCMD_DROP,
    SCMD_EAT, SCMD_FILL, SCMD_JUNK, SCMD_POUR, SCMD_SIP, SCMD_TASTE,
};
use crate::spells::SPELL_POISON;
use crate::structs::{
    AffectedType, CharData, ObjData, RoomRnum, AFF_POISON, APPLY_NONE, CONT_CLOSED, DRUNK, FULL,
    ITEM_CONTAINER, ITEM_DRINKCON, ITEM_FOOD, ITEM_FOUNTAIN, ITEM_LIGHT, ITEM_MONEY, ITEM_NODONATE,
    ITEM_NODROP, ITEM_POTION, ITEM_SCROLL, ITEM_STAFF, ITEM_WAND, ITEM_WEAR_ABOUT, ITEM_WEAR_ARMS,
    ITEM_WEAR_BODY, ITEM_WEAR_FEET, ITEM_WEAR_FINGER, ITEM_WEAR_HANDS, ITEM_WEAR_HEAD,
    ITEM_WEAR_HOLD, ITEM_WEAR_LEGS, ITEM_WEAR_NECK, ITEM_WEAR_SHIELD, ITEM_WEAR_TAKE,
    ITEM_WEAR_WAIST, ITEM_WEAR_WIELD, ITEM_WEAR_WRIST, LVL_GOD, LVL_IMMORT, NOWHERE, NUM_WEARS,
    PULSE_VIOLENCE, THIRST, WEAR_ABOUT, WEAR_ARMS, WEAR_BODY, WEAR_FEET, WEAR_FINGER_R, WEAR_HANDS,
    WEAR_HEAD, WEAR_HOLD, WEAR_LEGS, WEAR_LIGHT, WEAR_NECK_1, WEAR_SHIELD, WEAR_WAIST, WEAR_WIELD,
    WEAR_WRIST_R,
};
use crate::util::{clone_vec, clone_vec2, rand_number};
use crate::{an, send_to_char, Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

fn perform_put(game: &mut Game, ch: &Rc<CharData>, obj: &Rc<ObjData>, cont: &Rc<ObjData>) {
    if cont.get_obj_weight() + obj.get_obj_weight() > cont.get_obj_val(0) {
        game.db.act(
            "$p won't fit in $P.",
            false,
            Some(ch),
            Some(obj),
            Some(cont),
            TO_CHAR,
        );
    } else if obj.obj_flagged(ITEM_NODROP) && cont.in_room() != NOWHERE {
        game.db.act(
            "You can't get $p out of your hand.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
    } else {
        obj_from_char(obj);
        game.db.obj_to_obj(obj, cont);

        game.db.act(
            "$n puts $p in $P.",
            true,
            Some(ch),
            Some(obj),
            Some(cont),
            TO_ROOM,
        );

        /* Yes, I realize this is strange until we have auto-equip on rent. -gg */
        if obj.obj_flagged(ITEM_NODROP) && !cont.obj_flagged(ITEM_NODROP) {
            cont.set_obj_extra_bit(ITEM_NODROP);

            game.db.act(
                "You get a strange feeling as you put $p in $P.",
                false,
                Some(ch),
                Some(obj),
                Some(cont),
                TO_CHAR,
            );
        } else {
            game.db.act(
                "You put $p in $P.",
                false,
                Some(ch),
                Some(obj),
                Some(cont),
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
pub fn do_put(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
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
    let mut cont = None;
    let mut obj;

    if theobj.is_empty() {
        send_to_char(ch, "Put what in what?\r\n");
    } else if cont_dotmode != FIND_INDIV {
        send_to_char(
            ch,
            "You can only put things into one container at a time.\r\n",
        );
    } else if thecont.is_empty() {
        send_to_char(
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
        game.db.generic_find(
            &thecont,
            (FIND_OBJ_INV | FIND_OBJ_ROOM) as i64,
            ch,
            &mut tmp_char,
            &mut cont,
        );
        if cont.is_none() {
            send_to_char(
                ch,
                format!("You don't see {} {} here.\r\n", an!(thecont), thecont).as_str(),
            );
        } else if cont.as_ref().unwrap().get_obj_type() != ITEM_CONTAINER {
            game.db.act(
                "$p is not a container.",
                false,
                Some(ch),
                Some(cont.as_ref().unwrap()),
                None,
                TO_CHAR,
            );
        } else if cont.as_ref().unwrap().objval_flagged(CONT_CLOSED) {
            send_to_char(ch, "You'd better open it first!\r\n");
        } else {
            if obj_dotmode == FIND_INDIV {
                /* put <obj> <container> */

                if {
                    obj = game.db.get_obj_in_list_vis(ch, &theobj, None, ch.carrying.borrow());
                    obj.is_none()
                } {
                    send_to_char(
                        ch,
                        format!("You aren't carrying {} {}.\r\n", an!(theobj), theobj).as_str(),
                    );
                } else if Rc::ptr_eq(obj.as_ref().unwrap(), cont.as_ref().unwrap()) && howmany == 1
                {
                    send_to_char(ch, "You attempt to fold it into itself, but fail.\r\n");
                } else {
                    while obj.is_some() && howmany != 0 {
                        if !Rc::ptr_eq(obj.as_ref().unwrap(), cont.as_ref().unwrap()) {
                            howmany -= 1;
                            perform_put(game, ch, obj.as_ref().unwrap(), cont.as_ref().unwrap());
                        }
                        obj = game.db.get_obj_in_list_vis(ch, &theobj, None, ch.carrying.borrow());
                    }
                }
            } else {
                for obj in ch.carrying.borrow().iter() {
                    if !Rc::ptr_eq(obj, cont.as_ref().unwrap())
                        && (obj_dotmode == FIND_ALL || isname(&theobj, &obj.name.borrow()))
                    {
                        found = true;
                        perform_put(game, ch, obj, cont.as_ref().unwrap());
                    }
                }
                if !found {
                    if obj_dotmode == FIND_ALL {
                        send_to_char(ch, "You don't seem to have anything to put in it.\r\n");
                    } else {
                        send_to_char(
                            ch,
                            format!("You don't seem to have any {}s.\r\n", theobj).as_str(),
                        );
                    }
                }
            }
        }
    }
}

fn can_take_obj(game: &mut Game, ch: &Rc<CharData>, obj: &Rc<ObjData>) -> bool {
    if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
        game.db.act(
            "$p: you can't carry that many items.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return false;
    } else if (ch.is_carrying_w() + obj.get_obj_weight()) > ch.can_carry_w() as i32 {
        game.db.act(
            "$p: you can't carry that much weight.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        return false;
    } else if !obj.can_wear(ITEM_WEAR_TAKE) {
        game.db.act(
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

fn get_check_money(game: &mut Game, ch: &Rc<CharData>, obj: &Rc<ObjData>) {
    let value = obj.get_obj_val(0);

    if obj.get_obj_type() != ITEM_MONEY || value <= 0 {
        return;
    }

    game.db.extract_obj(obj);

    ch.set_gold(ch.get_gold() + value);

    if value == 1 {
        send_to_char(ch, "There was 1 coin.\r\n");
    } else {
        send_to_char(ch, format!("There were {} coins.\r\n", value).as_str());
    }
}

fn perform_get_from_container(
    game: &mut Game,
    ch: &Rc<CharData>,
    obj: &Rc<ObjData>,
    cont: &Rc<ObjData>,
    mode: i32,
) {
    if mode == FIND_OBJ_INV || can_take_obj(game, ch, obj) {
        if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
            game.db.act(
                "$p: you can't hold any more items.",
                false,
                Some(ch),
                Some(obj),
                None,
                TO_CHAR,
            );
        } else {
            DB::obj_from_obj(obj);
            DB::obj_to_char(obj, ch);
            game.db.act(
                "You get $p from $P.",
                false,
                Some(ch),
                Some(obj),
                Some(cont),
                TO_CHAR,
            );
            game.db.act(
                "$n gets $p from $P.",
                true,
                Some(ch),
                Some(obj),
                Some(cont),
                TO_ROOM,
            );
            get_check_money(game, ch, obj);
        }
    }
}

fn get_from_container(
    game: &mut Game,
    ch: &Rc<CharData>,
    cont: &Rc<ObjData>,
    arg: &str,
    mode: i32,
    howmany: i32,
) {
    let mut found = false;

    let mut howmany = howmany;
    let obj_dotmode = find_all_dots(arg);

    if cont.obj_flagged(CONT_CLOSED) {
        game.db.act("$p is closed.", false, Some(ch), Some(cont), None, TO_CHAR);
    } else if obj_dotmode == FIND_INDIV {
        let mut obj = game.db.get_obj_in_list_vis(ch, arg, None, cont.contains.borrow());
        if obj.is_none() {
            let buf = format!("There doesn't seem to be {} {} in $p.", an!(arg), arg);
            game.db.act(&buf, false, Some(ch), Some(cont), None, TO_CHAR);
        } else {
            while obj.is_some() && howmany != 0 {
                howmany -= 1;
                perform_get_from_container(game, ch, obj.as_ref().unwrap(), cont, mode);
                obj = game.db.get_obj_in_list_vis(ch, arg, None, cont.contains.borrow());
            }
        }
    } else {
        if obj_dotmode == FIND_ALLDOT && arg.is_empty() {
            send_to_char(ch, "Get all of what?\r\n");
            return;
        }
        let list = clone_vec(&cont.contains);
        for obj in list {
            if game.db.can_see_obj(ch, &obj)
                && (obj_dotmode == FIND_ALL || isname(arg, &obj.name.borrow()))
            {
                found = true;
                perform_get_from_container(game, ch, &obj, cont, mode);
            }
        }
        if !found {
            if obj_dotmode == FIND_ALL {
                game.db.act(
                    "$p seems to be empty.",
                    false,
                    Some(ch),
                    Some(cont),
                    None,
                    TO_CHAR,
                );
            } else {
                let buf = format!("You can't seem to find any {}s in $p.", arg);
                game.db.act(&buf, false, Some(ch), Some(cont), None, TO_CHAR);
            }
        }
    }
}

fn perform_get_from_room(game: &mut Game, ch: &Rc<CharData>, obj: &Rc<ObjData>) -> bool {
    if can_take_obj(game, ch, obj) {
        game.db.obj_from_room(obj);
        DB::obj_to_char(obj, ch);
        game.db.act("You get $p.", false, Some(ch), Some(obj), None, TO_CHAR);
        game.db.act("$n gets $p.", true, Some(ch), Some(obj), None, TO_ROOM);
        get_check_money(game, ch, obj);
        return true;
    }
    return false;
}

fn get_from_room(game: &mut Game, ch: &Rc<CharData>, arg: &str, howmany: i32) {
    let mut found = false;
    let mut howmany = howmany;
    let dotmode = find_all_dots(arg);

    if dotmode == FIND_INDIV {
        let mut obj = game.db.get_obj_in_list_vis2(
            ch,
            arg,
            None,
            &game.db.world[ch.in_room() as usize].contents,
        );
        if obj.is_none() {
            send_to_char(
                ch,
                format!("You don't see {} {} here.\r\n", an!(arg), arg).as_str(),
            );
        } else {
            while obj.is_some() {
                if howmany == 0 {
                    break;
                }
                howmany -= 1;
                perform_get_from_room(game, ch, obj.as_ref().unwrap());
                obj = game.db.get_obj_in_list_vis2(
                    ch,
                    arg,
                    None,
                    &game.db.world[ch.in_room() as usize].contents,
                );
            }
        }
    } else {
        if dotmode == FIND_ALLDOT && arg.is_empty() {
            send_to_char(ch, "Get all of what?\r\n");
            return;
        }
        let list = clone_vec2(&game.db.world[ch.in_room() as usize].contents);
        for obj in list.iter()
        {
            if game.db.can_see_obj(ch, obj) && (dotmode == FIND_ALL || isname(arg, &obj.name.borrow())) {
                found = true;
                perform_get_from_room(game, ch, obj);
            }
        }
        if !found {
            if dotmode == FIND_ALL {
                send_to_char(ch, "There doesn't seem to be anything here.\r\n");
            } else {
                send_to_char(ch, format!("You don't see any {}s here.\r\n", arg).as_str());
            }
        }
    }
}

pub fn do_get(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut arg3 = String::new();
    let mut tmp_char: Option<Rc<CharData>> = None;
    let mut cont: Option<Rc<ObjData>> = None;

    let mut found = false;
    one_argument(two_arguments(argument, &mut arg1, &mut arg2), &mut arg3); /* three_arguments */

    if arg1.is_empty() {
        send_to_char(ch, "Get what?\r\n");
    } else if arg2.is_empty() {
        get_from_room(game, ch, &arg1, 1);
    } else if is_number(&arg1) && arg3.is_empty() {
        get_from_room(game, ch, &arg2, arg1.parse::<i32>().unwrap());
    } else {
        let mut amount = 1;
        if is_number(&arg1) {
            amount = arg1.parse::<i32>().unwrap();
            arg1 = arg2; /* strcpy: OK (sizeof: arg1 == arg2) */
            arg2 = arg3; /* strcpy: OK (sizeof: arg2 == arg3) */
        }
        let cont_dotmode = find_all_dots(&arg2);
        if cont_dotmode == FIND_INDIV {
            let mode = game.db.generic_find(
                &arg2,
                (FIND_OBJ_INV | FIND_OBJ_ROOM) as i64,
                ch,
                &mut tmp_char,
                &mut cont,
            );
            if cont.is_none() {
                send_to_char(
                    ch,
                    format!("You don't have {} {}.\r\n", an!(&arg2), &arg2).as_str(),
                );
            } else if cont.as_ref().unwrap().get_obj_type() != ITEM_CONTAINER {
                game.db.act(
                    "$p is not a container.",
                    false,
                    Some(ch),
                    Some(cont.as_ref().unwrap()),
                    None,
                    TO_CHAR,
                );
            } else {
                get_from_container(game, ch, cont.as_ref().unwrap(), &arg1, mode, amount);
            }
        } else {
            if cont_dotmode == FIND_ALLDOT && arg2.is_empty() {
                send_to_char(ch, "Get from all of what?\r\n");
                return;
            }
            for cont in ch.carrying.borrow().iter() {
                if game.db.can_see_obj(ch, cont)
                    && (cont_dotmode == FIND_ALL || isname(&arg2, &cont.name.borrow()))
                {
                    if cont.get_obj_type() == ITEM_CONTAINER {
                        found = true;
                        get_from_container(game, ch, cont, &arg1, FIND_OBJ_INV, amount);
                    } else if cont_dotmode == FIND_ALLDOT {
                        found = true;
                        game.db.act(
                            "$p is not a container.",
                            false,
                            Some(ch),
                            Some(cont),
                            None,
                            TO_CHAR,
                        );
                    }
                }
            }
            let list = clone_vec2(&game.db.world[ch.in_room() as usize].contents);
            for cont in 
                list
                .iter()
            {
                if game.db.can_see_obj(ch, cont)
                    && (cont_dotmode == FIND_ALL || isname(&arg2, &cont.name.borrow()))
                {
                    if cont.get_obj_type() == ITEM_CONTAINER {
                        get_from_container(game, ch, cont, &arg1, FIND_OBJ_ROOM, amount);
                        found = true;
                    } else if cont_dotmode == FIND_ALLDOT {
                        game.db.act(
                            "$p is not a container.",
                            false,
                            Some(ch),
                            Some(cont),
                            None,
                            TO_CHAR,
                        );
                        found = true;
                    }
                }
            }
            if !found {
                if cont_dotmode == FIND_ALL {
                    send_to_char(ch, "You can't seem to find any containers.\r\n");
                } else {
                    send_to_char(
                        ch,
                        format!("You can't seem to find any {}s here.\r\n", &arg2).as_str(),
                    );
                }
            }
        }
    }
}

fn perform_drop_gold(game: &mut Game, ch: &Rc<CharData>, amount: i32, mode: u8, rdr: RoomRnum) {
    if amount <= 0 {
        send_to_char(ch, "Heh heh heh.. we are jolly funny today, eh?\r\n");
    } else if ch.get_gold() < amount {
        send_to_char(ch, "You don't have that many coins!\r\n");
    } else {
        if mode != SCMD_JUNK as u8 {
            ch.set_wait_state(PULSE_VIOLENCE as i32); /* to prevent coin-bombing */

            let obj = game.db.create_money(amount).unwrap();
            if mode == SCMD_DONATE as u8 {
                send_to_char(
                    ch,
                    "You throw some gold into the air where it disappears in a puff of smoke!\r\n",
                );
                game.db.act(
                    "$n throws some gold into the air where it disappears in a puff of smoke!",
                    false,
                    Some(ch),
                    None,
                    None,
                    TO_ROOM,
                );
                game.db.obj_to_room(&obj, rdr);
                game.db.act(
                    "$p suddenly appears in a puff of orange smoke!",
                    false,
                    None,
                    Some(obj.as_ref()),
                    None,
                    TO_ROOM,
                );
            } else {
                let buf = format!("$n drops {}.", money_desc(amount));
                game.db.act(&buf, true, Some(ch), None, None, TO_ROOM);

                send_to_char(ch, "You drop some gold.\r\n");
                game.db.obj_to_room(&obj, ch.in_room());
            }
        } else {
            let buf = format!(
                "$n drops {} which disappears in a puff of smoke!",
                money_desc(amount)
            );
            game.db.act(&buf, false, Some(ch), None, None, TO_ROOM);

            send_to_char(
                ch,
                "You drop some gold which disappears in a puff of smoke!\r\n",
            );
        }
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
    ch: &Rc<CharData>,
    obj: &Rc<ObjData>,
    mut mode: u8,
    sname: &str,
    rdr: RoomRnum,
) -> i32 {
    if obj.obj_flagged(ITEM_NODROP) {
        let buf = format!("You can't {} $p, it must be CURSED!", sname);
        game.db.act(&buf, false, Some(ch), Some(obj), None, TO_CHAR);
        return 0;
    }

    let buf = format!("You {} $p.{}", sname, vanish!(mode));
    game.db.act(&buf, false, Some(ch), Some(obj), None, TO_CHAR);

    let buf = format!("$n {}s $p.{}", sname, vanish!(mode));
    game.db.act(&buf, true, Some(ch), Some(obj), None, TO_ROOM);

    obj_from_char(obj);

    if (mode == SCMD_DONATE as u8) && obj.obj_flagged(ITEM_NODONATE) {
        mode = SCMD_JUNK as u8;
    }

    match mode {
        SCMD_DROP => {
            game.db.obj_to_room(obj, ch.in_room());
        }

        SCMD_DONATE => {
            game.db.obj_to_room(obj, rdr);
            game.db.act(
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
            let value = max(1, min(200, obj.get_obj_cost() / 16));
            game.db.extract_obj(obj);
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

pub fn do_drop(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
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
                    rdr = game.db.real_room(DONATION_ROOM_1);
                }
                /*    case 3: RDR = real_room(donation_room_2); break;
                      case 4: RDR = real_room(donation_room_3); break;
                */
                _ => {}
            }
            if rdr == NOWHERE {
                send_to_char(ch, "Sorry, you can't donate anything right now.\r\n");
                return;
            }
        }
        _ => {
            sname = "drop";
        }
    }

    let mut arg = String::new();
    let argument = one_argument(argument, &mut arg);
    let mut obj: Option<Rc<ObjData>>;
    let mut amount = 0;
    let dotmode;

    if arg.is_empty() {
        send_to_char(ch, format!("What do you want to {}?\r\n", sname).as_str());
        return;
    } else if is_number(&arg) {
        let mut multi = arg.parse::<i32>().unwrap();
        one_argument(argument, &mut arg);
        if arg == "coins" || arg == "coin" {
            perform_drop_gold(game, ch, multi, mode, rdr);
        } else if multi <= 0 {
            send_to_char(ch, "Yeah, that makes sense.\r\n");
        } else if arg.is_empty() {
            send_to_char(
                ch,
                format!("What do you want to {} {} of?\r\n", sname, multi).as_str(),
            );
        } else if {
            obj = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
            obj.is_none()
        } {
            send_to_char(
                ch,
                format!("You don't seem to have any {}s.\r\n", arg).as_str(),
            );
        } else {
            loop {
                amount += perform_drop(game, ch, obj.as_ref().unwrap(), mode, sname, rdr);
                obj = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
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
                send_to_char(ch, "Go to the dump if you want to junk EVERYTHING!\r\n");
            } else {
                send_to_char(
                    ch,
                    "Go do the donation room if you want to donate EVERYTHING!\r\n",
                );
                return;
            }
        }
        if dotmode == FIND_ALL {
            if ch.carrying.borrow().is_empty() {
                send_to_char(ch, "You don't seem to be carrying anything.\r\n");
            } else {
                for obj in clone_vec(&ch.carrying).iter() {
                    amount += perform_drop(game, ch, obj, mode, sname, rdr);
                }
            }
        } else if dotmode == FIND_ALLDOT {
            if arg.is_empty() {
                send_to_char(
                    ch,
                    format!("What do you want to {} all of?\r\n", sname).as_str(),
                );
                return;
            }
            if {
                obj = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
                obj.is_none()
            } {
                send_to_char(
                    ch,
                    format!("You don't seem to have any {}s.\r\n", arg).as_str(),
                );
            }

            while obj.is_some() {
                amount += perform_drop(game, ch, obj.as_ref().unwrap(), mode, sname, rdr);
                obj = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
            }
        } else {
            if {
                obj = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
                obj.is_none()
            } {
                send_to_char(
                    ch,
                    format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
                );
            } else {
                amount += perform_drop(game, ch, obj.as_ref().unwrap(), mode, sname, rdr);
            }
        }
    }

    if amount != 0 && subcmd == SCMD_JUNK as i32 {
        send_to_char(ch, "You have been rewarded by the gods!\r\n");
        game.db.act(
            "$n has been rewarded by the gods!",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        ch.set_gold(ch.get_gold() + amount);
    }
}

fn perform_give(game: &mut Game, ch: &Rc<CharData>, vict: &Rc<CharData>, obj: &Rc<ObjData>) {
    if obj.obj_flagged(ITEM_NODROP) {
        game.db.act(
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
        game.db.act(
            "$N seems to have $S hands full.",
            false,
            Some(ch),
            None,
            Some(vict),
            TO_CHAR,
        );
        return;
    }
    if obj.get_obj_weight() + vict.is_carrying_w() > vict.can_carry_w() as i32 {
        game.db.act(
            "$E can't carry that much weight.",
            false,
            Some(ch),
            None,
            Some(vict),
            TO_CHAR,
        );
        return;
    }
    obj_from_char(obj);
    DB::obj_to_char(obj, vict);
    game.db.act(
        "You give $p to $N.",
        false,
        Some(ch),
        Some(obj),
        Some(vict),
        TO_CHAR,
    );
    game.db.act(
        "$n gives you $p.",
        false,
        Some(ch),
        Some(obj),
        Some(vict),
        TO_VICT,
    );
    game.db.act(
        "$n gives $p to $N.",
        true,
        Some(ch),
        Some(obj),
        Some(vict),
        TO_NOTVICT,
    );
}

/* utility function for give */
fn give_find_vict(game: &mut Game, ch: &Rc<CharData>, arg: &str) -> Option<Rc<CharData>> {
    let vict;
    let mut arg = arg.trim_start().to_string();

    if arg.is_empty() {
        send_to_char(ch, "To who?\r\n");
    } else if {
        vict = game.db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(ch, NOPERSON);
    } else if Rc::ptr_eq(vict.as_ref().unwrap(), ch) {
        send_to_char(ch, "What's the point of that?\r\n");
    } else {
        return vict;
    }

    None
}

fn perform_give_gold(game: &mut Game, ch: &Rc<CharData>, vict: &Rc<CharData>, amount: i32) {
    let mut buf;

    if amount <= 0 {
        send_to_char(ch, "Heh heh heh ... we are jolly funny today, eh?\r\n");
        return;
    }
    if ch.get_gold() < amount && (ch.is_npc() || (ch.get_level() < LVL_GOD as u8)) {
        send_to_char(ch, "You don't have that many coins!\r\n");
        return;
    }
    send_to_char(ch, OK);

    buf = format!(
        "$n gives you {} gold coin{}.",
        amount,
        if amount == 1 { "" } else { "s" }
    );
    game.db.act(&buf, false, Some(ch), None, Some(vict), TO_VICT);

    buf = format!("$n gives {} to $N.", money_desc(amount));
    game.db.act(&buf, true, Some(ch), None, Some(vict), TO_NOTVICT);

    if ch.is_npc() || ch.get_level() < LVL_GOD as u8 {
        ch.set_gold(ch.get_gold() - amount);
    }
    vict.set_gold(vict.get_gold() + amount);
}

pub fn do_give(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();

    let mut argument = one_argument(argument, &mut arg);
    let mut amount;
    let mut vict = None;
    let mut obj = None;

    if arg.is_empty() {
        send_to_char(ch, "Give what to who?\r\n");
    } else if is_number(&arg) {
        amount = arg.parse::<i32>().unwrap();
        argument = one_argument(argument, &mut arg);
        if arg == "coins" || arg == "coin" {
            one_argument(argument, &mut arg);
            if {
                vict = give_find_vict(game, ch, &arg);
                vict.is_some()
            } {
                perform_give_gold(game, ch, vict.as_ref().unwrap(), amount);
                return;
            } else if arg.is_empty() {
                /* Give multiple code. */
                send_to_char(
                    ch,
                    format!("What do you want to give {} of?\r\n", amount).as_str(),
                );
            } else if {
                vict = give_find_vict(game, ch, argument);
                vict.is_none()
            } {
                return;
            } else if {
                obj = game
                    .db
                    .get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
                obj.is_none()
            } {
            }
            send_to_char(
                ch,
                format!("You don't seem to have any {}s.\r\n", arg).as_str(),
            );
        } else {
            while obj.is_some() && amount != 0 {
                amount -= 1;
                perform_give(game, ch, vict.as_ref().unwrap(), obj.as_ref().unwrap());
                obj = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
            }
        }
    } else {
        let mut buf1 = String::new();
        one_argument(argument, &mut buf1);
        if {
            vict = give_find_vict(game, ch, &buf1);
            vict.is_none()
        } {
            return;
        }
        let dotmode = find_all_dots(&arg);
        if dotmode == FIND_INDIV {
            if {
                obj = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
                obj.is_none()
            } {
                send_to_char(
                    ch,
                    format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
                );
            } else {
                perform_give(game, ch, vict.as_ref().unwrap(), obj.as_ref().unwrap());
            }
        } else {
            if dotmode == FIND_ALLDOT && arg.is_empty() {
                send_to_char(ch, "All of what?\r\n");
                return;
            }
            if ch.carrying.borrow().len() == 0 {
                send_to_char(ch, "You don't seem to be holding anything.\r\n");
            } else {
                for obj in ch.carrying.borrow().iter() {
                    if game.db.can_see_obj(ch, obj)
                        && (dotmode == FIND_ALL || isname(&arg, &obj.name.borrow()))
                    {
                        perform_give(game, ch, vict.as_ref().unwrap(), obj);
                    }
                }
            }
        }
    }
}

pub fn weight_change_object(game: &mut Game, obj: &Rc<ObjData>, weight: i32) {
    let tmp_ch;
    let tmp_obj;
    if obj.in_room() != NOWHERE {
        obj.incr_obj_weight(weight);
    } else if {
        tmp_ch = obj.carried_by.borrow().clone();
        tmp_ch.is_some()
    } {
        obj_from_char(obj);
        obj.incr_obj_weight(weight);
        DB::obj_to_char(obj, tmp_ch.as_ref().unwrap());
    } else if {
        tmp_obj = obj.in_obj.borrow();
        tmp_obj.is_some()
    } {
        DB::obj_from_obj(obj);
        obj.incr_obj_weight(weight);
        game.db.obj_to_obj(obj, tmp_obj.as_ref().unwrap());
    } else {
        error!("SYSERR: Unknown attempt to subtract weight from an object.");
    }
}

pub fn name_from_drinkcon(obj: Option<&Rc<ObjData>>) {
    if obj.is_none()
        || obj.unwrap().get_obj_type() != ITEM_DRINKCON
            && obj.unwrap().get_obj_type() != ITEM_FOUNTAIN
    {
        return;
    }
    let obj = obj.unwrap();

    let liqname = DRINKNAMES[obj.get_obj_val(2) as usize];
    if !isname(liqname, &obj.name.borrow()) {
        error!(
            "SYSERR: Can't remove liquid '{}' from '{}' ({}) item.",
            liqname,
            obj.name.borrow(),
            obj.item_number
        );
        return;
    }

    let mut new_name = String::new();
    let next = "";
    let mut bname = obj.name.borrow_mut();
    let mut cur_name = bname.as_str();
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

    *bname = new_name;
}

pub fn name_to_drinkcon(obj: Option<&Rc<ObjData>>, type_: i32) {
    let mut new_name = String::new();
    if obj.is_none()
        || obj.unwrap().get_obj_type() != ITEM_DRINKCON
            && obj.unwrap().get_obj_type() != ITEM_FOUNTAIN
    {
        return;
    }
    new_name.push_str(
        format!(
            "{} {}",
            obj.unwrap().name.borrow(),
            DRINKNAMES[type_ as usize]
        )
        .as_str(),
    );

    *obj.unwrap().name.borrow_mut() = new_name;
}

pub fn do_drink(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let mut arg = String::new();

    one_argument(argument, &mut arg);

    if ch.is_npc() {
        /* Cannot use ) on mobs. */
        return;
    }

    if arg.len() == 0 {
        send_to_char(ch, "Drink from what?\r\n");
        return;
    }
    let mut temp;
    let mut on_ground = false;
    if {
        temp = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
        temp.is_none()
    } {
        if {
            temp = game.db.get_obj_in_list_vis2(
                ch,
                &arg,
                None,
                &game.db.world[ch.in_room() as usize].contents,
            );
            temp.is_none()
        } {
            send_to_char(ch, "You can't find it!\r\n");
            return;
        } else {
            on_ground = true;
        }
    }
    let temp = temp.unwrap();
    if temp.get_obj_type() != ITEM_DRINKCON && temp.get_obj_type() != ITEM_FOUNTAIN {
        send_to_char(ch, "You can't drink from that!\r\n");
        return;
    }
    if on_ground && temp.get_obj_type() == ITEM_DRINKCON {
        send_to_char(ch, "You have to be holding that to drink from it.\r\n");
        return;
    }
    if ch.get_cond(DRUNK) > 10 && ch.get_cond(THIRST) > 0 {
        /* The pig is drunk */
        send_to_char(ch, "You can't seem to get close enough to your mouth.\r\n");
        game.db.act(
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
        send_to_char(ch, "Your stomach can't contain anymore!\r\n");
        return;
    }
    if temp.get_obj_val(1) == 0 {
        send_to_char(ch, "It's empty.\r\n");
        return;
    }
    let mut amount;
    if subcmd == SCMD_DRINK {
        let buf = format!(
            "$n DRINKS {} from $p.",
            DRINKS[temp.get_obj_val(2) as usize]
        );
        game.db.act(&buf, true, Some(ch), Some(&temp), None, TO_ROOM);

        send_to_char(
            ch,
            format!(
                "You drink the {}.\r\n",
                DRINKS[temp.get_obj_val(2) as usize]
            )
            .as_str(),
        );
        if DRINK_AFF[temp.get_obj_val(2) as usize][DRUNK as usize] > 0 {
            amount = (25 - ch.get_cond(THIRST)) as i32
                / DRINK_AFF[temp.get_obj_val(2) as usize][DRUNK as usize];
        } else {
            amount = rand_number(3, 10) as i32;
        }
    } else {
        game.db.act(
            "$n sips from $p.",
            true,
            Some(ch),
            Some(&temp),
            None,
            TO_ROOM,
        );
        send_to_char(
            ch,
            format!(
                "It tastes like {}.\r\n",
                DRINKS[temp.get_obj_val(2) as usize]
            )
            .as_str(),
        );
        amount = 1;
    }

    amount = min(amount, temp.get_obj_val(1));

    /* You can't subtract more than the object weighs */
    let weight = min(amount, temp.get_obj_weight());

    weight_change_object(game, &temp, -weight as i32); /* Subtract amount */

    game.db.gain_condition(
        ch,
        DRUNK,
        DRINK_AFF[temp.get_obj_val(2) as usize][DRUNK as usize] * amount / 4,
    );
    game.db.gain_condition(
        ch,
        FULL,
        DRINK_AFF[temp.get_obj_val(2) as usize][FULL as usize] * amount / 4,
    );
    game.db.gain_condition(
        ch,
        THIRST,
        DRINK_AFF[temp.get_obj_val(2) as usize][THIRST as usize] * amount / 4,
    );

    if ch.get_cond(DRUNK) > 10 {
        send_to_char(ch, "You feel drunk.\r\n");
    }

    if ch.get_cond(THIRST) > 20 {
        send_to_char(ch, "You don't feel thirsty any more.\r\n");
    }

    if ch.get_cond(FULL) > 20 {
        send_to_char(ch, "You are full.\r\n");
    }

    if temp.get_obj_val(3) != 0 {
        /* The crap was poisoned ! */
        send_to_char(ch, "Oops, it tasted rather strange!\r\n");
        game.db.act(
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

        affect_join(ch, &mut af, false, false, false, false);
    }
    /* empty the container, and no longer poison. */
    temp.set_obj_val(1, temp.get_obj_val(1) - amount);

    if temp.get_obj_val(1) == 0 {
        /* The last bit */
        name_from_drinkcon(Some(&temp));
        temp.set_obj_val(2, 0);
        temp.set_obj_val(3, 0);
    }
    return;
}

pub fn do_eat(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if ch.is_npc() {
        /* Cannot use ) on mobs. */
        return;
    }

    if arg.len() == 0 {
        send_to_char(ch, "Eat what?\r\n");
        return;
    }
    let food;
    if {
        food = game.db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
        food.is_none()
    } {
        send_to_char(
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
        return;
    }
    let food = food.unwrap();
    if subcmd == SCMD_TASTE
        && (food.get_obj_type() == ITEM_DRINKCON || food.get_obj_type() == ITEM_FOUNTAIN)
    {
        do_drink(game, ch, argument, 0, SCMD_SIP);
        return;
    }
    if (food.get_obj_type() != ITEM_FOOD) && (ch.get_level() < LVL_GOD as u8) {
        send_to_char(ch, "You can't eat THAT!\r\n");
        return;
    }
    if ch.get_cond(FULL) > 20 {
        /* Stomach full */
        send_to_char(ch, "You are too full to eat more!\r\n");
        return;
    }
    if subcmd == SCMD_EAT {
        game.db.act("You eat $p.", false, Some(ch), Some(&food), None, TO_CHAR);
        game.db.act("$n eats $p.", true, Some(ch), Some(&food), None, TO_ROOM);
    } else {
        game.db.act(
            "You nibble a little bit of $p.",
            false,
            Some(ch),
            Some(&food),
            None,
            TO_CHAR,
        );
        game.db.act(
            "$n tastes a little bit of $p.",
            true,
            Some(ch),
            Some(&food),
            None,
            TO_ROOM,
        );
    }

    let amount = if subcmd == SCMD_EAT {
        food.get_obj_val(0)
    } else {
        1
    };

    game.db.gain_condition(ch, FULL, amount);

    if ch.get_cond(FULL) > 20 {
        send_to_char(ch, "You are full.\r\n");
    }

    if food.get_obj_val(3) != 0 && (ch.get_level() < LVL_IMMORT as u8) {
        /* The crap was poisoned ! */
        send_to_char(ch, "Oops, that tasted rather strange!\r\n");
        game.db.act(
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

        affect_join(ch, &mut af, false, false, false, false);
    }
    if subcmd == SCMD_EAT {
        game.db.extract_obj(&food);
    } else {
        if {
            food.decr_obj_val(0);
            food.get_obj_val(0) == 0
        } {
            send_to_char(ch, "There's nothing left now.\r\n");
            game.db.extract_obj(&food);
        }
    }
}

pub fn do_pour(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
    let mut arg1 = String::new();
    let mut arg2 = String::new();
    let mut from_obj = None;
    let mut to_obj = None;
    let mut amount;
    let db = &game.db;

    two_arguments(argument, &mut arg1, &mut arg2);

    if subcmd == SCMD_POUR {
        if arg1.is_empty() {
            /* No arguments */
            send_to_char(ch, "From what do you want to pour?\r\n");
            return;
        }
        if {
            from_obj = db.get_obj_in_list_vis(ch, &arg1, None, ch.carrying.borrow());
            from_obj.is_none()
        } {
            send_to_char(ch, "You can't find it!\r\n");
            return;
        }
        let from_obj = from_obj.as_ref().unwrap();
        if from_obj.get_obj_type() != ITEM_DRINKCON {
            send_to_char(ch, "You can't pour from that!\r\n");
            return;
        }
    }
    if subcmd == SCMD_FILL {
        if arg1.is_empty() {
            /* no arguments */
            send_to_char(
                ch,
                "What do you want to fill?  And what are you filling it from?\r\n",
            );
            return;
        }
        if {
            to_obj = db.get_obj_in_list_vis(ch, &arg1, None, ch.carrying.borrow());
            to_obj.is_none()
        } {
            send_to_char(ch, "You can't find it!\r\n");
            return;
        }
        let to_obj = to_obj.as_ref().unwrap();
        if to_obj.get_obj_type() != ITEM_DRINKCON {
            db.act(
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
            db.act(
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
            from_obj = db.get_obj_in_list_vis2(
                ch,
                &arg2,
                None,
                &db.world[ch.in_room() as usize].contents,
            );
            from_obj.is_none()
        } {
            send_to_char(
                ch,
                format!("There doesn't seem to be {} {} here.\r\n", an!(arg2), arg2).as_str(),
            );
            return;
        }
        let from_obj = from_obj.as_ref().unwrap();
        if from_obj.get_obj_type() != ITEM_FOUNTAIN {
            db.act(
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
    let from_obj = from_obj.as_ref().unwrap();

    if from_obj.get_obj_val(1) == 0 {
        db.act(
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
            send_to_char(ch, "Where do you want it?  Out or in what?\r\n");
            return;
        }
        if arg2 == "out" {
            db.act(
                "$n empties $p.",
                true,
                Some(ch),
                Some(from_obj),
                None,
                TO_ROOM,
            );
            db.act(
                "You empty $p.",
                false,
                Some(ch),
                Some(from_obj),
                None,
                TO_CHAR,
            );

            weight_change_object(game, from_obj, -from_obj.get_obj_val(1)); /* Empty */

            name_from_drinkcon(Some(from_obj));
            from_obj.set_obj_val(1, 0);
            from_obj.set_obj_val(2, 0);
            from_obj.set_obj_val(3, 0);

            return;
        }
        if {
            to_obj = db.get_obj_in_list_vis(ch, &arg2, None, ch.carrying.borrow());
            to_obj.is_none()
        } {
            send_to_char(ch, "You can't find it!\r\n");
            return;
        }
        let to_obj = to_obj.as_ref().unwrap();
        if (to_obj.get_obj_type() != ITEM_DRINKCON) && (to_obj.get_obj_type() != ITEM_FOUNTAIN) {
            send_to_char(ch, "You can't pour anything into that.\r\n");
            return;
        }
    }
    let to_obj = to_obj.as_ref().unwrap();

    if Rc::ptr_eq(to_obj, from_obj) {
        send_to_char(ch, "A most unproductive effort.\r\n");
        return;
    }
    if (to_obj.get_obj_val(1) != 0) && (to_obj.get_obj_val(2) != from_obj.get_obj_val(2)) {
        send_to_char(ch, "There is already another liquid in it!\r\n");
        return;
    }
    if !(to_obj.get_obj_val(1) < to_obj.get_obj_val(0)) {
        send_to_char(ch, "There is no room for more.\r\n");
        return;
    }
    if subcmd == SCMD_POUR {
        send_to_char(
            ch,
            format!(
                "You pour the {} into the {}.",
                DRINKS[from_obj.get_obj_val(2) as usize],
                arg2
            )
            .as_str(),
        );
    }

    if subcmd == SCMD_FILL {
        db.act(
            "You gently fill $p from $P.",
            false,
            Some(ch),
            Some(to_obj),
            Some(from_obj),
            TO_CHAR,
        );
        db.act(
            "$n gently fills $p from $P.",
            true,
            Some(ch),
            Some(to_obj),
            Some(from_obj),
            TO_ROOM,
        );
    }
    /* New alias */
    if to_obj.get_obj_val(1) == 0 {
        name_to_drinkcon(Some(to_obj), from_obj.get_obj_val(2));
    }
    /* First same type liq. */
    to_obj.set_obj_val(2, from_obj.get_obj_val(2));

    /* Then how much to pour */
    from_obj.set_obj_val(
        1,
        from_obj.get_obj_val(1) - {
            amount = to_obj.get_obj_val(0) - to_obj.get_obj_val(1);
            amount
        },
    );

    to_obj.set_obj_val(1, to_obj.get_obj_val(0));

    if from_obj.get_obj_val(1) < 0 {
        /* There was too little */
        to_obj.set_obj_val(1, to_obj.get_obj_val(1) + from_obj.get_obj_val(1));
        amount += from_obj.get_obj_val(1);
        name_from_drinkcon(Some(from_obj));
        from_obj.set_obj_val(1, 0);
        from_obj.set_obj_val(2, 0);
        from_obj.set_obj_val(3, 0);
    }
    /* Then the poison boogie */
    to_obj.set_obj_val(
        3,
        if to_obj.get_obj_val(3) != 0 || from_obj.get_obj_val(3) != 0 {
            1
        } else {
            0
        },
    );

    /* And the weight boogie */
    weight_change_object(game, from_obj, -amount);
    weight_change_object(game, to_obj, amount); /* Add weight */
}

fn wear_message(game: &mut Game, ch: &Rc<CharData>, obj: &Rc<ObjData>, _where: i32) {
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

    game.db.act(
        WEAR_MESSAGES[_where as usize][0],
        true,
        Some(ch),
        Some(obj),
        None,
        TO_ROOM,
    );
    game.db.act(
        WEAR_MESSAGES[_where as usize][1],
        false,
        Some(ch),
        Some(obj),
        None,
        TO_CHAR,
    );
}

fn perform_wear(game: &mut Game, ch: &Rc<CharData>, obj: &Rc<ObjData>, _where: i32) {
    /*
     * ITEM_WEAR_TAKE is used for objects that do not require special bits
     * to be put into that position (e.g. you can hold any object, not just
     * an object with a HOLD bit.)
     */
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
        game.db.act(
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
        send_to_char(ch, ALREADY_WEARING[_where as usize]);
        return;
    }
    wear_message(game, ch, obj, _where);
    obj_from_char(obj);
    game.db.equip_char(ch, obj, _where as i8);
}

pub fn find_eq_pos(ch: &Rc<CharData>, obj: &Rc<ObjData>, arg: &str) -> i16 {
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
        send_to_char(
            ch,
            format!("'{}'?  What part of your body is THAT?\r\n", arg).as_str(),
        );
    } else {
        _where = _where_o.unwrap() as i16;
    }

    _where
}

pub fn do_wear(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg1 = String::new();
    let mut arg2 = String::new();

    two_arguments(argument, &mut arg1, &mut arg2);

    if arg1.is_empty() {
        send_to_char(ch, "Wear what?\r\n");
        return;
    }
    let dotmode = find_all_dots(&arg1);

    if !arg2.is_empty() && dotmode != FIND_INDIV {
        send_to_char(
            ch,
            "You can't specify the same body location for more than one item!\r\n",
        );
        return;
    }
    let mut _where = -1;
    let mut items_worn = 0;
    if dotmode == FIND_ALL {
        for obj in clone_vec(&ch.carrying) {
            if game.db.can_see_obj(ch, &obj) && {
                _where = find_eq_pos(ch, &obj, "");
                _where >= 0
            } {
                items_worn += 1;
                perform_wear(game, ch, &obj, _where as i32);
            }
        }
        if items_worn == 0 {
            send_to_char(ch, "You don't seem to have anything wearable.\r\n");
        }
    } else if dotmode == FIND_ALLDOT {
        if arg1.is_empty() {
            send_to_char(ch, "Wear all of what?\r\n");
            return;
        }
        let mut obj;
        if {
            obj = game.db.get_obj_in_list_vis(ch, &arg1, None, ch.carrying.borrow());
            obj.is_none()
        } {
            send_to_char(
                ch,
                format!("You don't seem to have any {}s.\r\n", arg1).as_str(),
            );
        } else {
            while obj.is_some() {
                if {
                    _where = find_eq_pos(ch, obj.as_ref().unwrap(), "");
                    _where >= 0
                } {
                    perform_wear(game, ch, obj.as_ref().unwrap(), _where as i32);
                } else {
                    game.db.act(
                        "You can't wear $p.",
                        false,
                        Some(ch),
                        Some(obj.as_ref().unwrap()),
                        None,
                        TO_CHAR,
                    );
                }
                obj = game.db.get_obj_in_list_vis(ch, &arg1, None, ch.carrying.borrow());
            }
        }
    } else {
        let obj;
        if {
            obj = game.db.get_obj_in_list_vis(ch, &arg1, None, ch.carrying.borrow());
            obj.is_none()
        } {
            send_to_char(
                ch,
                format!("You don't seem to have {} {}.\r\n", an!(arg1), arg1).as_str(),
            );
        } else {
            if {
                _where = find_eq_pos(ch, obj.as_ref().unwrap(), &arg2);
                _where >= 0
            } {
                perform_wear(game, ch, obj.as_ref().unwrap(), _where as i32);
            } else if arg2.is_empty() {
                game.db.act(
                    "You can't wear $p.",
                    false,
                    Some(ch),
                    Some(obj.as_ref().unwrap()),
                    None,
                    TO_CHAR,
                );
            }
        }
    }
}

pub fn do_wield(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();

    let obj;
    let db = &game.db;
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(ch, "Wield what?\r\n");
    } else if {
        obj = db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
        obj.is_none()
    } {
        send_to_char(
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
    } else {
        let obj = obj.as_ref().unwrap();
        if !obj.can_wear(ITEM_WEAR_WIELD) {
            send_to_char(ch, "You can't wield that.\r\n");
        } else if obj.get_obj_weight() > STR_APP[ch.strength_apply_index()].wield_w as i32 {
            send_to_char(ch, "It's too heavy for you to use.\r\n");
        } else {
            perform_wear(game, ch, obj, WEAR_WIELD as i32);
        }
    }
}

pub fn do_grab(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    let obj;
    let db = &game.db;
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(ch, "Hold what?\r\n");
    } else if {
        obj = db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
        obj.is_none()
    } {
        send_to_char(
            ch,
            format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
        );
    } else {
        let obj = obj.as_ref().unwrap();

        if obj.get_obj_type() == ITEM_LIGHT {
            perform_wear(game, ch, obj, WEAR_LIGHT as i32);
        } else {
            if !obj.can_wear(ITEM_WEAR_HOLD)
                && obj.get_obj_type() != ITEM_WAND
                && obj.get_obj_type() != ITEM_STAFF
                && obj.get_obj_type() != ITEM_SCROLL
                && obj.get_obj_type() != ITEM_POTION
            {
                send_to_char(ch, "You can't hold that.\r\n");
            } else {
                perform_wear(game, ch, obj, WEAR_HOLD as i32);
            }
        }
    }
}

fn perform_remove(game: &mut Game, ch: &Rc<CharData>, pos: i8) {
    let obj;

    if {
        obj = ch.get_eq(pos as i8);
        obj.is_none()
    } {
        error!("SYSERR: perform_remove: bad pos {} passed.", pos);
    } else if obj.as_ref().unwrap().obj_flagged(ITEM_NODROP) {
        game.db.act(
            "You can't remove $p, it must be CURSED!",
            false,
            Some(ch),
            Some(obj.as_ref().unwrap()),
            None,
            TO_CHAR,
        );
    } else if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
        game.db.act(
            "$p: you can't carry that many items!",
            false,
            Some(ch),
            Some(obj.as_ref().unwrap()),
            None,
            TO_CHAR,
        );
    } else {
        let obj = obj.as_ref().unwrap();
        DB::obj_to_char(game.db.unequip_char(ch, pos).as_ref().unwrap(), ch);
        game.db.act(
            "You stop using $p.",
            false,
            Some(ch),
            Some(obj),
            None,
            TO_CHAR,
        );
        game.db.act(
            "$n stops using $p.",
            true,
            Some(ch),
            Some(obj),
            None,
            TO_ROOM,
        );
    }
}

pub fn do_remove(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if arg.is_empty() {
        send_to_char(ch, "Remove what?\r\n");
        return;
    }
    let dotmode = find_all_dots(&arg);

    let mut found = false;
    let i;
    if dotmode == FIND_ALL {
        for i in 0..NUM_WEARS {
            if ch.get_eq(i).is_some() {
                perform_remove(game, ch, i);
                found = true;
            }
        }
        if !found {
            send_to_char(ch, "You're not using anything.\r\n");
        }
    } else if dotmode == FIND_ALLDOT {
        if arg.is_empty() {
            send_to_char(ch, "Remove all of what?\r\n");
        } else {
            found = false;
            for i in 0..NUM_WEARS {
                if ch.get_eq(i).is_some()
                    && game.db.can_see_obj(ch, ch.get_eq(i).as_ref().unwrap())
                    && isname(&arg, &ch.get_eq(i).as_ref().unwrap().name.borrow())
                {
                    perform_remove(game, ch, i);
                    found = true;
                }
            }
            if !found {
                send_to_char(
                    ch,
                    format!("You don't seem to be using any {}s.\r\n", arg).as_str(),
                );
            }
        }
    } else {
        if {
            i = game.db.get_obj_pos_in_equip_vis(ch, &arg, None, &ch.equipment);
            i.is_none()
        } {
            send_to_char(
                ch,
                format!("You don't seem to be using {} {}.\r\n", an!(arg), arg).as_str(),
            );
        } else {
            perform_remove(game, ch, i.unwrap());
        }
    }
}
