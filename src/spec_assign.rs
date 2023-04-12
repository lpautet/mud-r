/* ************************************************************************
*   File: spec_assign.c                                 Part of CircleMUD *
*  Usage: Functions to assign function pointers to objs/mobs/rooms        *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

// /* external functions */
// SPECIAL(dump);
// SPECIAL(pet_shops);
// SPECIAL(postmaster);
// SPECIAL(cityguard);
// SPECIAL(receptionist);
// SPECIAL(cryogenicist);
// SPECIAL(guild_guard);
// SPECIAL(guild);
// SPECIAL(puff);
// SPECIAL(fido);
// SPECIAL(janitor);
// SPECIAL(mayor);
// SPECIAL(snake);
// SPECIAL(thief);
// SPECIAL(magic_user);
// SPECIAL(bank);
// SPECIAL(gen_board);
// void assign_kings_castle(void);
//
// /* local functions */
// void assign_mobiles(void);
// void assign_objects(void);
// void assign_rooms(void);
// void ASSIGNROOM(room_vnum room, SPECIAL(fname));
// void assignmob(db, mob_vnum mob, SPECIAL(fname));
// void ASSIGNOBJ(obj_vnum obj, SPECIAL(fname));

/* functions to perform assignments */

use crate::db::DB;
use crate::spec_procs::guild;
use crate::spec_procs::guild_guard;
use crate::structs::{MobVnum, Special, NOBODY};
use log::error;

fn assignmob(db: &mut DB, mob: MobVnum, fname: Special) {
    let rnum = db.real_mobile(mob);
    if rnum != NOBODY {
        db.mob_index[rnum as usize].func = Some(fname);
    } else if !db.mini_mud {
        error!(
            "SYSERR: Attempt to assign spec to non-existant mob #{}",
            mob
        );
    }
}

// void ASSIGNOBJ(obj_vnum obj, SPECIAL(fname))
// {
// obj_rnum rnum;
//
// if ((rnum = real_object(obj)) != NOTHING)
// obj_index[rnum].func = fname;
// else if (!mini_mud)
// log("SYSERR: Attempt to assign spec to non-existant obj #%d", obj);
// }
//
// void ASSIGNROOM(room_vnum room, SPECIAL(fname))
// {
// room_rnum rnum;
//
// if ((rnum = real_room(room)) != NOWHERE)
// world[rnum].func = fname;
// else if (!mini_mud)
// log("SYSERR: Attempt to assign spec to non-existant room #%d", room);
// }

/* ********************************************************************
*  Assignments                                                        *
******************************************************************** */

/* assign special procedures to mobiles */
pub fn assign_mobiles(db: &mut DB) {
    // assign_kings_castle();
    //
    // assignmob(db, 1, puff);
    //
    /* Immortal Zone */
    // assignmob(db, 1200, receptionist);
    // assignmob(db, 1201, postmaster);
    // assignmob(db, 1202, janitor);
    //
    /* Midgaard */
    // assignmob(db, 3005, receptionist);
    // assignmob(db, 3010, postmaster);
    assignmob(db, 3020, guild);
    assignmob(db, 3021, guild);
    assignmob(db, 3022, guild);
    assignmob(db, 3023, guild);
    assignmob(db, 3024, guild_guard);
    assignmob(db, 3025, guild_guard);
    assignmob(db, 3026, guild_guard);
    assignmob(db, 3027, guild_guard);
    // assignmob(db, 3059, cityguard);
    // assignmob(db, 3060, cityguard);
    // assignmob(db, 3061, janitor);
    // assignmob(db, 3062, fido);
    // assignmob(db, 3066, fido);
    // assignmob(db, 3067, cityguard);
    // assignmob(db, 3068, janitor);
    // assignmob(db, 3095, cryogenicist);
    // assignmob(db, 3105, mayor);

    /* MORIA */
    // assignmob(db, 4000, snake);
    // assignmob(db, 4001, snake);
    // assignmob(db, 4053, snake);
    // assignmob(db, 4100, magic_user);
    // assignmob(db, 4102, snake);
    // assignmob(db, 4103, thief);

    /* Redferne's */
    // assignmob(db, 7900, cityguard);

    /* PYRAMID */
    // assignmob(db, 5300, snake);
    // assignmob(db, 5301, snake);
    // assignmob(db, 5304, thief);
    // assignmob(db, 5305, thief);
    // assignmob(db, 5309, magic_user); /* should breath fire */
    // assignmob(db, 5311, magic_user);
    // assignmob(db, 5313, magic_user); /* should be a cleric */
    // assignmob(db, 5314, magic_user); /* should be a cleric */
    // assignmob(db, 5315, magic_user); /* should be a cleric */
    // assignmob(db, 5316, magic_user); /* should be a cleric */
    // assignmob(db, 5317, magic_user);

    /* High Tower Of Sorcery */
    // assignmob(db, 2501, magic_user); /* should likely be cleric */
    // assignmob(db, 2504, magic_user);
    // assignmob(db, 2507, magic_user);
    // assignmob(db, 2508, magic_user);
    // assignmob(db, 2510, magic_user);
    // assignmob(db, 2511, thief);
    // assignmob(db, 2514, magic_user);
    // assignmob(db, 2515, magic_user);
    // assignmob(db, 2516, magic_user);
    // assignmob(db, 2517, magic_user);
    // assignmob(db, 2518, magic_user);
    // assignmob(db, 2520, magic_user);
    // assignmob(db, 2521, magic_user);
    // assignmob(db, 2522, magic_user);
    // assignmob(db, 2523, magic_user);
    // assignmob(db, 2524, magic_user);
    // assignmob(db, 2525, magic_user);
    // assignmob(db, 2526, magic_user);
    // assignmob(db, 2527, magic_user);
    // assignmob(db, 2528, magic_user);
    // assignmob(db, 2529, magic_user);
    // assignmob(db, 2530, magic_user);
    // assignmob(db, 2531, magic_user);
    // assignmob(db, 2532, magic_user);
    // assignmob(db, 2533, magic_user);
    // assignmob(db, 2534, magic_user);
    // assignmob(db, 2536, magic_user);
    // assignmob(db, 2537, magic_user);
    // assignmob(db, 2538, magic_user);
    // assignmob(db, 2540, magic_user);
    // assignmob(db, 2541, magic_user);
    // assignmob(db, 2548, magic_user);
    // assignmob(db, 2549, magic_user);
    // assignmob(db, 2552, magic_user);
    // assignmob(db, 2553, magic_user);
    // assignmob(db, 2554, magic_user);
    // assignmob(db, 2556, magic_user);
    // assignmob(db, 2557, magic_user);
    // assignmob(db, 2559, magic_user);
    // assignmob(db, 2560, magic_user);
    // assignmob(db, 2562, magic_user);
    // assignmob(db, 2564, magic_user);
    //
    /* SEWERS */
    // assignmob(db, 7006, snake);
    // assignmob(db, 7009, magic_user);
    // assignmob(db, 7200, magic_user);
    // assignmob(db, 7201, magic_user);
    // assignmob(db, 7202, magic_user);
    //
    /* FOREST */
    // assignmob(db, 6112, magic_user);
    // assignmob(db, 6113, snake);
    // assignmob(db, 6114, magic_user);
    // assignmob(db, 6115, magic_user);
    // assignmob(db, 6116, magic_user); /* should be a cleric */
    // assignmob(db, 6117, magic_user);
    //
    /* ARACHNOS */
    // assignmob(db, 6302, magic_user);
    // assignmob(db, 6309, magic_user);
    // assignmob(db, 6312, magic_user);
    // assignmob(db, 6314, magic_user);
    // assignmob(db, 6315, magic_user);
    //
    /* Desert */
    // assignmob(db, 5004, magic_user);
    // assignmob(db, 5005, guild_guard); /* brass dragon */
    // assignmob(db, 5010, magic_user);
    // assignmob(db, 5014, magic_user);
    //
    /* Drow City */
    // assignmob(db, 5103, magic_user);
    // assignmob(db, 5104, magic_user);
    // assignmob(db, 5107, magic_user);
    // assignmob(db, 5108, magic_user);
    //
    /* Old Thalos */
    // assignmob(db, 5200, magic_user);
    // assignmob(db, 5201, magic_user);
    // assignmob(db, 5209, magic_user);
    //
    /* New Thalos */
    /* 5481 - Cleric (or Mage... but he IS a high priest... *shrug*) */
    // assignmob(db, 5404, receptionist);
    // assignmob(db, 5421, magic_user);
    // assignmob(db, 5422, magic_user);
    // assignmob(db, 5423, magic_user);
    // assignmob(db, 5424, magic_user);
    // assignmob(db, 5425, magic_user);
    // assignmob(db, 5426, magic_user);
    // assignmob(db, 5427, magic_user);
    // assignmob(db, 5428, magic_user);
    // assignmob(db, 5434, cityguard);
    // assignmob(db, 5440, magic_user);
    // assignmob(db, 5455, magic_user);
    // assignmob(db, 5461, cityguard);
    // assignmob(db, 5462, cityguard);
    // assignmob(db, 5463, cityguard);
    // assignmob(db, 5482, cityguard);
    // /*
    // 5400 - Guildmaster (Mage)
    // 5401 - Guildmaster (Cleric)
    // 5402 - Guildmaster (Warrior)
    // 5403 - Guildmaster (Thief)
    // 5456 - Guildguard (Mage)
    // 5457 - Guildguard (Cleric)
    // 5458 - Guildguard (Warrior)
    // 5459 - Guildguard (Thief)
    // */
    //
    /* ROME */
    // assignmob(db, 12009, magic_user);
    // assignmob(db, 12018, cityguard);
    // assignmob(db, 12020, magic_user);
    // assignmob(db, 12021, cityguard);
    // assignmob(db, 12025, magic_user);
    // assignmob(db, 12030, magic_user);
    // assignmob(db, 12031, magic_user);
    // assignmob(db, 12032, magic_user);
    //
    /* King Welmar's Castle (not covered in castle.c) */
    // assignmob(db, 15015, thief);      /* Ergan... have a better idea? */
    // assignmob(db, 15032, magic_user); /* Pit Fiend, have something better?  Use it */
    //
    /* DWARVEN KINGDOM */
    // assignmob(db, 6500, cityguard);
    // assignmob(db, 6502, magic_user);
    // assignmob(db, 6509, magic_user);
    // assignmob(db, 6516, magic_user);
}

// /* assign special procedures to objects */
// void assign_objects(void)
// {
// ASSIGNOBJ(3096, gen_board);	/* social board */
// ASSIGNOBJ(3097, gen_board);	/* freeze board */
// ASSIGNOBJ(3098, gen_board);	/* immortal board */
// ASSIGNOBJ(3099, gen_board);	/* mortal board */
//
// ASSIGNOBJ(3034, bank);	/* atm */
// ASSIGNOBJ(3036, bank);	/* cashcard */
// }
//
//
//
// /* assign special procedures to rooms */
// void assign_rooms(void)
// {
// room_rnum i;
//
// ASSIGNROOM(3030, dump);
// ASSIGNROOM(3031, pet_shops);
//
// if (dts_are_dumps)
// for (i = 0; i <= top_of_world; i++)
// if (ROOM_FLAGGED(i, ROOM_DEATH))
// world[i].func = dump;
// }
