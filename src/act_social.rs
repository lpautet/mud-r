/* ************************************************************************
*   File: act.social.rs                                 Part of CircleMUD *
*  Usage: Functions to handle socials                                     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader};
use std::process;
use std::rc::Rc;

use log::error;
use regex::Regex;
use crate::depot::{Depot, DepotId, HasId};
use crate::structs::{Position, Sex};
use crate::{act, send_to_char, CharData, ObjData, TextData, VictimRef};

use crate::db::{DB, SOCMESS_FILE};
use crate::handler::{get_char_vis, FindFlags};
use crate::interpreter::{find_command, one_argument, CMD_INFO};
use crate::util::rand_number;
use crate::{Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_SLEEP, TO_VICT};

pub struct SocialMessg {
    act_nr: usize,
    hide: bool,
    min_victim_position: Position,
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

#[allow(clippy::too_many_arguments)]           
pub fn do_action(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, _texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, chid: DepotId, argument: &str, cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);
    let act_nr;

    let res = {
        act_nr = find_action(db, cmd);
        act_nr.is_none()
    }; 
    if res {
        send_to_char(&mut game.descriptors, ch, "That action is not supported.\r\n");
        return;
    }
    let act_nr = act_nr.unwrap();
    let action_char_found = &db.soc_mess_list[act_nr].char_found;
    let action_others_found = &db.soc_mess_list[act_nr].others_found;
    let action_vict_found = &db.soc_mess_list[act_nr].vict_found;
    let action_others_no_arg = &db.soc_mess_list[act_nr].others_no_arg;
    let action_not_found = &db.soc_mess_list[act_nr].not_found;
    let action_char_auto = &db.soc_mess_list[act_nr].char_auto;
    let action_others_auto = &db.soc_mess_list[act_nr].others_auto;
    let action_min_victim_position = db.soc_mess_list[act_nr].min_victim_position;
    let action_char_no_arg = &db.soc_mess_list[act_nr].char_no_arg;
    let action_hide = db.soc_mess_list[act_nr].hide;



    let mut buf = String::new();
    if !action_char_found.is_empty() && !argument.is_empty() {
        one_argument(argument, &mut buf);
    }

    if buf.is_empty() {
        send_to_char(&mut game.descriptors, ch, format!("{}\r\n", action_char_no_arg).as_str());
        act(&mut game.descriptors, chars, db,
            action_others_no_arg,
            action_hide,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        return;
    }
    let vict;
    let res = {
        vict = get_char_vis(&game.descriptors, chars,db,ch, &mut buf, None, FindFlags::CHAR_ROOM);
        vict.is_none()
    }; 
    if res {
        send_to_char(&mut game.descriptors, ch, format!("{}\r\n", &action_not_found).as_str());
    } else if vict.unwrap().id() == chid {
        send_to_char(&mut game.descriptors, ch, format!("{}\r\n", &action_char_auto).as_str());
        act(&mut game.descriptors, chars, db,
            action_others_auto,
            action_hide,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
    } else {
        let vict = vict.unwrap();
        if vict.get_pos() < action_min_victim_position  {
            act(&mut game.descriptors, chars, db,
                "$N is not in a proper position for that.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR | TO_SLEEP,
            );
        } else {
            act(&mut game.descriptors, chars, db,
                action_char_found,
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_CHAR | TO_SLEEP,
            );
            act(&mut game.descriptors, chars, db,
                action_others_found,
                action_hide,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, db,
                action_vict_found,
                action_hide,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]           
pub fn do_insult(game: &mut Game, db: &mut DB,chars: &mut Depot<CharData>, _texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>,  chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = chars.get(chid);
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if !arg.is_empty() {
        let victim;
        let res = {
            victim = get_char_vis(&game.descriptors, chars,db,ch, &mut arg, None, FindFlags::CHAR_ROOM);
            victim.is_none()
        }; 
        if res {
            send_to_char(&mut game.descriptors, ch, "Can't hear you!\r\n");
        } else {
            let victim = victim.unwrap();
            if victim.id() != chid {
                send_to_char(&mut game.descriptors, ch,
                    format!("You insult {}.\r\n", victim.get_name()).as_str(),
                );

                match rand_number(0, 2) {
                    0 => {
                        let ch = chars.get(chid);
                        if ch.get_sex() == Sex::Male {
                            if victim.get_sex() == Sex::Male {
                                act(&mut game.descriptors, chars, db,
                                    "$n accuses you of fighting like a woman!",
                                    false,
                                    Some(ch),
                                    None,
                                    Some(VictimRef::Char(victim)),
                                    TO_VICT,
                                );
                            } else {
                                act(&mut game.descriptors, chars, db,
                                    "$n says that women can't fight.",
                                    false,
                                    Some(ch),
                                    None,
                                    Some(VictimRef::Char(victim)),
                                    TO_VICT,
                                );
                            }
                        } else {
                            /* Ch == Woman */
                            if victim.get_sex() == Sex::Male {
                                act(&mut game.descriptors, chars, db,
                                    "$n accuses you of having the smallest... (brain?)",
                                    false,
                                    Some(ch),
                                    None,
                                    Some(VictimRef::Char(victim)),
                                    TO_VICT,
                                );
                            } else {
                                act(&mut game.descriptors, chars, db,"$n tells you that you'd lose a beauty contest against a troll.",
                                       false, Some(ch), None, Some(VictimRef::Char(victim)), TO_VICT);
                            }
                        }
                    }
                    1 => {
                        act(&mut game.descriptors, chars, db,
                            "$n calls your mother a bitch!",
                            false,
                            Some(ch),
                            None,
                            Some(VictimRef::Char(victim)),
                            TO_VICT,
                        );
                    }
                    _ => {
                        act(&mut game.descriptors, chars, db,
                            "$n tells you to get lost!",
                            false,
                            Some(ch),
                            None,
                            Some(VictimRef::Char(victim)),
                            TO_VICT,
                        );
                    }
                } /* end switch */

                act(&mut game.descriptors, chars, db,
                    "$n insults $N.",
                    true,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(victim)),
                    TO_NOTVICT,
                );
            } else {
                /* ch == victim */
                send_to_char(&mut game.descriptors, ch, "You feel insulted.\r\n");
            }
        }
    } else {
        send_to_char(&mut game.descriptors, ch, "I'm sure you don't want to insult *everybody*...\r\n");
    }
}

pub fn fread_action(reader: &mut BufReader<File>, nr: i32) -> Rc<str> {
    let mut buf = String::new();

    let r = reader
        .read_line(&mut buf)
        .unwrap_or_else(|_| panic!("SYSERR: fread_action: error while reading action #{}", nr));

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
    let res = {
        fl = OpenOptions::new().read(true).open(SOCMESS_FILE);
        fl.is_err()
    }; 
    if res {
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
    for cmd_info in CMD_INFO.iter() {
        if cmd_info.command_pointer as usize == do_action as usize {
            list_top += 1;
        }
    }

    db.soc_mess_list.reserve_exact(list_top + 1);
    let mut cur_soc = 0;
    let mut reader = BufReader::new(fl);
    /* now read 'em */
    let regex = Regex::new(r"^(\S+)\s(\d{1,9})\s(\d{1,9})").unwrap();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("Reading socials file");
        if line.starts_with('$') {
            break;
        }

        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!("SYSERR: format error in social file near social '{}'", line);
            process::exit(1);
        }
        let f = f.unwrap();
        let next_soc = &f[1];
        let hide = f[2].parse::<i32>().unwrap();
        let min_victim_position = Position::from(f[2].parse::<u8>().unwrap());

        let res = {
            cur_soc += 1;
            cur_soc > list_top
        }; 
        if res {
            error!(
                "SYSERR: Ran out of slots in social array. ({} > {})",
                cur_soc, list_top
            );
            break;
        }
        let hide = hide != 0;
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
            find_command(next_soc).unwrap_or_else(|| panic!("Cannot find command {next_soc}"));
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
