/* ************************************************************************
*   File: modify.rs                                     Part of CircleMUD *
*  Usage: Run-time modification of game variables                         *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
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
use crate::depot::DepotId;
use crate::handler::FIND_CHAR_WORLD;
use crate::interpreter::{any_one_arg, delete_doubledollar, one_argument};
use crate::spell_parser::{find_skill_num, UNUSED_SPELLNAME};
use crate::spells::TOP_SPELL_DEFINE;
use crate::structs::ConState::{ConExdesc, ConMenu, ConPlaying};
use crate::structs::{ LVL_IMMORT, PLR_MAILING, PLR_WRITING};
use crate::util::BRF;
use crate::{DescriptorData, Game,  PAGE_LENGTH, PAGE_WIDTH};

pub fn string_write(game: &mut Game, d_id: DepotId, writeto: Rc<RefCell<String>>, len: usize, mailto: i64) {
    let d = game.desc(d_id);
    if d.character.is_some() && !game.db.ch(d.character.unwrap()).is_npc() {
        game.db.ch_mut(d.character.unwrap()).set_plr_flag_bit(PLR_WRITING);
    }

    let d = game.desc_mut(d_id);
    d.str = Some(writeto);
    d.max_str = len;
    d.mail_to = mailto;
}

/* Add user input to the 'current' string (as defined by d->str) */
pub fn string_add(game: &mut Game, d_id: DepotId, str_: &str) {
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
    let the_str = game.desc_mut(d_id).str.as_ref().unwrap().clone();
    if RefCell::borrow(the_str.as_ref()).is_empty() {
        if str_.len() + 3 > game.desc_mut(d_id).max_str {
            let chid = game.desc_mut(d_id).character.as_ref().unwrap().clone();
            game.send_to_char(chid, "String too long - Truncated.\r\n");
            str_.truncate(game.desc_mut(d_id).max_str - 3);
            str_.push_str("\r\n");
            *RefCell::borrow_mut(the_str.as_ref()) = str_;
            terminator = true;
        } else {
            *RefCell::borrow_mut(the_str.as_ref()) = str_;
        }
    } else {
        if str_.len() + RefCell::borrow(the_str.as_ref()).len() + 3 > game.desc_mut(d_id).max_str {
            let chid = game.desc_mut(d_id).character.as_ref().unwrap().clone();
            game.send_to_char(chid, "String too long.  Last line skipped.\r\n");
            terminator = true;
        } else {
            RefCell::borrow_mut(the_str.as_ref()).push_str(str_.as_str());
        }
    }

    if terminator {
        if game.desc_mut(d_id).state() == ConPlaying
            && game.db.ch(game
                .desc(d_id)
                .character
                .unwrap())
                .plr_flagged(PLR_MAILING)
        {
            let message_pointer = game.desc_mut(d_id).str.as_ref().unwrap().clone();
            let mail_to = game.desc_mut(d_id).mail_to;
            let from = game.db.ch(game.desc(d_id).character.unwrap()).get_idnum();
            game.db.store_mail(
                mail_to,
                from,
                RefCell::borrow(message_pointer.as_ref()).as_str(),
            );
            game.desc_mut(d_id).mail_to = 0;
            game.desc_mut(d_id).str = None;
            game.write_to_output(d_id, "Message sent!\r\n");
            if !game.db.ch(game.desc(d_id).character.unwrap()).is_npc() {
                game.db.ch_mut(game.desc(d_id)
                    .character
                    .unwrap())
                    .remove_prf_flags_bits(PLR_MAILING | PLR_WRITING);
            }
        }

        game.desc_mut(d_id).str = None;

        if game.desc_mut(d_id).mail_to >= BOARD_MAGIC {
            let board_type = (game.desc_mut(d_id).mail_to - BOARD_MAGIC) as usize;
            board_save_board(&mut game.db.boards, board_type);
            game.desc_mut(d_id).mail_to = 0;
        }
        if game.desc_mut(d_id).state() == ConExdesc {
            game.write_to_output(d_id, MENU);
            game.desc_mut(d_id).set_state(ConMenu);
        }
        if game.desc(d_id).state() == ConPlaying
            && game.desc(d_id).character.is_some()
            && !game.db.ch(game.desc(d_id).character.unwrap()).is_npc()
        {
            game.db.ch_mut(game.desc(d_id)
                .character
                .unwrap())
                .remove_plr_flag(PLR_WRITING);
        }
    } else {
        RefCell::borrow_mut(the_str.as_ref()).push_str("\r\n");
    }
}

// /* **********************************************************************
// *  Modification of character skills                                     *
// ********************************************************************** */
pub fn do_skillset(game: &mut Game, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut name = String::new();

    let argument2 = one_argument(argument, &mut name);
    let argument = argument2;

    if name.is_empty() {
        /* no arguments. print an informative text */
        game.send_to_char(
            chid,
            "Syntax: skillset <name> '<skill>' <value>\r\n\
Skill being one of the following:\r\n",
        );
        let mut qend = 0;
        for i in 0..TOP_SPELL_DEFINE + 1 {
            if game.db.spell_info[i].name == UNUSED_SPELLNAME {
                /* This is valid. */
                continue;
            }
            game.send_to_char(chid, format!("{:18}", game.db.spell_info[i].name).as_str());
            qend += 1;
            if qend % 4 == 3 {
                game.send_to_char(chid, "\r\n");
            }
        }
        if qend % 4 != 0 {
            game.send_to_char(chid, "\r\n");
        }

        return;
    }
    let vict_id = game.get_char_vis(chid, &mut name, None, FIND_CHAR_WORLD);
    if vict_id.is_none() {
        game.send_to_char(chid, NOPERSON);
        return;
    }
    let vict_id = vict_id.unwrap();
    let vict = game.db.ch(vict_id);
    let mut argument = argument.trim_start().to_string();

    /* If there is no chars in argument */
    if argument.is_empty() {
        game.send_to_char(chid, "Skill name expected.\r\n");
        return;
    }
    if !argument.starts_with('\'') {
        game.send_to_char(chid, "Skill must be enclosed in: ''\r\n");
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
        game.send_to_char(chid, "Skill must be enclosed in: ''\r\n");
        return;
    }
    let help = argument.to_lowercase();
    let help = &help.as_str()[0..qend];

    let skill = find_skill_num(&game.db, help);
    if skill.is_none() {
        game.send_to_char(chid, "Unrecognized skill.\r\n");
        return;
    }
    let buf = String::new();
    let skill = skill.unwrap();

    if buf.is_empty() {
        game.send_to_char(chid, "Learned value expected.\r\n");
        return;
    }
    let value = buf.parse::<i8>();
    if value.is_err() {
        game.send_to_char(chid, "Invalid value.\r\n");
        return;
    }

    let value = value.unwrap();
    if value < 0 {
        game.send_to_char(chid, "Minimum value for learned is 0.\r\n");
        return;
    }
    if value > 100 {
        game.send_to_char(chid, "Max value for learned is 100.\r\n");
        return;
    }
    if vict.is_npc() {
        game.send_to_char(chid, "You can't set NPC skills.\r\n");
        return;
    }

    /*
     * find_skill_num() guarantees a valid spell_info[] index, or -1, and we
     * checked for the -1 above so we are safe here.
     */
    let vict = game.db.ch_mut(vict_id);
    vict.set_skill(skill, value);
    let vict = game.db.ch(vict_id);
    game.mudlog(
        BRF,
        LVL_IMMORT as i32,
        true,
        format!(
            "{} changed {}'s {} to {}.",
            game.db.ch(chid).get_name(),
            vict.get_name(),
            game.db.spell_info[skill as usize].name,
            value
        )
        .as_str(),
    );
    let vict = game.db.ch(vict_id);
    game.send_to_char(
        chid,
        format!(
            "You change {}'s {} to {}.\r\n",
            vict.get_name(),
            game.db.spell_info[skill as usize].name,
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
pub fn paginate_string<'a>(msg: &'a str, d: &'a mut DescriptorData) -> &'a str {
    if d.showstr_count != 0 {
        d.showstr_vector.push(Rc::from(msg));
    }

    let mut s = msg;
    for _ in 1..d.showstr_count {
        let r = next_page(s);
        if r.is_some() {
            d.showstr_vector.push(Rc::from(r.unwrap()));
            s = r.unwrap();
        } else {
            break;
        }
    }

    d.showstr_page = 0;
    return s;
}

/* The call that gets the paging ball rolling... */
pub fn page_string(game: &mut Game, d_id: DepotId, msg: &str, keep_internal: bool) {
    if msg.is_empty() {
        return;
    }

    game.desc_mut(d_id).showstr_count = count_pages(msg);
    let need = game.desc_mut(d_id).showstr_count as usize;
    game.desc_mut(d_id).showstr_vector.reserve_exact(need);

    if keep_internal {
        game.desc_mut(d_id).showstr_head = Some(Rc::from(msg));
        let msg = game.desc_mut(d_id).showstr_head.as_ref().unwrap().clone();
        paginate_string(msg.as_ref(), game.desc_mut(d_id));
    } else {
        paginate_string(msg, game.desc_mut(d_id));
    }

    let actbuf = "";
    show_string(game, d_id, actbuf);
}

/* The call that displays the next page. */
pub fn show_string(game: &mut Game, d_id: DepotId, input: &str) {
    let mut buf = String::new();
    any_one_arg(input, &mut buf);

    if !buf.is_empty() {
        /* Q is for quit. :) */
        let cmd = buf.chars().next().unwrap().to_ascii_lowercase();
        if cmd == 'q' {
            game.desc_mut(d_id).showstr_vector.clear();
            game.desc_mut(d_id).showstr_count = 0;
            return;
        }
        /* R is for refresh, so back up one page internally so we can display
         * it again.
         */
        else if cmd == 'r' {
            game.desc_mut(d_id).showstr_page = max(0, game.desc_mut(d_id).showstr_page - 1);
        }
        /* B is for back, so back up two pages internally so we can display the
         * correct page here.
         */
        else if cmd == 'b' {
            game.desc_mut(d_id).showstr_page = max(0, game.desc_mut(d_id).showstr_page - 2);
        }
        /* Feature to 'goto' a page.  Just type the number of the page and you
         * are there!
         */
        else if cmd.is_digit(10) {
            let nr = buf.parse::<i32>();
            if nr.is_err() {
                let chid = game.desc_mut(d_id).character.as_ref().unwrap().clone();
                game.send_to_char(
                    chid,
                    "Valid commands while paging are RETURN, Q, R, B, or a numeric value.\r\n",
                );
            }
            game.desc_mut(d_id).showstr_page = max(
                0,
                min(nr.unwrap() - 1, game.desc_mut(d_id).showstr_count - 1),
            );
        } else if !buf.is_empty() {
            let to_char_id = game.desc_mut(d_id).character.as_ref().unwrap().clone();
            game.send_to_char(
                to_char_id,
                "Valid commands while paging are RETURN, Q, R, B, or a numeric value.\r\n",
            );
            return;
        }
    }
    /* If we're displaying the last page, just send it to the character, and
     * then free up the space we used.
     */
    if game.desc_mut(d_id).showstr_page + 1 >= game.desc_mut(d_id).showstr_count {
        let chid = game.desc_mut(d_id).character.as_ref().unwrap().clone();
        let showstr_page = game.desc_mut(d_id).showstr_page as usize;
        let msg = game.desc_mut(d_id).showstr_vector[showstr_page].clone();
        game.send_to_char(chid, msg.as_ref());
        game.desc_mut(d_id).showstr_vector.clear();
        game.desc_mut(d_id).showstr_count = 0;
        if game.desc_mut(d_id).showstr_head.is_some() {
            game.desc_mut(d_id).showstr_head = None;
        }
    }
    /* Or if we have more to show.... */
    else {
        let showstr_page = game.desc_mut(d_id).showstr_page as usize;
        let diff = game.desc_mut(d_id).showstr_vector[showstr_page].len()
            - game.desc_mut(d_id).showstr_vector[(showstr_page + 1) as usize].len();
        let buffer = &game.desc_mut(d_id).showstr_vector[showstr_page].as_ref()[..diff].to_string();
        let chid = game.desc_mut(d_id).character.as_ref().unwrap().clone();
        game.send_to_char(chid, buffer);
        game.desc_mut(d_id).showstr_page = showstr_page as i32 + 1;
    }
}
