/* ************************************************************************
*   File: act.comm.rs                                   Part of CircleMUD *
*  Usage: Player-level communication commands                             *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use crate::config::{HOLLER_MOVE_COST, LEVEL_CAN_SHOUT, NOPERSON, OK};
use crate::depot::{Depot, DepotId, HasId};
use crate::handler::{get_char_vis, get_obj_in_list_vis, get_player_vis, FIND_CHAR_ROOM, FIND_CHAR_WORLD};
use crate::interpreter::{
    delete_doubledollar, half_chop, two_arguments, CMD_INFO, SCMD_ASK, SCMD_HOLLER, SCMD_QSAY,
    SCMD_SHOUT, SCMD_WHISPER,
};
use crate::screen::{C_CMP, C_NRM, KGRN, KMAG, KNRM, KNUL, KRED, KYEL};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    AFF_GROUP, ITEM_NOTE, ITEM_PEN, LVL_GOD, LVL_IMMORT, MAX_NOTE_LENGTH, NOBODY, PLR_NOSHOUT,
    PLR_WRITING, PRF_COLOR_1, PRF_COLOR_2, PRF_DEAF, PRF_NOAUCT, PRF_NOGOSS, PRF_NOGRATZ,
    PRF_NOREPEAT, PRF_NOTELL, PRF_QUEST, ROOM_SOUNDPROOF, WEAR_HOLD,
};
use crate::util::can_see_obj;
use crate::{act, send_to_char, CharData, DescriptorData, ObjData, TextData, VictimRef, DB};
use crate::{
    _clrlevel, an, clr, Game, CCNRM, CCRED, COLOR_LEV, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_SLEEP,
    TO_VICT,
};

pub fn do_say(
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
    let argument = argument.trim_start();
    let ch = chars.get(chid);

    if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Yes, but WHAT do you want to say?\r\n");
    } else {
        act(&mut game.descriptors, chars, 
            db,
            &format!("$n says, '{}'", argument),
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        if !ch.is_npc() && ch.prf_flagged(PRF_NOREPEAT) {
            send_to_char(&mut game.descriptors, ch, OK);
        } else {
            let mut argument = argument.to_string();
            delete_doubledollar(&mut argument);
            send_to_char(&mut game.descriptors, ch, &format!("You say, '{}'\r\n", argument));
        }
    }
}

pub fn do_gsay(
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
    let argument = argument.trim_start();

    if !ch.aff_flagged(AFF_GROUP) {
        send_to_char(&mut game.descriptors, ch, "But you are not the member of a group!\r\n");
        return;
    }
    if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Yes, but WHAT do you want to group-say?\r\n");
    } else {
        let k_id: DepotId;
        if ch.master.is_some() {
            k_id = ch.master.unwrap();
        } else {
            k_id = chid;
        }

        let buf = format!("$n tells the group, '{}'", argument);
        let k = chars.get(k_id);
        if k.aff_flagged(AFF_GROUP) && k_id != chid {
            act(&mut game.descriptors, chars, 
                db,
                &buf,
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(k)),
                TO_VICT | TO_SLEEP,
            );
        }
        let followers_ids = k.followers.iter().map(|f| f.follower);
        for f_id in followers_ids {
            let f = chars.get(f_id);
            if f.aff_flagged(AFF_GROUP) && f_id != chid {
                act(&mut game.descriptors, chars, 
                    db,
                    &buf,
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(f)),
                    TO_VICT | TO_SLEEP,
                );
            }
        }
        if ch.prf_flagged(PRF_NOREPEAT) {
            send_to_char(&mut game.descriptors, ch, OK);
        } else {
            send_to_char(&mut game.descriptors, ch, &format!("You tell the group, '{}'\r\n", argument));
        }
    }
}

fn perform_tell(
    descs: &mut Depot<DescriptorData>,
    db: & DB,
    chars: &mut Depot<CharData>,
    chid: DepotId,
    vict_id: DepotId,
    arg: &str,
) {
    let ch = chars.get(chid);
    let vict = chars.get(vict_id);
    send_to_char(descs, vict, CCRED!(vict, C_NRM));
    act(descs, chars, 
        db,
        &format!("$n tells you, '{}'", arg),
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_VICT | TO_SLEEP,
    );
    send_to_char(descs, vict, CCNRM!(vict, C_NRM));

    if !ch.is_npc() && ch.prf_flagged(PRF_NOREPEAT) {
        send_to_char(descs, ch, OK);
    } else {
        send_to_char(descs, ch, CCRED!(ch, C_NRM));
        act(descs, chars, 
            db,
            &format!("You tell $N, '{}'", arg),
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR | TO_SLEEP,
        );
        send_to_char(descs, ch, CCNRM!(ch, C_NRM));
    }
    if !vict.is_npc() && !ch.is_npc() {
        let ch_idnum = ch.get_idnum();
        let vict = chars.get_mut(vict_id);
        vict.set_last_tell(ch_idnum);
    }
}

fn is_tell_ok(
    descs: &mut Depot<DescriptorData>, chars: &Depot<CharData>, 
    db: &DB,
    ch: &CharData,
    vict: &CharData,
) -> bool {
    if ch.id() == vict.id() {
        send_to_char(descs, ch, "You try to tell yourself something.\r\n");
    } else if !ch.is_npc() && ch.prf_flagged(PRF_NOTELL) {
        send_to_char(descs, 
            ch,
            "You can't tell other people while you have notell on.\r\n",
        );
    } else if db.room_flagged(ch.in_room(), ROOM_SOUNDPROOF) {
        send_to_char(descs, ch, "The walls seem to absorb your words.\r\n");
    } else if !vict.is_npc() && vict.desc.is_none() {
        /* linkless */
        act(descs, chars, 
            db,
            "$E's linkless at the moment.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR | TO_SLEEP,
        );
    } else if vict.plr_flagged(PLR_WRITING) {
        act(descs, chars, 
            db,
            "$E's writing a message right now; try again later.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR | TO_SLEEP,
        );
    } else if (!vict.is_npc() && vict.prf_flagged(PRF_NOTELL))
        || db.room_flagged(vict.in_room(), ROOM_SOUNDPROOF)
    {
        act(descs, chars, 
            db,
            "$E can't hear you.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR | TO_SLEEP,
        );
    } else {
        return true;
    }

    false
}

/*
 * Yes, do_tell probably could be combined with whisper and ask, but
 * called frequently, and should IMHO be kept as tight as possible.
 */
pub fn do_tell(
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
    let mut buf = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();
    half_chop(&mut argument, &mut buf, &mut buf2);
    let mut vict = None;
    if buf.is_empty() || buf2.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Who do you wish to tell what??\r\n");
    } else if ch.get_level() < LVL_IMMORT as u8 && {
        vict = get_player_vis(&game.descriptors, chars,db, ch, &mut buf, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, NOPERSON);
    } else if ch.get_level() >= LVL_IMMORT as u8 && {
        vict = get_char_vis(&game.descriptors, chars,db, ch, &mut buf, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, NOPERSON);
    } else if is_tell_ok(&mut game.descriptors, chars, db, ch, vict.unwrap()) {
        perform_tell(&mut game.descriptors, db, chars, chid, vict.unwrap().id(), &buf2);
    }
}

pub fn do_reply(
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
    if ch.is_npc() {
        return;
    }

    let argument = argument.trim_start();

    if ch.get_last_tell() == NOBODY as i64 {
        send_to_char(&mut game.descriptors, ch, "You have nobody to reply to!\r\n");
    } else if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "What is your reply?\r\n");
    } else {
        /*
         * Make sure the person you're replying to is still playing by searching
         * for them.  Note, now last tell is stored as player IDnum instead of
         * a pointer, which is much better because it's safer, plus will still
         * work if someone logs out and back in again.
         */

        /*
         * XXX: A descriptor list based search would be faster although
         *      we could not find link dead people.  Not that they can
         *      hear tells anyway. :) -gg 2/24/98
         */
        let last_tell_chid = db
            .character_list
            .iter()
            .map(|&i| chars.get(i))
            .find(|c| !c.is_npc() && c.get_idnum() == ch.get_last_tell());

        if last_tell_chid.is_none() {
            send_to_char(&mut game.descriptors, ch, "They are no longer playing.\r\n");
        } else if is_tell_ok(&mut game.descriptors, chars, db, ch, last_tell_chid.unwrap()) {
            perform_tell(&mut game.descriptors, db, chars, chid, last_tell_chid.unwrap().id(), argument);
        }
    }
}

pub fn do_spec_comm(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);

    let action_sing;
    let action_plur;
    let action_others;

    match subcmd {
        SCMD_WHISPER => {
            action_sing = "whisper to";
            action_plur = "whispers to";
            action_others = "$n whispers something to $N.";
        }

        SCMD_ASK => {
            action_sing = "ask";
            action_plur = "asks";
            action_others = "$n asks $N a question.";
        }

        _ => {
            action_sing = "oops";
            action_plur = "oopses";
            action_others = "$n is tongue-tied trying to speak with $N.";
        }
    }

    let mut argument = argument.to_string();
    let mut buf = String::new();
    let mut buf2 = String::new();

    half_chop(&mut argument, &mut buf, &mut buf2);
    let vict;
    if buf.is_empty() || buf2.is_empty() {
        send_to_char(&mut game.descriptors, 
            ch,
            format!("Whom do you want to {}.. and what??\r\n", action_sing).as_str(),
        );
    } else if {
        vict = get_char_vis(&game.descriptors, chars,db, ch, &mut buf, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, NOPERSON);
    } else if vict.unwrap().id() == chid {
        send_to_char(&mut game.descriptors, 
            ch,
            "You can't get your mouth close enough to your ear...\r\n",
        );
    } else {
        let vict = vict.unwrap();
        let buf1 = format!("$n {} you, '{}'", action_plur, buf2);
        act(&mut game.descriptors, chars, 
            db,
            &buf1,
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_VICT,
        );

        if ch.prf_flagged(PRF_NOREPEAT) {
            send_to_char(&mut game.descriptors, ch, OK);
        } else {
            send_to_char(&mut game.descriptors, 
                ch,
                format!("You {} {}, '{}'\r\n", action_sing, vict.get_name(), buf2).as_str(),
            );
        }
        act(&mut game.descriptors, chars, 
            db,
            action_others,
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_NOTVICT,
        );
    }
}

pub fn do_write(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    texts: &mut Depot<TextData>,
    objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut paper;
    let mut pen = None;
    let mut papername = String::new();
    let mut penname = String::new();

    two_arguments(&argument, &mut papername, &mut penname);

    if ch.desc.is_none() {
        return;
    }

    if papername.is_empty() {
        /* nothing was delivered */
        send_to_char(&mut game.descriptors, 
            ch,
            "Write?  With what?  ON what?  What are you trying to do?!?\r\n",
        );
        return;
    }
    if !penname.is_empty() {
        /* there were two arguments */
        if {
            paper = get_obj_in_list_vis(&game.descriptors, chars,db, objs, ch, &papername, None, &ch.carrying);
            paper.is_none()
        } {
            send_to_char(&mut game.descriptors, ch, format!("You have no {}.\r\n", papername).as_str());
            return;
        }
        if {
            pen = get_obj_in_list_vis(&game.descriptors, chars,db, objs, ch, &penname, None, &ch.carrying);
            pen.is_none()
        } {
            send_to_char(&mut game.descriptors, ch, format!("You have no {}.\r\n", penname).as_str());
            return;
        }
    } else {
        /* there was one arg.. let's see what we can find */
        if {
            paper = get_obj_in_list_vis(&game.descriptors, chars,db, objs, ch, &papername, None, &ch.carrying);
            paper.is_none()
        } {
            send_to_char(&mut game.descriptors, 
                ch,
                format!("There is no {} in your inventory.\r\n", papername).as_str(),
            );
            return;
        }
        if paper.unwrap().get_obj_type() == ITEM_PEN {
            /* oops, a pen.. */
            pen = paper;
            paper = None;
        } else if paper.unwrap().get_obj_type() != ITEM_NOTE {
            send_to_char(&mut game.descriptors, ch, "That thing has nothing to do with writing.\r\n");
            return;
        }
        /* One object was found.. now for the other one. */
        if ch.get_eq(WEAR_HOLD as i8).is_none() {
            send_to_char(&mut game.descriptors, 
                ch,
                format!(
                    "You can't write with {} {} alone.\r\n",
                    an!(papername),
                    papername
                )
                .as_str(),
            );
            return;
        }
        if !can_see_obj(&game.descriptors, chars, db, ch, objs.get(ch.get_eq(WEAR_HOLD as i8).unwrap())) {
            send_to_char(&mut game.descriptors, ch, "The stuff in your hand is invisible!  Yeech!!\r\n");
            return;
        }
        if pen.is_some() {
            paper = Some(objs.get(ch.get_eq(WEAR_HOLD as i8).unwrap()));
        } else {
            pen = Some(objs.get(ch.get_eq(WEAR_HOLD as i8).unwrap()));
        }
    }
    let pen = pen.unwrap();
    let paper = paper.unwrap();

    /* ok.. now let's see what kind of stuff we've found */
    if pen.get_obj_type() != ITEM_PEN {
        act(&mut game.descriptors, chars, 
            db,
            "$p is no good for writing with.",
            false,
            Some(ch),
            Some(pen),
            None,
            TO_CHAR,
        );
    } else if paper.get_obj_type() != ITEM_NOTE {
        act(&mut game.descriptors, chars, 
            db,
            "You can't write on $p.",
            false,
            Some(ch),
            Some(paper),
            None,
            TO_CHAR,
        );
    } else if !texts.get(paper.action_description).text.is_empty() {
        send_to_char(&mut game.descriptors, ch, "There's something written on it already.\r\n");
    } else {
        /* we can write - hooray! */
        send_to_char(&mut game.descriptors, ch, "Write your note.  End with '@' on a new line.\r\n");
        act(&mut game.descriptors, chars, 
            db,
            "$n begins to jot down a note.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let desc_id = ch.desc.unwrap();
        let desc = game.desc_mut(desc_id);
        desc.string_write(chars,  paper.action_description, MAX_NOTE_LENGTH as usize, 0);
    }
}

pub fn do_page(
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
    let mut arg = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut arg, &mut buf2);

    if ch.is_npc() {
        send_to_char(&mut game.descriptors, ch, "Monsters can't page.. go away.\r\n");
    } else if arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "Whom do you wish to page?\r\n");
    } else {
        let buf = format!("\x07\x07*$n* {}", buf2);
        if arg == "all" {
            if ch.get_level() > LVL_GOD as u8 {
                for d_id in game.descriptor_list.clone() {
                    let d = game.desc(d_id);
                    if d.state() == ConPlaying && d.character.is_some()
                    {
                        let vict_id = d.character.unwrap();
                        let vict = chars.get(vict_id);
                        act(&mut game.descriptors, chars, 
                            db,
                            &buf,
                            false,
                            Some(ch),
                            None,
                            Some(VictimRef::Char(vict)),
                            TO_VICT,
                        );
                    } else {
                        send_to_char(&mut game.descriptors, ch, "You will never be godly enough to do that!\r\n");
                    }
                }
                return;
            }
        }
        let vict;
        if {
            vict = get_char_vis(&game.descriptors, chars,db, ch, &mut arg, None, FIND_CHAR_WORLD);
            vict.is_some()
        } {
            let vict = vict.unwrap();
            act(&mut game.descriptors, chars, 
                db,
                &buf,
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT,
            );
            if ch.prf_flagged(PRF_NOREPEAT) {
                send_to_char(&mut game.descriptors, ch, OK);
            } else {
                act(&mut game.descriptors, chars, 
                    db,
                    &buf,
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(vict)),
                    TO_CHAR,
                );
            }
        } else {
            send_to_char(&mut game.descriptors, ch, "There is no such person in the game!\r\n");
        }
    }
}

/**********************************************************************
 * generalized communication func, originally by Fred C. Merkel (Torg) *
 *********************************************************************/

pub fn do_gen_comm(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    // char color_on[24];

    /* Array of flags which must _not_ be set in order for comm to be heard */
    const CHANNELS: [i64; 6] = [0, PRF_DEAF, PRF_NOGOSS, PRF_NOAUCT, PRF_NOGRATZ, 0];

    /*
     * COM_MSGS: [0] Message if you can't perform the action because of noshout
     *           [1] name of the action
     *           [2] message if you're not on the channel
     *           [3] a color string.
     */
    const COM_MSGS: [[&str; 4]; 5] = [
        ["You cannot holler!!\r\n", "holler", "", KYEL],
        [
            "You cannot shout!!\r\n",
            "shout",
            "Turn off your noshout flag first!\r\n",
            KYEL,
        ],
        [
            "You cannot gossip!!\r\n",
            "gossip",
            "You aren't even on the channel!\r\n",
            KYEL,
        ],
        [
            "You cannot auction!!\r\n",
            "auction",
            "You aren't even on the channel!\r\n",
            KMAG,
        ],
        [
            "You cannot congratulate!\r\n",
            "congrat",
            "You aren't even on the channel!\r\n",
            KGRN,
        ],
    ];

    /* to keep pets, etc from being ordered to shout */
    if ch.desc.is_none() {
        return;
    }

    if ch.plr_flagged(PLR_NOSHOUT) {
        send_to_char(&mut game.descriptors, ch, COM_MSGS[subcmd as usize][0]);
        return;
    }
    if db.room_flagged(ch.in_room(), ROOM_SOUNDPROOF) {
        send_to_char(&mut game.descriptors, ch, "The walls seem to absorb your words.\r\n");
        return;
    }
    /* level_can_shout defined in config.c */
    if ch.get_level() < LEVEL_CAN_SHOUT as u8 {
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "You must be at least level {} before you can {}.\r\n",
                LEVEL_CAN_SHOUT, COM_MSGS[subcmd as usize][1]
            )
            .as_str(),
        );
        return;
    }
    /* make sure the char is on the channel */
    if ch.prf_flagged(CHANNELS[subcmd as usize]) {
        send_to_char(&mut game.descriptors, ch, COM_MSGS[subcmd as usize][2]);
        return;
    }
    /* skip leading spaces */
    let argument = argument.trim_start();

    /* make sure that there is something there to say! */
    if argument.is_empty() {
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "Yes, {}, fine, {} we must, but WHAT???\r\n",
                COM_MSGS[subcmd as usize][1], COM_MSGS[subcmd as usize][1]
            )
            .as_str(),
        );
        return;
    }
    if subcmd == SCMD_HOLLER {
        if ch.get_move() < HOLLER_MOVE_COST as i16 {
            send_to_char(&mut game.descriptors, ch, "You're too exhausted to holler.\r\n");
            return;
        } else {
            let ch = chars.get_mut(chid);
            ch.set_move(ch.get_move() - HOLLER_MOVE_COST as i16);
        }
    }
    /* set up the color on code */
    let color_on = COM_MSGS[subcmd as usize][3];

    /* first, set up strings to be given to the communicator */
    let ch = chars.get(chid);
    if ch.prf_flagged(PRF_NOREPEAT) {
        send_to_char(&mut game.descriptors, ch, OK);
    } else {
        let messg = format!(
            "{}You {}, '{}'{}\r\n",
            if COLOR_LEV!(ch) >= C_CMP {
                color_on
            } else {
                ""
            },
            COM_MSGS[subcmd as usize][1],
            argument,
            CCNRM!(ch, C_CMP)
        );
        send_to_char(&mut game.descriptors, ch, messg.as_str());
    }

    let buf1 = format!("$n {}s, '{}'", COM_MSGS[subcmd as usize][1], argument);

    /* now send all the strings out */
    for d_id in game.descriptor_list.clone() {
        let d = game.desc(d_id);
        if d.state() == ConPlaying
            && d_id != ch.desc.unwrap()
            && d.character.is_some()
            && !chars.get(d.character.unwrap())
                .prf_flagged(CHANNELS[subcmd as usize])
            && !chars.get(d.character.unwrap())
                .plr_flagged(PLR_WRITING)
            && !db.room_flagged(
                chars.get(d.character.unwrap()).in_room(),
                ROOM_SOUNDPROOF,
            )
        {
            let ic_id = d.character.unwrap();
            let ic = chars.get(ic_id);
            if subcmd == SCMD_SHOUT
                && (db.world[ch.in_room() as usize].zone != db.world[ic.in_room() as usize].zone
                    || !ic.awake())
            {
                continue;
            }

            if COLOR_LEV!(ic) >= C_NRM {
                send_to_char(&mut game.descriptors, ic, color_on);
            }
            act(&mut game.descriptors, chars, 
                db,
                &buf1,
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(ic)),
                TO_VICT | TO_SLEEP,
            );
            if COLOR_LEV!(ic) >= C_NRM {
                send_to_char(&mut game.descriptors, ic, KNRM);
            }
        }
    }
}

pub fn do_qcomm(
    game: &mut Game,
    db: &mut DB,
    chars: &mut Depot<CharData>,
    _texts: &mut Depot<TextData>,
    _objs: &mut Depot<ObjData>,
    chid: DepotId,
    argument: &str,
    cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.prf_flagged(PRF_QUEST) {
        send_to_char(&mut game.descriptors, ch, "You aren't even part of the quest!\r\n");
        return;
    }
    let argument = argument.trim_start();

    if argument.is_empty() {
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "{}{}?  Yes, fine, {} we must, but WHAT??\r\n",
                CMD_INFO[cmd].command.chars().next().unwrap().to_uppercase(),
                &CMD_INFO[cmd].command[11..],
                CMD_INFO[cmd].command
            )
            .as_str(),
        );
    } else {
        let mut buf;

        if ch.prf_flagged(PRF_NOREPEAT) {
            send_to_char(&mut game.descriptors, ch, OK);
        } else if subcmd == SCMD_QSAY {
            buf = format!("You quest-say, '{}'", argument);
            act(&mut game.descriptors, chars, 
                db,
                &buf,
                false,
                Some(ch),
                None,
                Some(VictimRef::Str(argument)),
                TO_CHAR,
            );
        } else {
            act(&mut game.descriptors, chars, 
                db,
                argument,
                false,
                Some(ch),
                None,
                Some(VictimRef::Str(argument)),
                TO_CHAR,
            );
        }

        if subcmd == SCMD_QSAY {
            buf = format!("$n quest-says, '{}'", argument);
        } else {
            buf = argument.to_string();
        }

        for id in game.descriptor_list.clone() {
            let d = game.desc(id);
            if d.state() == ConPlaying
                && id != ch.desc.unwrap()
                && d.character.is_some()
                && chars.get(d.character.unwrap())
                    .prf_flagged(PRF_QUEST)
            {
                let vict_id = d.character.unwrap();
                let vict = chars.get(vict_id);
                act(&mut game.descriptors, chars, 
                    db,
                    &buf,
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(vict)),
                    TO_VICT | TO_SLEEP,
                );
            }
        }
    }
}
