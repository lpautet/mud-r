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
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::error;
use regex::Regex;

use crate::act_comm::{do_say, do_tell};
use crate::act_social::do_action;
use crate::constants::{DRINKS, ITEM_TYPES};
use crate::db::{fread_string, DB, REAL};
use crate::handler::{fname, get_number, isname, obj_from_char};
use crate::interpreter::{cmd_is, find_command, is_number, one_argument};
use crate::modify::page_string;
use crate::structs::{
    CharData, MobRnum, MobVnum, ObjData, ObjVnum, RoomRnum, RoomVnum, ITEM_DRINKCON, ITEM_STAFF,
    ITEM_WAND, LVL_GOD, MAX_OBJ_AFFECT, NOBODY, NOTHING,
};
use crate::util::get_line;
use crate::{an, is_set, send_to_char, Game, TO_CHAR, TO_ROOM};

pub struct ShopBuyData {
    pub type_: i32,
    pub keywords: Rc<str>,
}

impl ShopBuyData {
    pub fn buy_type(&self) -> i32 {
        self.type_
    }
    pub fn buy_word(&self) -> &str {
        self.keywords.as_ref()
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
    //SPECIAL (*func);		/* Secondary spec_proc for shopkeeper	*/
}

//
//
const MAX_TRADE: i32 = 5; /* List maximums for compatibility	*/
const MAX_PROD: i32 = 5; /*	with shops before v3.0		*/
const VERSION3_TAG: &str = "v3.0"; /* The file has v3.0 shops in it!	*/
const MAX_SHOP_OBJ: i32 = 100; /* "Soft" maximum for list maximums	*/
//
//
// /* Pretty general macros that could be used elsewhere */
// #define IS_GOD(ch)		(!IS_NPC(ch) && (GET_LEVEL(ch) >= LVL_GOD))
// #define END_OF(buffer)		((buffer) + strlen((buffer)))
//
//
// /* Possible states for objects trying to be sold */
// #define OBJECT_DEAD		0
// #define OBJECT_NOTOK		1
// #define OBJECT_OK		2
// #define OBJECT_NOVAL		3

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

// struct stack_data {
//     int data[100];
//     int len;
// } ;
//
// #define S_DATA(stack, index)	((stack)->data[(index)])
// #define S_LEN(stack)		((stack)->len)
//
//
// /* Which expression type we are now parsing */
// #define OPER_OPEN_PAREN		0
// #define OPER_CLOSE_PAREN	1
// #define OPER_OR			2
// #define OPER_AND		3
// #define OPER_NOT		4
// #define MAX_OPER		4
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
//
//
// /* ************************************************************************
// *   File: shop.c                                        Part of CircleMUD *
// *  Usage: shopkeepers: loading config files, spec procs.                  *
// *                                                                         *
// *  All rights reserved.  See license.doc for complete information.        *
// *                                                                         *
// *  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
// *  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
// ************************************************************************ */
//
// /***
//  * The entire shop rewrite for Circle 3.0 was done by Jeff Fink.  Thanks Jeff!
//  ***/
//
// #include "conf.h"
// #include "sysdep.h"
//
// #include "structs.h"
// #include "comm.h"
// #include "handler.h"
// #include "db.h"
// #include "interpreter.h"
// #include "utils.h"
// #include "shop.h"
// #include "constants.h"
//
// /* External variables */
// extern struct time_info_data time_info;
//
// /* Forward/External function declarations */
// ACMD(do_tell);
// ACMD(do_action);
// ACMD(do_echo);
// ACMD(do_say);
// void sort_keeper_objs(struct char_data *keeper, int shop_nr);
//
// /* Local variables */
// struct ShopData *shop_index;
// int top_shop = -1;
// int cmd_say, cmd_tell, cmd_emote, cmd_slap, cmd_puke;
//
// /* local functions */
// char *read_shop_message(int mnum, room_vnum shr, FILE *shop_f, const char *why);
// int read_type_list(FILE *shop_f, struct ShopBuyData *list, int new_format, int max);
// int read_list(FILE *shop_f, struct ShopBuyData *list, int new_format, int max, int type);
// void shopping_list(char *arg, struct char_data *ch, struct char_data *keeper, int shop_nr);
// void shopping_value(char *arg, struct char_data *ch, struct char_data *keeper, int shop_nr);
// void shopping_sell(char *arg, struct char_data *ch, struct char_data *keeper, int shop_nr);
// struct obj_data *get_selling_obj(struct char_data *ch, char *name, struct char_data *keeper, int shop_nr, int msg);
// struct obj_data *slide_obj(struct obj_data *obj, struct char_data *keeper, int shop_nr);
// void shopping_buy(char *arg, struct char_data *ch, struct char_data *keeper, int shop_nr);
// struct obj_data *get_purchase_obj(struct char_data *ch, char *arg, struct char_data *keeper, int shop_nr, int msg);
// struct obj_data *get_hash_obj_vis(struct char_data *ch, char *name, struct obj_data *list);
// struct obj_data *get_slide_obj_vis(struct char_data *ch, char *name, struct obj_data *list);
// void boot_the_shops(FILE *shop_f, char *filename, int rec_count);
// void assign_the_shopkeepers(void);
// char *customer_string(int shop_nr, int detailed);
// void list_all_shops(struct char_data *ch);
// void list_detailed_shop(struct char_data *ch, int shop_nr);
// void show_shops(struct char_data *ch, char *arg);
// int is_ok_char(struct char_data *keeper, struct char_data *ch, int shop_nr);
// int is_open(struct char_data *keeper, int shop_nr, int msg);
// int is_ok(struct char_data *keeper, struct char_data *ch, int shop_nr);
// void push(struct stack_data *stack, int pushval);
// int top(struct stack_data *stack);
// int pop(struct stack_data *stack);
// void evaluate_operation(struct stack_data *ops, struct stack_data *vals);
// int find_oper_num(char token);
// int evaluate_expression(struct obj_data *obj, char *expr);
// int trade_with(struct obj_data *item, int shop_nr);
// int same_obj(struct obj_data *obj1, struct obj_data *obj2);
// int shop_producing(struct obj_data *item, int shop_nr);
// int transaction_amt(char *arg);
// char *times_message(struct obj_data *obj, char *name, int num);
// int buy_price(struct obj_data *obj, int shop_nr, struct char_data *keeper, struct char_data *buyer);
// int sell_price(struct obj_data *obj, int shop_nr, struct char_data *keeper, struct char_data *seller);
// char *list_object(struct obj_data *obj, int cnt, int oindex, int shop_nr, struct char_data *keeper, struct char_data *seller);
// int ok_shop_room(int shop_nr, room_vnum room);
// SPECIAL(shop_keeper);
// int ok_damage_shopkeeper(struct char_data *ch, struct char_data *victim);
// int add_to_list(struct ShopBuyData *list, int type, int *len, int *val);
// int end_read_list(struct ShopBuyData *list, int len, int error);
// void read_line(FILE *shop_f, const char *string, void *data);
// void destroy_shops(void);
//
//
// /* config arrays */
// const char *operator_str[] = {
// "[({",
// "])}",
// "|+",
// "&*",
// "^'"
// } ;
//
// /* Constant list for printing out who we sell to */
// const char *trade_letters[] = {
// "Good",                 /* First, the alignment based ones */
// "Evil",
// "Neutral",
// "Magic User",           /* Then the class based ones */
// "Cleric",
// "Thief",
// "Warrior",
// "\n"
// };
//
//
// const char *shop_bits[] = {
// "WILL_FIGHT",
// "USES_BANK",
// "\n"
// };

fn is_ok_char(game: &Game, keeper: &Rc<CharData>, ch: &Rc<CharData>, shop: &ShopData) -> bool {
    // char buf[MAX_INPUT_LENGTH];
    let db = &game.db;
    if !db.can_see(keeper, ch) {
        //do_say(MSG_NO_SEE_CHAR, actbuf, cmd_say, 0);
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
        do_tell(game, keeper, &buf, cmd_tell.load(Ordering::Relaxed), 0);
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
        do_tell(game, keeper, &buf, cmd_tell.load(Ordering::Relaxed), 0);
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
        do_say(game, keeper, &buf, cmd_tell.load(Ordering::Relaxed), 0);
    }
    false
}

fn is_ok(game: &Game, keeper: &Rc<CharData>, ch: &Rc<CharData>, shop: &ShopData) -> bool {
    if is_open(game, keeper, shop, true) {
        return is_ok_char(game, keeper, ch, shop);
    }
    false
}

// void push(struct stack_data *stack, int pushval)
// {
// S_DATA(stack, S_LEN(stack)++) = pushval;
// }
//
//
// int top(struct stack_data *stack)
// {
// if (S_LEN(stack) > 0)
// return (S_DATA(stack, S_LEN(stack) - 1));
// else
// return (NOTHING);
// }
//
//
// int pop(struct stack_data *stack)
// {
// if (S_LEN(stack) > 0)
// return (S_DATA(stack, --S_LEN(stack)));
// else {
// log("SYSERR: Illegal expression %d in shop keyword list.", S_LEN(stack));
// return (0);
// }
// }
//
//
// void evaluate_operation(struct stack_data *ops, struct stack_data *vals)
// {
// int oper;
//
// if ((oper = pop(ops)) == OPER_NOT)
// push(vals, !pop(vals));
// else {
// int val1 = pop(vals),
// val2 = pop(vals);
//
// /* Compiler would previously short-circuit these. */
// if (oper == OPER_AND)
// push(vals, val1 && val2);
// else if (oper == OPER_OR)
// push(vals, val1 || val2);
// }
// }
//
//
// int find_oper_num(char token)
// {
// int oindex;
//
// for (oindex = 0; oindex <= MAX_OPER; oindex++)
// if (strchr(operator_str[oindex], token))
// return (oindex);
// return (NOTHING);
// }
//
//
// int evaluate_expression(struct obj_data *obj, char *expr)
// {
// struct stack_data ops, vals;
// char *ptr, *end, name[MAX_STRING_LENGTH];
// int temp, eindex;
//
// if (!expr || !*expr)	/* Allows opening ( first. */
// return (TRUE);
//
// ops.len = vals.len = 0;
// ptr = expr;
// while (*ptr) {
// if (isspace(*ptr))
// ptr++;
// else {
// if ((temp = find_oper_num(*ptr)) == NOTHING) {
// end = ptr;
// while (*ptr && !isspace(*ptr) && find_oper_num(*ptr) == NOTHING)
// ptr++;
// strncpy(name, end, ptr - end);	/* strncpy: OK (name/end:MAX_STRING_LENGTH) */
// name[ptr - end] = '\0';
// for (eindex = 0; *extra_bits[eindex] != '\n'; eindex++)
// if (!str_cmp(name, extra_bits[eindex])) {
// push(&vals, OBJ_FLAGGED(obj, 1 << eindex));
// break;
// }
// if (*extra_bits[eindex] == '\n')
// push(&vals, isname(name, obj->name));
// } else {
// if (temp != OPER_OPEN_PAREN)
// while (top(&ops) > temp)
// evaluate_operation(&ops, &vals);
//
// if (temp == OPER_CLOSE_PAREN) {
// if ((temp = pop(&ops)) != OPER_OPEN_PAREN) {
// log("SYSERR: Illegal parenthesis in shop keyword expression.");
// return (FALSE);
// }
// } else
// push(&ops, temp);
// ptr++;
// }
// }
// }
// while (top(&ops) != NOTHING)
// evaluate_operation(&ops, &vals);
// temp = pop(&vals);
// if (top(&vals) != NOTHING) {
// log("SYSERR: Extra operands left on shop keyword expression stack.");
// return (FALSE);
// }
// return (temp);
// }
//
//
// int trade_with(struct obj_data *item, int shop_nr)
// {
// int counter;
//
// if (GET_OBJ_COST(item) < 1)
// return (OBJECT_NOVAL);
//
// if (OBJ_FLAGGED(item, ITEM_NOSELL))
// return (OBJECT_NOTOK);
//
// for (counter = 0; SHOP_BUYTYPE(shop_nr, counter) != NOTHING; counter++)
// if (SHOP_BUYTYPE(shop_nr, counter) == GET_OBJ_TYPE(item)) {
// if (GET_OBJ_VAL(item, 2) == 0 &&
// (GET_OBJ_TYPE(item) == ITEM_WAND ||
// GET_OBJ_TYPE(item) == ITEM_STAFF))
// return (OBJECT_DEAD);
// else if (evaluate_expression(item, SHOP_BUYWORD(shop_nr, counter)))
// return (OBJECT_OK);
// }
// return (OBJECT_NOTOK);
// }

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
    db: &DB,
    ch: &Rc<CharData>,
    arg: &str,
    keeper: &Rc<CharData>,
    show: &ShopData,
    msg: bool,
) -> Option<Rc<ObjData>> {
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
                // TODO implement do_tell
                // shop_index[shop_nr].no_such_item1, GET_NAME(ch));
                // do_tell(keeper, buf, cmd_tell, 0);
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

// /*
//  * When the shopkeeper is buying, we reverse the discount. Also make sure
//  * we don't buy for more than we sell for, to prevent infinite money-making.
//  */
// int sell_price(struct obj_data *obj, int shop_nr, struct char_data *keeper, struct char_data *seller)
// {
// float sell_cost_modifier = SHOP_SELLPROFIT(shop_nr) * (1 - (GET_CHA(keeper) - GET_CHA(seller)) / (float)70);
// float buy_cost_modifier = SHOP_BUYPROFIT(shop_nr) * (1 + (GET_CHA(keeper) - GET_CHA(seller)) / (float)70);
//
// if (sell_cost_modifier > buy_cost_modifier)
// sell_cost_modifier = buy_cost_modifier;
//
// return (int) (GET_OBJ_COST(obj) * sell_cost_modifier);
// }

fn shopping_buy(
    game: &Game,
    arg: &str,
    ch: &Rc<CharData>,
    keeper: &Rc<CharData>,
    shop: &mut ShopData,
) {
    let db = &game.db;
    // char tempstr[MAX_INPUT_LENGTH], tempbuf[MAX_INPUT_LENGTH];
    // struct obj_data *obj, *last_obj = NULL;
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
        do_tell(game, keeper, &buf, cmd_tell.load(Ordering::Relaxed), 0);
        return;
    }
    if arg.is_empty() || buynum == 0 {
        let buf = format!("{} What do you want to buy??", ch.get_name());
        do_tell(game, keeper, &buf, cmd_tell.load(Ordering::Relaxed), 0);
        return;
    }
    let mut obj: Option<Rc<ObjData>>;
    if {
        obj = get_purchase_obj(db, ch, &arg, keeper, shop, true);
        obj.is_none()
    } {
        return;
    }
    if buy_price(obj.as_ref().unwrap(), shop, keeper, ch) > ch.get_gold() && !ch.is_god() {
        // TODO implement do_tell
        // snprintf(actbuf, sizeof(actbuf), shop_index[shop_nr].missing_cash2, GET_NAME(ch));
        // do_tell(keeper, actbuf, cmd_tell, 0);

        match shop.temper1 {
            0 => {
                do_action(
                    game,
                    keeper,
                    &ch.get_name(),
                    cmd_puke.load(Ordering::Relaxed),
                    0,
                );
            }

            1 => {
                // TODO implement do_echo
                // do_echo(keeper, strcpy(actbuf,
                //                        "smokes on his joint."), cmd_emote, SCMD_EMOTE);    /* strcpy: OK */
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
        obj = get_purchase_obj(db, ch, &arg, keeper, shop, false);
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
        do_tell(game, keeper, &buf, cmd_tell.load(Ordering::Relaxed), 0);
    }
    if !ch.is_god() {
        keeper.set_gold(keeper.get_gold() + goldamt);
    }

    let tempstr = times_message(Some(&ch.carrying.borrow()[0]), "", bought);

    let tempbuf = format!("$n buys {}.", tempstr);
    db.act(&tempbuf, false, Some(ch), obj.as_ref(), None, TO_ROOM);

    // TODO implement do_tell
    // snprintf(tempbuf, sizeof(tempbuf), shop_index[shop_nr].message_buy, GET_NAME(ch), goldamt);
    // do_tell(keeper, tempbuf, cmd_tell, 0);

    send_to_char(ch, format!("You now have {}.\r\n", tempstr).as_str());

    if shop.shop_uses_bank() {
        if keeper.get_gold() > MAX_OUTSIDE_BANK {
            shop.bank_account += keeper.get_gold() - MAX_OUTSIDE_BANK;
            keeper.set_gold(MAX_OUTSIDE_BANK);
        }
    }
}

// struct obj_data *get_selling_obj(struct char_data *ch, char *name, struct char_data *keeper, int shop_nr, int msg)
// {
// char buf[MAX_INPUT_LENGTH];
// struct obj_data *obj;
// int result;
//
// if (!(obj = get_obj_in_list_vis(ch, name, NULL, ch->carrying))) {
// if (msg) {
// char tbuf[MAX_INPUT_LENGTH];
//
// snprintf(tbuf, sizeof(tbuf), shop_index[shop_nr].no_such_item2, GET_NAME(ch));
// do_tell(keeper, tbuf, cmd_tell, 0);
// }
// return (NULL);
// }
// if ((result = trade_with(obj, shop_nr)) == OBJECT_OK)
// return (obj);
//
// if (!msg)
// return (0);
//
// switch (result) {
// case OBJECT_NOVAL:
// snprintf(buf, sizeof(buf), "%s You've got to be kidding, that thing is worthless!", GET_NAME(ch));
// break;
// case OBJECT_NOTOK:
// snprintf(buf, sizeof(buf), shop_index[shop_nr].do_not_buy, GET_NAME(ch));
// break;
// case OBJECT_DEAD:
// snprintf(buf, sizeof(buf), "%s %s", GET_NAME(ch), MSG_NO_USED_WANDSTAFF);
// break;
// default:
// log("SYSERR: Illegal return value of %d from trade_with() (%s)", result, __FILE__);	/* Someone might rename it... */
// snprintf(buf, sizeof(buf), "%s An error has occurred.", GET_NAME(ch));
// break;
// }
// do_tell(keeper, buf, cmd_tell, 0);
// return (NULL);
// }

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

// void shopping_sell(char *arg, struct char_data *ch, struct char_data *keeper, int shop_nr)
// {
// char tempstr[MAX_INPUT_LENGTH], name[MAX_INPUT_LENGTH], tempbuf[MAX_INPUT_LENGTH];
// struct obj_data *obj;
// int sellnum, sold = 0, goldamt = 0;
//
// if (!(is_ok(keeper, ch, shop_nr)))
// return;
//
// if ((sellnum = transaction_amt(arg)) < 0) {
// char buf[MAX_INPUT_LENGTH];
//
// snprintf(buf, sizeof(buf), "%s A negative amount?  Try buying something.", GET_NAME(ch));
// do_tell(keeper, buf, cmd_tell, 0);
// return;
// }
// if (!*arg || !sellnum) {
// char buf[MAX_INPUT_LENGTH];
//
// snprintf(buf, sizeof(buf), "%s What do you want to sell??", GET_NAME(ch));
// do_tell(keeper, buf, cmd_tell, 0);
// return;
// }
// one_argument(arg, name);
// if (!(obj = get_selling_obj(ch, name, keeper, shop_nr, TRUE)))
// return;
//
// if (GET_GOLD(keeper) + SHOP_BANK(shop_nr) < sell_price(obj, shop_nr, keeper, ch)) {
// char buf[MAX_INPUT_LENGTH];
//
// snprintf(buf, sizeof(buf), shop_index[shop_nr].missing_cash1, GET_NAME(ch));
// do_tell(keeper, buf, cmd_tell, 0);
// return;
// }
// while (obj && GET_GOLD(keeper) + SHOP_BANK(shop_nr) >= sell_price(obj, shop_nr, keeper, ch) && sold < sellnum) {
// int charged = sell_price(obj, shop_nr, keeper, ch);
//
// goldamt += charged;
// GET_GOLD(keeper) -= charged;
//
// sold++;
// obj_from_char(obj);
// slide_obj(obj, keeper, shop_nr);	/* Seems we don't use return value. */
// obj = get_selling_obj(ch, name, keeper, shop_nr, FALSE);
// }
//
// if (sold < sellnum) {
// char buf[MAX_INPUT_LENGTH];
//
// if (!obj)
// snprintf(buf, sizeof(buf), "%s You only have %d of those.", GET_NAME(ch), sold);
// else if (GET_GOLD(keeper) + SHOP_BANK(shop_nr) < sell_price(obj, shop_nr, keeper, ch))
// snprintf(buf, sizeof(buf), "%s I can only afford to buy %d of those.", GET_NAME(ch), sold);
// else
// snprintf(buf, sizeof(buf), "%s Something really screwy made me buy %d.", GET_NAME(ch), sold);
//
// do_tell(keeper, buf, cmd_tell, 0);
// }
// GET_GOLD(ch) += goldamt;
//
// strlcpy(tempstr, times_message(0, name, sold), sizeof(tempstr));
// snprintf(tempbuf, sizeof(tempbuf), "$n sells %s.", tempstr);
// act(tempbuf, FALSE, ch, obj, 0, TO_ROOM);
//
// snprintf(tempbuf, sizeof(tempbuf), shop_index[shop_nr].message_sell, GET_NAME(ch), goldamt);
// do_tell(keeper, tempbuf, cmd_tell, 0);
//
// send_to_char(ch, "The shopkeeper now has %s.\r\n", tempstr);
//
// if (GET_GOLD(keeper) < MIN_OUTSIDE_BANK) {
// goldamt = MIN(MAX_OUTSIDE_BANK - GET_GOLD(keeper), SHOP_BANK(shop_nr));
// SHOP_BANK(shop_nr) -= goldamt;
// GET_GOLD(keeper) += goldamt;
// }
// }
//
//
// void shopping_value(char *arg, struct char_data *ch, struct char_data *keeper, int shop_nr)
// {
// char buf[MAX_STRING_LENGTH], name[MAX_INPUT_LENGTH];
// struct obj_data *obj;
//
// if (!is_ok(keeper, ch, shop_nr))
// return;
//
// if (!*arg) {
// snprintf(buf, sizeof(buf), "%s What do you want me to evaluate??", GET_NAME(ch));
// do_tell(keeper, buf, cmd_tell, 0);
// return;
// }
// one_argument(arg, name);
// if (!(obj = get_selling_obj(ch, name, keeper, shop_nr, TRUE)))
// return;
//
// snprintf(buf, sizeof(buf), "%s I'll give you %d gold coins for that!", GET_NAME(ch), sell_price(obj, shop_nr, keeper, ch));
// do_tell(keeper, buf, cmd_tell, 0);
// }

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
        quantity.push_str("Unlimited"); /* strcpy: OK (for 'quantity >= 10') */
    } else {
        quantity.push_str(format!("{}", cnt).as_str()); /* sprintf: OK (for 'quantity >= 11', 32-bit int) */
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

    // if (SHOP_FUNC(shop_nr))	/* Check secondary function */
    // if ((SHOP_FUNC(shop_nr)) (ch, me, cmd, argument))
    // return (TRUE);

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
            cmd_slap.load(Ordering::Relaxed),
            0,
        );
        return true;
    }

    if cmd_is(cmd, "buy") {
        shopping_buy(game, argument, ch, keeper, shop);
        return true;
        // } else if cmd_si(cmd, "sell") {
        //     shopping_sell(argument, ch, keeper, shop);
        //     return true;
        // } else if cmd_is(cmd, "value") {
        //     shopping_value(argument, ch, keeper, shop);
        //     return true;
    } else if cmd_is(cmd, "list") {
        shopping_list(game, argument, ch, keeper, shop);
        return true;
    }
    return false;
}

// int ok_damage_shopkeeper(struct char_data *ch, struct char_data *victim)
// {
// int sindex;
//
// if (!IS_MOB(victim) || mob_index[GET_MOB_RNUM(victim)].func != shop_keeper)
// return (TRUE);
//
// /* Prevent "invincible" shopkeepers if they're charmed. */
// if (AFF_FLAGGED(victim, AFF_CHARM))
// return (TRUE);
//
// for (sindex = 0; sindex <= top_shop; sindex++)
// if (GET_MOB_RNUM(victim) == SHOP_KEEPER(sindex) && !SHOP_KILL_CHARS(sindex)) {
// char buf[MAX_INPUT_LENGTH];
//
// snprintf(buf, sizeof(buf), "%s %s", GET_NAME(ch), MSG_CANT_KILL_KEEPER);
// do_tell(victim, buf, cmd_tell, 0);
//
// do_action(victim, GET_NAME(ch), cmd_slap, 0);
// return (FALSE);
// }
//
// return (TRUE);
// }
//
//
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
    // if (error)
    // log("SYSERR: Raise MAX_SHOP_OBJ constant in shop.h to %d", len + error);
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
        for count in 0..max {
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
            // info!("{}", buf);
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
        //     sscanf(buf, "%d", &num);
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
            error!("SYSERR: Shop #{} has %d before %s, message #{}.", shr, mnum);
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
            "SYSERR: Shop #{} has too many specifiers for message #{}. %s={} %d={}",
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
    pub fn boot_the_shops(&mut self, shop_f: File, filename: &str, rec_count: i32) {
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
                    // info!(
                    //     "{} {} {} ",
                    //     shop.vnum,
                    //     count,
                    //     list[count as usize].buy_type()
                    // );
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

static cmd_say: AtomicUsize = AtomicUsize::new(0);
static cmd_tell: AtomicUsize = AtomicUsize::new(0);
// static cmd_emote: Cell<usize> = Cell::new(0);
static cmd_slap: AtomicUsize = AtomicUsize::new(0);
static cmd_puke: AtomicUsize = AtomicUsize::new(0);

pub fn assign_the_shopkeepers(db: &mut DB) {
    // TODO implement emote
    cmd_say.store(find_command("say").unwrap(), Ordering::Relaxed);
    cmd_tell.store(find_command("tell").unwrap(), Ordering::Relaxed);
    // cmd_emote = find_command("emote");
    cmd_slap.store(find_command("slap").unwrap(), Ordering::Relaxed);
    cmd_puke.store(find_command("puke").unwrap(), Ordering::Relaxed);

    for shop in db.shop_index.borrow_mut().iter_mut() {
        if shop.keeper == NOBODY {
            continue;
        }
        db.mob_index[shop.keeper as usize].func = Some(shop_keeper);
        /* Having SHOP_FUNC() as 'shop_keeper' will cause infinite recursion. */
        // if (mob_index[SHOP_KEEPER(cindex)].func & & mob_index[SHOP_KEEPER(cindex)].func != shop_keeper)
        // SHOP_FUNC(cindex) = mob_index[SHOP_KEEPER(cindex)].func;
    }
    // TODO implement shopkeeper spec proc
}

// char *customer_string(int shop_nr, int detailed)
// {
// int sindex = 0, flag = 1, nlen;
// size_t len = 0;
// static char buf[256];
//
// while (*trade_letters[sindex] != '\n' && len + 1 < sizeof(buf)) {
// if (detailed) {
// if (!IS_SET(flag, SHOP_TRADE_WITH(shop_nr))) {
// nlen = snprintf(buf + len, sizeof(buf) - len, ", %s", trade_letters[sindex]);
//
// if (len + nlen >= sizeof(buf) || nlen < 0)
// break;
//
// len += nlen;
// }
// } else {
// buf[len++] = (IS_SET(flag, SHOP_TRADE_WITH(shop_nr)) ? '_' : *trade_letters[sindex]);
// buf[len] = '\0';
//
// if (len >= sizeof(buf))
// break;
// }
//
// sindex++;
// flag <<= 1;
// }
//
// buf[sizeof(buf) - 1] = '\0';
// return (buf);
// }
//
//
// /* END_OF inefficient */
// void list_all_shops(struct char_data *ch)
// {
// const char *list_all_shops_header =
// " ##   Virtual   Where    Keeper    Buy   Sell   Customers\r\n"
// "---------------------------------------------------------\r\n";
// int shop_nr, headerlen = strlen(list_all_shops_header);
// size_t len = 0;
// char buf[MAX_STRING_LENGTH], buf1[16];
//
// *buf = '\0';
// for (shop_nr = 0; shop_nr <= top_shop && len < sizeof(buf); shop_nr++) {
// /* New page in page_string() mechanism, print the header again. */
// if (!(shop_nr % (PAGE_LENGTH - 2))) {
// /*
//  * If we don't have enough room for the header, or all we have room left
//  * for is the header, then don't add it and just quit now.
//  */
// if (len + headerlen + 1 >= sizeof(buf))
// break;
// strcpy(buf + len, list_all_shops_header);	/* strcpy: OK (length checked above) */
// len += headerlen;
// }
//
// if (SHOP_KEEPER(shop_nr) == NOBODY)
// strcpy(buf1, "<NONE>");	/* strcpy: OK (for 'buf1 >= 7') */
// else
// sprintf(buf1, "%6d", mob_index[SHOP_KEEPER(shop_nr)].vnum);	/* sprintf: OK (for 'buf1 >= 11', 32-bit int) */
//
// len += snprintf(buf + len, sizeof(buf) - len,
// "%3d   %6d   %6d    %s   %3.2f   %3.2f    %s\r\n",
// shop_nr + 1, SHOP_NUM(shop_nr), SHOP_ROOM(shop_nr, 0), buf1,
// SHOP_SELLPROFIT(shop_nr), SHOP_BUYPROFIT(shop_nr),
// customer_string(shop_nr, FALSE));
// }
//
// page_string(ch->desc, buf, TRUE);
// }
//
//
// void list_detailed_shop(struct char_data *ch, int shop_nr)
// {
// struct char_data *k;
// int sindex, column;
// char *ptrsave;
//
// send_to_char(ch, "Vnum:       [%5d], Rnum: [%5d]\r\n", SHOP_NUM(shop_nr), shop_nr + 1);
//
//
// send_to_char(ch, "Rooms:      ");
// column = 12;	/* ^^^ strlen ^^^ */
// for (sindex = 0; SHOP_ROOM(shop_nr, sindex) != NOWHERE; sindex++) {
// char buf1[128];
// int linelen, temp;
//
// if (sindex) {
// send_to_char(ch, ", ");
// column += 2;
// }
//
// if ((temp = real_room(SHOP_ROOM(shop_nr, sindex))) != NOWHERE)
// linelen = snprintf(buf1, sizeof(buf1), "%s (#%d)", world[temp].name, GET_ROOM_VNUM(temp));
// else
// linelen = snprintf(buf1, sizeof(buf1), "<UNKNOWN> (#%d)", SHOP_ROOM(shop_nr, sindex));
//
// /* Implementing word-wrapping: assumes screen-size == 80 */
// if (linelen + column >= 78 && column >= 20) {
// send_to_char(ch, "\r\n            ");
// /* 12 is to line up with "Rooms:" printed first, and spaces above. */
// column = 12;
// }
//
// if (!send_to_char(ch, "%s", buf1))
// return;
// column += linelen;
// }
// if (!sindex)
// send_to_char(ch, "Rooms:      None!");
//
// send_to_char(ch, "\r\nShopkeeper: ");
// if (SHOP_KEEPER(shop_nr) != NOBODY) {
// send_to_char(ch, "%s (#%d), Special Function: %s\r\n",
// GET_NAME(&mob_proto[SHOP_KEEPER(shop_nr)]),
// mob_index[SHOP_KEEPER(shop_nr)].vnum,
// YESNO(SHOP_FUNC(shop_nr)));
//
// if ((k = get_char_num(SHOP_KEEPER(shop_nr))))
// send_to_char(ch, "Coins:      [%9d], Bank: [%9d] (Total: %d)\r\n",
// GET_GOLD(k), SHOP_BANK(shop_nr), GET_GOLD(k) + SHOP_BANK(shop_nr));
// } else
// send_to_char(ch, "<NONE>\r\n");
//
//
// send_to_char(ch, "Customers:  %s\r\n", (ptrsave = customer_string(shop_nr, TRUE)) ? ptrsave : "None");
//
//
// send_to_char(ch, "Produces:   ");
// column = 12;	/* ^^^ strlen ^^^ */
// for (sindex = 0; SHOP_PRODUCT(shop_nr, sindex) != NOTHING; sindex++) {
// char buf1[128];
// int linelen;
//
// if (sindex) {
// send_to_char(ch, ", ");
// column += 2;
// }
// linelen = snprintf(buf1, sizeof(buf1), "%s (#%d)",
// obj_proto[SHOP_PRODUCT(shop_nr, sindex)].short_description,
// obj_index[SHOP_PRODUCT(shop_nr, sindex)].vnum);
//
// /* Implementing word-wrapping: assumes screen-size == 80 */
// if (linelen + column >= 78 && column >= 20) {
// send_to_char(ch, "\r\n            ");
// /* 12 is to line up with "Produces:" printed first, and spaces above. */
// column = 12;
// }
//
// if (!send_to_char(ch, "%s", buf1))
// return;
// column += linelen;
// }
// if (!sindex)
// send_to_char(ch, "Produces:   Nothing!");
//
// send_to_char(ch, "\r\nBuys:       ");
// column = 12;	/* ^^^ strlen ^^^ */
// for (sindex = 0; SHOP_BUYTYPE(shop_nr, sindex) != NOTHING; sindex++) {
// char buf1[128];
// size_t linelen;
//
// if (sindex) {
// send_to_char(ch, ", ");
// column += 2;
// }
//
// linelen = snprintf(buf1, sizeof(buf1), "%s (#%d) [%s]",
// ITEM_TYPES[SHOP_BUYTYPE(shop_nr, sindex)],
// SHOP_BUYTYPE(shop_nr, sindex),
// SHOP_BUYWORD(shop_nr, sindex) ? SHOP_BUYWORD(shop_nr, sindex) : "all");
//
// /* Implementing word-wrapping: assumes screen-size == 80 */
// if (linelen + column >= 78 && column >= 20) {
// send_to_char(ch, "\r\n            ");
// /* 12 is to line up with "Buys:" printed first, and spaces above. */
// column = 12;
// }
//
// if (!send_to_char(ch, "%s", buf1))
// return;
// column += linelen;
// }
// if (!sindex)
// send_to_char(ch, "Buys:       Nothing!");
//
// send_to_char(ch, "\r\nBuy at:     [%4.2f], Sell at: [%4.2f], Open: [%d-%d, %d-%d]\r\n",
// SHOP_SELLPROFIT(shop_nr), SHOP_BUYPROFIT(shop_nr), SHOP_OPEN1(shop_nr),
// SHOP_CLOSE1(shop_nr), SHOP_OPEN2(shop_nr), SHOP_CLOSE2(shop_nr));
//
//
// /* Need a local buffer. */
// {
// char buf1[128];
// sprintbit(SHOP_BITVECTOR(shop_nr), shop_bits, buf1, sizeof(buf1));
// send_to_char(ch, "Bits:       %s\r\n", buf1);
// }
// }
//
//
// void show_shops(struct char_data *ch, char *arg)
// {
// int shop_nr;
//
// if (!*arg)
// list_all_shops(ch);
// else {
// if (!str_cmp(arg, ".")) {
// for (shop_nr = 0; shop_nr <= top_shop; shop_nr++)
// if (ok_shop_room(shop_nr, GET_ROOM_VNUM(IN_ROOM(ch))))
// break;
//
// if (shop_nr > top_shop) {
// send_to_char(ch, "This isn't a shop!\r\n");
// return;
// }
// } else if (is_number(arg))
// shop_nr = atoi(arg) - 1;
// else
// shop_nr = -1;
//
// if (shop_nr < 0 || shop_nr > top_shop) {
// send_to_char(ch, "Illegal shop number.\r\n");
// return;
// }
// list_detailed_shop(ch, shop_nr);
// }
// }
//
//
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
