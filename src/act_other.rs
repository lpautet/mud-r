/* ************************************************************************
*   File: act.other.rs                                  Part of CircleMUD *
*  Usage: Miscellaneous player-level commands                             *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use chrono::Utc;
use std::cmp::{max, min};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;

use crate::depot::{Depot, DepotId, HasId};
use crate::{act, save_char, send_to_char, CharData, DescriptorData, ObjData, TextData, VictimRef, DB};
use log::error;

use crate::act_wizard::perform_immort_vis;
use crate::alias::write_aliases;
use crate::config::{AUTO_SAVE, FREE_RENT, MAX_FILESIZE, NOPERSON, OK, PT_ALLOWED};
use crate::constants::DEX_APP_SKILL;
use crate::db::{BUG_FILE, IDEA_FILE, TYPO_FILE};
use crate::fight::{appear, die};
use crate::handler::{affect_from_char, affect_to_char, get_char_vis, get_obj_in_list_vis, isname, obj_from_char, obj_to_char, FIND_CHAR_ROOM};
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
    AffectFlags, AffectedType, RoomFlags, APPLY_NONE, ITEM_POTION, ITEM_SCROLL, ITEM_STAFF, ITEM_WAND, LVL_IMMORT, MAX_TITLE_LENGTH, NUM_WEARS, PLR_LOADROOM, PLR_NOTITLE, POS_FIGHTING, POS_SLEEPING, POS_STUNNED, PRF_AUTOEXIT, PRF_BRIEF, PRF_COMPACT, PRF_DEAF, PRF_DISPAUTO, PRF_DISPHP, PRF_DISPMANA, PRF_DISPMOVE, PRF_HOLYLIGHT, PRF_NOAUCT, PRF_NOGOSS, PRF_NOGRATZ, PRF_NOHASSLE, PRF_NOREPEAT, PRF_NOTELL, PRF_NOWIZ, PRF_QUEST, PRF_ROOMFLAGS, PRF_SUMMONABLE, WEAR_HOLD
};
use crate::util::{can_see, can_see_obj, rand_number, stop_follower, CMP, NRM};
use crate::{an, Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

pub fn do_quit(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() || ch.desc.is_none() {
        return;
    }

    if subcmd != SCMD_QUIT && ch.get_level() < LVL_IMMORT as u8 {
        send_to_char(&mut game.descriptors, ch, "You have to type quit--no less, to quit!\r\n");
    } else if ch.get_pos() == POS_FIGHTING {
        send_to_char(&mut game.descriptors, ch, "No way!  You're fighting for your life!\r\n");
    } else if ch.get_pos() < POS_STUNNED {
        send_to_char(&mut game.descriptors, ch, "You die before your time...\r\n");
        die(chid, game,chars, db, texts,objs);
    } else {
        act(&mut game.descriptors, chars, 
            db,
            "$n has left the game.",
            true,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let ch = chars.get(chid);
        game.mudlog(chars,
            NRM,
            max(LVL_IMMORT as i32, ch.get_invis_lev() as i32),
            true,
            format!("{} has quit the game.", ch.get_name()).as_str(),
        );
        send_to_char(&mut game.descriptors, ch, "Goodbye, friend.. Come back soon!\r\n");

        /*  We used to check here for duping attempts, but we may as well
         *  do it right in extract_char(), since there is no check if a
         *  player rents out and it can leave them in an equally screwy
         *  situation.
         */

        if FREE_RENT {
            crash_rentsave(game, chars, db, objs,chid, 0);
        }

        /* If someone is quitting in their house, let them load back here. */
        let ch = chars.get(chid);
        if !ch.plr_flagged(PLR_LOADROOM) && db.room_flagged(ch.in_room(), RoomFlags::HOUSE) {
            let val = db.get_room_vnum(ch.in_room());
            let ch = chars.get_mut(chid);
            ch.set_loadroom(val);
        }

        db.extract_char(chars, chid); /* Char is saved before extracting. */
    }
}

pub fn do_save(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() || ch.desc.is_none() {
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
            send_to_char(&mut game.descriptors, ch, "Saving aliases.\r\n");
            let ch = chars.get(chid);
            write_aliases(ch);
            return;
        }
        send_to_char(&mut game.descriptors, 
            ch,
            format!("Saving {} and aliases.\r\n", ch.get_name()).as_str(),
        );
    }
    let ch = chars.get(chid);
    write_aliases(ch);
    save_char(&mut game.descriptors, db, chars,texts, objs,chid);
    crash_crashsave(chars, db,objs, chid);
    let ch = chars.get(chid);
    if db.room_flagged(ch.in_room(), RoomFlags::HOUSE_CRASH) {
        let in_room = db.get_room_vnum(ch.in_room());
        house_crashsave(chars, db, objs, in_room);
    }
}

/* generic function for commands which are normally overridden by
special procedures - i.e., shop commands, mail commands, etc. */
pub fn do_not_here(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    send_to_char(&mut game.descriptors, ch, "Sorry, but you cannot do that here!\r\n");
}

pub fn do_sneak(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get_mut(chid);
    if ch.is_npc() || ch.get_skill(SKILL_SNEAK) == 0 {
        send_to_char(&mut game.descriptors, ch, "You have no idea how to do that.\r\n");
        return;
    }
    send_to_char(&mut game.descriptors, ch, "Okay, you'll try to move silently for a while.\r\n");
    if ch.aff_flagged(AffectFlags::SNEAK) {
        affect_from_char( objs,ch, SKILL_SNEAK as i16);
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
        bitvector: AffectFlags::SNEAK,
    };

    affect_to_char( objs, ch, af);
}

pub fn do_hide(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() || ch.get_skill(SKILL_HIDE) == 0 {
        send_to_char(&mut game.descriptors, ch, "You have no idea how to do that.\r\n");
        return;
    }

    send_to_char(&mut game.descriptors, ch, "You attempt to hide yourself.\r\n");
    let ch = chars.get_mut(chid);
    if ch.aff_flagged(AffectFlags::HIDE) {
        ch.remove_aff_flags(AffectFlags::HIDE);
    }

    let percent = rand_number(1, 101); /* 101% is a complete failure */

    if percent
        > (ch.get_skill(SKILL_HIDE) as i16 + DEX_APP_SKILL[ch.get_dex() as usize].hide) as u32
    {
        return;
    }
    ch.set_aff_flags_bits(AffectFlags::HIDE);
}

pub fn do_steal(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() || ch.get_skill(SKILL_STEAL) == 0 {
        send_to_char(&mut game.descriptors, ch, "You have no idea how to do that.\r\n");
        return;
    }
    if db.room_flagged(ch.in_room(), RoomFlags::PEACEFUL) {
        send_to_char(&mut game.descriptors, 
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
        vict = get_char_vis(&game.descriptors, chars,db, ch, &mut vict_name, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, "Steal what from who?\r\n");
        return;
    } else if vict.unwrap().id() == chid {
        send_to_char(&mut game.descriptors, ch, "Come on now, that's rather stupid!\r\n");
        return;
    }
    let mut ohoh = false;

    let vict = vict.unwrap();
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
    let vict_id = vict.id();
    if obj_name != "coins" && obj_name != "gold" {
        if {
            obj = get_obj_in_list_vis(&game.descriptors, chars,db, objs,ch, &mut obj_name, None, &vict.carrying);
            obj.is_none()
        } {
            for eq_pos in 0..NUM_WEARS {
                if vict.get_eq(eq_pos).is_some()
                    && isname(
                        &obj_name,
                        objs.get(vict.get_eq(eq_pos).unwrap()).name.as_ref(),
                    )
                    && can_see_obj(&game.descriptors, chars, db, ch, objs.get(vict.get_eq(eq_pos).unwrap()))
                {
                    obj = vict.get_eq(eq_pos).map(|i| objs.get(i));
                    the_eq_pos = eq_pos;
                }
            }
            if obj.is_none() {
                act(&mut game.descriptors, chars, 
                    db,
                    "$E hasn't got that item.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(vict)),
                    TO_CHAR,
                );
                return;
            } else {
                /* It is equipment */
                if vict.get_pos() > POS_STUNNED {
                    send_to_char(&mut game.descriptors, ch, "Steal the equipment now?  Impossible!\r\n");
                    return;
                } else {
                    let obj = obj.unwrap();
                    act(&mut game.descriptors, chars, 
                        db,
                        "You unequip $p and steal it.",
                        false,
                        Some(ch),
                        Some(obj),
                        None,
                        TO_CHAR,
                    );
                    act(&mut game.descriptors, chars, 
                        db,
                        "$n steals $p from $N.",
                        false,
                        Some(ch),
                        Some(obj),
                        Some(VictimRef::Char(vict)),
                        TO_NOTVICT,
                    );
                    let eqid = db.unequip_char(chars, objs,vict.id(), the_eq_pos).unwrap();
                    let eq = objs.get_mut(eqid);
                    let ch = chars.get_mut(chid);
                    obj_to_char(eq, ch);
                }
            }
        } else {
            /* obj found in inventory */
            let obj = obj.unwrap();

            percent += obj.get_obj_weight(); /* Make heavy harder */
            if percent > ch.get_skill(SKILL_STEAL) as u32 as i32 {
                ohoh = true;
                send_to_char(&mut game.descriptors, ch, "Oops..\r\n");
                act(&mut game.descriptors, chars, 
                    db,
                    "$n tried to steal something from you!",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(vict)),
                    TO_VICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$n tries to steal something from $N.",
                    true,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(vict)),
                    TO_NOTVICT,
                );
            } else {
                /* Steal the item */
                if ch.is_carrying_n() + 1 < ch.can_carry_n() as u8 {
                    if ch.is_carrying_w() + obj.get_obj_weight() < ch.can_carry_w() as i32 {
                        let obj_id = obj.id();
                        let obj = objs.get_mut(obj_id);
                        obj_from_char(chars, obj);
                        let ch = chars.get_mut(chid);
                        obj_to_char(obj, ch);
                        send_to_char(&mut game.descriptors, ch, "Got it!\r\n");
                    }
                } else {
                    send_to_char(&mut game.descriptors, ch, "You cannot carry that much.\r\n");
                }
            }
        }
    } else {
        /* Steal some coins */
        if vict.awake() && percent > ch.get_skill(SKILL_STEAL) as u32 as i32 {
            ohoh = true;
            send_to_char(&mut game.descriptors, ch, "Oops..\r\n");
            act(&mut game.descriptors, chars, 
                db,
                "You discover that $n has $s hands in your wallet.",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n tries to steal gold from $N.",
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_NOTVICT,
            );
        } else {
            /* Steal some gold coins */
            let mut gold = vict.get_gold() * rand_number(1, 10) as i32 / 100;
            gold = min(1782, gold);
            if gold > 0 {
                let ch = chars.get_mut(chid);
                ch.set_gold(ch.get_gold() + gold);
                let vict = chars.get_mut(vict_id);
                vict.set_gold(vict.get_gold() - gold);
                let ch = chars.get(chid);
                if gold > 1 {
                    send_to_char(&mut game.descriptors, 
                        ch,
                        format!("Bingo!  You got {} gold coins.\r\n", gold).as_str(),
                    );
                } else {
                    send_to_char(&mut game.descriptors, ch, "You manage to swipe a solitary gold coin.\r\n");
                }
            } else {
                send_to_char(&mut game.descriptors, ch, "You couldn't get any gold...\r\n");
            }
        }
    }
    let vict = chars.get(vict_id);
    if ohoh && vict.is_npc() && vict.awake() {
        game.hit(chars, db, texts, objs,vict_id, chid, TYPE_UNDEFINED);
    }
}

pub fn do_practice(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() {
        return;
    }
    let mut arg = String::new();
    one_argument(argument, &mut arg);

    if !arg.is_empty() {
        send_to_char(&mut game.descriptors, ch, "You can only practice skills in your guild.\r\n");
    } else {
        list_skills(game, chars, db, chid);
    }
}

pub fn do_visible(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.get_level() >= LVL_IMMORT as u8 {
        perform_immort_vis(&mut game.descriptors, db, chars, objs,chid);
        return;
    }
    let ch = chars.get(chid);
    if ch.aff_flagged(AffectFlags::INVISIBLE) {
        appear(&mut game.descriptors, chars, db, objs,chid);
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, ch, "You break the spell of invisibility.\r\n");
    } else {
        send_to_char(&mut game.descriptors, ch, "You are already visible.\r\n");
    }
}

pub fn do_title(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if ch.is_npc() {
        send_to_char(&mut game.descriptors, ch, "Your title is fine... go away.\r\n");
    } else if ch.plr_flagged(PLR_NOTITLE) {
        send_to_char(&mut game.descriptors, 
            ch,
            "You can't title yourself -- you shouldn't have abused it!\r\n",
        );
    } else if argument.contains('(') || argument.contains('(') {
        send_to_char(&mut game.descriptors, ch, "Titles can't contain the ( or ) characters.\r\n");
    } else if argument.len() > MAX_TITLE_LENGTH {
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "Sorry, titles can't be longer than {} characters.\r\n",
                MAX_TITLE_LENGTH
            )
            .as_str(),
        );
    } else {
        let ch = chars.get_mut(chid);
        ch.set_title(Some(argument.into()));
        let ch = chars.get(chid);

        send_to_char(&mut game.descriptors, 
            ch,
            format!("Okay, you're now {} {}.\r\n", ch.get_name(), ch.get_title()).as_str(),
        );
    }
}

fn perform_group(descs: &mut Depot<DescriptorData>, db: &mut DB,chars: &mut Depot<CharData>, chid: DepotId, vict_id: DepotId) -> i32 {
    let ch = chars.get(chid);
    let vict = chars.get(vict_id);
    if vict.aff_flagged(AffectFlags::GROUP) || !can_see(descs, chars, db, ch, vict) {
        return 0;
    }
    let vict = chars.get_mut(vict_id);
    vict.set_aff_flags_bits(AffectFlags::GROUP);
    let ch = chars.get(chid);
    let vict = chars.get(vict_id);
    if chid != vict_id {
        act(descs, chars, 
            db,
            "$N is now a member of your group.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR,
        );
    }
    act(descs, chars, 
        db,
        "You are now a member of $n's group.",
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_VICT,
    );
    act(descs, chars, 
        db,
        "$N is now a member of $n's group.",
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(vict)),
        TO_NOTVICT,
    );
    return 1;
}

fn print_group(descs: &mut Depot<DescriptorData>, db: &mut DB,chars: &mut Depot<CharData>, chid: DepotId) {
    let ch = chars.get(chid);
    if !ch.aff_flagged(AffectFlags::GROUP) {
        send_to_char(descs, ch, "But you are not the member of a group!\r\n");
    } else {
        send_to_char(descs, ch, "Your group consists of:\r\n");
        let ch = chars.get(chid);
        let k_id = if ch.master.is_some() {
            ch.master.unwrap()
        } else {
            chid
        };
        let k = chars.get(k_id);

        if k.aff_flagged(AffectFlags::GROUP) {
            let buf = format!(
                "     [{:3}H {:3}M {:3}V] [{:2} {}] $N (Head of group)",
                k.get_hit(),
                k.get_mana(),
                k.get_move(),
                k.get_level(),
                k.class_abbr()
            );
            act(descs, chars, 
                db,
                &buf,
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(k)),
                TO_CHAR,
            );
        }
        for f in &k.followers {
            let follower = chars.get(f.follower);
            if !follower.aff_flagged(AffectFlags::GROUP) {
                continue;
            }

            let buf = format!(
                "     [{:3}H {:3}M {:3}V] [{:2} {}] $N",
                follower.get_hit(),
                follower.get_mana(),
                follower.get_move(),
                follower.get_level(),
                follower.class_abbr()
            );
            act(descs, chars, 
                db,
                &buf,
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(follower)),
                TO_CHAR,
            );
        }
    }
}

pub fn do_group(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf = String::new();

    one_argument(argument, &mut buf);

    if buf.is_empty() {
        print_group(&mut game.descriptors, db,chars, chid);
        return;
    }

    if ch.master.is_some() {
        act(&mut game.descriptors, chars, 
            db,
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
        perform_group(&mut game.descriptors, db, chars,chid, chid);
        let mut found = 0;
        let ch = chars.get(chid);
        for f in ch.followers.clone() {
            found += perform_group(&mut game.descriptors, db, chars,chid, f.follower);
        }
        if found == 0 {
            let ch = chars.get(chid);
            send_to_char(&mut game.descriptors, ch, "Everyone following you is already in your group.\r\n");
        }
        return;
    }
    let vict;

    if {
        vict = get_char_vis(&game.descriptors, chars,db, ch, &mut buf, None, FIND_CHAR_ROOM);
        vict.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, NOPERSON);
    } else if (vict.unwrap().master.is_none()
        ||vict.unwrap().master.unwrap() != chid)
        && vict.unwrap().id() != chid
    {
        let vict = vict.unwrap();
        act(&mut game.descriptors, chars, 
            db,
            "$N must follow you to enter your group.",
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(vict)),
            TO_CHAR,
        );
    } else {
        let vict = vict.unwrap();
        let ch = chars.get(chid);

        if !vict.aff_flagged(AffectFlags::GROUP) {
            perform_group(&mut game.descriptors, db, chars,chid, vict.id());
        } else {
            if chid != vict.id() {
                act(&mut game.descriptors, chars, 
                    db,
                    "$N is no longer a member of your group.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(vict)),
                    TO_CHAR,
                );
            }
            act(&mut game.descriptors, chars, 
                db,
                "You have been kicked out of $n's group!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_VICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$N has been kicked out of $n's group!",
                false,
                Some(ch),
                None,
                Some(VictimRef::Char(vict)),
                TO_NOTVICT,
            );
            let vict = chars.get_mut(vict.id());
            vict.remove_aff_flags(AffectFlags::GROUP);
        }
    }
}

pub fn do_ungroup(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf = String::new();
    one_argument(argument, &mut buf);

    if buf.is_empty() {
        if ch.master.is_some() || !ch.aff_flagged(AffectFlags::GROUP) {
            send_to_char(&mut game.descriptors, ch, "But you lead no group!\r\n");
            return;
        }

        for f in ch.followers.clone() {
            let follower = chars.get(f.follower);
            if follower.aff_flagged(AffectFlags::GROUP) {
                let follower = chars.get_mut(f.follower);
                follower.remove_aff_flags(AffectFlags::GROUP);
                let follower = chars.get(f.follower);
                let ch = chars.get(chid);
                act(&mut game.descriptors, chars, 
                    db,
                    "$N has disbanded the group.",
                    true,
                    Some(follower),
                    None,
                    Some(VictimRef::Char(ch)),
                    TO_CHAR,
                );
                let follower = chars.get(f.follower);
                if !follower.aff_flagged(AffectFlags::CHARM) {
                    stop_follower(&mut game.descriptors, chars, db, objs,f.follower);
                }
            }
        }
        let ch = chars.get_mut(chid);
        ch.remove_aff_flags(AffectFlags::GROUP);

        send_to_char(&mut game.descriptors, ch, "You disband the group.\r\n");
        return;
    }
    let tch;
    if {
        tch = get_char_vis(&game.descriptors, chars,db, ch, &mut buf, None, FIND_CHAR_ROOM);
        tch.is_none()
    } {
        send_to_char(&mut game.descriptors, ch, "There is no such person!\r\n");
        return;
    }
    let tch = tch.unwrap();
    if tch.master.is_none() || tch.master.unwrap() != chid {
        send_to_char(&mut game.descriptors, ch, "That person is not following you!\r\n");
        return;
    }

    if !tch.aff_flagged(AffectFlags::GROUP) {
        send_to_char(&mut game.descriptors, ch, "That person isn't in your group.\r\n");
        return;
    }
    let tchid = tch.id();
    let tch = chars.get_mut(tchid);
    tch.remove_aff_flags(AffectFlags::GROUP);
    let ch = chars.get(chid);
    let tch = chars.get(tchid);
    act(&mut game.descriptors, chars, 
        db,
        "$N is no longer a member of your group.",
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(tch)),
        TO_CHAR,
    );
    act(&mut game.descriptors, chars, 
        db,
        "You have been kicked out of $n's group!",
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(tch)),
        TO_VICT,
    );
    act(&mut game.descriptors, chars, 
        db,
        "$N has been kicked out of $n's group!",
        false,
        Some(ch),
        None,
        Some(VictimRef::Char(tch)),
        TO_NOTVICT,
    );
    let tch = chars.get(tchid);
    if !tch.aff_flagged(AffectFlags::CHARM) {
        stop_follower(&mut game.descriptors, chars, db,objs, tchid);
    }
}

pub fn do_report(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if !ch.aff_flagged(AffectFlags::GROUP) {
        send_to_char(&mut game.descriptors, ch, "But you are not a member of any group!\r\n");
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

    let k_id = if ch.master.is_some() {
        ch.master.unwrap()
    } else {
        chid
    };
    let k = chars.get(k_id);
    for f in &k.followers {
        let follower = chars.get(f.follower);
        if follower.aff_flagged(AffectFlags::GROUP) && f.follower != chid {
            act(&mut game.descriptors, chars, 
                db,
                &buf,
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(follower)),
                TO_VICT,
            );
        }
    }
    if k_id != chid {
        act(&mut game.descriptors, chars, 
            db,
            &buf,
            true,
            Some(ch),
            None,
            Some(VictimRef::Char(k)),
            TO_VICT,
        );
    }

    send_to_char(&mut game.descriptors, ch, "You report to the group.\r\n");
}

pub fn do_split(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() {
        return;
    }
    let mut buf = String::new();
    one_argument(argument, &mut buf);
    let amount;
    if is_number(&buf) {
        amount = buf.parse::<i32>().unwrap();
        if amount <= 0 {
            send_to_char(&mut game.descriptors, ch, "Sorry, you can't do that.\r\n");
            return;
        }
        if amount > ch.get_gold() {
            send_to_char(&mut game.descriptors, ch, "You don't seem to have that much gold to split.\r\n");
            return;
        }
        let k_id = if ch.master.is_some() {
            ch.master.unwrap()
        } else {
            chid
        };
        let k = chars.get(k_id);
        let mut num;
        if k.aff_flagged(AffectFlags::GROUP) && k.in_room() == ch.in_room() {
            num = 1;
        } else {
            num = 0;
        }

        for f in &k.followers {
            let follower = chars.get(f.follower);
            if follower.aff_flagged(AffectFlags::GROUP)
                && !follower.is_npc()
                && follower.in_room() == ch.in_room()
            {
                num += 1;
            }
        }
        let share;
        let rest;
        if num != 0 && ch.aff_flagged(AffectFlags::GROUP) {
            share = amount / num;
            rest = amount % num;
        } else {
            send_to_char(&mut game.descriptors, ch, "With whom do you wish to share your gold?\r\n");
            return;
        }
        let ch = chars.get_mut(chid);
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
        let k = chars.get(k_id);
        let ch = chars.get(chid);
        if k.aff_flagged(AffectFlags::GROUP) && k.in_room() == ch.in_room() && !k.is_npc() && k_id != chid {
            let k = chars.get_mut(k_id);
            k.set_gold(k.get_gold() + share);
            send_to_char(&mut game.descriptors, k, &buf);
        }
        let k = chars.get(k_id);
        for f in  k.followers.clone() {
            let follower = chars.get(f.follower);
            let ch = chars.get(chid);
            if follower.aff_flagged(AffectFlags::GROUP)
                && !follower.is_npc()
                && follower.in_room() == ch.in_room()
                && f.follower != chid
            {
                let follower = chars.get_mut(f.follower);
                follower.set_gold(follower.get_gold() + share);

                send_to_char(&mut game.descriptors, follower, &buf);
            }
        }
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "You split {} coins among {} members -- {} coins each.\r\n",
                amount, num, share
            )
            .as_str(),
        );

        if rest != 0 {
            send_to_char(&mut game.descriptors, 
                ch,
                format!(
                    "{} coin{} {} not splitable, so you keep the money.\r\n",
                    rest,
                    if rest == 1 { "" } else { "s" },
                    if rest == 1 { "was" } else { "were" }
                )
                .as_str(),
            );
            let ch = chars.get_mut(chid);
            ch.set_gold(ch.get_gold() + rest);
        }
    } else {
        send_to_char(&mut game.descriptors, 
            ch,
            "How many coins do you wish to split with your group?\r\n",
        );
        return;
    }
}

pub fn do_use(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut buf = String::new();
    let mut arg = String::new();
    let mut argument = argument.to_string();

    half_chop(&mut argument, &mut arg, &mut buf);
    if arg.is_empty() {
        send_to_char(&mut game.descriptors, 
            ch,
            format!("What do you want to {}?\r\n", CMD_INFO[cmd].command).as_str(),
        );
        return;
    }
    let mut mag_item = ch.get_eq(WEAR_HOLD as i8).map(|i| objs.get(i));

    if mag_item.is_none() || !isname(&arg, mag_item.unwrap().name.as_ref()) {
        match subcmd {
            SCMD_RECITE | SCMD_QUAFF => {
                if {
                    mag_item = get_obj_in_list_vis(&game.descriptors, chars,db, objs,ch, &arg, None, &ch.carrying);
                    mag_item.is_none()
                } {
                    send_to_char(&mut game.descriptors, 
                        ch,
                        format!("You don't seem to have {} {}.\r\n", an!(arg), arg).as_str(),
                    );
                    return;
                }
            }
            SCMD_USE => {
                send_to_char(&mut game.descriptors, 
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
    let mag_item = mag_item.unwrap();
    match subcmd {
        SCMD_QUAFF => {
            if mag_item.get_obj_type() != ITEM_POTION {
                send_to_char(&mut game.descriptors, ch, "You can only quaff potions.\r\n");
                return;
            }
        }
        SCMD_RECITE => {
            if mag_item.get_obj_type() != ITEM_SCROLL {
                send_to_char(&mut game.descriptors, ch, "You can only recite scrolls.\r\n");
                return;
            }
        }
        SCMD_USE => {
            if mag_item.get_obj_type() != ITEM_WAND
                && mag_item.get_obj_type() != ITEM_STAFF
            {
                send_to_char(&mut game.descriptors, ch, "You can't seem to figure out how to use it.\r\n");
                return;
            }
        }
        _ => {}
    }

    mag_objectmagic(game, chars, db, texts,objs, chid, mag_item.id(), &buf);
}

pub fn do_wimpy(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    let mut arg = String::new();

    /* 'wimp_level' is a player_special. -gg 2/25/98 */
    if ch.is_npc() {
        return;
    }

    one_argument(argument, &mut arg);

    if arg.is_empty() {
        if ch.get_wimp_lev() != 0 {
            send_to_char(&mut game.descriptors, 
                ch,
                format!(
                    "Your current wimp level is {} hit points.\r\n",
                    ch.get_wimp_lev()
                )
                .as_str(),
            );
            return;
        } else {
            send_to_char(&mut game.descriptors, ch, "At the moment, you're not a wimp.  (sure, sure...)\r\n");
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
                send_to_char(&mut game.descriptors, ch, "Heh, heh, heh.. we are jolly funny today, eh?\r\n");
            } else if wimp_lev > ch.get_max_hit() as i32 {
                send_to_char(&mut game.descriptors, ch, "That doesn't make much sense, now does it?\r\n");
            } else if wimp_lev > (ch.get_max_hit() / 2) as i32 {
                send_to_char(&mut game.descriptors, 
                    ch,
                    "You can't set your wimp level above half your hit points.\r\n",
                );
            } else {
                send_to_char(&mut game.descriptors, 
                    ch,
                    format!(
                        "Okay, you'll wimp out if you drop below {} hit points.\r\n",
                        wimp_lev
                    )
                    .as_str(),
                );
                let ch = chars.get_mut(chid);
                ch.set_wimp_lev(wimp_lev);
            }
        } else {
            send_to_char(&mut game.descriptors, 
                ch,
                "Okay, you'll now tough out fights to the bitter end.\r\n",
            );
            let ch = chars.get_mut(chid);
            ch.set_wimp_lev(0);
        }
    } else {
        send_to_char(&mut game.descriptors, 
            ch,
            "Specify at how many hit points you want to wimp out at.  (0 to disable)\r\n",
        );
    }
}

pub fn do_display(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    _cmd: usize,
    _subcmd: i32,
) {
    let ch = chars.get(chid);
    if ch.is_npc() {
        send_to_char(&mut game.descriptors, ch, "Monsters don't need displays.  Go away.\r\n");
        return;
    }
    let argument = argument.trim_start();

    if argument.len() == 0 {
        send_to_char(&mut game.descriptors, 
            ch,
            "Usage: prompt { { H | M | V } | all | auto | none }\r\n",
        );
        return;
    }

    if argument == "auto" {
        let ch = chars.get_mut(chid);
        ch.toggle_prf_flag_bits(PRF_DISPAUTO);
        let ch = chars.get(chid);
        send_to_char(&mut game.descriptors, 
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

    let ch = chars.get_mut(chid);
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
                    send_to_char(&mut game.descriptors, 
                        ch,
                        "Usage: prompt { { H | M | V } | all | auto | none }\r\n",
                    );
                    return;
                }
            }
        }
    }

    send_to_char(&mut game.descriptors, ch, OK);
}

pub fn do_gen_write(
    game: &mut Game,
    db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    argument: &str,
    cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
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
        send_to_char(&mut game.descriptors, ch, "Monsters can't have ideas - Go away.\r\n");
        return;
    }

    let mut argument = argument.trim_start().to_string();
    delete_doubledollar(&mut argument);

    if argument.is_empty() {
        send_to_char(&mut game.descriptors, ch, "That must be a mistake...\r\n");
        return;
    }
    game.mudlog(chars,
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
        send_to_char(&mut game.descriptors, 
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
        send_to_char(&mut game.descriptors, ch, "Could not open the file.  Sorry.\r\n");
        return;
    }
    let ch = chars.get(chid);
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
        send_to_char(&mut game.descriptors, ch, "Could not write to the file.  Sorry.\r\n");
        return;
    }

    send_to_char(&mut game.descriptors, ch, "Okay.  Thanks!\r\n");
}

const TOG_ON: usize = 1;
const TOG_OFF: usize = 0;

macro_rules! prf_tog_chk {
    ($ch:expr, $flag:expr) => {
        ($ch.toggle_prf_flag_bits($flag) & $flag) != 0
    };
}

pub fn do_gen_tog(
    game: &mut Game,
    _db: &mut DB,chars: &mut Depot<CharData>,_texts: &mut Depot<TextData>,_objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _argument: &str,
    _cmd: usize,
    subcmd: i32,
) {
    let ch = chars.get(chid);
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
    let ch = chars.get_mut(chid);
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
                game.config.nameserver_is_slow = !game.config.nameserver_is_slow;
                game.config.nameserver_is_slow
            }
        }
        SCMD_AUTOEXIT => {
            result = prf_tog_chk!(ch, PRF_AUTOEXIT);
        }
        SCMD_TRACK => {
            result = {
                game.config.track_through_doors = !game.config.track_through_doors;
                game.config.track_through_doors
            }
        }
        _ => {
            error!("SYSERR: Unknown subcmd {} in do_gen_toggle.", subcmd);
            return;
        }
    }

    if result {
        send_to_char(&mut game.descriptors, ch, TOG_MESSAGES[subcmd as usize][TOG_ON]);
    } else {
        send_to_char(&mut game.descriptors, ch, TOG_MESSAGES[subcmd as usize][TOG_OFF]);
    }

    return;
}
