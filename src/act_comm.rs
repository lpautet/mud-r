/* ************************************************************************
*   File: act.comm.rs                                   Part of CircleMUD *
*  Usage: Player-level communication commands                             *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use std::rc::Rc;
use crate::VictimRef;
use crate::config::{HOLLER_MOVE_COST, LEVEL_CAN_SHOUT, NOPERSON, OK};
use crate::handler::{FIND_CHAR_ROOM, FIND_CHAR_WORLD};
use crate::interpreter::{
    delete_doubledollar, half_chop, two_arguments, CMD_INFO, SCMD_ASK, SCMD_HOLLER, SCMD_QSAY,
    SCMD_SHOUT, SCMD_WHISPER,
};
use crate::modify::string_write;
use crate::screen::{C_CMP, C_NRM, KGRN, KMAG, KNRM, KNUL, KRED, KYEL};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    CharData, AFF_GROUP, ITEM_NOTE, ITEM_PEN, LVL_GOD, LVL_IMMORT, MAX_NOTE_LENGTH, NOBODY,
    PLR_NOSHOUT, PLR_WRITING, PRF_COLOR_1, PRF_COLOR_2, PRF_DEAF, PRF_NOAUCT, PRF_NOGOSS,
    PRF_NOGRATZ, PRF_NOREPEAT, PRF_NOTELL, PRF_QUEST, ROOM_SOUNDPROOF, WEAR_HOLD,
};
use crate::{
    _clrlevel, an, clr, Game, CCNRM, CCRED, COLOR_LEV, TO_CHAR, TO_NOTVICT, TO_ROOM,
    TO_SLEEP, TO_VICT,
};

pub fn do_say(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut argument = argument.trim_start().to_string();

    if argument.is_empty() {
        game.send_to_char(ch, "Yes, but WHAT do you want to say?\r\n");
    } else {
        let buf = format!("$n says, '{}'", argument);
        game.act(&buf, false, Some(ch), None, None, TO_ROOM);

        if !ch.is_npc() && ch.prf_flagged(PRF_NOREPEAT) {
            game.send_to_char(ch, OK);
        } else {
            delete_doubledollar(&mut argument);
            game.send_to_char(ch, format!("You say, '{}'\r\n", argument).as_str());
        }
    }
}

pub fn do_gsay(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let argument = argument.trim_start();

    if !ch.aff_flagged(AFF_GROUP) {
        game.send_to_char(ch, "But you are not the member of a group!\r\n");
        return;
    }
    if argument.is_empty() {
        game.send_to_char(ch, "Yes, but WHAT do you want to group-say?\r\n");
    } else {
        let k;
        if ch.master.borrow().is_some() {
            k = ch.master.borrow().as_ref().unwrap().clone();
        } else {
            k = ch.clone();
        }

        let buf = format!("$n tells the group, '{}'", argument);
        if k.aff_flagged(AFF_GROUP) && !Rc::ptr_eq(&k, ch) {
            game.act(&buf, false, Some(ch), None, Some(VictimRef::Char(&k)), TO_VICT | TO_SLEEP);
        }
        for f in k.followers.borrow().iter() {
            if f.follower.aff_flagged(AFF_GROUP) && !Rc::ptr_eq(&f.follower, ch) {
                game.act(
                    &buf,
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(&f.follower)),
                    TO_VICT | TO_SLEEP,
                );
            }
        }
        if ch.prf_flagged(PRF_NOREPEAT) {
            game.send_to_char(ch, OK);
        } else {
            game.send_to_char(
                ch,
                format!("You tell the group, '{}'\r\n", argument).as_str(),
            );
        }
    }
}

fn perform_tell(game: &mut Game, ch: &Rc<CharData>, vict: &Rc<CharData>, arg: &str) {
    game.send_to_char(vict, CCRED!(vict, C_NRM));
    let buf = format!("$n tells you, '{}'", arg);
    game.act(&buf, false, Some(ch), None, Some(VictimRef::Char(vict)), TO_VICT | TO_SLEEP);
    game.send_to_char(vict, CCNRM!(vict, C_NRM));

    if !ch.is_npc() && ch.prf_flagged(PRF_NOREPEAT) {
        game.send_to_char(ch, OK);
    } else {
        game.send_to_char(ch, CCRED!(ch, C_CMP));
        let buf = format!("You tell $N, '{}'", arg);
        game.act(&buf, false, Some(ch), None, Some(VictimRef::Char(vict)), TO_CHAR | TO_SLEEP);
        game.send_to_char(ch, CCNRM!(ch, C_CMP));
    }

    if !vict.is_npc() && !ch.is_npc() {
        vict.set_last_tell(ch.get_idnum());
    }
}

fn is_tell_ok(game: &mut Game, ch: &Rc<CharData>, vict: &Rc<CharData>) -> bool {
    if Rc::ptr_eq(ch, vict) {
        game.send_to_char(ch, "You try to tell yourself something.\r\n");
    } else if !ch.is_npc() && ch.prf_flagged(PRF_NOTELL) {
        game.send_to_char(
            ch,
            "You can't tell other people while you have notell on.\r\n",
        );
    } else if game.db.room_flagged(ch.in_room(), ROOM_SOUNDPROOF) {
        game.send_to_char(ch, "The walls seem to absorb your words.\r\n");
    } else if !vict.is_npc() && vict.desc.borrow().is_none() {
        /* linkless */
        game.act(
            "$E's linkless at the moment.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR | TO_SLEEP,
        );
    } else if vict.plr_flagged(PLR_WRITING) {
        game.act(
            "$E's writing a message right now; try again later.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR | TO_SLEEP,
        );
    } else if (!vict.is_npc() && vict.prf_flagged(PRF_NOTELL))
        || game.db.room_flagged(vict.in_room(), ROOM_SOUNDPROOF)
    {
        game.act(
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

    return false;
}

/*
 * Yes, do_tell probably could be combined with whisper and ask, but
 * called frequently, and should IMHO be kept as tight as possible.
 */
pub fn do_tell(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();
    half_chop(&mut argument, &mut buf, &mut buf2);
    let mut vict = None;
    if buf.is_empty() || buf2.is_empty() {
        game.send_to_char(ch, "Who do you wish to tell what??\r\n");
    } else if ch.get_level() < LVL_IMMORT as u8 && {
        vict = game.get_player_vis(ch, &mut buf, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        game.send_to_char(ch, NOPERSON);
    } else if ch.get_level() >= LVL_IMMORT as u8 && {
        vict = game.get_char_vis(ch, &mut buf, None, FIND_CHAR_WORLD);
        vict.is_none()
    } {
        game.send_to_char(ch, NOPERSON);
    } else if is_tell_ok(game, ch, vict.as_ref().unwrap()) {
        perform_tell(game, ch, vict.as_ref().unwrap(), &buf2);
    }
}

pub fn do_reply(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.is_npc() {
        return;
    }

    let argument = argument.trim_start();

    if ch.get_last_tell() == NOBODY as i64 {
        game.send_to_char(ch, "You have nobody to reply to!\r\n");
    } else if argument.is_empty() {
        game.send_to_char(ch, "What is your reply?\r\n");
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
        let tch = game.db
            .character_list
            .iter()
            .find(|c| !c.is_npc() && c.get_idnum() == ch.get_last_tell()).map(|e| e.clone());

        if tch.is_none() {
            game.send_to_char(ch, "They are no longer playing.\r\n");
        } else if is_tell_ok(game, ch, tch.as_ref().unwrap()) {
            perform_tell(game, ch, tch.as_ref().unwrap(), argument);
        }
    }
}

pub fn do_spec_comm(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
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
        game.send_to_char(
            ch,
            format!("Whom do you want to {}.. and what??\r\n", action_sing).as_str(),
        );
    } else if {
        vict = game.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        game.send_to_char(ch, NOPERSON);
    } else if Rc::ptr_eq(vict.as_ref().unwrap(), ch) {
        game.send_to_char(
            ch,
            "You can't get your mouth close enough to your ear...\r\n",
        );
    } else {
        let vict = vict.as_ref().unwrap();

        let buf1 = format!("$n {} you, '{}'", action_plur, buf2);
        game.act(&buf1, false, Some(ch), None, Some(VictimRef::Char(vict)), TO_VICT);

        if ch.prf_flagged(PRF_NOREPEAT) {
            game.send_to_char(ch, OK);
        } else {
            game.send_to_char(
                ch,
                format!("You {} {}, '{}'\r\n", action_sing, vict.get_name(), buf2).as_str(),
            );
        }
        game.act(action_others, false, Some(ch), None, Some(VictimRef::Char(vict)), TO_NOTVICT);
    }
}

pub fn do_write(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut paper;
    let mut pen = None;
    let mut papername = String::new();
    let mut penname = String::new();

    two_arguments(&argument, &mut papername, &mut penname);

    if ch.desc.borrow().is_none() {
        return;
    }

    if papername.is_empty() {
        /* nothing was delivered */
        game.send_to_char(
            ch,
            "Write?  With what?  ON what?  What are you trying to do?!?\r\n",
        );
        return;
    }
    if !penname.is_empty() {
        /* there were two arguments */
        if {
            paper = game.get_obj_in_list_vis(ch, &papername, None, &ch.carrying.borrow());
            paper.is_none()
        } {
            game.send_to_char(ch, format!("You have no {}.\r\n", papername).as_str());
            return;
        }
        if {
            pen = game.get_obj_in_list_vis(ch, &penname, None, &ch.carrying.borrow());
            pen.is_none()
        } {
            game.send_to_char(ch, format!("You have no {}.\r\n", penname).as_str());
            return;
        }
    } else {
        /* there was one arg.. let's see what we can find */
        if {
            paper = game.get_obj_in_list_vis(ch, &papername, None, &ch.carrying.borrow());
            paper.is_none()
        } {
            game.send_to_char(
                ch,
                format!("There is no {} in your inventory.\r\n", papername).as_str(),
            );
            return;
        }
        if game.db.obj(paper.unwrap()).get_obj_type() == ITEM_PEN {
            /* oops, a pen.. */
            pen = paper;
            paper = None;
        } else if game.db.obj(paper.unwrap()).get_obj_type() != ITEM_NOTE {
            game.send_to_char(ch, "That thing has nothing to do with writing.\r\n");
            return;
        }
        /* One object was found.. now for the other one. */
        if ch.get_eq(WEAR_HOLD as i8).is_none() {
            game.send_to_char(
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
        if !game.can_see_obj(ch, game.db.obj(ch.get_eq(WEAR_HOLD as i8).unwrap())) {
            game.send_to_char(ch, "The stuff in your hand is invisible!  Yeech!!\r\n");
            return;
        }
        if pen.is_some() {
            paper = ch.get_eq(WEAR_HOLD as i8);
        } else {
            pen = ch.get_eq(WEAR_HOLD as i8);
        }
    }
    let pen = pen.unwrap();
    let paper = paper.unwrap();

    /* ok.. now let's see what kind of stuff we've found */
    if game.db.obj(pen).get_obj_type() != ITEM_PEN {
        game.act(
            "$p is no good for writing with.",
            false,
            Some(ch),
            Some(pen),
            None,
            TO_CHAR,
        );
    } else if game.db.obj(paper).get_obj_type() != ITEM_NOTE {
        game.act(
            "You can't write on $p.",
            false,
            Some(ch),
            Some(paper),
            None,
            TO_CHAR,
        );
    } else if !game.db.obj(paper).action_description.borrow().is_empty() {
        game.send_to_char(ch, "There's something written on it already.\r\n");
    } else {
        /* we can write - hooray! */
        game.send_to_char(ch, "Write your note.  End with '@' on a new line.\r\n");
        game.act(
            "$n begins to jot down a note.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        string_write(
            game.descriptor_list.get_mut(ch.desc.borrow().unwrap()),
            game.db.obj(paper).action_description.clone(),
            MAX_NOTE_LENGTH as usize,
            0,
        );
    }
}

pub fn do_page(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();
    let mut buf2 = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut arg, &mut buf2);

    if ch.is_npc() {
        game.send_to_char(ch, "Monsters can't page.. go away.\r\n");
    } else if arg.is_empty() {
        game.send_to_char(ch, "Whom do you wish to page?\r\n");
    } else {
        let buf = format!("\007\007*$n* {}", buf2);
        if arg == "all" {
            if ch.get_level() > LVL_GOD as u8 {
                for d_id in game.descriptor_list.ids() {
                    if game.desc(d_id).state() == ConPlaying && game.desc(d_id).character.is_some() {
                        let vict_obj = game.desc(d_id).character.as_ref().unwrap().clone();
                        game.act(
                            &buf,
                            false,
                            Some(ch),
                            None,
                            Some(VictimRef::Char(&vict_obj)),
                            TO_VICT,
                        );
                    } else {
                        game.send_to_char(ch, "You will never be godly enough to do that!\r\n");
                    }
                }
                return;
            }
        }
        let vict;
        if {
            vict = game.get_char_vis(ch, &mut arg, None, FIND_CHAR_WORLD);
            vict.is_some()
        } {
            let vict = vict.as_ref().unwrap();

            game.act(&buf, false, Some(ch), None, Some(VictimRef::Char(vict)), TO_VICT);
            if ch.prf_flagged(PRF_NOREPEAT) {
                game.send_to_char(ch, OK);
            } else {
                game.act(&buf, false, Some(ch), None, Some(VictimRef::Char(vict)), TO_CHAR);
            }
        } else {
            game.send_to_char(ch, "There is no such person in the game!\r\n");
        }
    }
}

/**********************************************************************
 * generalized communication func, originally by Fred C. Merkel (Torg) *
 *********************************************************************/

pub fn do_gen_comm(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, subcmd: i32) {
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
    if ch.desc.borrow().is_none() {
        return;
    }

    if ch.plr_flagged(PLR_NOSHOUT) {
        game.send_to_char(ch, COM_MSGS[subcmd as usize][0]);
        return;
    }
    if game.db.room_flagged(ch.in_room(), ROOM_SOUNDPROOF) {
        game.send_to_char(ch, "The walls seem to absorb your words.\r\n");
        return;
    }
    /* level_can_shout defined in config.c */
    if ch.get_level() < LEVEL_CAN_SHOUT as u8 {
        game.send_to_char(
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
        game.send_to_char(ch, COM_MSGS[subcmd as usize][2]);
        return;
    }
    /* skip leading spaces */
    let argument = argument.trim_start();

    /* make sure that there is something there to say! */
    if argument.is_empty() {
        game.send_to_char(
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
            game.send_to_char(ch, "You're too exhausted to holler.\r\n");
            return;
        } else {
            ch.set_move(ch.get_move() - HOLLER_MOVE_COST as i16);
        }
    }
    /* set up the color on code */
    let color_on = COM_MSGS[subcmd as usize][3];

    /* first, set up strings to be given to the communicator */
    if ch.prf_flagged(PRF_NOREPEAT) {
        game.send_to_char(ch, OK);
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
        game.send_to_char(
            ch,
            
            messg.as_str(),
        );
    }

    let buf1 = format!("$n {}s, '{}'", COM_MSGS[subcmd as usize][1], argument);

    /* now send all the strings out */
    for d_id in game.descriptor_list.ids() {
        if game.desc(d_id).state() == ConPlaying
            && d_id == ch.desc.borrow().unwrap()
            && game.desc(d_id).character.is_some()
            && !game.desc(d_id)
                .character
                .as_ref()
                .unwrap()
                .prf_flagged(CHANNELS[subcmd as usize])
            && !game.desc(d_id)
                .character
                .as_ref()
                .unwrap()
                .plr_flagged(PLR_WRITING)
            && !game.db.room_flagged(
                game.desc(d_id).character.as_ref().unwrap().in_room(),
                ROOM_SOUNDPROOF,
            )
        {
            let ic = game.desc(d_id).character.as_ref().unwrap().clone();
            if subcmd == SCMD_SHOUT
                && (game.db.world[ch.in_room() as usize].zone != game.db.world[ic.in_room() as usize].zone
                    || !ic.awake())
            {
                continue;
            }

            if COLOR_LEV!(ic) >= C_NRM {
                game.send_to_char(ic.as_ref(), color_on);
            }
            game.act(&buf1, false, Some(ch), None, Some(VictimRef::Char(&ic)), TO_VICT | TO_SLEEP);
            if COLOR_LEV!(ic) >= C_NRM {
                game.send_to_char(ic.as_ref(), KNRM);
            }
        }
    }
}

pub fn do_qcomm(game: &mut Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    if ch.prf_flagged(PRF_QUEST) {
        game.send_to_char(ch, "You aren't even part of the quest!\r\n");
        return;
    }
    let argument = argument.trim_start();

    if argument.is_empty() {
        game.send_to_char(
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
            game.send_to_char(ch, OK);
        } else if subcmd == SCMD_QSAY {
            buf = format!("You quest-say, '{}'", argument);
            game.act(
                &buf,
                false,
                Some(ch),
                None,
                Some(VictimRef::Str(&argument.to_string())),
                TO_CHAR,
            );
        } else {
            game.act(
                argument,
                false,
                Some(ch),
                None,
                Some(VictimRef::Str(&argument.to_string())),
                TO_CHAR,
            );
        }

        if subcmd == SCMD_QSAY {
            buf = format!("$n quest-says, '{}'", argument);
        } else {
            buf = argument.to_string();
        }

        for id in game.descriptor_list.ids() {
            if game.descriptor_list.get(id).state() == ConPlaying
                && id == ch.desc.borrow().unwrap()
                && game.descriptor_list.get(id).character.is_some()
                && game.descriptor_list.get(id).character
                    .as_ref()
                    .unwrap()
                    .prf_flagged(PRF_QUEST)
            {
                let vict_obj = game.descriptor_list.get(id).character.as_ref().unwrap().clone();
                game.act(
                    &buf,
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(&vict_obj)),
                    TO_VICT | TO_SLEEP,
                );
            }
        }
    }
}
