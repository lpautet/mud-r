/* ***********************************************************************
*  File: alias.rs                				A utility to CircleMUD	 *
* Usage: writing/reading player's aliases.              				 *
*	                                    								 *
* Code done by Jeremy Hess and Chad Thompson             				 *
* Modifed by George Greer for inclusion into CircleMUD bpl15.	    	 *
*								                                     	 *
* Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
* CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.		         *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                           *
*********************************************************************** */

use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::rc::Rc;

use log::error;

use crate::interpreter::AliasData;
use crate::structs::CharData;
use crate::util::{get_filename, FileType};

pub fn write_aliases(ch: &CharData) {
    let mut fname = String::new();
    get_filename(&mut fname, FileType::Alias, ch.get_name());
    let res = fs::remove_file(&fname);
    match res {
        Err(e) if e.kind() != ErrorKind::NotFound => {
            error!("Cannot remove alias file {}\n", e);
            return;
        }
        _ => (),
    }

    if ch.player_specials.aliases.is_empty() {
        return;
    }

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&fname);

    if file.is_err() {
        error!(
            "SYSERR: Couldn't save aliases for {} in '{}': {}.",
            ch.get_name(),
            fname,
            file.err().unwrap()
        );
        return;
    }

    let mut file = file.unwrap();

    for temp in ch.player_specials.aliases.iter() {
        let aliaslen = temp.alias.len();
        let repllen = temp.replacement.len();

        let buf = format!(
            "{}\n{}\n{}\n{}\n{}\n",
            aliaslen,
            temp.alias,
            repllen,
            &temp.replacement[1..],
            temp.type_
        );
        file.write_all(buf.as_bytes()).expect("writing alias file");
    }
}

pub fn read_aliases(ch: &mut CharData) {
    let mut xbuf = String::new();
    get_filename(&mut xbuf, FileType::Alias, ch.get_name());

    let r = OpenOptions::new().read(true).open(&xbuf);

    if r.is_err() {
        let err = r.err().unwrap();
        if err.kind() != ErrorKind::NotFound {
            error!(
                "SYSERR: Couldn't open alias file '{}' for {}. {}",
                &xbuf,
                ch.get_name(),
                err
            );
        }
        return;
    }

    let mut reader = BufReader::new(r.unwrap());
    loop {
        let mut t2 = AliasData {
            alias: Rc::from(""),
            replacement: Rc::from(""),
            type_: 0,
        };
        /* Read the aliased command. */
        let mut buf = String::new();
        let mut line = reader.read_line(&mut buf);
        if line.is_err() {
            break;
        }
        if buf.is_empty() {
            // empty line must mean end of file
            break;
        }
        buf.clear();
        line = reader.read_line(&mut buf);
        if line.is_err() {
            break;
        }
        t2.alias = Rc::from(buf.trim_end());

        /* Build the replacement. */
        let mut buf = String::new();
        let mut line = reader.read_line(&mut buf);
        if line.is_err() {
            break;
        }
        buf.clear();
        buf.push(' ');
        line = reader.read_line(&mut buf);
        if line.is_err() {
            break;
        }
        t2.replacement = Rc::from(buf.trim_end());

        /* Figure out the alias type. */
        let mut buf = String::new();
        let line = reader.read_line(&mut buf);
        if line.is_err() {
            break;
        }
        let r = buf.trim_end().parse::<i32>();
        if r.is_err() {
            error!(
                "Error with alias '{}' type: '{}' ({})",
                t2.alias,
                buf.trim_end(),
                t2.replacement
            );
            break;
        }
        t2.type_ = r.unwrap();
        ch.player_specials.aliases.push(t2);
    }
}

pub fn delete_aliases(charname: &str) {
    let mut filename = String::new();

    if !get_filename(&mut filename, FileType::Alias, charname) {
        return;
    }

    let r = fs::remove_file(&filename);

    if r.is_err() {
        let err = r.err().unwrap();
        if err.kind() != ErrorKind::NotFound {
            error!("SYSERR: deleting alias file {}: {}", filename, err);
        }
    }
}
