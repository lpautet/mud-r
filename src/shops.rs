/* ************************************************************************
*   File: shop.rs                                       Part of CircleMUD *
*  Usage: shop file definitions, structures, constants                    *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use std::cmp::min;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::error;
use regex::Regex;
use crate::depot::{Depot, DepotId, HasId};
use crate::{act, send_to_char, TextData, VictimRef};

use crate::act_comm::{do_say, do_tell};
use crate::act_social::do_action;
use crate::act_wizard::do_echo;
use crate::constants::{DRINKS, EXTRA_BITS, ITEM_TYPES};
use crate::db::{fread_string, DB, REAL};
use crate::handler::{fname, get_number, get_obj_in_list_num, get_obj_in_list_vis, isname, obj_from_char, obj_to_char};
use crate::interpreter::{cmd_is, find_command, is_number, one_argument, SCMD_EMOTE};
use crate::modify::page_string;
use crate::structs::{
    MeRef, CharData, MobRnum, MobVnum, ObjData, ObjVnum, RoomRnum, RoomVnum, Special, AFF_CHARM,
    ITEM_DRINKCON, ITEM_NOSELL, ITEM_STAFF, ITEM_WAND, LVL_GOD, MAX_OBJ_AFFECT, NOBODY, NOTHING,
    NOWHERE,
};
use crate::util::{can_see, can_see_obj, get_line, sprintbit};
use crate::{an, is_set,  yesno, Game, PAGE_LENGTH, TO_CHAR, TO_ROOM};

pub struct ShopBuyData {
    pub type_: i32,
    pub keywords: Rc<str>,
}

impl ShopBuyData {
    pub fn buy_type(&self) -> i32 {
        self.type_
    }
}

pub struct ShopData {
    pub vnum: RoomVnum,
    /* Virtual number of this shop		*/
    pub producing: Vec<ObjVnum>,
    /* Which item to produce (virtual)	*/
    pub profit_buy: f32,
    /* Factor to multiply cost with		*/
    pub profit_sell: f32,
    /* Factor to multiply cost with		*/
    pub type_: Vec<ShopBuyData>,
    /* Which items to trade			*/
    pub no_such_item1: Rc<str>,
    /* Message if keeper hasn't got an item	*/
    pub no_such_item2: Rc<str>,
    /* Message if player hasn't got an item	*/
    pub missing_cash1: Rc<str>,
    /* Message if keeper hasn't got cash	*/
    pub missing_cash2: Rc<str>,
    /* Message if player hasn't got cash	*/
    pub do_not_buy: Rc<str>,
    /* If keeper dosn't buy such things	*/
    pub message_buy: Rc<str>,
    /* Message when player buys item	*/
    pub message_sell: Rc<str>,
    /* Message when player sells item	*/
    pub temper1: i32,
    /* How does keeper react if no money	*/
    pub bitvector: i32,
    /* Can attack? Use bank? Cast here?	*/
    pub keeper: MobRnum,
    /* The mobile who owns the shop (rnum)	*/
    pub with_who: i32,
    /* Who does the shop trade with?	*/
    pub in_room: Vec<RoomVnum>,
    /* Where is the shop?			*/
    pub open1: i32,
    pub open2: i32,
    /* When does the shop open?		*/
    pub close1: i32,
    pub close2: i32,
    /* When does the shop close?		*/
    pub bank_account: i32,
    /* Store all gold over 15000 (disabled)	*/
    pub lastsort: i32,
    /* How many items are sorted in inven?	*/
    pub func: Option<Special>, /* Secondary spec_proc for shopkeeper	*/
}

const MAX_TRADE: i32 = 5; /* List maximums for compatibility	*/
const MAX_PROD: i32 = 5; /*	with shops before v3.0		*/
const VERSION3_TAG: &str = "v3.0"; /* The file has v3.0 shops in it!	*/
// const MAX_SHOP_OBJ: i32 = 100; /* "Soft" maximum for list maximums	*/
/* Possible states for objects trying to be sold */
const OBJECT_DEAD: i32 = 0;
const OBJECT_NOTOK: i32 = 1;
const OBJECT_OK: i32 = 2;
const OBJECT_NOVAL: i32 = 3;

impl CharData {
    pub fn is_god(&self) -> bool {
        !self.is_npc() && self.get_level() >= LVL_GOD as u8
    }
}

/* Types of lists to read */
pub const LIST_PRODUCE: i32 = 0;
pub const LIST_TRADE: i32 = 1;
pub const LIST_ROOM: i32 = 2;

/* Whom will we not trade with (bitvector for SHOP_TRADE_WITH()) */
pub const TRADE_NOGOOD: i32 = 1 << 0;
pub const TRADE_NOEVIL: i32 = 1 << 1;
pub const TRADE_NONEUTRAL: i32 = 1 << 2;
pub const TRADE_NOMAGIC_USER: i32 = 1 << 3;
pub const TRADE_NOCLERIC: i32 = 1 << 4;
pub const TRADE_NOTHIEF: i32 = 1 << 5;
pub const TRADE_NOWARRIOR: i32 = 1 << 6;

struct StackData {
    data: [i32; 100],
    len: usize,
}

impl StackData {
    fn new() -> StackData {
        StackData {
            data: [0; 100],
            len: 0,
        }
    }
}

/* Which expression type we are now parsing */
const OPER_OPEN_PAREN: i32 = 0;
const OPER_CLOSE_PAREN: i32 = 1;
const OPER_OR: i32 = 2;
const OPER_AND: i32 = 3;
const OPER_NOT: i32 = 4;
// const MAX_OPER: i32 = 4;

impl ShopData {
    fn shop_product(&self, num: i32) -> ObjVnum {
        self.producing[num as usize]
    }
}

impl ShopData {
    fn notrade_good(&self) -> bool {
        is_set!(self.with_who, TRADE_NOGOOD)
    }
    fn notrade_evil(&self) -> bool {
        is_set!(self.with_who, TRADE_NOEVIL)
    }
    fn notrade_neutral(&self) -> bool {
        is_set!(self.with_who, TRADE_NONEUTRAL)
    }
    fn notrade_magic_user(&self) -> bool {
        is_set!(self.with_who, TRADE_NOMAGIC_USER)
    }
    fn notrade_cleric(&self) -> bool {
        is_set!(self.with_who, TRADE_NOCLERIC)
    }
    fn notrade_thief(&self) -> bool {
        is_set!(self.with_who, TRADE_NOTHIEF)
    }
    fn notrade_warrior(&self) -> bool {
        is_set!(self.with_who, TRADE_NOWARRIOR)
    }
}

pub const WILL_START_FIGHT: i32 = 1 << 0;
pub const WILL_BANK_MONEY: i32 = 1 << 1;

impl ShopData {
    fn shop_kill_chars(&self) -> bool {
        is_set!(self.bitvector, WILL_START_FIGHT)
    }
    fn shop_uses_bank(&self) -> bool {
        is_set!(self.bitvector, WILL_BANK_MONEY)
    }
}

pub const MIN_OUTSIDE_BANK: i32 = 5000;
pub const MAX_OUTSIDE_BANK: i32 = 15000;

pub const MSG_NOT_OPEN_YET: &str = "Come back later!";
pub const MSG_NOT_REOPEN_YET: &str = "Sorry, we have closed, but come back later.";
pub const MSG_CLOSED_FOR_DAY: &str = "Sorry, come back tomorrow.";
pub const MSG_NO_STEAL_HERE: &str = "$n is a bloody thief!";
pub const MSG_NO_SEE_CHAR: &str = "I don't trade with someone I can't see!";
pub const MSG_NO_SELL_ALIGN: &str = "Get out of here before I call the guards!";
pub const MSG_NO_SELL_CLASS: &str = "We don't serve your kind here!";
pub const MSG_NO_USED_WANDSTAFF: &str = "I don't buy used up wands or staves!";
pub const MSG_CANT_KILL_KEEPER: &str = "Get out of here before I call the guards!";

/***
 * The entire shop rewrite for Circle 3.0 was done by Jeff Fink.  Thanks Jeff!
 ***/

/* config arrays */
const OPERATOR_STR: [&str; 5] = ["[({", "])}", "|+", "&*", "^'"];

/* Constant list for printing out who we sell to */
const TRADE_LETTERS: [&str; 8] = [
    "Good", /* First, the alignment based ones */
    "Evil",
    "Neutral",
    "Magic User", /* Then the class based ones */
    "Cleric",
    "Thief",
    "Warrior",
    "\n",
];

const SHOP_BITS: [&str; 3] = ["WILL_FIGHT", "USES_BANK", "\n"];

fn is_ok_char(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, keeper_id:DepotId, chid: DepotId, shop_nr: usize) -> bool {
    let ch = chars.get(chid);
    let keeper = chars.get(keeper_id);
    // char buf[MAX_INPUT_LENGTH];
    if !can_see(&game.descriptors, chars, db,keeper, ch) {
        do_say(
            game, db,chars,texts,objs,
            keeper_id,
            MSG_NO_SEE_CHAR,
            CMD_SAY.load(Ordering::Relaxed),
            0,
        );
        return false;
    }
    if ch.is_god() {
        return true;
    }

    if ch.is_good() && db.shop_index[shop_nr].notrade_good()
        || ch.is_evil() && db.shop_index[shop_nr].notrade_evil()
        || ch.is_neutral() && db.shop_index[shop_nr].notrade_neutral()
    {
        let buf = format!("{} {}", ch.get_name(), MSG_NO_SELL_ALIGN);
        do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return false;
    }
    if ch.is_npc() {
        return true;
    }

    if ch.is_magic_user() && db.shop_index[shop_nr].notrade_magic_user()
        || ch.is_cleric() && db.shop_index[shop_nr].notrade_cleric()
        || ch.is_thief() && db.shop_index[shop_nr].notrade_thief()
        || ch.is_warrior() && db.shop_index[shop_nr].notrade_warrior()
    {
        let buf = format!("{} {}", ch.get_name(), MSG_NO_SELL_CLASS);
        do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return false;
    }
    true
}

fn is_open(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>,  keeper_id: DepotId, shop_nr: usize, msg: bool) -> bool {
    let mut buf = String::new();
    if db.shop_index[shop_nr].open1 > db.time_info.hours {
        buf.push_str(MSG_NOT_OPEN_YET);
    } else if db.shop_index[shop_nr].close1 < db.time_info.hours {
        if db.shop_index[shop_nr].open2 > db.time_info.hours {
            buf.push_str(MSG_NOT_REOPEN_YET);
        } else if db.shop_index[shop_nr].close2 < db.time_info.hours {
            buf.push_str(MSG_CLOSED_FOR_DAY);
        }
    }
    if buf.is_empty() {
        return true;
    }

    if msg {
        do_say(game, db,chars, texts, objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
    }
    false
}

fn is_ok(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>,  keeper_id: DepotId, chid: DepotId, shop_nr: usize) -> bool {
    if is_open(game, chars, db, texts,objs,keeper_id, shop_nr, true) {
        return is_ok_char(game, chars, db, texts,objs,keeper_id, chid, shop_nr);
    }
    false
}

fn push(stack: &mut StackData, pushval: i32) {
    stack.data[stack.len] = pushval;
    stack.len += 1;
}

fn top(stack: &StackData) -> i32 {
    return if stack.len != 0 {
        stack.data[stack.len - 1]
    } else {
        NOTHING as i32
    };
}

fn pop(stack: &mut StackData) -> i32 {
    return if stack.len > 0 {
        stack.len -= 1;
        stack.data[stack.len]
    } else {
        error!(
            "SYSERR: Illegal expression {} in shop keyword list.",
            stack.len
        );
        0
    };
}

fn evaluate_operation(ops: &mut StackData, vals: &mut StackData) {
    let oper = pop(ops);

    if oper == OPER_NOT {
        let v = pop(vals);
        push(vals, if v == 0 { 1 } else { 0 });
    } else {
        let val1 = pop(vals);
        let val2 = pop(vals);

        /* Compiler would previously short-circuit these. */
        if oper == OPER_AND {
            push(vals, if (val1 != 0) && (val2 != 0) { 1 } else { 0 });
        } else if oper == OPER_OR {
            push(vals, if (val1 != 0) || (val2 != 0) { 1 } else { 0 });
        }
    }
}

fn find_oper_num(token: &str) -> Option<usize> {
    OPERATOR_STR.iter().position(|o| o.contains(token))
}

fn evaluate_expression(obj: &ObjData, expr: &str) -> i32 {
    let mut ops = StackData::new();
    let mut vals = StackData::new();

    if expr.is_empty() {
        /* Allows opening ( first. */
        return 1;
    }

    ops.len = 0;
    vals.len = 0;
    let mut ptr = 0;
    let mut end;
    while !expr[ptr..].is_empty() {
        if &expr[ptr..ptr] == " " {
            ptr += 1;
        } else {
            let temp = find_oper_num(&expr[ptr..ptr]);
            if temp.is_none() {
                end = ptr;
                while ptr < expr.len()
                    && &expr[ptr..ptr] != " "
                    && find_oper_num(&expr[ptr..ptr]).is_none()
                {
                    ptr += 1;
                }
                let name = &expr[end..ptr];

                let mut findex = 0;
                for eindex in 0..EXTRA_BITS.len() {
                    if name == EXTRA_BITS[eindex] {
                        push(&mut vals, if obj.obj_flagged(1 << eindex) { 1 } else { 0 });
                        findex = eindex;
                        break;
                    }
                }
                if EXTRA_BITS[findex] == "\n" {
                    push(
                        &mut vals,
                        if isname(name, obj.name.as_ref()) {
                            1
                        } else {
                            0
                        },
                    );
                }
            } else {
                let temp = temp.unwrap() as i32;
                if temp != OPER_OPEN_PAREN {
                    while top(&ops) > temp {
                        evaluate_operation(&mut ops, &mut vals);
                    }
                }

                if temp == OPER_CLOSE_PAREN {
                    let temp = pop(&mut ops);
                    if temp != OPER_OPEN_PAREN {
                        error!("SYSERR: Illegal parenthesis in shop keyword expression.");
                        return 0;
                    }
                } else {
                    push(&mut ops, temp);
                }
                ptr += 1;
            }
        }
    }
    while top(&ops) != NOTHING as i32 {
        evaluate_operation(&mut ops, &mut vals);
    }
    let temp = pop(&mut vals);
    if top(&vals) != NOTHING as i32 {
        error!("SYSERR: Extra operands left on shop keyword expression stack.");
        return 0;
    }
    return temp;
}

fn trade_with(item: &ObjData, shop: &ShopData) -> i32 {
    if item.get_obj_cost() < 1 {
        return OBJECT_NOVAL;
    }

    if item.obj_flagged(ITEM_NOSELL) {
        return OBJECT_NOTOK;
    }

    let mut counter = 0usize;

    while shop.type_[counter].type_ != NOTHING as i32 {
        if shop.type_[counter].type_ == item.get_obj_type() as i32 {
            if item.get_obj_val(2) == 0
                && (item.get_obj_type() == ITEM_WAND || item.get_obj_type() == ITEM_STAFF)
            {
                return OBJECT_DEAD;
            }
        } else if evaluate_expression(item, &shop.type_[counter].keywords) != 0 {
            return OBJECT_OK;
        }
        counter += 1;
    }
    return OBJECT_NOTOK;
}

fn same_obj(obj1: &ObjData, obj2: &ObjData) -> bool {
    if obj1.get_obj_rnum() != obj2.get_obj_rnum() {
        return false;
    }

    if obj1.get_obj_cost() != obj2.get_obj_cost() {
        return false;
    }

    if obj1.get_obj_extra() != obj2.get_obj_extra() {
        return false;
    }

    for aindex in 0..MAX_OBJ_AFFECT as usize {
        if obj1.affected[aindex].location != obj2.affected[aindex].location
            || obj1.affected[aindex].modifier != obj2.affected[aindex].modifier
        {
            return false;
        }
    }
    true
}

fn shop_producing(db: &DB, objs: & Depot<ObjData>, item_id: DepotId, shop_nr: usize) -> bool {
    if objs.get(item_id).get_obj_rnum() == NOTHING {
        return false;
    }
    for counter in 0..db.shop_index[shop_nr].producing.len() {
        if db.shop_index[shop_nr].producing[counter] == NOTHING {
            break;
        }
        if same_obj(
            objs.get(item_id),
            &db.obj_proto[db.shop_index[shop_nr].producing[counter] as usize],
        ) {
            return true;
        }
    }
    false
}

fn transaction_amt(arg: &mut String) -> i32 {
    /*
     * If we have two arguments, it means 'buy 5 3', or buy 5 of #3.
     * We don't do that if we only have one argument, like 'buy 5', buy #5.
     * Code from Andrey Fidrya <andrey@ALEX-UA.COM>
     */
    let mut buf = String::new();
    let buywhat = one_argument(arg, &mut buf);
    if !buywhat.is_empty() && !buf.is_empty() && is_number(&buf) {
        arg.truncate(buf.len() + 1);
        return buf.parse::<i32>().unwrap();
    }
    1
}

fn times_message(obj: Option<&ObjData>, name: &str, num: i32) -> String {
    let mut buf = String::new();
    if obj.is_some() {
        buf.push_str(obj.unwrap().short_description.as_ref());
    } else {
        let pos = name.find('.');
        let ptr;
        if pos.is_none() {
            ptr = name;
        } else {
            ptr = &name[1..];
        }
        buf.push_str(format!("({} {}", an!(ptr), ptr).as_str());
    }

    if num > 1 {
        buf.push_str(format!(" (x {})", num).as_str());
    }
    buf
}

fn get_slide_obj_vis(
    game: &Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    name: &str,
    list: &Vec<DepotId>,
) -> Option<DepotId> {
    let ch = chars.get(chid);
    let mut tmpname = name.to_string();
    let number;
    let mut last_match= None;
    if {
        number = get_number(&mut tmpname);
        number == 0
    } {
        return None;
    }
    let mut j = 1;
    for i in list {
        if j > number {
            break;
        }
        if isname(&tmpname, objs.get(*i).name.as_ref()) {
            if can_see_obj(&game.descriptors, chars, db,ch, objs.get(*i))
                && (last_match.is_none() || !same_obj(objs.get(last_match.unwrap()), objs.get(*i)))
            {
                if j == number {
                    return Some(*i);
                }
                last_match = Some(*i);
                j += 1;
            }
        }
    }
    None
}

fn get_hash_obj_vis(
    game: &Game, chars: &Depot<CharData>, db: &DB,objs: & Depot<ObjData>, 
    chid: DepotId,
    name: &str,
    list: &Vec<DepotId>,
) -> Option<DepotId> {
    let ch = chars.get(chid);
    let mut qindex;
    if is_number(name) {
        qindex = name.parse::<i32>().unwrap();
    } else if is_number(&name[1..]) {
        qindex = name[1..].parse::<i32>().unwrap();
    } else {
        return None;
    }
    let mut last_obj: Option<DepotId> = None;
    for l in list.iter() {
        if can_see_obj(&game.descriptors, chars, db,ch, objs.get(*l)) && objs.get(*l).get_obj_cost() > 0 {
            if last_obj.is_some() && !same_obj(objs.get(last_obj.unwrap()), objs.get(*l)) {
                if {
                    qindex -= 1;
                    qindex == 0
                } {
                    return Some(*l);
                }
                last_obj = Some(*l);
            }
        }
    }
    None
}

fn get_purchase_obj(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    arg: &str,
    keeper_id: DepotId,
    shop_nr: usize,
    msg: bool,
) -> Option<DepotId> {
    let mut name = String::new();
    one_argument(arg, &mut name);
    let mut oid: Option<DepotId>;
    loop {
        let keeper = chars.get(keeper_id);
        if name.starts_with('#') || is_number(&name) {
            oid = get_hash_obj_vis(game, chars, db,objs, chid, &name, &keeper.carrying);
        } else {
            oid = get_slide_obj_vis(game, chars, db,objs, chid, &name, &keeper.carrying.clone());
        }
        if oid.is_none() {
            if msg {
                let ch = chars.get(chid);
                let buf = db.shop_index[shop_nr]
                    .no_such_item1
                    .replace("%s", &ch.get_name());
                do_tell(game, db,chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
            }
            return None;
        }
        if objs.get(oid.unwrap()).get_obj_cost() <= 0 {
            db.extract_obj( chars, objs, oid.unwrap());
            oid = None;
        }
        if oid.is_some() {
            break;
        }
    }
    oid
}

/*
 * Shop purchase adjustment, based on charisma-difference from buyer to keeper.
 *
 * for i in `seq 15 -15`; do printf " * %3d: %6.4f\n" $i \
 * `echo "scale=4; 1+$i/70" | bc`; done
 *
 *  Shopkeeper higher charisma (markup)
 *  ^  15: 1.2142  14: 1.2000  13: 1.1857  12: 1.1714  11: 1.1571
 *  |  10: 1.1428   9: 1.1285   8: 1.1142   7: 1.1000   6: 1.0857
 *  |   5: 1.0714   4: 1.0571   3: 1.0428   2: 1.0285   1: 1.0142
 *  +   0: 1.0000
 *  |  -1: 0.9858  -2: 0.9715  -3: 0.9572  -4: 0.9429  -5: 0.9286
 *  |  -6: 0.9143  -7: 0.9000  -8: 0.8858  -9: 0.8715 -10: 0.8572
 *  v -11: 0.8429 -12: 0.8286 -13: 0.8143 -14: 0.8000 -15: 0.7858
 *  Player higher charisma (discount)
 *
 * Most mobiles have 11 charisma so an 18 charisma player would get a 10%
 * discount beyond the basic price.  That assumes they put a lot of points
 * into charisma, because on the flip side they'd get 11% inflation by
 * having a 3.
 */
fn buy_price(chars: &Depot<CharData>,
    db: &DB,objs: & Depot<ObjData>, 
    oid: DepotId,
    shop_nr: usize,
    keeper_id: DepotId,
    buyer_id: DepotId,
) -> i32 {
    let keeper = chars.get(keeper_id);
    let buyer = chars.get(buyer_id);
        return (objs.get(oid).get_obj_cost() as f32
        * db.shop_index[shop_nr].profit_buy
        * (1f32 + keeper.get_cha() as f32 - buyer.get_cha() as f32)
        / 70f32) as i32;
}

/*
 * When the shopkeeper is buying, we reverse the discount. Also make sure
 * we don't buy for more than we sell for, to prevent infinite money-making.
 */
fn sell_price(
    obj: &ObjData,
    shop: &ShopData,
    keeper: &CharData,
    seller: &CharData,
) -> i32 {
    let mut sell_cost_modifier =
        shop.profit_sell * (1f32 - (keeper.get_cha() - seller.get_cha()) as f32 / 70.0);
    let buy_cost_modifier =
        shop.profit_buy * (1f32 + (keeper.get_cha() - seller.get_cha()) as f32 / 70.0);

    if sell_cost_modifier > buy_cost_modifier {
        sell_cost_modifier = buy_cost_modifier;
    }

    (obj.get_obj_cost() as f32 * sell_cost_modifier) as i32
}

fn shopping_buy(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    arg: &str,
    chid:DepotId,
    keeper_id: DepotId,
    shop_nr: usize,
) {
    if !is_ok(game, chars, db, texts, objs,keeper_id, chid, shop_nr) {
        return;
    }
    let keeper = chars.get(keeper_id);
    if db.shop_index[shop_nr].lastsort < keeper.is_carrying_n() as i32 {
        sort_keeper_objs(game, chars, db,objs, keeper_id, shop_nr);
    }
    let ch = chars.get(chid);
    let mut arg = arg.to_string();
    let buynum;
    if {
        buynum = transaction_amt(&mut arg);
        buynum < 0
    } {
        let buf = format!(
            "{}s A negative amount?  Try selling me something.",
            ch.get_name()
        );
        do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    if arg.is_empty() || buynum == 0 {
        let buf = format!("{} What do you want to buy??", ch.get_name());
        do_tell(game, db,chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    let mut oid: Option<DepotId>;
    if {
        oid = get_purchase_obj(game, chars, db, texts,objs,chid, &arg, keeper_id, shop_nr, true);
        oid.is_none()
    } {
        return;
    }
    let ch = chars.get(chid);
    if buy_price(chars, &db, objs,oid.unwrap(), shop_nr, keeper_id, chid) > ch.get_gold()
        && !ch.is_god()
    {
        let actbuf = db.shop_index[shop_nr]
            .missing_cash2
            .replace("%s", &ch.get_name());
        do_tell(game, db, chars, texts,objs,keeper_id, &actbuf, CMD_TELL.load(Ordering::Relaxed), 0);

        let temper1 = db.shop_index[shop_nr].temper1;
        let ch = chars.get(chid);
        match temper1 {
            0 => {
                do_action(
                    game,db,chars, texts,objs,
                    keeper_id,
                    &ch.get_name().clone(),
                    CMD_PUKE.load(Ordering::Relaxed),
                    0,
                );
            }

            1 => {
                do_echo(
                    game,db,chars, texts,objs,
                    keeper_id,
                    "smokes on his joint.",
                    CMD_EMOTE.load(Ordering::Relaxed),
                    SCMD_EMOTE,
                );
                return;
            }
            _ => {
                return;
            }
        }
    }
    let ch = chars.get(chid);
    if ch.is_carrying_n() + 1 > ch.can_carry_n() as u8 {
        send_to_char(&mut game.descriptors, ch,
            format!(
                "{}: You can't carry any more items.\r\n",
                fname(objs.get(oid.unwrap()).name.as_ref())
            )
            .as_str(),
        );
        return;
    }
    if ch.is_carrying_w() + objs.get(oid.unwrap()).get_obj_weight() > ch.can_carry_w() as i32 {
        send_to_char(&mut game.descriptors, ch,
            format!(
                "{}: You can't carry that much weight.\r\n",
                fname(objs.get(oid.unwrap()).name.as_ref())
            )
            .as_str(),
        );
        return;
    }
    let mut bought = 0;
    let mut goldamt = 0;
    let mut last_oid: Option<DepotId> = None;
    while { let ch = chars.get(chid); oid.is_some()
        && (ch.get_gold() >= buy_price(chars, &db, objs,oid.unwrap(), shop_nr, keeper_id, chid)
            || ch.is_god())
        && ch.is_carrying_n() < ch.can_carry_n() as u8
        && bought < buynum
        && ch.is_carrying_w() + objs.get(oid.unwrap()).get_obj_weight() <= ch.can_carry_w() as i32 }
    {
        bought += 1;

        /* Test if producing shop ! */
        if shop_producing(&db,objs, oid.unwrap(), shop_nr) {
            oid = 
                db
                .read_object(objs, objs.get(oid.unwrap()).get_obj_rnum(), REAL);
        } else {
            obj_from_char(chars, objs.get_mut(oid.unwrap()));
            db.shop_index[shop_nr].lastsort -= 1;
        }
        obj_to_char(objs.get_mut( oid.unwrap()), chars.get_mut(chid));

        let charged = buy_price(chars, &db, objs,oid.unwrap(), shop_nr, keeper_id, chid);
        goldamt += charged;
        let ch = chars.get(chid);
        if !ch.is_god() {
            let ch = chars.get_mut(chid);
            ch.set_gold(ch.get_gold() - charged);
        }

        last_oid = Some(oid.unwrap());
        oid = get_purchase_obj(game, chars, db, texts,objs,chid, &arg, keeper_id, shop_nr, false);
        if oid.is_some() && !same_obj(objs.get(oid.unwrap()), objs.get(last_oid.unwrap())) {
            break;
        }
    }
    let buf;
    if bought < buynum {
        let ch = chars.get(chid);
        if oid.is_none() || !same_obj(objs.get(last_oid.unwrap()), objs.get(oid.unwrap())) {
            buf = format!("{} I only have {} to sell you.", ch.get_name(), bought);
        } else if ch.get_gold() < buy_price(chars, &db, objs, oid.unwrap(), shop_nr, keeper_id, chid) {
            buf = format!("{} You can only afford {}.", ch.get_name(), bought);
        } else if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
            buf = format!("{} You can only hold {}.", ch.get_name(), bought);
        } else if ch.is_carrying_w() + objs.get(oid.unwrap()).get_obj_weight()
            > ch.can_carry_w() as i32
        {
            buf = format!("{} You can only carry {}.", ch.get_name(), bought);
        } else {
            buf = format!(
                "{} Something screwy only gave you {}.",
                ch.get_name(),
                bought,
            );
        }
        do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
    }
    let ch = chars.get(chid);
    if !ch.is_god() {
        let keeper = chars.get_mut(keeper_id);
        keeper.set_gold(keeper.get_gold() + goldamt);
    }
    let ch = chars.get(chid);
    let tempstr = times_message(Some(objs.get(ch.carrying[0])), "", bought);

    let tempbuf = format!("$n buys {}.", tempstr);
    let obj = oid.map(|id| objs.get(id));
    act(&mut game.descriptors, chars, db,
        &tempbuf,
        false,
        Some(ch),
        obj,
        None,
        TO_ROOM,
    );
    let ch = chars.get(chid);
    let tmpbuf = db.shop_index[0]
        .message_buy
        .replace("%s", &ch.get_name())
        .replace("%d", &goldamt.to_string());
    do_tell(game, db, chars, texts,objs,keeper_id, &tmpbuf, CMD_TELL.load(Ordering::Relaxed), 0);
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch, format!("You now have {}.\r\n", tempstr).as_str());

    if db.shop_index[shop_nr].shop_uses_bank() {
        let keeper = chars.get(keeper_id);
        if keeper.get_gold() > MAX_OUTSIDE_BANK {
            db.shop_index[shop_nr].bank_account += keeper.get_gold() - MAX_OUTSIDE_BANK;
            let keeper = chars.get_mut(keeper_id);
            keeper.set_gold(MAX_OUTSIDE_BANK);
        }
    }
}

fn get_selling_obj(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    name: &str,
    keeper_id: DepotId,
    shop_nr: usize,
    msg: i32,
) -> Option<DepotId> {
    let ch = chars.get(chid);
    let obj = get_obj_in_list_vis(&game.descriptors, chars,db,objs,ch, name, None, &ch.carrying);
    if obj.is_none() {
        if msg != 0 {
            let tbuf = db.shop_index[0].no_such_item2.replace("%s", &ch.get_name());

            do_tell(game, db, chars, texts,objs,keeper_id, &tbuf, CMD_TELL.load(Ordering::Relaxed), 0);
        }
        return None;
    }
    let obj = obj.unwrap();
    let result = trade_with(obj,  &db.shop_index[shop_nr]);
    if result == OBJECT_OK {
        return Some(obj.id());
    }

    if msg == 0 {
        return None;
    }
    let buf;
    match result {
        OBJECT_NOVAL => {
            buf = format!(
                "{} You've got to be kidding, that thing is worthless!",
                ch.get_name()
            );
        }
        OBJECT_NOTOK => {
            buf = db.shop_index[shop_nr]
                .do_not_buy
                .replace("%s", &ch.get_name());
        }
        OBJECT_DEAD => {
            buf = format!("{} {}", ch.get_name(), MSG_NO_USED_WANDSTAFF);
        }
        _ => {
            error!(
                "SYSERR: Illegal return value of {} from trade_with()",
                result
            ); /* Someone might rename it... */
            buf = format!("{} An error has occurred.", ch.get_name());
        }
    }
    do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
    None
}

fn slide_obj(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>,  oid: DepotId, keeper_id: DepotId, shop_nr: usize) {
    let keeper = chars.get(keeper_id);
    /*
       This function is a slight hack!  To make sure that duplicate items are
       only listed once on the "list", this function groups "identical"
       objects together on the shopkeeper's inventory list.  The hack involves
       knowing how the list is put together, and manipulating the order of
       the objects on the list.  (But since most of DIKU is not encapsulated,
       and information hiding is almost never used, it isn't that big a deal) -JF
    */

    if db.shop_index[shop_nr].lastsort < keeper.is_carrying_n() as i32 {
        sort_keeper_objs(game, chars, db, objs,keeper_id, shop_nr);
    }
    /* Extract the object if it is identical to one produced */
    if shop_producing(db, objs,oid, shop_nr) {
        db.extract_obj(chars, objs, oid);
        return;
    }
    db.shop_index[shop_nr].lastsort += 1;
    let keeper = chars.get_mut(keeper_id);
    let obj = objs.get_mut(oid);
    obj_to_char(obj, keeper);
    let len = keeper.carrying.len();
    let obj_id = keeper.carrying.remove(len - 1);
    let mut idx: Option<usize> = None;
    let keeper = chars.get(keeper_id);
    for i in 0..keeper.carrying.len() {
        if same_obj(objs.get(keeper.carrying[i]), objs.get(obj_id)) {
            idx = Some(i);
        }
    }
    let keeper = chars.get_mut(keeper_id);
    if idx.is_some() {
        keeper
            .carrying
            .insert(idx.unwrap(), obj_id);
    } else {
        keeper.carrying.push(obj_id);
    }

}

fn sort_keeper_objs(game: &mut Game,chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>,  keeper_id: DepotId, shop_nr: usize) {
    let mut list: Vec<DepotId> = vec![];
    while { let keeper = chars.get(keeper_id); db.shop_index[shop_nr].lastsort < keeper.is_carrying_n() as i32 } {
        let keeper = chars.get(keeper_id);
        let oid = keeper.carrying[0];
        obj_from_char(chars, objs.get_mut(oid));
        list.push(oid);
    }

    while list.len() != 0 {
        let temp_id = list.remove(0);
        let keeper = chars.get_mut(keeper_id);
        if shop_producing(&db, objs, temp_id, shop_nr)
            && get_obj_in_list_num(objs, objs.get(temp_id).get_obj_rnum(), &keeper.carrying)
                .is_none()
        {
            obj_to_char(objs.get_mut(temp_id), keeper);
            db.shop_index[shop_nr].lastsort += 1;
        } else {
            slide_obj(game, chars, db, objs, temp_id, keeper_id, shop_nr);
        }
    }
}

fn shopping_sell(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    arg: &str,
    chid: DepotId,
    keeper_id: DepotId,
    shop_nr: usize,
) {
    let mut sold = 0;
    let mut goldamt = 0;

    if !is_ok(game, chars, db,texts,objs,keeper_id, chid, shop_nr) {
        return;
    }
    let mut arg = arg.to_string();
    let sellnum = transaction_amt(&mut arg);
    if sellnum < 0 {
        let ch = chars.get(chid);
        let buf = format!(
            "{} A negative amount?  Try buying something.",
            ch.get_name()
        );
        do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    if arg.is_empty() || sellnum == 0 {
        let ch = chars.get(chid);
        let buf = format!("{} What do you want to sell??", ch.get_name());
        do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    let mut name = String::new();
    one_argument(&arg, &mut name);
    let oid = get_selling_obj(game, chars, db,texts,objs,chid, &name, keeper_id, shop_nr, 1);
    if oid.is_none() {
        return;
    }
    let oid = oid.unwrap();
    let ch = chars.get(chid);
    let keeper = chars.get(keeper_id);
    if keeper.get_gold() + db.shop_index[shop_nr].bank_account
        < sell_price(objs.get(oid), & db.shop_index[shop_nr], keeper, ch)
    {
        let buf = db.shop_index[shop_nr]
            .missing_cash1
            .replace("%s", &ch.get_name());
        do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    let mut oid = Some(oid);
    while {let keeper = chars.get(keeper_id);  let ch = chars.get(chid); oid.is_some()
        && keeper.get_gold() + db.shop_index[shop_nr].bank_account
            >= sell_price(
                objs.get(oid.unwrap()),
                & db.shop_index[shop_nr],
                keeper,
                ch,
            )
        && sold < sellnum }
    {
        let ch = chars.get(chid);
        let keeper = chars.get(keeper_id);
        let charged = sell_price(
            objs.get(oid.unwrap()),
            & db.shop_index[shop_nr],
            keeper,
            ch,
        );

        goldamt += charged;
        let keeper = chars.get_mut(keeper_id);
        keeper.set_gold(keeper.get_gold() - charged);

        sold += 1;
        obj_from_char(chars, objs.get_mut (oid.unwrap()));
        slide_obj(game, chars, db,objs,oid.unwrap(), keeper_id, shop_nr); /* Seems we don't use return value. */
        oid = get_selling_obj(game, chars, db,texts,objs,chid, &name, keeper_id, shop_nr, 0);
    }
    let ch = chars.get(chid);
    let keeper = chars.get(keeper_id);
    if sold < sellnum {
        let buf;
        if oid.is_none() {
            let ch = chars.get(chid);
            buf = format!("{} You only have {} of those.", ch.get_name(), sold);
        } else if keeper.get_gold() + db.shop_index[shop_nr].bank_account
            < sell_price(
                objs.get(oid.unwrap()),
                & db.shop_index[shop_nr],
                keeper,
                ch,
            )
        {
            buf = format!(
                "{} I can only afford to buy {} of those.",
                ch.get_name(),
                sold
            );
        } else {
            buf = format!(
                "{} Something really screwy made me buy {}.",
                ch.get_name(),
                sold
            );
        }

        do_tell(game,db,chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
    }
    let ch = chars.get_mut(chid);
    ch.set_gold(ch.get_gold() + goldamt);

    let tempstr = times_message(None, &name, sold);
    let tempbuf = format!("$n sells {}.", tempstr);
    let ch = chars.get(chid);
    let obj = oid.map(|id| objs.get(id));
    act(&mut game.descriptors, chars, db,
        &tempbuf,
        false,
        Some(ch),
obj,
        None,
        TO_ROOM,
    );
    let ch = chars.get(chid);
    let tempbuf = db.shop_index[shop_nr]
        .message_sell
        .replace("%s", &ch.get_name())
        .replace("%d", &goldamt.to_string());
    do_tell(game, db, chars, texts,objs,keeper_id, &tempbuf, CMD_TELL.load(Ordering::Relaxed), 0);
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch,
        format!("The shopkeeper now has {}.\r\n", tempstr).as_str(),
    );
    let keeper = chars.get(keeper_id);
    if keeper.get_gold() < MIN_OUTSIDE_BANK {
        let goldamt = min(
            MAX_OUTSIDE_BANK - keeper.get_gold(),
            db.shop_index[shop_nr].bank_account,
        );
        db.shop_index[shop_nr].bank_account -= goldamt;
        let keeper = chars.get_mut(keeper_id);
        keeper.set_gold(keeper.get_gold() + goldamt);
    }
}

fn shopping_value(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    arg: &str,
    chid:DepotId,
    keeper_id: DepotId,
    shop_nr: usize,
) {
    if !is_ok(game,chars, db, texts,objs,keeper_id, chid, shop_nr) {
        return;
    }

    if arg.is_empty() {
        let ch = chars.get(chid);
        let buf = format!("{} What do you want me to evaluate??", ch.get_name());
        do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    let mut name = String::new();
    one_argument(arg, &mut name);
    let oid = get_selling_obj(game, chars, db,texts,objs,chid, &name, keeper_id, shop_nr, 1);
    if oid.is_none() {
        return;
    }
    let ch = chars.get(chid);
    let keeper = chars.get(keeper_id);
    let buf = format!(
        "{} I'll give you {} gold coins for that!",
        ch.get_name(),
        sell_price(
            objs.get(oid.unwrap()),
            & db.shop_index[shop_nr],
            keeper,
            ch
        )
    );
    do_tell(game, db, chars, texts,objs,keeper_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
}

fn list_object(
    db: &DB, chars: &Depot<CharData>,
    oid: DepotId,objs: & Depot<ObjData>, 
    cnt: i32,
    aindex: i32,
    shop_nr: usize,
    keeper_id: DepotId,
    chid: DepotId,
) -> String {
    let mut result = String::new();
    let mut quantity = String::new();
    let itemname;
    if shop_producing(db,objs, oid, shop_nr) {
        quantity.push_str("Unlimited");
    } else {
        quantity.push_str(format!("{}", cnt).as_str());
    }
    match objs.get(oid).get_obj_type() {
        ITEM_DRINKCON => {
            if objs.get(oid).get_obj_val(1) != 0 {
                itemname = Rc::from(format!(
                    "{} of {}",
                    objs.get(oid).short_description,
                    DRINKS[objs.get(oid).get_obj_val(2) as usize]
                ).as_str());
            } else {
                itemname = objs.get(oid).short_description.clone();
            }
        }

        ITEM_WAND | ITEM_STAFF => {
            itemname = Rc::from(format!(
                "{}{}",
                objs.get(oid).short_description,
                if objs.get(oid).get_obj_val(2) < objs.get(oid).get_obj_val(1) {
                    " (partially used)"
                } else {
                    ""
                }
            ).as_str());
        }

        _ => {
            itemname = objs.get(oid).short_description.clone();
        }
    }

    result.push_str(
        format!(
            " {:2})  {:9}   {:48} {:6}\r\n",
            aindex,
            quantity,
            itemname,
            buy_price(chars, db,objs, oid, shop_nr, keeper_id, chid)
        )
        .as_str(),
    );
    result
}

pub fn shopping_list(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    arg: &str,
    chid: DepotId,
    keeper_id: DepotId,
    shop_nr: usize,
) {
    let mut cnt = 0;
    let mut lindex = 0;
    let mut found = false;
    let mut name = String::new();

    /* cnt is the number of that particular object available */

    if !is_ok(game, chars, db, texts, objs,keeper_id, chid, shop_nr) {
        return;
    }
    let keeper = chars.get(keeper_id);
    if db.shop_index[shop_nr].lastsort < keeper.is_carrying_n() as i32 {
        sort_keeper_objs(game, chars, db, objs, keeper_id, shop_nr);
    }

    one_argument(arg, &mut name);

    let mut buf = String::from(" ##   Available   Item                                               Cost\r\n-------------------------------------------------------------------------\r\n");
    let mut last_oid: Option<DepotId> = None;
    let keeper = chars.get(keeper_id);
    if keeper.carrying.len() != 0 {
        for &oid in &keeper.carrying {
            let ch = chars.get(chid);
            if can_see_obj(&game.descriptors, chars, db,ch, objs.get(oid)) && objs.get(oid).get_obj_cost() > 0 {
                if last_oid.is_none() {
                    last_oid = Some(oid);
                    cnt = 1;
                } else if last_oid.is_some() && same_obj(objs.get(last_oid.unwrap()), objs.get(oid)) {
                    cnt += 1;
                } else {
                    lindex += 1;
                    if name.is_empty() || isname(&name, objs.get(last_oid.unwrap()).name.as_ref()) {
                        buf.push_str(&list_object(
                            db, chars, 
                            last_oid.unwrap(),objs,
                            cnt,
                            lindex,
                            shop_nr,
                            keeper_id,
                            chid,
                        ));
                        found = true;
                    }
                    cnt = 1;
                    last_oid = Some(oid);
                }
            }
        }
    }
    let ch = chars.get(chid);
    lindex += 1;
    if last_oid.is_none() {
        /* we actually have nothing in our list for sale, period */
        send_to_char(&mut game.descriptors, ch, "Currently, there is nothing for sale.\r\n");
    } else if !name.is_empty() && !found {
        /* nothing the char was looking for was found */
        send_to_char(&mut game.descriptors, ch, "Presently, none of those are for sale.\r\n");
    } else {
        if name.is_empty() || isname(&name, objs.get(last_oid.unwrap()).name.as_ref()) {
            /* show last obj */
            buf.push_str(&list_object(
                db, chars, 
                last_oid.unwrap(),objs,
                cnt,
                lindex,
                shop_nr,
                keeper_id,
                chid,
            ));
            let ch = chars.get(chid);
            let d_id = ch.desc.unwrap();
            page_string(&mut game.descriptors, chars,  d_id, &buf, true);
        }
    }
}

fn ok_shop_room(shop: &ShopData, room: RoomVnum) -> bool {
    for mindex in 0..shop.in_room.len() {
        if shop.in_room[mindex] == room {
            return true;
        }
    }
    false
}

pub fn shop_keeper(
    game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    me: MeRef,
    cmd: i32,
    argument: &str,
) -> bool {
    let keeper_id;
    match me {
        MeRef::Char(me_chid) => keeper_id = me_chid,
        _ => panic!("Unexpected MeRef type in shop_keeper"),
    }
    let keeper = chars.get(keeper_id);
    let shop_nr;
    {
        let shops = &db.shop_index;
        let shopo = shops.iter().position(|s| s.keeper == keeper.nr);

        if shopo.is_none() {
            return false;
        }

        shop_nr = shopo.unwrap();
    }

    if db.shop_index[shop_nr].func.is_some() {
        let func = db.shop_index[shop_nr].func.unwrap();
        if func(game, chars, db,texts,objs, chid, me, cmd, argument) {
            return true;
        }
    }

    if keeper_id == chid {
        if cmd != 0 {
            db.shop_index[shop_nr].lastsort = 0;
        }
        return false;
    }
    let ch = chars.get(chid);
    if {
        let room = db.get_room_vnum(ch.in_room());
        !ok_shop_room(&mut db.shop_index[shop_nr], room)
    } {
        return false;
    }
    let keeper = chars.get(keeper_id);
    if !keeper.awake() {
        return false;
    }
    let ch = chars.get(chid);
    if cmd_is(cmd, "steal") {
        let argm = format!("$N shouts '{}'", MSG_NO_STEAL_HERE);
        act(&mut game.descriptors, chars, db,&argm, false, Some(ch), None, Some(VictimRef::Char(keeper)), TO_CHAR);
        let ch = chars.get(chid);
                do_action(
            game, db, chars, texts,objs,
            keeper_id,
            &ch.get_name().clone(),
            CMD_SLAP.load(Ordering::Relaxed),
            0,
        );
        return true;
    }

    if cmd_is(cmd, "buy") {
        shopping_buy(game, chars, db, texts,objs,argument, chid, keeper_id, shop_nr);
        return true;
    } else if cmd_is(cmd, "sell") {
        shopping_sell(game, chars, db, texts,objs,argument, chid, keeper_id, shop_nr);
        return true;
    } else if cmd_is(cmd, "value") {
        shopping_value(game, chars, db, texts,objs,argument, chid, keeper_id, shop_nr);
        return true;
    } else if cmd_is(cmd, "list") {
        shopping_list(game,chars, db, texts,objs,argument, chid, keeper_id, shop_nr);
        return true;
    }
    return false;
}

pub fn ok_damage_shopkeeper(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, chid: DepotId, victim_id: DepotId) -> bool {
    let ch = chars.get(chid);
    let victim = chars.get(victim_id);
    if !db.is_mob(victim)
        || db.mob_index[victim.get_mob_rnum() as usize]
            .func
            .is_some()
            && db.mob_index[victim.get_mob_rnum() as usize]
                .func
                .unwrap() as usize
                != shop_keeper as usize
    {
        return true;
    }

    /* Prevent "invincible" shopkeepers if they're charmed. */
    if victim.aff_flagged(AFF_CHARM) {
        return true;
    }

    let l = db.shop_index.len();
    for sindex in 0..l {
        if victim.get_mob_rnum() == db.shop_index[sindex].keeper
            && !db.shop_index[sindex].shop_kill_chars()
        {
            let buf = format!("{} {}", ch.get_name(), MSG_CANT_KILL_KEEPER);
            do_tell(game, db, chars, texts,objs,victim_id, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
            let ch = chars.get(chid);
            do_action(
                game, db, chars, texts,objs,
                victim_id,
                &ch.get_name().clone(),
                CMD_SLAP.load(Ordering::Relaxed),
                0,
            );
            return false;
        }
    }

    true
}

/* val == obj_vnum and obj_rnum (?) */
fn add_to_list(db: &DB, list: &mut Vec<ShopBuyData>, type_: i32, val: &mut i32) -> i32 {
    if *val != NOTHING as i32 {
        return {
            if type_ == LIST_PRODUCE {
                *val = db.real_object(*val as ObjVnum) as i32;
            }
            if *val != NOTHING as i32 {
                list.push(ShopBuyData {
                    type_: *val,
                    keywords: Rc::from(""),
                });
            } else {
                *val = NOTHING as i32;
            }
            0
        };
    };
    0
}

fn end_read_list(list: &mut Vec<ShopBuyData>, error: i32) -> usize {
    if error != 0 {
        error!("SYSERR: Raise MAX_SHOP_OBJ constant in shop.h to {}", error);
    }
    list.push(ShopBuyData {
        type_: NOTHING as i32,
        keywords: Rc::from(""),
    });

    return list.len();
}

fn read_line_int(db: &DB, reader: &mut BufReader<File>, data: &mut i32) {
    let mut buf = String::new();
    if get_line(reader, &mut buf) != 0 {
        let r = buf.parse::<i32>();
        if r.is_ok() {
            *data = r.unwrap();
            return;
        }
    }
    error!(
        "SYSERR: Error in shop #{}, near '{}' with int",
        db.shop_index.len(),
        buf
    );
    process::exit(1);
}

fn read_line_float(db: &DB, reader: &mut BufReader<File>, data: &mut f32) {
    let mut buf = String::new();
    if get_line(reader, &mut buf) != 0 {
        let r = buf.parse::<f32>();
        if r.is_ok() {
            *data = r.unwrap();
            return;
        }
    }
    error!(
        "SYSERR: Error in shop #{}, near '{}' with float",
        db.shop_index.len(),
        buf
    );
    process::exit(1);
}

fn read_list(
    db: &DB,
    reader: &mut BufReader<File>,
    list: &mut Vec<ShopBuyData>,
    new_format: bool,
    max: i32,
    type_: i32,
) -> usize {
    let mut temp = -1;
    let mut error = 0;
    if new_format {
        loop {
            read_line_int(db, reader, &mut temp);
            if temp < 0 {
                /* Always "-1" the string. */
                break;
            }
            error += add_to_list(db, list, type_, &mut temp);
        }
    } else {
        for _ in 0..max {
            read_line_int(db, reader, &mut temp);
            error += add_to_list(db, list, type_, &mut temp);
        }
    }
    return end_read_list(list, error);
}

/* END_OF inefficient. */
fn read_type_list(
    db: &DB,
    reader: &mut BufReader<File>,
    list: &mut Vec<ShopBuyData>,
    new_format: bool,
    max: i32,
) -> usize {
    let mut error = 0;

    if !new_format {
        return read_list(db, reader, list, false, max, LIST_TRADE);
    }
    let mut buf = String::new();

    loop {
        buf.clear();
        reader.read_line(&mut buf).expect("Error reading shop");

        let pos = buf.find(';');
        if pos.is_some() {
            buf.truncate(pos.unwrap());
        } else {
            buf.pop();
        }

        let mut num = NOTHING as i32;

        if buf != "-1" {
            let mut tindex = 0;
            loop {
                if ITEM_TYPES[tindex] == "\n" {
                    break;
                }

                if buf == ITEM_TYPES[tindex] {
                    num = tindex as i32;
                    buf.push_str(ITEM_TYPES[tindex]);
                    break;
                }
                tindex += 1;
            }
        }

        // TODO ??
        // ptr = buf;
        // if num == NOTHING {
        //     sscanf(buf, "{}", &num);
        //     while (!isdigit(*ptr))
        //     ptr + +;
        //     while (isdigit(*ptr))
        //     ptr + +;
        // }
        // while (isspace(*ptr))
        // ptr + +;
        // while (isspace(*(END_OF(ptr) - 1)))
        //     * (END_OF(ptr) - 1) = '\0';
        error += add_to_list(db, list, LIST_TRADE, &mut num);
        // if (*ptr)
        // BUY_WORD(list[len - 1]) = strdup(ptr);
        if num < 0 {
            break;
        }
    }
    return end_read_list(list, error);
}

fn read_shop_message(mnum: i32, shr: RoomRnum, reader: &mut BufReader<File>, why: &str) -> Rc<str> {
    let mut err = 0;
    let mut ds = 0;
    let mut ss = 0;
    let tbuf;
    if {
        tbuf = fread_string(reader, why);
        tbuf.len() == 0
    } {
        return Rc::from("");
    }

    let cht = tbuf
        .find('%')
        .expect("Cannot find % in shop message string");
    if &tbuf[cht + 1..cht + 2] == "s" {
        ss += 1;
    } else if &tbuf[cht + 1..cht + 2] == "d" && (mnum == 5 || mnum == 6) {
        if ss == 0 {
            error!("SYSERR: Shop #{} has before {}, message", shr, mnum);
            err += 1;
        }
        ds += 1;
    } else if &tbuf[cht + 1..cht + 2] != "%" {
        error!(
            "SYSERR: Shop #{} has invalid format '%{}' in message #{}.",
            shr,
            &tbuf[cht + 1..cht + 2],
            mnum
        );
        err += 1;
    }

    if ss > 1 || ds > 1 {
        error!(
            "SYSERR: Shop #{} has too many specifiers for message #{}. {} {}",
            shr, mnum, ss, ds
        );
        err += 1;
    }

    if err != 0 {
        return Rc::from("");
    }
    return Rc::from(tbuf);
}

pub fn boot_the_shops(db: &mut DB, shop_f: File, filename: &str, _rec_count: i32) {
    let mut new_format = false;
    let mut reader = BufReader::new(shop_f);
    let mut done = false;
    let mut buf2 = format!("beginning of shop file {}", filename);

    while !done {
        let buf = fread_string(&mut reader, &buf2);
        if buf.starts_with('#') {
            /* New shop */

            let regex = Regex::new(r"^#(-?\+?\d{1,9})").unwrap();
            let f = regex.captures(&buf).unwrap();
            let mut temp = f[1].parse::<i32>().unwrap();
            buf2 = format!("shop #{} in shop file {}", temp, filename);

            let mut shop = ShopData {
                vnum: temp as RoomVnum,
                producing: vec![],
                profit_buy: 0.0,
                profit_sell: 0.0,
                type_: vec![],
                no_such_item1: Rc::from(""),
                no_such_item2: Rc::from(""),
                missing_cash1: Rc::from(""),
                missing_cash2: Rc::from(""),
                do_not_buy: Rc::from(""),
                message_buy: Rc::from(""),
                message_sell: Rc::from(""),
                temper1: 0,
                bitvector: 0,
                keeper: 0,
                with_who: 0,
                in_room: vec![],
                open1: 0,
                open2: 0,
                close1: 0,
                close2: 0,
                bank_account: 0,
                lastsort: 0,
                func: None,
            };

            let mut list: Vec<ShopBuyData> = vec![];
            temp = read_list(
                db,
                &mut reader,
                &mut list,
                new_format,
                MAX_PROD,
                LIST_PRODUCE,
            ) as i32;
            for count in 0..temp {
                shop.producing
                    .push(list[count as usize].buy_type() as ObjVnum);
            }

            read_line_float(db, &mut reader, &mut shop.profit_buy);
            read_line_float(db, &mut reader, &mut shop.profit_sell);

            list.clear();
            temp = read_type_list(db, &mut reader, &mut list, new_format, MAX_TRADE) as i32;

            for count in 0..temp as usize {
                shop.type_.push({
                    ShopBuyData {
                        type_: list[count].type_,
                        keywords: list[count].keywords.clone(),
                    }
                })
            }

            shop.no_such_item1 = read_shop_message(0, shop.vnum, &mut reader, &buf2);
            shop.no_such_item2 = read_shop_message(1, shop.vnum, &mut reader, &buf2);
            shop.do_not_buy = read_shop_message(2, shop.vnum, &mut reader, &buf2);
            shop.missing_cash1 = read_shop_message(3, shop.vnum, &mut reader, &buf2);
            shop.missing_cash2 = read_shop_message(4, shop.vnum, &mut reader, &buf2);
            shop.message_buy = read_shop_message(5, shop.vnum, &mut reader, &buf2);
            shop.message_sell = read_shop_message(6, shop.vnum, &mut reader, &buf2);
            read_line_int(db, &mut reader, &mut shop.temper1);
            read_line_int(db, &mut reader, &mut shop.bitvector);
            let mut shop_keeper = NOBODY as i32;
            read_line_int(db, &mut reader, &mut shop_keeper);
            shop.keeper = db.real_mobile(shop_keeper as MobVnum);
            read_line_int(db, &mut reader, &mut shop.with_who);
            let mut list: Vec<ShopBuyData> = vec![];
            temp = read_list(db, &mut reader, &mut list, new_format, 1, LIST_ROOM) as i32;
            for count in 0..temp as usize {
                shop.in_room.push(list[count].type_ as RoomVnum);
            }

            read_line_int(db, &mut reader, &mut shop.open1);
            read_line_int(db, &mut reader, &mut shop.close1);
            read_line_int(db, &mut reader, &mut shop.open2);
            read_line_int(db, &mut reader, &mut shop.close2);

            db.shop_index.push(shop);
        } else {
            if buf.starts_with('$') {
                /* EOF */
                done = true;
            } else if buf.contains(VERSION3_TAG) {
                /* New format marker */
                new_format = true;
            }
        }
    }
}

static CMD_SAY: AtomicUsize = AtomicUsize::new(0);
static CMD_TELL: AtomicUsize = AtomicUsize::new(0);
static CMD_EMOTE: AtomicUsize = AtomicUsize::new(0);
static CMD_SLAP: AtomicUsize = AtomicUsize::new(0);
static CMD_PUKE: AtomicUsize = AtomicUsize::new(0);

pub fn assign_the_shopkeepers(db: &mut DB) {
    CMD_SAY.store(find_command("say").unwrap(), Ordering::Relaxed);
    CMD_TELL.store(find_command("tell").unwrap(), Ordering::Relaxed);
    CMD_TELL.store(find_command("emote").unwrap(), Ordering::Relaxed);
    CMD_SLAP.store(find_command("slap").unwrap(), Ordering::Relaxed);
    CMD_PUKE.store(find_command("puke").unwrap(), Ordering::Relaxed);

    for shop in db.shop_index.iter_mut() {
        if shop.keeper == NOBODY {
            continue;
        }
        db.mob_index[shop.keeper as usize].func = Some(shop_keeper);
        /* Having SHOP_FUNC() as 'shop_keeper' will cause infinite recursion. */
        if db.mob_index[shop.keeper as usize].func.is_some()
            && db.mob_index[shop.keeper as usize].func.unwrap() as usize != shop_keeper as usize
        {
            db.mob_index[shop.keeper as usize].func = db.mob_index[shop.keeper as usize].func;
        }
    }
}

fn customer_string(shop: &ShopData, detailed: bool) -> String {
    let mut sindex = 0;
    let mut flag = 1;
    let mut buf = String::new();
    while TRADE_LETTERS[sindex] != "\n" {
        if detailed {
            if !is_set!(flag, shop.with_who) {
                buf.push_str(format!(", {}", TRADE_LETTERS[sindex]).as_str());
            }
        } else {
            buf.push(if !is_set!(flag, shop.with_who) {
                '_'
            } else {
                TRADE_LETTERS[sindex].chars().next().unwrap()
            });
        }
        sindex += 1;
        flag <<= 1;
    }
    buf
}

// /* END_OF inefficient */
fn list_all_shops(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, chid: DepotId) {
    let ch = chars.get(chid);
    const LIST_ALL_SHOPS_HEADER: &str =
        " ##   Virtual   Where    Keeper    Buy   Sell   Customers\r\n\
---------------------------------------------------------\r\n";
    let mut buf = String::new();
    for (shop_nr, shop) in db.shop_index.iter().enumerate() {
        /* New page in page_string() mechanism, print the header again. */
        if shop_nr as i32 % (PAGE_LENGTH - 2) == 0 {
            /*
             * If we don't have enough room for the header, or all we have room left
             * for is the header, then don't add it and just quit now.
             */

            buf.push_str(LIST_ALL_SHOPS_HEADER);
        }
        let mut buf1 = String::new();
        if shop.keeper == NOBODY {
            buf1.push_str("<NONE>");
        } else {
            buf1.push_str(format!("{:6}", db.mob_index[shop.keeper as usize].vnum).as_str());
        }

        buf.push_str(
            format!(
                "{:3}   {:6}   {:6}    {}   {:5}   {:5}    {}\r\n",
                shop_nr + 1,
                shop.vnum,
                shop.in_room[0],
                buf1,
                shop.profit_sell,
                shop.profit_buy,
                customer_string(shop, false)
            )
            .as_str(),
        );
    }
    let d_id = ch.desc.unwrap();
    page_string(&mut game.descriptors, chars,  d_id, &buf, true);
}

fn list_detailed_shop(game: &mut Game, chars: &Depot<CharData>, db: &DB, chid: DepotId, shop_nr: i32) {
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch,
        format!(
            "Vnum:       [{:5}], Rnum: [{:5}]\r\n",
            db.shop_index[shop_nr as usize].vnum,
            shop_nr + 1
        )
        .as_str(),
    );

    send_to_char(&mut game.descriptors, ch, "Rooms:      ");
    let mut column = 12; /* ^^^ strlen ^^^ */
    let mut count = 0;
    for sindex in 0..db.shop_index[shop_nr as usize].in_room.len() {
        count += 1;
        if sindex != 0 {
            send_to_char(&mut game.descriptors, ch, ", ");
            column += 2;
        }
        let temp;
        let buf1;
        if {
            temp = db.real_room(db.shop_index[shop_nr as usize].in_room[sindex]);
            temp != NOWHERE
        } {
            buf1 = format!(
                "{} (#{})",
                db.world[temp as usize].name,
                db.get_room_vnum(temp)
            );
        } else {
            buf1 = format!("<UNKNOWN> (#{})", db.shop_index[shop_nr as usize].in_room[sindex]);
        }

        /* Implementing word-wrapping: assumes screen-size == 80 */
        if buf1.len() + column >= 78 && column >= 20 {
            send_to_char(&mut game.descriptors, ch, "\r\n            ");
            /* 12 is to line up with "Rooms:" printed first, and spaces above. */
            column = 12;
        }

        send_to_char(&mut game.descriptors, ch, &buf1);

        column += buf1.len();
    }
    if count == 0 {
        send_to_char(&mut game.descriptors, ch, "Rooms:      None!");
    }

    send_to_char(&mut game.descriptors, ch, "\r\nShopkeeper: ");
    if db.shop_index[shop_nr as usize].keeper != NOBODY {
        send_to_char(&mut game.descriptors, ch,
            format!(
                "{} (#{}), Special Function: {}\r\n",
                db.mob_protos[db.shop_index[shop_nr as usize].keeper as usize].get_name(),
                db.mob_index[db.shop_index[shop_nr as usize].keeper as usize].vnum,
                yesno!(db.shop_index[shop_nr as usize].func.is_some())
            )
            .as_str(),
        );
        let k;
        if {
            k = db.get_char_num(chars, db.shop_index[shop_nr as usize].keeper);
            k.is_some()
        } {
            let k = k.unwrap();
            send_to_char(&mut game.descriptors, ch,
                format!(
                    "Coins:      [{:9}], Bank: [{:9}] (Total: {})\r\n",
                    k.get_gold(),
                    db.shop_index[shop_nr as usize].bank_account,
                    k.get_gold() + db.shop_index[shop_nr as usize].bank_account
                )
                .as_str(),
            );
        } else {
            send_to_char(&mut game.descriptors, ch, "<NONE>\r\n");
        }
    }
    let ptrsave;
    send_to_char(&mut game.descriptors, ch,
        format!(
            "Customers:  {}\r\n",
            if {
                ptrsave = customer_string(&db.shop_index[shop_nr as usize], true);
                !ptrsave.is_empty()
            } {
                ptrsave
            } else {
                "None".to_string()
            }
        )
        .as_str(),
    );

    send_to_char(&mut game.descriptors, ch, "Produces:   ");
    let mut column = 12; /* ^^^ strlen ^^^ */
    let mut sindex = 0;
    let mut buf1 = String::new();
    let mut count = 0;
    while db.shop_index[shop_nr as usize].shop_product(sindex) != NOTHING {
        count += 1;
        if sindex != 0 {
            send_to_char(&mut game.descriptors, ch, ", ");
            column += 2;
        }
        let nbuf = format!(
            "{} (#{})",
            db.obj_proto[db.shop_index[shop_nr as usize].shop_product(sindex) as usize].short_description,
            db.obj_index[db.shop_index[shop_nr as usize].shop_product(sindex) as usize].vnum
        );
        buf1.push_str(&nbuf);
        /* Implementing word-wrapping: assumes screen-size == 80 */
        if nbuf.len() + column >= 78 && column >= 20 {
            send_to_char(&mut game.descriptors, ch, "\r\n            ");
            /* 12 is to line up with "Produces:" printed first, and spaces above. */
            column = 12;
        }

        send_to_char(&mut game.descriptors, ch, &buf1);
        buf1.clear();
        column += nbuf.len();
        sindex += 1;
    }
    if count == 0 {
        send_to_char(&mut game.descriptors, ch, "Produces:   Nothing!");
    }

    send_to_char(&mut game.descriptors, ch, "\r\nBuys:       ");
    let mut column = 12; /* ^^^ strlen ^^^ */

    sindex = 0;
    count = 0;
    while db.shop_index[shop_nr as usize].type_[sindex as usize].type_ != NOTHING as i32 {
        count += 1;

        let buf1;
        if sindex != 0 {
            send_to_char(&mut game.descriptors, ch, ", ");
            column += 2;
        }

        buf1 = format!(
            "{} (#{}) [{}]",
            ITEM_TYPES[db.shop_index[shop_nr as usize].type_[sindex as usize].type_ as usize],
            db.shop_index[shop_nr as usize].type_[sindex as usize].type_,
            if !db.shop_index[shop_nr as usize].type_[sindex as usize].keywords.is_empty() {
                &db.shop_index[shop_nr as usize].type_[sindex as usize].keywords
            } else {
                "all"
            }
        );

        /* Implementing word-wrapping: assumes screen-size == 80 */
        if buf1.len() + column >= 78 && column >= 20 {
            send_to_char(&mut game.descriptors, ch, "\r\n            ");
            /* 12 is to line up with "Buys:" printed first, and spaces above. */
            column = 12;
        }

        send_to_char(&mut game.descriptors, ch, &buf1);

        column += buf1.len();
        sindex += 1;
    }
    if count == 0 {
        send_to_char(&mut game.descriptors, ch, "Buys:       Nothing!");
    }

    send_to_char(&mut game.descriptors, ch,
        format!(
            "\r\nBuy at:     [{:6}], Sell at: [{:6}], Open: [{}-{}, {}-{}]\r\n",
            db.shop_index[shop_nr as usize].profit_sell, db.shop_index[shop_nr as usize].profit_buy, db.shop_index[shop_nr as usize].open1, db.shop_index[shop_nr as usize].close1, db.shop_index[shop_nr as usize].open2, db.shop_index[shop_nr as usize].close2
        )
        .as_str(),
    );

    /* Need a local buffer. */
    let mut buf1 = String::new();
    sprintbit(db.shop_index[shop_nr as usize].bitvector as i64, &SHOP_BITS, &mut buf1);
    send_to_char(&mut game.descriptors, ch, format!("Bits:       {}\r\n", buf1).as_str());
}

pub fn show_shops(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, chid: DepotId, arg: &str) {
    let ch = chars.get(chid);

    if arg.is_empty() {
        list_all_shops(game, chars, db, chid);
    } else {
        let mut shop_nr = None;
        if arg == "." {
            for (i, shop) in db.shop_index.iter().enumerate() {
                if ok_shop_room(shop, db.get_room_vnum(ch.in_room())) {
                    shop_nr = Some(i as i32);
                    break;
                }
            }

            if shop_nr.is_none() {
                send_to_char(&mut game.descriptors, ch, "This isn't a shop!\r\n");
                return;
            }
        } else if is_number(arg) {
            let ap = arg.parse::<i32>();
            if ap.is_ok() {
                shop_nr = Some(ap.unwrap() - 1);
            }
        }
        if shop_nr.is_none() {
            send_to_char(&mut game.descriptors, ch, "Illegal shop number.\r\n");
            return;
        }
        list_detailed_shop(game, chars, db, chid, shop_nr.unwrap());
    }
}

pub fn destroy_shops(db: &mut DB) {
    db.shop_index.clear();
}
