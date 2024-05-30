/* ************************************************************************
*   File: db.rs                                         Part of CircleMUD *
*  Usage: Loading/saving chars, booting/resetting world, internal funcs   *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
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

use crate::act_informative::sort_commands;
use crate::act_social::{boot_social_messages, SocialMessg};
use crate::ban::{load_banned, read_invalid_list};
use crate::boards::BoardSystem;
use crate::castle::KingWelmar;
use crate::class::init_spell_levels;
use crate::config::{FROZEN_START_ROOM, IMMORT_START_ROOM, MORTAL_START_ROOM, OK};
use crate::constants::{
    ACTION_BITS_COUNT, AFFECTED_BITS_COUNT, DRINKNAMES, EXTRA_BITS_COUNT, ROOM_BITS_COUNT,
    WEAR_BITS_COUNT,
};
use crate::handler::{affect_remove, fname, isname};
use crate::house::{house_boot, HouseControlRec, MAX_HOUSES};
use crate::interpreter::{one_argument, one_word, search_block};
use crate::mail::MailSystem;
use crate::modify::paginate_string;
use crate::objsave::update_obj_file;
use crate::shops::{assign_the_shopkeepers, boot_the_shops, destroy_shops, ShopData};
use crate::spec_assign::{assign_mobiles, assign_objects, assign_rooms};
use crate::spec_procs::{sort_spells, Mayor};
use crate::spell_parser::{mag_assign_spells, skill_name, UNUSED_SPELLNAME};
use crate::spells::{SpellInfoType, MAX_SPELLS, TOP_SPELL_DEFINE};
use crate::structs::ConState::ConPlaying;
use crate::structs::{
    AffectedType, CharAbilityData, CharData, CharFileU, CharPlayerData, CharPointData,
    CharSpecialData, CharSpecialDataSaved, ExtraDescrData, IndexData, MessageList, MobRnum,
    MobSpecialData, MobVnum, ObjAffectedType, ObjData, ObjFlagData, ObjRnum, ObjVnum,
    PlayerSpecialData, PlayerSpecialDataSaved, RoomData, RoomDirectionData, RoomRnum, RoomVnum,
    TimeData, TimeInfoData, WeatherData, ZoneRnum, ZoneVnum, AFF_POISON, APPLY_NONE, EX_CLOSED,
    EX_ISDOOR, EX_LOCKED, EX_PICKPROOF, HOST_LENGTH, ITEM_DRINKCON, ITEM_FOUNTAIN, ITEM_POTION,
    ITEM_SCROLL, ITEM_STAFF, ITEM_WAND, LVL_GOD, LVL_IMMORT, LVL_IMPL, MAX_AFFECT, MAX_NAME_LENGTH,
    MAX_OBJ_AFFECT, MAX_PWD_LENGTH, MAX_SKILLS, MAX_TITLE_LENGTH, MAX_TONGUE, MOB_AGGRESSIVE,
    MOB_AGGR_EVIL, MOB_AGGR_GOOD, MOB_AGGR_NEUTRAL, MOB_ISNPC, MOB_NOTDEADYET, NOBODY, NOTHING,
    NOWHERE, NUM_OF_DIRS, NUM_WEARS, PASSES_PER_SEC, POS_STANDING, PULSE_ZONE, SEX_MALE,
    SKY_CLOUDLESS, SKY_CLOUDY, SKY_LIGHTNING, SKY_RAINING, SUN_DARK, SUN_LIGHT, SUN_RISE, SUN_SET,
};
use crate::util::{
   dice, get_line, mud_time_passed, mud_time_to_secs, prune_crlf, rand_number,
    time_now, touch, CMP, NRM, SECS_PER_REAL_HOUR,
};
use crate::{check_player_special, get_last_tell_mut, send_to_char, Game};

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
pub const MAIL_FILE: &str = "etc/plrmail"; /* for the mudmail system	*/
pub const BAN_FILE: &str = "etc/badsites"; /* for the siteban system	*/
pub const HCONTROL_FILE: &str = "etc/hcontrol"; /* for the house system	*/
pub const TIME_FILE: &str = "etc/time";

pub const LIB_PLRALIAS: &str = "plralias/";

pub const SUF_OBJS: &str = "objs";
pub const SUF_TEXT: &str = "text";
pub const SUF_ALIAS: &str = "alias";

pub struct PlayerIndexElement {
    pub(crate) name: String,
    id: i64,
}

pub struct HelpIndexElement {
    pub keyword: Rc<str>,
    pub entry: Rc<str>,
    pub duplicate: i32,
}

pub struct DB {
    pub world: RefCell<Vec<Rc<RoomData>>>,
    pub character_list: Vec<Rc<CharData>>,
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
    pub(crate) player_table: RefCell<Vec<PlayerIndexElement>>,
    /* index to plr file	 */
    pub(crate) player_fl: RefCell<Option<File>>,
    /* file desc of player file	 */
    top_idnum: Cell<i32>,
    /* highest idnum in use		 */
    pub no_mail: Cell<bool>,
    /* mail disabled?		 */
    pub mini_mud: bool,
    /* mini-mud mode?		 */
    pub no_rent_check: bool,
    /* skip rent check on boot?	 */
    pub boot_time: Cell<u64>,
    pub no_specials: bool,
    /* time of mud boot		 */
    pub circle_restrict: Cell<u8>,
    /* level of game restriction	 */
    pub r_mortal_start_room: Cell<RoomRnum>,
    /* rnum of mortal start room	 */
    pub r_immort_start_room: Cell<RoomRnum>,
    /* rnum of immort start room	 */
    pub r_frozen_start_room: Cell<RoomRnum>,
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
    pub time_info: Cell<TimeInfoData>,
    /* the infomation about the time    */
    pub weather_info: Cell<WeatherData>,
    /* the infomation about the weather */
    // struct player_special_data dummy_mob;	/* dummy spec area for mobs	*/
    pub reset_q: RefCell<Vec<ZoneRnum>>,
    pub extractions_pending: Cell<i32>,
    pub timer: Cell<u128>,
    pub cmd_sort_info: Vec<usize>,
    pub combat_list: RefCell<Vec<Rc<CharData>>>,
    pub shop_index: Vec<ShopData>,
    pub spell_sort_info: [i32; MAX_SKILLS + 1],
    pub spell_info: [SpellInfoType; TOP_SPELL_DEFINE + 1],
    pub soc_mess_list: Vec<SocialMessg>,
    pub ban_list: RefCell<Vec<BanListElement>>,
    pub invalid_list: RefCell<Vec<Rc<str>>>,
    pub boards: RefCell<BoardSystem>,
    pub house_control: RefCell<[HouseControlRec; MAX_HOUSES]>,
    pub num_of_houses: Cell<usize>,
    pub mails: RefCell<MailSystem>,
    pub(crate) mayor: RefCell<Mayor>,
    pub(crate) king_welmar: KingWelmar,
    pub scheck: bool,
}

pub const REAL: i32 = 0;
pub const VIRTUAL: i32 = 1;

/* structure for the reset commands */
struct ResetCom {
    command: Cell<char>,
    /* current command                      */
    if_flag: bool,
    /* if TRUE: exe only if preceding exe'd */
    arg1: i32,
    /*                                      */
    arg2: i32,
    /* Arguments to the command             */
    arg3: i32,
    /*                                      */
    line: i32,
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
    cmd: Vec<ResetCom>,
    /* command table for reset	          */

    /*
     * Reset mode:
     *   0: Don't reset, and don't update age.
     *   1: Reset if no PC's are located in zone.
     *   2: Just reset.
     */
}

/* don't change these */
pub const BAN_NEW: i32 = 1;
pub const BAN_SELECT: i32 = 2;
pub const BAN_ALL: i32 = 3;

pub struct BanListElement {
    pub site: Rc<str>,
    pub type_: i32,
    pub date: u64,
    pub name: Rc<str>,
}

impl DB {
    pub fn get_name_by_id(&self, id: i64) -> Option<String> {
        let pt = self.player_table.borrow();
        pt.iter().find(|p| p.id == id).map(|p| p.name.clone())
    }

    pub fn get_id_by_name(&self, name: &str) -> i64 {
        let pt = self.player_table.borrow();
        let r = pt.iter().find(|p| p.name == name);
        if r.is_some() {
            r.unwrap().id
        } else {
            -1
        }
    }
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
/* Wipe out all the loaded text files, for shutting down. */
impl DB {
    pub fn free_text_files(&mut self) {
        let textfiles = [
            &mut self.wizlist,
            &mut self.immlist,
            &mut self.news,
            &mut self.credits,
            &mut self.motd,
            &mut self.imotd,
            &mut self.help,
            &mut self.info,
            &mut self.policies,
            &mut self.handbook,
            &mut self.background,
            &mut self.greetings,
        ];

        for rf in textfiles {
            *rf = Rc::from("");
        }
    }
}

/*
 * Too bad it doesn't check the return values to let the user
 * know about -1 values.  This will result in an 'Okay.' to a
 * 'reload' command even when the string was not replaced.
 * To fix later, if desired. -gg 6/24/99
 */
pub fn do_reboot(game: &mut Game, ch: &Rc<CharData>, argument: &str, _cmd: usize, _subcmd: i32) {
    let mut arg = String::new();

    one_argument(argument, &mut arg);
    let mut n = Rc::from("");
    match arg.as_str() {
        "all" | "*" => {
            if game.file_to_string_alloc(GREETINGS_FILE, &mut n) == 0 {
                game.db.greetings = n.clone();
                prune_crlf(&mut game.db.greetings);
            }
            game.file_to_string_alloc(WIZLIST_FILE, &mut n);
            game.db.wizlist = n.clone();
            game.file_to_string_alloc(IMMLIST_FILE, &mut n);
            game.db.immlist = n.clone();
            game.file_to_string_alloc(NEWS_FILE, &mut n);
            game.db.news = n.clone();
            game.file_to_string_alloc(CREDITS_FILE, &mut n);
            game.db.credits = n.clone();
            game.file_to_string_alloc(MOTD_FILE, &mut n);
            game.db.motd = n.clone();
            game.file_to_string_alloc(IMOTD_FILE, &mut n);
            game.db.imotd = n.clone();
            game.file_to_string_alloc(HELP_PAGE_FILE, &mut n);
            game.db.help = n.clone();
            game.file_to_string_alloc(INFO_FILE, &mut n);
            game.db.info = n.clone();
            game.file_to_string_alloc(POLICIES_FILE, &mut n);
            game.db.policies = n.clone();
            game.file_to_string_alloc(HANDBOOK_FILE, &mut n);
            game.db.handbook = n.clone();
            game.file_to_string_alloc(BACKGROUND_FILE, &mut n);
            game.db.background = n.clone();
        }
        "wizlist" => {
            game.file_to_string_alloc(WIZLIST_FILE, &mut n);
            game.db.wizlist = n.clone();
        }
        "immlist" => {
            game.file_to_string_alloc(IMMLIST_FILE, &mut n);
            game.db.immlist = n.clone();
        }
        "news" => {
            game.file_to_string_alloc(NEWS_FILE, &mut n);
            game.db.news = n.clone();
        }
        "credits" => {
            game.file_to_string_alloc(CREDITS_FILE, &mut n);
            game.db.credits = n.clone();
        }
        "motd" => {
            game.file_to_string_alloc(MOTD_FILE, &mut n);
            game.db.motd = n.clone();
        }
        "imotd" => {
            game.file_to_string_alloc(IMOTD_FILE, &mut n);
            game.db.imotd = n.clone();
        }
        "help" => {
            game.file_to_string_alloc(HELP_PAGE_FILE, &mut n);
            game.db.help = n.clone();
        }
        "info" => {
            game.file_to_string_alloc(INFO_FILE, &mut n);
            game.db.info = n.clone();
        }
        "policy" => {
            game.file_to_string_alloc(POLICIES_FILE, &mut n);
            game.db.policies = n.clone();
        }
        "handbook" => {
            game.file_to_string_alloc(HANDBOOK_FILE, &mut n);
            game.db.handbook = n.clone();
        }
        "background" => {
            game.file_to_string_alloc(BACKGROUND_FILE, &mut n);
            game.db.background = n.clone();
        }
        "greetings" => {
            if game.file_to_string_alloc(GREETINGS_FILE, &mut n) == 0 {
                game.db.greetings = n.clone();
                prune_crlf(&mut game.db.greetings);
            }
        }
        "xhelp" => {
            game.db.help_table.clear();
            game.db.index_boot(DB_BOOT_HLP);
        }
        _ => {
            send_to_char(ch, "Unknown reload option.\r\n");
            return;
        }
    }

    send_to_char(ch, OK);
}

pub(crate) fn boot_world(game: &mut Game) {
    info!("Loading zone table.");
    game.db.index_boot(DB_BOOT_ZON);

    info!("Loading rooms.");
    game.db.index_boot(DB_BOOT_WLD);

    info!("Renumbering rooms.");
    game.db.renum_world();

    info!("Checking start rooms.");
    game.db.check_start_rooms();

    info!("Loading mobs and generating index.");
    game.db.index_boot(DB_BOOT_MOB);

    info!("Loading objs and generating index.");
    game.db.index_boot(DB_BOOT_OBJ);

    info!("Renumbering zone table.");
    renum_zone_table(game);

    if !game.db.no_specials {
        info!("Loading shops.");
        game.db.index_boot(DB_BOOT_SHP);
    }
}
impl DB {
    /* Free the world, in a memory allocation sense. */
    pub fn destroy_db(&mut self) {
        /* Active Mobiles & Players */
        for chtmp in self.character_list.iter() {
            free_char(chtmp);
        }
        self.character_list.clear();

        /* Active Objects */
        self.object_list.borrow_mut().clear();

        /* Rooms */
        for cnt in 0..self.world.borrow().len() {
            // self.world.borrow_mut()[cnt].ex_descriptions.clear();

            for itr in 0..NUM_OF_DIRS {
                if self.world.borrow()[cnt].dir_option[itr].is_none() {
                    continue;
                }
                // self.world.borrow_mut()[cnt].dir_option[itr] = None;
            }
        }
        self.world.borrow_mut().clear();

        /* Objects */
        self.obj_proto.clear();
        self.obj_index.clear();

        /* Mobiles */
        for cnt in 0..self.mob_protos.len() {
            while !self.mob_protos[cnt].affected.borrow().is_empty() {
                affect_remove(
                    &self.mob_protos[cnt],
                    &self.mob_protos[cnt].affected.borrow()[0],
                );
            }
        }
        self.mob_protos.clear();
        self.mob_index.clear();

        /* Shops */
        destroy_shops(self);

        /* Zones */
        self.zone_table.borrow_mut().clear();
    }

    pub fn new() -> DB {
        DB {
            world: RefCell::new(vec![]),
            character_list: vec![],
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
            no_mail: Cell::new(false),
            mini_mud: false,
            no_rent_check: false,
            boot_time: Cell::new(time_now()),
            no_specials: false,
            circle_restrict: Cell::new(0),
            r_mortal_start_room: Cell::new(NOWHERE),
            r_immort_start_room: Cell::new(NOWHERE),
            r_frozen_start_room: Cell::new(NOWHERE),
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
            time_info: Cell::from(TimeInfoData {
                hours: 0,
                day: 0,
                month: 0,
                year: 0,
            }),
            weather_info: Cell::new(WeatherData {
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
            shop_index: vec![],
            spell_sort_info: [0; MAX_SKILLS + 1],
            spell_info: [SpellInfoType::default(); TOP_SPELL_DEFINE + 1],
            soc_mess_list: vec![],
            ban_list: RefCell::new(vec![]),
            invalid_list: RefCell::new(vec![]),
            boards: RefCell::new(BoardSystem::new()),
            house_control: RefCell::new([HouseControlRec::new(); MAX_HOUSES]),
            num_of_houses: Cell::new(0),
            mails: RefCell::new(MailSystem::new()),
            mayor: RefCell::new(Mayor::new()),
            king_welmar: KingWelmar::new(),
            scheck: false,
        }
    }

    /* body of the booting system */
    pub fn boot_db(game: &mut Game) {
        info!("Boot db -- BEGIN.");

        info!("Resetting the game time:");
        game.db.reset_time();

        info!("Reading news, credits, help, bground, info & motds.");
        let mut buf = Rc::from("");
        game.file_to_string_alloc(NEWS_FILE, &mut buf);
        game.db.news = buf.clone();
        game.file_to_string_alloc(CREDITS_FILE, &mut buf);
        game.db.credits = buf.clone();
        game.file_to_string_alloc(MOTD_FILE, &mut buf);
        game.db.motd = buf.clone();
        game.file_to_string_alloc(IMOTD_FILE, &mut buf);
        game.db.imotd = buf.clone();
        game.file_to_string_alloc(HELP_PAGE_FILE, &mut buf);
        game.db.help = buf.clone();
        game.file_to_string_alloc(INFO_FILE, &mut buf);
        game.db.info = buf.clone();
        game.file_to_string_alloc(WIZLIST_FILE, &mut buf);
        game.db.wizlist = buf.clone();
        game.file_to_string_alloc(IMMLIST_FILE, &mut buf);
        game.db.immlist = buf.clone();
        game.file_to_string_alloc(POLICIES_FILE, &mut buf);
        game.db.policies = buf.clone();
        game.file_to_string_alloc(HANDBOOK_FILE, &mut buf);
        game.db.handbook = buf.clone();
        game.file_to_string_alloc(BACKGROUND_FILE, &mut buf);
        game.db.background = buf.clone();
        game.file_to_string_alloc(GREETINGS_FILE, &mut buf);
        game.db.greetings = buf.clone();
        prune_crlf(&mut game.db.greetings);

        info!("Loading spell definitions.");
        mag_assign_spells(&mut game.db);

        boot_world(game);

        info!("Loading help entries.");
        game.db.index_boot(DB_BOOT_HLP);

        info!("Generating player index.");
        game.db.build_player_index();

        info!("Loading fight messages.");
        game.db.load_messages();

        info!("Loading social messages.");
        boot_social_messages(&mut game.db);

        info!("Assigning function pointers:");

        if !game.db.no_specials {
            info!("   Mobiles.");
            assign_mobiles(&mut game.db);
            info!("   Shopkeepers.");
            assign_the_shopkeepers(&mut game.db);
            info!("   Objects.");
            assign_objects(&mut game.db);
            info!("   Rooms.");
            assign_rooms(&mut game.db);
        }

        info!("Assigning spell and skill levels.");
        init_spell_levels(&mut game.db);
        //
        info!("Sorting command list and spells.");
        sort_commands(&mut game.db);
        sort_spells(&mut game.db);

        info!("Booting mail system.");
        if !game.db.mails.borrow_mut().scan_file() {
            info!("    Mail boot failed -- Mail system disabled");
            game.db.no_mail.set(true);
        }
        info!("Reading banned site and invalid-name list.");
        load_banned(&mut game.db);
        read_invalid_list(&mut game.db);

        if !game.db.no_rent_check {
            info!("Deleting timed-out crash and rent files:");
            update_obj_file(&game.db);
            info!("   Done.");
        }

        // /* Moved here so the object limit code works. -gg 6/24/98 */
        if !game.db.mini_mud {
            info!("Booting houses.");
            house_boot(&mut game.db);
        }

        let zone_count = game.db.zone_table.borrow().len();
        for i in 0..zone_count {
            info!(
                "Resetting #{}: {} (rooms {}-{}).",
                game.db.zone_table.borrow()[i].number,
                game.db.zone_table.borrow()[i].name,
                game.db.zone_table.borrow()[i].bot,
                game.db.zone_table.borrow()[i].top
            );
            game.reset_zone(i);
        }

        // TODO reset_q.head = reset_q.tail = NULL;

        // TODO boot_time = time(0);

        info!("Boot db -- DONE.");
    }

    /* reset the time in the game from file */
    fn reset_time(&self) {
        let mut beginning_of_time = 0;

        match OpenOptions::new().read(true).open(TIME_FILE) {
            Err(err) => {
                info!("SYSERR: Can't open '{}': {}", TIME_FILE, err);
            }
            Ok(bgtime) => {
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
        }

        if beginning_of_time == 0 {
            beginning_of_time = 650336715;
        }

        let time_info = mud_time_passed(time_now(), beginning_of_time as u64);
        self.time_info.set(time_info);

        let mut weather_info = self.weather_info.get();

        weather_info.sunlight = match time_info.hours {
            0..=4 => SUN_DARK,
            5 => SUN_RISE,
            6..=20 => SUN_LIGHT,
            21 => SUN_SET,
            _ => SUN_DARK,
        };

        info!(
            "   Current Gametime: {}H {}D {}M {}Y.",
            time_info.hours, time_info.day, time_info.month, time_info.year
        );

        weather_info.pressure = 960;
        if (time_info.month >= 7) && (time_info.month <= 12) {
            weather_info.pressure += dice(1, 50);
        } else {
            weather_info.pressure += dice(1, 80);
        }

        weather_info.change = 0;

        weather_info.sky = match weather_info.pressure {
            ..=980 => SKY_LIGHTNING,
            ..=1000 => SKY_RAINING,
            ..=1020 => SKY_CLOUDY,
            _ => SKY_CLOUDLESS,
        };

        self.weather_info.set(weather_info);
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

impl DB {
    pub fn free_player_index(&mut self) {
        self.player_table.borrow_mut().clear();
    }
}

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
        player_file = OpenOptions::new()
            .write(true)
            .read(true)
            .open(PLAYER_FILE)
            .unwrap_or_else(|err| {
                if err.kind() != ErrorKind::NotFound {
                    error!("SYSERR: fatal error opening playerfile: {}", err);
                    process::exit(1);
                } else {
                    info!("No playerfile.  Creating a new one.");
                    touch(Path::new(PLAYER_FILE)).expect("SYSERR: fatal error creating playerfile");
                    OpenOptions::new()
                        .write(true)
                        .read(true)
                        .open(PLAYER_FILE)
                        .expect("SYSERR: fatal error opening playerfile after creation")
                }
            });

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
            self.player_table.borrow_mut().clear();
            return;
        }

        loop {
            let mut dummy = CharFileU::new();

            unsafe {
                let config_slice = slice::from_raw_parts_mut(
                    &mut dummy as *mut _ as *mut u8,
                    mem::size_of::<CharFileU>(),
                );
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
    }

    return total_keywords;
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

        prefix = match mode {
            DB_BOOT_WLD => WLD_PREFIX,
            DB_BOOT_MOB => MOB_PREFIX,
            DB_BOOT_OBJ => OBJ_PREFIX,
            DB_BOOT_ZON => ZON_PREFIX,
            DB_BOOT_SHP => SHP_PREFIX,
            DB_BOOT_HLP => HLP_PREFIX,
            _ => {
                error!("SYSERR: Unknown subcommand {} to index_boot!", mode);
                process::exit(1);
            }
        };

        if self.mini_mud {
            index_filename = MINDEX_FILE;
        } else {
            index_filename = INDEX_FILE;
        }

        let mut buf2 = format!("{}{}", prefix, index_filename);
        let db_index = File::open(buf2.as_str()).unwrap_or_else(|err| {
            error!("SYSERR: opening index file '{}': {}", buf2.as_str(), err);
            process::exit(1);
        });

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
                    boot_the_shops(self, db_file.unwrap(), &buf2, rec_count);
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
            light: Cell::new(0), /* Zero light sources */
            func: RefCell::new(None),
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
                    rd.ex_descriptions.push(ExtraDescrData {
                        keyword: fread_string(reader, buf2.as_str()),
                        description: fread_string(reader, buf2.as_str()),
                    });
                }
                'S' => {
                    /* end of room */
                    break;
                }
                _ => {
                    error!("{}", buf);
                    process::exit(1);
                }
            }
        }
        self.world.borrow_mut().push(Rc::new(rd));
    }

    /* read direction data */
    fn setup_dir(&self, reader: &mut BufReader<File>, room: &mut RoomData, dir: i32) {
        let mut t = [0; 5];
        let mut line = String::new();

        let buf2 = format!("room #{}, direction D{}", room.number, dir);

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
        room.dir_option[dir as usize] = Some(Rc::new(rdr));
    }

    // /* make sure the start rooms exist & resolve their vnums to rnums */
    fn check_start_rooms(&self) {
        self.r_mortal_start_room
            .set(self.real_room(MORTAL_START_ROOM));
        if self.r_mortal_start_room.get() == NOWHERE {
            error!("SYSERR:  Mortal start room does not exist.  Change in config.c.");
            process::exit(1);
        }
        self.r_immort_start_room
            .set(self.real_room(IMMORT_START_ROOM));
        if self.r_immort_start_room.get() == NOWHERE {
            if !self.mini_mud {
                error!("SYSERR:  Warning: Immort start room does not exist.  Change in config.c.");
                self.r_immort_start_room.set(self.r_mortal_start_room.get());
            }
        }
        self.r_frozen_start_room
            .set(self.real_room(FROZEN_START_ROOM));
        if self.r_frozen_start_room.get() == NOWHERE {
            if !self.mini_mud {
                error!("SYSERR:  Warning: Frozen start room does not exist.  Change in config.c.");
                self.r_frozen_start_room.set(self.r_mortal_start_room.get());
            }
        }
    }

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
}
fn renum_zone_table(game: &mut Game) {
    let mut olda;
    let mut oldb;
    let mut oldc;
    let db = &game.db;

    for zone in db.zone_table.borrow_mut().iter_mut() {
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
                    zcmd.arg1 = db.real_mobile(zcmd.arg1 as MobVnum) as i32;
                    a = zcmd.arg1;
                    zcmd.arg3 = db.real_room(zcmd.arg3 as RoomRnum) as i32;
                    c = zcmd.arg3;
                }
                'O' => {
                    zcmd.arg1 = db.real_object(zcmd.arg1 as ObjVnum) as i32;
                    a = zcmd.arg1;
                    if zcmd.arg3 != NOWHERE as i32 {
                        zcmd.arg3 = db.real_room(zcmd.arg3 as RoomRnum) as i32;
                        c = zcmd.arg3;
                    }
                }
                'G' => {
                    zcmd.arg1 = db.real_object(zcmd.arg1 as ObjVnum) as i32;
                    a = zcmd.arg1;
                }
                'E' => {
                    zcmd.arg1 = db.real_object(zcmd.arg1 as ObjVnum) as i32;
                    a = zcmd.arg1;
                }
                'P' => {
                    zcmd.arg1 = db.real_object(zcmd.arg1 as ObjVnum) as i32;
                    a = zcmd.arg1;
                    zcmd.arg3 = db.real_object(zcmd.arg3 as ObjVnum) as i32;
                    c = zcmd.arg3;
                }
                'D' => {
                    zcmd.arg1 = db.real_room(zcmd.arg1 as RoomRnum) as i32;
                    a = zcmd.arg1;
                }
                'R' => {
                    /* rem obj from room */
                    zcmd.arg1 = db.real_room(zcmd.arg1 as RoomRnum) as i32;
                    a = zcmd.arg1;
                    zcmd.arg2 = db.real_object(zcmd.arg2 as RoomRnum) as i32;
                    b = zcmd.arg2;
                }
                _ => {}
            }

            if a == NOWHERE as i32 || b == NOWHERE as i32 || c == NOWHERE as i32 {
                if !db.mini_mud {
                    let buf = format!(
                        "Invalid vnum {}, cmd disabled",
                        if a == NOWHERE as i32 {
                            olda
                        } else if b == NOWHERE as i32 {
                            oldb
                        } else {
                            oldc
                        }
                    );
                    let mut cmd_no = cmd_no as i32;
                    game.log_zone_error(zone.number as usize, zcmd, &buf, &mut cmd_no);
                }
                zcmd.command.set('*');
            }
        }
    }
}
impl DB {
    fn parse_simple_mob(&mut self, reader: &mut BufReader<File>, mobch: &mut CharData, nr: i32) {
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
     */
    fn interpret_espec(&mut self, keyword: &str, value: &str, mobch: &mut CharData, nr: i32) {
        let mut num_arg = 0;

        /*
         * If there isn't a colon, there is no value.  While Boolean options are
         * possible, we don't actually have any.  Feel free to make some.
         */
        if !value.is_empty() {
            let r = value.parse::<i32>();
            num_arg = if r.is_ok() { r.unwrap() } else { 0 };
        }

        match keyword {
            "BareHandAttack" => {
                num_arg = max(0, min(99, num_arg));
                mobch.mob_specials.attack_type = num_arg as u8;
            }

            "Str" => {
                num_arg = max(3, min(25, num_arg));
                mobch.real_abils.borrow_mut().str = num_arg as i8;
            }

            "StrAdd" => {
                num_arg = max(0, min(100, num_arg));
                mobch.real_abils.borrow_mut().str_add = num_arg as i8;
            }

            "Int" => {
                num_arg = max(3, min(25, num_arg));
                mobch.real_abils.borrow_mut().intel = num_arg as i8;
            }

            "Wis" => {
                num_arg = max(3, min(25, num_arg));
                mobch.real_abils.borrow_mut().wis = num_arg as i8;
            }

            "Dex" => {
                num_arg = max(3, min(25, num_arg));
                mobch.real_abils.borrow_mut().dex = num_arg as i8;
            }

            "Con" => {
                num_arg = max(3, min(25, num_arg));
                mobch.real_abils.borrow_mut().con = num_arg as i8;
            }

            "Cha" => {
                num_arg = max(3, min(25, num_arg));
                mobch.real_abils.borrow_mut().cha = num_arg as i8;
            }

            _ => {
                error!(
                    "SYSERR: Warning: unrecognized espec keyword {} in mob #{}",
                    keyword, nr
                );
            }
        }
    }

    fn parse_espec(&mut self, buf: &str, mobch: &mut CharData, nr: i32) {
        let mut buf = buf;
        let mut ptr = "";
        let p = buf.find(':');
        if p.is_some() {
            let p = p.unwrap();
            ptr = &buf[p + 1..];
            buf = &buf[0..p];
            ptr = ptr.trim_start();
        }

        self.interpret_espec(buf, ptr, mobch, nr);
    }

    fn parse_enhanced_mob(&mut self, reader: &mut BufReader<File>, mobch: &mut CharData, nr: i32) {
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
                self.parse_espec(&line, mobch, nr);
            }
        }

        error!("SYSERR: Unexpected end of file reached after mob #{}", nr);
        process::exit(1);
    }

    fn parse_mobile(&mut self, reader: &mut BufReader<File>, nr: i32) {
        let mut line = String::new();

        self.mob_index.push(IndexData {
            vnum: nr as MobVnum,
            number: 0,
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
        let mut line = String::new();

        let i = self.obj_index.len() as ObjVnum;
        self.obj_index.push(IndexData {
            vnum: nr,
            number: 0,
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
        if obj.get_obj_type() == ITEM_DRINKCON || obj.get_obj_type() == ITEM_FOUNTAIN {
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
                    let obj = Rc::from(obj);
                    self.check_object(&obj);
                    self.obj_proto.push(obj);
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

        let zname = zonename;

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
    }
}

fn get_one_line(reader: &mut BufReader<File>, buf: &mut String) {
    let r = reader.read_line(buf);
    if r.is_err() {
        error!("SYSERR: error reading help file: not terminated with $?");
        process::exit(1);
    }

    *buf = buf.trim_end().to_string();
}

impl DB {
    pub fn free_help(&mut self) {
        self.help_table.clear();
    }

    pub fn load_help(&mut self, fl: File) {
        let mut entry = String::new();
        let mut key = String::new();
        let mut reader = BufReader::new(fl);
        /* get the first keyword line */
        get_one_line(&mut reader, &mut key);
        while !key.starts_with('$') {
            key.push_str("\r\n");
            entry.push_str(&key);

            /* read in the corresponding help entry */
            let mut line = String::new();
            get_one_line(&mut reader, &mut line);
            while !line.starts_with('#') {
                line.push_str("\r\n");
                entry.push_str(&line);
                line.clear();
                get_one_line(&mut reader, &mut line);
            }

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
                let new_el = HelpIndexElement {
                    keyword: el.keyword.clone(),
                    entry: el.entry.clone(),
                    duplicate: el.duplicate,
                };
                self.help_table.push(new_el);
                scan = one_word(&scan, &mut next_key);
            }

            /* get next keyword line (or $) */
            key.clear();
            entry.clear();
            get_one_line(&mut reader, &mut key);
        }
    }

    /*************************************************************************
    *  procedures for resetting, both play-time and boot-time	 	 *
    *************************************************************************/

    pub fn vnum_mobile(&self, searchname: &str, ch: &Rc<CharData>) -> i32 {
        let mut found = 0;
        for nr in 0..self.mob_protos.len() {
            let mp = &self.mob_protos[nr];
            if isname(searchname, &mp.player.borrow().name) {
                found += 1;
                send_to_char(
                    ch,
                    format!(
                        "{:3}. [{:5}] {}\r\n",
                        found,
                        self.mob_index[nr].vnum,
                        mp.player.borrow().short_descr
                    )
                    .as_str(),
                );
            }
        }
        return found;
    }

    pub fn vnum_object(&self, searchname: &str, ch: &Rc<CharData>) -> i32 {
        let mut found = 0;
        for nr in 0..self.obj_proto.len() {
            let op = &self.obj_proto[nr];
            if isname(searchname, &op.name.borrow()) {
                found += 1;
                send_to_char(
                    ch,
                    format!(
                        "{:3}. [{:5}] {}\r\n",
                        found, self.obj_index[nr].vnum, op.short_description
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

    /* create a new mobile from a prototype */
    pub(crate) fn read_mobile(&mut self, nr: MobVnum, _type: i32) -> Option<Rc<CharData>> /* and mob_rnum */
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

        let mob = self.mob_protos[i as usize].make_copy();

        if mob.points.borrow().max_hit == 0 {
            let max_hit = dice(
                mob.points.borrow().hit as i32,
                mob.points.borrow().mana as i32,
            ) + mob.points.borrow().movem as i32;
            mob.points.borrow_mut().max_hit = max_hit as i16;
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

        self.mob_index[i as usize].number+= 1;

        let rc = Rc::from(mob);
        self.character_list.push(rc.clone());

        Some(rc)
    }

    /* create an object, and add it to the object list */
    pub fn create_obj(
        &self,
        num: ObjVnum,
        name: &str,
        short_description: &str,
        description: &str,
        obj_type: u8,
        obj_wear: i32,
        weight: i32,
        cost: i32,
        rent: i32,
    ) -> Rc<ObjData> {
        let mut obj = ObjData::new();

        clear_object(&mut obj);
        obj.item_number = num;
        obj.name = RefCell::from(name.to_string());
        obj.description = description.to_string();
        obj.short_description = short_description.to_string();
        obj.set_obj_type(obj_type);
        obj.set_obj_wear(obj_wear);
        obj.set_obj_weight(weight);
        obj.set_obj_cost(cost);
        obj.set_obj_rent(rent);
        let ret = Rc::from(obj);
        self.object_list.borrow_mut().push(ret.clone());

        ret
    }

    /* create a new object from a prototype */
    pub fn read_object(&mut self, nr: ObjVnum, _type: i32) -> Option<Rc<ObjData>> /* and obj_rnum */ {
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

        self.obj_index[i as usize].number+= 1;

        Some(rc)
    }
}

const ZO_DEAD: i32 = 999;

impl Game {
    /* update zone ages, queue for reset if necessary, and dequeue when possible */
    pub(crate) fn zone_update(&mut self) {
        /* jelson 10/22/92 */
        self.db.timer.set(self.db.timer.get());
        if (self.db.timer.get() * PULSE_ZONE / PASSES_PER_SEC) >= 60 {
            /* one minute has passed */
            /*
             * NOT accurate unless PULSE_ZONE is a multiple of PASSES_PER_SEC or a
             * factor of 60
             */

            self.db.timer.set(0);

            /* since one minute has passed, increment zone ages */
            for (i, zone) in self.db.zone_table.borrow().iter().enumerate() {
                if zone.age.get() < zone.lifespan && zone.reset_mode != 0 {
                    zone.age.set(zone.age.get() + 1);
                }

                if zone.age.get() >= zone.lifespan
                    && zone.age.get() < ZO_DEAD
                    && zone.reset_mode != 0
                {
                    /* enqueue zone */
                    self.db.reset_q.borrow_mut().push(i as RoomRnum);

                    zone.age.set(ZO_DEAD);
                }
            }
        } /* end - one minute has passed */

        /* dequeue zones (if possible) and reset */
        /* this code is executed every 10 seconds (i.e. PULSE_ZONE) */
        let update_list = self.db.reset_q.borrow().clone();
        for update_u in update_list {
            if self.db.zone_table.borrow()[update_u as usize].reset_mode == 2
                || is_empty(self, update_u)
            {
                self.reset_zone(update_u as usize);
                self.mudlog(
                    CMP,
                    LVL_GOD as i32,
                    false,
                    format!(
                        "Auto zone reset: {}",
                        self.db.zone_table.borrow()[update_u as usize].name
                    )
                    .as_str(),
                );
            }
        }
        self.db.reset_q.borrow_mut().clear();
    }

    /* execute the reset command table of a given zone */
    fn log_zone_error(&self, zone: usize, zcmd: &ResetCom, message: &str, last_cmd: &mut i32) {
        self.mudlog(
            NRM,
            LVL_GOD as i32,
            true,
            format!("SYSERR: zone file: {}", message).as_str(),
        );
        self.mudlog(
            NRM,
            LVL_GOD as i32,
            true,
            format!(
                "SYSERR: ...offending cmd: '{}' cmd in zone #{}, line {}",
                zcmd.command.get(),
                zone,
                zcmd.line
            )
            .as_str(),
        );
        *last_cmd = 0;
    }

    pub(crate) fn reset_zone(&mut self, zone: usize) {
        let mut last_cmd = 0;
        let mut obj;
        let mut mob = None;
        let cmd_count = self.db.zone_table.borrow()[zone].cmd.len();
        for cmd_no in 0..cmd_count {
           // let zcmd = &self.db.zone_table.borrow()[zone].cmd[cmd_no];
            if self.db.zone_table.borrow()[zone].cmd[cmd_no].command.get() == 'S' {
                break;
            }
            if self.db.zone_table.borrow()[zone].cmd[cmd_no].if_flag && last_cmd == 0 {
                continue;
            }

            /*  This is the list of actual zone commands.  If any new
             *  zone commands are added to the game, be certain to update
             *  the list of commands in load_zone() so that the counting
             *  will still be correct. - ae.
             */
            let command = self.db.zone_table.borrow()[zone].cmd[cmd_no].command.get();
            match  command {
                '*' => {
                    /* ignore command */
                    last_cmd = 0;
                }

                'M' => {
                    /* read a mobile */
                    if self.db.mob_index[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].number < self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 {
                        let nr = self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as MobVnum;
                        mob = self.db.read_mobile(nr, REAL);
                        self.db
                            .char_to_room(mob.as_ref().unwrap(), self.db.zone_table.borrow()[zone].cmd[cmd_no].arg3 as RoomRnum);
                        last_cmd = 1;
                    } else {
                        last_cmd = 0;
                    }
                }

                'O' => {
                    /* read an object */
                    if self.db.obj_index[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].number < self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 {
                        if self.db.zone_table.borrow()[zone].cmd[cmd_no].arg3 != NOWHERE as i32 {
                            let nr = self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as ObjVnum;
                            obj = self.db.read_object(nr, REAL);
                            self.db
                                .obj_to_room(obj.as_ref().unwrap(), self.db.zone_table.borrow()[zone].cmd[cmd_no].arg3 as RoomRnum);
                            last_cmd = 1;
                        } else {
                            let nr = self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as ObjVnum;
                            obj = self.db.read_object(nr, REAL);
                            obj.as_ref().unwrap().in_room.set(NOWHERE);
                            last_cmd = 1;
                        }
                    } else {
                        last_cmd = 0;
                    }
                }

                'P' => {
                    /* object to object */
                    if self.db.obj_index[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].number < self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 {
                        let nr = self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as ObjVnum;
                        obj = self.db.read_object(nr, REAL);
                        let obj_to = self.db.get_obj_num(self.db.zone_table.borrow()[zone].cmd[cmd_no].arg3 as ObjRnum);
                        if obj_to.is_none() {
                            self.log_zone_error(
                                zone,
                                &self.db.zone_table.borrow()[zone].cmd[cmd_no],
                                "target obj not found, command disabled",
                                &mut last_cmd,
                            );
                            self.db.zone_table.borrow()[zone].cmd[cmd_no].command.set('*');
                            continue;
                        }
                        self.db
                            .obj_to_obj(obj.as_ref().unwrap(), obj_to.as_ref().unwrap());
                        last_cmd = 1;
                    } else {
                        last_cmd = 0;
                    }
                }

                'G' => {
                    /* obj_to_char */
                    if mob.is_none() {
                        self.log_zone_error(
                            zone,
                            &self.db.zone_table.borrow()[zone].cmd[cmd_no],
                            "attempt to give obj to non-existant mob, command disabled",
                            &mut last_cmd,
                        );

                        self.db.zone_table.borrow()[zone].cmd[cmd_no].command.set('*');
                        continue;
                    }
                    if self.db.obj_index[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].number < self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 {
                        let nr = self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as ObjVnum;
                        obj = self.db.read_object(nr, REAL);
                        DB::obj_to_char(obj.as_ref().unwrap(), mob.as_ref().unwrap());
                        last_cmd = 1;
                    } else {
                        last_cmd = 0;
                    }
                }

                'E' => {
                    /* object to equipment list */
                    if mob.is_none() {
                        self.log_zone_error(
                            zone,
                            &self.db.zone_table.borrow()[zone].cmd[cmd_no],
                            "trying to equip non-existant mob, command disabled",
                            &mut last_cmd,
                        );

                        self.db.zone_table.borrow()[zone].cmd[cmd_no].command.set('*');
                        continue;
                    }
                    if self.db.obj_index[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].number < self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 {
                        if self.db.zone_table.borrow()[zone].cmd[cmd_no].arg3 < 0 || self.db.zone_table.borrow()[zone].cmd[cmd_no].arg3 >= NUM_WEARS as i32 {
                            self.log_zone_error(
                                zone,
                                &self.db.zone_table.borrow()[zone].cmd[cmd_no],
                                "invalid equipment pos number",
                                &mut last_cmd,
                            );
                        } else {
                            let nr = self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as ObjVnum;
                            obj = self.db.read_object(nr, REAL);
                            self.db.equip_char(
                                mob.as_ref().unwrap(),
                                obj.as_ref().unwrap(),
                                self.db.zone_table.borrow()[zone].cmd[cmd_no].arg3 as i8,
                            );
                            last_cmd = 1;
                        }
                    } else {
                        last_cmd = 0;
                    }
                }

                'R' => {
                    /* rem obj from room */
                    obj = self.db.get_obj_in_list_num(
                        self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 as i16,
                        self.db.world.borrow()[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize]
                            .contents
                            .borrow()
                            .as_ref(),
                    );
                    if obj.is_some() {
                        self.db.extract_obj(obj.as_ref().unwrap());
                    }
                    last_cmd = 1;
                }

                'D' => {
                    /* set state of door */
                    if self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 < 0
                        || self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 >= NUM_OF_DIRS as i32
                        || (self.db.world.borrow()[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].dir_option
                            [self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 as usize]
                            .is_none())
                    {
                        self.log_zone_error(
                            zone,
                            &self.db.zone_table.borrow()[zone].cmd[cmd_no],
                            "door does not exist, command disabled",
                            &mut last_cmd,
                        );
                        self.db.zone_table.borrow()[zone].cmd[cmd_no].command.set('*');
                    } else {
                        match self.db.zone_table.borrow()[zone].cmd[cmd_no].arg3 {
                            0 => {
                                self.db.world.borrow()[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].dir_option
                                    [self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .remove_exit_info_bit(EX_LOCKED as i32);
                                self.db.world.borrow()[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].dir_option
                                    [self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .remove_exit_info_bit(EX_CLOSED as i32);
                            }

                            1 => {
                                self.db.world.borrow()[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].dir_option
                                    [self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .set_exit_info_bit(EX_LOCKED as i32);
                                self.db.world.borrow()[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].dir_option
                                    [self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .remove_exit_info_bit(EX_CLOSED as i32);
                            }

                            2 => {
                                self.db.world.borrow()[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].dir_option
                                    [self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 as usize]
                                    .as_ref()
                                    .unwrap()
                                    .set_exit_info_bit(EX_LOCKED as i32);
                                self.db.world.borrow()[self.db.zone_table.borrow()[zone].cmd[cmd_no].arg1 as usize].dir_option
                                    [self.db.zone_table.borrow()[zone].cmd[cmd_no].arg2 as usize]
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
                        zone,
                        &self.db.zone_table.borrow()[zone].cmd[cmd_no],
                        "unknown cmd in reset table; cmd disabled",
                        &mut last_cmd,
                    );
                    self.db.zone_table.borrow()[zone].cmd[cmd_no].command.set('*');
                }
            }
        }

        self.db.zone_table.borrow()[zone].age.set(0);
    }
}

/* for use in reset_zone; return TRUE if zone 'nr' is free of PC's  */
fn is_empty(game: &Game, zone_nr: ZoneRnum) -> bool {
    for i in game.descriptor_list.iter() {
        if i.state() != ConPlaying {
            continue;
        }
        if i.character.borrow().as_ref().unwrap().in_room() == NOWHERE {
            continue;
        }
        if i.character.borrow().as_ref().unwrap().get_level() >= LVL_IMMORT as u8 {
            continue;
        }
        if game.db.world.borrow()[i.character.borrow().as_ref().unwrap().in_room() as usize].zone
            != zone_nr
        {
            continue;
        }
        return false;
    }
    true
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
    pub fn save_char(&self, ch: &Rc<CharData>) {
        let mut st: CharFileU = CharFileU::new();

        if ch.is_npc() || ch.desc.borrow().is_none() || ch.get_pfilepos() < 0 {
            return;
        }

        self.char_to_store(ch, &mut st);

        copy_to_stored(
            &mut st.host,
            ch.desc.borrow().as_ref().unwrap().host.as_ref(),
        );

        unsafe {
            let player_slice =
                slice::from_raw_parts(&mut st as *mut _ as *mut u8, mem::size_of::<CharFileU>());
            self.player_fl
                .borrow_mut()
                .as_mut()
                .unwrap()
                .write_all_at(
                    player_slice,
                    (ch.get_pfilepos() as usize * mem::size_of::<CharFileU>()) as u64,
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
    ch.set_sex(st.sex);
    ch.set_class(st.chclass);
    ch.set_level(st.level);

    ch.player.borrow_mut().short_descr = String::new();
    ch.player.borrow_mut().long_descr = String::new();
    ch.player.borrow_mut().title = Some(parse_c_string(&st.title));
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
    ch.player_specials.borrow_mut().poofin = Rc::from("");
    ch.player_specials.borrow_mut().poofout = Rc::from("");
    ch.set_last_tell(NOBODY as i64);

    if ch.points.borrow().max_mana < 100 {
        ch.points.borrow_mut().max_mana = 100;
    }

    ch.char_specials.borrow_mut().carry_weight = 0;
    ch.char_specials.borrow_mut().carry_items = 0;
    ch.points.borrow_mut().armor = 100;
    ch.points.borrow_mut().hitroll = 0;
    ch.points.borrow_mut().damroll = 0;

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
impl DB {
    pub fn char_to_store(&self, ch: &Rc<CharData>, st: &mut CharFileU) {
        /* Unaffect everything a character can be affected by */
        let mut char_eq: [Option<Rc<ObjData>>; NUM_WEARS as usize] =
            [(); NUM_WEARS as usize].map(|_| None);

        for i in 0..NUM_WEARS {
            if ch.get_eq(i).is_some() {
                char_eq[i as usize] = self.unequip_char(ch, i);
            } else {
                char_eq[i as usize] = None;
            }
        }
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

        while !ch.affected.borrow().is_empty() {
            affect_remove(ch, &ch.affected.borrow()[0]);
        }

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

        if ch.has_title() && !ch.get_title().is_empty() {
            copy_to_stored(&mut st.title, &ch.get_title());
        } else {
            st.title[0] = 0;
        }
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

        for i in 0..NUM_WEARS {
            if char_eq[i as usize].is_some() {
                self.equip_char(ch, char_eq[i as usize].as_ref().unwrap(), i);
            }
        }
        /*   affect_total(ch); unnecessary, I think !?! */
    } /* Char to store */
}

pub fn copy_to_stored(to: &mut [u8], from: &str) -> usize {
    let bytes = from.as_bytes();
    let bytes_copied = min(to.len(), from.len());
    to[0..bytes_copied].copy_from_slice(&bytes[0..bytes_copied]);
    if bytes_copied != to.len() {
        to[bytes_copied] = 0;
    }
    bytes_copied
}

/*
 * Create a new entry in the in-memory index table for the player file.
 * If the name already exists, by overwriting a deleted character, then
 * we re-use the old position.
 */
impl DB {
    pub(crate) fn create_entry(&self, name: &str) -> usize {
        let i: usize;
        let pos = self.get_ptable_by_name(name);

        return if pos.is_none() {
            /* new name */
            i = self.player_table.borrow().len();
            self.player_table.borrow_mut().push(PlayerIndexElement {
                name: name.to_lowercase(),
                id: i as i64,
            });
            i
        } else {
            let pos = pos.unwrap();

            let mut pt = self.player_table.borrow_mut();
            let mut pie = pt.get_mut(pos);
            pie.as_mut().unwrap().name = name.to_lowercase();
            pos
        };
    }
}

/************************************************************************
 *  funcs of a (more or less) general utility nature			*
 ************************************************************************/

/* read and allocate space for a '~'-terminated string from a given file */
pub fn fread_string(reader: &mut BufReader<File>, error: &str) -> String {
    let mut buf = String::new();
    let mut tmp = String::new();
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
        if done {
            break;
        }
    }

    return buf;
}

/* release memory allocated for a char struct */
pub fn free_char(ch: &Rc<CharData>) {
    ch.player_specials.borrow_mut().aliases.clear();

    while !ch.affected.borrow().is_empty() {
        affect_remove(ch, &ch.affected.borrow()[0]);
    }

    if ch.desc.borrow().is_some() {
        *ch.desc.borrow_mut() = None;
    }
}

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
        for in_use in &self.descriptor_list {
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

        for in_use in &*self.descriptor_list {
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

/* clear some of the the working variables of a char */
pub fn reset_char(ch: &CharData) {
    for i in 0..NUM_WEARS {
        ch.set_eq(i, None);
    }

    ch.followers.borrow_mut().clear();
    *ch.master.borrow_mut() = None;
    ch.set_in_room(NOWHERE);
    ch.carrying.borrow_mut().clear();
    ch.set_fighting(None);
    ch.char_specials.borrow_mut().position = POS_STANDING;
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

        ch.set_title(None);
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
            let top_n = self.player_table.borrow().len();
            self.player_table.borrow_mut()[i].id = top_n as i64; //*self.top_idnum.borrow() as i64;
            ch.set_idnum(top_n as i64); /*self.top_idnum.borrow()*/
        }

        for i in 1..MAX_SKILLS {
            if ch.get_level() < LVL_IMPL as u8 {
                RefCell::borrow_mut(&ch.player_specials).saved.skills[i] = 0;
            } else {
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
            return NOTHING;
        }
        r.unwrap() as ObjRnum
    }
}

/* returns the real number of the zone with given virtual number */
pub fn real_zone(db: &DB, vnum: RoomVnum) -> Option<usize> {
    db.zone_table.borrow().iter().position(|z| z.number == vnum)
}

/*
 * Extend later to include more checks.
 *
 * TODO: Add checks for unknown bitvectors.
 */
impl DB {
    fn check_object(&self, obj: &Rc<ObjData>) -> bool {
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

        match obj.get_obj_type() {
            ITEM_DRINKCON => {
                let space_pos = obj.name.borrow().rfind(' ');
                let onealias = if space_pos.is_some() {
                    (&obj.name.borrow().as_str()[space_pos.unwrap() + 1..]).to_string()
                } else {
                    obj.name.borrow().as_str().to_string()
                };
                if search_block(&onealias, &DRINKNAMES, true).is_none() {
                    error = true;
                    error!(
                        "SYSERR: Object #{} ({}) doesn't have drink type as last alias. ({})",
                        self.get_obj_vnum(obj),
                        obj.short_description,
                        obj.name.borrow()
                    );
                }
                if obj.get_obj_val(1) > obj.get_obj_val(0) {
                    error = true;
                    error!(
                        "SYSERR: Object #{} ({}) contains ({}) more than maximum ({}).",
                        self.get_obj_vnum(obj),
                        obj.short_description,
                        obj.get_obj_val(1),
                        obj.get_obj_val(0)
                    );
                }
            }
            ITEM_FOUNTAIN => {
                if obj.get_obj_val(1) > obj.get_obj_val(0) {
                    error = true;
                    error!(
                        "SYSERR: Object #{} ({}) contains ({}) more than maximum ({}).",
                        self.get_obj_vnum(obj),
                        obj.short_description,
                        obj.get_obj_val(1),
                        obj.get_obj_val(0)
                    );
                }
            }
            ITEM_SCROLL | ITEM_POTION => {
                error |= self.check_object_level(obj, 0);
                error |= self.check_object_spell_number(obj, 1);
                error |= self.check_object_spell_number(obj, 2);
                error |= self.check_object_spell_number(obj, 3);
            }

            ITEM_WAND | ITEM_STAFF => {
                error |= self.check_object_level(obj, 0);
                error |= self.check_object_spell_number(obj, 3);
                if obj.get_obj_val(2) > obj.get_obj_val(1) {
                    error = true;
                    error!(
                        "SYSERR: Object #{} ({}) has more charges ({}) than maximum ({}).",
                        self.get_obj_vnum(obj),
                        obj.short_description,
                        obj.get_obj_val(2),
                        obj.get_obj_val(1)
                    );
                }
            }
            _ => {}
        }

        return error;
    }

    fn check_object_spell_number(&self, obj: &Rc<ObjData>, val: usize) -> bool {
        let mut error = false;

        if obj.get_obj_val(val) == -1 {
            /* i.e.: no spell */
            return error;
        }

        /*
         * Check for negative spells, spells beyond the top define, and any
         * spell which is actually a skill.
         */
        if obj.get_obj_val(val) < 0 {
            error = true;
        }
        if obj.get_obj_val(val) > TOP_SPELL_DEFINE as i32 {
            error = true;
        }
        if obj.get_obj_val(val) > MAX_SPELLS && obj.get_obj_val(val) <= MAX_SKILLS as i32 {
            error = true;
        }
        if error {
            error!(
                "SYSERR: Object #{} ({}) has out of range spell #{}.",
                self.get_obj_vnum(obj),
                obj.short_description,
                obj.get_obj_val(val)
            );
        }
        /*
         * This bug has been fixed, but if you don't like the special behavior...
         */
        // #if 0
        // if (GET_OBJ_TYPE(obj) == ITEM_STAFF &&
        // HAS_SPELL_ROUTINEobj.get_obj_val( val), MAG_AREAS | MAG_MASSES))
        // log("... '%s' (#%d) uses %s spell '%s'.",
        // obj->short_description,	GET_ObjVnum(obj),
        // HAS_SPELL_ROUTINEobj.get_obj_val( val), MAG_AREAS) ? "area" : "mass",
        // skill_nameobj.get_obj_val( val)));
        // #endif

        if self.scheck {
            /* Spell names don't exist in syntax check mode. */
            return error;
        }

        /* Now check for unnamed spells. */
        let spellname = skill_name(self, obj.get_obj_val(val));

        if spellname == UNUSED_SPELLNAME || "UNDEFINED" == spellname {
            error = true;
            error!(
                "SYSERR: Object #{} ({}) uses '{}' spell #{}.",
                self.get_obj_vnum(obj),
                obj.short_description,
                spellname,
                obj.get_obj_val(val)
            );
        }

        return error;
    }

    fn check_object_level(&self, obj: &Rc<ObjData>, val: usize) -> bool {
        let error = false;

        if obj.get_obj_val(val) < 0 || obj.get_obj_val(val) > LVL_IMPL as i32 && error {
            error!(
                "SYSERR: Object #{} ({}) has out of range level #{}.",
                self.get_obj_vnum(obj),
                obj.short_description,
                obj.get_obj_val(val)
            );
        }

        return error;
    }
}

fn check_bitvector_names(bits: i64, namecount: usize, whatami: &str, whatbits: &str) -> bool {
    let mut error = false;

    /* See if any bits are set above the ones we know about. */
    if bits <= (!0 >> (64 - namecount)) {
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
            pfilepos: Cell::new(0),
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
                aliases: vec![],
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
            // next_fighting: RefCell::new(None),
            followers: RefCell::new(vec![]),
            master: RefCell::new(None),
        }
    }
    fn make_copy(&self) -> CharData {
        CharData {
            pfilepos: Cell::new(self.get_pfilepos()),
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
                aliases: vec![],
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
