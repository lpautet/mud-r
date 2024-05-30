/* ************************************************************************
*   File: act.other.rs                                  Part of CircleMUD *
*  Usage: Miscellaneous player-level commands                             *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use chrono::Utc;
use std::cmp::{max, min};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::rc::Rc;

use log::error;

use crate::act_wizard::perform_immort_vis;
use crate::alias::write_aliases;
use crate::config::{AUTO_SAVE, FREE_RENT, MAX_FILESIZE, NOPERSON, OK, PT_ALLOWED};
use crate::constants::DEX_APP_SKILL;
use crate::db::{BUG_FILE, DB, IDEA_FILE, TYPO_FILE};
use crate::fight::die;
use crate::handler::{affect_from_char, affect_to_char, isname, obj_from_char, FIND_CHAR_ROOM};
use crate::house::house_crashsave;
use crate::interpreter::{
    delete_doubledollar, half_chop, is_number, one_argument, two_arguments, CMD_INFO,
    SCMD_AUTOEXIT, SCMD_BRIEF, SCMD_BUG, SCMD_COMPACT, SCMD_DEAF, SCMD_HOLYLIGHT, SCMD_IDEA,
    SCMD_NOAUCTION, SCMD_NOGOSSIP, SCMD_NOGRATZ, SCMD_NOHASSLE, SCMD_NOREPEAT, SCMD_NOSUMMON,
    SCMD_NOTELL, SCMD_NOWIZ, SCMD_QUAFF, SCMD_QUEST, SCMD_QUIT, SCMD_RECITE, SCMD_ROOMFLAGS,
    SCMD_SLOWNS, SCMD_TRACK, SCMD_TYPO, SCMD_USE,
};
use crate::objsave::{crash_crashsave, crash_rentsave};
use crate::shops::shop_keeper;
use crate::spec_procs::list_skills;
use crate::spell_parser::mag_objectmagic;
use crate::spells::{SKILL_HIDE, SKILL_SNEAK, SKILL_STEAL, TYPE_UNDEFINED};
use crate::structs::{
    AffectedType, CharData, AFF_CHARM, AFF_GROUP, AFF_HIDE, AFF_INVISIBLE, AFF_SNEAK, APPLY_NONE,
    ITEM_POTION, ITEM_SCROLL, ITEM_STAFF, ITEM_WAND, LVL_IMMORT, MAX_TITLE_LENGTH, NUM_WEARS,
    PLR_LOADROOM, PLR_NOTITLE, POS_FIGHTING, POS_SLEEPING, POS_STUNNED, PRF_AUTOEXIT, PRF_BRIEF,
    PRF_COMPACT, PRF_DEAF, PRF_DISPAUTO, PRF_DISPHP, PRF_DISPMANA, PRF_DISPMOVE, PRF_HOLYLIGHT,
    PRF_NOAUCT, PRF_NOGOSS, PRF_NOGRATZ, PRF_NOHASSLE, PRF_NOREPEAT, PRF_NOTELL, PRF_NOWIZ,
    PRF_QUEST, PRF_ROOMFLAGS, PRF_SUMMONABLE, ROOM_HOUSE, ROOM_HOUSE_CRASH, ROOM_PEACEFUL,
    WEAR_HOLD,
};
use crate::util::{clone_vec, rand_number, CMP, NRM};
use crate::{an, send_to_char, Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

pub fn do_quit(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, subcmd: i32) {
    if ch.is_npc() || ch.desc.borrow().is_none() {
        return;
    }

    if subcmd != SCMD_QUIT && ch.get_level() < LVL_IMMORT as u8 {
        send_to_char(ch, "You have to type quit--no less, to quit!\r\n");
    } else if ch.get_pos() == POS_FIGHTING {
        send_to_char(ch, "No way!  You're fighting for your life!\r\n");
    } else if ch.get_pos() < POS_STUNNED {
        send_to_char(ch, "You die before your time...\r\n");
        die(ch, game);
    } else {
        game.db
            .act("$n has left the game.", true, Some(ch), None, None, TO_ROOM);
        game.mudlog(
            NRM,
            max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
            true,
            format!("{} has quit the game.", ch.get_name()).as_str(),
        );
        send_to_char(ch, "Goodbye, friend.. Come back soon!\r\n");

        /*  We used to check here for duping attempts, but we may as well
         *  do it right in extract_char(), since there is no check if a
         *  player rents out and it can leave them in an equally screwy
         *  situation.
         */

        if FREE_RENT {
            crash_rentsave(&mut game.db, ch, 0);
        }

        /* If someone is quitting in their house, let them load back here. */
        if !ch.plr_flagged(PLR_LOADROOM) && game.db.room_flagged(ch.in_room(), ROOM_HOUSE) {
            ch.set_loadroom(game.db.get_room_vnum(ch.in_room()));
        }

        game.db.extract_char(ch); /* Char is saved before extracting. */
    }
}

pub fn do_save(game: &mut Game, ch: &Rc<CharData>, _argument: &str, cmd: usize, _subcmd: i32) {
    if ch.is_npc() || ch.desc.borrow().is_none() {
        return;
    }

    /* Only tell the char we're saving if they actually typed "save" */
    if cmd != 0 {
        /*
         * This prevents item duplication by two PC's using coordinated saves
         * (or one PC with a house) and system crashes. Note that houses are
         * still automatically saved without this enabled. This code assumes
         * that guest immortals aren't trustworthy. If you've disabled guest
         * immortal advances from mortality, you may want < instead of <=.
         */
        if AUTO_SAVE && ch.get_level() <= LVL_IMMORT as u8 {
            send_to_char(ch, "Saving aliases.\r\n");
            write_aliases(ch);
            return;
        }
        send_to_char(
            ch,
            format!("Saving {} and aliases.\r\n", ch.get_name()).as_str(),
        );
    }

    write_aliases(ch);
    game.db.save_char(ch);
    crash_crashsave(&game.db, ch);

    if game.db.room_flagged(ch.in_room(), ROOM_HOUSE_CRASH) {
        house_crashsave(&game.db, game.db.get_room_vnum(ch.in_room()));
    }
}

/* generic function for commands which are normally overridden by
special procedures - i.e., shop commands, mail commands, etc. */
pub fn do_not_here(
    _game: &mut Game,
    ch: &Rc<CharData>,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    send_to_char(ch, "Sorry, but you cannot do that here!\r\n");
}

pub fn do_sneak(_game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.is_npc() || ch.get_skill(SKILL_SNEAK) == 0 {
        send_to_char(ch, "You have no idea how to do that.\r\n");
        return;
    }
    send_to_char(ch, "Okay, you'll try to move silently for a while.\r\n");
    if ch.aff_flagged(AFF_SNEAK) {
        affect_from_char(ch, SKILL_SNEAK as i16);
    }

    let percent = rand_number(1, 101); /* 101% is a complete failure */

    if percent
        > (ch.get_skill(SKILL_SNEAK) as i16 + DEX_APP_SKILL[ch.get_dex() as usize].sneak) as u32
    {
        return;
    }

    let af = AffectedType {
        _type: SKILL_SNEAK as i16,
        duration: ch.get_level() as i16,
        modifier: 0,
        location: APPLY_NONE as u8,
        bitvector: AFF_SNEAK,
    };

    affect_to_char(ch, &af);
}

pub fn do_hide(_game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.is_npc() || ch.get_skill(SKILL_HIDE) == 0 {
        send_to_char(ch, "You have no idea how to do that.\r\n");
        return;
    }

    send_to_char(ch, "You attempt to hide yourself.\r\n");

    if ch.aff_flagged(AFF_HIDE) {
        ch.remove_aff_flags(AFF_HIDE);
    }

    let percent = rand_number(1, 101); /* 101% is a complete failure */

    if percent
        > (ch.get_skill(SKILL_HIDE) as i16 + DEX_APP_SKILL[ch.get_dex() as usize].hide) as u32
    {
        return;
    }
    ch.set_aff_flags_bits(AFF_HIDE);
}

pub fn do_steal(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    if ch.is_npc() || ch.get_skill(SKILL_STEAL) == 0 {
        send_to_char(ch, "You have no idea how to do that.\r\n");
        return;
    }
    if db.room_flagged(ch.in_room(), ROOM_PEACEFUL) {
        send_to_char(
            ch,
            "This room just has such a peaceful, easy feeling...\r\n",
        );
        return;
    }
    let mut obj_name = String::new();
    let mut vict_name = String::new();
    two_arguments(argument, &mut obj_name, &mut vict_name);
    let vict;
    if {
        vict = db.get_char_vis(ch, &mut vict_name, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(ch, "Steal what from who?\r\n");
        return;
    } else if Rc::ptr_eq(vict.as_ref().unwrap(), ch) {
        send_to_char(ch, "Come on now, that's rather stupid!\r\n");
        return;
    }
    let mut ohoh = false;

    let vict = vict.as_ref().unwrap();
    /* 101% is a complete failure */
    let mut percent =
        rand_number(1, 101) as i32 - DEX_APP_SKILL[ch.get_dex() as usize].p_pocket as i32;

    if vict.get_pos() < POS_SLEEPING {
        percent = -1; /* ALWAYS SUCCESS, unless heavy object. */
    }

    let mut pcsteal = false;
    if !PT_ALLOWED && !vict.is_npc() {
        pcsteal = true;
    }

    if !vict.awake() {
        /* Easier to steal from sleeping people. */
        percent -= 50;
    }

    /* NO NO With Imp's and Shopkeepers, and if player thieving is not allowed */
    if vict.get_level() >= LVL_IMMORT as u8
        || pcsteal
        || (db.get_mob_spec(vict).is_some()
            && db.get_mob_spec(vict).unwrap() as usize == shop_keeper as usize)
    {
        percent = 101; /* Failure */
    }
    let mut obj;
    let mut the_eq_pos = -1;
    if obj_name != "coins" && obj_name != "gold" {
        if {
            obj = db.get_obj_in_list_vis(ch, &mut obj_name, None, vict.carrying.borrow());
            obj.is_none()
        } {
            for eq_pos in 0..NUM_WEARS {
                if vict.get_eq(eq_pos).is_some()
                    && isname(
                        &obj_name,
                        vict.get_eq(eq_pos).as_ref().unwrap().name.borrow().as_ref(),
                    )
                    && db.can_see_obj(ch, vict.get_eq(eq_pos).as_ref().unwrap())
                {
                    obj = vict.get_eq(eq_pos);
                    the_eq_pos = eq_pos;
                }
            }
            if obj.is_none() {
                db.act(
                    "$E hasn't got that item.",
                    false,
                    Some(ch),
                    None,
                    Some(vict),
                    TO_CHAR,
                );
                return;
            } else {
                /* It is equipment */
                if vict.get_pos() > POS_STUNNED {
                    send_to_char(ch, "Steal the equipment now?  Impossible!\r\n");
                    return;
                } else {
                    let obj = obj.as_ref().unwrap();
                    db.act(
                        "You unequip $p and steal it.",
                        false,
                        Some(ch),
                        Some(obj),
                        None,
                        TO_CHAR,
                    );
                    db.act(
                        "$n steals $p from $N.",
                        false,
                        Some(ch),
                        Some(obj),
                        Some(vict),
                        TO_NOTVICT,
                    );
                    DB::obj_to_char(db.unequip_char(vict, the_eq_pos).as_ref().unwrap(), ch);
                }
            }
        } else {
            /* obj found in inventory */
            let obj = obj.as_ref().unwrap();

            percent += obj.get_obj_weight(); /* Make heavy harder */
            if percent > ch.get_skill(SKILL_STEAL) as u32 as i32 {
                ohoh = true;
                send_to_char(ch, "Oops..\r\n");
                db.act(
                    "$n tried to steal something from you!",
                    false,
                    Some(ch),
                    None,
                    Some(vict),
                    TO_VICT,
                );
                db.act(
                    "$n tries to steal something from $N.",
                    true,
                    Some(ch),
                    None,
                    Some(vict),
                    TO_NOTVICT,
                );
            } else {
                /* Steal the item */
                if ch.is_carrying_n() + 1 < ch.can_carry_n() as u8 {
                    if ch.is_carrying_w() + obj.get_obj_weight() < ch.can_carry_w() as i32 {
                        obj_from_char(obj);
                        DB::obj_to_char(obj, ch);
                        send_to_char(ch, "Got it!\r\n");
                    }
                } else {
                    send_to_char(ch, "You cannot carry that much.\r\n");
                }
            }
        }
    } else {
        /* Steal some coins */
        if vict.awake() && percent > ch.get_skill(SKILL_STEAL) as u32 as i32 {
            ohoh = true;
            send_to_char(ch, "Oops..\r\n");
            db.act(
                "You discover that $n has $s hands in your wallet.",
                false,
                Some(ch),
                None,
                Some(vict),
                TO_VICT,
            );
            db.act(
                "$n tries to steal gold from $N.",
                true,
                Some(ch),
                None,
                Some(vict),
                TO_NOTVICT,
            );
        } else {
            /* Steal some gold coins */
            let mut gold = vict.get_gold() * rand_number(1, 10) as i32 / 100;
            gold = min(1782, gold);
            if gold > 0 {
                ch.set_gold(ch.get_gold() + gold);
                vict.set_gold(vict.get_gold() - gold);

                if gold > 1 {
                    send_to_char(
                        ch,
                        format!("Bingo!  You got {} gold coins.\r\n", gold).as_str(),
                    );
                } else {
                    send_to_char(ch, "You manage to swipe a solitary gold coin.\r\n");
                }
            } else {
                send_to_char(ch, "You couldn't get any gold...\r\n");
            }
        }
    }

    if ohoh && vict.is_npc() && vict.awake() {
        game.hit(vict, ch, TYPE_UNDEFINED);
    }
}

pub fn do_practice(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.is_npc() {
        return;
    }
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if !arg.is_empty() {
        send_to_char(ch, "You can only practice skills in your guild.\r\n");
    } else {
        list_skills(&game.db, ch);
    }
}

pub fn do_visible(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.get_level() >= LVL_IMMORT as u8 {
        perform_immort_vis(&game.db, ch);
        return;
    }

    if ch.aff_flagged(AFF_INVISIBLE) {
        game.db.appear(ch);
        send_to_char(ch, "You break the spell of invisibility.\r\n");
    } else {
        send_to_char(ch, "You are already visible.\r\n");
    }
}

pub fn do_title(_game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if ch.is_npc() {
        send_to_char(ch, "Your title is fine... go away.\r\n");
    } else if ch.plr_flagged(PLR_NOTITLE) {
        send_to_char(
            ch,
            "You can't title yourself -- you shouldn't have abused it!\r\n",
        );
    } else if argument.contains('(') || argument.contains('(') {
        send_to_char(ch, "Titles can't contain the ( or ) characters.\r\n");
    } else if argument.len() > MAX_TITLE_LENGTH {
        send_to_char(
            ch,
            format!(
                "Sorry, titles can't be longer than {} characters.\r\n",
                MAX_TITLE_LENGTH
            )
            .as_str(),
        );
    } else {
        ch.set_title(Some(argument));

        send_to_char(
            ch,
            format!("Okay, you're now {} {}.\r\n", ch.get_name(), ch.get_title()).as_str(),
        );
    }
}

fn perform_group(db: &DB, ch: &Rc<CharData>, vict: &Rc<CharData>) -> i32 {
    if vict.aff_flagged(AFF_GROUP) || !db.can_see(ch, vict) {
        return 0;
    }

    vict.set_aff_flags_bits(AFF_GROUP);

    if !Rc::ptr_eq(ch, vict) {
        db.act(
            "$N is now a member of your group.",
            false,
            Some(ch),
            None,
            Some(vict),
            TO_CHAR,
        );
    }
    db.act(
        "You are now a member of $n's group.",
        false,
        Some(ch),
        None,
        Some(vict),
        TO_VICT,
    );
    db.act(
        "$N is now a member of $n's group.",
        false,
        Some(ch),
        None,
        Some(vict),
        TO_NOTVICT,
    );
    return 1;
}

fn print_group(db: &DB, ch: &Rc<CharData>) {
    if !ch.aff_flagged(AFF_GROUP) {
        send_to_char(ch, "But you are not the member of a group!\r\n");
    } else {
        send_to_char(ch, "Your group consists of:\r\n");

        let k = if ch.master.borrow().is_some() {
            ch.master.borrow().as_ref().unwrap().clone()
        } else {
            ch.clone()
        };

        if k.aff_flagged(AFF_GROUP) {
            let buf = format!(
                "     [{:3}H {:3}M {:3}V] [{:2} {}] $N (Head of group)",
                k.get_hit(),
                k.get_mana(),
                k.get_move(),
                k.get_level(),
                k.class_abbr()
            );
            db.act(&buf, false, Some(ch), None, Some(&k), TO_CHAR);
        }

        for f in k.followers.borrow().iter() {
            if !f.follower.aff_flagged(AFF_GROUP) {
                continue;
            }

            let buf = format!(
                "     [{:3}H {:3}M {:3}V] [{:2} {}] $N",
                f.follower.get_hit(),
                f.follower.get_mana(),
                f.follower.get_move(),
                f.follower.get_level(),
                f.follower.class_abbr()
            );
            db.act(
                &buf,
                false,
                Some(ch),
                None,
                Some(f.follower.as_ref()),
                TO_CHAR,
            );
        }
    }
}

pub fn do_group(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let db = &game.db;

    one_argument(argument, &mut buf);

    if buf.is_empty() {
        print_group(db, ch);
        return;
    }

    if ch.master.borrow().is_some() {
        db.act(
            "You can not enroll group members without being head of a group.",
            false,
            Some(ch),
            None,
            None,
            TO_CHAR,
        );
        return;
    }

    if buf == "all" {
        perform_group(db, ch, ch);
        let mut found = 0;
        for f in ch.followers.borrow().iter() {
            found += perform_group(db, ch, &f.follower);
        }
        if found == 0 {
            send_to_char(ch, "Everyone following you is already in your group.\r\n");
        }
        return;
    }
    let vict;

    if {
        vict = db.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(ch, NOPERSON);
    } else if (vict.as_ref().unwrap().master.borrow().is_none()
        || !Rc::ptr_eq(vict.as_ref().unwrap().master.borrow().as_ref().unwrap(), ch))
        && !Rc::ptr_eq(vict.as_ref().unwrap(), ch)
    {
        db.act(
            "$N must follow you to enter your group.",
            false,
            Some(ch),
            None,
            Some(vict.as_ref().unwrap()),
            TO_CHAR,
        );
    } else {
        let vict = vict.as_ref().unwrap();

        if !vict.aff_flagged(AFF_GROUP) {
            perform_group(db, ch, vict);
        } else {
            if !Rc::ptr_eq(ch, vict) {
                db.act(
                    "$N is no longer a member of your group.",
                    false,
                    Some(ch),
                    None,
                    Some(vict),
                    TO_CHAR,
                );
            }
            db.act(
                "You have been kicked out of $n's group!",
                false,
                Some(ch),
                None,
                Some(vict),
                TO_VICT,
            );
            db.act(
                "$N has been kicked out of $n's group!",
                false,
                Some(ch),
                None,
                Some(vict),
                TO_NOTVICT,
            );
            vict.remove_prf_flags_bits(AFF_GROUP);
        }
    }
}

pub fn do_ungroup(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut buf = String::new();
    let db = &game.db;
    one_argument(argument, &mut buf);

    if buf.is_empty() {
        if ch.master.borrow().is_some() || !ch.aff_flagged(AFF_GROUP) {
            send_to_char(ch, "But you lead no group!\r\n");
            return;
        }

        for f in clone_vec(&ch.followers).iter() {
            if f.follower.aff_flagged(AFF_GROUP) {
                f.follower.remove_aff_flags(AFF_GROUP);

                db.act(
                    "$N has disbanded the group.",
                    true,
                    Some(&f.follower),
                    None,
                    Some(ch),
                    TO_CHAR,
                );
                if !f.follower.aff_flagged(AFF_CHARM) {
                    db.stop_follower(&f.follower);
                }
            }
        }
        ch.remove_aff_flags(AFF_GROUP);

        send_to_char(ch, "You disband the group.\r\n");
        return;
    }
    let tch;
    if {
        tch = db.get_char_vis(ch, &mut buf, None, FIND_CHAR_ROOM);
        tch.is_none()
    } {
        send_to_char(ch, "There is no such person!\r\n");
        return;
    }
    let tch = tch.as_ref().unwrap();
    if tch.master.borrow().is_none() || !Rc::ptr_eq(tch.master.borrow().as_ref().unwrap(), ch) {
        send_to_char(ch, "That person is not following you!\r\n");
        return;
    }

    if !tch.aff_flagged(AFF_GROUP) {
        send_to_char(ch, "That person isn't in your group.\r\n");
        return;
    }

    tch.remove_aff_flags(AFF_GROUP);

    db.act(
        "$N is no longer a member of your group.",
        false,
        Some(ch),
        None,
        Some(tch),
        TO_CHAR,
    );
    db.act(
        "You have been kicked out of $n's group!",
        false,
        Some(ch),
        None,
        Some(tch),
        TO_VICT,
    );
    db.act(
        "$N has been kicked out of $n's group!",
        false,
        Some(ch),
        None,
        Some(tch),
        TO_NOTVICT,
    );

    if !tch.aff_flagged(AFF_CHARM) {
        db.stop_follower(tch);
    }
}

pub fn do_report(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, _subcmd: i32) {
    let db = &game.db;
    if !ch.aff_flagged(AFF_GROUP) {
        send_to_char(ch, "But you are not a member of any group!\r\n");
        return;
    }

    let buf = format!(
        "$n reports: {}/{}H, {}/{}M, {}/{}V\r\n",
        ch.get_hit(),
        ch.get_max_hit(),
        ch.get_mana(),
        ch.get_max_mana(),
        ch.get_move(),
        ch.get_max_move()
    );

    let k = if ch.master.borrow().is_some() {
        ch.master.borrow().as_ref().unwrap().clone()
    } else {
        ch.clone()
    };

    for f in k.followers.borrow().iter() {
        if f.follower.aff_flagged(AFF_GROUP) && !Rc::ptr_eq(&f.follower, ch) {
            db.act(&buf, true, Some(ch), None, Some(&f.follower), TO_VICT);
        }
    }
    if !Rc::ptr_eq(&k, ch) {
        db.act(&buf, true, Some(ch), None, Some(&k), TO_VICT);
    }

    send_to_char(ch, "You report to the group.\r\n");
}

pub fn do_split(_game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.is_npc() {
        return;
    }
    let mut buf = String::new();
    one_argument(argument, &mut buf);
    let amount;
    if is_number(&buf) {
        amount = buf.parse::<i32>().unwrap();
        if amount <= 0 {
            send_to_char(ch, "Sorry, you can't do that.\r\n");
            return;
        }
        if amount > ch.get_gold() {
            send_to_char(ch, "You don't seem to have that much gold to split.\r\n");
            return;
        }
        let k = if ch.master.borrow().is_some() {
            ch.master.borrow().as_ref().unwrap().clone()
        } else {
            ch.clone()
        };

        let mut num;
        if k.aff_flagged(AFF_GROUP) && k.in_room() == ch.in_room() {
            num = 1;
        } else {
            num = 0;
        }

        for f in k.followers.borrow().iter() {
            if f.follower.aff_flagged(AFF_GROUP)
                && !f.follower.is_npc()
                && f.follower.in_room() == ch.in_room()
            {
                num += 1;
            }
        }
        let share;
        let rest;
        if num != 0 && ch.aff_flagged(AFF_GROUP) {
            share = amount / num;
            rest = amount % num;
        } else {
            send_to_char(ch, "With whom do you wish to share your gold?\r\n");
            return;
        }

        ch.set_gold(ch.get_gold() - share * (num - 1));

        /* Abusing signed/unsigned to make sizeof work. */
        let mut buf = format!(
            "{} splits {} coins; you receive {}.\r\n",
            ch.get_name(),
            amount,
            share
        );
        if rest != 0 {
            buf.push_str(
                format!(
                    "{} coin{} {} not splitable, so {} keeps the money.\r\n",
                    rest,
                    if rest == 1 { "" } else { "s" },
                    if rest == 1 { "was" } else { "were" },
                    ch.get_name()
                )
                .as_str(),
            );
        }
        if k.aff_flagged(AFF_GROUP)
            && k.in_room() == ch.in_room()
            && !k.is_npc()
            && !Rc::ptr_eq(&k, ch)
        {
            k.set_gold(k.get_gold() + share);
            send_to_char(&k, &buf);
        }

        for f in k.followers.borrow().iter() {
            if f.follower.aff_flagged(AFF_GROUP)
                && !f.follower.is_npc()
                && f.follower.in_room() == ch.in_room()
                && !Rc::ptr_eq(&f.follower, ch)
            {
                f.follower.set_gold(f.follower.get_gold() + share);

                send_to_char(&f.follower, &buf);
            }
        }
        send_to_char(
            ch,
            format!(
                "You split {} coins among {} members -- {} coins each.\r\n",
                amount, num, share
            )
            .as_str(),
        );

        if rest != 0 {
            send_to_char(
                ch,
                format!(
                    "{} coin{} {} not splitable, so you keep the money.\r\n",
                    rest,
                    if rest == 1 { "" } else { "s" },
                    if rest == 1 { "was" } else { "were" }
                )
                .as_str(),
            );
            ch.set_gold(ch.get_gold() + rest);
        }
    } else {
        send_to_char(
            ch,
            "How many coins do you wish to split with your group?\r\n",
        );
        return;
    }
}

pub fn do_use(game: &mut Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let db = &game.db;
    let mut buf = String::new();
    let mut arg = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut arg, &mut buf);
    if arg.is_empty() {
        send_to_char(
            ch,
            format!("What do you want to {}?\r\n", CMD_INFO[cmd].command).as_str(),
        );
        return;
    }
    let mut mag_item = ch.get_eq(WEAR_HOLD as i8);

    if mag_item.is_none() || !isname(&arg, &mag_item.as_ref().unwrap().name.borrow()) {
        match subcmd {
            SCMD_RECITE | SCMD_QUAFF => {
                if {
                    mag_item = db.get_obj_in_list_vis(ch, &arg, None, ch.carrying.borrow());
                    mag_item.is_none()
                } {
                    send_to_char(
                        ch,
                        format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
                    );
                    return;
                }
            }
            SCMD_USE => {
                send_to_char(
                    ch,
                    format!("You don't seem to be holding {} {}.\r\n", an!(arg), arg).as_str(),
                );
                return;
            }
            _ => {
                error!("SYSERR: Unknown subcmd {} passed to do_use.", subcmd);
                return;
            }
        }
    }
    let mag_item = mag_item.as_ref().unwrap();
    match subcmd {
        SCMD_QUAFF => {
            if mag_item.get_obj_type() != ITEM_POTION {
                send_to_char(ch, "You can only quaff potions.\r\n");
                return;
            }
        }
        SCMD_RECITE => {
            if mag_item.get_obj_type() != ITEM_SCROLL {
                send_to_char(ch, "You can only recite scrolls.\r\n");
                return;
            }
        }
        SCMD_USE => {
            if mag_item.get_obj_type() != ITEM_WAND && mag_item.get_obj_type() != ITEM_STAFF {
                send_to_char(ch, "You can't seem to figure out how to use it.\r\n");
                return;
            }
        }
        _ => {}
    }

    mag_objectmagic(game, ch, mag_item, &buf);
}

pub fn do_wimpy(_game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();

    /* 'wimp_level' is a player_special. -gg 2/25/98 */
    if ch.is_npc() {
        return;
    }

    one_argument(argument, &mut arg);

    if arg.is_empty() {
        if ch.get_wimp_lev() != 0 {
            send_to_char(
                ch,
                format!(
                    "Your current wimp level is {} hit points.\r\n",
                    ch.get_wimp_lev()
                )
                .as_str(),
            );
            return;
        } else {
            send_to_char(ch, "At the moment, you're not a wimp.  (sure, sure...)\r\n");
            return;
        }
    }
    let wimp_lev;
    if arg.chars().next().unwrap().is_digit(10) {
        if {
            wimp_lev = arg.parse::<i32>().unwrap();
            wimp_lev != 0
        } {
            if wimp_lev < 0 {
                send_to_char(ch, "Heh, heh, heh.. we are jolly funny today, eh?\r\n");
            } else if wimp_lev > ch.get_max_hit() as i32 {
                send_to_char(ch, "That doesn't make much sense, now does it?\r\n");
            } else if wimp_lev > (ch.get_max_hit() / 2) as i32 {
                send_to_char(
                    ch,
                    "You can't set your wimp level above half your hit points.\r\n",
                );
            } else {
                send_to_char(
                    ch,
                    format!(
                        "Okay, you'll wimp out if you drop below {} hit points.\r\n",
                        wimp_lev
                    )
                    .as_str(),
                );
                ch.set_wimp_lev(wimp_lev);
            }
        } else {
            send_to_char(
                ch,
                "Okay, you'll now tough out fights to the bitter end.\r\n",
            );
            ch.set_wimp_lev(0);
        }
    } else {
        send_to_char(
            ch,
            "Specify at how many hit points you want to wimp out at.  (0 to disable)\r\n",
        );
    }
}

pub fn do_display(_game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    if ch.is_npc() {
        send_to_char(ch, "Monsters don't need displays.  Go away.\r\n");
        return;
    }
    let argument = argument.trim_start();

    if argument.len() == 0 {
        send_to_char(
            ch,
            "Usage: prompt { { H | M | V } | all | auto | none }\r\n",
        );
        return;
    }

    if argument == "auto" {
        ch.toggle_prf_flag_bits(PRF_DISPAUTO);
        send_to_char(
            ch,
            format!(
                "Auto prompt {}abled.\r\n",
                if ch.prf_flagged(PRF_DISPAUTO) {
                    "en"
                } else {
                    "dis"
                }
            )
            .as_str(),
        );
        return;
    }

    if argument == "on" || argument == "all" {
        ch.set_prf_flags_bits(PRF_DISPHP | PRF_DISPMANA | PRF_DISPMOVE);
    } else if argument == "off" || argument == "none" {
        ch.remove_prf_flags_bits(PRF_DISPHP | PRF_DISPMANA | PRF_DISPMOVE);
    } else {
        ch.remove_prf_flags_bits(PRF_DISPHP | PRF_DISPMANA | PRF_DISPMOVE);

        for c in argument.chars() {
            match c.to_ascii_lowercase() {
                'h' => {
                    ch.set_prf_flags_bits(PRF_DISPHP);
                }
                'm' => {
                    ch.set_prf_flags_bits(PRF_DISPMANA);
                }
                'v' => {
                    ch.set_prf_flags_bits(PRF_DISPMOVE);
                }
                _ => {
                    send_to_char(
                        ch,
                        "Usage: prompt { { H | M | V } | all | auto | none }\r\n",
                    );
                    return;
                }
            }
        }
    }

    send_to_char(ch, OK);
}

pub fn do_gen_write(game: &mut Game, ch: &Rc<CharData>, argument: &str, cmd: usize, subcmd: i32) {
    let db = &game.db;
    let filename;
    match subcmd {
        SCMD_BUG => {
            filename = BUG_FILE;
        }
        SCMD_TYPO => {
            filename = TYPO_FILE;
        }
        SCMD_IDEA => {
            filename = IDEA_FILE;
        }
        _ => {
            return;
        }
    }

    let dt = Utc::now();

    if ch.is_npc() {
        send_to_char(ch, "Monsters can't have ideas - Go away.\r\n");
        return;
    }

    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if argument.is_empty() {
        send_to_char(ch, "That must be a mistake...\r\n");
        return;
    }
    game.mudlog(
        CMP,
        LVL_IMMORT as i32,
        false,
        format!(
            "{} {}: {}",
            ch.get_name(),
            CMD_INFO[cmd as usize].command,
            argument
        )
        .as_str(),
    );

    let r = fs::metadata(filename);
    if r.is_err() {
        error!(
            "SYSERR: Can't get file metadata ({}): {}",
            filename,
            r.err().unwrap()
        );
        return;
    }
    let fm = r.unwrap();

    if fm.len() >= MAX_FILESIZE as u64 {
        send_to_char(
            ch,
            "Sorry, the file is full right now.. try again later.\r\n",
        );
        return;
    }
    let fl = OpenOptions::new().write(true).append(true).open(filename);
    if fl.is_err() {
        error!(
            "SYSERR: do_gen_write, opening {} {}",
            filename,
            fl.err().unwrap()
        );
        send_to_char(ch, "Could not open the file.  Sorry.\r\n");
        return;
    }

    let buf = format!(
        "{:8} ({:6}) [{:5}] {}\n",
        ch.get_name(),
        dt,
        db.get_room_vnum(ch.in_room()),
        argument
    );
    let r = fl.unwrap().write_all(buf.as_ref());
    if r.is_err() {
        error!(
            "SYSERR: do_gen_write, writing {} {}",
            filename,
            r.err().unwrap()
        );
        send_to_char(ch, "Could not write to the file.  Sorry.\r\n");
        return;
    }

    send_to_char(ch, "Okay.  Thanks!\r\n");
}

const TOG_ON: usize = 1;
const TOG_OFF: usize = 0;

macro_rules! prf_tog_chk {
    ($ch:expr, $flag:expr) => {
        ($ch.toggle_prf_flag_bits($flag) & $flag) != 0
    };
}

pub fn do_gen_tog(game: &mut Game, ch: &Rc<CharData>, _argument: &str, _cmd: usize, subcmd: i32) {
    const TOG_MESSAGES: [[&str; 2]; 17] = [
        [
            "You are now safe from summoning by other players.\r\n",
            "You may now be summoned by other players.\r\n",
        ],
        ["Nohassle disabled.\r\n", "Nohassle enabled.\r\n"],
        ["Brief mode off.\r\n", "Brief mode on.\r\n"],
        ["Compact mode off.\r\n", "Compact mode on.\r\n"],
        [
            "You can now hear tells.\r\n",
            "You are now deaf to tells.\r\n",
        ],
        [
            "You can now hear auctions.\r\n",
            "You are now deaf to auctions.\r\n",
        ],
        [
            "You can now hear shouts.\r\n",
            "You are now deaf to shouts.\r\n",
        ],
        [
            "You can now hear gossip.\r\n",
            "You are now deaf to gossip.\r\n",
        ],
        [
            "You can now hear the congratulation messages.\r\n",
            "You are now deaf to the congratulation messages.\r\n",
        ],
        [
            "You can now hear the Wiz-channel.\r\n",
            "You are now deaf to the Wiz-channel.\r\n",
        ],
        [
            "You are no longer part of the Quest.\r\n",
            "Okay, you are part of the Quest!\r\n",
        ],
        [
            "You will no longer see the room flags.\r\n",
            "You will now see the room flags.\r\n",
        ],
        [
            "You will now have your communication repeated.\r\n",
            "You will no longer have your communication repeated.\r\n",
        ],
        ["HolyLight mode off.\r\n", "HolyLight mode on.\r\n"],
        [
            "Nameserver_is_slow changed to NO; IP addresses will now be resolved.\r\n",
            "Nameserver_is_slow changed to YES; sitenames will no longer be resolved.\r\n",
        ],
        ["Autoexits disabled.\r\n", "Autoexits enabled.\r\n"],
        [
            "Will no longer track through doors.\r\n",
            "Will now track through doors.\r\n",
        ],
    ];

    if ch.is_npc() {
        return;
    }
    let result;
    match subcmd {
        SCMD_NOSUMMON => {
            result = prf_tog_chk!(ch, PRF_SUMMONABLE);
        }
        SCMD_NOHASSLE => {
            result = prf_tog_chk!(ch, PRF_NOHASSLE);
        }
        SCMD_BRIEF => {
            result = prf_tog_chk!(ch, PRF_BRIEF);
        }
        SCMD_COMPACT => {
            result = prf_tog_chk!(ch, PRF_COMPACT);
        }
        SCMD_NOTELL => {
            result = prf_tog_chk!(ch, PRF_NOTELL);
        }
        SCMD_NOAUCTION => {
            result = prf_tog_chk!(ch, PRF_NOAUCT);
        }
        SCMD_DEAF => {
            result = prf_tog_chk!(ch, PRF_DEAF);
        }
        SCMD_NOGOSSIP => {
            result = prf_tog_chk!(ch, PRF_NOGOSS);
        }
        SCMD_NOGRATZ => {
            result = prf_tog_chk!(ch, PRF_NOGRATZ);
        }
        SCMD_NOWIZ => {
            result = prf_tog_chk!(ch, PRF_NOWIZ);
        }
        SCMD_QUEST => {
            result = prf_tog_chk!(ch, PRF_QUEST);
        }
        SCMD_ROOMFLAGS => {
            result = prf_tog_chk!(ch, PRF_ROOMFLAGS);
        }
        SCMD_NOREPEAT => {
            result = prf_tog_chk!(ch, PRF_NOREPEAT);
        }
        SCMD_HOLYLIGHT => {
            result = prf_tog_chk!(ch, PRF_HOLYLIGHT);
        }
        SCMD_SLOWNS => {
            result = {
                game.config
                    .nameserver_is_slow
                    =!game.config.nameserver_is_slow;
                game.config.nameserver_is_slow
            }
        }
        SCMD_AUTOEXIT => {
            result = prf_tog_chk!(ch, PRF_AUTOEXIT);
        }
        SCMD_TRACK => {
            result = {
                game.config
                    .track_through_doors
                    =!game.config.track_through_doors;
                game.config.track_through_doors
            }
        }
        _ => {
            error!("SYSERR: Unknown subcmd {} in do_gen_toggle.", subcmd);
            return;
        }
    }

    if result {
        send_to_char(ch, TOG_MESSAGES[subcmd as usize][TOG_ON]);
    } else {
        send_to_char(ch, TOG_MESSAGES[subcmd as usize][TOG_OFF]);
    }

    return;
}
