/* ************************************************************************
*   File: db.c                                          Part of CircleMUD *
*  Usage: Loading/saving chars, booting/resetting world, internal funcs   *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */
use regex::Regex;

use crate::modify::paginate_string;
use crate::structs::{
    obj_vnum, room_rnum, room_vnum, zone_rnum, zone_vnum, AffectedType, CharAbilityData, CharData,
    CharFileU, CharPointData, CharSpecialDataSaved, ExtraDescrData, PlayerSpecialDataSaved,
    RoomData, RoomDirectionData, AFF_POISON, EX_ISDOOR, EX_PICKPROOF, HOST_LENGTH, LVL_IMPL,
    MAX_AFFECT, MAX_NAME_LENGTH, MAX_PWD_LENGTH, MAX_SKILLS, MAX_TITLE_LENGTH, MAX_TONGUE, NOBODY,
    NOWHERE, NUM_OF_DIRS, POS_STANDING, SEX_MALE,
};
use crate::util::{get_line, prune_crlf, rand_number, time_now, touch, SECS_PER_REAL_HOUR};
use crate::{check_player_special, get_last_tell_mut, MainGlobals};
use log::{error, info, warn};
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::cmp::min;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, ErrorKind, Seek, SeekFrom};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::rc::Rc;
use std::{fs, io, mem, process, slice};

pub const KILLSCRIPT: &str = "./.killscript";
const BACKGROUND_FILE: &str = "text/background";
const GREETINGS_FILE: &str = "text/greetings";
const IMOTD_FILE: &str = "text/imotd";
const MOTD_FILE: &str = "text/motd";
const PLAYER_FILE: &str = "etc/players";

struct PlayerIndexElement {
    name: String,
    id: i64,
}

pub struct DB {
    //   pub globals: &'a MainGlobals,
    pub world: RefCell<Vec<Rc<RoomData>>>,
    pub top_of_world: RefCell<room_rnum>,
    /* ref to top element of world	 */
    pub character_list: RefCell<Vec<Rc<CharData>>>,
    /* global linked list of * chars	 */
    // struct index_data *mob_index;	/* index table for mobile file	 */
    // struct char_data *mob_proto;	/* prototypes for mobs		 */
    // mob_rnum top_of_mobt = 0;	/* top of mobile index table	 */
    //
    // struct obj_data *object_list = NULL;	/* global linked list of objs	 */
    // struct index_data *obj_index;	/* index table for object file	 */
    // struct obj_data *obj_proto;	/* prototypes for objs		 */
    // obj_rnum top_of_objt = 0;	/* top of object index table	 */
    zone_table: RefCell<Vec<ZoneData>>,
    /* zone table			 */
    top_of_zone_table: RefCell<zone_rnum>,
    /* top element of zone tab	 */
    // struct message_list fight_messages[MAX_MESSAGES];	/* fighting messages	 */
    //
    player_table: RefCell<Vec<PlayerIndexElement>>,
    /* index to plr file	 */
    player_fl: RefCell<Option<File>>,
    /* file desc of player file	 */
    top_of_p_table: RefCell<i32>,
    /* ref to top of table		 */
    top_idnum: RefCell<i32>,
    /* highest idnum in use		 */
    //
    // int no_mail = 0;		/* mail disabled?		 */
    // int mini_mud = 0;		/* mini-mud mode?		 */
    // int no_rent_check = 0;		/* skip rent check on boot?	 */
    // time_t boot_time = 0;		/* time of mud boot		 */
    // int circle_restrict = 0;	/* level of game restriction	 */
    pub r_mortal_start_room: RefCell<room_rnum>,
    /* rnum of mortal start room	 */
    pub r_immort_start_room: RefCell<room_rnum>,
    /* rnum of immort start room	 */
    pub r_frozen_start_room: RefCell<room_rnum>,
    /* rnum of frozen start room	 */
    //
    // char *credits = NULL;		/* game credits			 */
    // char *news = NULL;		/* mud news			 */
    pub motd: String,
    /* message of the day - mortals */
    pub imotd: String,
    /* message of the day - immorts */
    pub greetings: RefCell<String>,
    /* opening credits screen	*/
    // char *help = NULL;		/* help screen			 */
    // char *info = NULL;		/* info page			 */
    // char *wizlist = NULL;		/* list of higher gods		 */
    // char *immlist = NULL;		/* list of peon gods		 */
    pub background: String,
    /* background story		 */
    // char *handbook = NULL;		/* handbook for new immortals	 */
    // char *policies = NULL;		/* policies page		 */
    //
    // struct help_index_element *help_table = 0;	/* the help table	 */
    // int top_of_helpt = 0;		/* top of help index table	 */
    //
    // struct time_info_data time_info;/* the infomation about the time    */
    // struct weather_data weather_info;	/* the infomation about the weather */
    // struct player_special_data dummy_mob;	/* dummy spec area for mobs	*/
    // struct reset_q_type reset_q;	/* queue of zones to be reset	 */
}

const REAL: i32 = 0;
const VIRTUAL: i32 = 1;

/* structure for the reset commands */
pub struct ResetCom {
    pub command: char,
    /* current command                      */
    pub if_flag: bool,
    /* if TRUE: exe only if preceding exe'd */
    pub arg1: i32,
    /*                                      */
    pub arg2: i32,
    /* Arguments to the command             */
    pub arg3: i32,
    /*                                      */
    pub line: i32,
    /* line number this command appears on  */

    /*
     *  Commands:              *
     *  'M': Read a mobile     *
     *  'O': Read an object    *
     *  'G': Give obj to mob   *
     *  'P': Put obj in obj    *
     *  'G': Obj to char       *
     *  'E': Obj to char equip *
     *  'D': Set state of door *
    */
}

/* zone definition structure. for the 'zone-table'   */
pub struct ZoneData {
    pub name: String,
    /* name of this zone                  */
    pub lifespan: i32,
    /* how long between resets (minutes)  */
    pub age: i32,
    /* current age of this zone (minutes) */
    pub bot: room_vnum,
    /* starting room number for this zone */
    pub top: room_vnum,
    /* upper limit for rooms in this zone */
    pub reset_mode: i32,
    /* conditions for reset (see below)   */
    pub number: zone_vnum,
    /* virtual number of this zone	  */
    pub cmd: Vec<ResetCom>,
    /* command table for reset	          */

    /*
     * Reset mode:
     *   0: Don't reset, and don't update age.
     *   1: Reset if no PC's are located in zone.
     *   2: Just reset.
     */
}
// /* external functions */
// void paginate_string(char *str, struct descriptor_data *d);
// struct time_info_data *mud_time_passed(time_t t2, time_t t1);
// void free_alias(struct alias_data *a);
// void load_messages(void);
// void weather_and_time(int mode);
// void mag_assign_spells(void);
// void boot_social_messages(void);
// void update_obj_file(void);	/* In objsave.c */
// void sort_commands(void);
// void sort_spells(void);
// void load_banned(void);
// void Read_Invalid_List(void);
// void boot_the_shops(FILE *shop_f, char *filename, int rec_count);
// int hsort(const void *a, const void *b);
// void prune_crlf(char *txt);
// void destroy_shops(void);
//
// /* external vars */
// extern int no_specials;
// extern int scheck;
// extern room_vnum MORTAL_START_ROOM;
// extern room_vnum IMMORT_START_ROOM;
// extern room_vnum FROZEN_START_ROOM;
// extern struct descriptor_data *descriptor_list;
// extern const char *unused_spellname;	/* spell_parser.c */
//
// /*************************************************************************
// *  routines for booting the system                                       *
// *************************************************************************/
//
// /* this is necessary for the autowiz system */
// void reboot_wizlists(void)
// {
// file_to_string_alloc(WIZLIST_FILE, &wizlist);
// file_to_string_alloc(IMMLIST_FILE, &immlist);
// }
//
//
// /* Wipe out all the loaded text files, for shutting down. */
// void free_text_files(void)
// {
// char **textfiles[] = {
// &wizlist, &immlist, &news, &credits, &motd, &imotd, &help, &info,
// &policies, &handbook, &background, &greetings, NULL
// };
// int rf;
//
// for (rf = 0; textfiles[rf]; rf++)
// if (*textfiles[rf]) {
// free(*textfiles[rf]);
// *textfiles[rf] = NULL;
// }
// }
//
//
// /*
//  * Too bad it doesn't check the return values to let the user
//  * know about -1 values.  This will result in an 'Okay.' to a
//  * 'reload' command even when the string was not replaced.
//  * To fix later, if desired. -gg 6/24/99
//  */
// ACMD(do_reboot)
// {
// char arg[MAX_INPUT_LENGTH];
//
// one_argument(argument, arg);
//
// if (!str_cmp(arg, "all") || *arg == '*') {
// if (file_to_string_alloc(GREETINGS_FILE, &greetings) == 0)
// prune_crlf(greetings);
// file_to_string_alloc(WIZLIST_FILE, &wizlist);
// file_to_string_alloc(IMMLIST_FILE, &immlist);
// file_to_string_alloc(NEWS_FILE, &news);
// file_to_string_alloc(CREDITS_FILE, &credits);
// file_to_string_alloc(MOTD_FILE, &motd);
// file_to_string_alloc(IMOTD_FILE, &imotd);
// file_to_string_alloc(HELP_PAGE_FILE, &help);
// file_to_string_alloc(INFO_FILE, &info);
// file_to_string_alloc(POLICIES_FILE, &policies);
// file_to_string_alloc(HANDBOOK_FILE, &handbook);
// file_to_string_alloc(BACKGROUND_FILE, &background);
// } else if (!str_cmp(arg, "wizlist"))
// file_to_string_alloc(WIZLIST_FILE, &wizlist);
// else if (!str_cmp(arg, "immlist"))
// file_to_string_alloc(IMMLIST_FILE, &immlist);
// else if (!str_cmp(arg, "news"))
// file_to_string_alloc(NEWS_FILE, &news);
// else if (!str_cmp(arg, "credits"))
// file_to_string_alloc(CREDITS_FILE, &credits);
// else if (!str_cmp(arg, "motd"))
// file_to_string_alloc(MOTD_FILE, &motd);
// else if (!str_cmp(arg, "imotd"))
// file_to_string_alloc(IMOTD_FILE, &imotd);
// else if (!str_cmp(arg, "help"))
// file_to_string_alloc(HELP_PAGE_FILE, &help);
// else if (!str_cmp(arg, "info"))
// file_to_string_alloc(INFO_FILE, &info);
// else if (!str_cmp(arg, "policy"))
// file_to_string_alloc(POLICIES_FILE, &policies);
// else if (!str_cmp(arg, "handbook"))
// file_to_string_alloc(HANDBOOK_FILE, &handbook);
// else if (!str_cmp(arg, "background"))
// file_to_string_alloc(BACKGROUND_FILE, &background);
// else if (!str_cmp(arg, "greetings")) {
// if (file_to_string_alloc(GREETINGS_FILE, &greetings) == 0)
// prune_crlf(greetings);
// } else if (!str_cmp(arg, "xhelp")) {
// if (help_table)
// free_help();
// index_boot(DB_BOOT_HLP);
// } else {
// send_to_char(ch, "Unknown reload option.\r\n");
// return;
// }
//
// send_to_char(ch, "%s", OK);
// }

impl DB {
    fn boot_world(&mut self) {
        info!("Loading zone table.");
        self.index_boot(DB_BOOT_ZON);

        info!("Loading rooms.");
        self.index_boot(DB_BOOT_WLD);

        info!("Renumbering rooms.");
        self.renum_world();

        info!("Checking start rooms.");
        self.check_start_rooms();

        // log("Loading mobs and generating index.");
        // index_boot(DB_BOOT_MOB);
        //
        // log("Loading objs and generating index.");
        // index_boot(DB_BOOT_OBJ);

        info!("Renumbering zone table.");
        self.renum_zone_table();

        // if (!no_specials) {
        //     log("Loading shops.");
        //     index_boot(DB_BOOT_SHP);
        // }
    }
}

// void free_extra_descriptions(struct extra_descr_data *edesc)
// {
// struct extra_descr_data *enext;
//
// for (; edesc; edesc = enext) {
// enext = edesc->next;
//
// free(edesc->keyword);
// free(edesc->description);
// free(edesc);
// }
// }
//
//
// /* Free the world, in a memory allocation sense. */
// void destroy_db(void)
// {
// ssize_t cnt, itr;
// struct char_data *chtmp;
// struct obj_data *objtmp;
//
// /* Active Mobiles & Players */
// while (character_list) {
// chtmp = character_list;
// character_list = character_list->next;
// free_char(chtmp);
// }
//
// /* Active Objects */
// while (object_list) {
// objtmp = object_list;
// object_list = object_list->next;
// free_obj(objtmp);
// }
//
// /* Rooms */
// for (cnt = 0; cnt <= top_of_world; cnt++) {
// if (world[cnt].name)
// free(world[cnt].name);
// if (world[cnt].description)
// free(world[cnt].description);
// free_extra_descriptions(world[cnt].ex_description);
//
// for (itr = 0; itr < NUM_OF_DIRS; itr++) {
// if (!world[cnt].dir_option[itr])
// continue;
//
// if (world[cnt].dir_option[itr]->general_description)
// free(world[cnt].dir_option[itr]->general_description);
// if (world[cnt].dir_option[itr]->keyword)
// free(world[cnt].dir_option[itr]->keyword);
// free(world[cnt].dir_option[itr]);
// }
// }
// free(world);
//
// /* Objects */
// for (cnt = 0; cnt <= top_of_objt; cnt++) {
// if (obj_proto[cnt].name)
// free(obj_proto[cnt].name);
// if (obj_proto[cnt].description)
// free(obj_proto[cnt].description);
// if (obj_proto[cnt].short_description)
// free(obj_proto[cnt].short_description);
// if (obj_proto[cnt].action_description)
// free(obj_proto[cnt].action_description);
// free_extra_descriptions(obj_proto[cnt].ex_description);
// }
// free(obj_proto);
// free(obj_index);
//
// /* Mobiles */
// for (cnt = 0; cnt <= top_of_mobt; cnt++) {
// if (mob_proto[cnt].player.name)
// free(mob_proto[cnt].player.name);
// if (mob_proto[cnt].player.title)
// free(mob_proto[cnt].player.title);
// if (mob_proto[cnt].player.short_descr)
// free(mob_proto[cnt].player.short_descr);
// if (mob_proto[cnt].player.long_descr)
// free(mob_proto[cnt].player.long_descr);
// if (mob_proto[cnt].player.description)
// free(mob_proto[cnt].player.description);
//
// while (mob_proto[cnt].affected)
// affect_remove(&mob_proto[cnt], mob_proto[cnt].affected);
// }
// free(mob_proto);
// free(mob_index);
//
// /* Shops */
// destroy_shops();
//
// /* Zones */
// for (cnt = 0; cnt <= top_of_zone_table; cnt++) {
// if (zone_table[cnt].name)
// free(zone_table[cnt].name);
// if (zone_table[cnt].cmd)
// free(zone_table[cnt].cmd);
// }
// free(zone_table);
// }
//
//
/* body of the booting system */
pub fn boot_db<'a>(main_globals: &'a MainGlobals) -> DB {
    let mut ret = DB {
        //   globals: Some(main_globals.clone()),
        world: RefCell::new(vec![]),
        top_of_world: RefCell::new(0),
        character_list: RefCell::new(vec![]),
        zone_table: RefCell::new(vec![]),
        top_of_zone_table: RefCell::new(0),
        player_table: RefCell::new(vec![]),
        player_fl: RefCell::new(None),
        top_of_p_table: RefCell::new(0),
        top_idnum: RefCell::new(0),
        r_mortal_start_room: RefCell::new(0),
        r_immort_start_room: RefCell::new(0),
        r_frozen_start_room: RefCell::new(0),
        motd: "MOTD placeholder".to_string(),
        imotd: "IMOTD placeholder".to_string(),
        greetings: RefCell::new("Greetings Placeholder".parse().unwrap()),
        background: "BACKGROUND placeholder".to_string(),
    };
    // zone_rnum i;
    //
    info!("Boot db -- BEGIN.");
    //
    // log("Resetting the game time:");
    // reset_time();
    //
    info!("Reading news, credits, help, bground, info & motds.");
    // file_to_string_alloc(NEWS_FILE, &news);
    // file_to_string_alloc(CREDITS_FILE, &credits);
    main_globals.file_to_string_alloc(MOTD_FILE, &mut ret.motd);
    main_globals.file_to_string_alloc(IMOTD_FILE, &mut ret.imotd);
    // file_to_string_alloc(HELP_PAGE_FILE, &help);
    // file_to_string_alloc(INFO_FILE, &info);
    // file_to_string_alloc(WIZLIST_FILE, &wizlist);
    // file_to_string_alloc(IMMLIST_FILE, &immlist);
    // file_to_string_alloc(POLICIES_FILE, &policies);
    // file_to_string_alloc(HANDBOOK_FILE, &handbook);
    main_globals.file_to_string_alloc(BACKGROUND_FILE, &mut ret.background);
    if main_globals.file_to_string_alloc(GREETINGS_FILE, &mut ret.greetings.borrow_mut()) == 0 {
        prune_crlf(&mut ret.greetings.borrow_mut());
    }
    //
    // log("Loading spell definitions.");
    // mag_assign_spells();
    //
    ret.boot_world();
    //
    // log("Loading help entries.");
    // index_boot(DB_BOOT_HLP);

    info!("Generating player index.");
    ret.build_player_index();

    // log("Loading fight messages.");
    // load_messages();
    //
    // log("Loading social messages.");
    // boot_social_messages();
    //
    // log("Assigning function pointers:");
    //
    // if (!no_specials) {
    // log("   Mobiles.");
    // assign_mobiles();
    // log("   Shopkeepers.");
    // assign_the_shopkeepers();
    // log("   Objects.");
    // assign_objects();
    // log("   Rooms.");
    // assign_rooms();
    // }
    //
    // log("Assigning spell and skill levels.");
    // init_spell_levels();
    //
    // log("Sorting command list and spells.");
    // sort_commands();
    // sort_spells();
    //
    // log("Booting mail system.");
    // if (!scan_file()) {
    // log("    Mail boot failed -- Mail system disabled");
    // no_mail = 1;
    // }
    // log("Reading banned site and invalid-name list.");
    // load_banned();
    // Read_Invalid_List();
    //
    // if (!no_rent_check) {
    // log("Deleting timed-out crash and rent files:");
    // update_obj_file();
    // log("   Done.");
    // }
    //
    // /* Moved here so the object limit code works. -gg 6/24/98 */
    // if (!mini_mud) {
    // log("Booting houses.");
    // House_boot();
    // }
    //
    // for (i = 0; i <= top_of_zone_table; i++) {
    // log("Resetting #%d: %s (rooms %d-%d).", zone_table[i].number,
    // zone_table[i].name, zone_table[i].bot, zone_table[i].top);
    // reset_zone(i);
    // }
    //
    // reset_q.head = reset_q.tail = NULL;
    //
    // boot_time = time(0);
    //
    info!("Boot db -- DONE.");

    return ret;
}

//
//
// /* reset the time in the game from file */
// void reset_time(void)
// {
// time_t beginning_of_time = 0;
// FILE *bgtime;
//
// if ((bgtime = fopen(TIME_FILE, "r")) == NULL)
// log("SYSERR: Can't read from '%s' time file.", TIME_FILE);
// else {
// fscanf(bgtime, "%ld\n", &beginning_of_time);
// fclose(bgtime);
// }
// if (beginning_of_time == 0)
// beginning_of_time = 650336715;
//
// time_info = *mud_time_passed(time(0), beginning_of_time);
//
// if (time_info.hours <= 4)
// weather_info.sunlight = SUN_DARK;
// else if (time_info.hours == 5)
// weather_info.sunlight = SUN_RISE;
// else if (time_info.hours <= 20)
// weather_info.sunlight = SUN_LIGHT;
// else if (time_info.hours == 21)
// weather_info.sunlight = SUN_SET;
// else
// weather_info.sunlight = SUN_DARK;
//
// log("   Current Gametime: %dH %dD %dM %dY.", time_info.hours,
// time_info.day, time_info.month, time_info.year);
//
// weather_info.pressure = 960;
// if ((time_info.month >= 7) && (time_info.month <= 12))
// weather_info.pressure += dice(1, 50);
// else
// weather_info.pressure += dice(1, 80);
//
// weather_info.change = 0;
//
// if (weather_info.pressure <= 980)
// weather_info.sky = SKY_LIGHTNING;
// else if (weather_info.pressure <= 1000)
// weather_info.sky = SKY_RAINING;
// else if (weather_info.pressure <= 1020)
// weather_info.sky = SKY_CLOUDY;
// else
// weather_info.sky = SKY_CLOUDLESS;
// }
//
//
// /* Write the time in 'when' to the MUD-time file. */
// void save_mud_time(struct time_info_data *when)
// {
// FILE *bgtime;
//
// if ((bgtime = fopen(TIME_FILE, "w")) == NULL)
// log("SYSERR: Can't write to '%s' time file.", TIME_FILE);
// else {
// fprintf(bgtime, "%ld\n", mud_time_to_secs(when));
// fclose(bgtime);
// }
// }
//
//
// void free_player_index(void)
// {
// int tp;
//
// if (!player_table)
// return;
//
// for (tp = 0; tp <= top_of_p_table; tp++)
// if (player_table[tp].name)
// free(player_table[tp].name);
//
// free(player_table);
// player_table = NULL;
// top_of_p_table = 0;
// }

impl DB {
    /* generate index table for the player file */
    fn build_player_index<'a>(&mut self) {
        let mut nr = -1;
        let size: usize;
        let recs: u64;
        // struct char_file_u
        // dummy;

        let player_file: File;
        let r = OpenOptions::new().write(true).read(true).open(PLAYER_FILE);
        if r.is_err() {
            let err = r.err().unwrap();
            if err.kind() != ErrorKind::NotFound {
                error!("SYSERR: fatal error opening playerfile: {}", err);
                process::exit(1);
            } else {
                info!("No playerfile.  Creating a new one.");
                touch(Path::new(PLAYER_FILE)).expect("SYSERR: fatal error creating playerfile");
                player_file = File::open(PLAYER_FILE)
                    .expect("SYSERR: fatal error opening playerfile after creation");
            }
        } else {
            player_file = r.unwrap();
        }

        *self.player_fl.borrow_mut() = Some(player_file);

        let mut t = self.player_fl.borrow_mut();
        let mut file_mut = t.as_mut().unwrap();
        let size = file_mut
            .seek(SeekFrom::End(0))
            .expect("SYSERR: fatal error seeking playerfile");
        file_mut
            .rewind()
            .expect("SYSERR: fatal error rewinding playerfile");

        if size % mem::size_of::<CharFileU>() as u64 != 0 {
            warn!("WARNING:  PLAYERFILE IS PROBABLY CORRUPT!");
        }
        recs = size / mem::size_of::<CharFileU>() as u64;
        if recs != 0 {
            info!("   {} players in database.", recs);
            // CREATE(player_table, struct PlayerIndexElement, recs);
        } else {
            // player_table = NULL;
            *self.top_of_p_table.borrow_mut() = -1;
            return;
        }

        // for (; ; ) {
        //     fread(&dummy, sizeof(struct char_file_u), 1, player_fl);
        //     if (feof(player_fl))
        //     break;
        //
        //     /* new record */
        //     nr + +;
        //     CREATE(player_table[nr].name, char, strlen(dummy.name) + 1);
        //     for (i = 0; (* (player_table[nr].name + i) = LOWER(* (dummy.name + i))); i+ +)
        //     ;
        //     player_table[nr].id = dummy.char_specials_saved.idnum;
        //     top_idnum = MAX(top_idnum, dummy.char_specials_saved.idnum);
        // }

        *self.top_of_p_table.borrow_mut() = nr;
    }
}

// /*
//  * Thanks to Andrey (andrey@alex-ua.com) for this bit of code, although I
//  * did add the 'goto' and changed some "while()" into "do { } while()".
//  *	-gg 6/24/98 (technically 6/25/98, but I care not.)
//  */
// int count_alias_records(FILE *fl)
// {
// char key[READ_SIZE], next_key[READ_SIZE];
// char line[READ_SIZE], *scan;
// int total_keywords = 0;
//
// /* get the first keyword line */
// get_one_line(fl, key);
//
// while (*key != '$') {
// /* skip the text */
// do {
// get_one_line(fl, line);
// if (feof(fl))
// goto ackeof;
// } while (*line != '#');
//
// /* now count keywords */
// scan = key;
// do {
// scan = one_word(scan, next_key);
// if (*next_key)
// ++total_keywords;
// } while (*next_key);
//
// /* get next keyword line (or $) */
// get_one_line(fl, key);
//
// if (feof(fl))
// goto ackeof;
// }
//
// return (total_keywords);
//
// /* No, they are not evil. -gg 6/24/98 */
// ackeof:
// log("SYSERR: Unexpected end of help file.");
// exit(1);	/* Some day we hope to handle these things better... */
// }

/* function to count how many hash-mark delimited records exist in a file */
fn count_hash_records(fl: File) -> i32 {
    let mut count = 0;
    let reader = BufReader::new(fl);
    for l in reader.lines() {
        if l.is_ok() && l.unwrap().starts_with('#') {
            count += 1;
        }
    }
    count
}

const INDEX_FILE: &str = "index"; /* index of world files		*/
const MINDEX_FILE: &str = "index.mini"; /* ... and for mini-mud-mode	*/
const WLD_PREFIX: &str = "world/wld/"; /* room definitions	*/
const MOB_PREFIX: &str = "world/mob/"; /* monster prototypes	*/
const OBJ_PREFIX: &str = "world/obj/"; /* object prototypes	*/
const ZON_PREFIX: &str = "world/zon/"; /* zon defs & command tables */
const SHP_PREFIX: &str = "world/shp/"; /* shop definitions	*/
//#define HLP_PREFIX	LIB_TEXT"help"SLASH	/* for HELP <keyword>	*/
/* arbitrary constants used by index_boot() (must be unique) */
const DB_BOOT_WLD: u8 = 0;
const DB_BOOT_MOB: u8 = 1;
const DB_BOOT_OBJ: u8 = 2;
const DB_BOOT_ZON: u8 = 3;
const DB_BOOT_SHP: u8 = 4;
const DB_BOOT_HLP: u8 = 5;

impl DB {
    fn index_boot(&mut self, mode: u8) {
        let mut index_filename: &str;
        let mut prefix: &str; /* NULL or egcs 1.1 complains */
        let mut rec_count = 0;
        let mut size: [u8; 2] = [0; 2];
        //FILE *db_index, *db_file;
        //int rec_count = 0, size[2];
        //char buf2[PATH_MAX], buf1[MAX_STRING_LENGTH];

        match mode {
            DB_BOOT_WLD => {
                prefix = WLD_PREFIX;
            }
            DB_BOOT_MOB => {
                prefix = MOB_PREFIX;
            }
            DB_BOOT_OBJ => {
                prefix = OBJ_PREFIX;
            }
            DB_BOOT_ZON => {
                prefix = ZON_PREFIX;
            }
            DB_BOOT_SHP => {
                prefix = SHP_PREFIX;
            }
            // DB_BOOT_HLP => {
            //     prefix = HLP_PREFIX;
            // }
            _ => {
                error!("SYSERR: Unknown subcommand {} to index_boot!", mode);
                process::exit(1);
            }
        }

        // if (mini_mud)
        // index_filename = MINDEX_FILE;
        // else
        index_filename = INDEX_FILE;

        let mut buf2 = format!("{}{}", prefix, index_filename);
        let db_index = File::open(buf2.as_str());
        if db_index.is_err() {
            error!(
                "SYSERR: opening index file '{}': {}",
                buf2.as_str(),
                db_index.err().unwrap()
            );
            process::exit(1);
        }
        let mut db_index = db_index.unwrap();

        let mut reader = BufReader::new(db_index);
        /* first, count the number of records in the file so we can malloc */
        let mut buf1 = String::new();
        reader
            .read_line(&mut buf1)
            .expect("Error while reading index file #1");
        buf1 = buf1.trim_end().to_string();
        while !buf1.starts_with('$') {
            let buf2 = format!("{}{}", prefix, buf1.as_str());
            let db_file = File::open(buf2.as_str());
            if db_file.is_err() {
                error!(
                    "SYSERR: File '{}' listed in '{}{}': {}",
                    buf2.as_str(),
                    prefix,
                    index_filename,
                    db_file.err().unwrap()
                );
                buf1.clear();
                reader
                    .read_line(&mut buf1)
                    .expect("Error while reading index file #2");
                buf1 = buf1.trim_end().to_string();
                continue;
            } else {
                if mode == DB_BOOT_ZON {
                    rec_count += 1;
                    // } else if mode == DB_BOOT_HLP {
                    //     rec_count += count_alias_records(db_file);
                } else {
                    rec_count += count_hash_records(db_file.unwrap());
                }
            }
            buf1.clear();
            reader
                .read_line(&mut buf1)
                .expect("Error while reading index file #3");
            buf1 = buf1.trim_end().to_string();
        }

        /* Exit if 0 records, unless this is shops */
        if rec_count == 0 {
            if mode == DB_BOOT_SHP {
                return;
            }
            error!(
                "SYSERR: boot error - 0 records counted in {}{}.",
                prefix, index_filename
            );
            process::exit(1);
        }

        /*
         * NOTE: "bytes" does _not_ include strings or other later malloc'd things.
         */
        match mode {
            DB_BOOT_WLD => {
                //CREATE(world, struct room_data, rec_count);
                //size[0] = sizeof(struct room_data) *rec_count;
                self.world.borrow_mut().reserve_exact(rec_count as usize);
                size[0] = 0;
                info!("   {} rooms, {} bytes.", rec_count, size[0]);
            }
            // DB_BOOT_MOB => {
            //     CREATE(mob_proto, struct char_data, rec_count);
            //     CREATE(mob_index, struct index_data, rec_count);
            //     size[0] = sizeof(struct index_data) *rec_count;
            //     size[1] = sizeof(struct char_data) *rec_count;
            //     log("   %d mobs, %d bytes in index, %d bytes in prototypes.", rec_count, size[0], size[1]);
            // }
            // DB_BOOT_OBJ => {
            //     CREATE(obj_proto, struct obj_data, rec_count);
            //     CREATE(obj_index, struct index_data, rec_count);
            //     size[0] = sizeof(struct index_data) *rec_count;
            //     size[1] = sizeof(struct obj_data) *rec_count;
            //     log("   %d objs, %d bytes in index, %d bytes in prototypes.", rec_count, size[0], size[1]);
            // }
            DB_BOOT_ZON => {
                // CREATE(zone_table, struct zone_data, rec_count);
                // size[0] = sizeof(struct zone_data) *rec_count;
                self.zone_table
                    .borrow_mut()
                    .reserve_exact(rec_count as usize);
                size[0] = 0;
                info!("   {} zones, {} bytes.", rec_count, size[0]);
            }
            // DB_BOOT_HLP => {
            //     CREATE(help_table, struct help_index_element, rec_count);
            //     size[0] = sizeof(struct help_index_element) *rec_count;
            //     log("   %d entries, %d bytes.", rec_count, size[0]);
            // }
            _ => {}
        }

        reader.rewind().expect("Cannot rewind DB index file reader");
        buf1.clear();
        reader
            .read_line(&mut buf1)
            .expect("Cannot read index line #4");
        buf1 = buf1.trim_end().to_string();
        while !buf1.starts_with('$') {
            buf2 = format!("{}{}", prefix, buf1.as_str());
            let db_file = File::open(buf2.as_str());
            if db_file.is_err() {
                error!("SYSERR: {}: {}", buf2.as_str(), db_file.err().unwrap());
                process::exit(1);
            }

            match mode {
                DB_BOOT_WLD | DB_BOOT_OBJ | DB_BOOT_MOB => {
                    self.discrete_load(db_file.unwrap(), mode, buf2.as_str());
                }
                DB_BOOT_ZON => {
                    self.load_zones(db_file.unwrap(), buf2.as_str());
                }
                //        DB_BOOT_HLP => {
                //            /*
                // * If you think about it, we have a race here.  Although, this is the
                // * "point-the-gun-at-your-own-foot" type of race.
                // */
                //             load_help(db_file);
                //        }
                // DB_BOOT_SHP => {
                //     boot_the_shops(db_file, buf2, rec_count);
                // }
                _ => {}
            }

            //fclose(db_file);
            //fscanf(db_index, "%s\n", buf1);
            buf1.clear();
            reader
                .read_line(&mut buf1)
                .expect("Error while reading index file #5");
            buf1 = buf1.trim_end().to_string();
        }
        //fclose(db_index);

        /* sort the help index */
        // if (mode == DB_BOOT_HLP) {
        //     qsort(help_table, top_of_helpt, sizeof(struct help_index_element), hsort);
        //     top_of_helpt - -;
        // }
    }

    fn discrete_load(&mut self, file: File, mode: u8, filename: &str) {
        let mut nr = -1;
        let mut last: i32;
        let mut line = String::new();
        let mut reader = BufReader::new(file);

        const MODES: [&'static str; 3] = ["world", "mob", "obj"];

        loop {
            /*
             * we have to do special processing with the obj files because they have
             * no end-of-record marker :(
             */
            if mode != DB_BOOT_OBJ || nr < 0 {
                if get_line(&mut reader, &mut line) == 0 {
                    if nr == -1 {
                        error!(
                            "SYSERR: {} file {} is empty!",
                            MODES[mode as usize], filename
                        );
                    } else {
                        error!("SYSERR: Format error in {} after {} #{}\n...expecting a new {}, but file ended!\n(maybe the file is not terminated with '$'?)", filename,
                            MODES[mode as usize], nr, MODES[mode as usize]);
                    }
                    process::exit(1);
                }
            }
            if line.starts_with('$') {
                return;
            }

            if line.starts_with('#') {
                last = nr;
                let regex = Regex::new(r"^#(\d{1,9})").unwrap();
                let f = regex.captures(line.as_str());
                if f.is_none() {
                    error!(
                        "SYSERR: Format error after {} #{}",
                        MODES[mode as usize], last
                    );
                    process::exit(1);
                }
                let f = f.unwrap();
                nr = f[1].parse::<i32>().unwrap();

                if nr >= 99999 {
                    return;
                } else {
                    match mode {
                        DB_BOOT_WLD => {
                            self.parse_room(&mut reader, nr);
                        }
                        // DB_BOOT_MOB => {
                        //     parse_mobile(fl, nr);
                        // }
                        // DB_BOOT_OBJ => {
                        //     strlcpy(line, parse_object(fl, nr), sizeof(line));
                        // }
                        _ => {}
                    }
                }
            } else {
                error!(
                    "SYSERR: Format error in {} file {} near {} #{}",
                    MODES[mode as usize], filename, MODES[mode as usize], nr
                );
                error!("SYSERR: ... offending line: '{}'", line);
                process::exit(1);
            }
        }
    }
}

fn asciiflag_conv(flag: &str) -> i64 {
    let mut flags: i64 = 0;
    let mut is_num = true;

    for p in flag.chars() {
        if p.is_lowercase() {
            flags |= 1 << (p as u8 - b'a');
        } else if p.is_uppercase() {
            flags |= 1 << (26 + p as u8 - b'A');
        }

        if !p.is_digit(10) {
            is_num = false;
        }
    }

    if is_num {
        flags = flag.parse::<i64>().unwrap();
    }

    return flags;
}

impl DB {
    /* load the rooms */
    fn parse_room(&self, reader: &mut BufReader<File>, virtual_nr: i32) {
        //static int room_nr = 0, zone = 0;
        let mut t = [0; 10];
        let mut i: i32;
        let mut line = String::new();
        let mut zone = 0;
        let mut room_nr = self.world.borrow().len();
        //     char line[READ_SIZE], flags[128], buf2[MAX_STRING_LENGTH], buf[128];
        // struct extra_descr_data * new_descr;

        /* This really had better fit or there are other problems. */
        let buf2 = format!("room #{}", virtual_nr);

        if virtual_nr < self.zone_table.borrow()[zone].bot as i32 {
            error!("SYSERR: Room #{} is below zone {}.", virtual_nr, zone);
            process::exit(1);
        }
        while virtual_nr > self.zone_table.borrow()[zone].top as i32 {
            zone += 1;
            if zone >= self.zone_table.borrow().len() {
                error!("SYSERR: Room {} is outside of any zone.", virtual_nr);
                process::exit(1);
            }
        }
        let mut rd = RoomData {
            number: virtual_nr as room_vnum,
            zone: zone as zone_rnum,
            sector_type: 0,
            name: fread_string(reader, buf2.as_str()),
            description: fread_string(reader, buf2.as_str()),
            ex_description: None,
            //dir_option: [None, None, None, None, None, None],
            dir_option: [None, None, None, None, None, None],
            room_flags: 0,
            light: RefCell::new(0),
            people: RefCell::new(None),
        };

        if get_line(reader, &mut line) == 0 {
            error!(
                "SYSERR: Expecting roomflags/sector type of room #{} but file ended!",
                virtual_nr,
            );
            process::exit(1);
        }

        let regex = Regex::new(r"^(\d{1,9})\s(\S*)\s(\d{1,9})").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!(
                "SYSERR: Format error in roomflags/sector type of room #{}",
                virtual_nr,
            );
            process::exit(1);
        }
        let f = f.unwrap();
        t[0] = f[1].parse::<i32>().unwrap();
        let mut flags = &f[2];
        t[2] = f[3].parse::<i32>().unwrap();

        /* t[0] is the zone number; ignored with the zone-file system */

        rd.room_flags = asciiflag_conv(flags) as i32;
        let msg = format!("object #{}", virtual_nr); /* sprintf: OK (until 399-bit integers) */
        check_bitvector_names(rd.room_flags as i64, ROOM_BITS_COUNT, msg.as_str(), "room");

        rd.sector_type = t[2];

        //rd.func = NULL;
        //rd.contents = NULL;
        rd.people = RefCell::new(None);
        *rd.light.borrow_mut() = 0; /* Zero light sources */

        // for i in 0..NUM_OF_DIRS {
        //     rd.dir_option[i] = None;
        // }

        rd.ex_description = None;

        let buf = format!(
            "SYSERR: Format error in room #{} (expecting D/E/S)",
            virtual_nr
        );

        loop {
            if get_line(reader, &mut line) == 0 {
                error!("{}", buf);
                process::exit(1);
            }
            match line.remove(0) {
                'D' => {
                    self.setup_dir(reader, &mut rd, line.parse::<i32>().unwrap());
                }
                'E' => {
                    //CREATE(new_descr, struct extra_descr_data, 1);
                    rd.ex_description = Some(Box::new(ExtraDescrData {
                        keyword: fread_string(reader, buf2.as_str()),
                        description: fread_string(reader, buf2.as_str()),
                        next: rd.ex_description,
                    }));
                }
                'S' => {
                    /* end of room */
                    *self.top_of_world.borrow_mut() = room_nr as room_rnum;
                    room_nr += 1;
                    break;
                }
                _ => {
                    error!("{}", buf);
                    process::exit(1);
                }
            }
        }
        self.world.borrow_mut().push(Rc::new(rd));
        *self.top_of_world.borrow_mut() += 1;
    }

    /* read direction data */
    fn setup_dir(&self, reader: &mut BufReader<File>, room: &mut RoomData, dir: i32) {
        let mut t = [0; 5];
        let mut line = String::new();
        // char line[READ_SIZE], buf2[128];

        let buf2 = format!(
            "room #{}, direction D{}",
            room.number,
            //get_room_vnum!(self, room as usize),
            dir
        );

        let mut rdr = RoomDirectionData {
            general_description: fread_string(reader, buf2.as_str()),
            keyword: fread_string(reader, buf2.as_str()),
            exit_info: 0,
            key: 0,
            to_room: RefCell::new(0),
        };

        if get_line(reader, &mut line) == 0 {
            error!("SYSERR: Format error, {}", buf2);
            process::exit(1);
        }

        let regex = Regex::new(r"^(-?\d{1,9})\s(-?\d{1,9})\s(-?\d{1,9})").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!("SYSERR: Format error, {}", buf2);
            process::exit(1);
        }
        let f = f.unwrap();
        t[0] = f[1].parse::<i32>().unwrap();
        t[1] = f[2].parse::<i32>().unwrap();
        t[2] = f[3].parse::<i32>().unwrap();
        if t[0] == 1 {
            rdr.exit_info = EX_ISDOOR;
        } else if t[0] == 2 {
            rdr.exit_info = EX_ISDOOR | EX_PICKPROOF;
        } else {
            rdr.exit_info = 0;
        }

        rdr.key = t[1] as obj_vnum;
        *rdr.to_room.borrow_mut() = t[2] as room_rnum;

        //let mut a = RefCell::borrow_mut(self.world.get(room as usize).unwrap());
        room.dir_option[dir as usize] = Some(Rc::new(rdr));
        // let b = &mut a.dir_option;
        // b[dir as usize] = Some(Box::new(rdr));

        // let mut a = self.world.get(room as usize);
        // let mut b = a.unwrap();
        // let &mut c = b.dir_option[dir as usize];

        //b.dir_option[dir as usize] = Box::new(None);
    }

    // /* make sure the start rooms exist & resolve their vnums to rnums */
    fn check_start_rooms(&self) {
        *self.r_mortal_start_room.borrow_mut() =
            real_room(self.world.borrow().as_ref(), MORTAL_START_ROOM);
        if *self.r_mortal_start_room.borrow() == NOWHERE {
            error!("SYSERR:  Mortal start room does not exist.  Change in config.c.");
            process::exit(1);
        }
        *self.r_immort_start_room.borrow_mut() =
            real_room(self.world.borrow().as_ref(), IMMORT_START_ROOM);
        if *self.r_immort_start_room.borrow() == NOWHERE {
            // if (!mini_mud)
            error!("SYSERR:  Warning: Immort start room does not exist.  Change in config.c.");
            *self.r_immort_start_room.borrow_mut() = *self.r_mortal_start_room.borrow();
        }
        *self.r_frozen_start_room.borrow_mut() =
            real_room(self.world.borrow().as_ref(), FROZEN_START_ROOM);
        if *self.r_frozen_start_room.borrow() == NOWHERE {
            // if (!mini_mud)
            error!("SYSERR:  Warning: Frozen start room does not exist.  Change in config.c.");
            *self.r_frozen_start_room.borrow_mut() = *self.r_mortal_start_room.borrow();
        }
    }
}

impl DB {
    /* resolve all vnums into rnums in the world */
    fn renum_world(&mut self) {
        for (i, room_data) in self.world.borrow().iter().enumerate() {
            for door in 0..NUM_OF_DIRS {
                let to_room: room_rnum;
                {
                    if room_data.dir_option[door].is_none() {
                        continue;
                    }
                    to_room = *room_data.dir_option[door]
                        .as_ref()
                        .unwrap()
                        .to_room
                        .borrow();
                }
                if to_room != NOWHERE {
                    let rn = real_room(self.world.borrow().as_ref(), to_room);
                    *room_data.dir_option[door]
                        .as_ref()
                        .unwrap()
                        .to_room
                        .borrow_mut() = rn;
                }
            }
        }
    }

    // #define ZCMD zone_table[zone].cmd[cmd_no]

    /*
     * "resulve vnums into rnums in the zone reset tables"
     *
     * Or in English: Once all of the zone reset tables have been loaded, we
     * resolve the virtual numbers into real numbers all at once so we don't have
     * to do it repeatedly while the game is running.  This does make adding any
     * room, mobile, or object a little more difficult while the game is running.
     *
     * NOTE 1: Assumes NOWHERE == NOBODY == NOTHING.
     * NOTE 2: Assumes sizeof(room_rnum) >= (sizeof(mob_rnum) and sizeof(obj_rnum))
     */

    fn renum_zone_table(&mut self) {
        //int cmd_no;
        //room_rnum a, b, c, olda, oldb, oldc;
        //zone_rnum zone;
        //char buf[128];
        let mut olda = 0;
        let mut oldb = 0;
        let mut oldc = 0;

        for zone in self.zone_table.borrow_mut().iter_mut() {
            for cmd_no in 0..zone.cmd.len() {
                let zcmd = &mut zone.cmd[cmd_no];
                if zcmd.command == 'S' {
                    break;
                }
                let mut a = 0;
                let mut b = 0;
                let mut c = 0;
                olda = zcmd.arg1;
                oldb = zcmd.arg2;
                oldc = zcmd.arg3;
                match zcmd.command {
                    'M' => {
                        //a = ZCMD.arg1 = real_mobile(ZCMD.arg1);
                        zcmd.arg3 =
                            real_room(self.world.borrow().as_ref(), zcmd.arg3 as room_vnum) as i32;
                        c = zcmd.arg3;
                    }
                    'O' => {
                        //a = ZCMD.arg1 = real_object(ZCMD.arg1);
                        if zcmd.arg3 != NOWHERE as i32 {
                            zcmd.arg3 =
                                real_room(self.world.borrow().as_ref(), zcmd.arg3 as room_vnum)
                                    as i32;
                            c = zcmd.arg3;
                        }
                    }
                    'G' => {
                        //a = ZCMD.arg1 = real_object(ZCMD.arg1);
                    }
                    'E' => {
                        // a = ZCMD.arg1 = real_object(ZCMD.arg1);
                    }
                    'P' => {
                        // a = ZCMD.arg1 = real_object(ZCMD.arg1);
                        // c = ZCMD.arg3 = real_object(ZCMD.arg3);
                    }
                    'D' => {
                        zcmd.arg1 =
                            real_room(self.world.borrow().as_ref(), zcmd.arg1 as room_vnum) as i32;
                        a = zcmd.arg1;
                    }
                    'R' => {
                        /* rem obj from room */
                        zcmd.arg2 =
                            real_room(self.world.borrow().as_ref(), zcmd.arg2 as room_vnum) as i32;
                        b = zcmd.arg2;
                    }
                    _ => {}
                }

                if a == NOWHERE as i32 || b == NOWHERE as i32 || c == NOWHERE as i32 {
                    // TODO // if ( ! mini_mud) {
                    format!(
                        "Invalid vnum {}, cmd disabled",
                        if a == NOWHERE as i32 {
                            olda
                        } else if b == NOWHERE as i32 {
                            oldb
                        } else {
                            oldc
                        }
                    );
                    // TODO log_zone_error(zone, cmd_no, buf);
                    // }
                    zcmd.command = '*';
                }
            }
        }
    }
}

// void parse_simple_mob(FILE *mob_f, int i, int nr)
// {
// int j, t[10];
// char line[READ_SIZE];
//
// mob_proto[i].real_abils.str = 11;
// mob_proto[i].real_abils.intel = 11;
// mob_proto[i].real_abils.wis = 11;
// mob_proto[i].real_abils.dex = 11;
// mob_proto[i].real_abils.con = 11;
// mob_proto[i].real_abils.cha = 11;
//
// if (!get_line(mob_f, line)) {
// log("SYSERR: Format error in mob #%d, file ended after S flag!", nr);
// exit(1);
// }
//
// if (sscanf(line, " %d %d %d %dd%d+%d %dd%d+%d ",
// t, t + 1, t + 2, t + 3, t + 4, t + 5, t + 6, t + 7, t + 8) != 9) {
// log("SYSERR: Format error in mob #%d, first line after S flag\n"
// "...expecting line of form '# # # #d#+# #d#+#'", nr);
// exit(1);
// }
//
// GET_LEVEL(mob_proto + i) = t[0];
// GET_HITROLL(mob_proto + i) = 20 - t[1];
// GET_AC(mob_proto + i) = 10 * t[2];
//
// /* max hit = 0 is a flag that H, M, V is xdy+z */
// GET_MAX_HIT(mob_proto + i) = 0;
// GET_HIT(mob_proto + i) = t[3];
// GET_MANA(mob_proto + i) = t[4];
// GET_MOVE(mob_proto + i) = t[5];
//
// GET_MAX_MANA(mob_proto + i) = 10;
// GET_MAX_MOVE(mob_proto + i) = 50;
//
// mob_proto[i].mob_specials.damnodice = t[6];
// mob_proto[i].mob_specials.damsizedice = t[7];
// GET_DAMROLL(mob_proto + i) = t[8];
//
// if (!get_line(mob_f, line)) {
// log("SYSERR: Format error in mob #%d, second line after S flag\n"
// "...expecting line of form '# #', but file ended!", nr);
// exit(1);
// }
//
// if (sscanf(line, " %d %d ", t, t + 1) != 2) {
// log("SYSERR: Format error in mob #%d, second line after S flag\n"
// "...expecting line of form '# #'", nr);
// exit(1);
// }
//
// GET_GOLD(mob_proto + i) = t[0];
// GET_EXP(mob_proto + i) = t[1];
//
// if (!get_line(mob_f, line)) {
// log("SYSERR: Format error in last line of mob #%d\n"
// "...expecting line of form '# # #', but file ended!", nr);
// exit(1);
// }
//
// if (sscanf(line, " %d %d %d ", t, t + 1, t + 2) != 3) {
// log("SYSERR: Format error in last line of mob #%d\n"
// "...expecting line of form '# # #'", nr);
// exit(1);
// }
//
// GET_POS(mob_proto + i) = t[0];
// GET_DEFAULT_POS(mob_proto + i) = t[1];
// GET_SEX(mob_proto + i) = t[2];
//
// GET_CLASS(mob_proto + i) = 0;
// GET_WEIGHT(mob_proto + i) = 200;
// GET_HEIGHT(mob_proto + i) = 198;
//
// /*
//  * these are now save applies; base save numbers for MOBs are now from
//  * the warrior save table.
//  */
// for (j = 0; j < 5; j++)
// GET_SAVE(mob_proto + i, j) = 0;
// }
//
//
// /*
//  * interpret_espec is the function that takes espec keywords and values
//  * and assigns the correct value to the mob as appropriate.  Adding new
//  * e-specs is absurdly easy -- just add a new CASE statement to this
//  * function!  No other changes need to be made anywhere in the code.
//  *
//  * CASE		: Requires a parameter through 'value'.
//  * BOOL_CASE	: Being specified at all is its value.
//  */
//
// #define CASE(test)	\
// if (value && !matched && !str_cmp(keyword, test) && (matched = TRUE))
//
// #define BOOL_CASE(test)	\
// if (!value && !matched && !str_cmp(keyword, test) && (matched = TRUE))
//
// #define RANGE(low, high)	\
// (num_arg = MAX((low), MIN((high), (num_arg))))
//
// void interpret_espec(const char *keyword, const char *value, int i, int nr)
// {
// int num_arg = 0, matched = FALSE;
//
// /*
//  * If there isn't a colon, there is no value.  While Boolean options are
//  * possible, we don't actually have any.  Feel free to make some.
// */
// if (value)
// num_arg = atoi(value);
//
// CASE("BareHandAttack") {
// RANGE(0, 99);
// mob_proto[i].mob_specials.attack_type = num_arg;
// }
//
// CASE("Str") {
// RANGE(3, 25);
// mob_proto[i].real_abils.str = num_arg;
// }
//
// CASE("StrAdd") {
// RANGE(0, 100);
// mob_proto[i].real_abils.str_add = num_arg;
// }
//
// CASE("Int") {
// RANGE(3, 25);
// mob_proto[i].real_abils.intel = num_arg;
// }
//
// CASE("Wis") {
// RANGE(3, 25);
// mob_proto[i].real_abils.wis = num_arg;
// }
//
// CASE("Dex") {
// RANGE(3, 25);
// mob_proto[i].real_abils.dex = num_arg;
// }
//
// CASE("Con") {
// RANGE(3, 25);
// mob_proto[i].real_abils.con = num_arg;
// }
//
// CASE("Cha") {
// RANGE(3, 25);
// mob_proto[i].real_abils.cha = num_arg;
// }
//
// if (!matched) {
// log("SYSERR: Warning: unrecognized espec keyword %s in mob #%d",
// keyword, nr);
// }
// }
//
// #undef CASE
// #undef BOOL_CASE
// #undef RANGE
//
// void parse_espec(char *buf, int i, int nr)
// {
// char *ptr;
//
// if ((ptr = strchr(buf, ':')) != NULL) {
// *(ptr++) = '\0';
// while (isspace(*ptr))
// ptr++;
// }
// interpret_espec(buf, ptr, i, nr);
// }
//
//
// void parse_enhanced_mob(FILE *mob_f, int i, int nr)
// {
// char line[READ_SIZE];
//
// parse_simple_mob(mob_f, i, nr);
//
// while (get_line(mob_f, line)) {
// if (!strcmp(line, "E"))	/* end of the enhanced section */
// return;
// else if (*line == '#') {	/* we've hit the next mob, maybe? */
// log("SYSERR: Unterminated E section in mob #%d", nr);
// exit(1);
// } else
// parse_espec(line, i, nr);
// }
//
// log("SYSERR: Unexpected end of file reached after mob #%d", nr);
// exit(1);
// }
//
//
// void parse_mobile(FILE *mob_f, int nr)
// {
// static int i = 0;
// int j, t[10];
// char line[READ_SIZE], *tmpptr, letter;
// char f1[128], f2[128], buf2[128];
//
// mob_index[i].vnum = nr;
// mob_index[i].number = 0;
// mob_index[i].func = NULL;
//
// clear_char(mob_proto + i);
//
// /*
//  * Mobiles should NEVER use anything in the 'player_specials' structure.
//  * The only reason we have every mob in the game share this copy of the
//  * structure is to save newbie coders from themselves. -gg 2/25/98
//  */
// mob_proto[i].player_specials = &dummy_mob;
// sprintf(buf2, "mob vnum %d", nr);	/* sprintf: OK (for 'buf2 >= 19') */
//
// /***** String data *****/
// mob_proto[i].player.name = fread_string(mob_f, buf2);
// tmpptr = mob_proto[i].player.short_descr = fread_string(mob_f, buf2);
// if (tmpptr && *tmpptr)
// if (!str_cmp(fname(tmpptr), "a") || !str_cmp(fname(tmpptr), "an") ||
// !str_cmp(fname(tmpptr), "the"))
// *tmpptr = LOWER(*tmpptr);
// mob_proto[i].player.long_descr = fread_string(mob_f, buf2);
// mob_proto[i].player.description = fread_string(mob_f, buf2);
// GET_TITLE(mob_proto + i) = NULL;
//
// /* *** Numeric data *** */
// if (!get_line(mob_f, line)) {
// log("SYSERR: Format error after string section of mob #%d\n"
// "...expecting line of form '# # # {S | E}', but file ended!", nr);
// exit(1);
// }
//
// #ifdef CIRCLE_ACORN	/* Ugh. */
// if (sscanf(line, "%s %s %d %s", f1, f2, t + 2, &letter) != 4) {
// #else
// if (sscanf(line, "%s %s %d %c", f1, f2, t + 2, &letter) != 4) {
// #endif
// log("SYSERR: Format error after string section of mob #%d\n"
// "...expecting line of form '# # # {S | E}'", nr);
// exit(1);
// }
//
// MOB_FLAGS(mob_proto + i) = asciiflag_conv(f1);
// SET_BIT(MOB_FLAGS(mob_proto + i), MOB_ISNPC);
// if (MOB_FLAGGED(mob_proto + i, MOB_NOTDEADYET)) {
// /* Rather bad to load mobiles with this bit already set. */
// log("SYSERR: Mob #%d has reserved bit MOB_NOTDEADYET set.", nr);
// REMOVE_BIT(MOB_FLAGS(mob_proto + i), MOB_NOTDEADYET);
// }
// check_bitvector_names(MOB_FLAGS(mob_proto + i), action_bits_count, buf2, "mobile");
//
// AFF_FLAGS(mob_proto + i) = asciiflag_conv(f2);
// check_bitvector_names(AFF_FLAGS(mob_proto + i), affected_bits_count, buf2, "mobile affect");
//
// GET_ALIGNMENT(mob_proto + i) = t[2];
//
// /* AGGR_TO_ALIGN is ignored if the mob is AGGRESSIVE. */
// if (MOB_FLAGGED(mob_proto + i, MOB_AGGRESSIVE) && MOB_FLAGGED(mob_proto + i, MOB_AGGR_GOOD | MOB_AGGR_EVIL | MOB_AGGR_NEUTRAL))
// log("SYSERR: Mob #%d both Aggressive and Aggressive_to_Alignment.", nr);
//
// switch (UPPER(letter)) {
// case 'S':	/* Simple monsters */
// parse_simple_mob(mob_f, i, nr);
// break;
// case 'E':	/* Circle3 Enhanced monsters */
// parse_enhanced_mob(mob_f, i, nr);
// break;
// /* add new mob types here.. */
// default:
// log("SYSERR: Unsupported mob type '%c' in mob #%d", letter, nr);
// exit(1);
// }
//
// mob_proto[i].aff_abils = mob_proto[i].real_abils;
//
// for (j = 0; j < NUM_WEARS; j++)
// mob_proto[i].equipment[j] = NULL;
//
// mob_proto[i].nr = i;
// mob_proto[i].desc = NULL;
//
// top_of_mobt = i++;
// }
//
//
//
//
// /* read all objects from obj file; generate index and prototypes */
// char *parse_object(FILE *obj_f, int nr)
// {
// static int i = 0;
// static char line[READ_SIZE];
// int t[10], j, retval;
// char *tmpptr;
// char f1[READ_SIZE], f2[READ_SIZE], buf2[128];
// struct extra_descr_data *new_descr;
//
// obj_index[i].vnum = nr;
// obj_index[i].number = 0;
// obj_index[i].func = NULL;
//
// clear_object(obj_proto + i);
// obj_proto[i].item_number = i;
//
// sprintf(buf2, "object #%d", nr);	/* sprintf: OK (for 'buf2 >= 19') */
//
// /* *** string data *** */
// if ((obj_proto[i].name = fread_string(obj_f, buf2)) == NULL) {
// log("SYSERR: Null obj name or format error at or near %s", buf2);
// exit(1);
// }
// tmpptr = obj_proto[i].short_description = fread_string(obj_f, buf2);
// if (tmpptr && *tmpptr)
// if (!str_cmp(fname(tmpptr), "a") || !str_cmp(fname(tmpptr), "an") ||
// !str_cmp(fname(tmpptr), "the"))
// *tmpptr = LOWER(*tmpptr);
//
// tmpptr = obj_proto[i].description = fread_string(obj_f, buf2);
// if (tmpptr && *tmpptr)
// CAP(tmpptr);
// obj_proto[i].action_description = fread_string(obj_f, buf2);
//
// /* *** numeric data *** */
// if (!get_line(obj_f, line)) {
// log("SYSERR: Expecting first numeric line of %s, but file ended!", buf2);
// exit(1);
// }
// if ((retval = sscanf(line, " %d %s %s", t, f1, f2)) != 3) {
// log("SYSERR: Format error in first numeric line (expecting 3 args, got %d), %s", retval, buf2);
// exit(1);
// }
//
// /* Object flags checked in check_object(). */
// GET_OBJ_TYPE(obj_proto + i) = t[0];
// GET_OBJ_EXTRA(obj_proto + i) = asciiflag_conv(f1);
// GET_OBJ_WEAR(obj_proto + i) = asciiflag_conv(f2);
//
// if (!get_line(obj_f, line)) {
// log("SYSERR: Expecting second numeric line of %s, but file ended!", buf2);
// exit(1);
// }
// if ((retval = sscanf(line, "%d %d %d %d", t, t + 1, t + 2, t + 3)) != 4) {
// log("SYSERR: Format error in second numeric line (expecting 4 args, got %d), %s", retval, buf2);
// exit(1);
// }
// GET_OBJ_VAL(obj_proto + i, 0) = t[0];
// GET_OBJ_VAL(obj_proto + i, 1) = t[1];
// GET_OBJ_VAL(obj_proto + i, 2) = t[2];
// GET_OBJ_VAL(obj_proto + i, 3) = t[3];
//
// if (!get_line(obj_f, line)) {
// log("SYSERR: Expecting third numeric line of %s, but file ended!", buf2);
// exit(1);
// }
// if ((retval = sscanf(line, "%d %d %d", t, t + 1, t + 2)) != 3) {
// log("SYSERR: Format error in third numeric line (expecting 3 args, got %d), %s", retval, buf2);
// exit(1);
// }
// GET_OBJ_WEIGHT(obj_proto + i) = t[0];
// GET_OBJ_COST(obj_proto + i) = t[1];
// GET_OBJ_RENT(obj_proto + i) = t[2];
//
// /* check to make sure that weight of containers exceeds curr. quantity */
// if (GET_OBJ_TYPE(obj_proto + i) == ITEM_DRINKCON || GET_OBJ_TYPE(obj_proto + i) == ITEM_FOUNTAIN) {
// if (GET_OBJ_WEIGHT(obj_proto + i) < GET_OBJ_VAL(obj_proto + i, 1))
// GET_OBJ_WEIGHT(obj_proto + i) = GET_OBJ_VAL(obj_proto + i, 1) + 5;
// }
//
// /* *** extra descriptions and affect fields *** */
//
// for (j = 0; j < MAX_OBJ_AFFECT; j++) {
// obj_proto[i].affected[j].location = APPLY_NONE;
// obj_proto[i].affected[j].modifier = 0;
// }
//
// strcat(buf2, ", after numeric constants\n"	/* strcat: OK (for 'buf2 >= 87') */
// "...expecting 'E', 'A', '$', or next object number");
// j = 0;
//
// for (;;) {
// if (!get_line(obj_f, line)) {
// log("SYSERR: Format error in %s", buf2);
// exit(1);
// }
// switch (*line) {
// case 'E':
// CREATE(new_descr, struct extra_descr_data, 1);
// new_descr->keyword = fread_string(obj_f, buf2);
// new_descr->description = fread_string(obj_f, buf2);
// new_descr->next = obj_proto[i].ex_description;
// obj_proto[i].ex_description = new_descr;
// break;
// case 'A':
// if (j >= MAX_OBJ_AFFECT) {
// log("SYSERR: Too many A fields (%d max), %s", MAX_OBJ_AFFECT, buf2);
// exit(1);
// }
// if (!get_line(obj_f, line)) {
// log("SYSERR: Format error in 'A' field, %s\n"
// "...expecting 2 numeric constants but file ended!", buf2);
// exit(1);
// }
//
// if ((retval = sscanf(line, " %d %d ", t, t + 1)) != 2) {
// log("SYSERR: Format error in 'A' field, %s\n"
// "...expecting 2 numeric arguments, got %d\n"
// "...offending line: '%s'", buf2, retval, line);
// exit(1);
// }
// obj_proto[i].affected[j].location = t[0];
// obj_proto[i].affected[j].modifier = t[1];
// j++;
// break;
// case '$':
// case '#':
// check_object(obj_proto + i);
// top_of_objt = i++;
// return (line);
// default:
// log("SYSERR: Format error in (%c): %s", *line, buf2);
// exit(1);
// }
// }
// }
//
//
// #define Z	zone_table[zone]

impl DB {
    /* load the zone table and command tables */
    fn load_zones(&mut self, fl: File, zonename: &str) {
        //static zone_rnum zone = 0;
        let mut zone: zone_rnum = 0;
        let mut line_num = 0;
        let mut z = ZoneData {
            name: "".to_string(),
            lifespan: 0,
            age: 0,
            bot: 0,
            top: 0,
            reset_mode: 0,
            number: 0,
            cmd: vec![],
        };

        //        int
        //      cmd_no, num_of_cmds = 0, line_num = 0, tmp, error;
        //    char * ptr, buf[READ_SIZE], zname[READ_SIZE], buf2[MAX_STRING_LENGTH];

        let zname = zonename.clone();
        //strlcpy(zname, zonename, sizeof(zname));

        let mut buf = String::new();
        let mut reader = BufReader::new(fl);

        /* Skip first 3 lines lest we mistake the zone name for a command. */
        for tmp in 0..3 {
            reader
                .read_line(&mut buf)
                .expect("Cannot read header for zon file");
        }

        /*  More accurate count. Previous was always 4 or 5 too high. -gg 2001/1/17
         *  Note that if a new zone command is added to reset_zone(), this string
         *  will need to be updated to suit. - ae.
         */
        let mut num_of_cmds = 0;

        buf.clear();
        while reader.read_line(&mut buf).is_ok() {
            buf = buf.trim_end().to_string();
            if buf.len() == 0 {
                break;
            }
            if "MOPGERD".contains(buf.chars().into_iter().next().unwrap()) || buf == "S" {
                num_of_cmds += 1;
            }
            buf.clear();
        }

        reader.rewind().expect("Cannot rewind zone file");

        if num_of_cmds == 0 {
            error!("SYSERR: {} is empty!", zname);
            process::exit(1);
        } else {
            z.cmd.reserve_exact(num_of_cmds);
            //CREATE(Z.cmd, struct reset_com, num_of_cmds);
        }

        line_num += get_line(&mut reader, &mut buf);

        let regex = Regex::new(r"^#(\d{1,9})").unwrap();
        let f = regex.captures(buf.as_str());
        if f.is_none() {
            error!("SYSERR: Format error #1 in {}, line {}", zname, line_num);
            process::exit(1);
        }
        let f = f.unwrap();
        z.number = f[1].parse::<zone_vnum>().unwrap();

        line_num += get_line(&mut reader, &mut buf);
        let r = buf.find('~');
        if r.is_some() {
            buf.truncate(r.unwrap());
        }
        z.name = buf.clone();

        line_num += get_line(&mut reader, &mut buf);
        let regex = Regex::new(r"^(\d{1,9})\s(\d{0,9})\s(\d{0,9})\s(\d{0,9})").unwrap();
        let f = regex.captures(buf.as_str());
        if f.is_none() {
            error!(
                "SYSERR: Format error #1 in numeric constant line of {}",
                zname,
            );
            process::exit(1);
        }
        let f = f.unwrap();
        z.bot = f[1].parse::<room_vnum>().unwrap();
        z.top = f[2].parse::<room_vnum>().unwrap();
        z.lifespan = f[3].parse::<i32>().unwrap();
        z.reset_mode = f[4].parse::<i32>().unwrap();

        if z.bot > z.top {
            error!(
                "SYSERR: Zone {} bottom ({}) > top ({}).",
                z.number, z.bot, z.top
            );
            process::exit(1);
        }

        let mut cmd_no = 0;

        loop {
            let tmp = get_line(&mut reader, &mut buf);
            if tmp == 0 {
                error!("SYSERR: Format error in {} - premature end of file", zname);
                process::exit(1);
            }
            line_num += tmp;
            buf = buf.trim_start().to_string();

            let mut zcmd = ResetCom {
                command: 0 as char,
                if_flag: false,
                arg1: 0,
                arg2: 0,
                arg3: 0,
                line: 0,
            };

            let original_buf = buf.clone();
            zcmd.command = buf.remove(0);

            if zcmd.command == '*' {
                continue;
            }

            if zcmd.command == 'S' || zcmd.command == '$' {
                zcmd.command = 'S';
                break;
            }
            let mut error = 0;
            let mut tmp: i32 = -1;
            if "MOEPD".find(zcmd.command).is_none() {
                /* a 3-arg command */
                let regex = Regex::new(r"^\s(\d{1,9})\s(\d{0,9})\s(\d{0,9})").unwrap();
                let f = regex.captures(buf.as_str());
                if f.is_none() {
                    error = 1;
                } else {
                    let f = f.unwrap();
                    tmp = f[1].parse::<i32>().unwrap();
                    zcmd.arg1 = f[2].parse::<i32>().unwrap();
                    zcmd.arg2 = f[3].parse::<i32>().unwrap();
                }
            } else {
                let regex = Regex::new(r"^\s(\d{1,9})\s(\d{0,9})\s(\d{0,9})\s(\d{0,9})").unwrap();
                let f = regex.captures(buf.as_str());
                if f.is_none() {
                    error = 1;
                } else {
                    let f = f.unwrap();
                    tmp = f[1].parse::<i32>().unwrap();
                    zcmd.arg1 = f[2].parse::<i32>().unwrap();
                    zcmd.arg2 = f[3].parse::<i32>().unwrap();
                    zcmd.arg3 = f[4].parse::<i32>().unwrap();
                }
            }

            zcmd.if_flag = if tmp == 0 { false } else { true };

            if error != 0 {
                error!(
                    "SYSERR: Format error in {}, line {}: '{}'",
                    zname, line_num, original_buf
                );
                process::exit(1);
            }
            zcmd.line = line_num;
            cmd_no += 1;
            z.cmd.push(zcmd);
        }

        if num_of_cmds != cmd_no + 1 {
            error!(
                "SYSERR: Zone command count mismatch for {}. Estimated: {}, Actual: {}",
                zname,
                num_of_cmds,
                cmd_no + 1,
            );
            process::exit(1);
        }

        self.zone_table.borrow_mut().push(z);
        zone += 1;
        *self.top_of_zone_table.borrow_mut() = zone;
    }
}
// #undef Z
//
//
// void get_one_line(FILE *fl, char *buf)
// {
// if (fgets(buf, READ_SIZE, fl) == NULL) {
// log("SYSERR: error reading help file: not terminated with $?");
// exit(1);
// }
//
// buf[strlen(buf) - 1] = '\0'; /* take off the trailing \n */
// }
//
//
// void free_help(void)
// {
// int hp;
//
// if (!help_table)
// return;
//
// for (hp = 0; hp <= top_of_helpt; hp++) {
// if (help_table[hp].keyword)
// free(help_table[hp].keyword);
// if (help_table[hp].entry && !help_table[hp].duplicate)
// free(help_table[hp].entry);
// }
//
// free(help_table);
// help_table = NULL;
// top_of_helpt = 0;
// }
//
//
// void load_help(FILE *fl)
// {
// #if defined(CIRCLE_MACINTOSH)
// static char key[READ_SIZE + 1], next_key[READ_SIZE + 1], entry[32384]; /* too big for stack? */
// #else
// char key[READ_SIZE + 1], next_key[READ_SIZE + 1], entry[32384];
// #endif
// size_t entrylen;
// char line[READ_SIZE + 1], *scan;
// struct help_index_element el;
//
// /* get the first keyword line */
// get_one_line(fl, key);
// while (*key != '$') {
// strcat(key, "\r\n");	/* strcat: OK (READ_SIZE - "\n" + "\r\n" == READ_SIZE + 1) */
// entrylen = strlcpy(entry, key, sizeof(entry));
//
// /* read in the corresponding help entry */
// get_one_line(fl, line);
// while (*line != '#' && entrylen < sizeof(entry) - 1) {
// entrylen += strlcpy(entry + entrylen, line, sizeof(entry) - entrylen);
//
// if (entrylen + 2 < sizeof(entry) - 1) {
// strcpy(entry + entrylen, "\r\n");	/* strcpy: OK (size checked above) */
// entrylen += 2;
// }
// get_one_line(fl, line);
// }
//
// if (entrylen >= sizeof(entry) - 1) {
// int keysize;
// const char *truncmsg = "\r\n*TRUNCATED*\r\n";
//
// strcpy(entry + sizeof(entry) - strlen(truncmsg) - 1, truncmsg);	/* strcpy: OK (assuming sane 'entry' size) */
//
// keysize = strlen(key) - 2;
// log("SYSERR: Help entry exceeded buffer space: %.*s", keysize, key);
//
// /* If we ran out of buffer space, eat the rest of the entry. */
// while (*line != '#')
// get_one_line(fl, line);
// }
//
// /* now, add the entry to the index with each keyword on the keyword line */
// el.duplicate = 0;
// el.entry = strdup(entry);
// scan = one_word(key, next_key);
// while (*next_key) {
// el.keyword = strdup(next_key);
// help_table[top_of_helpt++] = el;
// el.duplicate++;
// scan = one_word(scan, next_key);
// }
//
// /* get next keyword line (or $) */
// get_one_line(fl, key);
// }
// }
//
//
// int hsort(const void *a, const void *b)
// {
// const struct help_index_element *a1, *b1;
//
// a1 = (const struct help_index_element *) a;
// b1 = (const struct help_index_element *) b;
//
// return (str_cmp(a1->keyword, b1->keyword));
// }
//
//
// /*************************************************************************
// *  procedures for resetting, both play-time and boot-time	 	 *
// *************************************************************************/
//
//
// int vnum_mobile(char *searchname, struct char_data *ch)
// {
// int nr, found = 0;
//
// for (nr = 0; nr <= top_of_mobt; nr++)
// if (isname(searchname, mob_proto[nr].player.name))
// send_to_char(ch, "%3d. [%5d] %s\r\n", ++found, mob_index[nr].vnum, mob_proto[nr].player.short_descr);
//
// return (found);
// }
//
//
//
// int vnum_object(char *searchname, struct char_data *ch)
// {
// int nr, found = 0;
//
// for (nr = 0; nr <= top_of_objt; nr++)
// if (isname(searchname, obj_proto[nr].name))
// send_to_char(ch, "%3d. [%5d] %s\r\n", ++found, obj_index[nr].vnum, obj_proto[nr].short_description);
//
// return (found);
// }
//
//
// /* create a character, and add it to the char list */
// struct char_data *create_char(void)
// {
// struct char_data *ch;
//
// CREATE(ch, struct char_data, 1);
// clear_char(ch);
// ch->next = character_list;
// character_list = ch;
//
// return (ch);
// }
//
//
// /* create a new mobile from a prototype */
// struct char_data *read_mobile(mob_vnum nr, int type) /* and mob_rnum */
// {
// mob_rnum i;
// struct char_data *mob;
//
// if (type == VIRTUAL) {
// if ((i = real_mobile(nr)) == NOBODY) {
// log("WARNING: Mobile vnum %d does not exist in database.", nr);
// return (NULL);
// }
// } else
// i = nr;
//
// CREATE(mob, struct char_data, 1);
// clear_char(mob);
// *mob = mob_proto[i];
// mob->next = character_list;
// character_list = mob;
//
// if (!mob->points.max_hit) {
// mob->points.max_hit = dice(mob->points.hit, mob->points.mana) +
// mob->points.move;
// } else
// mob->points.max_hit = rand_number(mob->points.hit, mob->points.mana);
//
// mob->points.hit = mob->points.max_hit;
// mob->points.mana = mob->points.max_mana;
// mob->points.move = mob->points.max_move;
//
// mob->player.time.birth = time(0);
// mob->player.time.played = 0;
// mob->player.time.logon = time(0);
//
// mob_index[i].number++;
//
// return (mob);
// }
//
//
// /* create an object, and add it to the object list */
// struct obj_data *create_obj(void)
// {
// struct obj_data *obj;
//
// CREATE(obj, struct obj_data, 1);
// clear_object(obj);
// obj->next = object_list;
// object_list = obj;
//
// return (obj);
// }
//
//
// /* create a new object from a prototype */
// struct obj_data *read_object(obj_vnum nr, int type) /* and obj_rnum */
// {
// struct obj_data *obj;
// obj_rnum i = type == VIRTUAL ? real_object(nr) : nr;
//
// if (i == NOTHING || i > top_of_objt) {
// log("Object (%c) %d does not exist in database.", type == VIRTUAL ? 'V' : 'R', nr);
// return (NULL);
// }
//
// CREATE(obj, struct obj_data, 1);
// clear_object(obj);
// *obj = obj_proto[i];
// obj->next = object_list;
// object_list = obj;
//
// obj_index[i].number++;
//
// return (obj);
// }
//
//
//
// #define ZO_DEAD  999
//
// /* update zone ages, queue for reset if necessary, and dequeue when possible */
// void zone_update(void)
// {
// int i;
// struct reset_q_element *update_u, *temp;
// static int timer = 0;
//
// /* jelson 10/22/92 */
// if (((++timer * PULSE_ZONE) / PASSES_PER_SEC) >= 60) {
// /* one minute has passed */
// /*
//  * NOT accurate unless PULSE_ZONE is a multiple of PASSES_PER_SEC or a
//  * factor of 60
//  */
//
// timer = 0;
//
// /* since one minute has passed, increment zone ages */
// for (i = 0; i <= top_of_zone_table; i++) {
// if (zone_table[i].age < zone_table[i].lifespan &&
// zone_table[i].reset_mode)
// (zone_table[i].age)++;
//
// if (zone_table[i].age >= zone_table[i].lifespan &&
// zone_table[i].age < ZO_DEAD && zone_table[i].reset_mode) {
// /* enqueue zone */
//
// CREATE(update_u, struct reset_q_element, 1);
//
// update_u->zone_to_reset = i;
// update_u->next = 0;
//
// if (!reset_q.head)
// reset_q.head = reset_q.tail = update_u;
// else {
// reset_q.tail->next = update_u;
// reset_q.tail = update_u;
// }
//
// zone_table[i].age = ZO_DEAD;
// }
// }
// }	/* end - one minute has passed */
//
//
// /* dequeue zones (if possible) and reset */
// /* this code is executed every 10 seconds (i.e. PULSE_ZONE) */
// for (update_u = reset_q.head; update_u; update_u = update_u->next)
// if (zone_table[update_u->zone_to_reset].reset_mode == 2 ||
// is_empty(update_u->zone_to_reset)) {
// reset_zone(update_u->zone_to_reset);
// mudlog(CMP, LVL_GOD, FALSE, "Auto zone reset: %s", zone_table[update_u->zone_to_reset].name);
// /* dequeue */
// if (update_u == reset_q.head)
// reset_q.head = reset_q.head->next;
// else {
// for (temp = reset_q.head; temp->next != update_u;
// temp = temp->next);
//
// if (!update_u->next)
// reset_q.tail = temp;
//
// temp->next = update_u->next;
// }
//
// free(update_u);
// break;
// }
// }
//
// void log_zone_error(zone_rnum zone, int cmd_no, const char *message)
// {
// mudlog(NRM, LVL_GOD, TRUE, "SYSERR: zone file: %s", message);
// mudlog(NRM, LVL_GOD, TRUE, "SYSERR: ...offending cmd: '%c' cmd in zone #%d, line %d",
// ZCMD.command, zone_table[zone].number, ZCMD.line);
// }

// #define ZONE_ERROR(message) \
// { log_zone_error(zone, cmd_no, message); last_cmd = 0; }
//
// /* execute the reset command table of a given zone */
// void reset_zone(zone_rnum zone)
// {
// int cmd_no, last_cmd = 0;
// struct char_data *mob = NULL;
// struct obj_data *obj, *obj_to;
//
// for (cmd_no = 0; ZCMD.command != 'S'; cmd_no++) {
//
// if (ZCMD.if_flag && !last_cmd)
// continue;
//
// /*  This is the list of actual zone commands.  If any new
//  *  zone commands are added to the game, be certain to update
//  *  the list of commands in load_zone() so that the counting
//  *  will still be correct. - ae.
//  */
// switch (ZCMD.command) {
// case '*':			/* ignore command */
// last_cmd = 0;
// break;
//
// case 'M':			/* read a mobile */
// if (mob_index[ZCMD.arg1].number < ZCMD.arg2) {
// mob = read_mobile(ZCMD.arg1, REAL);
// char_to_room(mob, ZCMD.arg3);
// last_cmd = 1;
// } else
// last_cmd = 0;
// break;
//
// case 'O':			/* read an object */
// if (obj_index[ZCMD.arg1].number < ZCMD.arg2) {
// if (ZCMD.arg3 != NOWHERE) {
// obj = read_object(ZCMD.arg1, REAL);
// obj_to_room(obj, ZCMD.arg3);
// last_cmd = 1;
// } else {
// obj = read_object(ZCMD.arg1, REAL);
// IN_ROOM(obj) = NOWHERE;
// last_cmd = 1;
// }
// } else
// last_cmd = 0;
// break;
//
// case 'P':			/* object to object */
// if (obj_index[ZCMD.arg1].number < ZCMD.arg2) {
// obj = read_object(ZCMD.arg1, REAL);
// if (!(obj_to = get_obj_num(ZCMD.arg3))) {
// ZONE_ERROR("target obj not found, command disabled");
// ZCMD.command = '*';
// break;
// }
// obj_to_obj(obj, obj_to);
// last_cmd = 1;
// } else
// last_cmd = 0;
// break;
//
// case 'G':			/* obj_to_char */
// if (!mob) {
// ZONE_ERROR("attempt to give obj to non-existant mob, command disabled");
// ZCMD.command = '*';
// break;
// }
// if (obj_index[ZCMD.arg1].number < ZCMD.arg2) {
// obj = read_object(ZCMD.arg1, REAL);
// obj_to_char(obj, mob);
// last_cmd = 1;
// } else
// last_cmd = 0;
// break;
//
// case 'E':			/* object to equipment list */
// if (!mob) {
// ZONE_ERROR("trying to equip non-existant mob, command disabled");
// ZCMD.command = '*';
// break;
// }
// if (obj_index[ZCMD.arg1].number < ZCMD.arg2) {
// if (ZCMD.arg3 < 0 || ZCMD.arg3 >= NUM_WEARS) {
// ZONE_ERROR("invalid equipment pos number");
// } else {
// obj = read_object(ZCMD.arg1, REAL);
// equip_char(mob, obj, ZCMD.arg3);
// last_cmd = 1;
// }
// } else
// last_cmd = 0;
// break;
//
// case 'R': /* rem obj from room */
// if ((obj = get_obj_in_list_num(ZCMD.arg2, world[ZCMD.arg1].contents)) != NULL)
// extract_obj(obj);
// last_cmd = 1;
// break;
//
//
// case 'D':			/* set state of door */
// if (ZCMD.arg2 < 0 || ZCMD.arg2 >= NUM_OF_DIRS ||
// (world[ZCMD.arg1].dir_option[ZCMD.arg2] == NULL)) {
// ZONE_ERROR("door does not exist, command disabled");
// ZCMD.command = '*';
// } else
// switch (ZCMD.arg3) {
// case 0:
// REMOVE_BIT(world[ZCMD.arg1].dir_option[ZCMD.arg2]->exit_info,
// EX_LOCKED);
// REMOVE_BIT(world[ZCMD.arg1].dir_option[ZCMD.arg2]->exit_info,
// EX_CLOSED);
// break;
// case 1:
// SET_BIT(world[ZCMD.arg1].dir_option[ZCMD.arg2]->exit_info,
// EX_CLOSED);
// REMOVE_BIT(world[ZCMD.arg1].dir_option[ZCMD.arg2]->exit_info,
// EX_LOCKED);
// break;
// case 2:
// SET_BIT(world[ZCMD.arg1].dir_option[ZCMD.arg2]->exit_info,
// EX_LOCKED);
// SET_BIT(world[ZCMD.arg1].dir_option[ZCMD.arg2]->exit_info,
// EX_CLOSED);
// break;
// }
// last_cmd = 1;
// break;
//
// default:
// ZONE_ERROR("unknown cmd in reset table; cmd disabled");
// ZCMD.command = '*';
// break;
// }
// }
//
// zone_table[zone].age = 0;
// }
//
//
//
// /* for use in reset_zone; return TRUE if zone 'nr' is free of PC's  */
// int is_empty(zone_rnum zone_nr)
// {
// struct descriptor_data *i;
//
// for (i = descriptor_list; i; i = i->next) {
// if (STATE(i) != CON_PLAYING)
// continue;
// if (IN_ROOM(i->character) == NOWHERE)
// continue;
// if (GET_LEVEL(i->character) >= LVL_IMMORT)
// continue;
// if (world[IN_ROOM(i->character)].zone != zone_nr)
// continue;
//
// return (0);
// }
//
// return (1);
// }

/*************************************************************************
*  stuff related to the save/load player system				 *
*************************************************************************/

impl DB {
    fn get_ptable_by_name(&self, name: &str) -> Option<usize> {
        return self
            .player_table
            .borrow()
            .iter()
            .position(|pie| pie.name == name);
    }
}

// long get_id_by_name(const char *name)
// {
// int i;
//
// for (i = 0; i <= top_of_p_table; i++)
// if (!str_cmp(player_table[i].name, name))
// return (player_table[i].id);
//
// return (-1);
// }
//
//
// char *get_name_by_id(long id)
// {
// int i;
//
// for (i = 0; i <= top_of_p_table; i++)
// if (player_table[i].id == id)
// return (player_table[i].name);
//
// return (NULL);
// }
use crate::config::{FROZEN_START_ROOM, IMMORT_START_ROOM, MORTAL_START_ROOM};
use crate::constants::ROOM_BITS_COUNT;
use std::io::Read;

impl DB {
    /* Load a char, TRUE if loaded, FALSE if not */
    pub fn load_char(&self, name: &str, char_element: &mut CharFileU) -> Option<usize> {
        let player_i = self.get_ptable_by_name(name);
        if player_i.is_none() {
            return player_i;
        }
        let player_i = player_i.unwrap();
        let mut t = self.player_fl.borrow_mut();
        let mut pfile = t.as_mut().unwrap();

        let record_size = mem::size_of::<CharFileU>();
        pfile
            .seek(SeekFrom::Start((player_i * record_size) as u64))
            .expect("Error while reading player file");
        unsafe {
            let config_slice =
                slice::from_raw_parts_mut(char_element as *mut _ as *mut u8, record_size);
            // `read_exact()` comes from `Read` impl for `&[u8]`
            pfile.read_exact(config_slice).unwrap();
        }
        return Some(player_i);
    }

    /*
     * write the vital data of a player to the player file
     *
     * And that's it! No more fudging around with the load room.
     * Unfortunately, 'host' modifying is still here due to lack
     * of that variable in the char_data structure.
     */
    pub fn save_char(&self, ch: &CharData) {
        //struct char_file_u st;
        let mut st: CharFileU = CharFileU {
            name: [0; MAX_NAME_LENGTH + 1],
            description: [0; 240],
            title: [0; MAX_TITLE_LENGTH + 1],
            sex: 0,
            chclass: 0,
            level: 0,
            hometown: 0,
            birth: 0,
            played: 0,
            weight: 0,
            height: 0,
            pwd: [0; MAX_PWD_LENGTH],
            char_specials_saved: CharSpecialDataSaved {
                alignment: 0,
                idnum: 0,
                act: 0,
                affected_by: 0,
                apply_saving_throw: [0; 5],
            },
            player_specials_saved: PlayerSpecialDataSaved {
                skills: [0; MAX_SKILLS + 1],
                padding0: 0,
                talks: [false; MAX_TONGUE],
                wimp_level: 0,
                freeze_level: 0,
                invis_level: 0,
                load_room: 0,
                pref: 0,
                bad_pws: 0,
                conditions: [0; 3],
                spare0: 0,
                spare1: 0,
                spare2: 0,
                spare3: 0,
                spare4: 0,
                spare5: 0,
                spells_to_learn: 0,
                spare7: 0,
                spare8: 0,
                spare9: 0,
                spare10: 0,
                spare11: 0,
                spare12: 0,
                spare13: 0,
                spare14: 0,
                spare15: 0,
                spare16: 0,
                spare17: 0,
                spare18: 0,
                spare19: 0,
                spare20: 0,
                spare21: 0,
            },
            abilities: CharAbilityData {
                str: 0,
                str_add: 0,
                intel: 0,
                wis: 0,
                dex: 0,
                con: 0,
                cha: 0,
            },
            points: CharPointData {
                mana: 0,
                max_mana: 0,
                hit: 0,
                max_hit: 0,
                movem: 0,
                max_move: 0,
                armor: 0,
                gold: 0,
                bank_gold: 0,
                exp: 0,
                hitroll: 0,
                damroll: 0,
            },
            affected: [AffectedType {
                _type: 0,
                duration: 0,
                modifier: 0,
                location: 0,
                bitvector: 0,
            }; MAX_AFFECT],
            last_logon: 0,
            host: [0; HOST_LENGTH + 1],
        };

        if ch.is_npc() || ch.desc.borrow().is_none() || ch.get_pfilepos() < 0 {
            return;
        }

        char_to_store(ch, &mut st);

        {
            copy_to_stored(
                &mut st.host,
                ch.desc.borrow().as_ref().unwrap().host.borrow().as_str(),
            );
        }
        // strncpy(st.host, ch -> desc -> host, HOST_LENGTH);    /* strncpy: OK (s.host:HOST_LENGTH+1) */
        // st.host[HOST_LENGTH] = '\0';

        let record_size = mem::size_of::<CharFileU>();
        //self.player_fl.fseek(SeekFrom::Start((get_pfilepos!(ch) * record_size) as u64)).expect("Error while seeking for writing player");
        unsafe {
            let player_slice = slice::from_raw_parts(&mut st as *mut _ as *mut u8, record_size);
            self.player_fl
                .borrow_mut()
                .as_mut()
                .unwrap()
                .write_all_at(
                    player_slice,
                    (ch.get_pfilepos() * record_size as i32) as u64,
                )
                .expect("Error while writing player record to file");
        }
    }
}

/* copy data from the file structure to a char struct */
pub fn store_to_char(st: &CharFileU, ch: &CharData) {
    //int i;

    /* to save memory, only PC's -- not MOB's -- have player_specials */
    // if (ch->player_specials == NULL)
    // CREATE(ch->player_specials, struct player_special_data, 1);

    ch.set_sex(st.sex);
    ch.set_class(st.chclass);
    ch.set_level(st.level);

    ch.player.borrow_mut().short_descr = String::new();
    ch.player.borrow_mut().long_descr = String::new();
    //ch.player.title = st.title;
    ch.player.borrow_mut().description = std::str::from_utf8(&st.description).unwrap().to_string();

    ch.player.borrow_mut().hometown = st.hometown;
    ch.player.borrow_mut().time.birth = st.birth;
    ch.player.borrow_mut().time.played = st.played;
    ch.player.borrow_mut().time.logon = time_now();

    ch.player.borrow_mut().weight = st.weight;
    ch.player.borrow_mut().height = st.height;

    *ch.real_abils.borrow_mut() = st.abilities;
    *ch.aff_abils.borrow_mut() = st.abilities;
    *ch.points.borrow_mut() = st.points;
    ch.char_specials.borrow_mut().saved = st.char_specials_saved;
    RefCell::borrow_mut(&ch.player_specials).saved = st.player_specials_saved;
    // POOFIN(ch) = NULL;
    // POOFOUT(ch) = NULL;
    // GET_LAST_TELL(ch) = NOBODY;

    if ch.points.borrow().max_mana < 100 {
        ch.points.borrow_mut().max_mana = 100;
    }

    ch.char_specials.borrow_mut().carry_weight = 0;
    ch.char_specials.borrow_mut().carry_items = 0;
    ch.points.borrow_mut().armor = 100;
    ch.points.borrow_mut().hitroll = 0;
    ch.points.borrow_mut().damroll = 0;

    // if (ch.player.name)
    // free(ch.player.name);
    ch.player.borrow_mut().name = std::str::from_utf8(&st.name)
        .expect("Error while loading player name from file")
        .parse()
        .unwrap();
    ch.player.borrow_mut().passwd.copy_from_slice(&st.pwd);

    /* Add all spell effects */
    for i in 0..MAX_AFFECT {
        if st.affected[i]._type != 0 {
            ch.affected.borrow_mut().push(st.affected[i]);
        }
    }

    /*
     * If you're not poisioned and you've been away for more than an hour of
     * real time, we'll set your HMV back to full
     */

    if !ch.aff_flagged(AFF_POISON) && time_now() - st.last_logon >= SECS_PER_REAL_HOUR {
        ch.set_hit(ch.get_max_hit());
        ch.set_move(ch.get_max_move());
        ch.set_mana(ch.get_max_mana());
    }
} /* store_to_char */

/* copy vital data from a players char-structure to the file structure */
fn char_to_store(ch: &CharData, st: &mut CharFileU) {
    //int i;
    //struct affected_type *af;
    //struct obj_data *char_eq[NUM_WEARS];

    /* Unaffect everything a character can be affected by */

    // for (i = 0; i < NUM_WEARS; i++) {
    // if (GET_EQ(ch, i))
    // char_eq[i] = unequip_char(ch, i);
    // else
    // char_eq[i] = NULL;
    // }
    if ch.affected.borrow().len() > MAX_AFFECT {
        error!("SYSERR: WARNING: OUT OF STORE ROOM FOR AFFECTED TYPES!!!");
    }

    for i in 0..MAX_AFFECT {
        let a = ch.affected.borrow();
        let af = a.get(i);
        if af.is_some() {
            let af = af.unwrap();
            st.affected[i] = *af;
        } else {
            st.affected[i]._type = 0; /* Zero signifies not used */
            st.affected[i].duration = 0;
            st.affected[i].modifier = 0;
            st.affected[i].location = 0;
            st.affected[i].bitvector = 0;
        }
    }

    /*
     * remove the affections so that the raw values are stored; otherwise the
     * effects are doubled when the char logs back in.
     */

    ch.affected.borrow_mut().clear();
    // while (ch->affected)
    // affect_remove(ch, ch->affected);

    *ch.aff_abils.borrow_mut() = *ch.real_abils.borrow();

    st.birth = ch.player.borrow().time.birth;
    st.played = ch.player.borrow().time.played;
    st.played += (time_now() - ch.player.borrow().time.logon) as i32;
    st.last_logon = time_now();

    ch.player.borrow_mut().time.played = st.played;
    ch.player.borrow_mut().time.logon = time_now();

    st.hometown = ch.player.borrow().hometown;
    st.weight = ch.get_weight();
    st.height = ch.get_height();
    st.sex = ch.get_sex();
    st.chclass = ch.get_class();
    st.level = ch.get_level();
    st.abilities = *ch.real_abils.borrow();
    st.points = *ch.points.borrow();
    st.char_specials_saved = ch.char_specials.borrow().saved;
    st.player_specials_saved = RefCell::borrow(&ch.player_specials).saved;

    st.points.armor = 100;
    st.points.hitroll = 0;
    st.points.damroll = 0;

    // if (GET_TITLE(ch)) {
    //     strlcpy(st.title, GET_TITLE(ch), MAX_TITLE_LENGTH);
    // } else {
    //     *st.title = '\0';
    // }
    if !ch.player.borrow().description.is_empty() {
        if ch.player.borrow().description.len() >= st.description.len() {
            error!(
                "SYSERR: char_to_store: {}'s description length: {}, max: {}!  Truncated.",
                ch.get_pc_name(),
                ch.player.borrow().description.len(),
                st.description.len()
            );
            ch.player
                .borrow_mut()
                .description
                .truncate(st.description.len() - 3);
            ch.player.borrow_mut().description.push_str("\r\n");
        }
        copy_to_stored(&mut st.description, &ch.player.borrow().description);
        //strcpy(st.description, ch.player.description);    /* strcpy: OK (checked above) */
    } else {
        st.description[0] = 0;
    }
    copy_to_stored(&mut st.name, ch.get_name().as_ref());
    st.pwd.copy_from_slice(&ch.get_passwd());

    /* add spell and eq affections back in now */
    for i in 0..MAX_AFFECT {
        if st.affected[i]._type != 0 {
            ch.affected.borrow_mut().push(st.affected[i]);
        }
    }

    // for (i = 0; i < NUM_WEARS; i+ +) {
    //     if (char_eq[i]) {
    //         equip_char(ch, char_eq[i], i);
    //     }
    // }
    /*   affect_total(ch); unnecessary, I think !?! */
} /* Char to store */

fn copy_to_stored(to: &mut [u8], from: &str) {
    let bytes = from.as_bytes();
    let bytes_copied = min(to.len(), from.len());
    to[0..bytes_copied].copy_from_slice(&bytes[0..bytes_copied]);
    if bytes_copied != to.len() {
        to[bytes_copied] = 0;
    }
}

// void save_etext(struct char_data *ch)
// {
// /* this will be really cool soon */
// }

/*
 * Create a new entry in the in-memory index table for the player file.
 * If the name already exists, by overwriting a deleted character, then
 * we re-use the old position.
 */
impl DB {
    pub(crate) fn create_entry(&self, name: &str) -> usize {
        //int i, pos;
        let i: usize;
        let pos = self.get_ptable_by_name(name);

        if pos.is_none() {
            /* new name */
            i = self.player_table.borrow().len();
            self.player_table.borrow_mut().push(PlayerIndexElement {
                name: name.to_lowercase(),
                id: i as i64,
            });
            return i;
        } else {
            let pos = pos.unwrap();

            let mut pt = self.player_table.borrow_mut();
            let mut pie = pt.get_mut(pos);
            pie.as_mut().unwrap().name = name.to_lowercase();
            return pos;
        }
    }
}

/************************************************************************
*  funcs of a (more or less) general utility nature			*
************************************************************************/

/* read and allocate space for a '~'-terminated string from a given file */
fn fread_string(reader: &mut BufReader<File>, error: &str) -> String {
    let mut buf = String::new();
    let mut tmp = String::new();
    // char buf[MAX_STRING_LENGTH], tmp[513];
    // char *point;
    // int done = 0, length = 0, templength;
    //
    // *buf = '\0';
    let mut done = false;
    loop {
        tmp.clear();
        let r = reader.read_line(&mut tmp);
        if r.is_err() {
            error!(
                "SYSERR: fread_string: format error at or near {}: {}",
                error,
                r.err().unwrap()
            );
            process::exit(1);
        }

        /* If there is a '~', end the string; else put an "\r\n" over the '\n'. */
        let point = tmp.find('~');
        if point.is_some() {
            tmp.truncate(point.unwrap());
            done = true;
        } else {
        }

        buf.push_str(tmp.as_str());
        // templength = strlen(tmp);

        // if (length + templength >= MAX_STRING_LENGTH) {
        // log("SYSERR: fread_string: string too large (db.c)");
        // log("%s", error);
        // exit(1);
        // } else {
        // strcat(buf + length, tmp);	/* strcat: OK (size checked above) */
        // length += templength;
        // }
        if done {
            break;
        }
    }

    /* allocate space for the new string and copy it */
    return buf;
    //return (strlen(buf)?;
    //strdup(buf): NULL);
}

// /* release memory allocated for a char struct */
// void free_char(struct char_data *ch)
// {
// int i;
// struct alias_data *a;
//
// if (ch->player_specials != NULL && ch->player_specials != &dummy_mob) {
// while ((a = GET_ALIASES(ch)) != NULL) {
// GET_ALIASES(ch) = (GET_ALIASES(ch))->next;
// free_alias(a);
// }
// if (ch->player_specials->poofin)
// free(ch->player_specials->poofin);
// if (ch->player_specials->poofout)
// free(ch->player_specials->poofout);
// free(ch->player_specials);
// if (IS_NPC(ch))
// log("SYSERR: Mob %s (#%d) had player_specials allocated!", GET_NAME(ch), GET_MOB_VNUM(ch));
// }
// if (!IS_NPC(ch) || (IS_NPC(ch) && GET_MOB_RNUM(ch) == NOBODY)) {
// /* if this is a player, or a non-prototyped non-player, free all */
// if (GET_NAME(ch))
// free(GET_NAME(ch));
// if (ch->player.title)
// free(ch->player.title);
// if (ch->player.short_descr)
// free(ch->player.short_descr);
// if (ch->player.long_descr)
// free(ch->player.long_descr);
// if (ch->player.description)
// free(ch->player.description);
// } else if ((i = GET_MOB_RNUM(ch)) != NOBODY) {
// /* otherwise, free strings only if the string is not pointing at proto */
// if (ch->player.name && ch->player.name != mob_proto[i].player.name)
// free(ch->player.name);
// if (ch->player.title && ch->player.title != mob_proto[i].player.title)
// free(ch->player.title);
// if (ch->player.short_descr && ch->player.short_descr != mob_proto[i].player.short_descr)
// free(ch->player.short_descr);
// if (ch->player.long_descr && ch->player.long_descr != mob_proto[i].player.long_descr)
// free(ch->player.long_descr);
// if (ch->player.description && ch->player.description != mob_proto[i].player.description)
// free(ch->player.description);
// }
// while (ch->affected)
// affect_remove(ch, ch->affected);
//
// if (ch->desc)
// ch->desc->character = NULL;
//
// free(ch);
// }
//
//
//
//
// /* release memory allocated for an obj struct */
// void free_obj(struct obj_data *obj)
// {
// int nr;
//
// if ((nr = GET_OBJ_RNUM(obj)) == NOTHING) {
// if (obj->name)
// free(obj->name);
// if (obj->description)
// free(obj->description);
// if (obj->short_description)
// free(obj->short_description);
// if (obj->action_description)
// free(obj->action_description);
// if (obj->ex_description)
// free_extra_descriptions(obj->ex_description);
// } else {
// if (obj->name && obj->name != obj_proto[nr].name)
// free(obj->name);
// if (obj->description && obj->description != obj_proto[nr].description)
// free(obj->description);
// if (obj->short_description && obj->short_description != obj_proto[nr].short_description)
// free(obj->short_description);
// if (obj->action_description && obj->action_description != obj_proto[nr].action_description)
// free(obj->action_description);
// if (obj->ex_description && obj->ex_description != obj_proto[nr].ex_description)
// free_extra_descriptions(obj->ex_description);
// }
//
// free(obj);
// }
//
//

/*
 * Steps:
 *   1: Read contents of a text file.
 *   2: Make sure no one is using the pointer in paging.
 *   3: Allocate space.
 *   4: Point 'buf' to it.
 *
 * We don't want to free() the string that someone may be
 * viewing in the pager.  page_string() keeps the internal
 * strdup()'d copy on ->showstr_head and it won't care
 * if we delete the original.  Otherwise, strings are kept
 * on ->showstr_vector but we'll only match if the pointer
 * is to the string we're interested in and not a copy.
 *
 * If someone is reading a global copy we're trying to
 * replace, give everybody using it a different copy so
 * as to avoid special cases.
 */
impl MainGlobals {
    fn file_to_string_alloc<'a>(&self, name: &'a str, buf: &'a mut String) -> i32 {
        //int temppage;
        //char temp[MAX_STRING_LENGTH];
        //struct descriptor_data *in_use;

        for in_use in &*self.descriptor_list.borrow() {
            if RefCell::borrow(&in_use.showstr_vector.borrow()[0]).as_str() == buf {
                return -1;
            }
        }

        /* Lets not free() what used to be there unless we succeeded. */
        let r = file_to_string(name);
        if r.is_err() {
            return -1;
        }
        let temp = r.unwrap();

        for in_use in &*self.descriptor_list.borrow() {
            // if (!in_use->showstr_count || *in_use->showstr_vector != *buf)
            // continue;
            if *RefCell::borrow(&in_use.showstr_count) == 0
                || RefCell::borrow(&RefCell::borrow(&in_use.showstr_vector)[0]).as_str() != buf
            {
                continue;
            }

            let temppage = RefCell::borrow(&in_use.showstr_page);
            *RefCell::borrow_mut(&in_use.showstr_head) = in_use.showstr_vector.borrow()[0].clone();
            *RefCell::borrow_mut(&in_use.showstr_page) = *temppage;
            paginate_string(
                RefCell::borrow(&in_use.showstr_head.borrow()).as_str(),
                in_use,
            );
        }
        *buf = temp;
        return 0;
    }
}

/* read contents of a text file, and place in buf */
fn file_to_string(name: &str) -> io::Result<String> {
    let r = fs::read_to_string(name);
    if r.is_err() {
        error!("SYSERR: reading {}: {}", name, r.as_ref().err().unwrap());
    }
    return r;
}

// FILE *fl;
// char tmp[READ_SIZE + 3];
// int len;
//
// *buf = '\0';
//
// if (!(fl = fopen(name, "r"))) {
// log("SYSERR: reading %s: %s", name, strerror(errno));
// return (-1);
// }
//
// for (;;) {
// if (!fgets(tmp, READ_SIZE, fl))	/* EOF check */
// break;
// if ((len = strlen(tmp)) > 0)
// tmp[len - 1] = '\0'; /* take off the trailing \n */
// strcat(tmp, "\r\n");	/* strcat: OK (tmp:READ_SIZE+3) */
//
// if (strlen(buf) + strlen(tmp) + 1 > MAX_STRING_LENGTH) {
// log("SYSERR: %s: string too big (%d max)", name, MAX_STRING_LENGTH);
// *buf = '\0';
// fclose(fl);
// return (-1);
// }
// strcat(buf, tmp);	/* strcat: OK (size checked above) */
// }
//
// fclose(fl);
//
// return (0);
//}

/* clear some of the the working variables of a char */
pub fn reset_char(ch: &CharData) {
    // TODO implement WEAR
    // for (i = 0; i < NUM_WEARS; i++)
    // GET_EQ(ch, i) = NULL;

    ch.followers.borrow_mut().clear();
    *ch.master.borrow_mut() = None;
    ch.set_in_room(NOWHERE);
    // TODO implement carrying
    //ch->carrying = NULL;
    *ch.next.borrow_mut() = None;
    *ch.next_fighting.borrow_mut() = None;
    *ch.next_in_room.borrow_mut() = None;
    ch.set_fighting(None);
    ch.char_specials.borrow_mut().position = POS_STANDING;
    ch.mob_specials.borrow_mut().default_pos = POS_STANDING;
    ch.char_specials.borrow_mut().carry_weight = 0;
    ch.char_specials.borrow_mut().carry_items = 0;

    if ch.get_hit() <= 0 {
        ch.set_hit(1);
    }
    if ch.get_move() <= 0 {
        ch.set_move(1);
    }
    if ch.get_mana() <= 0 {
        ch.set_mana(1);
    }

    get_last_tell_mut!(ch) = NOBODY as i64;
}

// /* clear ALL the working variables of a char; do NOT free any space alloc'ed */
// void clear_char(struct char_data *ch)
// {
// memset((char *) ch, 0, sizeof(struct char_data));
//
// IN_ROOM(ch) = NOWHERE;
// GET_PFILEPOS(ch) = -1;
// GET_MOB_RNUM(ch) = NOBODY;
// GET_WAS_IN(ch) = NOWHERE;
// GET_POS(ch) = POS_STANDING;
// ch->mob_specials.default_pos = POS_STANDING;
//
// GET_AC(ch) = 100;		/* Basic Armor */
// if (ch->points.max_mana < 100)
// ch->points.max_mana = 100;
// }
//
//
// void clear_object(struct obj_data *obj)
// {
// memset((char *) obj, 0, sizeof(struct obj_data));
//
// obj->item_number = NOTHING;
// IN_ROOM(obj) = NOWHERE;
// obj->worn_on = NOWHERE;
// }

/*
 * Called during character creation after picking character class
 * (and then never again for that character).
 */
impl DB {
    pub(crate) fn init_char(&self, ch: &CharData) {
        let i: i32;

        /* create a player_special structure */
        // if ch.player_specials
        // CREATE(ch->player_specials, struct player_special_data, 1);

        /* *** if this is our first player --- he be God *** */
        if *self.top_of_p_table.borrow() == 0 {
            ch.set_level(LVL_IMPL as u8);
            ch.set_exp(7000000);

            /* The implementor never goes through do_start(). */
            ch.set_max_hit(500);
            ch.set_max_mana(100);
            ch.set_max_move(82);
            ch.set_hit(ch.get_max_hit());
            ch.set_mana(ch.get_max_mana());
            ch.set_move(ch.get_max_move());
        }

        //set_title(ch, NULL);
        ch.player.borrow_mut().short_descr = String::new();
        ch.player.borrow_mut().long_descr = String::new();
        ch.player.borrow_mut().description = String::new();

        let now = time_now();
        ch.player.borrow_mut().time.birth = now;
        ch.player.borrow_mut().time.logon = now;
        ch.player.borrow_mut().time.played = 0;

        ch.set_home(1);
        ch.set_ac(100);

        for i in 0..MAX_TONGUE {
            ch.set_talk_mut(i, false);
        }

        /*
         * make favors for sex -- or in English, we bias the height and weight of the
         * character depending on what gender they've chosen for themselves. While it
         * is possible to have a tall, heavy female it's not as likely as a male.
         *
         * Height is in centimeters. Weight is in pounds.  The only place they're
         * ever printed (in stock code) is SPELL_IDENTIFY.
         */
        if ch.get_sex() == SEX_MALE {
            ch.set_weight(rand_number(120, 180) as u8);
            ch.set_height(rand_number(160, 200) as u8); /* 5'4" - 6'8" */
        } else {
            ch.set_weight(rand_number(100, 160) as u8);
            ch.set_height(rand_number(150, 180) as u8); /* 5'0" - 6'0" */
        }

        let i = self.get_ptable_by_name(ch.get_name().as_ref());
        if i.is_none() {
            error!(
                "SYSERR: init_char: Character '{}' not found in player table.",
                ch.get_name()
            );
        } else {
            let i = i.unwrap();
            *self.top_idnum.borrow_mut() += 1;
            self.player_table.borrow_mut()[i].id = *self.top_idnum.borrow() as i64;
            ch.set_idnum(*self.top_idnum.borrow() as i64);
        }

        for i in 1..MAX_SKILLS {
            if ch.get_level() < LVL_IMPL as u8 {
                //set_skill!(ch, i, 0);
                RefCell::borrow_mut(&ch.player_specials).saved.skills[i] = 0;
            } else {
                //set_skill!(ch, i, 100);
                RefCell::borrow_mut(&ch.player_specials).saved.skills[i] = 100;
            }
        }

        ch.set_aff_flags(0);

        for i in 0..5 {
            ch.set_save(i, 0);
        }

        ch.real_abils.borrow_mut().intel = 25;
        ch.real_abils.borrow_mut().wis = 25;
        ch.real_abils.borrow_mut().dex = 25;
        ch.real_abils.borrow_mut().str = 25;
        ch.real_abils.borrow_mut().str_add = 100;
        ch.real_abils.borrow_mut().con = 25;
        ch.real_abils.borrow_mut().cha = 25;

        let cond_value = if ch.get_level() == LVL_IMPL as u8 {
            -1
        } else {
            24
        };
        for i in 0..3 {
            ch.set_cond(i, cond_value);
        }
        ch.set_loadroom(NOWHERE);
    }
}

// /* returns the real number of the room with given virtual number */
pub fn real_room(world: &Vec<Rc<RoomData>>, vnum: room_vnum) -> room_rnum {
    let mut bot = 0 as room_rnum;
    let mut top = (world.len() - 1) as room_rnum;
    let mut mid: room_rnum;

    /* perform binary search on world-table */
    loop {
        mid = (bot + top) / 2;

        if world[mid as usize].number == vnum {
            return mid;
        }

        if bot >= top {
            return NOWHERE;
        }

        if world[mid as usize].number > vnum {
            top = mid - 1;
        } else {
            bot = mid + 1;
        }
    }
}

// /* returns the real number of the monster with given virtual number */
// mob_rnum real_mobile(mob_vnum vnum)
// {
// mob_rnum bot, top, mid;
//
// bot = 0;
// top = top_of_mobt;
//
// /* perform binary search on mob-table */
// for (;;) {
// mid = (bot + top) / 2;
//
// if ((mob_index + mid)->vnum == vnum)
// return (mid);
// if (bot >= top)
// return (NOBODY);
// if ((mob_index + mid)->vnum > vnum)
// top = mid - 1;
// else
// bot = mid + 1;
// }
// }
//
//
// /* returns the real number of the object with given virtual number */
// obj_rnum real_object(obj_vnum vnum)
// {
// obj_rnum bot, top, mid;
//
// bot = 0;
// top = top_of_objt;
//
// /* perform binary search on obj-table */
// for (;;) {
// mid = (bot + top) / 2;
//
// if ((obj_index + mid)->vnum == vnum)
// return (mid);
// if (bot >= top)
// return (NOTHING);
// if ((obj_index + mid)->vnum > vnum)
// top = mid - 1;
// else
// bot = mid + 1;
// }
// }
//
//
// /* returns the real number of the zone with given virtual number */
// room_rnum real_zone(room_vnum vnum)
// {
// room_rnum bot, top, mid;
//
// bot = 0;
// top = top_of_zone_table;
//
// /* perform binary search on zone-table */
// for (;;) {
// mid = (bot + top) / 2;
//
// if ((zone_table + mid)->number == vnum)
// return (mid);
// if (bot >= top)
// return (NOWHERE);
// if ((zone_table + mid)->number > vnum)
// top = mid - 1;
// else
// bot = mid + 1;
// }
// }
//
//
// /*
//  * Extend later to include more checks.
//  *
//  * TODO: Add checks for unknown bitvectors.
//  */
// int check_object(struct obj_data *obj)
// {
// char objname[MAX_INPUT_LENGTH + 32];
// int error = FALSE;
//
// if (GET_OBJ_WEIGHT(obj) < 0 && (error = TRUE))
// log("SYSERR: Object #%d (%s) has negative weight (%d).",
// GET_OBJ_VNUM(obj), obj->short_description, GET_OBJ_WEIGHT(obj));
//
// if (GET_OBJ_RENT(obj) < 0 && (error = TRUE))
// log("SYSERR: Object #%d (%s) has negative cost/day (%d).",
// GET_OBJ_VNUM(obj), obj->short_description, GET_OBJ_RENT(obj));
//
// snprintf(objname, sizeof(objname), "Object #%d (%s)", GET_OBJ_VNUM(obj), obj->short_description);
// error |= check_bitvector_names(GET_OBJ_WEAR(obj), wear_bits_count, objname, "object wear");
// error |= check_bitvector_names(GET_OBJ_EXTRA(obj), extra_bits_count, objname, "object extra");
// error |= check_bitvector_names(GET_OBJ_AFFECT(obj), affected_bits_count, objname, "object affect");
//
// switch (GET_OBJ_TYPE(obj)) {
// case ITEM_DRINKCON:
// {
// char onealias[MAX_INPUT_LENGTH], *space = strrchr(obj->name, ' ');
//
// strlcpy(onealias, space ? space + 1 : obj->name, sizeof(onealias));
// if (search_block(onealias, drinknames, TRUE) < 0 && (error = TRUE))
// log("SYSERR: Object #%d (%s) doesn't have drink type as last alias. (%s)",
// GET_OBJ_VNUM(obj), obj->short_description, obj->name);
// }
// /* Fall through. */
// case ITEM_FOUNTAIN:
// if (GET_OBJ_VAL(obj, 1) > GET_OBJ_VAL(obj, 0) && (error = TRUE))
// log("SYSERR: Object #%d (%s) contains (%d) more than maximum (%d).",
// GET_OBJ_VNUM(obj), obj->short_description,
// GET_OBJ_VAL(obj, 1), GET_OBJ_VAL(obj, 0));
// break;
// case ITEM_SCROLL:
// case ITEM_POTION:
// error |= check_object_level(obj, 0);
// error |= check_object_spell_number(obj, 1);
// error |= check_object_spell_number(obj, 2);
// error |= check_object_spell_number(obj, 3);
// break;
// case ITEM_WAND:
// case ITEM_STAFF:
// error |= check_object_level(obj, 0);
// error |= check_object_spell_number(obj, 3);
// if (GET_OBJ_VAL(obj, 2) > GET_OBJ_VAL(obj, 1) && (error = TRUE))
// log("SYSERR: Object #%d (%s) has more charges (%d) than maximum (%d).",
// GET_OBJ_VNUM(obj), obj->short_description,
// GET_OBJ_VAL(obj, 2), GET_OBJ_VAL(obj, 1));
// break;
// }
//
// return (error);
// }
//
// int check_object_spell_number(struct obj_data *obj, int val)
// {
// int error = FALSE;
// const char *spellname;
//
// if (GET_OBJ_VAL(obj, val) == -1)	/* i.e.: no spell */
// return (error);
//
// /*
//  * Check for negative spells, spells beyond the top define, and any
//  * spell which is actually a skill.
//  */
// if (GET_OBJ_VAL(obj, val) < 0)
// error = TRUE;
// if (GET_OBJ_VAL(obj, val) > TOP_SPELL_DEFINE)
// error = TRUE;
// if (GET_OBJ_VAL(obj, val) > MAX_SPELLS && GET_OBJ_VAL(obj, val) <= MAX_SKILLS)
// error = TRUE;
// if (error)
// log("SYSERR: Object #%d (%s) has out of range spell #%d.",
// GET_OBJ_VNUM(obj), obj->short_description, GET_OBJ_VAL(obj, val));
//
// /*
//  * This bug has been fixed, but if you don't like the special behavior...
//  */
// #if 0
// if (GET_OBJ_TYPE(obj) == ITEM_STAFF &&
// HAS_SPELL_ROUTINE(GET_OBJ_VAL(obj, val), MAG_AREAS | MAG_MASSES))
// log("... '%s' (#%d) uses %s spell '%s'.",
// obj->short_description,	GET_OBJ_VNUM(obj),
// HAS_SPELL_ROUTINE(GET_OBJ_VAL(obj, val), MAG_AREAS) ? "area" : "mass",
// skill_name(GET_OBJ_VAL(obj, val)));
// #endif
//
// if (scheck)		/* Spell names don't exist in syntax check mode. */
// return (error);
//
// /* Now check for unnamed spells. */
// spellname = skill_name(GET_OBJ_VAL(obj, val));
//
// if ((spellname == unused_spellname || !str_cmp("UNDEFINED", spellname)) && (error = TRUE))
// log("SYSERR: Object #%d (%s) uses '%s' spell #%d.",
// GET_OBJ_VNUM(obj), obj->short_description, spellname,
// GET_OBJ_VAL(obj, val));
//
// return (error);
// }
//
// int check_object_level(struct obj_data *obj, int val)
// {
// int error = FALSE;
//
// if ((GET_OBJ_VAL(obj, val) < 0 || GET_OBJ_VAL(obj, val) > LVL_IMPL) && (error = TRUE))
// log("SYSERR: Object #%d (%s) has out of range level #%d.",
// GET_OBJ_VNUM(obj), obj->short_description, GET_OBJ_VAL(obj, val));
//
// return (error);
// }

fn check_bitvector_names(bits: i64, namecount: usize, whatami: &str, whatbits: &str) -> bool {
    let mut flagnum: u32;
    let mut error = false;

    /* See if any bits are set above the ones we know about. */
    if bits <= (!0 as i64 >> (64 - namecount)) {
        return false;
    }

    for flagnum in namecount..64 {
        if ((1 << flagnum) & bits) != 0 {
            error!(
                "SYSERR: {} has unknown {} flag, bit {} (0 through {} known).",
                whatami,
                whatbits,
                flagnum,
                namecount - 1
            );
            error = true;
        }
    }

    return error;
}
