/* ************************************************************************
*   File: db.c                                          Part of CircleMUD *
*  Usage: Loading/saving chars, booting/resetting world, internal funcs   *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */
use std::cell::{Cell, RefCell};
use std::cmp::{max, min};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::Path;
use std::rc::Rc;
use std::{fs, io, mem, process, slice};

use log::{error, info, warn};
use regex::Regex;

use crate::act_social::SocialMessg;
use crate::class::init_spell_levels;
use crate::{check_player_special, get_last_tell_mut, send_to_char, Game};
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
use crate::constants::{
    ACTION_BITS_COUNT, AFFECTED_BITS_COUNT, EXTRA_BITS_COUNT, ROOM_BITS_COUNT, WEAR_BITS_COUNT,
};
use crate::handler::{fname, isname};
use crate::interpreter::one_word;
use crate::modify::paginate_string;
use crate::shops::{assign_the_shopkeepers, ShopData};
use crate::spec_assign::assign_mobiles;
use crate::spec_procs::sort_spells;
use crate::spell_parser::mag_assign_spells;
use crate::spells::{SpellInfoType, TOP_SPELL_DEFINE};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    AffectedType, CharAbilityData, CharData, CharFileU, CharPlayerData, CharPointData,
    CharSpecialData, CharSpecialDataSaved, ExtraDescrData, IndexData, MessageList, MobRnum,
    MobSpecialData, MobVnum, ObjAffectedType, ObjData, ObjFlagData, ObjRnum, ObjVnum,
    PlayerSpecialData, PlayerSpecialDataSaved, RoomData, RoomDirectionData, RoomRnum, TimeData,
    TimeInfoData, WeatherData, ZoneRnum, ZoneVnum, AFF_POISON, APPLY_NONE, EX_CLOSED, EX_ISDOOR,
    EX_LOCKED, EX_PICKPROOF, HOST_LENGTH, ITEM_DRINKCON, ITEM_FOUNTAIN, LVL_GOD, LVL_IMMORT,
    LVL_IMPL, MAX_AFFECT, MAX_NAME_LENGTH, MAX_OBJ_AFFECT, MAX_PWD_LENGTH, MAX_SKILLS,
    MAX_TITLE_LENGTH, MAX_TONGUE, MOB_AGGRESSIVE, MOB_AGGR_EVIL, MOB_AGGR_GOOD, MOB_AGGR_NEUTRAL,
    MOB_ISNPC, MOB_NOTDEADYET, NOBODY, NOTHING, NOWHERE, NUM_OF_DIRS, NUM_WEARS, PASSES_PER_SEC,
    POS_STANDING, PULSE_ZONE, SEX_MALE, SKY_CLOUDLESS, SKY_CLOUDY, SKY_LIGHTNING, SKY_RAINING,
    SUN_DARK, SUN_LIGHT, SUN_RISE, SUN_SET,
};
use crate::util::{
    dice, get_line, mud_time_passed, mud_time_to_secs, prune_crlf, rand_number, time_now, touch,
    CMP, NRM, SECS_PER_REAL_HOUR,
};

const CREDITS_FILE: &str = "./text/credits";
const NEWS_FILE: &str = "./text/news";
const MOTD_FILE: &str = "./text/motd";
const IMOTD_FILE: &str = "./text/imotd";
const GREETINGS_FILE: &str = "./text/greetings";
const HELP_PAGE_FILE: &str = "./text/help/screen";
const INFO_FILE: &str = "./text/info";
const WIZLIST_FILE: &str = "./text/wizlist";
const IMMLIST_FILE: &str = "./text/immlist";
const BACKGROUND_FILE: &str = "text/background";
const POLICIES_FILE: &str = "text/policies";
const HANDBOOK_FILE: &str = "text/handbook";

pub const IDEA_FILE: &str = "./misc/ideas"; /* for the 'idea'-command	*/
pub const TYPO_FILE: &str = "./misc/typos"; /*         'typo'		*/
pub const BUG_FILE: &str = "./misc/bugs"; /*         'bug'		*/
pub const MESS_FILE: &str = "./misc/messages"; /* damage messages		*/
pub const SOCMESS_FILE: &str = "./misc/socials"; /* messages for social acts	*/
pub const XNAME_FILE: &str = "./misc/xnames"; /* invalid name substrings	*/

pub const LIB_PLRTEXT: &str = "plrtext/";
pub const LIB_PLROBJS: &str = "plrobjs/";

pub const KILLSCRIPT_FILE: &str = "./.killscript";
pub const FASTBOOT_FILE: &str = "./fastboot";
pub const PAUSE_FILE: &str = "./pause";

pub const PLAYER_FILE: &str = "etc/players";
pub const LIB_PLRALIAS: &str = "plralias/";

pub const TIME_FILE: &str = "etc/time";

pub const SUF_OBJS: &str = "objs";
pub const SUF_TEXT: &str = "text";
pub const SUF_ALIAS: &str = "alias";

struct PlayerIndexElement {
    name: String,
    id: i64,
}

#[derive(Clone)]
pub struct HelpIndexElement {
    pub keyword: Rc<str>,
    pub entry: Rc<str>,
    pub duplicate: i32,
}

pub struct DB {
    pub world: RefCell<Vec<Rc<RoomData>>>,
    pub character_list: RefCell<Vec<Rc<CharData>>>,
    /* global linked list of * chars	 */
    pub mob_index: Vec<IndexData>,
    /* index table for mobile file	 */
    pub mob_protos: Vec<Rc<CharData>>,
    /* prototypes for mobs		 */
    pub object_list: RefCell<Vec<Rc<ObjData>>>,
    /* global linked list of objs	 */
    pub obj_index: Vec<IndexData>,
    /* index table for object file	 */
    pub obj_proto: Vec<Rc<ObjData>>,
    /* prototypes for objs		 */
    pub(crate) zone_table: RefCell<Vec<ZoneData>>,
    /* zone table			 */
    pub(crate) fight_messages: Vec<MessageList>,
    /* fighting messages	 */
    player_table: RefCell<Vec<PlayerIndexElement>>,
    /* index to plr file	 */
    player_fl: RefCell<Option<File>>,
    /* file desc of player file	 */
    top_idnum: Cell<i32>,
    /* highest idnum in use		 */
    pub no_mail: bool,
    /* mail disabled?		 */
    pub mini_mud: bool,
    /* mini-mud mode?		 */
    pub no_rent_check: bool,
    /* skip rent check on boot?	 */
    pub boot_time: Cell<u128>,
    pub no_specials: bool,
    /* time of mud boot		 */
    pub circle_restrict: u8,
    /* level of game restriction	 */
    pub r_mortal_start_room: RefCell<RoomRnum>,
    /* rnum of mortal start room	 */
    pub r_immort_start_room: RefCell<RoomRnum>,
    /* rnum of immort start room	 */
    pub r_frozen_start_room: RefCell<RoomRnum>,
    /* rnum of frozen start room	 */
    pub credits: Rc<str>,
    /* game credits			 */
    pub news: Rc<str>,
    /* mud news			 */
    pub motd: Rc<str>,
    /* message of the day - mortals */
    pub imotd: Rc<str>,
    /* message of the day - immorts */
    pub greetings: Rc<str>,
    /* opening credits screen	*/
    pub help: Rc<str>,
    /* help screen			 */
    pub info: Rc<str>,
    /* info page	 */
    pub wizlist: Rc<str>,
    /* list of higher gods		 */
    pub immlist: Rc<str>,
    /* list of peon gods		 */
    pub background: Rc<str>,
    /* background story		 */
    pub handbook: Rc<str>,
    /* handbook for new immortals	 */
    pub policies: Rc<str>,
    /* policies page		 */
    pub help_table: Vec<HelpIndexElement>,
    /* the help table	 */
    pub time_info: RefCell<TimeInfoData>,
    /* the infomation about the time    */
    pub weather_info: RefCell<WeatherData>,
    /* the infomation about the weather */
    // struct player_special_data dummy_mob;	/* dummy spec area for mobs	*/
    pub reset_q: RefCell<Vec<ZoneRnum>>,
    pub extractions_pending: Cell<i32>,
    pub timer: Cell<u128>,
    pub cmd_sort_info: Vec<usize>,
    pub combat_list: RefCell<Vec<Rc<CharData>>>,
    pub shop_index: RefCell<Vec<ShopData>>,
    pub spell_sort_info: [i32; MAX_SKILLS as usize + 1],
    pub spell_info: [SpellInfoType; (TOP_SPELL_DEFINE + 1) as usize],
    pub soc_mess_list: Vec<SocialMessg>,
}

pub const REAL: i32 = 0;
pub const VIRTUAL: i32 = 1;

/* structure for the reset commands */
pub struct ResetCom {
    pub command: Cell<char>,
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
    pub age: Cell<i32>,
    /* current age of this zone (minutes) */
    pub bot: RoomRnum,
    /* starting room number for this zone */
    pub top: RoomRnum,
    /* upper limit for rooms in this zone */
    pub reset_mode: i32,
    /* conditions for reset (see below)   */
    pub number: ZoneVnum,
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

/*************************************************************************
*  routines for booting the system                                       *
*************************************************************************/

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
    pub(crate) fn boot_world(&mut self) {
        info!("Loading zone table.");
        self.index_boot(DB_BOOT_ZON);

        info!("Loading rooms.");
        self.index_boot(DB_BOOT_WLD);

        info!("Renumbering rooms.");
        self.renum_world();

        info!("Checking start rooms.");
        self.check_start_rooms();

        info!("Loading mobs and generating index.");
        self.index_boot(DB_BOOT_MOB);

        info!("Loading objs and generating index.");
        self.index_boot(DB_BOOT_OBJ);

        info!("Renumbering zone table.");
        self.renum_zone_table();

        if !self.no_specials {
            info!("Loading shops.");
            self.index_boot(DB_BOOT_SHP);
        }
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
impl DB {
    pub fn new() -> DB {
        DB {
            world: RefCell::new(vec![]),
            character_list: RefCell::new(vec![]),
            mob_index: vec![],
            mob_protos: vec![],
            object_list: RefCell::new(vec![]),
            obj_index: vec![],
            obj_proto: vec![],
            zone_table: RefCell::new(vec![]),
            fight_messages: vec![],
            player_table: RefCell::new(vec![]),
            player_fl: RefCell::new(None),
            top_idnum: Cell::new(0),
            no_mail: false,
            mini_mud: false,
            no_rent_check: false,
            boot_time: Cell::new(0),
            no_specials: false,
            circle_restrict: 0,
            r_mortal_start_room: RefCell::new(0),
            r_immort_start_room: RefCell::new(0),
            r_frozen_start_room: RefCell::new(0),
            credits: Rc::from("CREDITS placeholder"),
            news: Rc::from("NEWS placeholder"),
            motd: Rc::from("MOTD placeholder"),
            imotd: Rc::from("IMOTD placeholder"),
            greetings: Rc::from("Greetings Placeholder"),
            help: Rc::from("HELP placeholder"),
            info: Rc::from("INFO placeholder"),
            wizlist: Rc::from("WIZLIST placeholder"),
            immlist: Rc::from("IMMLIST placeholder"),
            background: Rc::from("BACKGROUND placeholder"),
            handbook: Rc::from("HANDOOK placeholder"),
            policies: Rc::from("POLICIES placeholder"),
            help_table: vec![],
            time_info: RefCell::from(TimeInfoData {
                hours: 0,
                day: 0,
                month: 0,
                year: 0,
            }),
            weather_info: RefCell::new(WeatherData {
                pressure: 0,
                change: 0,
                sky: 0,
                sunlight: 0,
            }),
            reset_q: RefCell::new(vec![]),
            extractions_pending: Cell::new(0),
            timer: Cell::new(0),
            cmd_sort_info: vec![],
            combat_list: RefCell::new(vec![]),
            shop_index: RefCell::new(vec![]),
            spell_sort_info: [0; MAX_SKILLS + 1],
            spell_info: [SpellInfoType::default(); TOP_SPELL_DEFINE + 1],
            soc_mess_list: vec![],
        }
    }

    /* body of the booting system */
    pub fn boot_db(main_globals: &Game) -> DB {
        let mut ret = DB::new();

        info!("Boot db -- BEGIN.");

        info!("Resetting the game time:");
        ret.reset_time();

        info!("Reading news, credits, help, bground, info & motds.");
        main_globals.file_to_string_alloc(NEWS_FILE, &mut ret.news);
        main_globals.file_to_string_alloc(CREDITS_FILE, &mut ret.credits);
        main_globals.file_to_string_alloc(MOTD_FILE, &mut ret.motd);
        main_globals.file_to_string_alloc(IMOTD_FILE, &mut ret.imotd);
        main_globals.file_to_string_alloc(HELP_PAGE_FILE, &mut ret.help);
        main_globals.file_to_string_alloc(INFO_FILE, &mut ret.info);
        main_globals.file_to_string_alloc(WIZLIST_FILE, &mut ret.wizlist);
        main_globals.file_to_string_alloc(IMMLIST_FILE, &mut ret.immlist);
        main_globals.file_to_string_alloc(POLICIES_FILE, &mut ret.policies);
        main_globals.file_to_string_alloc(HANDBOOK_FILE, &mut ret.handbook);
        main_globals.file_to_string_alloc(BACKGROUND_FILE, &mut ret.background);
        main_globals.file_to_string_alloc(GREETINGS_FILE, &mut ret.greetings);
        prune_crlf(&mut ret.greetings);

        info!("Loading spell definitions.");
        mag_assign_spells(&mut ret);

        ret.boot_world();

        info!("Loading help entries.");
        ret.index_boot(DB_BOOT_HLP);

        info!("Generating player index.");
        ret.build_player_index();

        info!("Loading fight messages.");
        ret.load_messages();

        info!("Loading social messages.");
        ret.boot_social_messages();

        info!("Assigning function pointers:");

        if !ret.no_specials {
            info!("   Mobiles.");
            assign_mobiles(&mut ret);
            info!("   Shopkeepers.");
            assign_the_shopkeepers(&mut ret);
            // info!("   Objects.");
            // assign_objects();
            // info!("   Rooms.");
            // assign_rooms();
        }

        info!("Assigning spell and skill levels.");
        init_spell_levels(&mut ret);
        //
        info!("Sorting command list and spells.");
        ret.sort_commands();
        sort_spells(&mut ret);
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

        for (i, zone) in ret.zone_table.borrow().iter().enumerate() {
            info!(
                "Resetting #{}: {} (rooms {}-{}).",
                zone.number, zone.name, zone.bot, zone.top
            );
            ret.reset_zone(main_globals, i);
        }

        // reset_q.head = reset_q.tail = NULL;
        //
        // boot_time = time(0);
        //
        info!("Boot db -- DONE.");
        ret
    }

    /* reset the time in the game from file */
    fn reset_time(&self) {
        let mut beginning_of_time = 0;
        //FILE *bgtime;

        let bgtime = OpenOptions::new().read(true).open(TIME_FILE);
        if bgtime.is_err() {
            info!("SYSERR: Can't open '{}'", TIME_FILE);
        } else {
            let bgtime = bgtime.unwrap();
            let mut reader = BufReader::new(bgtime);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .expect(format!("SYSERR: Can't read from '{}'", TIME_FILE).as_str());
            line = line.trim().to_string();
            beginning_of_time = line
                .parse::<u128>()
                .expect(format!("SYSERR: Invalid mud time: {}", line).as_str());
        }
        if beginning_of_time == 0 {
            beginning_of_time = 650336715;
        }

        *self.time_info.borrow_mut() = mud_time_passed(time_now(), beginning_of_time as u64);

        if self.time_info.borrow().hours <= 4 {
            self.weather_info.borrow_mut().sunlight = SUN_DARK;
        } else if self.time_info.borrow().hours == 5 {
            self.weather_info.borrow_mut().sunlight = SUN_RISE;
        } else if self.time_info.borrow().hours <= 20 {
            self.weather_info.borrow_mut().sunlight = SUN_LIGHT;
        } else if self.time_info.borrow().hours == 21 {
            self.weather_info.borrow_mut().sunlight = SUN_SET;
        } else {
            self.weather_info.borrow_mut().sunlight = SUN_DARK;
        }

        info!(
            "   Current Gametime: {}H {}D {}M {}Y.",
            self.time_info.borrow().hours,
            self.time_info.borrow().day,
            self.time_info.borrow().month,
            self.time_info.borrow().year
        );

        self.weather_info.borrow_mut().pressure = 960;
        if (self.time_info.borrow().month >= 7) && (self.time_info.borrow().month <= 12) {
            self.weather_info.borrow_mut().pressure += dice(1, 50);
        } else {
            self.weather_info.borrow_mut().pressure += dice(1, 80);
        }

        self.weather_info.borrow_mut().change = 0;

        if self.weather_info.borrow().pressure <= 980 {
            self.weather_info.borrow_mut().sky = SKY_LIGHTNING;
        } else if self.weather_info.borrow().pressure <= 1000 {
            self.weather_info.borrow_mut().sky = SKY_RAINING;
        } else if self.weather_info.borrow().pressure <= 1020 {
            self.weather_info.borrow_mut().sky = SKY_CLOUDY;
        } else {
            self.weather_info.borrow_mut().sky = SKY_CLOUDLESS;
        }
    }
}

/* Write the time in 'when' to the MUD-time file. */
pub fn save_mud_time(when: &TimeInfoData) {
    let bgtime = OpenOptions::new()
        .write(true)
        .create(true)
        .open(TIME_FILE)
        .expect(format!("SYSERR: Cannot open time file: {}", TIME_FILE).as_str());
    let mut writer = BufWriter::new(bgtime);
    let content = format!("{}\n", mud_time_to_secs(when));
    writer
        .write_all(content.as_bytes())
        .expect(format!("SYSERR: Cannot write to time file: {}", TIME_FILE).as_str());
}

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

pub fn parse_c_string(cstr: &[u8]) -> String {
    let mut ret: String = std::str::from_utf8(cstr)
        .expect(format!("Error while parsing C string {:?}", cstr).as_str())
        .parse()
        .unwrap();
    let p = ret.find('\0');
    if p.is_some() {
        ret.truncate(p.unwrap());
    }
    ret
}

impl DB {
    /* generate index table for the player file */
    fn build_player_index<'a>(&mut self) {
        let recs: u64;

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
                player_file = OpenOptions::new()
                    .write(true)
                    .read(true)
                    .open(PLAYER_FILE)
                    .expect("SYSERR: fatal error opening playerfile after creation");
            }
        } else {
            player_file = r.unwrap();
        }

        *self.player_fl.borrow_mut() = Some(player_file);

        let mut t = self.player_fl.borrow_mut();
        let file_mut = t.as_mut().unwrap();
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
            self.player_table.borrow_mut().reserve_exact(recs as usize);
        } else {
            // player_table = NULL;
            //*self.top_of_p_table.borrow_mut() = -1;
            return;
        }

        loop {
            let mut dummy = CharFileU::new();

            unsafe {
                let config_slice = slice::from_raw_parts_mut(
                    &mut dummy as *mut _ as *mut u8,
                    mem::size_of::<CharFileU>(),
                );
                // `read_exact()` comes from `Read` impl for `&[u8]`
                let r = file_mut.read_exact(config_slice);
                if r.is_err() {
                    let r = r.err().unwrap();
                    if r.kind() == ErrorKind::UnexpectedEof {
                        break;
                    }
                    error!(
                        "[SYSERR] Error while reading player file for indexing: {}",
                        r
                    );
                    process::exit(1);
                }
            }

            let mut pie = PlayerIndexElement {
                name: parse_c_string(&dummy.name),
                id: dummy.char_specials_saved.idnum,
            };
            pie.name = pie.name.to_lowercase();
            self.player_table.borrow_mut().push(pie);
            self.top_idnum.set(max(
                self.top_idnum.get(),
                dummy.char_specials_saved.idnum as i32,
            ));
        }
    }
}

/*
 * Thanks to Andrey (andrey@alex-ua.com) for this bit of code, although I
 * did add the 'goto' and changed some "while()" into "do { } while()".
 *	-gg 6/24/98 (technically 6/25/98, but I care not.)
 */
fn count_alias_records(fl: File) -> i32 {
    let mut total_keywords = 0;
    let mut key = String::new();
    let mut line = String::new();
    /* get the first keyword line */
    let mut reader = BufReader::new(fl);
    get_one_line(&mut reader, &mut key);

    while !key.starts_with('$') {
        /* skip the text */
        loop {
            line.clear();
            get_one_line(&mut reader, &mut line);
            // if (feof(fl))
            // goto ackeof;
            if line.starts_with('#') {
                break;
            }
        }

        /* now count keywords */
        let mut scan = key.as_str();
        let mut next_key = String::new();
        loop {
            scan = one_word(&mut scan, &mut next_key);
            if !next_key.is_empty() {
                total_keywords += 1;
            } else {
                break;
            }
        }

        /* get next keyword line (or $) */
        key.clear();
        get_one_line(&mut reader, &mut key);

        // if (feof(fl))
        // goto ackeof;
    }

    return total_keywords;
    // ackeof:
    // log("SYSERR: Unexpected end of help file.");
    // exit(1);	/* Some day we hope to handle these things better... */
}

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
const HLP_PREFIX: &str = "text/help/"; /* for HELP <keyword>	*/
/* arbitrary constants used by index_boot() (must be unique) */
const DB_BOOT_WLD: u8 = 0;
const DB_BOOT_MOB: u8 = 1;
const DB_BOOT_OBJ: u8 = 2;
const DB_BOOT_ZON: u8 = 3;
const DB_BOOT_SHP: u8 = 4;
const DB_BOOT_HLP: u8 = 5;

impl DB {
    fn index_boot(&mut self, mode: u8) {
        let index_filename: &str;
        let prefix: &str;
        let mut rec_count = 0;
        let mut size: [usize; 2] = [0; 2];

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
            DB_BOOT_HLP => {
                prefix = HLP_PREFIX;
            }
            _ => {
                error!("SYSERR: Unknown subcommand {} to index_boot!", mode);
                process::exit(1);
            }
        }

        if self.mini_mud {
            index_filename = MINDEX_FILE;
        } else {
            index_filename = INDEX_FILE;
        }

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
        let db_index = db_index.unwrap();

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
                } else if mode == DB_BOOT_HLP {
                    rec_count += count_alias_records(db_file.unwrap());
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

        match mode {
            DB_BOOT_WLD => {
                self.world.borrow_mut().reserve_exact(rec_count as usize);
                size[0] = mem::size_of::<CharData>() * rec_count as usize;
                info!("   {} rooms, {} bytes.", rec_count, size[0]);
            }
            DB_BOOT_MOB => {
                size[0] = mem::size_of::<IndexData>() * rec_count as usize;
                size[1] = mem::size_of::<CharData>() * rec_count as usize;
                self.mob_protos.reserve_exact(rec_count as usize);
                self.mob_index.reserve_exact(rec_count as usize);
                info!(
                    "   {} mobs, {} bytes in index, {} bytes in prototypes.",
                    rec_count, size[0], size[1],
                );
            }
            DB_BOOT_OBJ => {
                self.obj_proto.reserve_exact(rec_count as usize);
                self.obj_index.reserve_exact(rec_count as usize);
                size[0] = mem::size_of::<IndexData>() * rec_count as usize;
                size[1] = mem::size_of::<ObjData>() * rec_count as usize;
                info!(
                    "   {} objs, {} bytes in index, {} bytes in prototypes.",
                    rec_count, size[0], size[1]
                );
            }
            DB_BOOT_ZON => {
                self.zone_table
                    .borrow_mut()
                    .reserve_exact(rec_count as usize);
                size[0] = mem::size_of::<ZoneData>() * rec_count as usize;
                info!("   {} zones, {} bytes.", rec_count, size[0]);
            }
            DB_BOOT_HLP => {
                self.help_table.reserve_exact(rec_count as usize);
                size[0] = mem::size_of::<HelpIndexElement>() & rec_count as usize;
                info!("   {} entries, {} bytes.", rec_count, size[0]);
            }
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
                DB_BOOT_HLP => {
                    /*
                     * If you think about it, we have a race here.  Although, this is the
                     * "point-the-gun-at-your-own-foot" type of race.
                     */
                    self.load_help(db_file.unwrap());
                }
                DB_BOOT_SHP => {
                    self.boot_the_shops(db_file.unwrap(), &buf2, rec_count);
                }
                _ => {}
            }

            buf1.clear();
            reader
                .read_line(&mut buf1)
                .expect("Error while reading index file #5");
            buf1 = buf1.trim_end().to_string();
        }

        /* sort the help index */
        if mode == DB_BOOT_HLP {
            self.help_table
                .sort_by_key(|e| String::from(e.keyword.as_ref()));
        }
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
                        DB_BOOT_MOB => {
                            self.parse_mobile(&mut reader, nr);
                        }
                        DB_BOOT_OBJ => {
                            line = self
                                .parse_object(&mut reader, nr as MobVnum)
                                .parse()
                                .unwrap();
                        }
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
        let mut t = [0; 10];
        let mut line = String::new();
        let mut zone = 0;

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
            number: virtual_nr as RoomRnum,
            zone: zone as ZoneRnum,
            sector_type: 0,
            name: fread_string(reader, buf2.as_str()),
            description: fread_string(reader, buf2.as_str()),
            ex_descriptions: vec![],
            dir_option: [None, None, None, None, None, None],
            room_flags: Cell::new(0),
            light: Cell::new(0),
            func: None,
            contents: RefCell::new(vec![]),
            peoples: RefCell::new(vec![]),
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
        let flags = &f[2];
        t[2] = f[3].parse::<i32>().unwrap();

        /* t[0] is the zone number; ignored with the zone-file system */

        rd.room_flags.set(asciiflag_conv(flags) as i32);
        let msg = format!("object #{}", virtual_nr); /* sprintf: OK (until 399-bit integers) */
        check_bitvector_names(
            rd.room_flags.get() as i64,
            ROOM_BITS_COUNT,
            msg.as_str(),
            "room",
        );

        rd.sector_type = t[2];

        //rd.func = NULL;
        rd.peoples = RefCell::new(vec![]);
        rd.light.set(0); /* Zero light sources */

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
                    rd.ex_descriptions.push(ExtraDescrData {
                        keyword: fread_string(reader, buf2.as_str()),
                        description: fread_string(reader, buf2.as_str()),
                    });
                }
                'S' => {
                    /* end of room */
                    // *self.top_of_world.borrow_mut() = room_nr as RoomRnum;
                    //room_nr += 1;
                    break;
                }
                _ => {
                    error!("{}", buf);
                    process::exit(1);
                }
            }
        }
        self.world.borrow_mut().push(Rc::new(rd));
        // *self.top_of_world.borrow_mut() += 1;
    }

    /* read direction data */
    fn setup_dir(&self, reader: &mut BufReader<File>, room: &mut RoomData, dir: i32) {
        let mut t = [0; 5];
        let mut line = String::new();
        // char line[READ_SIZE], buf2[128];

        let buf2 = format!(
            "room #{}, direction D{}",
            room.number,
            //get_RoomRnum!(self, room as usize),
            dir
        );

        let mut rdr = RoomDirectionData {
            general_description: fread_string(reader, buf2.as_str()),
            keyword: fread_string(reader, buf2.as_str()),
            exit_info: Cell::from(0),
            key: 0,
            to_room: Cell::new(0),
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
            rdr.exit_info.set(EX_ISDOOR);
        } else if t[0] == 2 {
            rdr.exit_info.set(EX_ISDOOR | EX_PICKPROOF);
        } else {
            rdr.exit_info.set(0);
        }

        rdr.key = t[1] as ObjVnum;
        rdr.to_room.set(t[2] as RoomRnum);

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
        *self.r_mortal_start_room.borrow_mut() = self.real_room(MORTAL_START_ROOM);
        if *self.r_mortal_start_room.borrow() == NOWHERE {
            error!("SYSERR:  Mortal start room does not exist.  Change in config.c.");
            process::exit(1);
        }
        *self.r_immort_start_room.borrow_mut() = self.real_room(IMMORT_START_ROOM);
        if *self.r_immort_start_room.borrow() == NOWHERE {
            // if (!mini_mud)
            error!("SYSERR:  Warning: Immort start room does not exist.  Change in config.c.");
            *self.r_immort_start_room.borrow_mut() = *self.r_mortal_start_room.borrow();
        }
        *self.r_frozen_start_room.borrow_mut() = self.real_room(FROZEN_START_ROOM);
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
        for (_, room_data) in self.world.borrow().iter().enumerate() {
            for door in 0..NUM_OF_DIRS {
                let to_room: RoomRnum;
                {
                    if room_data.dir_option[door].is_none() {
                        continue;
                    }
                    to_room = room_data.dir_option[door].as_ref().unwrap().to_room.get();
                }
                if to_room != NOWHERE {
                    let rn = self.real_room(to_room);
                    room_data.dir_option[door].as_ref().unwrap().to_room.set(rn);
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
     * NOTE 2: Assumes sizeof(RoomRnum) >= (sizeof(mob_rnum) and sizeof(obj_rnum))
     */

    fn renum_zone_table(&mut self) {
        let mut olda;
        let mut oldb;
        let mut oldc;

        for zone in self.zone_table.borrow_mut().iter_mut() {
            for cmd_no in 0..zone.cmd.len() {
                let zcmd = &mut zone.cmd[cmd_no];
                if zcmd.command.get() == 'S' {
                    break;
                }
                let mut a = 0;
                let mut b = 0;
                let mut c = 0;
                olda = zcmd.arg1;
                oldb = zcmd.arg2;
                oldc = zcmd.arg3;
                match zcmd.command.get() {
                    'M' => {
                        zcmd.arg1 = self.real_mobile(zcmd.arg1 as MobVnum) as i32;
                        a = zcmd.arg1;
                        zcmd.arg3 = self.real_room(zcmd.arg3 as RoomRnum) as i32;
                        c = zcmd.arg3;
                    }
                    'O' => {
                        zcmd.arg1 = self.real_object(zcmd.arg1 as ObjVnum) as i32;
                        a = zcmd.arg1;
                        if zcmd.arg3 != NOWHERE as i32 {
                            zcmd.arg3 = self.real_room(zcmd.arg3 as RoomRnum) as i32;
                            c = zcmd.arg3;
                        }
                    }
                    'G' => {
                        zcmd.arg1 = self.real_object(zcmd.arg1 as ObjVnum) as i32;
                        a = zcmd.arg1;
                    }
                    'E' => {
                        zcmd.arg1 = self.real_object(zcmd.arg1 as ObjVnum) as i32;
                        a = zcmd.arg1;
                    }
                    'P' => {
                        zcmd.arg1 = self.real_object(zcmd.arg1 as ObjVnum) as i32;
                        a = zcmd.arg1;
                        zcmd.arg3 = self.real_object(zcmd.arg3 as ObjVnum) as i32;
                        c = zcmd.arg3;
                    }
                    'D' => {
                        zcmd.arg1 = self.real_room(zcmd.arg1 as RoomRnum) as i32;
                        a = zcmd.arg1;
                    }
                    'R' => {
                        /* rem obj from room */
                        zcmd.arg1 = self.real_room(zcmd.arg1 as RoomRnum) as i32;
                        a = zcmd.arg1;
                        zcmd.arg2 = self.real_room(zcmd.arg2 as RoomRnum) as i32;
                        b = zcmd.arg2;
                    }
                    _ => {}
                }

                if a == NOWHERE as i32 || b == NOWHERE as i32 || c == NOWHERE as i32 {
                    if !self.mini_mud {
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
                    }
                    zcmd.command.set('*');
                }
            }
        }
    }

    fn parse_simple_mob(&mut self, reader: &mut BufReader<File>, mobch: &mut CharData, nr: i32) {
        // int j, t[10];
        // char line[READ_SIZE];
        let mut line = String::new();

        mobch.real_abils.borrow_mut().str = 11;
        mobch.real_abils.borrow_mut().intel = 11;
        mobch.real_abils.borrow_mut().wis = 11;
        mobch.real_abils.borrow_mut().dex = 11;
        mobch.real_abils.borrow_mut().con = 11;
        mobch.real_abils.borrow_mut().cha = 11;

        if get_line(reader, &mut line) == 0 {
            error!(
                "SYSERR: Format error in mob #{}, file ended after S flag!",
                nr
            );
            process::exit(1);
        }

        let regex = Regex::new(r"^(-?\d{1,9})\s(-?\d{1,9})\s(-?\d{1,9})\s(-?\d{1,9})d(-?\d{1,9})\+(-?\d{1,9})\s(-?\d{1,9})d(-?\d{1,9})\+(-?\d{1,9})").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!("SYSERR: Format error in mob #{}, first line after S flag\n...expecting line of form '# # # #d#+# #d#+#'", nr);
            process::exit(1);
        }
        let t = f.unwrap();

        mobch.set_level(t[1].parse::<u8>().unwrap());
        mobch.set_hitroll(20 - t[2].parse::<i8>().unwrap());
        mobch.set_ac(10 * t[3].parse::<i16>().unwrap());

        /* max hit = 0 is a flag that H, M, V is xdy+z */
        mobch.set_max_hit(0);
        mobch.set_hit(t[4].parse::<i16>().unwrap());
        mobch.set_mana(t[5].parse::<i16>().unwrap());
        mobch.set_move(t[6].parse::<i16>().unwrap());

        mobch.set_max_mana(10);
        mobch.set_max_move(50);

        mobch.mob_specials.damnodice = t[7].parse::<u8>().unwrap();
        mobch.mob_specials.damsizedice = t[8].parse::<u8>().unwrap();
        mobch.set_damroll(t[9].parse::<i8>().unwrap());

        if get_line(reader, &mut line) == 0 {
            error!("SYSERR: Format error in mob #{}, second line after S flag\n...expecting line of form '# #', but file ended!", nr);
            process::exit(1);
        }

        let regex = Regex::new(r"^(-?\d{1,9})\s(-?\d{1,9})").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!("SYSERR: Format error in mob #{}, second line after S flag\n...expecting line of form '# #'", nr);
            process::exit(1);
        }
        let t = f.unwrap();

        mobch.set_gold(t[1].parse::<i32>().unwrap());
        mobch.set_exp(t[2].parse::<i32>().unwrap());

        if get_line(reader, &mut line) == 0 {
            error!("SYSERR: Format error in last line of mob #{}\n...expecting line of form '# # #', but file ended!", nr);
            process::exit(1);
        }

        let regex = Regex::new(r"^(-?\d{1,9})\s(-?\d{1,9})\s(-?\d{1,9})").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!(
                "SYSERR: Format error in last line of mob #{}\n...expecting line of form '# # #'",
                nr
            );
            process::exit(1);
        }
        let t = f.unwrap();

        mobch.set_pos(t[1].parse::<u8>().unwrap());
        mobch.set_default_pos(t[2].parse::<u8>().unwrap());
        mobch.set_sex(t[3].parse::<u8>().unwrap());

        mobch.set_class(0);
        mobch.set_weight(200);
        mobch.set_height(198);

        /*
         * these are now save applies; base save numbers for MOBs are now from
         * the warrior save table.
         */
        for j in 0..5 {
            mobch.set_save(j, 0);
        }
    }

    /*
     * interpret_espec is the function that takes espec keywords and values
     * and assigns the correct value to the mob as appropriate.  Adding new
     * e-specs is absurdly easy -- just add a new CASE statement to this
     * function!  No other changes need to be made anywhere in the code.
     *
     * CASE		: Requires a parameter through 'value'.
     * BOOL_CASE	: Being specified at all is its value.
     */

    // # define
    // CASE(test)    \
    // if (value && !matched && !str_cmp(keyword, test) && (matched = TRUE))
    //
    // # define
    // BOOL_CASE(test)    \
    // if (!value && !matched && !str_cmp(keyword, test) && (matched = TRUE))
    //
    // # define
    // RANGE(low, high)    \
    // (num_arg = MAX((low), MIN((high), (num_arg))))

    // void
    // interpret_espec(const char
    // *keyword, const char
    // *value, int
    // i, int
    // nr)
    // {
    //     int
    //     num_arg = 0, matched = FALSE;
    //
    //     /*
    //      * If there isn't a colon, there is no value.  While Boolean options are
    //      * possible, we don't actually have any.  Feel free to make some.
    //     */
    //     if (value)
    //     num_arg = atoi(value);
    //
    //     CASE("BareHandAttack")
    //     {
    //         RANGE(0, 99);
    //         mob_proto[i].mob_specials.attack_type = num_arg;
    //     }
    //
    //     CASE("Str")
    //     {
    //         RANGE(3, 25);
    //         mob_proto[i].real_abils.str = num_arg;
    //     }
    //
    //     CASE("StrAdd")
    //     {
    //         RANGE(0, 100);
    //         mob_proto[i].real_abils.str_add = num_arg;
    //     }
    //
    //     CASE("Int")
    //     {
    //         RANGE(3, 25);
    //         mob_proto[i].real_abils.intel = num_arg;
    //     }
    //
    //     CASE("Wis")
    //     {
    //         RANGE(3, 25);
    //         mob_proto[i].real_abils.wis = num_arg;
    //     }
    //
    //     CASE("Dex")
    //     {
    //         RANGE(3, 25);
    //         mob_proto[i].real_abils.dex = num_arg;
    //     }
    //
    //     CASE("Con")
    //     {
    //         RANGE(3, 25);
    //         mob_proto[i].real_abils.con = num_arg;
    //     }
    //
    //     CASE("Cha")
    //     {
    //         RANGE(3, 25);
    //         mob_proto[i].real_abils.cha = num_arg;
    //     }
    //
    //     if (!matched) {
    //         log("SYSERR: Warning: unrecognized espec keyword %s in mob #%d",
    //             keyword, nr);
    //     }
    // }
    //
    // # undef
    // CASE
    // # undef
    // BOOL_CASE
    // # undef
    // RANGE

    // fn parse_espec(char * buf, int i, int nr)
    // {
    //     char * ptr;
    //
    //     if ((ptr = strchr(buf, ':')) != NULL) {
    //         *(ptr + +) = '\0';
    //         while (isspace(*ptr))
    //         ptr + +;
    //     }
    //     interpret_espec(buf, ptr, i, nr);
    // }

    fn parse_enhanced_mob(&mut self, reader: &mut BufReader<File>, mobch: &mut CharData, nr: i32) {
        // char
        // line[READ_SIZE];
        let mut line = String::new();

        self.parse_simple_mob(reader, mobch, nr);

        while get_line(reader, &mut line) != 0 {
            if line == "E" {
                /* end of the enhanced section */
                return;
            } else if line.starts_with('#') {
                /* we've hit the next mob, maybe? */
                error!("SYSERR: Unterminated E section in mob #{}", nr);
                process::exit(1);
            } else {
                // TODO implement spec proc
                //parse_espec(line, i, nr);
            }
        }

        error!("SYSERR: Unexpected end of file reached after mob #{}", nr);
        process::exit(1);
    }

    fn parse_mobile(&mut self, reader: &mut BufReader<File>, nr: i32) {
        //static int i = 0;
        // int j, t[10];
        // char line[READ_SIZE], * tmpptr, letter;
        // char f1[128], f2[128], buf2[128];
        let mut line = String::new();

        self.mob_index.push(IndexData {
            vnum: nr as MobVnum,
            number: Cell::from(0),
            func: None,
        });

        let mut mobch = CharData::new();
        clear_char(&mut mobch);

        /*
         * Mobiles should NEVER use anything in the 'player_specials' structure.
         * The only reason we have every mob in the game share this copy of the
         * structure is to save newbie coders from themselves. -gg 2/25/98
         */
        // TODO mobch.player_specials = &dummy_mob;
        let buf2 = format!("mob vnum {}", nr);

        /***** String data *****/
        mobch.player.borrow_mut().name = fread_string(reader, buf2.as_str());
        let mut tmpstr = fread_string(reader, buf2.as_str());
        if !tmpstr.is_empty() {
            let f1 = fname(tmpstr.as_str());
            let f = f1.as_ref();
            if f == "a" || f == "an" || f == "the" {
                let c = tmpstr.remove(0);
                tmpstr.insert(0, char::to_ascii_lowercase(&c));
            }
        }
        mobch.player.borrow_mut().short_descr = tmpstr;
        mobch.player.borrow_mut().long_descr = fread_string(reader, buf2.as_str());
        mobch.player.borrow_mut().description =
            Rc::new(RefCell::from(fread_string(reader, buf2.as_str())));
        mobch.set_title(None);

        /* *** Numeric data *** */
        if get_line(reader, &mut line) == 0 {
            error!("SYSERR: Format error after string section of mob #{}\n...expecting line of form '# # # {{S | E}}', but file ended!", nr);
            process::exit(1);
        }

        let regex = Regex::new(r"^(\S+)\s(\S+)\s(-?\+?\d{1,9})\s([SE])").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!("SYSERR: Format error after string section of mob #{}\n...expecting line of form '# # # {{S | E}}'", nr);
            process::exit(1);
        }
        let f = f.unwrap();

        mobch.set_mob_flags(asciiflag_conv(&f[1]));
        mobch.set_mob_flags_bit(MOB_ISNPC);
        if mobch.mob_flagged(MOB_NOTDEADYET) {
            /* Rather bad to load mobiles with this bit already set. */
            error!("SYSERR: Mob #{} has reserved bit MOB_NOTDEADYET set.", nr);
            mobch.remove_mob_flags_bit(MOB_NOTDEADYET);
        }
        check_bitvector_names(
            mobch.mob_flags(),
            ACTION_BITS_COUNT,
            buf2.as_str(),
            "mobile",
        );

        mobch.set_aff_flags(asciiflag_conv(&f[2]));
        check_bitvector_names(
            mobch.aff_flags(),
            AFFECTED_BITS_COUNT,
            buf2.as_str(),
            "mobile affect",
        );

        mobch.set_alignment(f[3].parse::<i32>().unwrap());

        /* AGGR_TO_ALIGN is ignored if the mob is AGGRESSIVE. */
        if mobch.mob_flagged(MOB_AGGRESSIVE)
            && mobch.mob_flagged(MOB_AGGR_GOOD | MOB_AGGR_EVIL | MOB_AGGR_NEUTRAL)
        {
            error!(
                "SYSERR: Mob #{} both Aggressive and Aggressive_to_Alignment.",
                nr
            );
        }

        match f[4].to_uppercase().as_str() {
            "S" => {
                /* Simple monsters */
                self.parse_simple_mob(reader, &mut mobch, nr);
            }
            "E" => {
                /* Circle3 Enhanced monsters */
                self.parse_enhanced_mob(reader, &mut mobch, nr);
            }
            /* add new mob types here.. */
            _ => {
                error!("SYSERR: Unsupported mob type '{}' in mob #{}", &f[4], nr);
                process::exit(1);
            }
        }

        *mobch.aff_abils.borrow_mut() = *mobch.real_abils.borrow();

        for j in 0..NUM_WEARS {
            mobch.equipment.borrow_mut()[j as usize] = None;
        }

        mobch.nr = self.mob_protos.len() as MobRnum;
        mobch.desc = RefCell::new(None);

        self.mob_protos.push(Rc::from(mobch));
    }

    /* read all objects from obj file; generate index and prototypes */
    fn parse_object(&mut self, reader: &mut BufReader<File>, nr: MobVnum) -> String {
        // static int i = 0;
        //static char line[READ_SIZE];
        // int t[10], j, retval;
        // char * tmpptr;
        // char f1[READ_SIZE], f2[READ_SIZE], buf2[128];
        // struct extra_descr_data * new_descr;
        let mut line = String::new();

        let i = self.obj_index.len() as ObjVnum;
        self.obj_index.push(IndexData {
            vnum: nr,
            number: Cell::from(0),
            func: None,
        });

        let mut obj = ObjData {
            item_number: 0,
            in_room: Cell::new(0),
            obj_flags: ObjFlagData {
                value: [Cell::new(0), Cell::new(0), Cell::new(0), Cell::new(0)],
                type_flag: 0,
                wear_flags: 0,
                extra_flags: Cell::new(0),
                weight: Cell::new(0),
                cost: 0,
                cost_per_day: 0,
                timer: Cell::new(0),
                bitvector: Cell::new(0),
            },
            affected: [
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
            ],
            name: RefCell::from("".to_string()),
            description: "".to_string(),
            short_description: "".to_string(),
            action_description: Rc::new(RefCell::new(String::new())),
            ex_descriptions: vec![],
            carried_by: RefCell::new(None),
            worn_by: RefCell::new(None),
            worn_on: Cell::new(0),
            in_obj: RefCell::new(None),
            contains: RefCell::new(vec![]),
            next_content: RefCell::new(None),
            next: RefCell::new(None),
        };

        clear_object(&mut obj);
        obj.item_number = i;

        let buf2 = format!("object #{}", nr); /* sprintf: OK (for 'buf2 >= 19') */

        /* *** string data *** */
        *obj.name.borrow_mut() = fread_string(reader, &buf2);
        if obj.name.borrow().is_empty() {
            error!("SYSERR: Null obj name or format error at or near {}", buf2);
            process::exit(1);
        }
        let mut tmpstr = fread_string(reader, &buf2);
        if !tmpstr.is_empty() {
            let f = fname(tmpstr.as_str());
            if f.as_ref() == "a" || f.as_ref() == "an" || f.as_ref() == "the" {
                let c = tmpstr.remove(0);
                tmpstr.insert(0, char::to_ascii_lowercase(&c));
            }
        }
        obj.short_description = tmpstr;

        let tmpptr = fread_string(reader, &buf2);
        obj.description = tmpptr;
        obj.action_description = Rc::new(RefCell::from(fread_string(reader, &buf2)));

        /* *** numeric data *** */
        if get_line(reader, &mut line) == 0 {
            error!(
                "SYSERR: Expecting first numeric line of {}, but file ended!",
                buf2
            );
            process::exit(1);
        }

        let regex = Regex::new(r"^(\d{1,9})\s(\S+)\s(\S+)").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!(
                "SYSERR: Format error in first numeric line (expecting 3 args), {}",
                buf2
            );
            process::exit(1);
        }
        let f = f.unwrap();

        /* Object flags checked in check_object(). */
        obj.set_obj_type(f[1].parse::<u8>().unwrap());
        obj.set_obj_extra(asciiflag_conv(&f[2]) as i32);
        obj.set_obj_wear(asciiflag_conv(&f[3]) as i32);

        if get_line(reader, &mut line) == 0 {
            error!(
                "SYSERR: Expecting second numeric line of {}, but file ended!",
                buf2
            );
            process::exit(1);
        }
        let regex =
            Regex::new(r"^(-?\+?\d{1,9})\s(-?\+?\d{1,9})\s(-?\+?\d{1,9})\s(-?\+?\d{1,9})").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!(
                "SYSERR: Format error in second numeric line (expecting 4 args), {}",
                buf2
            );
            process::exit(1);
        }
        let f = f.unwrap();
        obj.set_obj_val(0, f[1].parse::<i32>().unwrap());
        obj.set_obj_val(1, f[2].parse::<i32>().unwrap());
        obj.set_obj_val(2, f[3].parse::<i32>().unwrap());
        obj.set_obj_val(3, f[4].parse::<i32>().unwrap());

        if get_line(reader, &mut line) == 0 {
            error!(
                "SYSERR: Expecting third numeric line of {}, but file ended!",
                buf2
            );
            process::exit(1);
        }
        let regex = Regex::new(r"^(-?\+?\d{1,9})\s(-?\+?\d{1,9})\s(-?\+?\d{1,9})").unwrap();
        let f = regex.captures(line.as_str());
        if f.is_none() {
            error!(
                "SYSERR: Format error in third numeric line (expecting 3 args), {}",
                buf2
            );
            process::exit(1);
        }
        let f = f.unwrap();
        obj.set_obj_weight(f[1].parse::<i32>().unwrap());
        obj.set_obj_cost(f[2].parse::<i32>().unwrap());
        obj.set_obj_rent(f[3].parse::<i32>().unwrap());

        /* check to make sure that weight of containers exceeds curr. quantity */
        if obj.get_obj_type() == ITEM_DRINKCON as u8 || obj.get_obj_type() == ITEM_FOUNTAIN as u8 {
            if obj.get_obj_weight() < obj.get_obj_val(1) {
                obj.set_obj_weight(obj.get_obj_val(1) + 5);
            }
        }

        /* *** extra descriptions and affect fields *** */
        for j in 0..MAX_OBJ_AFFECT {
            obj.affected[j as usize].get().location = APPLY_NONE as u8;
            obj.affected[j as usize].get().modifier = 0;
        }

        let buf2 = ", after numeric constants\n...expecting 'E', 'A', '$', or next object number";
        let mut j = 0;

        loop {
            if get_line(reader, &mut line) == 0 {
                error!("SYSERR: Format error in {}", buf2);
                process::exit(1);
            }
            match line.chars().next().unwrap() {
                'E' => {
                    let new_descr = ExtraDescrData {
                        keyword: fread_string(reader, buf2),
                        description: fread_string(reader, buf2),
                    };
                    obj.ex_descriptions.push(new_descr);
                }
                'A' => {
                    if obj.ex_descriptions.len() >= MAX_OBJ_AFFECT as usize {
                        error!(
                            "SYSERR: Too many A fields ({} max), {}",
                            MAX_OBJ_AFFECT, buf2
                        );
                        process::exit(1);
                    }
                    if get_line(reader, &mut line) == 0 {
                        error!("SYSERR: Format error in 'A' field, {}\n...expecting 2 numeric constants but file ended!", buf2);
                        process::exit(1);
                    }
                    let regex = Regex::new(r"^(-?\+?\d{1,9})\s+(-?\+?\d{1,9})").unwrap();
                    let f = regex.captures(line.as_str());
                    if f.is_none() {
                        error!("SYSERR: Format error in 'A' field, {}\n...expecting 2 numeric arguments\n...offending line: '{}'", buf2, line);
                        process::exit(1);
                    }
                    let f = f.unwrap();

                    obj.affected[j].get().location = f[1].parse::<i32>().unwrap() as u8;
                    obj.affected[j].get().modifier = f[2].parse().unwrap();
                    j += 1;
                }
                '$' | '#' => {
                    self.check_object(&obj);
                    self.obj_proto.push(Rc::from(obj));
                    return line.clone();
                }
                _ => {
                    error!("SYSERR: Format error in ({}): {}", line, buf2);
                    process::exit(1);
                }
            }
        }
    }

    /* load the zone table and command tables */
    fn load_zones(&mut self, fl: File, zonename: &str) {
        //static ZoneRnum zone = 0;
        let mut line_num = 0;
        let mut z = ZoneData {
            name: "".to_string(),
            lifespan: 0,
            age: Cell::from(0),
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
        for _ in 0..3 {
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
        z.number = f[1].parse::<ZoneVnum>().unwrap();

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
        z.bot = f[1].parse::<RoomRnum>().unwrap();
        z.top = f[2].parse::<RoomRnum>().unwrap();
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
                command: Cell::new(0 as char),
                if_flag: false,
                arg1: 0,
                arg2: 0,
                arg3: 0,
                line: 0,
            };

            let original_buf = buf.clone();
            zcmd.command.set(buf.remove(0));

            if zcmd.command.get() == '*' {
                continue;
            }

            if zcmd.command.get() == 'S' || zcmd.command.get() == '$' {
                zcmd.command.set('S');
                break;
            }
            let mut error = 0;
            let mut tmp: i32 = -1;
            if "MOEPD".find(zcmd.command.get()).is_none() {
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
        //*self.top_of_zone_table.borrow_mut() = zone;
    }
}
// #undef Z

fn get_one_line(reader: &mut BufReader<File>, buf: &mut String) {
    let r = reader.read_line(buf);
    if r.is_err() {
        error!("SYSERR: error reading help file: not terminated with $?");
        process::exit(1);
    }

    *buf = buf.trim_end().to_string();
}

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

impl DB {
    pub fn load_help(&mut self, fl: File) {
        let mut entry = String::new();
        let mut key = String::new();
        let mut reader = BufReader::new(fl);
        /* get the first keyword line */
        get_one_line(&mut reader, &mut key);
        while !key.starts_with('$') {
            // strcat(key, "\r\n"); /* strcat: OK (READ_SIZE - "\n" + "\r\n" == READ_SIZE + 1) */
            // entrylen = strlcpy(entry, key, sizeof(entry));
            key.push_str("\r\n");
            entry.push_str(&key);

            /* read in the corresponding help entry */
            let mut line = String::new();
            get_one_line(&mut reader, &mut line);
            while !line.starts_with('#') {
                line.push_str("\r\n");
                entry.push_str(&line);

                // if (entrylen + 2 < sizeof(entry) - 1) {
                // strcpy(entry + entrylen, "\r\n"); /* strcpy: OK (size checked above) */
                // entrylen += 2;
                // }
                line.clear();
                get_one_line(&mut reader, &mut line);
            }

            // if (entrylen > = sizeof(entry) - 1) {
            // int keysize;
            // const char * truncmsg = "\r\n*TRUNCATED*\r\n";

            // strcpy(entry + sizeof(entry) - strlen(truncmsg) - 1, truncmsg); /* strcpy: OK (assuming sane 'entry' size) */
            // keysize = strlen(key) - 2;
            // log("SYSERR: Help entry exceeded buffer space: %.*s", keysize, key);

            /* If we ran out of buffer space, eat the rest of the entry. */
            // while ( *line != '#')
            // get_one_line(fl, line);
            // }

            let mut el = HelpIndexElement {
                keyword: Rc::from(""),
                entry: Rc::from(entry.clone()),
                duplicate: 0,
            };

            let mut next_key = String::new();
            /* now, add the entry to the index with each keyword on the keyword line */
            let mut scan = one_word(&key, &mut next_key);
            while next_key.len() != 0 {
                el.keyword = Rc::from(next_key.clone());
                el.duplicate += 1;
                self.help_table.push(el.clone());
                scan = one_word(&scan, &mut next_key);
            }

            /* get next keyword line (or $) */
            key.clear();
            entry.clear();
            get_one_line(&mut reader, &mut key);
        }
    }
}

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
/*************************************************************************
*  procedures for resetting, both play-time and boot-time	 	 *
*************************************************************************/

pub fn vnum_mobile(db: &DB, searchname: &str, ch: &Rc<CharData>) -> i32 {
    let mut found = 0;
    for nr in 0..db.mob_protos.len() {
        let mp = &db.mob_protos[nr];
        if isname(searchname, &mp.player.borrow().name) {
            found += 1;
            send_to_char(
                ch,
                format!(
                    "{:3}. [{:5}] {}\r\n",
                    found,
                    db.mob_index[nr].vnum,
                    mp.player.borrow().short_descr
                )
                .as_str(),
            );
        }
    }
    return found;
}

pub fn vnum_object(db: &DB, searchname: &str, ch: &Rc<CharData>) -> i32 {
    let mut found = 0;
    for nr in 0..db.obj_proto.len() {
        let op = &db.obj_proto[nr];
        if isname(searchname, &op.name.borrow()) {
            found += 1;
            send_to_char(
                ch,
                format!(
                    "{:3}. [{:5}] {}\r\n",
                    found, db.obj_index[nr].vnum, op.short_description
                )
                .as_str(),
            );
        }
    }

    return found;
}

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

impl DB {
    /* create a new mobile from a prototype */
    pub(crate) fn read_mobile(&self, nr: MobVnum, _type: i32) -> Option<Rc<CharData>> /* and mob_rnum */
    {
        let i;
        if _type == VIRTUAL {
            i = self.real_mobile(nr);
            if i == NOBODY {
                warn!("WARNING: Mobile vnum {} does not exist in database.", nr);
                return None;
            }
        } else {
            i = nr;
        }

        // let mut mob = CharData::new();
        // clear_char(&mut mob);
        let mob = self.mob_protos[i as usize].make_copy();

        if mob.points.borrow().max_hit == 0 {
            let max_hit = dice(
                mob.points.borrow().hit as i32,
                mob.points.borrow().mana as i32,
            ) + mob.points.borrow().movem as i32;
            mob.points.borrow_mut().max_hit = (max_hit) as i16;
        } else {
            let max_hit = rand_number(
                mob.points.borrow().hit as u32,
                mob.points.borrow().mana as u32,
            ) as i16;
            mob.points.borrow_mut().max_hit = max_hit;
        }

        {
            let mut mp = mob.points.borrow_mut();
            mp.hit = mp.max_hit;
            mp.mana = mp.max_mana;
            mp.movem = mp.max_move;
        }
        mob.player.borrow_mut().time.birth = time_now();
        mob.player.borrow_mut().time.played = 0;
        mob.player.borrow_mut().time.logon = time_now();

        self.mob_index[i as usize]
            .number
            .set(self.mob_index[i as usize].number.get() + 1);

        let rc = Rc::from(mob);
        self.character_list.borrow_mut().push(rc.clone());

        Some(rc)
    }

    /* create an object, and add it to the object list */
    // pub fn create_obj(&self) -> Rc<ObjData> {
    //     let mut obj = ObjData::new();
    //
    //     clear_object(&mut obj);
    //     let ret = Rc::from(obj);
    //     self.object_list.borrow_mut().push(ret.clone());
    //     ret
    // }

    /* create a new object from a prototype */
    pub fn read_object(&self, nr: ObjVnum, _type: i32) -> Option<Rc<ObjData>> /* and obj_rnum */ {
        let i = if _type == VIRTUAL {
            self.real_object(nr)
        } else {
            nr
        };

        if i == NOTHING || i >= self.obj_index.len() as i16 {
            warn!(
                "Object ({}) {} does not exist in database.",
                if _type == VIRTUAL { 'V' } else { 'R' },
                nr
            );
            return None;
        }

        let obj = self.obj_proto[i as usize].make_copy();
        let rc = Rc::from(obj);
        self.object_list.borrow_mut().push(rc.clone());

        self.obj_index[i as usize]
            .number
            .set(self.obj_index[i as usize].number.get() + 1);

        Some(rc)
    }
}

const ZO_DEAD: i32 = 999;

impl DB {
    /* update zone ages, queue for reset if necessary, and dequeue when possible */
    pub(crate) fn zone_update(&self, main_globals: &Game) {
        // int i;
        // struct ResetQElement * update_u, * temp;
        // static int timer = 0;

        /* jelson 10/22/92 */
        self.timer.set(self.timer.get());
        if (self.timer.get() * PULSE_ZONE / PASSES_PER_SEC) >= 60 {
            /* one minute has passed */
            /*
             * NOT accurate unless PULSE_ZONE is a multiple of PASSES_PER_SEC or a
             * factor of 60
             */

            self.timer.set(0);

            /* since one minute has passed, increment zone ages */
            for (i, zone) in self.zone_table.borrow().iter().enumerate() {
                if zone.age.get() < zone.lifespan && zone.reset_mode != 0 {
                    zone.age.set(zone.age.get() + 1);
                }

                if zone.age.get() >= zone.lifespan
                    && zone.age.get() < ZO_DEAD
                    && zone.reset_mode != 0
                {
                    /* enqueue zone */
                    self.reset_q.borrow_mut().push(i as RoomRnum);

                    zone.age.set(ZO_DEAD);
                }
            }
        } /* end - one minute has passed */

        /* dequeue zones (if possible) and reset */
        /* this code is executed every 10 seconds (i.e. PULSE_ZONE) */
        for update_u in self.reset_q.borrow().iter() {
            if self.zone_table.borrow()[*update_u as usize].reset_mode == 2
                || main_globals.is_empty(*update_u)
            {
                self.reset_zone(main_globals, *update_u as usize);
                main_globals.mudlog(
                    CMP,
                    LVL_GOD as i32,
                    false,
                    format!(
                        "Auto zone reset: {}",
                        self.zone_table.borrow()[*update_u as usize].name
                    )
                    .as_str(),
                );
            }
        }
        self.reset_q.borrow_mut().clear();
    }

    // #define ZONE_ERROR(message) \
    // { log_zone_error(zone, cmd_no, message); last_cmd = 0; }

    /* execute the reset command table of a given zone */
    fn log_zone_error(
        &self,
        main_globals: &Game,
        zone: usize,
        cmd_no: i32,
        zcmd: &ResetCom,
        message: &str,
        last_cmd: &mut i32,
    ) {
        main_globals.mudlog(
            NRM,
            LVL_GOD as i32,
            true,
            format!("SYSERR: zone file: {}", message).as_str(),
        );
        main_globals.mudlog(
            NRM,
            LVL_GOD as i32,
            true,
            format!(
                "SYSERR: ...offending cmd: '{}' cmd in zone #{}, line {}",
                zcmd.command.get(),
                self.zone_table.borrow()[zone as usize].number,
                zcmd.line
            )
            .as_str(),
        );
        *last_cmd = 0;
    }

    fn reset_zone(&self, main_globals: &Game, zone: usize) {
        //int cmd_no, last_cmd = 0;
        //struct char_data *mob = NULL;
        //struct obj_data * obj, *obj_to;
        let mut last_cmd = 0;
        let mut obj;
        let mut mob = None;
        for cmd_no in 0..self.zone_table.borrow()[zone as usize].cmd.len() {
            let zcmd = &self.zone_table.borrow()[zone as usize].cmd[cmd_no as usize];
            if zcmd.command.get() == 'S' {
                break;
            }
            if zcmd.if_flag && last_cmd == 0 {
                continue;
            }

            /*  This is the list of actual zone commands.  If any new
             *  zone commands are added to the game, be certain to update
             *  the list of commands in load_zone() so that the counting
             *  will still be correct. - ae.
             */
            match zcmd.command.get() {
                '*' => {
                    /* ignore command */
                    last_cmd = 0;
                    break;
                }

                'M' => {
                    /* read a mobile */
                    if self.mob_index[zcmd.arg1 as usize].number.get() < zcmd.arg2 {
                        mob = self.read_mobile(zcmd.arg1 as MobVnum, REAL);
                        self.char_to_room(mob.as_ref(), zcmd.arg3 as RoomRnum);
                        last_cmd = 1;
                    } else {
                        last_cmd = 0;
                    }
                }

                'O' => {
                    /* read an object */
                    if self.obj_index[zcmd.arg1 as usize].number.get() < zcmd.arg2 {
                        if zcmd.arg3 != NOWHERE as i32 {
                            obj = self.read_object(zcmd.arg1 as ObjVnum, REAL);
                            self.obj_to_room(obj.as_ref(), zcmd.arg3 as RoomRnum);
                            last_cmd = 1;
                        } else {
                            obj = self.read_object(zcmd.arg1 as ObjVnum, REAL);
                            obj.as_ref().unwrap().in_room.set(NOWHERE);
                            last_cmd = 1;
                        }
                    } else {
                        last_cmd = 0;
                    }
                }

                'P' => {
                    /* object to object */
                    if self.obj_index[zcmd.arg1 as usize].number.get() < zcmd.arg2 {
                        obj = self.read_object(zcmd.arg1 as ObjVnum, REAL);
                        let obj_to = self.get_obj_num(zcmd.arg3 as ObjRnum);
                        if obj_to.is_none() {
                            self.log_zone_error(
                                main_globals,
                                zone,
                                cmd_no as i32,
                                zcmd,
                                "target obj not found, command disabled",
                                &mut last_cmd,
                            );
                            zcmd.command.set('*');
                            break;
                        }
                        self.obj_to_obj(obj.as_ref(), obj_to.as_ref());
                        last_cmd = 1;
                    } else {
                        last_cmd = 0;
                    }
                }

                'G' => {
                    /* obj_to_char */
                    if mob.is_none() {
                        self.log_zone_error(
                            main_globals,
                            zone,
                            cmd_no as i32,
                            zcmd,
                            "attempt to give obj to non-existant mob, command disabled",
                            &mut last_cmd,
                        );

                        zcmd.command.set('*');
                        break;
                    }
                    if self.obj_index[zcmd.arg1 as usize].number.get() < zcmd.arg2 {
                        obj = self.read_object(zcmd.arg1 as ObjVnum, REAL);
                        DB::obj_to_char(obj.as_ref(), mob.as_ref());
                        last_cmd = 1;
                    } else {
                        last_cmd = 0;
                    }
                }

                'E' => {
                    /* object to equipment list */
                    if mob.is_none() {
                        self.log_zone_error(
                            main_globals,
                            zone,
                            cmd_no as i32,
                            zcmd,
                            "trying to equip non-existant mob, command disabled",
                            &mut last_cmd,
                        );

                        zcmd.command.set('*');
                        break;
                    }
                    if self.obj_index[zcmd.arg1 as usize].number.get() < zcmd.arg2 {
                        if zcmd.arg3 < 0 || zcmd.arg3 >= NUM_WEARS as i32 {
                            self.log_zone_error(
                                main_globals,
                                zone,
                                cmd_no as i32,
                                zcmd,
                                "invalid equipment pos number",
                                &mut last_cmd,
                            );
                        } else {
                            obj = self.read_object(zcmd.arg1 as ObjVnum, REAL);
                            self.equip_char(mob.as_ref(), obj.as_ref(), zcmd.arg3 as i8);
                            last_cmd = 1;
                        }
                    } else {
                        last_cmd = 0;
                    }
                }

                'R' => {
                    /* rem obj from room */
                    obj = self.get_obj_in_list_num(
                        zcmd.arg2 as i16,
                        self.world.borrow()[zcmd.arg1 as usize]
                            .contents
                            .borrow()
                            .as_ref(),
                    );
                    if obj.is_some() {
                        self.extract_obj(obj.as_ref().unwrap());
                    }
                    last_cmd = 1;
                }

                'D' => {
                    /* set state of door */
                    if zcmd.arg2 < 0
                        || zcmd.arg2 >= NUM_OF_DIRS as i32
                        || (self.world.borrow()[zcmd.arg1 as usize].dir_option[zcmd.arg2 as usize]
                            .is_none())
                    {
                        self.log_zone_error(
                            main_globals,
                            zone,
                            cmd_no as i32,
                            zcmd,
                            "door does not exist, command disabled",
                            &mut last_cmd,
                        );
                        zcmd.command.set('*');
                    } else {
                        match zcmd.arg3 {
                            0 => {
                                self.world.borrow()[zcmd.arg1 as usize].dir_option
                                    [zcmd.arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .remove_exit_info_bit(EX_LOCKED as i32);
                                self.world.borrow()[zcmd.arg1 as usize].dir_option
                                    [zcmd.arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .remove_exit_info_bit(EX_CLOSED as i32);
                            }

                            1 => {
                                self.world.borrow()[zcmd.arg1 as usize].dir_option
                                    [zcmd.arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .set_exit_info_bit(EX_LOCKED as i32);
                                self.world.borrow()[zcmd.arg1 as usize].dir_option
                                    [zcmd.arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .remove_exit_info_bit(EX_CLOSED as i32);
                            }

                            2 => {
                                self.world.borrow()[zcmd.arg1 as usize].dir_option
                                    [zcmd.arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .set_exit_info_bit(EX_LOCKED as i32);
                                self.world.borrow()[zcmd.arg1 as usize].dir_option
                                    [zcmd.arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .set_exit_info_bit(EX_CLOSED as i32);
                            }
                            _ => {}
                        }
                    }
                    last_cmd = 1;
                }

                _ => {
                    self.log_zone_error(
                        main_globals,
                        zone,
                        cmd_no as i32,
                        zcmd,
                        "unknown cmd in reset table; cmd disabled",
                        &mut last_cmd,
                    );
                    zcmd.command.set('*');
                }
            }
        }

        self.zone_table.borrow()[zone as usize].age.set(0);
    }
}

// /* for use in reset_zone; return TRUE if zone 'nr' is free of PC's  */
impl Game {
    fn is_empty(&self, zone_nr: ZoneRnum) -> bool {
        for i in self.descriptor_list.borrow().iter() {
            if i.state() != ConPlaying {
                continue;
            }
            if i.character.borrow().as_ref().unwrap().in_room() == NOWHERE {
                continue;
            }
            if i.character.borrow().as_ref().unwrap().get_level() >= LVL_IMMORT as u8 {
                continue;
            }
            if self.db.world.borrow()[i.character.borrow().as_ref().unwrap().in_room() as usize]
                .zone
                != zone_nr
            {
                continue;
            }
            return false;
        }
        true
    }
}
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

impl DB {
    /* Load a char, TRUE if loaded, FALSE if not */
    pub fn load_char(&self, name: &str, char_element: &mut CharFileU) -> Option<usize> {
        let player_i = self.get_ptable_by_name(name);
        if player_i.is_none() {
            return player_i;
        }
        let player_i = player_i.unwrap();
        let mut t = self.player_fl.borrow_mut();
        let pfile = t.as_mut().unwrap();

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
        let mut st: CharFileU = CharFileU::new();

        if ch.is_npc() || ch.desc.borrow().is_none() || ch.get_pfilepos() < 0 {
            return;
        }

        char_to_store(ch, &mut st);

        copy_to_stored(
            &mut st.host,
            ch.desc.borrow().as_ref().unwrap().host.borrow().as_str(),
        );

        let record_size = mem::size_of::<CharFileU>();
        // self.player_fl.borrow_mut().as_mut().unwrap()
        //     .fseek(SeekFrom::Start((ch.get_pfilepos() * record_size) as u64))
        //     .expect("Error while seeking for writing player");
        unsafe {
            let player_slice = slice::from_raw_parts(&mut st as *mut _ as *mut u8, record_size);
            self.player_fl
                .borrow_mut()
                .as_mut()
                .unwrap()
                .write_all_at(
                    player_slice,
                    (ch.get_pfilepos() as usize * record_size) as u64,
                )
                .expect("Error while writing player record to file");
        }
    }
}

impl CharFileU {
    pub fn new() -> CharFileU {
        CharFileU {
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
    ch.player.borrow_mut().description = Rc::new(RefCell::from(parse_c_string(&st.description)));

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
    ch.player.borrow_mut().name = parse_c_string(&st.name);
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
    if !RefCell::borrow(&ch.player.borrow().description).is_empty() {
        if RefCell::borrow(&ch.player.borrow().description).len() >= st.description.len() {
            error!(
                "SYSERR: char_to_store: {}'s description length: {}, max: {}!  Truncated.",
                ch.get_pc_name(),
                RefCell::borrow(&ch.player.borrow().description).len(),
                st.description.len()
            );
            RefCell::borrow_mut(&ch.player.borrow().description)
                .truncate(&st.description.len() - 3);
            RefCell::borrow_mut(&ch.player.borrow().description).push_str("\r\n");
        }
        copy_to_stored(
            &mut st.description,
            &RefCell::borrow(&ch.player.borrow().description),
        );
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
pub fn fread_string(reader: &mut BufReader<File>, error: &str) -> String {
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
impl Game {
    fn file_to_string_alloc<'a>(&self, name: &'a str, buf: &'a mut Rc<str>) -> i32 {
        //int temppage;
        //char temp[MAX_STRING_LENGTH];
        //struct descriptor_data *in_use;

        for in_use in &*self.descriptor_list.borrow() {
            if &in_use.showstr_vector.borrow_mut()[0].as_ref() == &buf.as_ref() {
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
            if in_use.showstr_count.get() == 0
                || in_use.showstr_vector.borrow()[0].as_ref() != buf.as_ref()
            {
                continue;
            }

            let temppage = in_use.showstr_page.get();
            *in_use.showstr_head.borrow_mut() = Some(in_use.showstr_vector.borrow()[0].clone());
            in_use.showstr_page.set(temppage);
            paginate_string(in_use.showstr_head.borrow().as_ref().unwrap(), in_use);
        }
        *buf = Rc::from(temp);
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
    for i in 0..NUM_WEARS {
        ch.set_eq(i, None);
    }

    ch.followers.borrow_mut().clear();
    *ch.master.borrow_mut() = None;
    ch.set_in_room(NOWHERE);
    ch.carrying.borrow_mut().clear();
    *ch.next.borrow_mut() = None;
    // *ch.next_fighting.borrow_mut() = None;
    *ch.next_in_room.borrow_mut() = None;
    ch.set_fighting(None);
    ch.char_specials.borrow_mut().position = POS_STANDING;
    //ch.mob_specials.borrow_mut().default_pos = POS_STANDING;
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

/* clear ALL the working variables of a char; do NOT free any space alloc'ed */
pub fn clear_char(ch: &mut CharData) {
    //memset((char *) ch, 0, sizeof(struct char_data));

    ch.set_in_room(NOWHERE);
    ch.set_pfilepos(-1);

    ch.set_mob_rnum(NOBODY);
    ch.set_was_in(NOWHERE);
    ch.set_pos(POS_STANDING);
    ch.mob_specials.default_pos = POS_STANDING;

    ch.set_ac(100); /* Basic Armor */
    if ch.points.borrow().max_mana < 100 {
        ch.points.borrow_mut().max_mana = 100;
    }
}

fn clear_object(obj: &mut ObjData) {
    obj.item_number = NOTHING;
    obj.set_in_room(NOWHERE);
    obj.worn_on.set(NOWHERE);
}

/*
 * Called during character creation after picking character class
 * (and then never again for that character).
 */
impl DB {
    pub(crate) fn init_char(&self, ch: &CharData) {
        /* create a player_special structure */
        // if ch.player_specials
        // CREATE(ch->player_specials, struct player_special_data, 1);

        /* *** if this is our first player --- he be God *** */
        if self.player_table.borrow().len() == 1 {
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
        ch.player.borrow_mut().description = Rc::new(RefCell::new(String::new()));

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
            //*self.top_idnum.borrow_mut() += 1;
            let top_n = self.player_table.borrow().len();
            self.player_table.borrow_mut()[i].id = top_n as i64; //*self.top_idnum.borrow() as i64;
            ch.set_idnum(top_n as i64); /*self.top_idnum.borrow()*/
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

impl DB {
    // /* returns the real number of the room with given virtual number */
    pub fn real_room(&self, vnum: RoomRnum) -> RoomRnum {
        let r = self
            .world
            .borrow()
            .binary_search_by_key(&vnum, |idx| idx.number);
        if r.is_err() {
            return NOWHERE;
        }
        r.unwrap() as RoomRnum
    }

    /* returns the real number of the monster with given virtual number */
    pub fn real_mobile(&self, vnum: MobVnum) -> MobRnum {
        let r = self.mob_index.binary_search_by_key(&vnum, |idx| idx.vnum);
        if r.is_err() {
            return NOBODY;
        }
        r.unwrap() as MobRnum
    }

    /* returns the real number of the object with given virtual number */
    pub fn real_object(&self, vnum: ObjVnum) -> ObjRnum {
        let r = self.obj_index.binary_search_by_key(&vnum, |idx| idx.vnum);
        if r.is_err() {
            return NOBODY;
        }
        r.unwrap() as ObjRnum
    }
}

// /* returns the real number of the zone with given virtual number */
// RoomRnum real_zone(RoomRnum vnum)
// {
// RoomRnum bot, top, mid;
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

/*
 * Extend later to include more checks.
 *
 * TODO: Add checks for unknown bitvectors.
 */
impl DB {
    fn check_object(&self, obj: &ObjData) -> bool {
        let mut error = false;

        if obj.get_obj_weight() < 0 {
            error = true;
            info!(
                "SYSERR: Object #{} ({}) has negative weight ({}).",
                self.get_obj_vnum(obj),
                obj.short_description,
                obj.get_obj_weight()
            );
        }

        if obj.get_obj_rent() < 0 {
            error = true;
            error!(
                "SYSERR: Object #{} ({}) has negative cost/day ({}).",
                self.get_obj_vnum(obj),
                obj.short_description,
                obj.get_obj_rent()
            );
        }

        let objname = format!(
            "Object #{} ({})",
            self.get_obj_vnum(obj),
            obj.short_description
        );
        error |= check_bitvector_names(
            obj.get_obj_wear() as i64,
            WEAR_BITS_COUNT,
            objname.as_str(),
            "object wear",
        );
        error |= check_bitvector_names(
            obj.get_obj_extra() as i64,
            EXTRA_BITS_COUNT,
            objname.as_str(),
            "object extra",
        );
        error |= check_bitvector_names(
            obj.get_obj_affect(),
            AFFECTED_BITS_COUNT,
            objname.as_str(),
            "object affect",
        );

        // match obj.get_obj_type()
        // {
        //     ITEM_DRINKCON => {
        //         char
        //         onealias[MAX_INPUT_LENGTH], *space = strrchr(obj->name, ' ');
        //
        //         strlcpy(onealias, space? space + 1: obj->name, sizeof(onealias));
        //         if (search_block(onealias, DRINKNAMES, TRUE) < 0 && (error = TRUE))
        //         log("SYSERR: Object #%d (%s) doesn't have drink type as last alias. (%s)",
        //             GET_ObjVnum(obj), obj->short_description, obj->name);
        //     }
        //     /* Fall through. */
        //     case
        //     ITEM_FOUNTAIN:
        //     if (GET_OBJ_VAL(obj, 1) > GET_OBJ_VAL(obj, 0) && (error = TRUE))
        //     log("SYSERR: Object #%d (%s) contains (%d) more than maximum (%d).",
        //         GET_ObjVnum(obj), obj->short_description,
        //         GET_OBJ_VAL(obj, 1), GET_OBJ_VAL(obj, 0));
        //     break;
        //     case
        //     ITEM_SCROLL:
        //         case
        //     ITEM_POTION:
        //         error |= check_object_level(obj, 0);
        //     error |= check_object_spell_number(obj, 1);
        //     error |= check_object_spell_number(obj, 2);
        //     error |= check_object_spell_number(obj, 3);
        //     break;
        //     case
        //     ITEM_WAND:
        //         case
        //     ITEM_STAFF:
        //         error |= check_object_level(obj, 0);
        //     error |= check_object_spell_number(obj, 3);
        //     if (GET_OBJ_VAL(obj, 2) > GET_OBJ_VAL(obj, 1) && (error = TRUE))
        //     log("SYSERR: Object #%d (%s) has more charges (%d) than maximum (%d).",
        //         GET_ObjVnum(obj), obj->short_description,
        //         GET_OBJ_VAL(obj, 2), GET_OBJ_VAL(obj, 1));
        //     break;
        // }

        return error;
    }
}

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
// GET_ObjVnum(obj), obj->short_description, GET_OBJ_VAL(obj, val));
//
// /*
//  * This bug has been fixed, but if you don't like the special behavior...
//  */
// #if 0
// if (GET_OBJ_TYPE(obj) == ITEM_STAFF &&
// HAS_SPELL_ROUTINE(GET_OBJ_VAL(obj, val), MAG_AREAS | MAG_MASSES))
// log("... '%s' (#%d) uses %s spell '%s'.",
// obj->short_description,	GET_ObjVnum(obj),
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
// GET_ObjVnum(obj), obj->short_description, spellname,
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
// GET_ObjVnum(obj), obj->short_description, GET_OBJ_VAL(obj, val));
//
// return (error);
// }

fn check_bitvector_names(bits: i64, namecount: usize, whatami: &str, whatbits: &str) -> bool {
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

const NONE_OBJDATA: Option<Rc<ObjData>> = None;

impl CharData {
    pub fn new() -> CharData {
        CharData {
            pfilepos: RefCell::new(0),
            nr: 0,
            in_room: Cell::new(0),
            was_in_room: Cell::new(0),
            wait: Cell::new(0),
            player: RefCell::new(CharPlayerData {
                passwd: [0; 16],
                name: "".to_string(),
                short_descr: "".to_string(),
                long_descr: "".to_string(),
                description: Rc::new(RefCell::new(String::new())),
                title: Option::from("".to_string()),
                sex: 0,
                chclass: 0,
                level: 0,
                hometown: 0,
                time: TimeData {
                    birth: 0,
                    logon: 0,
                    played: 0,
                },
                weight: 0,
                height: 0,
            }),
            real_abils: RefCell::new(CharAbilityData {
                str: 0,
                str_add: 0,
                intel: 0,
                wis: 0,
                dex: 0,
                con: 0,
                cha: 0,
            }),
            aff_abils: RefCell::new(CharAbilityData {
                str: 0,
                str_add: 0,
                intel: 0,
                wis: 0,
                dex: 0,
                con: 0,
                cha: 0,
            }),
            points: RefCell::new(CharPointData {
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
            }),
            char_specials: RefCell::new(CharSpecialData {
                fighting: None,
                hunting: None,
                position: 0,
                carry_weight: 0,
                carry_items: 0,
                timer: Cell::new(0),
                saved: CharSpecialDataSaved {
                    alignment: 0,
                    idnum: 0,
                    act: 0,
                    affected_by: 0,
                    apply_saving_throw: [0; 5],
                },
            }),
            player_specials: RefCell::new(PlayerSpecialData {
                saved: PlayerSpecialDataSaved {
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
                poofin: Rc::from(""),
                poofout: Rc::from(""),
                last_tell: 0,
            }),
            mob_specials: MobSpecialData {
                memory: RefCell::new(vec![]),
                attack_type: 0,
                default_pos: 0,
                damnodice: 0,
                damsizedice: 0,
            },
            affected: RefCell::new(vec![]),
            equipment: RefCell::new([NONE_OBJDATA; NUM_WEARS as usize]),
            carrying: RefCell::new(vec![]),
            desc: RefCell::new(None),
            next_in_room: RefCell::new(None),
            next: RefCell::new(None),
            // next_fighting: RefCell::new(None),
            followers: RefCell::new(vec![]),
            master: RefCell::new(None),
        }
    }
    fn make_copy(&self) -> CharData {
        CharData {
            pfilepos: RefCell::new(self.get_pfilepos()),
            nr: self.nr,
            in_room: Cell::new(self.in_room()),
            was_in_room: Cell::new(self.was_in_room.get()),
            wait: Cell::new(self.wait.get()),
            player: RefCell::new(CharPlayerData {
                passwd: self.player.borrow().passwd,
                name: self.player.borrow().name.clone(),
                short_descr: self.player.borrow().short_descr.clone(),
                long_descr: self.player.borrow().long_descr.clone(),
                description: self.player.borrow().description.clone(),
                title: Option::from("".to_string()),
                sex: self.player.borrow().sex,
                chclass: self.player.borrow().chclass,
                level: self.player.borrow().level,
                hometown: self.player.borrow().hometown,
                time: self.player.borrow().time,
                weight: self.player.borrow().weight,
                height: self.player.borrow().height,
            }),
            real_abils: RefCell::new(*self.real_abils.borrow()),
            aff_abils: RefCell::new(*self.aff_abils.borrow()),
            points: RefCell::new(*self.points.borrow()),
            char_specials: RefCell::new(CharSpecialData {
                fighting: None,
                hunting: None,
                position: self.char_specials.borrow().position,
                carry_weight: self.char_specials.borrow().carry_weight,
                carry_items: self.char_specials.borrow().carry_items,
                timer: Cell::from(self.char_specials.borrow().timer.get()),
                saved: self.char_specials.borrow().saved,
            }),
            player_specials: RefCell::new(PlayerSpecialData {
                saved: PlayerSpecialDataSaved {
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
                poofin: Rc::from(""),
                poofout: Rc::from(""),
                last_tell: 0,
            }),
            mob_specials: MobSpecialData {
                memory: RefCell::new(vec![]),
                attack_type: self.mob_specials.attack_type,
                default_pos: self.mob_specials.default_pos,
                damnodice: self.mob_specials.damnodice,
                damsizedice: self.mob_specials.damsizedice,
            },
            affected: RefCell::new(vec![]),
            equipment: RefCell::new([NONE_OBJDATA; NUM_WEARS as usize]),
            carrying: RefCell::new(vec![]),
            desc: RefCell::new(None),
            next_in_room: RefCell::new(None),
            next: RefCell::new(None),
            // next_fighting: RefCell::new(None),
            followers: RefCell::new(vec![]),
            master: RefCell::new(None),
        }
    }
}

impl ObjData {
    pub(crate) fn new() -> ObjData {
        ObjData {
            item_number: 0,
            in_room: Cell::new(0),
            obj_flags: ObjFlagData {
                value: [Cell::new(0), Cell::new(0), Cell::new(0), Cell::new(0)],
                type_flag: 0,
                wear_flags: 0,
                extra_flags: Cell::new(0),
                weight: Cell::new(0),
                cost: 0,
                cost_per_day: 0,
                timer: Cell::new(0),
                bitvector: Cell::new(0),
            },
            affected: [
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
            ],
            name: RefCell::from("".to_string()),
            description: "".to_string(),
            short_description: "".to_string(),
            action_description: Rc::new(RefCell::new(String::new())),
            ex_descriptions: vec![],
            carried_by: RefCell::new(None),
            worn_by: RefCell::new(None),
            worn_on: Cell::new(0),
            in_obj: RefCell::new(None),
            contains: RefCell::new(vec![]),
            next_content: RefCell::new(None),
            next: RefCell::new(None),
        }
    }
    fn make_copy(&self) -> ObjData {
        let mut ret = ObjData {
            item_number: self.item_number,
            in_room: Cell::from(self.in_room.get()),
            obj_flags: ObjFlagData {
                value: [
                    Cell::from(self.obj_flags.value[0].get()),
                    Cell::from(self.obj_flags.value[1].get()),
                    Cell::from(self.obj_flags.value[2].get()),
                    Cell::from(self.obj_flags.value[3].get()),
                ],
                type_flag: self.obj_flags.type_flag,
                wear_flags: self.obj_flags.wear_flags,
                extra_flags: Cell::from(self.obj_flags.extra_flags.get()),
                weight: Cell::from(self.obj_flags.weight.get()),
                cost: self.obj_flags.cost,
                cost_per_day: self.obj_flags.cost_per_day,
                timer: Cell::from(self.obj_flags.timer.get()),
                bitvector: Cell::from(self.obj_flags.bitvector.get()),
            },
            affected: [
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
                Cell::from(ObjAffectedType {
                    location: 0,
                    modifier: 0,
                }),
            ],
            name: self.name.clone(),
            description: self.description.clone(),
            short_description: self.short_description.clone(),
            action_description: self.action_description.clone(),
            ex_descriptions: vec![],
            carried_by: RefCell::new(None),
            worn_by: RefCell::new(None),
            worn_on: Cell::new(0),
            in_obj: RefCell::new(None),
            contains: RefCell::new(vec![]),
            next_content: RefCell::new(None),
            next: RefCell::new(None),
        };
        for x in &self.ex_descriptions {
            ret.ex_descriptions.push(ExtraDescrData {
                keyword: x.keyword.clone(),
                description: x.description.clone(),
            })
        }
        ret
    }
}
