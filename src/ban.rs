/* ************************************************************************
*   File: ban.rs                                        Part of CircleMUD *
*  Usage: banning/unbanning/checking sites and player names               *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2024 - 2025 Laurent Pautet                     *
************************************************************************ */

use std::cmp::max;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use std::rc::Rc;

use log::{error, info};
use regex::Regex;

use crate::db::{BanListElement, BanType, BAN_FILE, DB, XNAME_FILE};
use crate::depot::{Depot, DepotId};
use crate::interpreter::{one_argument, two_arguments};
use crate::structs::ConState::ConPlaying;
use crate::structs::LVL_GOD;
use crate::util::{ctime, time_now, DisplayMode};
use crate::{send_to_char, CharData, Game, ObjData, TextData};

const BAN_TYPES: [&str; 5] = ["no", "new", "select", "all", "ERROR"];
const BAN_TYPES_VALUES: [BanType; 4] = [BanType::None, BanType::New, BanType::Select, BanType::All];

pub fn load_banned(db: &mut DB) {
    #[allow(clippy::needless_late_init)]
    let fl;
    match OpenOptions::new().read(true).open(BAN_FILE) {
        Err(err) => {
            if err.kind() != ErrorKind::NotFound {
                error!("SYSERR: Unable to open banfile '{}': {}", BAN_FILE, err);
            } else {
                info!("   Ban file '{}' doesn't exist.", BAN_FILE);
            }
            return;
        }
        Ok(f) => fl = f,
    }

    let mut reader = BufReader::new(fl);

    let regex: Regex = Regex::new(r"(\S+)\s(\S+)\s#(\d{1,9})\s(\S+)")
        .unwrap_or_else(|e| panic!("regex error: {}", e));
    loop {
        let mut line = String::new();
        let r = reader
            .read_line(&mut line)
            .expect("Error while reading ban file");
        if r == 0 {
            break;
        }

        let f = regex
            .captures(line.as_str())
            .unwrap_or_else(|| panic!("SYSERR: Format error in ban file"));
        let ban_type = &f[1];
        let mut ble = BanListElement {
            site: Rc::from(&f[2]),
            type_: BanType::None,
            date: f[3]
                .parse::<u64>()
                .unwrap_or_else(|e| panic!("SYSERR: Format error in ban file: {}", e)),
            name: Rc::from(&f[4]),
        };

        let bt = BAN_TYPES.iter().position(|e| *e == ban_type);
        ble.type_ = if let Some(bt) = bt {
            BAN_TYPES_VALUES[bt]
        } else {
            BanType::None
        };
        db.ban_list.push(ble);
    }
}

pub fn isbanned(db: &DB, hostname: &str) -> BanType {
    if hostname.is_empty() {
        return BanType::None;
    }
    let hostname = hostname.to_lowercase();
    let mut i = BanType::None;
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
    let fl = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(BAN_FILE);

    match fl {
        Err(err) => {
            error!("SYSERR: Unable to open '{BAN_FILE}' for writing {err}");
        }
        Ok(fl) => {
            let mut writer = BufWriter::new(fl);
            for ban_node in db.ban_list.iter() {
                _write_one_node(&mut writer, ban_node); /* recursively write from end to start */
            }
        }
    }
}

macro_rules! ban_list_format {
    () => {
        "{:25}  {:8}  {:10}  {:16}\r\n"
    };
}

#[allow(clippy::too_many_arguments)]
pub fn do_ban(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if argument.is_empty() {
        if db.ban_list.is_empty() {
            send_to_char(&mut game.descriptors, ch, "No sites are banned.\r\n");
            return;
        }
        send_to_char(
            &mut game.descriptors,
            ch,
            format!(
                ban_list_format!(),
                "Banned Site Name", "Ban Type", "Banned On", "Banned By"
            )
            .as_str(),
        );
        send_to_char(
            &mut game.descriptors,
            ch,
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
            let timestr = if db.ban_list[idx].date != 0 {
                ctime(db.ban_list[idx].date)
            } else {
                "Unknown".to_string()
            };
            send_to_char(
                &mut game.descriptors,
                ch,
                format!(
                    ban_list_format!(),
                    db.ban_list[idx].site,
                    BAN_TYPES[db.ban_list[idx].type_ as usize],
                    timestr,
                    db.ban_list[idx].name
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
        send_to_char(
            &mut game.descriptors,
            ch,
            "Usage: ban {all | select | new} site_name\r\n",
        );
        return;
    }
    if !(flag == "select" || flag == "all" || flag == "new") {
        send_to_char(
            &mut game.descriptors,
            ch,
            "Flag must be ALL, SELECT, or NEW.\r\n",
        );
        return;
    }
    let ban_node = db.ban_list.iter().find(|b| b.site.as_ref() == site);
    if ban_node.is_some() {
        send_to_char(
            &mut game.descriptors,
            ch,
            "That site has already been banned -- unban it to change the ban type.\r\n",
        );
        return;
    }

    let mut ban_node = BanListElement {
        site: Rc::from(site.to_lowercase().as_str()),
        type_: BanType::None,
        date: time_now(),
        name: ch.get_name().clone(),
    };

    let p = BAN_TYPES.iter().position(|t| *t == flag);
    let mut ban_node_type = BanType::None;
    if let Some(p) = p {
        ban_node_type = BAN_TYPES_VALUES[p];
        ban_node.type_ = ban_node_type;
    }

    db.ban_list.push(ban_node);

    let ch = chars.get(chid);
    game.mudlog(
        chars,
        DisplayMode::Normal,
        max(LVL_GOD as i32, ch.get_invis_lev() as i32),
        true,
        format!(
            "{} has banned {} for {} players.",
            ch.get_name(),
            site,
            BAN_TYPES[ban_node_type as usize]
        )
        .as_str(),
    );
    send_to_char(&mut game.descriptors, ch, "Site banned.\r\n");
    write_ban_list(db);
}

#[allow(clippy::too_many_arguments)]
pub fn do_unban(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut site = String::new();
    one_argument(argument, &mut site);
    if site.is_empty() {
        send_to_char(&mut game.descriptors, ch, "A site to unban might help.\r\n");
        return;
    }
    let p;
    if let Some(i) = db.ban_list.iter().position(|b| b.site.as_ref() == site) {
        p = i;
    } else {
        send_to_char(
            &mut game.descriptors,
            ch,
            "That site is not currently banned.\r\n",
        );
        return;
    }

    let ban_node = db.ban_list.remove(p);
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch, "Site unbanned.\r\n");
    let ch = chars.get(chid);
    game.mudlog(
        chars,
        DisplayMode::Normal,
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

    write_ban_list(db);
}

/**************************************************************************
 *  Code to check for invalid names (i.e., profanity, etc.)		  *
 *  Written by Sharon P. Goza						  *
 **************************************************************************/

pub fn valid_name(game: &mut Game, chars: &Depot<CharData>, db: &DB, newname: &str) -> bool {
    /*
     * Make sure someone isn't trying to create this same name.  We want to
     * do a 'str_cmp' so people can't do 'Bob' and 'BoB'.  The creating login
     * will not have a character name yet and other people sitting at the
     * prompt won't have characters yet.
     */
    for &dt_id in &game.descriptor_list {
        let dt = game.desc(dt_id);
        if let Some(character_id) = dt.character {
            let character = chars.get(character_id);

            if character.get_name().as_ref() != "" && character.get_name().as_ref() == newname {
                return dt.state() == ConPlaying;
            }
        }
    }

    /* return valid if list doesn't exist */
    if db.invalid_list.is_empty() {
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

    true
}

/* What's with the wacky capitalization in here? */
pub fn free_invalid_list(db: &mut DB) {
    db.invalid_list.clear();
}

pub fn read_invalid_list(db: &mut DB) {
    match OpenOptions::new().read(true).open(XNAME_FILE) {
        Err(err) => {
            error!("SYSERR: Unable to open '{XNAME_FILE}' for reading {err}");
        }
        Ok(fl) => {
            let mut reader = BufReader::new(fl);

            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Err(err) => {
                        error!("Error while reading ban file! {}", err);
                        break;
                    }
                    Ok(r) => {
                        if r == 0 {
                            break;
                        }
                        db.invalid_list.push(Rc::from(line));
                    }
                }
            }
        }
    }
}
