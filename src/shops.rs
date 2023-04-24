/* ************************************************************************
*   File: shop.h                                        Part of CircleMUD *
*  Usage: shop file definitions, structures, constants                    *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::any::Any;
use std::borrow::BorrowMut;
use std::cmp::min;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::error;
use regex::Regex;

use crate::act_comm::{do_say, do_tell};
use crate::act_social::do_action;
use crate::act_wizard::do_echo;
use crate::constants::{DRINKS, EXTRA_BITS, ITEM_TYPES};
use crate::db::{fread_string, DB, REAL};
use crate::handler::{fname, get_number, isname, obj_from_char};
use crate::interpreter::{cmd_is, find_command, is_number, one_argument, SCMD_EMOTE};
use crate::modify::page_string;
use crate::structs::{
    CharData, MobRnum, MobVnum, ObjData, ObjVnum, RoomRnum, RoomVnum, AFF_CHARM, ITEM_DRINKCON,
    ITEM_NOSELL, ITEM_STAFF, ITEM_WAND, LVL_GOD, MAX_OBJ_AFFECT, NOBODY, NOTHING, NOWHERE,
};
use crate::util::{get_line, sprintbit};
use crate::{an, is_set, send_to_char, yesno, Game, PAGE_LENGTH, TO_CHAR, TO_ROOM};

pub struct ShopBuyData {
    pub type_: i32,
    pub keywords: Rc<str>,
}

impl ShopBuyData {
    pub fn buy_type(&self) -> i32 {
        self.type_
    }
    // pub fn buy_word(&self) -> &str {
    //     self.keywords.as_ref()
    // }
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
    //SPECIAL (*func);		/* Secondary spec_proc for shopkeeper	*/
}

//
//
const MAX_TRADE: i32 = 5; /* List maximums for compatibility	*/
const MAX_PROD: i32 = 5; /*	with shops before v3.0		*/
const VERSION3_TAG: &str = "v3.0"; /* The file has v3.0 shops in it!	*/
// const MAX_SHOP_OBJ: i32 = 100; /* "Soft" maximum for list maximums	*/
//
//
// /* Pretty general macros that could be used elsewhere */
// #define IS_GOD(ch)		(!IS_NPC(ch) && (GET_LEVEL(ch) >= LVL_GOD))
// #define END_OF(buffer)		((buffer) + strlen((buffer)))

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

// #define S_DATA(stack, index)	((stack)->data[(index)])
// #define S_LEN(stack)		((stack)->len)
//
//
/* Which expression type we are now parsing */
const OPER_OPEN_PAREN: i32 = 0;
const OPER_CLOSE_PAREN: i32 = 1;
const OPER_OR: i32 = 2;
const OPER_AND: i32 = 3;
const OPER_NOT: i32 = 4;
// const MAX_OPER: i32 = 4;

//
//
// #define SHOP_NUM(i)		(shop_index[(i)].vnum)
// #define SHOP_KEEPER(i)		(shop_index[(i)].keeper)
// #define SHOP_OPEN1(i)		(shop_index[(i)].open1)
// #define SHOP_CLOSE1(i)		(shop_index[(i)].close1)
// #define SHOP_OPEN2(i)		(shop_index[(i)].open2)
// #define SHOP_CLOSE2(i)		(shop_index[(i)].close2)
// #define SHOP_ROOM(i, num)	(shop_index[(i)].in_room[(num)])
// #define SHOP_BUYTYPE(i, num)	(BUY_TYPE(shop_index[(i)].type[(num)]))
// #define SHOP_BUYWORD(i, num)	(BUY_WORD(shop_index[(i)].type[(num)]))
// #define SHOP_PRODUCT(i, num)	(shop_index[(i)].producing[(num)])
impl ShopData {
    fn shop_product(&self, num: i32) -> ObjVnum {
        self.producing[num as usize]
    }
}

// #define SHOP_BANK(i)		(shop_index[(i)].bank_account)
// #define SHOP_BROKE_TEMPER(i)	(shop_index[(i)].temper1)
// #define SHOP_BITVECTOR(i)	(shop_index[(i)].bitvector)
// #define SHOP_TRADE_WITH(i)	(shop_index[(i)].with_who)
// #define SHOP_SORT(i)		(shop_index[(i)].lastsort)
// #define SHOP_BUYPROFIT(i)	(shop_index[(i)].profit_buy)
// #define SHOP_SELLPROFIT(i)	(shop_index[(i)].profit_sell)
// #define SHOP_FUNC(i)		(shop_index[(i)].func)
//
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

//
pub const MIN_OUTSIDE_BANK: i32 = 5000;
pub const MAX_OUTSIDE_BANK: i32 = 15000;
//
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

fn is_ok_char(game: &Game, keeper: &Rc<CharData>, ch: &Rc<CharData>, shop: &ShopData) -> bool {
    // char buf[MAX_INPUT_LENGTH];
    let db = &game.db;
    if !db.can_see(keeper, ch) {
        do_say(
            game,
            keeper,
            MSG_NO_SEE_CHAR,
            CMD_SAY.load(Ordering::Relaxed),
            0,
        );
        return false;
    }
    if ch.is_god() {
        return true;
    }

    if ch.is_good() && shop.notrade_good()
        || ch.is_evil() && shop.notrade_evil()
        || ch.is_neutral() && shop.notrade_neutral()
    {
        let buf = format!("{} {}", ch.get_name(), MSG_NO_SELL_ALIGN);
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return false;
    }
    if ch.is_npc() {
        return true;
    }

    if ch.is_magic_user() && shop.notrade_magic_user()
        || ch.is_cleric() && shop.notrade_cleric()
        || ch.is_thief() && shop.notrade_thief()
        || ch.is_warrior() && shop.notrade_warrior()
    {
        let buf = format!("{} {}", ch.get_name(), MSG_NO_SELL_CLASS);
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return false;
    }
    true
}

fn is_open(game: &Game, keeper: &Rc<CharData>, shop: &ShopData, msg: bool) -> bool {
    let db = &game.db;
    let mut buf = String::new();
    if shop.open1 > db.time_info.borrow().hours {
        buf.push_str(MSG_NOT_OPEN_YET);
    } else if shop.close1 < db.time_info.borrow().hours {
        if shop.open2 > db.time_info.borrow().hours {
            buf.push_str(MSG_NOT_REOPEN_YET);
        } else if shop.close2 < db.time_info.borrow().hours {
            buf.push_str(MSG_CLOSED_FOR_DAY);
        }
    }
    if buf.is_empty() {
        return true;
    }

    if msg {
        do_say(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
    }
    false
}

fn is_ok(game: &Game, keeper: &Rc<CharData>, ch: &Rc<CharData>, shop: &ShopData) -> bool {
    if is_open(game, keeper, shop, true) {
        return is_ok_char(game, keeper, ch, shop);
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
    if stack.len > 0 {
        stack.len -= 1;
        return stack.data[stack.len];
    } else {
        error!(
            "SYSERR: Illegal expression {} in shop keyword list.",
            stack.len
        );
        return 0;
    }
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

fn evaluate_expression(obj: &Rc<ObjData>, expr: &str) -> i32 {
    let mut ops = StackData::new();
    let mut vals = StackData::new();
    // char *ptr, *end, name[MAX_STRING_LENGTH];
    // int temp, eindex;

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
                        if isname(name, obj.name.borrow().as_ref()) {
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

fn trade_with(item: &Rc<ObjData>, shop: &ShopData) -> i32 {
    if item.get_obj_cost() < 1 {
        return OBJECT_NOVAL;
    }

    if item.obj_flagged(ITEM_NOSELL) {
        return OBJECT_NOTOK;
    }

    let mut counter = 0 as usize;

    while shop.type_[counter].type_ != NOTHING as i32 {
        if shop.type_[counter as usize].type_ == item.get_obj_type() as i32 {
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

fn same_obj(obj1: &Rc<ObjData>, obj2: &Rc<ObjData>) -> bool {
    // if (!obj1 || !obj2)
    // return (obj1 == obj2);

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
        if obj1.affected[aindex].get().location != obj2.affected[aindex].get().location
            || obj1.affected[aindex].get().modifier != obj2.affected[aindex].get().modifier
        {
            return false;
        }
    }
    true
}

fn shop_producing(db: &DB, item: &Rc<ObjData>, shop: &ShopData) -> bool {
    if item.get_obj_rnum() == NOTHING {
        return false;
    }
    for counter in 0..shop.producing.len() as usize {
        if shop.producing[counter] == NOTHING {
            break;
        }
        if same_obj(item, &db.obj_proto[shop.producing[counter] as usize]) {
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

fn times_message(obj: Option<&Rc<ObjData>>, name: &str, num: i32) -> String {
    let mut buf = String::new();
    if obj.is_some() {
        buf.push_str(obj.as_ref().unwrap().short_description.as_str())
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
    db: &DB,
    ch: &Rc<CharData>,
    name: &str,
    list: &Vec<Rc<ObjData>>,
) -> Option<Rc<ObjData>> {
    let mut tmpname = name.to_string();
    let number;
    let mut last_match = None;
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
        if isname(&tmpname, &i.name.borrow()) {
            if db.can_see_obj(ch, i)
                && (last_match.is_none() || !same_obj(last_match.as_ref().unwrap(), i))
            {
                if j == number {
                    return Some(i.clone());
                }
                last_match = Some(i.clone());
                j += 1;
            }
        }
    }
    None
}

fn get_hash_obj_vis(
    db: &DB,
    ch: &Rc<CharData>,
    name: &str,
    list: &Vec<Rc<ObjData>>,
) -> Option<Rc<ObjData>> {
    // struct obj_data *loop, *last_obj = NULL;
    // int qindex;
    let mut qindex;
    if is_number(name) {
        qindex = name.parse::<i32>().unwrap();
    } else if is_number(&name[1..]) {
        qindex = name[1..].parse::<i32>().unwrap();
    } else {
        return None;
    }
    let mut last_obj: Option<&Rc<ObjData>> = None;
    for l in list {
        if db.can_see_obj(ch, l) && l.get_obj_cost() > 0 {
            if last_obj.is_some() && !same_obj(last_obj.as_ref().unwrap(), l) {
                if {
                    qindex -= 1;
                    qindex == 0
                } {
                    return Some(l.clone());
                }
                last_obj = Some(l);
            }
        }
    }
    None
}

fn get_purchase_obj(
    game: &Game,
    ch: &Rc<CharData>,
    arg: &str,
    keeper: &Rc<CharData>,
    shop: &ShopData,
    msg: bool,
) -> Option<Rc<ObjData>> {
    let db = &game.db;
    let mut name = String::new();
    one_argument(arg, &mut name);
    let mut obj: Option<Rc<ObjData>>;
    loop {
        if name.starts_with('#') || is_number(&name) {
            obj = get_hash_obj_vis(db, ch, &name, &keeper.carrying.borrow());
        } else {
            obj = get_slide_obj_vis(db, ch, &name, &keeper.carrying.borrow());
        }
        if obj.is_none() {
            if msg {
                let buf = shop.no_such_item1.replace("%s", &ch.get_name());
                do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
            }
            return None;
        }
        if obj.as_ref().unwrap().get_obj_cost() <= 0 {
            db.extract_obj(obj.as_ref().unwrap());
            obj = None;
        }
        if obj.is_some() {
            break;
        }
    }
    obj.clone()
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
fn buy_price(
    obj: &Rc<ObjData>,
    shop: &ShopData,
    keeper: &Rc<CharData>,
    buyer: &Rc<CharData>,
) -> i32 {
    return (obj.get_obj_cost() as f32
        * shop.profit_buy
        * (1 as f32 + keeper.get_cha() as f32 - buyer.get_cha() as f32)
        / 70 as f32) as i32;
}

/*
 * When the shopkeeper is buying, we reverse the discount. Also make sure
 * we don't buy for more than we sell for, to prevent infinite money-making.
 */
fn sell_price(
    obj: &Rc<ObjData>,
    shop: &ShopData,
    keeper: &Rc<CharData>,
    seller: &Rc<CharData>,
) -> i32 {
    let mut sell_cost_modifier =
        shop.profit_sell * (1 as f32 - (keeper.get_cha() - seller.get_cha()) as f32 / 70.0);
    let buy_cost_modifier =
        shop.profit_buy * (1 as f32 + (keeper.get_cha() - seller.get_cha()) as f32 / 70.0);

    if sell_cost_modifier > buy_cost_modifier {
        sell_cost_modifier = buy_cost_modifier;
    }

    (obj.get_obj_cost() as f32 * sell_cost_modifier) as i32
}

fn shopping_buy(
    game: &Game,
    arg: &str,
    ch: &Rc<CharData>,
    keeper: &Rc<CharData>,
    shop: &mut ShopData,
) {
    let db = &game.db;
    // char tempstr[MAX_INPUT_LENGTH], tempbuf[MAX_INPUT_LENGTH];
    // obj: &Rc<ObjData>, *last_obj = NULL;
    // int goldamt = 0, buynum, bought = 0;

    if !is_ok(game, keeper, ch, shop) {
        return;
    }

    if shop.lastsort < keeper.is_carrying_n() as i32 {
        sort_keeper_objs(db, keeper, shop);
    }

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
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    if arg.is_empty() || buynum == 0 {
        let buf = format!("{} What do you want to buy??", ch.get_name());
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    let mut obj: Option<Rc<ObjData>>;
    if {
        obj = get_purchase_obj(game, ch, &arg, keeper, shop, true);
        obj.is_none()
    } {
        return;
    }
    if buy_price(obj.as_ref().unwrap(), shop, keeper, ch) > ch.get_gold() && !ch.is_god() {
        let actbuf = shop.missing_cash2.replace("%s", &ch.get_name());
        do_tell(game, keeper, &actbuf, CMD_TELL.load(Ordering::Relaxed), 0);

        match shop.temper1 {
            0 => {
                do_action(
                    game,
                    keeper,
                    &ch.get_name(),
                    CMD_PUKE.load(Ordering::Relaxed),
                    0,
                );
            }

            1 => {
                do_echo(
                    game,
                    keeper,
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
    if ch.is_carrying_n() + 1 > ch.can_carry_n() as u8 {
        send_to_char(
            ch,
            format!(
                "{}: You can't carry any more items.\r\n",
                fname(&obj.as_ref().unwrap().name.borrow())
            )
            .as_str(),
        );
        return;
    }
    if ch.is_carrying_w() + obj.as_ref().unwrap().get_obj_weight() > ch.can_carry_w() as i32 {
        send_to_char(
            ch,
            format!(
                "{}: You can't carry that much weight.\r\n",
                fname(&obj.as_ref().unwrap().name.borrow())
            )
            .as_str(),
        );
        return;
    }
    let mut bought = 0;
    let mut goldamt = 0;
    let mut last_obj: Option<Rc<ObjData>> = None;
    while obj.is_some()
        && (ch.get_gold() >= buy_price(obj.as_ref().unwrap(), shop, keeper, ch) || ch.is_god())
        && ch.is_carrying_n() < ch.can_carry_n() as u8
        && bought < buynum
        && ch.is_carrying_w() + obj.as_ref().unwrap().get_obj_weight() <= ch.can_carry_w() as i32
    {
        bought += 1;

        /* Test if producing shop ! */
        if shop_producing(db, obj.as_ref().unwrap(), shop) {
            obj = db.read_object(obj.as_ref().unwrap().get_obj_rnum(), REAL);
        } else {
            obj_from_char(Some(obj.as_ref().unwrap()));
            shop.lastsort -= 1;
        }
        DB::obj_to_char(Some(obj.as_ref().unwrap()), Some(ch));

        let charged = buy_price(obj.as_ref().unwrap(), shop, keeper, ch);
        goldamt += charged;
        if !ch.is_god() {
            ch.set_gold(ch.get_gold() - charged);
        }

        last_obj = Some(obj.as_ref().unwrap().clone());
        obj = get_purchase_obj(game, ch, &arg, keeper, shop, false);
        if obj.is_some() && !same_obj(obj.as_ref().unwrap(), last_obj.as_ref().unwrap()) {
            break;
        }
    }
    let buf;
    if bought < buynum {
        if obj.is_none() || !same_obj(last_obj.as_ref().unwrap(), obj.as_ref().unwrap()) {
            buf = format!("{} I only have {} to sell you.", ch.get_name(), bought);
        } else if ch.get_gold() < buy_price(obj.as_ref().unwrap(), shop, keeper, ch) {
            buf = format!("{} You can only afford {}.", ch.get_name(), bought);
        } else if ch.is_carrying_n() >= ch.can_carry_n() as u8 {
            buf = format!("{} You can only hold {}.", ch.get_name(), bought);
        } else if ch.is_carrying_w() + obj.as_ref().unwrap().get_obj_weight()
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
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
    }
    if !ch.is_god() {
        keeper.set_gold(keeper.get_gold() + goldamt);
    }

    let tempstr = times_message(Some(&ch.carrying.borrow()[0]), "", bought);

    let tempbuf = format!("$n buys {}.", tempstr);
    db.act(&tempbuf, false, Some(ch), obj.as_ref(), None, TO_ROOM);

    let tmpbuf = db.shop_index.borrow()[0]
        .message_buy
        .replace("%s", &ch.get_name())
        .replace("%d", &goldamt.to_string());
    do_tell(game, keeper, &tmpbuf, CMD_TELL.load(Ordering::Relaxed), 0);

    send_to_char(ch, format!("You now have {}.\r\n", tempstr).as_str());

    if shop.shop_uses_bank() {
        if keeper.get_gold() > MAX_OUTSIDE_BANK {
            shop.bank_account += keeper.get_gold() - MAX_OUTSIDE_BANK;
            keeper.set_gold(MAX_OUTSIDE_BANK);
        }
    }
}

fn get_selling_obj(
    game: &Game,
    ch: &Rc<CharData>,
    name: &str,
    keeper: &Rc<CharData>,
    shop: &ShopData,
    msg: i32,
) -> Option<Rc<ObjData>> {
    let db = &game.db;
    let obj = db.get_obj_in_list_vis(ch, name, None, ch.carrying.borrow());
    if obj.is_none() {
        if msg != 0 {
            let tbuf = db.shop_index.borrow()[0]
                .no_such_item2
                .replace("%s", &ch.get_name());

            do_tell(game, keeper, &tbuf, CMD_TELL.load(Ordering::Relaxed), 0);
        }
        return None;
    }
    let obj = obj.as_ref().unwrap();
    let result = trade_with(obj, shop);
    if result == OBJECT_OK {
        return Some(obj.clone());
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
            buf = shop.do_not_buy.replace("%s", &ch.get_name());
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
    do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
    None
}

fn slide_obj(
    db: &DB,
    obj: &Rc<ObjData>,
    keeper: &Rc<CharData>,
    shop: &mut ShopData,
) -> Rc<ObjData> {
    /*
       This function is a slight hack!  To make sure that duplicate items are
       only listed once on the "list", this function groups "identical"
       objects together on the shopkeeper's inventory list.  The hack involves
       knowing how the list is put together, and manipulating the order of
       the objects on the list.  (But since most of DIKU is not encapsulated,
       and information hiding is almost never used, it isn't that big a deal) -JF
    */

    if shop.lastsort < keeper.is_carrying_n() as i32 {
        sort_keeper_objs(db, keeper, shop);
    }
    let temp;
    /* Extract the object if it is identical to one produced */
    if shop_producing(db, obj, shop) {
        temp = obj.get_obj_rnum();
        db.extract_obj(obj);
        return db.obj_proto[temp as usize].clone();
    }
    shop.lastsort += 1;
    DB::obj_to_char(Some(obj), Some(keeper));

    let len = keeper.carrying.borrow().len();
    let obj = keeper.carrying.borrow_mut().remove(len - 1);
    let mut idx: Option<usize> = None;
    for i in 0..keeper.carrying.borrow().len() {
        if same_obj(&keeper.carrying.borrow()[i], &obj) {
            idx = Some(i);
        }
    }
    if idx.is_some() {
        keeper
            .carrying
            .borrow_mut()
            .insert(idx.unwrap(), obj.clone());
    } else {
        keeper.carrying.borrow_mut().push(obj.clone());
    }

    obj.clone()
}

fn sort_keeper_objs(db: &DB, keeper: &Rc<CharData>, shop: &mut ShopData) {
    let mut list: Vec<Rc<ObjData>> = vec![];
    while shop.lastsort < keeper.is_carrying_n() as i32 {
        let obj = keeper.carrying.borrow()[0].clone();
        obj_from_char(Some(&obj));
        list.push(obj);
    }

    while list.len() != 0 {
        let temp = list.remove(0);
        if shop_producing(db, &temp, shop)
            && db
                .get_obj_in_list_num(temp.get_obj_rnum(), &keeper.carrying.borrow())
                .is_none()
        {
            DB::obj_to_char(Some(&temp), Some(keeper));
            shop.lastsort += 1;
        } else {
            slide_obj(db, &temp, keeper, shop);
        }
    }
}

fn shopping_sell(
    game: &Game,
    arg: &str,
    ch: &Rc<CharData>,
    keeper: &Rc<CharData>,
    shop: &mut ShopData,
) {
    let db = &game.db;
    let mut sold = 0;
    let mut goldamt = 0;

    if !(is_ok(game, keeper, ch, shop)) {
        return;
    }
    let mut arg = arg.to_string();
    let sellnum = transaction_amt(&mut arg);
    if sellnum < 0 {
        let buf = format!(
            "{} A negative amount?  Try buying something.",
            ch.get_name()
        );
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    if arg.is_empty() || sellnum == 0 {
        let buf = format!("{} What do you want to sell??", ch.get_name());
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    let mut name = String::new();
    one_argument(&arg, &mut name);
    let obj = get_selling_obj(game, ch, &name, keeper, shop, 1);
    if obj.is_none() {
        return;
    }
    let obj = obj.as_ref().unwrap();

    if keeper.get_gold() + shop.bank_account < sell_price(obj, shop, keeper, ch) {
        let buf = shop.missing_cash1.replace("%s", &ch.get_name());
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    let mut obj = Some(obj.clone());
    while obj.is_some()
        && keeper.get_gold() + shop.bank_account
            >= sell_price(obj.as_ref().unwrap(), shop, keeper, ch)
        && sold < sellnum
    {
        let charged = sell_price(obj.as_ref().unwrap(), shop, keeper, ch);

        goldamt += charged;
        keeper.set_gold(keeper.get_gold() - charged);

        sold += 1;
        obj_from_char(obj.as_ref());
        slide_obj(db, obj.as_ref().unwrap(), keeper, shop); /* Seems we don't use return value. */
        obj = get_selling_obj(game, ch, &name, keeper, shop, 0).clone();
    }

    if sold < sellnum {
        let buf;
        if obj.is_none() {
            buf = format!("{} You only have {} of those.", ch.get_name(), sold);
        } else if keeper.get_gold() + shop.bank_account
            < sell_price(obj.as_ref().unwrap(), shop, keeper, ch)
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

        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
    }
    ch.set_gold(ch.get_gold() + goldamt);

    let tempstr = times_message(None, &name, sold);
    let tempbuf = format!("$n sells {}.", tempstr);
    db.act(&tempbuf, false, Some(ch), obj.as_ref(), None, TO_ROOM);

    let tempbuf = shop
        .message_sell
        .replace("%s", &ch.get_name())
        .replace("%d", &goldamt.to_string());
    do_tell(game, keeper, &tempbuf, CMD_TELL.load(Ordering::Relaxed), 0);

    send_to_char(
        ch,
        format!("The shopkeeper now has {}.\r\n", tempstr).as_str(),
    );

    if keeper.get_gold() < MIN_OUTSIDE_BANK {
        let goldamt = min(MAX_OUTSIDE_BANK - keeper.get_gold(), shop.bank_account);
        shop.bank_account -= goldamt;
        keeper.set_gold(keeper.get_gold() + goldamt);
    }
}

fn shopping_value(
    game: &Game,
    arg: &str,
    ch: &Rc<CharData>,
    keeper: &Rc<CharData>,
    shop: &ShopData,
) {
    if !is_ok(game, keeper, ch, shop) {
        return;
    }

    if arg.is_empty() {
        let buf = format!("{} What do you want me to evaluate??", ch.get_name());
        do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
        return;
    }
    let mut name = String::new();
    one_argument(arg, &mut name);
    let obj = get_selling_obj(game, ch, &name, keeper, shop, 1);
    if obj.is_none() {
        return;
    }

    let buf = format!(
        "{} I'll give you {} gold coins for that!",
        ch.get_name(),
        sell_price(obj.as_ref().unwrap(), shop, keeper, ch)
    );
    do_tell(game, keeper, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
}

fn list_object(
    db: &DB,
    obj: &Rc<ObjData>,
    cnt: i32,
    aindex: i32,
    shop: &ShopData,
    keeper: &Rc<CharData>,
    ch: &Rc<CharData>,
) -> String {
    let mut result = String::new();
    let mut quantity = String::new();
    let itemname;
    if shop_producing(db, obj, shop) {
        quantity.push_str("Unlimited");
    } else {
        quantity.push_str(format!("{}", cnt).as_str());
    }
    match obj.get_obj_type() {
        ITEM_DRINKCON => {
            if obj.get_obj_val(1) != 0 {
                itemname = format!(
                    "{} of {}",
                    obj.short_description,
                    DRINKS[obj.get_obj_val(2) as usize]
                );
            } else {
                itemname = obj.short_description.clone();
            }
        }

        ITEM_WAND | ITEM_STAFF => {
            itemname = format!(
                "{}{}",
                obj.short_description,
                if obj.get_obj_val(2) < obj.get_obj_val(1) {
                    " (partially used)"
                } else {
                    ""
                }
            );
        }

        _ => {
            itemname = obj.short_description.clone();
        }
    }

    result.push_str(
        format!(
            " {:2})  {:9}   {:48} {:6}\r\n",
            aindex,
            quantity,
            itemname,
            buy_price(obj, shop, keeper, ch)
        )
        .as_str(),
    );
    result.clone()
}

pub fn shopping_list(
    game: &Game,
    arg: &str,
    ch: &Rc<CharData>,
    keeper: &Rc<CharData>,
    shop: &mut ShopData,
) {
    let db = &game.db;
    let mut cnt = 0;
    let mut lindex = 0;
    let mut found = false;
    let mut name = String::new();

    /* cnt is the number of that particular object available */

    if !is_ok(game, keeper, ch, shop) {
        return;
    }

    if shop.lastsort < keeper.is_carrying_n() as i32 {
        sort_keeper_objs(db, keeper, shop);
    }

    one_argument(arg, &mut name);

    let mut buf = String::from(" ##   Available   Item                                               Cost\r\n-------------------------------------------------------------------------\r\n");
    let mut last_obj: Option<Rc<ObjData>> = None;

    if keeper.carrying.borrow().len() != 0 {
        let cl = keeper.carrying.borrow();
        for obj in cl.iter() {
            if db.can_see_obj(ch, obj) && obj.get_obj_cost() > 0 {
                if last_obj.is_none() {
                    last_obj = Some(obj.clone());
                    cnt = 1;
                } else if last_obj.is_some() && same_obj(last_obj.as_ref().unwrap(), obj) {
                    cnt += 1;
                } else {
                    lindex += 1;
                    if name.is_empty() || isname(&name, &last_obj.as_ref().unwrap().name.borrow()) {
                        buf.push_str(&list_object(
                            db,
                            last_obj.as_ref().unwrap(),
                            cnt,
                            lindex,
                            shop,
                            keeper,
                            ch,
                        ));
                        found = true;
                    }
                    cnt = 1;
                    last_obj = Some(obj.clone());
                }
            }
        }
    }
    lindex += 1;
    if last_obj.is_none() {
        /* we actually have nothing in our list for sale, period */
        send_to_char(ch, "Currently, there is nothing for sale.\r\n");
    } else if !name.is_empty() && !found {
        /* nothing the char was looking for was found */
        send_to_char(ch, "Presently, none of those are for sale.\r\n");
    } else {
        if name.is_empty() || isname(&name, &last_obj.as_ref().unwrap().name.borrow()) {
            /* show last obj */
            buf.push_str(&list_object(
                db,
                last_obj.as_ref().unwrap(),
                cnt,
                lindex,
                shop,
                keeper,
                ch,
            ));
            page_string(ch.desc.borrow().as_ref(), &buf, true);
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

pub fn shop_keeper(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool {
    let db = &game.db;
    let mut b = db.shop_index.borrow_mut();
    let keeper = me
        .downcast_ref::<Rc<CharData>>()
        .expect("Unexpected type for Rc<CharData> in shop_keeper");
    let shop = b.iter_mut().find(|s| s.keeper == keeper.nr);

    if shop.is_none() {
        return false;
    }

    let mut shop = shop.unwrap().borrow_mut();

    // TODO implement spec for spec
    // if (SHOP_FUNC(shop_nr))	/* Check secondary function */
    // if ((SHOP_FUNC(shop_nr)) (ch, me, cmd, argument))
    // return (true);

    if Rc::ptr_eq(keeper, ch) {
        if cmd != 0 {
            shop.lastsort = 0;
        }
        return false;
    }

    if !ok_shop_room(shop, db.get_room_vnum(ch.in_room())) {
        return false;
    }

    if !keeper.awake() {
        return false;
    }

    if cmd_is(cmd, "steal") {
        let argm = format!("$N shouts '{}'", MSG_NO_STEAL_HERE);
        db.act(&argm, false, Some(ch), None, Some(keeper), TO_CHAR);
        do_action(
            game,
            keeper,
            &ch.get_name(),
            CMD_SLAP.load(Ordering::Relaxed),
            0,
        );
        return true;
    }

    if cmd_is(cmd, "buy") {
        shopping_buy(game, argument, ch, keeper, shop);
        return true;
    } else if cmd_is(cmd, "sell") {
        shopping_sell(game, argument, ch, keeper, shop);
        return true;
    } else if cmd_is(cmd, "value") {
        shopping_value(game, argument, ch, keeper, shop);
        return true;
    } else if cmd_is(cmd, "list") {
        shopping_list(game, argument, ch, keeper, shop);
        return true;
    }
    return false;
}

pub fn ok_damage_shopkeeper(game: &Game, ch: &Rc<CharData>, victim: &Rc<CharData>) -> bool {
    let db = &game.db;
    if !db.is_mob(victim)
        || db.mob_index[victim.get_mob_rnum() as usize].func.is_some()
            && db.mob_index[victim.get_mob_rnum() as usize].func.unwrap() as usize
                != shop_keeper as usize
    {
        return true;
    }

    /* Prevent "invincible" shopkeepers if they're charmed. */
    if victim.aff_flagged(AFF_CHARM) {
        return true;
    }

    for sindex in 0..db.shop_index.borrow().len() {
        if victim.get_mob_rnum() == db.shop_index.borrow()[sindex].keeper
            && !db.shop_index.borrow()[sindex].shop_kill_chars()
        {
            let buf = format!("{} {}", ch.get_name(), MSG_CANT_KILL_KEEPER);
            do_tell(game, victim, &buf, CMD_TELL.load(Ordering::Relaxed), 0);
            do_action(
                game,
                victim,
                &ch.get_name(),
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
        db.shop_index.borrow().len(),
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
        db.shop_index.borrow().len(),
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
    // int tindex, num, len = 0, error = 0;
    // char *ptr;
    // char buf[MAX_STRING_LENGTH];
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
    // int cht, ss = 0, ds = 0, err = 0;
    // char *tbuf;
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

impl DB {
    pub fn boot_the_shops(&mut self, shop_f: File, filename: &str, _rec_count: i32) {
        // char *buf, buf2[256];
        // int temp, count, new_format = FALSE;
        // struct ShopBuyData list[MAX_SHOP_OBJ + 1];
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
                };

                let mut list: Vec<ShopBuyData> = vec![];
                temp = read_list(
                    self,
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

                read_line_float(self, &mut reader, &mut shop.profit_buy);
                read_line_float(self, &mut reader, &mut shop.profit_sell);

                list.clear();
                temp = read_type_list(self, &mut reader, &mut list, new_format, MAX_TRADE) as i32;

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
                read_line_int(self, &mut reader, &mut shop.temper1);
                read_line_int(self, &mut reader, &mut shop.bitvector);
                let mut shop_keeper = NOBODY as i32;
                read_line_int(self, &mut reader, &mut shop_keeper);
                shop.keeper = self.real_mobile(shop_keeper as MobVnum);
                read_line_int(self, &mut reader, &mut shop.with_who);
                let mut list: Vec<ShopBuyData> = vec![];
                temp = read_list(self, &mut reader, &mut list, new_format, 1, LIST_ROOM) as i32;
                for count in 0..temp as usize {
                    shop.in_room.push(list[count].type_ as RoomVnum);
                }

                read_line_int(self, &mut reader, &mut shop.open1);
                read_line_int(self, &mut reader, &mut shop.close1);
                read_line_int(self, &mut reader, &mut shop.open2);
                read_line_int(self, &mut reader, &mut shop.close2);

                self.shop_index.borrow_mut().push(shop);
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

    for shop in db.shop_index.borrow_mut().iter_mut() {
        if shop.keeper == NOBODY {
            continue;
        }
        db.mob_index[shop.keeper as usize].func = Some(shop_keeper);
        // TODO func in func
        /* Having SHOP_FUNC() as 'shop_keeper' will cause infinite recursion. */
        // if (mob_index[SHOP_KEEPER(cindex)].func & & mob_index[SHOP_KEEPER(cindex)].func != shop_keeper)
        // SHOP_FUNC(cindex) = mob_index[SHOP_KEEPER(cindex)].func;
    }
    // TODO implement shopkeeper spec proc
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
fn list_all_shops(db: &DB, ch: &Rc<CharData>) {
    const LIST_ALL_SHOPS_HEADER: &str =
        " ##   Virtual   Where    Keeper    Buy   Sell   Customers\r\n\
---------------------------------------------------------\r\n";
    let mut buf = String::new();
    for (shop_nr, shop) in db.shop_index.borrow().iter().enumerate() {
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

    page_string(ch.desc.borrow().as_ref(), &buf, true);
}

fn list_detailed_shop(db: &DB, ch: &Rc<CharData>, shop_nr: i32) {
    let shop = &db.shop_index.borrow_mut()[shop_nr as usize];
    send_to_char(
        ch,
        format!(
            "Vnum:       [{:5}], Rnum: [{:5}]\r\n",
            shop.vnum,
            shop_nr + 1
        )
        .as_str(),
    );

    send_to_char(ch, "Rooms:      ");
    let mut column = 12; /* ^^^ strlen ^^^ */
    let mut count = 0;
    for sindex in 0..shop.in_room.len() {
        count += 1;
        if sindex != 0 {
            send_to_char(ch, ", ");
            column += 2;
        }
        let temp;
        let buf1;
        if {
            temp = db.real_room(shop.in_room[sindex]);
            temp != NOWHERE
        } {
            buf1 = format!(
                "{} (#{})",
                db.world.borrow()[temp as usize].name,
                db.get_room_vnum(temp)
            );
        } else {
            buf1 = format!("<UNKNOWN> (#{})", shop.in_room[sindex]);
        }

        /* Implementing word-wrapping: assumes screen-size == 80 */
        if buf1.len() + column >= 78 && column >= 20 {
            send_to_char(ch, "\r\n            ");
            /* 12 is to line up with "Rooms:" printed first, and spaces above. */
            column = 12;
        }

        send_to_char(ch, &buf1);

        column += buf1.len();
    }
    if count == 0 {
        send_to_char(ch, "Rooms:      None!");
    }

    send_to_char(ch, "\r\nShopkeeper: ");
    if shop.keeper != NOBODY {
        send_to_char(
            ch,
            format!(
                "{} (#{}), Special Function: {}\r\n",
                db.mob_protos[shop.keeper as usize].get_name(),
                db.mob_index[shop.keeper as usize].vnum,
                // TODO implement shop spec proc function
                yesno!(/*shop.function*/ false)
            )
            .as_str(),
        );
        let k;
        if {
            k = db.get_char_num(shop.keeper);
            k.is_some()
        } {
            let k = k.as_ref().unwrap();

            send_to_char(
                ch,
                format!(
                    "Coins:      [{:9}], Bank: [{:9}] (Total: {})\r\n",
                    k.get_gold(),
                    shop.bank_account,
                    k.get_gold() + shop.bank_account
                )
                .as_str(),
            );
        } else {
            send_to_char(ch, "<NONE>\r\n");
        }
    }
    let ptrsave;
    send_to_char(
        ch,
        format!(
            "Customers:  {}\r\n",
            if {
                ptrsave = customer_string(shop, true);
                !ptrsave.is_empty()
            } {
                ptrsave
            } else {
                "None".to_string()
            }
        )
        .as_str(),
    );

    send_to_char(ch, "Produces:   ");
    let mut column = 12; /* ^^^ strlen ^^^ */
    let mut sindex = 0;
    let mut buf1 = String::new();
    let mut count = 0;
    while shop.shop_product(sindex) != NOTHING {
        count += 1;
        if sindex != 0 {
            send_to_char(ch, ", ");
            column += 2;
        }
        let nbuf = format!(
            "{} (#{})",
            db.obj_proto[shop.shop_product(sindex) as usize].short_description,
            db.obj_index[shop.shop_product(sindex) as usize].vnum
        );
        buf1.push_str(&nbuf);
        /* Implementing word-wrapping: assumes screen-size == 80 */
        if nbuf.len() + column >= 78 && column >= 20 {
            send_to_char(ch, "\r\n            ");
            /* 12 is to line up with "Produces:" printed first, and spaces above. */
            column = 12;
        }

        send_to_char(ch, &buf1);
        buf1.clear();
        column += nbuf.len();
        sindex += 1;
    }
    if count == 0 {
        send_to_char(ch, "Produces:   Nothing!");
    }

    send_to_char(ch, "\r\nBuys:       ");
    let mut column = 12; /* ^^^ strlen ^^^ */

    sindex = 0;
    count = 0;
    while shop.type_[sindex as usize].type_ != NOTHING as i32 {
        count += 1;

        let buf1;
        if sindex != 0 {
            send_to_char(ch, ", ");
            column += 2;
        }

        buf1 = format!(
            "{} (#{}) [{}]",
            ITEM_TYPES[shop.type_[sindex as usize].type_ as usize],
            shop.type_[sindex as usize].type_,
            if !shop.type_[sindex as usize].keywords.is_empty() {
                shop.type_[sindex as usize].keywords.clone()
            } else {
                Rc::from("all")
            }
        );

        /* Implementing word-wrapping: assumes screen-size == 80 */
        if buf1.len() + column >= 78 && column >= 20 {
            send_to_char(ch, "\r\n            ");
            /* 12 is to line up with "Buys:" printed first, and spaces above. */
            column = 12;
        }

        send_to_char(ch, &buf1);

        column += buf1.len();
        sindex += 1;
    }
    if count == 0 {
        send_to_char(ch, "Buys:       Nothing!");
    }

    send_to_char(
        ch,
        format!(
            "\r\nBuy at:     [{:6}], Sell at: [{:6}], Open: [{}-{}, {}-{}]\r\n",
            shop.profit_sell, shop.profit_buy, shop.open1, shop.close1, shop.open2, shop.close2
        )
        .as_str(),
    );

    /* Need a local buffer. */
    let mut buf1 = String::new();
    sprintbit(shop.bitvector as i64, &SHOP_BITS, &mut buf1);
    send_to_char(ch, format!("Bits:       {}\r\n", buf1).as_str());
}

pub fn show_shops(db: &DB, ch: &Rc<CharData>, arg: &str) {
    if arg.is_empty() {
        list_all_shops(db, ch);
    } else {
        let mut shop_nr = None;
        if arg == "." {
            for (i, shop) in db.shop_index.borrow().iter().enumerate() {
                if ok_shop_room(shop, db.get_room_vnum(ch.in_room())) {
                    shop_nr = Some(i as i32);
                    break;
                }
            }

            if shop_nr.is_none() {
                send_to_char(ch, "This isn't a shop!\r\n");
                return;
            }
        } else if is_number(arg) {
            let ap = arg.parse::<i32>();
            if ap.is_ok() {
                shop_nr = Some(ap.unwrap() - 1);
            }
        }
        if shop_nr.is_none() {
            send_to_char(ch, "Illegal shop number.\r\n");
            return;
        }
        list_detailed_shop(db, ch, shop_nr.unwrap());
    }
}

// void destroy_shops(void)
// {
// ssize_t cnt, itr;
//
// if (!shop_index)
// return;
//
// for (cnt = 0; cnt <= top_shop; cnt++) {
// if (shop_index[cnt].no_such_item1)
// free(shop_index[cnt].no_such_item1);
// if (shop_index[cnt].no_such_item2)
// free(shop_index[cnt].no_such_item2);
// if (shop_index[cnt].missing_cash1)
// free(shop_index[cnt].missing_cash1);
// if (shop_index[cnt].missing_cash2)
// free(shop_index[cnt].missing_cash2);
// if (shop_index[cnt].do_not_buy)
// free(shop_index[cnt].do_not_buy);
// if (shop_index[cnt].message_buy)
// free(shop_index[cnt].message_buy);
// if (shop_index[cnt].message_sell)
// free(shop_index[cnt].message_sell);
// if (shop_index[cnt].in_room)
// free(shop_index[cnt].in_room);
// if (shop_index[cnt].producing)
// free(shop_index[cnt].producing);
//
// if (shop_index[cnt].type) {
// for (itr = 0; BUY_TYPE(shop_index[cnt].type[itr]) != NOTHING; itr++)
// if (BUY_WORD(shop_index[cnt].type[itr]))
// free(BUY_WORD(shop_index[cnt].type[itr]));
// free(shop_index[cnt].type);
// }
// }
//
// free(shop_index);
// shop_index = NULL;
// top_shop = -1;
// }
