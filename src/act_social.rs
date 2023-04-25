/* ************************************************************************
*   File: act.social.c                                  Part of CircleMUD *
*  Usage: Functions to handle socials                                     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader};
use std::process;
use std::rc::Rc;

use log::error;
use regex::Regex;

use crate::db::{DB, SOCMESS_FILE};
use crate::handler::FIND_CHAR_ROOM;
use crate::interpreter::{find_command, one_argument, CMD_INFO};
use crate::structs::{CharData, SEX_MALE};
use crate::util::rand_number;
use crate::{send_to_char, Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_SLEEP, TO_VICT};

// /* local globals */
// static int list_top = -1;

pub struct SocialMessg {
    act_nr: usize,
    hide: bool,
    min_victim_position: i32,
    /* Position of victim */

    /* No argument was supplied */
    char_no_arg: Rc<str>,
    others_no_arg: Rc<str>,

    /* An argument was there, and a victim was found */
    char_found: Rc<str>,
    /* if NULL, read no further, ignore args */
    others_found: Rc<str>,
    vict_found: Rc<str>,

    /* An argument was there, but no victim was found */
    not_found: Rc<str>,

    /* The victim turned out to be the character */
    char_auto: Rc<str>,
    others_auto: Rc<str>,
}

fn find_action(db: &DB, cmd: usize) -> Option<usize> {
    db.soc_mess_list.iter().position(|e| e.act_nr == cmd)
}

#[allow(unused_variables)]
pub fn do_action(game: &mut Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let db = &game.db;
    let act_nr;

    if {
        act_nr = find_action(db, cmd);
        act_nr.is_none()
    } {
        send_to_char(ch, "That action is not supported.\r\n");
        return;
    }
    let act_nr = act_nr.unwrap();
    let action = &db.soc_mess_list[act_nr];

    let mut buf = String::new();
    if !action.char_found.is_empty() && !argument.is_empty() {
        one_argument(argument, &mut buf);
    }

    if buf.is_empty() {
        send_to_char(ch, format!("{}\r\n", action.char_no_arg).as_str());
        db.act(
            &action.others_no_arg,
            action.hide,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        return;
    }
    let vict;
    if {
        vict = db.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(ch, format!("{}\r\n", &action.not_found).as_str());
    } else if Rc::ptr_eq(vict.as_ref().unwrap(), ch) {
        send_to_char(ch, format!("{}\r\n", &action.char_auto).as_str());
        db.act(
            &action.others_auto,
            action.hide,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
    } else {
        let vict = vict.as_ref().unwrap();
        if vict.get_pos() < action.min_victim_position as u8 {
            db.act(
                "$N is not in a proper position for that.",
                false,
                Some(ch),
                None,
                Some(vict),
                TO_CHAR | TO_SLEEP,
            );
        } else {
            db.act(
                &action.char_found,
                false,
                Some(ch),
                None,
                Some(vict),
                TO_CHAR | TO_SLEEP,
            );
            db.act(
                &action.others_found,
                action.hide,
                Some(ch),
                None,
                Some(vict),
                TO_NOTVICT,
            );
            db.act(
                &action.vict_found,
                action.hide,
                Some(ch),
                None,
                Some(vict),
                TO_VICT,
            );
        }
    }
}

#[allow(unused_variables)]
pub fn do_insult(game: &mut Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let db = &game.db;
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if !arg.is_empty() {
        let victim;
        if {
            victim = db.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
            victim.is_none()
        } {
            send_to_char(ch, "Can't hear you!\r\n");
        } else {
            let victim = victim.as_ref().unwrap();
            if !Rc::ptr_eq(victim, ch) {
                send_to_char(
                    ch,
                    format!("You insult {}.\r\n", victim.get_name()).as_str(),
                );

                match rand_number(0, 2) {
                    0 => {
                        if ch.get_sex() == SEX_MALE {
                            if victim.get_sex() == SEX_MALE {
                                db.act(
                                    "$n accuses you of fighting like a woman!",
                                    false,
                                    Some(ch),
                                    None,
                                    Some(victim),
                                    TO_VICT,
                                );
                            } else {
                                db.act(
                                    "$n says that women can't fight.",
                                    false,
                                    Some(ch),
                                    None,
                                    Some(victim),
                                    TO_VICT,
                                );
                            }
                        } else {
                            /* Ch == Woman */
                            if victim.get_sex() == SEX_MALE {
                                db.act(
                                    "$n accuses you of having the smallest... (brain?)",
                                    false,
                                    Some(ch),
                                    None,
                                    Some(victim),
                                    TO_VICT,
                                );
                            } else {
                                db.act("$n tells you that you'd lose a beauty contest against a troll.",
                                       false, Some(ch), None, Some(victim), TO_VICT);
                            }
                        }
                    }
                    1 => {
                        db.act(
                            "$n calls your mother a bitch!",
                            false,
                            Some(ch),
                            None,
                            Some(victim),
                            TO_VICT,
                        );
                    }
                    _ => {
                        db.act(
                            "$n tells you to get lost!",
                            false,
                            Some(ch),
                            None,
                            Some(victim),
                            TO_VICT,
                        );
                    }
                } /* end switch */

                db.act(
                    "$n insults $N.",
                    true,
                    Some(ch),
                    None,
                    Some(victim),
                    TO_NOTVICT,
                );
            } else {
                /* ch == victim */
                send_to_char(ch, "You feel insulted.\r\n");
            }
        }
    } else {
        send_to_char(ch, "I'm sure you don't want to insult *everybody*...\r\n");
    }
}

pub fn fread_action(reader: &mut BufReader<File>, nr: i32) -> Rc<str> {
    let mut buf = String::new();

    let r = reader
        .read_line(&mut buf)
        .expect(format!("SYSERR: fread_action: error while reading action #{}", nr).as_str());

    if r == 0 {
        error!("SYSERR: fread_action: unexpected EOF near action #{}", nr);
        process::exit(1);
    }
    if buf.starts_with('#') {
        return Rc::from("");
    }
    Rc::from(buf)
}

// void free_social_messages(void)
// {
// int ac;
// struct SocialMessg *soc;
//
// for (ac = 0; ac <= list_top; ac++) {
// soc = &soc_mess_list[ac];
//
// if (soc->char_no_arg)	free(soc->char_no_arg);
// if (soc->others_no_arg)	free(soc->others_no_arg);
// if (soc->char_found)	free(soc->char_found);
// if (soc->others_found)	free(soc->others_found);
// if (soc->vict_found)	free(soc->vict_found);
// if (soc->not_found)		free(soc->not_found);
// if (soc->char_auto)		free(soc->char_auto);
// if (soc->others_auto)	free(soc->others_auto);
// }
// free(soc_mess_list);
// }

impl DB {
    pub fn boot_social_messages(&mut self) {
        /* open social file */
        let fl;
        if {
            fl = OpenOptions::new().read(true).open(SOCMESS_FILE);
            fl.is_err()
        } {
            error!(
                "SYSERR: can't open socials file '{}': {}",
                SOCMESS_FILE,
                fl.err().unwrap()
            );
            process::exit(1);
        }
        let fl = fl.unwrap();
        let mut list_top = 0;
        /* count socials & allocate space */
        for nr in 0..CMD_INFO.len() - 1 {
            if CMD_INFO[nr].command_pointer as usize == do_action as usize {
                list_top += 1;
            }
        }

        self.soc_mess_list.reserve_exact(list_top + 1);
        let mut cur_soc = 0;
        let mut reader = BufReader::new(fl);
        /* now read 'em */
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("Reading socials file");
            if line.starts_with('$') {
                break;
            }

            let regex = Regex::new(r"^(\S+)\s(\d{1,9})\s(\d{1,9})").unwrap();
            let f = regex.captures(line.as_str());
            if f.is_none() {
                error!("SYSERR: format error in social file near social '{}'", line);
                process::exit(1);
            }
            let f = f.unwrap();
            let next_soc = &f[1];
            let hide = f[2].parse::<i32>().unwrap();
            let min_victim_position = f[2].parse::<i32>().unwrap();

            if {
                cur_soc += 1;
                cur_soc > list_top
            } {
                error!(
                    "SYSERR: Ran out of slots in social array. ({} > {})",
                    cur_soc, list_top
                );
                break;
            }
            let hide = if hide == 0 { false } else { true };
            let mut sm = SocialMessg {
                act_nr: 0,
                hide,
                min_victim_position,
                char_no_arg: Rc::from(""),
                others_no_arg: Rc::from(""),
                char_found: Rc::from(""),
                others_found: Rc::from(""),
                vict_found: Rc::from(""),
                not_found: Rc::from(""),
                char_auto: Rc::from(""),
                others_auto: Rc::from(""),
            };

            /* read the stuff */
            sm.act_nr =
                find_command(next_soc).expect(format!("Cannot find command {next_soc}").as_str());
            let nr = sm.act_nr as i32;
            sm.char_no_arg = fread_action(&mut reader, nr);
            sm.others_no_arg = fread_action(&mut reader, nr);
            sm.char_found = fread_action(&mut reader, nr);

            /* if no char_found, the rest is to be ignored */
            if sm.char_found.is_empty() {
                self.soc_mess_list.push(sm);
                line.clear();
                reader.read_line(&mut line).expect("reading social file");
                continue;
            }

            sm.others_found = fread_action(&mut reader, nr);
            sm.vict_found = fread_action(&mut reader, nr);
            sm.not_found = fread_action(&mut reader, nr);
            sm.char_auto = fread_action(&mut reader, nr);
            sm.others_auto = fread_action(&mut reader, nr);

            /* If social not found, re-use this slot.  'curr_soc' will be reincremented. */
            if nr < 0 {
                error!("SYSERR: Unknown social '{}' in social file.", next_soc);
                cur_soc -= 1;
                continue;
            }

            /* If the command we found isn't do_action, we didn't count it for the CREATE(). */
            if CMD_INFO[nr as usize].command_pointer as usize != do_action as usize {
                error!(
                    "SYSERR: Social '{}' already assigned to a command.",
                    next_soc
                );
                cur_soc -= 1;
                continue;
            }
            self.soc_mess_list.push(sm);
            line.clear();
            reader.read_line(&mut line).expect("reading social file");
        }

        /* now, sort 'em */
        self.soc_mess_list.sort_by_key(|e| e.act_nr);
    }
}
