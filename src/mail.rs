/* ************************************************************************
*   File: mail.rs                                       Part of CircleMUD *
*  Usage: header file for mail system                                     *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

/******* MUD MAIL SYSTEM HEADER FILE **********************
 ***     written by Jeremy Elson (jelson@circlemud.org) ***
 *********************************************************/

/* You can modify the following constants to fit your own MUD.  */

/* minimum level a player must be to send mail	*/
use std::fs::OpenOptions;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::rc::Rc;
use std::{mem, process, slice};

use crate::depot::{Depot, DepotId, HasId};
use crate::{ObjData, TextData, VictimRef};
use log::{error, info};

use crate::db::{clear_char, copy_to_stored, parse_c_string, store_to_char, DB, MAIL_FILE};
use crate::interpreter::{cmd_is, one_argument};
use crate::structs::{
    CharData, CharFileU, MeRef, ITEM_NOTE, ITEM_WEAR_HOLD, ITEM_WEAR_TAKE, NOTHING, PLR_DELETED,
    PLR_MAILING,
};
use crate::util::{ctime, time_now, touch};
use crate::{Game, TO_ROOM, TO_VICT};

const MIN_MAIL_LEVEL: i32 = 2;

/* # of gold coins required to send mail	*/
const STAMP_PRICE: i32 = 150;

/* Maximum size of mail in bytes (arbitrary)	*/
const MAX_MAIL_SIZE: usize = 4096;

/* size of mail file allocation blocks		*/
const BLOCK_SIZE: usize = 100;

/*
 * NOTE:  Make sure that your block size is big enough -- if not,
 * HEADER_BLOCK_DATASIZE will end up negative.  This is a bad thing.
 * Check the define below to make sure it is >0 when choosing values
 * for NAME_SIZE and BLOCK_SIZE.  100 is a nice round number for
 * BLOCK_SIZE and is the default ... why bother trying to change it
 * anyway?
 *
 * The mail system will always allocate disk space in chunks of size
 * BLOCK_SIZE.
 */

/* USER CHANGABLE DEFINES ABOVE **
***************************************************************************
**   DON'T TOUCH DEFINES BELOW  */

const HEADER_BLOCK: i32 = -1;
const LAST_BLOCK: i64 = -2;
const DELETED_BLOCK: i32 = -3;

/*
 * note: next_block is part of header_blk in a data block; we can't combine
 * them here because we have to be able to differentiate a data block from a
 * header block when booting mail system.
 */
#[repr(C, packed)]
struct HeaderDataType {
    next_block: i64,
    from: i64,
    /* idnum of the mail's sender		*/
    to: i64,
    /* idnum of mail's recipient		*/
    mail_time: u64,
    /* when was the letter mailed?		*/
}

/* size of the data part of a header block */
const HEADER_BLOCK_DATASIZE: usize =
    BLOCK_SIZE - mem::size_of::<i64>() - mem::size_of::<HeaderDataType>() - mem::size_of::<u8>();

/* size of the data part of a data block */
const DATA_BLOCK_DATASIZE: usize = BLOCK_SIZE - mem::size_of::<i64>() - mem::size_of::<u8>();

#[repr(C, packed)]
struct HeaderBlockType {
    block_type: i64,
    /* is this a header or data block?	*/
    header_data: HeaderDataType,
    /* other header data		*/
    txt: [u8; HEADER_BLOCK_DATASIZE + 1],
    /* actual text plus 1 for None	*/
}

#[repr(C, packed)]
struct DataBlockType {
    block_type: i64,
    /* -1 if header block, -2 if last data block
    in mail, otherwise a link to the next */
    txt: [u8; DATA_BLOCK_DATASIZE + 1],
    /* actual text plus 1 for None	*/
}

struct MailIndexType {
    recipient: i64,
    /* who is this mail for?	*/
    position_list: Vec<u64>,
}

pub struct MailSystem {
    mail_index: Vec<MailIndexType>,
    free_list: Vec<u64>,
    file_end_pos: u64,
}

impl MailSystem {
    pub fn new() -> MailSystem {
        MailSystem {
            mail_index: vec![],
            free_list: vec![],
            file_end_pos: 0,
        }
    }
}

/* -------------------------------------------------------------------------- */

fn mail_recip_ok(game: &mut Game, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>,  name: &str) -> bool {
    let mut ret = false;
    let mut tmp_store = CharFileU::new();
    let mut victim = CharData::default();
    clear_char(&mut victim);
    if db.load_char(name, &mut tmp_store).is_some() {
        store_to_char(texts, &tmp_store, &mut victim);
        let victim = &Rc::from(victim);
        db.char_to_room(objs,victim.id(), 0);
        if !victim.plr_flagged(PLR_DELETED) {
            ret = true;
        }
        game.extract_char_final(db,texts,objs,victim.id());
    }
    ret
}

/*
 * void push_free_list(long #1)
 * #1 - What byte offset into the file the block resides.
 *
 * Net effect is to store a list of free blocks in the mail file in a linked
 * list.  This is called when people receive their messages and at startup
 * when the list is created.
 */
impl MailSystem {
    fn push_free_list(&mut self, pos: u64) {
        self.free_list.push(pos);
    }

    /*
     * long pop_free_list(none)
     * Returns the offset of a free block in the mail file.
     *
     * Typically used whenever a person mails a message.  The blocks are not
     * guaranteed to be sequential or in any order at all.
     */
    fn pop_free_list(&mut self) -> u64 {
        /*
         * If we don't have any free blocks, we append to the file.
         */
        if self.free_list.is_empty() {
            return self.file_end_pos;
        }
        return self.free_list.remove(0);
    }

    pub(crate) fn clear_free_list(&mut self) {
        while !self.free_list.is_empty() {
            self.pop_free_list();
        }
    }

    /*
     * main_index_type *find_char_in_index(long #1)
     * #1 - The idnum of the person to look for.
     * Returns a pointer to the mail block found.
     *
     * Finds the first mail block for a specific person based on id number.
     */
    fn find_char_in_index(&mut self, searchee: i64) -> Option<usize> {
        if searchee < 0 {
            error!(
                "SYSERR: Mail system -- non fatal error #1 (searchee == {}).",
                searchee
            );
            return None;
        }
        self.mail_index
            .iter_mut()
            .position(|e| e.recipient == searchee)
    }
}
impl DB {
    /*
     * void write_to_file(void * #1, int #2, long #3)
     * #1 - A pointer to the data to write, usually the 'block' record.
     * #2 - How much to write (because we'll write NUL terminated strings.)
     * #3 - What offset (block position) in the file to write to.
     *
     * Writes a mail block back into the database at the given location.
     */
    fn write_to_file(&mut self, slice: &[u8], filepos: u64) {
        if filepos % BLOCK_SIZE as u64 != 0 {
            error!(
                "SYSERR: Mail system -- fatal error #2!!! (invalid file position {})",
                filepos
            );
            self.no_mail = true;
            return;
        }
        let mail_file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(MAIL_FILE);
        if mail_file.is_err() {
            error!(
                "SYSERR: Unable to open mail file '{}': {}",
                MAIL_FILE,
                mail_file.err().unwrap()
            );
            self.no_mail = true;
            return;
        }
        let mut mail_file = mail_file.unwrap();

        mail_file
            .write_all_at(slice, filepos)
            .expect("Writing mail file");

        /* find end of file */
        mail_file
            .seek(SeekFrom::End(0))
            .expect("Seeking to the end of mail file");
        self.mails.file_end_pos = mail_file
            .stream_position()
            .expect("getting stream position of mail file");
        return;
    }

    /*
     * void read_from_file(void * #1, int #2, long #3)
     * #1 - A pointer to where we should store the data read.
     * #2 - How large the block we're reading is.
     * #3 - What position in the file to read.
     *
     * This reads a block from the mail database file.
     */
    fn read_from_file(&mut self, slice: &mut [u8], filepos: u64) {
        if filepos % BLOCK_SIZE as u64 != 0 {
            error!(
                "SYSERR: Mail system -- fatal error #2!!! (invalid file position {})",
                filepos
            );
            self.no_mail = true;
            return;
        }
        let mail_file = OpenOptions::new().read(true).open(MAIL_FILE);
        if mail_file.is_err() {
            error!(
                "SYSERR: Unable to open mail file '{}': {}",
                MAIL_FILE,
                mail_file.err().unwrap()
            );
            self.no_mail = true;
            return;
        }
        let mail_file = mail_file.unwrap();

        mail_file
            .read_exact_at(slice, filepos)
            .expect("Reading mail file");
    }
}
impl MailSystem {
    fn index_mail(&mut self, id_to_index: i64, pos: u64) {
        if id_to_index < 0 {
            error!(
                "SYSERR: Mail system -- non-fatal error #4. (id_to_index == {})",
                id_to_index
            );
            return;
        }
        let new_index = self.find_char_in_index(id_to_index);
        if new_index.is_none() {
            /* name not already in index.. add it */

            let new = MailIndexType {
                recipient: id_to_index,
                position_list: vec![],
            };
            /* add to front of list */
            self.mail_index.insert(0, new);
        }
        /* now, add this position to front of position list */
        self.mail_index[0].position_list.insert(0, pos);
    }

    /*
     * int scan_file(none)
     * Returns false if mail file is corrupted or true if everything correct.
     *
     * This is called once during boot-up.  It scans through the mail file
     * and indexes all entries currently in the mail file.
     */
    pub fn scan_file(&mut self) -> bool {
        let mut total_messages = 0;
        let mut block_num = 0 as u64;

        let mail_file = OpenOptions::new().read(true).open(MAIL_FILE);
        if mail_file.is_err() {
            info!("   Mail file non-existant... creating new file.");
            touch(Path::new(MAIL_FILE)).expect("Creating mail file");
            return true;
        }
        let mut mail_file = mail_file.unwrap();
        loop {
            let mut next_block = HeaderBlockType {
                block_type: 0,
                header_data: HeaderDataType {
                    next_block: 0,
                    from: 0,
                    to: 0,
                    mail_time: 0,
                },
                txt: [0; HEADER_BLOCK_DATASIZE + 1],
            };

            let header_slice;
            unsafe {
                header_slice = slice::from_raw_parts_mut(
                    &mut next_block as *mut _ as *mut u8,
                    mem::size_of::<HeaderBlockType>(),
                );
            }
            let r = mail_file.read_exact(header_slice);
            if r.is_err() {
                let err = r.err().unwrap();
                if err.kind() == ErrorKind::UnexpectedEof {
                    break;
                }
                error!("Wrror while reading mail header file");
                return false;
            }

            if next_block.block_type == HEADER_BLOCK as i64 {
                self.index_mail(next_block.header_data.to, block_num * BLOCK_SIZE as u64);
                total_messages += 1;
            } else if next_block.block_type == DELETED_BLOCK as i64 {
                self.push_free_list(block_num * BLOCK_SIZE as u64);
            }
            block_num += 1;
        }

        self.file_end_pos = mail_file.stream_position().unwrap();
        info!("   {} bytes read.", self.file_end_pos);
        if self.file_end_pos % BLOCK_SIZE as u64 != 0 {
            error!("SYSERR: Error booting mail system -- Mail file corrupt!");
            error!("SYSERR: Mail disabled!");
            return false;
        }
        info!("   Mail file read -- {} messages.", total_messages);
        true
    } /* end of scan_file */

    /*
     * int has_mail(long #1)
     * #1 - id number of the person to check for mail.
     * Returns true or false.
     *
     * A simple little function which tells you if the guy has mail or not.
     */
    pub fn has_mail(&mut self, recipient: i64) -> bool {
        self.find_char_in_index(recipient).is_some()
    }
}
impl DB {
    /*
     * void store_mail(long #1, long #2, char * #3)
     * #1 - id number of the person to mail to.
     * #2 - id number of the person the mail is from.
     * #3 - The actual message to send.
     *
     * call store_mail to store mail.  (hard, huh? :-) )  Pass 3 arguments:
     * who the mail is to (long), who it's from (long), and a pointer to the
     * actual message text (char *).
     */
    pub(crate) fn store_mail(&mut self, to: i64, from: i64, message_pointer: &str) {
        let msg_txt = message_pointer;

        let total_length = message_pointer.len();

        if mem::size_of::<HeaderBlockType>() != mem::size_of::<DataBlockType>()
            || mem::size_of::<HeaderBlockType>() != BLOCK_SIZE
        {
            error!("MAIL SYSTEM IS BROKEN !");
            process::exit(1);
        }

        if from < 0 || to < 0 || message_pointer.is_empty() {
            error!(
                "SYSERR: Mail system -- non-fatal error #5. (from == {}, to == {})",
                from, to
            );
            return;
        }
        let mut header = HeaderBlockType {
            block_type: HEADER_BLOCK as i64,
            header_data: HeaderDataType {
                next_block: LAST_BLOCK,
                from,
                to,
                mail_time: time_now(),
            },
            txt: [0; HEADER_BLOCK_DATASIZE + 1],
        };

        copy_to_stored(&mut header.txt, msg_txt);

        let mut target_address = self.mails.pop_free_list(); /* find next free block */
        self.mails.index_mail(to, target_address); /* add it to mail index in memory */
        let slice;
        unsafe {
            slice = slice::from_raw_parts(
                &header as *const _ as *const u8,
                mem::size_of::<HeaderBlockType>(),
            );
        }
        self.write_to_file(slice, target_address);

        if msg_txt.len() <= HEADER_BLOCK_DATASIZE {
            return; /* that was the whole message */
        }

        let mut bytes_written = HEADER_BLOCK_DATASIZE;
        let mut msg_txt = &msg_txt[HEADER_BLOCK_DATASIZE..]; /* move pointer to next bit of text */

        /*
         * find the next block address, then rewrite the header to reflect where
         * the next block is.
         */
        let mut last_address = target_address;
        target_address = self.mails.pop_free_list();
        header.header_data.next_block = target_address as i64;
        let slice;
        unsafe {
            slice = slice::from_raw_parts(
                &header as *const _ as *const u8,
                mem::size_of::<HeaderBlockType>(),
            );
        }
        self.write_to_file(slice, last_address);

        /* now write the current data block */
        let mut data = DataBlockType {
            block_type: LAST_BLOCK,
            txt: [0; DATA_BLOCK_DATASIZE + 1],
        };

        let copied = copy_to_stored(&mut data.txt, msg_txt);
        data.txt[DATA_BLOCK_DATASIZE] = 0;
        let slice;
        unsafe {
            slice = slice::from_raw_parts(
                &header as *const _ as *const u8,
                mem::size_of::<DataBlockType>(),
            );
        }
        self.write_to_file(slice, target_address);
        bytes_written += copied;
        msg_txt = &msg_txt[copied..];

        /*
         * if, after 1 header block and 1 data block there is STILL part of the
         * message left to write to the file, keep writing the new data blocks and
         * rewriting the old data blocks to reflect where the next block is.  Yes,
         * this is kind of a hack, but if the block size is big enough it won't
         * matter anyway.  Hopefully, MUD players won't pour their life stories out
         * into the Mud Mail System anyway.
         *
         * Note that the block_type data field in data blocks is either a number >=0,
         * meaning a link to the next block, or LAST_BLOCK flag (-2) meaning the
         * last block in the current message.  This works much like DOS' FAT.
         */
        while bytes_written < total_length {
            last_address = target_address;
            target_address = self.mails.pop_free_list();

            /* rewrite the previous block to link it to the next */
            data.block_type = target_address as i64;
            let slice;
            unsafe {
                slice = slice::from_raw_parts(
                    &data as *const _ as *const u8,
                    mem::size_of::<DataBlockType>(),
                );
            }
            self.write_to_file(slice, last_address);

            /* now write the next block, assuming it's the last.  */
            data.block_type = LAST_BLOCK;
            let copied = copy_to_stored(&mut data.txt, msg_txt);
            data.txt[DATA_BLOCK_DATASIZE] = 0;
            let slice;
            unsafe {
                slice = slice::from_raw_parts(
                    &data as *const _ as *const u8,
                    mem::size_of::<DataBlockType>(),
                );
            }
            self.write_to_file(slice, target_address);

            bytes_written += copied;
            msg_txt = &msg_txt[copied..];
        }
    } /* store mail */

    /*
     * char *read_delete(long #1)
     * #1 - The id number of the person we're checking mail for.
     * Returns the message text of the mail received.
     *
     * Retrieves one messsage for a player. The mail is then discarded from
     * the file and the mail index.
     */
    fn read_delete(&mut self, recipient: i64) -> Option<String> {
        if recipient < 0 {
            error!(
                "SYSERR: Mail system -- non-fatal error #6. (recipient: {})",
                recipient
            );
            return None;
        }
        let mail_idx = self.mails.find_char_in_index(recipient);
        if mail_idx.is_none() {
            error!("SYSERR: Mail system -- post office spec_proc error?  Error #7. (invalid character in index)");
            return None;
        }
        let mail_idx = mail_idx.unwrap();
        let mail_pointer = &mut self.mails.mail_index[mail_idx];
        let position_pointer = mail_pointer.position_list.get(0);
        if position_pointer.is_none() {
            error!("SYSERR: Mail system -- non-fatal error #8. (invalid position pointer)");
            return None;
        }
        let position_pointer = position_pointer.unwrap();
        let mut mail_address;
        if mail_pointer.position_list.len() == 1 {
            /* just 1 entry in list. */
            mail_address = *position_pointer;

            self.mails.mail_index.remove(mail_idx);
        } else {
            mail_address = mail_pointer
                .position_list
                .remove(mail_pointer.position_list.len() - 1);
        }

        let mut header = HeaderBlockType {
            block_type: 0,
            header_data: HeaderDataType {
                next_block: 0,
                from: 0,
                to: 0,
                mail_time: 0,
            },
            txt: [0; HEADER_BLOCK_DATASIZE + 1],
        };

        /* ok, now lets do some readin'! */
        let slice;
        unsafe {
            slice = slice::from_raw_parts_mut(
                &mut header as *mut _ as *mut u8,
                mem::size_of::<HeaderBlockType>(),
            );
        }
        self.read_from_file(slice, mail_address);

        if header.block_type != HEADER_BLOCK as i64 {
            let bt = header.block_type;
            error!("SYSERR: Oh dear. (Header block {} != {})", bt, HEADER_BLOCK);
            self.no_mail = true;
            error!("SYSERR: Mail system disabled!  -- Error #9. (Invalid header block.)");
            return None;
        }
        let tmstr = ctime(header.header_data.mail_time);

        let from = self.get_name_by_id(header.header_data.from);
        let to = self.get_name_by_id(recipient);

        let mut buf = format!(
            " * * * * Midgaard Mail System * * * *\r\n\
Date: {}\r\n\
  To: {}\r\n\
From: {}\r\n\
\r\n\
{}",
            tmstr,
            if to.is_some() {
                to.unwrap()
            } else {
                "Unknown"
            },
            if from.is_some() {
                from.unwrap()
            } else {
                "Unknown"
            },
            parse_c_string(&header.txt)
        );

        let mut following_block = header.header_data.next_block;

        /* mark the block as deleted */
        header.block_type = DELETED_BLOCK as i64;
        let slice;
        unsafe {
            slice = slice::from_raw_parts(
                &header as *const _ as *const u8,
                mem::size_of::<HeaderBlockType>(),
            );
        }
        self.write_to_file(slice, mail_address);
        self.mails.push_free_list(mail_address);

        while following_block != LAST_BLOCK {
            let mut data = DataBlockType {
                block_type: 0,
                txt: [0; DATA_BLOCK_DATASIZE + 1],
            };
            let slice;
            unsafe {
                slice = slice::from_raw_parts_mut(
                    &mut data as *mut _ as *mut u8,
                    mem::size_of::<DataBlockType>(),
                );
            }
            self.read_from_file(slice, following_block as u64);

            buf.push_str(parse_c_string(&data.txt).as_str()); /* strcat: OK (data.txt:DATA_BLOCK_DATASIZE < buf:MAX_MAIL_SIZE) */
            mail_address = following_block as u64;
            following_block = data.block_type;
            data.block_type = DELETED_BLOCK as i64;
            let slice;
            unsafe {
                slice = slice::from_raw_parts(
                    &data as *const _ as *const u8,
                    mem::size_of::<DataBlockType>(),
                );
            }
            self.write_to_file(slice, mail_address);
            self.mails.push_free_list(mail_address);
        }

        Some(buf)
    }
}

/****************************************************************
* Below is the spec_proc for a postmaster using the above       *
* routines.  Written by Jeremy Elson (jelson@circlemud.org) *
****************************************************************/

pub fn postmaster(game: &mut Game, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>,  chid: DepotId, me: MeRef, cmd: i32, argument: &str) -> bool {
    let ch = db.ch(chid);
    if ch.desc.is_none() || ch.is_npc() {
        return false; /* so mobs don't get caught here */
    }

    if !(cmd_is(cmd, "mail") || cmd_is(cmd, "check") || cmd_is(cmd, "receive")) {
        return false;
    }
    if db.no_mail {
        game.send_to_char(ch,
            "Sorry, the mail system is having technical difficulties.\r\n",
        );
        return false;
    }

    return if cmd_is(cmd, "mail") {
        match me {
            MeRef::Char(mailman) => postmaster_send_mail(game, db,texts,objs,chid, mailman, cmd, argument),
            _ => panic!("Unexpected MeRef type in postmaster"),
        }
        true
    } else if cmd_is(cmd, "check") {
        match me {
            MeRef::Char(mailman) => postmaster_check_mail(game,db, chid, mailman, cmd, argument),
            _ => panic!("Unexpected MeRef type in postmaster"),
        }
        true
    } else if cmd_is(cmd, "receive") {
        match me {
            MeRef::Char(mailman) => postmaster_receive_mail(game, db,texts,objs,chid, mailman, cmd, argument),
            _ => panic!("Unexpected MeRef type in postmaster"),
        }
        true
    } else {
        false
    };
}

fn postmaster_send_mail(
    game: &mut Game, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    mailman_id: DepotId,
    _cmd: i32,
    arg: &str,
) {
    let ch = db.ch(chid);
    let mailman = db.ch(mailman_id);
    if ch.get_level() < MIN_MAIL_LEVEL as u8 {
        let buf = format!(
            "$n tells you, 'Sorry, you have to be level {} to send mail!'",
            MIN_MAIL_LEVEL
        );
        game.act(db,
            &buf,
            false,
            Some(mailman),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        return;
    }
    let mut buf = String::new();
    one_argument(arg, &mut buf);

    if buf.is_empty() {
        /* you'll get no argument from me! */
        game.act(db,
            "$n tells you, 'You need to specify an addressee!'",
            false,
            Some(mailman),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        return;
    }
    if ch.get_gold() < STAMP_PRICE {
        let buf = format!(
            "$n tells you, 'A stamp costs {} coin{}.'\r\n\
$n tells you, '...which I see you can't afford.'",
            STAMP_PRICE,
            if STAMP_PRICE == 1 { "" } else { "s" }
        );
        game.act(db,
            &buf,
            false,
            Some(mailman),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        return;
    }
    let recipient = db.get_id_by_name(&buf);
    if recipient < 0 || !mail_recip_ok(game, db,texts, objs,&buf) {
        let mailman = db.ch(mailman_id);
        let ch = db.ch(chid);
        game.act(db,
            "$n tells you, 'No one by that name is registered here!'",
            false,
            Some(mailman),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        return;
    }
    let ch = db.ch(chid);
    game.act(db,
        "$n starts to write some mail.",
        true,
        Some(ch),
        None,
        None,
        TO_ROOM,
    );
    let buf = format!(
        "$n tells you, 'I'll take {} coins for the stamp.'\r\n\
$n tells you, 'Write your message, use @ on a new line when done.'",
        STAMP_PRICE
    );
    let mailman = db.ch(mailman_id);
    game.act(db,
        &buf,
        false,
        Some(mailman),
        None,
        Some(VictimRef::Char(ch)),
        TO_VICT,
    );
    let ch = db.ch_mut(chid);
    ch.set_gold(ch.get_gold() - STAMP_PRICE);
    ch.set_plr_flag_bit(PLR_MAILING); /* string_write() sets writing. */

    /* Start writing! */
    let desc_id = ch.desc.unwrap();
    let desc = game.desc_mut(desc_id);
    desc.string_write(
        db,
        texts.add_text(String::new()),
        MAX_MAIL_SIZE,
        recipient,
    );
}

fn postmaster_check_mail(
    game: &mut Game, db: &mut DB,
    chid: DepotId,
    mailman_id: DepotId,
    _cmd: i32,
    _arg: &str,
) {
    let ch = db.ch(chid);
    if db.mails.has_mail(ch.get_idnum()) {
        let ch = db.ch(chid);
        let mailman = db.ch(mailman_id);
        game.act(db,
            "$n tells you, 'You have mail waiting.'",
            false,
            Some(mailman),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
    } else {
        let mailman = db.ch(mailman_id);
        let ch = db.ch(chid);
        game.act(db,
            "$n tells you, 'Sorry, you don't have any mail waiting.'",
            false,
            Some(mailman),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
    }
}

fn postmaster_receive_mail(
    game: &mut Game, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    mailman_id: DepotId,
    _cmd: i32,
    _arg: &str,
) {
    let ch = db.ch(chid);
    if !db.mails.has_mail(ch.get_idnum()) {
        let buf = "$n tells you, 'Sorry, you don't have any mail waiting.'";
        let ch = db.ch(chid);
        let mailman = db.ch(mailman_id);
        game.act(db,
            buf,
            false,
            Some(mailman),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        return;
    }
    while { let ch = db.ch(chid); db.mails.has_mail(ch.get_idnum()) } {
        let oid = db.create_obj(objs,
            NOTHING,
            "mail paper letter",
            "a piece of mail",
            "Someone has left a piece of mail here.",
            ITEM_NOTE,
            ITEM_WEAR_TAKE | ITEM_WEAR_HOLD,
            1,
            30,
            10,
        );
        let ch = db.ch(chid);
        let mail_content = db.read_delete(ch.get_idnum());
        let mail_content = if mail_content.is_some() {
            mail_content.unwrap()
        } else {
            "Mail system error - please report.  Error #11.\r\n".to_string()
        };
        objs.get_mut(oid).action_description = texts.add_text(mail_content);
        db.obj_to_char(objs,oid, chid);
        let mailman = db.ch(mailman_id);
        let ch = db.ch(chid);
        game.act(db,
            "$n gives you a piece of mail.",
            false,
            Some(mailman),
            None,
            Some(VictimRef::Char(ch)),
            TO_VICT,
        );
        game.act(db,
            "$N gives $n a piece of mail.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(mailman)),
            TO_ROOM,
        );
    }
}
