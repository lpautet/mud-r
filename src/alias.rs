/* ***********************************************************************
*  File: alias.c				A utility to CircleMUD	 *
* Usage: writing/reading player's aliases.				 *
*									 *
* Code done by Jeremy Hess and Chad Thompson				 *
* Modifed by George Greer for inclusion into CircleMUD bpl15.		 *
*									 *
* Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
* CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.		 *
*********************************************************************** */

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::rc::Rc;

use log::error;

use crate::interpreter::AliasData;
use crate::structs::CharData;
use crate::util::{get_filename, ALIAS_FILE};

pub fn write_aliases(ch: &Rc<CharData>) {
    let mut fname = String::new();
    get_filename(&mut fname, ALIAS_FILE, &ch.get_name());
    // remove(fname);

    if ch.player_specials.borrow().aliases.len() == 0 {
        return;
    }

    let file = OpenOptions::new().write(true).open(&fname);

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

    for temp in ch.player_specials.borrow().aliases.iter() {
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

pub fn read_aliases(ch: &Rc<CharData>) {
    let mut xbuf = String::new();
    get_filename(&mut xbuf, ALIAS_FILE, &ch.get_name());

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
        buf.clear();
        line = reader.read_line(&mut buf);
        if line.is_err() {
            break;
        }
        t2.alias = Rc::from(buf);

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
        t2.replacement = Rc::from(buf);

        /* Figure out the alias type. */
        let mut buf = String::new();
        let line = reader.read_line(&mut buf);
        if line.is_err() {
            break;
        }
        let r = buf.parse::<i32>();
        if r.is_err() {
            break;
        }
        t2.type_ = r.unwrap();
        ch.player_specials.borrow_mut().aliases.push(t2);
    }
}

// void delete_aliases(const char *charname)
// {
// char filename[PATH_MAX];
//
// if (!get_filename(filename, sizeof(filename), ALIAS_FILE, charname))
// return;
//
// if (remove(filename) < 0 && errno != ENOENT)
// log("SYSERR: deleting alias file {}: {}", filename, strerror(errno));
// }
//
