/* ************************************************************************
*   File: graph.rs                                      Part of CircleMUD *
*  Usage: various graph algorithms                                        *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      * 
************************************************************************ */

use log::error;

use crate::constants::DIRS;
use crate::db::DB;
use crate::depot::DepotId;
use crate::handler::FIND_CHAR_WORLD;
use crate::interpreter::one_argument;
use crate::spells::SKILL_TRACK;
use crate::structs::{
     RoomRnum, AFF_NOTRACK, EX_CLOSED, NOWHERE, NUM_OF_DIRS, ROOM_BFS_MARK, ROOM_NOTRACK,
};
use crate::util::{hmhr, rand_number, BFS_ALREADY_THERE, BFS_ERROR, BFS_NO_PATH};
use crate::Game;

struct BfsQueueStruct {
    room: RoomRnum,
    dir: usize,
}

/* Utility functions */
fn mark(db: &mut DB, room: RoomRnum) {
    db.set_room_flags_bit(room, ROOM_BFS_MARK);
}

fn unmark(db: &mut DB, room: RoomRnum) {
    db.remove_room_flags_bit(room, ROOM_BFS_MARK);
}

fn is_marked(db: &DB, room: RoomRnum) -> bool {
    db.room_flagged(room, ROOM_BFS_MARK)
}

fn toroom(db: &DB, x: RoomRnum, y: usize) -> RoomRnum {
    db.world[x as usize].dir_option[y]
        .as_ref()
        .unwrap()
        .to_room
}

fn is_closed(db: &DB, x: RoomRnum, y: usize) -> bool {
    db.world[x as usize].dir_option[y]
        .as_ref()
        .unwrap()
        .exit_info
        & EX_CLOSED
        != 0
}

fn valid_edge(game: &mut Game, db: &DB, x: RoomRnum, y: usize) -> bool {
    if db.world[x as usize].dir_option[y].is_none() || toroom(db, x, y) == NOWHERE
    {
        return false;
    }
    if !game.config.track_through_doors && is_closed(db, x, y) {
        return false;
    }
    if db.room_flagged(toroom(db, x, y), ROOM_NOTRACK) || is_marked(db, toroom(db, x, y)) {
        return false;
    }

    true
}

struct BfsTracker {
    queue: Vec<BfsQueueStruct>,
}

impl BfsTracker {
    fn new() -> BfsTracker {
        BfsTracker { queue: vec![] }
    }
    fn bfs_enqueue(&mut self, room: RoomRnum, dir: usize) {
        self.queue.push(BfsQueueStruct { room, dir });
    }
    fn bfs_dequeue(&mut self) {
        self.queue.remove(0);
    }
}

/*
 * find_first_step: given a source room and a target room, find the first
 * step on the shortest path from the source to the target.
 *
 * Intended usage: in mobile_activity, give a mob a dir to go if they're
 * tracking another mob or a PC.  Or, a 'track' skill for PCs.
 */
fn find_first_step(game: &mut Game, db: &mut DB, src: RoomRnum, target: RoomRnum) -> i32 {
    if src == NOWHERE
        || target == NOWHERE
        || src >= db.world.len() as i16
        || target > db.world.len() as i16
    {
        error!(
            "SYSERR: Illegal value {} or {} passed to find_first_step.",
            src, target
        );
        return BFS_ERROR;
    }
    if src == target {
        return BFS_ALREADY_THERE;
    }

    /* clear marks first, some OLC systems will save the mark. */
    for curr_room in 0..db.world.len() {
        unmark( db, curr_room as RoomRnum);
    }

    mark( db, src);

    /* first, enqueue the first steps, saving which direction we're going. */

    let mut tracker = BfsTracker::new();

    for curr_dir in 0..NUM_OF_DIRS {
        if valid_edge(game, db, src, curr_dir) {
            let room_nr = toroom(&db, src, curr_dir);
            mark(db, room_nr);
            tracker.bfs_enqueue(toroom(&db, src, curr_dir), curr_dir);
        }
    }

    /* now, do the classic BFS. */
    while !tracker.queue.is_empty() {
        if tracker.queue[0].room == target {
            let curr_dir = tracker.queue[0].dir;
            return curr_dir as i32;
        } else {
            for curr_dir in 0..NUM_OF_DIRS {
                if valid_edge(game, db, tracker.queue[0].room, curr_dir) {
                    let room_nr = toroom(&db, tracker.queue[0].room, curr_dir);
                    mark(db, room_nr);
                    tracker.bfs_enqueue(
                        toroom(&db, tracker.queue[0].room, curr_dir),
                        tracker.queue[0].dir,
                    );
                }
            }
            tracker.bfs_dequeue();
        }
    }

    return BFS_NO_PATH;
}

/********************************************************
* Functions and Commands which use the above functions. *
********************************************************/

pub fn do_track(game: &mut Game, db: &mut DB, chid: DepotId, argument: &str, _cmd: usize, _subcmd: i32) {
    let ch = db.ch(chid);
    /* The character must have the track skill. */
    if ch.is_npc() || ch.get_skill(SKILL_TRACK) == 0 {
        game.send_to_char(db,chid, "You have no idea how.\r\n");
        return;
    }
    let mut arg = String::new();
    one_argument(argument, &mut arg);
    if arg.is_empty() {
        game.send_to_char(db,chid, "Whom are you trying to track?\r\n");
        return;
    }
    let vict_id;
    /* The person can't see the victim. */
    if {
        vict_id = game.get_char_vis(db, chid, &mut arg, None, FIND_CHAR_WORLD);
        vict_id.is_none()
    } {
        game.send_to_char(db,chid, "No one is around by that name.\r\n");
        return;
    }
    let vict_id = vict_id.unwrap();
    let vict = db.ch(vict_id);
    /* We can't track the victim. */
    if vict.aff_flagged(AFF_NOTRACK) {
        game.send_to_char(db,chid, "You sense no trail.\r\n");
        return;
    }

    /* 101 is a complete failure, no matter what the proficiency. */
    if rand_number(0, 101) >= ch.get_skill(SKILL_TRACK) as u32 {
        let mut tries = 10;
        /* Find a random direction. :) */
        let mut dir;
        loop {
            tries -= 1;
            dir = rand_number(0, (NUM_OF_DIRS - 1) as u32) as usize;
            if db.can_go(ch, dir) || tries == 0 {
                break;
            }
        }
        game.send_to_char(db,
            chid,
            format!("You sense a trail {} from here!\r\n", DIRS[dir]).as_ref(),
        );
        return;
    }

    /* They passed the skill check. */
    let dir = find_first_step(game, db, ch.in_room(), vict.in_room());

    match dir {
        BFS_ERROR => {
            game.send_to_char(db,chid, "Hmm.. something seems to be wrong.\r\n");
        }

        BFS_ALREADY_THERE => {
            game.send_to_char(db,chid, "You're already in the same room!!\r\n");
        }

        BFS_NO_PATH => {
            let vict = db.ch(vict_id);
            game.send_to_char(db,
                chid,
                format!("You can't sense a trail to {} from here.\r\n", hmhr(vict)).as_str(),
            );
        }
        _ => {
            game.send_to_char(db,
                chid,
                format!("You sense a trail {} from here!\r\n", DIRS[dir as usize]).as_str(),
            );
        }
    }
}

// void hunt_victim(struct char_data *ch)
// {
// int dir;
// byte found;
// struct char_data *tmp;
//
// if (!ch || !HUNTING(ch) || FIGHTING(ch))
// return;
//
// /* make sure the char still exists */
// for (found = FALSE, tmp = character_list; tmp && !found; tmp = tmp->next)
// if (HUNTING(ch) == tmp)
// found = TRUE;
//
// if (!found) {
// char actbuf[MAX_INPUT_LENGTH] = "Damn!  My prey is gone!!";
//
// do_say(ch, actbuf, 0, 0);
// HUNTING(ch) = None;
// return;
// }
// if ((dir = find_first_step(IN_ROOM(ch), IN_ROOM(HUNTING(ch)))) < 0) {
// char buf[MAX_INPUT_LENGTH];
//
// snprintf(buf, sizeof(buf), "Damn!  I lost {}!", HMHR(HUNTING(ch)));
// do_say(ch, buf, 0, 0);
// HUNTING(ch) = None;
// } else {
// perform_move(ch, dir, 1);
// if (IN_ROOM(ch) == IN_ROOM(HUNTING(ch)))
// hit(ch, HUNTING(ch), TYPE_UNDEFINED);
// }
// }
