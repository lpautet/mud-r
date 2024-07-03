/* ************************************************************************
*   File: ban.rs                                        Part of CircleMUD *
*  Usage: banning/unbanning/checking sites and player names               *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::process;
use std::rc::Rc;

use log::{error, info};
use regex::Regex;

use crate::db::{BanListElement, BAN_FILE, DB, XNAME_FILE};
use crate::depot::DepotId;
use crate::interpreter::{one_argument, two_arguments};
use crate::structs::ConState::ConPlaying;
use crate::structs::LVL_GOD;
use crate::util::{ctime, time_now, NRM};
use crate::Game;

const BAN_TYPES: [&str; 5] = ["no", "new", "select", "all", "ERROR"];

pub fn load_banned(db: &mut DB) {
    let fl = OpenOptions::new().read(true).open(BAN_FILE);

    if fl.is_err() {
        let err = fl.err().unwrap();
        if err.kind() != ErrorKind::NotFound {
            error!("SYSERR: Unable to open banfile '{}': {}", BAN_FILE, err);
        } else {
            info!("   Ban file '{}' doesn't exist.", BAN_FILE);
        }
        return;
    }

    let mut reader = BufReader::new(fl.unwrap());

    loop {
        let mut line = String::new();
        let r = reader
            .read_line(&mut line)
            .expect("Error while reading ban file");
        if r == 0 {
            break;
        }

        let regex = Regex::new(r"(\S+)\s(\S+)\s#(\d{1,9})\s(\S+)").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!("SYSERR: Format error in ban file");
            process::exit(1);
        }
        let f = f.unwrap();
        let ban_type = &f[1];
        let mut ble = BanListElement {
            site: Rc::from(&f[2]),
            type_: 0,
            date: f[3].parse::<u64>().unwrap(),
            name: Rc::from(&f[4]),
        };

        let bt = BAN_TYPES.iter().position(|e| *e == ban_type);
        ble.type_ = if bt.is_some() { bt.unwrap() } else { 0 } as i32;
        db.ban_list.push(ble);
    }
}

pub fn isbanned(db: &DB, hostname: &str) -> i32 {
    if hostname.is_empty() {
        return 0;
    }
    let hostname = hostname.to_lowercase();
    let mut i = 0;
    db.ban_list
        .iter()
        .filter(|b| hostname.contains(b.site.as_ref()))
        .for_each(|b| i = max(i, b.type_));
    i
}

fn _write_one_node(writer: &mut BufWriter<File>, node: &BanListElement) {
    let buf = format!(
        "{} {} {} {}\n",
        BAN_TYPES[node.type_ as usize], node.site, node.date, node.name
    );
    writer
        .write_all(buf.as_bytes())
        .expect("Error writing ban file");
}

fn write_ban_list(db: &DB) {
    let fl = OpenOptions::new().write(true).create(true).open(BAN_FILE);

    if fl.is_err() {
        let err = fl.err().unwrap();
        error!("SYSERR: Unable to open '{BAN_FILE}' for writing {err}");
        return;
    }
    let mut writer = BufWriter::new(fl.unwrap());
    for ban_node in db.ban_list.iter() {
        _write_one_node(&mut writer, ban_node); /* recursively write from end to start */
    }

    return;
}

macro_rules! ban_list_format {
    () => {
        "{:25}  {:8}  {:10}  {:16}\r\n"
    };
}

pub fn do_ban(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    if argument.is_empty() {
        if db.ban_list.is_empty() {
            game.send_to_char(ch, "No sites are banned.\r\n");
            return;
        }
        game.send_to_char(ch,
            format!(
                ban_list_format!(),
                "Banned Site Name", "Ban Type", "Banned On", "Banned By"
            )
            .as_str(),
        );
        game.send_to_char(ch,
            format!(
                ban_list_format!(),
                "---------------------------------",
                "---------------------------------",
                "---------------------------------",
                "---------------------------------"
            )
            .as_str(),
        );

        for idx in 0..db.ban_list.len() {
            let timestr;
            if db.ban_list[idx].date != 0 {
                timestr = ctime(db.ban_list[idx].date as u64);
            } else {
                timestr = "Unknown".to_string();
            }
            game.send_to_char(ch,
                format!(
                    ban_list_format!(),
                    db.ban_list[idx].site, BAN_TYPES[db.ban_list[idx].type_ as usize], timestr, db.ban_list[idx].name
                )
                .as_str(),
            );
        }
        return;
    }
    let mut flag = String::new();
    let mut site = String::new();
    two_arguments(argument, &mut flag, &mut site);
    if site.is_empty() || flag.is_empty() {
        game.send_to_char(ch, "Usage: ban {all | select | new} site_name\r\n");
        return;
    }
    if !(flag == "select" || flag == "all" || flag == "new") {
        game.send_to_char(ch, "Flag must be ALL, SELECT, or NEW.\r\n");
        return;
    }
    let ban_node = db.ban_list.iter().find(|b| b.site.as_ref() == site);
    if ban_node.is_some() {
        game.send_to_char(ch,
            "That site has already been banned -- unban it to change the ban type.\r\n",
        );
        return;
    }

    let mut ban_node = BanListElement {
        site: Rc::from(site.to_lowercase().as_str()),
        type_: 0,
        date: time_now(),
        name: ch.get_name().clone(),
    };

    let p = BAN_TYPES.iter().position(|t| *t == flag);
    let mut ban_node_type = 0;
    if p.is_some() {
        ban_node_type = p.unwrap();
        ban_node.type_ = ban_node_type as i32;
    }

    db.ban_list.push(ban_node);

    let ch = db.ch(chid);
    game.mudlog(db,
        NRM,
        max(LVL_GOD as i32, ch.get_invis_lev() as i32),
        true,
        format!(
            "{} has banned {} for {} players.",
            ch.get_name(),
            site,
            BAN_TYPES[ban_node_type]
        )
        .as_str(),
    );
    game.send_to_char(ch, "Site banned.\r\n");
    write_ban_list(&db);
}

pub fn do_unban(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    let mut site = String::new();
    one_argument(argument, &mut site);
    if site.is_empty() {
        game.send_to_char(ch, "A site to unban might help.\r\n");
        return;
    }
    let p = db
        .ban_list
        .iter()
        .position(|b| b.site.as_ref() == site);

    if p.is_none() {
        game.send_to_char(ch, "That site is not currently banned.\r\n");
        return;
    }

    let ban_node = db.ban_list.remove(p.unwrap());
    let ch = db.ch(chid);
    game.send_to_char(ch, "Site unbanned.\r\n");
    let ch = db.ch(chid);
    game.mudlog(db,
        NRM,
        max(LVL_GOD as i32, ch.get_invis_lev() as i32),
        true,
        format!(
            "{} removed the {}-player ban on {}.",
            ch.get_name(),
            BAN_TYPES[ban_node.type_ as usize],
            ban_node.site
        )
        .as_str(),
    );

    write_ban_list(&db);
}

/**************************************************************************
 *  Code to check for invalid names (i.e., profanity, etc.)		  *
 *  Written by Sharon P. Goza						  *
 **************************************************************************/

pub fn valid_name<'a>(game: &mut Game, db:&DB,  newname: &str) -> bool {
    /*
     * Make sure someone isn't trying to create this same name.  We want to
     * do a 'str_cmp' so people can't do 'Bob' and 'BoB'.  The creating login
     * will not have a character name yet and other people sitting at the
     * prompt won't have characters yet.
     */
    for dt in game.descriptor_list.iter() {
        let character_id = dt.character;

        if character_id.is_none() {
            continue;
        }

        let character_id = character_id.unwrap();
        let character = db.ch(character_id);

        if character.get_name().as_ref() != "" && character.get_name().as_ref() == newname {
            return dt.state() == ConPlaying;
        }
    }

    /* return valid if list doesn't exist */
    if db.invalid_list.len() == 0 {
        return true;
    }

    /* change to lowercase */
    let tmpname = newname.to_lowercase();

    /* Does the desired name contain a string in the invalid list? */
    for invalid in db.invalid_list.iter() {
        if tmpname.contains(invalid.as_ref()) {
            return false;
        }
    }

    return true;
}

/* What's with the wacky capitalization in here? */
pub fn free_invalid_list(db: &mut DB) {
    db.invalid_list.clear();
}

pub fn read_invalid_list(db: &mut DB) {
    let fp = OpenOptions::new().read(true).open(XNAME_FILE);

    if fp.is_err() {
        let err = fp.err().unwrap();
        error!("SYSERR: Unable to open '{XNAME_FILE}' for reading {err}");
        return;
    }

    let mut reader = BufReader::new(fp.unwrap());

    loop {
        let mut line = String::new();
        let r = reader.read_line(&mut line);
        if r.is_err() {
            error!("Error while reading ban file! {}", r.err().unwrap());
            break;
        }
        if r.unwrap() == 0 {
            break;
        }
        db.invalid_list.push(Rc::from(line));
    }
}
