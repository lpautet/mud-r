/* ************************************************************************
*   File: modify.c                                      Part of CircleMUD *
*  Usage: Run-time modification of game variables                         *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
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
//
//
// /* ************************************************************************
// *  modification of malloc'ed strings                                      *
// ************************************************************************ */
//
// /*
//  * Put '#if 1' here to erase ~, or roll your own method.  A common idea
//  * is smash/show tilde to convert the tilde to another innocuous character
//  * to save and then back to display it. Whatever you do, at least keep the
//  * function around because other MUD packages use it, like mudFTP.
//  *   -gg 9/9/98
//  */
// void smash_tilde(char *str)
// {
// #if 0
// /*
//  * Erase any ~'s inserted by people in the editor.  This prevents anyone
//  * using online creation from causing parse errors in the world files.
//  * Derived from an idea by Sammy <samedi@dhc.net> (who happens to like
//  * his tildes thank you very much.), -gg 2/20/98
//  */
// while ((str = strchr(str, '~')) != NULL)
// *str = ' ';
// #endif
// }
//
// /*
//  * Basic API function to start writing somewhere.
//  *
//  * 'data' isn't used in stock CircleMUD but you can use it to pass whatever
//  * else you may want through it.  The improved editor patch when updated
//  * could use it to pass the old text buffer, for instance.
//  */
// void string_write(struct descriptor_data *d, char **writeto, size_t len, long mailto, void *data)
// {
// if (d->character && !IS_NPC(d->character))
// SET_BIT(PLR_FLAGS(d->character), PLR_WRITING);
//
// if (data)
// mudlog(BRF, LVL_IMMORT, TRUE, "SYSERR: string_write: I don't understand special data.");
//
// d->str = writeto;
// d->max_str = len;
// d->mail_to = mailto;
// }
//
// /* Add user input to the 'current' string (as defined by d->str) */
// void string_add(struct descriptor_data *d, char *str)
// {
// int terminator;
//
// /* determine if this is the terminal string, and truncate if so */
// /* changed to only accept '@' at the beginning of line - J. Elson 1/17/94 */
//
// delete_doubledollar(str);
//
// if ((terminator = (*str == '@')))
// *str = '\0';
//
// smash_tilde(str);
//
// if (!(*d->str)) {
// if (strlen(str) + 3 > d->max_str) { /* \r\n\0 */
// send_to_char(d->character, "String too long - Truncated.\r\n");
// strcpy(&str[d->max_str - 3], "\r\n");	/* strcpy: OK (size checked) */
// CREATE(*d->str, char, d->max_str);
// strcpy(*d->str, str);	/* strcpy: OK (size checked) */
// terminator = 1;
// } else {
// CREATE(*d->str, char, strlen(str) + 3);
// strcpy(*d->str, str);	/* strcpy: OK (size checked) */
// }
// } else {
// if (strlen(str) + strlen(*d->str) + 3 > d->max_str) { /* \r\n\0 */
// send_to_char(d->character, "String too long.  Last line skipped.\r\n");
// terminator = 1;
// } else {
// RECREATE(*d->str, char, strlen(*d->str) + strlen(str) + 3); /* \r\n\0 */
// strcat(*d->str, str);	/* strcat: OK (size precalculated) */
// }
// }
//
// if (terminator) {
// if (STATE(d) == CON_PLAYING && (PLR_FLAGGED(d->character, PLR_MAILING))) {
// store_mail(d->mail_to, GET_IDNUM(d->character), *d->str);
// d->mail_to = 0;
// free(*d->str);
// free(d->str);
// write_to_output(d, "Message sent!\r\n");
// if (!IS_NPC(d->character))
// REMOVE_BIT(PLR_FLAGS(d->character), PLR_MAILING | PLR_WRITING);
// }
// d->str = NULL;
//
// if (d->mail_to >= BOARD_MAGIC) {
// Board_save_board(d->mail_to - BOARD_MAGIC);
// d->mail_to = 0;
// }
// if (STATE(d) == ConExdesc) {
// write_to_output(d, "%s", MENU);
// STATE(d) = ConMenu;
// }
// if (STATE(d) == CON_PLAYING && d->character && !IS_NPC(d->character))
// REMOVE_BIT(PLR_FLAGS(d->character), PLR_WRITING);
// } else
// strcat(*d->str, "\r\n");	/* strcat: OK (size checked) */
// }
//
//
//
// /* **********************************************************************
// *  Modification of character skills                                     *
// ********************************************************************** */
//
// ACMD(do_skillset)
// {
// struct char_data *vict;
// char name[MAX_INPUT_LENGTH];
// char buf[MAX_INPUT_LENGTH], help[MAX_STRING_LENGTH];
// int skill, value, i, qend;
//
// argument = one_argument(argument, name);
//
// if (!*name) {			/* no arguments. print an informative text */
// send_to_char(ch, "Syntax: skillset <name> '<skill>' <value>\r\n"
// "Skill being one of the following:\r\n");
// for (qend = 0, i = 0; i <= TOP_SPELL_DEFINE; i++) {
// if (spell_info[i].name == unused_spellname)	/* This is valid. */
// continue;
// send_to_char(ch, "%18s", spell_info[i].name);
// if (qend++ % 4 == 3)
// send_to_char(ch, "\r\n");
// }
// if (qend % 4 != 0)
// send_to_char(ch, "\r\n");
// return;
// }
//
// if (!(vict = get_char_vis(ch, name, NULL, FIND_CHAR_WORLD))) {
// send_to_char(ch, "%s", NOPERSON);
// return;
// }
// skip_spaces(&argument);
//
// /* If there is no chars in argument */
// if (!*argument) {
// send_to_char(ch, "Skill name expected.\r\n");
// return;
// }
// if (*argument != '\'') {
// send_to_char(ch, "Skill must be enclosed in: ''\r\n");
// return;
// }
// /* Locate the last quote and lowercase the magic words (if any) */
//
// for (qend = 1; argument[qend] && argument[qend] != '\''; qend++)
// argument[qend] = LOWER(argument[qend]);
//
// if (argument[qend] != '\'') {
// send_to_char(ch, "Skill must be enclosed in: ''\r\n");
// return;
// }
// strcpy(help, (argument + 1));	/* strcpy: OK (MAX_INPUT_LENGTH <= MAX_STRING_LENGTH) */
// help[qend - 1] = '\0';
// if ((skill = find_skill_num(help)) <= 0) {
// send_to_char(ch, "Unrecognized skill.\r\n");
// return;
// }
// argument += qend + 1;		/* skip to next parameter */
// argument = one_argument(argument, buf);
//
// if (!*buf) {
// send_to_char(ch, "Learned value expected.\r\n");
// return;
// }
// value = atoi(buf);
// if (value < 0) {
// send_to_char(ch, "Minimum value for learned is 0.\r\n");
// return;
// }
// if (value > 100) {
// send_to_char(ch, "Max value for learned is 100.\r\n");
// return;
// }
// if (IS_NPC(vict)) {
// send_to_char(ch, "You can't set NPC skills.\r\n");
// return;
// }
//
// /*
//  * find_skill_num() guarantees a valid spell_info[] index, or -1, and we
//  * checked for the -1 above so we are safe here.
//  */
// SET_SKILL(vict, skill, value);
// mudlog(BRF, LVL_IMMORT, TRUE, "%s changed %s's %s to %d.", GET_NAME(ch), GET_NAME(vict), spell_info[skill].name, value);
// send_to_char(ch, "You change %s's %s to %d.\r\n", GET_NAME(vict), spell_info[skill].name, value);
// }

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
        pages += 1;
    }
    pages
}

/* This function assigns all the pointers for showstr_vector for the
 * page_string function, after showstr_vector has been allocated and
 * showstr_count set.
 */
use crate::interpreter::any_one_arg;
use crate::{send_to_char, DescriptorData, PAGE_LENGTH, PAGE_WIDTH};
use std::cmp::{max, min};
use std::rc::Rc;

pub fn paginate_string<'a>(msg: &'a str, d: &'a DescriptorData) -> &'a str {
    if d.showstr_count.get() != 0 {
        d.showstr_vector.borrow_mut().push(Rc::from(msg));
    }

    let mut s = msg;
    for _i in 1..d.showstr_count.get() {
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
pub fn page_string(d: Option<Rc<DescriptorData>>, msg: &str, keep_internal: bool) {
    if d.is_none() {
        return;
    }

    let d = d.as_ref().unwrap();

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
    show_string(d.clone(), actbuf);
}

/* The call that displays the next page. */
fn show_string(d: Rc<DescriptorData>, input: &str) {
    // char buffer[MAX_STRING_LENGTH], buf[MAX_INPUT_LENGTH];
    // int diff;

    let mut buf = String::new();
    any_one_arg(input, &mut buf);

    /* Q is for quit. :) */
    let cmd = buf.chars().next().unwrap().to_ascii_lowercase();
    if cmd == 'q' {
        // free(d->showstr_vector);
        d.showstr_vector.borrow_mut().clear();
        d.showstr_count.set(0);
        //     if d.showstr_head
        //
        // if (d->showstr_head) {
        // free(d->showstr_head);
        // d->showstr_head = NULL;
        //}
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
        let buffer = sv[d.showstr_page.get() as usize].as_ref();
        /*
         * Fix for prompt overwriting last line in compact mode submitted by
         * Peter Ajamian <peter@pajamian.dhs.org> on 04/21/2001
         */
        // if (buffer[diff - 2] == '\r' && buffer[diff - 1]=='\n')
        // buffer[diff] = '\0';
        // else if (buffer[diff - 2] == '\n' && buffer[diff - 1] == '\r')
        // /* This is backwards.  Fix it. */
        // strcpy(buffer + diff - 2, "\r\n");	/* strcpy: OK (size checked) */
        // else if (buffer[diff - 1] == '\r' || buffer[diff - 1] == '\n')
        // /* Just one of \r\n.  Overwrite it. */
        // strcpy(buffer + diff - 1, "\r\n");	/* strcpy: OK (size checked) */
        // else
        // /* Tack \r\n onto the end to fix bug with prompt overwriting last line. */
        // strcpy(buffer + diff, "\r\n");	/* strcpy: OK (size checked) */
        send_to_char(d.character.borrow().as_ref().unwrap(), buffer);
        d.showstr_page.set(d.showstr_page.get() + 1);
    }
}
