/* ************************************************************************
*   File: modify.rs                                     Part of CircleMUD *
*  Usage: Run-time modification of game variables                         *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */
//
// const char *string_fields[] =
// {
// "name",
// "short",
// "long",
// "description",
// "title",
// "delete-description",
// "\n"
// };
//
//
// /* maximum length for text field x+1 */
// int length[] =
// {
// 15,
// 60,
// 256,
// 240,
// 60
// };

/*
 * Basic API function to start writing somewhere.
 *
 * 'data' isn't used in stock CircleMUD but you can use it to pass whatever
 * else you may want through it.  The improved editor patch when updated
 * could use it to pass the old text buffer, for instance.
 */
use std::cell::RefCell;
use std::cmp::{max, min};
use std::rc::Rc;

use crate::boards::{board_save_board, BOARD_MAGIC};
use crate::config::{MENU, NOPERSON};
use crate::db::DB;
use crate::handler::FIND_CHAR_WORLD;
use crate::interpreter::{any_one_arg, delete_doubledollar, one_argument};
use crate::spell_parser::{find_skill_num, UNUSED_SPELLNAME};
use crate::spells::TOP_SPELL_DEFINE;
use crate::structs::ConState::{ConExdesc, ConMenu, ConPlaying};
use crate::structs::{CharData, LVL_IMMORT, PLR_MAILING, PLR_WRITING};
use crate::util::BRF;
use crate::{send_to_char, write_to_output, DescriptorData, Game, PAGE_LENGTH, PAGE_WIDTH};

pub fn string_write(d: &DescriptorData, writeto: Rc<RefCell<String>>, len: usize, mailto: i64) {
    if d.character.borrow().is_some() && !d.character.borrow().as_ref().unwrap().is_npc() {
        d.character
            .borrow()
            .as_ref()
            .unwrap()
            .set_plr_flag_bit(PLR_WRITING);
    }

    *d.str.borrow_mut() = Some(writeto);
    d.max_str.set(len);
    d.mail_to.set(mailto);
}

/* Add user input to the 'current' string (as defined by d->str) */
pub fn string_add(db: &DB, d: &DescriptorData, str_: &str) {
    /* determine if this is the terminal string, and truncate if so */
    /* changed to only accept '@' at the beginning of line - J. Elson 1/17/94 */

    let mut str_ = str_.to_string();
    delete_doubledollar(&mut str_);
    let t = str_.chars().next();
    let t = if t.is_some() { t.unwrap() } else { '\0' };
    let mut terminator = false;
    if t == '@' {
        terminator = true;
        str_ = "".to_string();
    }

    // smash_tilde(str_);
    let mut the_stro = d.str.borrow_mut();
    let the_str = the_stro.as_ref().unwrap();
    if RefCell::borrow(the_str).is_empty() {
        if str_.len() + 3 > d.max_str.get() {
            send_to_char(
                d.character.borrow().as_ref().unwrap(),
                "String too long - Truncated.\r\n",
            );
            str_.truncate(d.max_str.get() - 3);
            str_.push_str("\r\n");
            *RefCell::borrow_mut(the_str) = str_;
            terminator = true;
        } else {
            *RefCell::borrow_mut(the_str) = str_;
        }
    } else {
        if str_.len() + RefCell::borrow(the_str).len() + 3 > d.max_str.get() {
            send_to_char(
                d.character.borrow().as_ref().unwrap(),
                "String too long.  Last line skipped.\r\n",
            );
            terminator = true;
        } else {
            RefCell::borrow_mut(the_str).push_str(str_.as_str());
        }
    }

    if terminator {
        if d.state() == ConPlaying
            && d.character
                .borrow()
                .as_ref()
                .unwrap()
                .plr_flagged(PLR_MAILING)
        {
            db.mails.borrow_mut().store_mail(
                db,
                d.mail_to.get(),
                d.character.borrow().as_ref().unwrap().get_idnum(),
                RefCell::borrow(d.str.borrow().as_ref().unwrap()).as_str(),
            );
            d.mail_to.set(0);
            *d.str.borrow_mut() = None;
            write_to_output(d, "Message sent!\r\n");
            if !d.character.borrow().as_ref().unwrap().is_npc() {
                d.character
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .remove_prf_flags_bits(PLR_MAILING | PLR_WRITING);
            }
        }

        *the_stro = None;

        if d.mail_to.get() >= BOARD_MAGIC {
            board_save_board(
                &mut db.boards.borrow_mut(),
                (d.mail_to.get() - BOARD_MAGIC) as usize,
            );
            d.mail_to.set(0);
        }
        if d.state() == ConExdesc {
            write_to_output(d, MENU);
            d.set_state(ConMenu);
        }
        if d.state() == ConPlaying
            && d.character.borrow().is_some()
            && !d.character.borrow().as_ref().unwrap().is_npc()
        {
            d.character
                .borrow()
                .as_ref()
                .unwrap()
                .remove_plr_flag(PLR_WRITING);
        }
    } else {
        RefCell::borrow_mut(the_str).push_str("\r\n");
    }
}

// /* **********************************************************************
// *  Modification of character skills                                     *
// ********************************************************************** */
pub fn do_skillset(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    let mut name = String::new();

    let argument2 = one_argument(argument, &mut name);
    let argument = argument2;

    if name.is_empty() {
        /* no arguments. print an informative text */
        send_to_char(
            ch,
            "Syntax: skillset <name> '<skill>' <value>\r\n\
Skill being one of the following:\r\n",
        );
        let mut qend = 0;
        for i in 0..TOP_SPELL_DEFINE + 1 {
            if db.spell_info[i].name == UNUSED_SPELLNAME {
                /* This is valid. */
                continue;
            }
            send_to_char(ch, format!("{:18}", db.spell_info[i].name).as_str());
            qend += 1;
            if qend % 4 == 3 {
                send_to_char(ch, "\r\n");
            }
        }
        if qend % 4 != 0 {
            send_to_char(ch, "\r\n");
        }

        return;
    }
    let vict = db.get_char_vis(ch, &mut name, None, FIND_CHAR_WORLD);
    if vict.is_none() {
        send_to_char(ch, NOPERSON);
        return;
    }
    let vict = vict.unwrap();
    let mut argument = argument.trim_start().to_string();

    /* If there is no chars in argument */
    if argument.is_empty() {
        send_to_char(ch, "Skill name expected.\r\n");
        return;
    }
    if !argument.starts_with('\'') {
        send_to_char(ch, "Skill must be enclosed in: ''\r\n");
        return;
    }
    /* Locate the last quote and lowercase the magic words (if any) */

    argument.remove(0);
    let mut last_c;
    let mut qend = 0;
    for c in argument.chars() {
        last_c = c;
        if last_c == '\'' {
            break;
        }
        qend += 1;
    }

    if &argument[qend..qend] != "\'" {
        send_to_char(ch, "Skill must be enclosed in: ''\r\n");
        return;
    }
    let help = argument.to_lowercase();
    let help = &help.as_str()[0..qend];

    let skill = find_skill_num(db, help);
    if skill.is_none() {
        send_to_char(ch, "Unrecognized skill.\r\n");
        return;
    }
    let buf = String::new();
    let skill = skill.unwrap();

    if buf.is_empty() {
        send_to_char(ch, "Learned value expected.\r\n");
        return;
    }
    let value = buf.parse::<i8>();
    if value.is_err() {
        send_to_char(ch, "Invalid value.\r\n");
        return;
    }

    let value = value.unwrap();
    if value < 0 {
        send_to_char(ch, "Minimum value for learned is 0.\r\n");
        return;
    }
    if value > 100 {
        send_to_char(ch, "Max value for learned is 100.\r\n");
        return;
    }
    if vict.is_npc() {
        send_to_char(ch, "You can't set NPC skills.\r\n");
        return;
    }

    /*
     * find_skill_num() guarantees a valid spell_info[] index, or -1, and we
     * checked for the -1 above so we are safe here.
     */
    vict.set_skill(skill, value);
    game.mudlog(
        BRF,
        LVL_IMMORT as i32,
        true,
        format!(
            "{} changed {}'s {} to {}.",
            ch.get_name(),
            vict.get_name(),
            db.spell_info[skill as usize].name,
            value
        )
        .as_str(),
    );
    send_to_char(
        ch,
        format!(
            "You change {}'s {} to {}.\r\n",
            vict.get_name(),
            db.spell_info[skill as usize].name,
            value
        )
        .as_str(),
    );
}

/*********************************************************************
* New Pagination Code
* Michael Buselli submitted the following code for an enhanced pager
* for CircleMUD.  All functions below are his.  --JE 8 Mar 96
*
*********************************************************************/

/* Traverse down the string until the beginning of the next page has been
 * reached.  Return NULL if this is the last page of the string.
 */
fn next_page(str: &str) -> Option<&str> {
    let mut col = 1;
    let mut line = 1;
    let mut spec_code = false;

    for (i, c) in str.bytes().enumerate() {
        /* If we're at the start of the next page, return this fact. */
        //else
        if line > PAGE_LENGTH {
            return Some(&str[i..]);
        }
        /* Check for the begining of an ANSI color code block. */
        else if c == 0x1B && !spec_code {
            spec_code = true;
        }
        /* Check for the end of an ANSI color code block. */
        else if c == 109 && spec_code {
            spec_code = false;
        }
        /* Check for everything else. */
        else if !spec_code {
            /* Carriage return puts us in column one. */
            if c == 13 {
                col = 1;
                /* Newline puts us on the next line. */
            } else if c == 10 {
                line += 1;
            }
            /* We need to check here and see if we are over the page width,
             * and if so, compensate by going to the begining of the next line.
             */
            else {
                col += 1;
                if col > PAGE_WIDTH {
                    col = 1;
                    line += 1;
                }
            }
        }
    }
    return None;
}

/* Function that returns the number of pages in the string. */
fn count_pages(msg: &str) -> i32 {
    let mut msg = msg;
    let mut pages = 1;
    loop {
        let r = next_page(msg);
        if r.is_none() {
            break;
        }
        msg = r.unwrap();
        pages += 1;
    }
    pages
}

/* This function assigns all the pointers for showstr_vector for the
 * page_string function, after showstr_vector has been allocated and
 * showstr_count set.
 */
pub fn paginate_string<'a>(msg: &'a str, d: &'a DescriptorData) -> &'a str {
    if d.showstr_count.get() != 0 {
        d.showstr_vector.borrow_mut().push(Rc::from(msg));
    }

    let mut s = msg;
    for _ in 1..d.showstr_count.get() {
        let r = next_page(s);
        if r.is_some() {
            d.showstr_vector.borrow_mut().push(Rc::from(r.unwrap()));
            s = r.unwrap();
        } else {
            break;
        }
    }

    d.showstr_page.set(0);
    return s;
}

/* The call that gets the paging ball rolling... */
pub fn page_string(d: &DescriptorData, msg: &str, keep_internal: bool) {

    if msg.is_empty() {
        return;
    }

    d.showstr_count.set(count_pages(msg));
    d.showstr_vector
        .borrow_mut()
        .reserve_exact(d.showstr_count.get() as usize);

    if keep_internal {
        *d.showstr_head.borrow_mut() = Some(Rc::from(msg));
        paginate_string(d.showstr_head.borrow().as_ref().unwrap(), d);
    } else {
        paginate_string(msg, d);
    }

    let actbuf = "";
    show_string(d, actbuf);
}

/* The call that displays the next page. */
pub fn show_string(d: &DescriptorData, input: &str) {
    let mut buf = String::new();
    any_one_arg(input, &mut buf);

    if !buf.is_empty() {
        /* Q is for quit. :) */
        let cmd = buf.chars().next().unwrap().to_ascii_lowercase();
        if cmd == 'q' {
            d.showstr_vector.borrow_mut().clear();
            d.showstr_count.set(0);
            return;
        }
        /* R is for refresh, so back up one page internally so we can display
         * it again.
         */
        else if cmd == 'r' {
            d.showstr_page.set(max(0, d.showstr_page.get() - 1));
        }
        /* B is for back, so back up two pages internally so we can display the
         * correct page here.
         */
        else if cmd == 'b' {
            d.showstr_page.set(max(0, d.showstr_page.get() - 2));
        }
        /* Feature to 'goto' a page.  Just type the number of the page and you
         * are there!
         */
        else if cmd.is_digit(10) {
            let nr = buf.parse::<i32>();
            if nr.is_err() {
                send_to_char(
                    d.character.borrow().as_ref().unwrap().as_ref(),
                    "Valid commands while paging are RETURN, Q, R, B, or a numeric value.\r\n",
                );
            }
            d.showstr_page
                .set(max(0, min(nr.unwrap() - 1, d.showstr_count.get() - 1)));
        } else if !buf.is_empty() {
            send_to_char(
                d.character.borrow().as_ref().unwrap().as_ref(),
                "Valid commands while paging are RETURN, Q, R, B, or a numeric value.\r\n",
            );
            return;
        }
    }
    /* If we're displaying the last page, just send it to the character, and
     * then free up the space we used.
     */
    if d.showstr_page.get() + 1 >= d.showstr_count.get() {
        send_to_char(
            d.character.borrow().as_ref().unwrap().as_ref(),
            d.showstr_vector.borrow()[d.showstr_page.get() as usize].as_ref(),
        );
        d.showstr_vector.borrow_mut().clear();
        d.showstr_count.set(0);
        if d.showstr_head.borrow().is_some() {
            *d.showstr_head.borrow_mut() = None;
        }
    }
    /* Or if we have more to show.... */
    else {
        let sv = d.showstr_vector.borrow();
        let diff =
            sv[d.showstr_page.get() as usize].len() - sv[(d.showstr_page.get() + 1) as usize].len();
        let buffer = &sv[d.showstr_page.get() as usize].as_ref()[..diff];
        send_to_char(d.character.borrow().as_ref().unwrap(), buffer);
        d.showstr_page.set(d.showstr_page.get() + 1);
    }
}
