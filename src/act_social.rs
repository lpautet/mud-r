/* ************************************************************************
*   File: act.social.rs                                 Part of CircleMUD *
*  Usage: Functions to handle socials                                     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
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
use crate::{Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_SLEEP, TO_VICT};

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

pub fn do_action(game: &mut Game, ch: &Rc<CharData>, argument: &str, cmd: usize, _subcmd: i32) {
    let act_nr;

    if {
        act_nr = find_action(&game.db, cmd);
        act_nr.is_none()
    } {
        game.send_to_char(ch, "That action is not supported.\r\n");
        return;
    }
    let act_nr = act_nr.unwrap();
    let action_char_found = game.db.soc_mess_list[act_nr].char_found.clone();
    let action_others_found = game.db.soc_mess_list[act_nr].others_found.clone();
    let action_vict_found = game.db.soc_mess_list[act_nr].vict_found.clone();
    let action_others_no_arg = game.db.soc_mess_list[act_nr].others_no_arg.clone();
    let action_not_found = game.db.soc_mess_list[act_nr].not_found.clone();
    let action_char_auto = game.db.soc_mess_list[act_nr].char_auto.clone();
    let action_others_auto = game.db.soc_mess_list[act_nr].others_auto.clone();
    let action_min_victim_position = game.db.soc_mess_list[act_nr].min_victim_position.clone();
    let action_char_no_arg = game.db.soc_mess_list[act_nr].char_no_arg.clone();
    let action_hide = game.db.soc_mess_list[act_nr].hide;



    let mut buf = String::new();
    if !action_char_found.is_empty() && !argument.is_empty() {
        one_argument(argument, &mut buf);
    }

    if buf.is_empty() {
        game.send_to_char(ch, format!("{}\r\n", action_char_no_arg).as_str());
        game.act(
            &action_others_no_arg,
            action_hide,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        return;
    }
    let vict;
    if {
        vict = game.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        game.send_to_char(ch, format!("{}\r\n", &action_not_found).as_str());
    } else if Rc::ptr_eq(vict.as_ref().unwrap(), ch) {
        game.send_to_char(ch, format!("{}\r\n", &action_char_auto).as_str());
        game.act(
            &action_others_auto,
            action_hide,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
    } else {
        let vict = vict.as_ref().unwrap();
        if vict.get_pos() < action_min_victim_position as u8 {
            game.act(
                "$N is not in a proper position for that.",
                false,
                Some(ch),
                None,
                Some(vict),
                TO_CHAR | TO_SLEEP,
            );
        } else {
            game.act(
                &action_char_found,
                false,
                Some(ch),
                None,
                Some(vict),
                TO_CHAR | TO_SLEEP,
            );
            game.act(
                &action_others_found,
                action_hide,
                Some(ch),
                None,
                Some(vict),
                TO_NOTVICT,
            );
            game.act(
                &action_vict_found,
                action_hide,
                Some(ch),
                None,
                Some(vict),
                TO_VICT,
            );
        }
    }
}

pub fn do_insult(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if !arg.is_empty() {
        let victim;
        if {
            victim = game.get_char_vis(ch, &mut arg, None, FIND_CHAR_ROOM);
            victim.is_none()
        } {
            game.send_to_char(ch, "Can't hear you!\r\n");
        } else {
            let victim = victim.as_ref().unwrap();
            if !Rc::ptr_eq(victim, ch) {
                game.send_to_char(
                    ch,
                    format!("You insult {}.\r\n", victim.get_name()).as_str(),
                );

                match rand_number(0, 2) {
                    0 => {
                        if ch.get_sex() == SEX_MALE {
                            if victim.get_sex() == SEX_MALE {
                                game.act(
                                    "$n accuses you of fighting like a woman!",
                                    false,
                                    Some(ch),
                                    None,
                                    Some(victim),
                                    TO_VICT,
                                );
                            } else {
                                game.act(
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
                                game.act(
                                    "$n accuses you of having the smallest... (brain?)",
                                    false,
                                    Some(ch),
                                    None,
                                    Some(victim),
                                    TO_VICT,
                                );
                            } else {
                                game.act("$n tells you that you'd lose a beauty contest against a troll.",
                                       false, Some(ch), None, Some(victim), TO_VICT);
                            }
                        }
                    }
                    1 => {
                        game.act(
                            "$n calls your mother a bitch!",
                            false,
                            Some(ch),
                            None,
                            Some(victim),
                            TO_VICT,
                        );
                    }
                    _ => {
                        game.act(
                            "$n tells you to get lost!",
                            false,
                            Some(ch),
                            None,
                            Some(victim),
                            TO_VICT,
                        );
                    }
                } /* end switch */

                game.act(
                    "$n insults $N.",
                    true,
                    Some(ch),
                    None,
                    Some(victim),
                    TO_NOTVICT,
                );
            } else {
                /* ch == victim */
                game.send_to_char(ch, "You feel insulted.\r\n");
            }
        }
    } else {
        game.send_to_char(ch, "I'm sure you don't want to insult *everybody*...\r\n");
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

pub fn free_social_messages(db: &mut DB) {
    db.soc_mess_list.clear();
}

pub fn boot_social_messages(db: &mut DB) {
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

    db.soc_mess_list.reserve_exact(list_top + 1);
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
            db.soc_mess_list.push(sm);
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
        db.soc_mess_list.push(sm);
        line.clear();
        reader.read_line(&mut line).expect("reading social file");
    }

    /* now, sort 'em */
    db.soc_mess_list.sort_by_key(|e| e.act_nr);
}
