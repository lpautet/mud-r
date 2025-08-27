/* ************************************************************************
*   File: castle.rs                                     Part of CircleMUD *
*  Usage: handling of multiple bulletin boards                            *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

/* IMPORTANT!
The below defined number is the zone number of the Kings Castle.
Change it to apply to your chosen zone number. The default zone
number (On Alex and Alfa) is 80 (That is rooms and mobs have numbers
in the 8000 series... */

use std::iter::Iterator;

use crate::depot::{Depot, DepotId};
use crate::handler::obj_to_char;
use crate::{act, send_to_char, DescriptorData, TextData, VictimRef};
use log::error;

use crate::act_movement::{do_follow, do_gen_door, perform_move};
use crate::db::DB;
use crate::interpreter::{SCMD_CLOSE, SCMD_LOCK, SCMD_OPEN, SCMD_UNLOCK};
use crate::spell_parser::cast_spell;
use crate::spells::{SPELL_COLOR_SPRAY, SPELL_FIREBALL, SPELL_HARM, SPELL_HEAL, TYPE_UNDEFINED};
use crate::structs::{
    CharData, MeRef, MobVnum, ObjData, RoomRnum, RoomVnum, Special, ItemType, WearFlags,
    NOBODY, NOWHERE, POS_FIGHTING, POS_SITTING, POS_SLEEPING, POS_STANDING,
};
use crate::util::rand_number;
use crate::{Game, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_VICT};

const Z_KINGS_C: i32 = 150;

/**********************************************************************\
|* Special procedures for Kings Castle by Pjotr (d90-pem@nada.kth.se) *|
|* Coded by Sapowox (d90-jkr@nada.kth.se)                             *|
\**********************************************************************/

/*
 * Assign castle special procedures.
 *
 * NOTE: The mobile number isn't fully specified. It's only an offset
 *	from the zone's base.
 */
fn castle_mob_spec(db: &mut DB, mobnum: MobVnum, specproc: Special) {
    let vmv = castle_virtual(db, mobnum);
    let mut rmr = NOBODY;

    if vmv != NOBODY {
        rmr = db.real_mobile(vmv);
    }

    if rmr == NOBODY {
        if !db.mini_mud {
            error!("SYSERR: assign_kings_castle(): can't find mob #{}.", vmv);
        }
    } else {
        db.mob_index[rmr as usize].func = Some(specproc);
    }
}

fn castle_virtual(db: &DB, offset: MobVnum) -> MobVnum {
    let zon = db.real_zone(Z_KINGS_C as RoomVnum);

    if zon.is_none() {
        return NOBODY;
    }

    return db.zone_table[zon.unwrap()].bot + offset;
}

fn castle_real_room(db: &DB, roomoffset: RoomVnum) -> RoomRnum {
    let zon = db.real_zone(Z_KINGS_C as RoomVnum);

    if zon.is_none() {
        return NOWHERE;
    }

    return db.real_room(db.zone_table[zon.unwrap()].bot + roomoffset);
}

/*
 * Routine: assign_kings_castle
 *
 * Used to assign function pointers to all mobiles in the Kings Castle.
 * Called from spec_assign.c.
 */
pub fn assign_kings_castle(db: &mut DB) {
    castle_mob_spec( db, 0, castle_guard); /* Gwydion */
    /* Added the previous line -- Furry */
    castle_mob_spec( db, 1, king_welmar); /* Our dear friend, the King */
    castle_mob_spec( db, 3, castle_guard); /* Jim */
    castle_mob_spec( db, 4, castle_guard); /* Brian */
    castle_mob_spec( db, 5, castle_guard); /* Mick */
    castle_mob_spec( db, 6, castle_guard); /* Matt */
    castle_mob_spec( db, 7, castle_guard); /* Jochem */
    castle_mob_spec( db, 8, castle_guard); /* Anne */
    castle_mob_spec( db, 9, castle_guard); /* Andrew */
    castle_mob_spec( db, 10, castle_guard); /* Bertram */
    castle_mob_spec( db, 11, castle_guard); /* Jeanette */
    castle_mob_spec( db, 12, peter); /* Peter */
    castle_mob_spec( db, 13, training_master); /* The training master */
    castle_mob_spec( db, 16, james); /* James the Butler */
    castle_mob_spec( db, 17, cleaning); /* Ze Cleaning Fomen */
    castle_mob_spec( db, 20, tim); /* Tim, Tom's twin */
    castle_mob_spec( db, 21, tom); /* Tom, Tim's twin */
    castle_mob_spec( db, 24, dick_n_david); /* Dick, guard of the
                                            * Treasury */
    castle_mob_spec( db, 25, dick_n_david); /* David, Dicks brother */
    castle_mob_spec( db, 26, jerry); /* Jerry, the Gambler */
    castle_mob_spec( db, 27, castle_guard); /* Michael */
    castle_mob_spec( db, 28, castle_guard); /* Hans */
    castle_mob_spec( db, 29, castle_guard); /* Boris */
}

/*
 * Routine: member_of_staff
 *
 * Used to see if a character is a member of the castle staff.
 * Used mainly by BANZAI:ng NPC:s.
 */
fn member_of_staff(db: &DB, ch: &CharData) -> bool {
    if !ch.is_npc() {
        return false;
    }

    let ch_num = db.get_mob_vnum(ch);

    if ch_num == castle_virtual(db, 1) {
        return true;
    }

    if ch_num > castle_virtual(db, 2) && ch_num < castle_virtual(db, 15) {
        return true;
    }

    if ch_num > castle_virtual(db, 15) && ch_num < castle_virtual(db, 18) {
        return true;
    }

    if ch_num > castle_virtual(db, 18) && ch_num < castle_virtual(db, 30) {
        return true;
    }

    return false;
}

/*
 * Function: member_of_royal_guard
 *
 * Returns true if the character is a guard on duty, otherwise false.
 * Used by Peter the captain of the royal guard.
 */
fn member_of_royal_guard(db: &DB, ch: &CharData) -> bool {
    if !ch.is_npc() {
        return false;
    }

    let ch_num = db.get_mob_vnum(ch);

    if ch_num == castle_virtual(db, 3) || ch_num == castle_virtual(db, 6) {
        return true;
    }

    if ch_num > castle_virtual(db, 7) && ch_num < castle_virtual(db, 12) {
        return true;
    }

    if ch_num > castle_virtual(db, 23) && ch_num < castle_virtual(db, 26) {
        return true;
    }

    return false;
}

/*
 * Function: find_npc_by_name
 *
 * Returns a pointer to an npc by the given name.
 * Used by Tim and Tom
 */
fn find_npc_by_name(chars: &Depot<CharData>, db: &DB, ch_at: &CharData, name: &str) -> Option<DepotId> {
    db.world[ch_at.in_room() as usize]
        .peoples
        .iter()
        .find(|e| chars.get(**e).is_npc() && chars.get(**e).player.short_descr.starts_with(name))
        .map(|e| *e)
}

/*
 * Function: find_guard
 *
 * Returns the pointer to a guard on duty.
 * Used by Peter the Captain of the Royal Guard
 */
fn find_guard(chars: &Depot<CharData>, db: &DB, ch_at: &CharData) -> Option<DepotId> {
    db.world[ch_at.in_room() as usize]
        .peoples
        .iter()
        .find(|e| chars.get(**e).fighting_id().is_none() && member_of_royal_guard(db, chars.get(**e)))
        .map(|e| *e)
}

/*
 * Function: get_victim
 *
 * Returns a pointer to a randomly chosen character in the same room,
 * fighting someone in the castle staff...
 * Used by BANZAII-ing characters and King Welmar...
 */
fn get_victim(chars: &Depot<CharData>, db: &DB, ch_at: &CharData) -> Option<DepotId> {
    let mut num_bad_guys = 0;

    for chid in db.world[ch_at.in_room() as usize].peoples.iter() {
        if chars.get(*chid).fighting_id().is_some()
            && member_of_staff(db, chars.get(chars.get(*chid).fighting_id().unwrap()))
        {
            num_bad_guys += 1;
        }
    }

    if num_bad_guys == 0 {
        return None;
    }

    let victim = rand_number(0, num_bad_guys); /* How nice, we give them a chance */
    if victim == 0 {
        return None;
    }

    num_bad_guys = 0;

    for chid in db.world[ch_at.in_room() as usize].peoples.iter() {
        if chars.get(*chid).fighting_id().is_none() {
            continue;
        }

        if !member_of_staff(db, chars.get(chars.get(*chid).fighting_id().unwrap())) {
            continue;
        }

        num_bad_guys += 1;

        if num_bad_guys != victim {
            continue;
        }

        return Some(*chid);
    }
    None
}

/*
 * Function: banzaii
 *
 * Makes a character banzaii on attackers of the castle staff.
 * Used by Guards, Tim, Tom, Dick, David, Peter, Master, King and Guards.
 */
fn banzaii(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>,  chid: DepotId) -> bool {
    let ch = chars.get(chid);
    let opponent_id = get_victim(chars,db, ch);
    if !ch.awake() || ch.get_pos() == POS_FIGHTING || opponent_id.is_none() {
        return false;
    }

    act(&mut game.descriptors, chars, 
        db,
        "$n roars: 'Protect the Kingdom of Great King Welmar!  BANZAIIII!!!'",
        false,
        Some(ch),
        None,
        None,
        TO_ROOM,
    );
    game.hit(chars, db, texts, objs,chid, opponent_id.unwrap(), TYPE_UNDEFINED);
    return true;
}

/*
 * Function: do_npc_rescue
 *
 * Makes ch_hero rescue ch_victim.
 * Used by Tim and Tom
 */
fn do_npc_rescue(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>, 
    chid_hero_id: DepotId,
    ch_victim_id: DepotId,
) -> bool {
    let chid_bad_guy = db.world[chars.get(chid_hero_id).in_room() as usize]
        .peoples
        .iter()
        .find(|id| match chars.get(**id).fighting_id() {
            Some(fighting_id) if fighting_id != ch_victim_id => true,
            _ => false,
        })
        .map(|e| *e);

    /* NO WAY I'll rescue the one I'm fighting! */
    if chid_bad_guy.is_none() || chid_bad_guy.unwrap() == chid_hero_id {
        return false;
    }
    let ch_hero = chars.get(chid_hero_id);
    let ch_victim = chars.get(ch_victim_id);
    act(&mut game.descriptors, chars, 
        db,
        "You bravely rescue $N.\r\n",
        false,
        Some(ch_hero),
        None,
        Some(VictimRef::Char(ch_victim)),
        TO_CHAR,
    );
    act(&mut game.descriptors, chars, 
        db,
        "You are rescued by $N, your loyal friend!\r\n",
        false,
        Some(ch_victim),
        None,
        Some(VictimRef::Char(ch_hero)),
        TO_CHAR,
    );
    act(&mut game.descriptors, chars, 
        db,
        "$n heroically rescues $N.",
        false,
        Some(ch_hero),
        None,
        Some(VictimRef::Char(ch_victim)),
        TO_NOTVICT,
    );
    if chars.get(chid_bad_guy.unwrap()).fighting_id().is_some() {
        db.stop_fighting(chars.get_mut(chid_bad_guy.unwrap()));
    }
    if chars.get(chid_hero_id).fighting_id().is_some() {
        db.stop_fighting(chars.get_mut(chid_hero_id));
    }

    game.set_fighting(chars, db, objs,chid_hero_id, chid_bad_guy.unwrap());
    game.set_fighting(chars, db, objs,chid_bad_guy.unwrap(), chid_hero_id);
    return true;
}

/*
 * Procedure to block a person trying to enter a room.
 * Used by Tim/Tom at Kings bedroom and Dick/David at treasury.
 */
fn block_way(
    descs: &mut Depot<DescriptorData>,
    chars: &Depot<CharData>, db: &DB,
    chid: DepotId,
    cmd: i32,
    _arg: &str,
    in_room: RoomVnum,
    prohibited_direction: i32,
) -> bool {
    let ch = chars.get(chid);
    let prohibited_direction = prohibited_direction + 1;
    if cmd != prohibited_direction {
        return false;
    }

    if ch.player.short_descr.starts_with("King Welmar") {
        return false;
    }

    if ch.in_room() != db.real_room(in_room) {
        return false;
    }

    if !member_of_staff(&db, ch) {
        act(descs, chars, 
            db,
            "The guard roars at $n and pushes $m back.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
    }

    send_to_char(descs, 
        ch,
        "The guard roars: 'Entrance is Prohibited!', and pushes you back.\r\n",
    );
    return true;
}

/*
 * Routine to check if an object is trash...
 * Used by James the Butler and the Cleaning Lady.
 */
fn is_trash(i: &ObjData) -> bool {
    if !i.objwear_flagged(WearFlags::TAKE) {
        return false;
    }

    if i.get_obj_type() == ItemType::Drinkcon || i.get_obj_cost() <= 10 {
        return true;
    }

    false
}

/*
 * Function: fry_victim
 *
 * Finds a suitabe victim, and cast some _NASTY_ spell on him.
 * Used by King Welmar
 */
fn fry_victim(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, chid: DepotId) {
    let ch = chars.get(chid);
    if ch.points.mana < 10 {
        return;
    }
    let tchid = get_victim(chars,db, ch);
    /* Find someone suitable to fry ! */
    if tchid.is_none() {
        return;
    }
    let tchid = tchid.unwrap();
    let tch = chars.get(tchid);

    match rand_number(0, 8) {
        1 | 2 | 3 => {
            send_to_char(&mut game.descriptors, ch, "You raise your hand in a dramatical gesture.\r\n");
            act(&mut game.descriptors, chars, 
                db,
                "$n raises $s hand in a dramatical gesture.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            cast_spell(game, chars, db, texts,objs, chid, Some(tchid), None, SPELL_COLOR_SPRAY);
        }
        4 | 5 => {
            send_to_char(&mut game.descriptors, ch, "You concentrate and mumble to yourself.\r\n");
            act(&mut game.descriptors, chars, 
                db,
                "$n concentrates, and mumbles to $mself.",
                true,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            cast_spell(game, chars, db, texts, objs,chid, Some(tchid), None, SPELL_HARM);
        }
        6 | 7 => {
            act(&mut game.descriptors, chars, 
                db,
                "You look deeply into the eyes of $N.",
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(tch)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n looks deeply into the eyes of $N.",
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(tch)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You see an ill-boding flame in the eye of $n.",
                true,
                Some(ch),
                None,
                Some(VictimRef::Char(tch)),
                TO_VICT,
            );
            cast_spell(game, chars, db, texts, objs,chid, Some(tchid), None, SPELL_FIREBALL);
        }
        _ => {
            if !rand_number(0, 1) == 0 {
                cast_spell(game, chars, db, texts,objs, chid, Some(chid), None, SPELL_HEAL);
            }
        }
    }

    chars.get_mut(chid).points.mana -= 10;

    return;
}

pub struct KingWelmar {
    pub path: &'static [u8],
    pub path_index: usize,
    pub move_: bool,
}

impl KingWelmar {
    pub fn new() -> KingWelmar {
        KingWelmar {
            path: BEDROOM_PATH,
            path_index: 0,
            move_: false,
        }
    }
}

const BEDROOM_PATH: &[u8; 12] = b"s33004o1c1S.";
const THRONE_PATH: &[u8; 14] = b"W3o3cG52211rg.";
const MONOLOG_PATH: &[u8; 9] = b"ABCDPPPP.";

const MONOLOG: [&str; 4] = [
    "$n proclaims 'Primus in regnis Geticis coronam'.",
    "$n proclaims 'regiam gessi, subiique regis'.",
    "$n proclaims 'munus et mores colui sereno'.",
    "$n proclaims 'principe dignos'.",
];

/*
 * Function: king_welmar
 *
 * Control the actions and movements of the King.
 * Used by King Welmar.
 */
pub fn king_welmar(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);
    if !db.king_welmar.move_ {
        if db.time_info.hours == 8 && ch.in_room() == castle_real_room(&db, 51) {
            db.king_welmar.move_ = true;
            db.king_welmar.path = THRONE_PATH;
            db.king_welmar.path_index = 0;
        } else if db.time_info.hours == 21 && ch.in_room() == castle_real_room(&db, 17) {
            db.king_welmar.move_ = true;
            db.king_welmar.path = BEDROOM_PATH;
            db.king_welmar.path_index = 0;
        } else if db.time_info.hours == 12 && ch.in_room() == castle_real_room(&db, 17) {
            db.king_welmar.move_ = true;
            db.king_welmar.path = MONOLOG_PATH;
            db.king_welmar.path_index = 0;
        }
    }
    let ch = chars.get(chid);
    if cmd != 0
        || ch.get_pos() < POS_SLEEPING
        || (ch.get_pos() == POS_SLEEPING && !db.king_welmar.move_)
    {
        return false;
    }

    if ch.get_pos() == POS_FIGHTING {
        fry_victim(game, chars, db, texts, objs,chid);
        return false;
    } else if banzaii(game, chars, db, texts, objs,chid) {
        return false;
    }

    if !db.king_welmar.move_ {
        return false;
    }
    let ch = chars.get(chid);
    match db.king_welmar.path[db.king_welmar.path_index] as char {
        '0' | '1' | '2' | '3' | '4' | '5' => {
            perform_move(
                game,
                db,chars, texts,objs,
                chid,
                (db.king_welmar.path[db.king_welmar.path_index] - b'0') as i32,
                true,
            );
        }

        'A' | 'B' | 'C' | 'D' => {
            act(&mut game.descriptors, chars, 
                db,
                MONOLOG[(db.king_welmar.path[db.king_welmar.path_index] - b'A') as usize],
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'P' => {}

        'W' => {
            let ch = chars.get_mut(chid);
            ch.set_pos(POS_STANDING);
            let ch = chars.get(chid);
            act(&mut game.descriptors, chars, 
                db,
                "$n awakens and stands up.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'S' => {
            let ch = chars.get_mut(chid);
            ch.set_pos(POS_SLEEPING);
            let ch = chars.get(chid);
            act(&mut game.descriptors, chars, 
                db,
                "$n lies down on $s beautiful bed and instantly falls asleep.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'r' => {
            let ch = chars.get_mut(chid);
            ch.set_pos(POS_SITTING);
            let ch = chars.get(chid);
            act(&mut game.descriptors, chars, 
                db,
                "$n sits down on $s great throne.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        's' => {
            let ch = chars.get_mut(chid);
            ch.set_pos(POS_STANDING);
            let ch = chars.get(chid);
            act(&mut game.descriptors, chars, db, "$n stands up.", false, Some(ch), None, None, TO_ROOM);
        }

        'G' => {
            act(&mut game.descriptors, chars, 
                db,
                "$n says 'Good morning, trusted friends.'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'g' => {
            act(&mut game.descriptors, chars, 
                db,
                "$n says 'Good morning, dear subjects.'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }

        'o' => {
            do_gen_door(game, db,chars, texts, objs,chid, "door", 0, SCMD_UNLOCK); /* strcpy: OK */
            do_gen_door(game, db, chars, texts,objs,chid, "door", 0, SCMD_OPEN); /* strcpy: OK */
        }

        'c' => {
            do_gen_door(game, db, chars, texts,objs,chid, "door", 0, SCMD_CLOSE); /* strcpy: OK */
            do_gen_door(game, db, chars, texts,objs,chid, "door", 0, SCMD_LOCK); /* strcpy: OK */
        }

        '.' => {
            db.king_welmar.move_ = false;
        }
        _ => {}
    }

    db.king_welmar.path_index += 1;
    false
}

/*
 * Function: training_master
 *
 * Acts actions to the training room, if his students are present.
 * Also allowes warrior-class to practice.
 * Used by the Training Master.
 */
pub fn training_master(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);
    if !ch.awake() || ch.get_pos() == POS_FIGHTING {
        return false;
    }

    if cmd != 0 {
        return false;
    }

    if banzaii(game, chars, db, texts, objs,chid) || rand_number(0, 2) != 0 {
        return false;
    }

    let db = &db;
    let ch = chars.get(chid);
    let pupil1 = find_npc_by_name(chars, db, ch, "Brian");
    if pupil1.is_none() {
        return false;
    }
    let mut pupil1_id = pupil1.unwrap();
    let pupil2 = find_npc_by_name(chars, db, ch, "Mick");
    if pupil2.is_none() {
        return false;
    }
    let mut pupil2_id = pupil2.unwrap();
    if chars.get(pupil1_id).fighting_id().is_some() || chars.get(pupil2_id).fighting_id().is_some() {
        return false;
    }
    if rand_number(0, 1) != 0 {
        let tch = pupil1_id;
        pupil1_id = pupil2_id;
        pupil2_id = tch;
    }

    let pupil1 = chars.get(pupil1_id);
    let pupil2 = chars.get(pupil2_id);

    match rand_number(0, 7) {
        0 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n hits $N on $s head with a powerful blow.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You hit $N on $s head with a powerful blow.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n hits you on your head with a powerful blow.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_VICT,
            );
        }

        1 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n hits $N in $s chest with a thrust.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You manage to thrust $N in the chest.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n manages to thrust you in your chest.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_VICT,
            );
        }

        2 => {
            send_to_char(&mut game.descriptors, ch, "You command your pupils to bow.\r\n");
            act(&mut game.descriptors, chars, 
                db,
                "$n commands $s pupils to bow.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n bows before $N.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$N bows before $n.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You bow before $N, who returns your gesture.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You bow before $n, who returns your gesture.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_VICT,
            );
        }

        3 => {
            act(&mut game.descriptors, chars, 
                db,
                "$N yells at $n, as he fumbles and drops $s sword.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(ch)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n quickly picks up $s weapon.",
                false,
                Some(pupil1),
                None,
                None,
                TO_ROOM,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$N yells at you, as you fumble, losing your weapon.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(ch)),
                TO_CHAR,
            );
            send_to_char(&mut game.descriptors, 
                chars.get(pupil1_id),
                "You quickly pick up your weapon again.\r\n",
            );
            act(&mut game.descriptors, chars, 
                db,
                "You yell at $n, as he fumbles, losing $s weapon.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(ch)),
                TO_VICT,
            );
        }

        4 => {
            act(&mut game.descriptors, chars, 
                db,
                "$N tricks $n, and slashes him across the back.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$N tricks you, and slashes you across your back.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You trick $n, and quickly slash him across $s back.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_VICT,
            );
        }

        5 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n lunges a blow at $N but $N parries skillfully.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You lunge a blow at $N but $E parries skillfully.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n lunges a blow at you, but you skillfully parry it.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_VICT,
            );
        }

        6 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n clumsily tries to kick $N, but misses.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You clumsily miss $N with your poor excuse for a kick.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n fails an unusually clumsy attempt at kicking you.",
                false,
                Some(pupil1),
                None,
                Some(VictimRef::Char(pupil2)),
                TO_VICT,
            );
        }

        _ => {
            send_to_char(&mut game.descriptors, ch, "You show your pupils an advanced technique.\r\n");
            act(&mut game.descriptors, chars, 
                db,
                "$n shows $s pupils an advanced technique.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }
    }

    false
}

pub fn tom(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    argument: &str,
) -> bool {
    return castle_twin_proc(game, chars, db, texts, objs,chid, cmd, argument, 48, "Tim");
}

pub fn tim(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    argument: &str,
) -> bool {
    return castle_twin_proc(game, chars, db, texts, objs, chid, cmd, argument, 49, "Tom");
}

/*
 * Common routine for the Castle Twins.
 */
fn castle_twin_proc(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    cmd: i32,
    arg: &str,
    ctlnum: MobVnum,
    twinname: &str,
) -> bool {
    let ch = chars.get(chid);
    if !ch.awake() {
        return false;
    }

    if cmd != 0 {
        return block_way(&mut game.descriptors, chars, db, chid, cmd, arg, castle_virtual(&db, ctlnum), 1);
    }

    let king_id = find_npc_by_name(chars, &db, ch, "King Welmar");

    if king_id.is_some() {
        let king_id = king_id.unwrap();
        if ch.master.is_none() {
            do_follow(game, db, chars, texts,objs,chid, "King Welmar", 0, 0); /* strcpy: OK */
            if chars.get(king_id).fighting_id().is_some() {
                do_npc_rescue(game, chars, db, objs, chid, king_id);
            }
        }
    }
    let ch = chars.get(chid);
    let twin_id = find_npc_by_name(chars, &db, ch, twinname);
    if twin_id.is_some() {
        let twin_id = twin_id.unwrap();
        let twin = chars.get(twin_id);
        if twin.fighting_id().is_some() && 2 & twin.get_hit() < ch.get_hit() {
            do_npc_rescue(game, chars,db, objs,chid, twin_id);
        }
    }
    let ch = chars.get(chid);
    if ch.get_pos() != POS_FIGHTING {
        banzaii(game, chars, db, texts, objs,chid);
    }

    false
}

/*
 * Routine for James the Butler.
 * Complains if he finds any trash...
 *
 * This doesn't make sure he _can_ carry it...
 */
fn james(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB,_texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    return castle_cleaner(game, chars, db, objs,chid, cmd, true);
}

/*
 * Common code for James and the Cleaning Woman.
 */
fn castle_cleaner(game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId, cmd: i32, gripe: bool) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || !ch.awake() || ch.get_pos() == POS_FIGHTING {
        return false;
    }

    for i_id in db.world[ch.in_room() as usize].contents.clone() {
        if !is_trash(objs.get(i_id)) {
            continue;
        }

        if gripe {
            act(&mut game.descriptors, chars, 
                db,
                "$n says: 'My oh my!  I ought to fire that lazy cleaning woman!'",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n picks up a piece of trash.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }
        let i = objs.get_mut(i_id);
        let ch = chars.get_mut(chid);
        db.obj_from_room(i);
        obj_to_char(i,  ch);
        return true;
    }

    false
}

/*
 * Routine for the Cleaning Woman.
 * Picks up any trash she finds...
 */
fn cleaning(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB,_texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    return castle_cleaner(game, chars,db, objs,chid, cmd, false);
}

/*
 * Routine: CastleGuard
 *
 * Standard routine for ordinary castle guards.
 */
fn castle_guard(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || !ch.awake() || (ch.get_pos() == POS_FIGHTING) {
        return false;
    }

    banzaii(game, chars, db, texts,objs, chid)
}

/*
 * Routine: DicknDave
 *
 * Routine for the guards Dick and David.
 */
fn dick_n_david(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if !ch.awake() {
        return false;
    }

    if cmd == 0 && ch.get_pos() != POS_FIGHTING {
        banzaii(game, chars, db, texts, objs,chid);
    }

    block_way(&mut game.descriptors, chars, db, chid, cmd, argument, castle_virtual(&db, 36), 1)
}

/*
 * Routine: peter
 * Routine for Captain of the Guards.
 */
fn peter(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB,texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if cmd != 0 || !ch.awake() || ch.get_pos() == POS_FIGHTING {
        return false;
    }

    if banzaii(game, chars, db, texts, objs,chid) {
        return false;
    }
    let db = &db;
    let ch = chars.get(chid);
    let ch_guard = find_guard(chars, db, ch);
    if rand_number(0, 3) == 0 && ch_guard.is_some() {
        let chid_guard = ch_guard.unwrap();
        let ch_guard = chars.get(chid_guard);
        match rand_number(0, 5) {
            0 => {
                act(&mut game.descriptors, chars, 
                    db,
                    "$N comes sharply into attention as $n inspects $M.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_NOTVICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$N comes sharply into attention as you inspect $M.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_CHAR,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "You go sharply into attention as $n inspects you.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_VICT,
                );
            }
            1 => {
                act(&mut game.descriptors, chars, 
                    db,
                    "$N looks very small, as $n roars at $M.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_NOTVICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$N looks very small as you roar at $M.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_CHAR,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "You feel very small as $N roars at you.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_VICT,
                );
            }
            2 => {
                act(&mut game.descriptors, chars, 
                    db,
                    "$n gives $N some Royal directions.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_NOTVICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "You give $N some Royal directions.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_CHAR,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$n gives you some Royal directions.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_VICT,
                );
            }
            3 => {
                act(&mut game.descriptors, chars, 
                    db,
                    "$n looks at you.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_VICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$n looks at $N.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_NOTVICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$n growls: 'Those boots need polishing!'",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_ROOM,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "You growl at $N.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_CHAR,
                );
            }
            4 => {
                act(&mut game.descriptors, chars, 
                    db,
                    "$n looks at you.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_VICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$n looks at $N.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_NOTVICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$n growls: 'Straighten that collar!'",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_ROOM,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "You growl at $N.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_CHAR,
                );
            }
            _ => {
                act(&mut game.descriptors, chars, 
                    db,
                    "$n looks at you.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_VICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$n looks at $N.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_NOTVICT,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "$n growls: 'That chain mail looks rusty!  CLEAN IT !!!'",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_ROOM,
                );
                act(&mut game.descriptors, chars, 
                    db,
                    "You growl at $N.",
                    false,
                    Some(ch),
                    None,
                    Some(VictimRef::Char(ch_guard)),
                    TO_CHAR,
                );
            }
        }
    }

    false
}

/*
 * Procedure for Jerry and Michael in x08 of King's Castle.
 * Code by Sapowox modified by Pjotr.(Original code from Master)
 */
fn jerry(
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, 
    chid: DepotId,
    _me: MeRef,
    cmd: i32,
    _argument: &str,
) -> bool {
    let ch = chars.get(chid);

    if !ch.awake() || ch.get_pos() == POS_FIGHTING {
        return false;
    }
    if cmd != 0 {
        return false;
    }

    if banzaii(game, chars, db, texts, objs, chid) || rand_number(0, 2) != 0 {
        return false;
    }
    let db = &db;

    let gambler1_id = chid;
    let ch = chars.get(chid);
    let gambler2_id = find_npc_by_name(chars, db, ch, "Michael");

    if gambler2_id.is_none() {
        return false;
    }
    let mut gambler1 = chars.get(gambler1_id);
    let mut gambler2 = chars.get(gambler2_id.unwrap());

    if gambler1.fighting_id().is_some() || gambler2.fighting_id().is_some() {
        return false;
    }
    let tch;
    if rand_number(0, 1) != 0 {
        tch = gambler1;
        gambler1 = gambler2;
        gambler2 = tch;
    }

    match rand_number(0, 5) {
        0 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n rolls the dice and cheers loudly at the result.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You roll the dice and cheer. GREAT!",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n cheers loudly as $e rolls the dice.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_VICT,
            );
        }
        1 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n curses the Goddess of Luck roundly as he sees $N's roll.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You curse the Goddess of Luck as $N rolls.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n swears angrily. You are in luck!",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_VICT,
            );
        }
        2 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n sighs loudly and gives $N some gold.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You sigh loudly at the pain of having to give $N some gold.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n sighs loudly as $e gives you your rightful win.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_VICT,
            );
        }
        3 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n smiles remorsefully as $N's roll tops $s.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You smile sadly as you see that $N beats you. Again.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n smiles remorsefully as your roll tops $s.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_VICT,
            );
        }
        4 => {
            act(&mut game.descriptors, chars, 
                db,
                "$n excitedly follows the dice with $s eyes.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You excitedly follow the dice with your eyes.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n excitedly follows the dice with $s eyes.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_VICT,
            );
        }
        _ => {
            act(&mut game.descriptors, chars, 
                db,
                "$n says 'Well, my luck has to change soon', as he shakes the dice.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_NOTVICT,
            );
            act(&mut game.descriptors, chars, 
                db,
                "You say 'Well, my luck has to change soon' and shake the dice.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_CHAR,
            );
            act(&mut game.descriptors, chars, 
                db,
                "$n says 'Well, my luck has to change soon', as he shakes the dice.",
                false,
                Some(gambler1),
                None,
                Some(VictimRef::Char(gambler2)),
                TO_VICT,
            );
        }
    }
    false
}
