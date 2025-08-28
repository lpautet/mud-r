/* ************************************************************************
*   File: main.rs                                       Part of CircleMUD *
*  Usage: Communication, socket handling, main(), central game loop       *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */
use std::borrow::Borrow;
use std::cmp::max;
use std::collections::LinkedList;
use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;
use std::string::ToString;
use std::time::{Duration, Instant};
use std::{env, fs, process, thread};

use tungstenite::{WebSocket, accept, Message};
use clap::Parser;

use depot::{Depot, DepotId, HasId};
use log::{debug, error, info, warn, LevelFilter};

use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::Appender;
use log4rs::config::Root;
use log4rs::encode::pattern::PatternEncoder;
use util::{can_see, objn, objs, pers};

use crate::act_social::free_social_messages;
use crate::ban::{free_invalid_list, isbanned};
use crate::boards::board_clear_all;
use crate::config::*;
use crate::constants::*;
use crate::db::*;
use crate::fight::free_messages;
use crate::handler::fname;
use crate::house::house_save_all;
use crate::interpreter::{command_interpreter, is_abbrev, nanny, perform_alias};
use crate::magic::affect_update;
use crate::modify::{show_string, string_add};
use crate::objsave::crash_save_all;
use crate::structs::ConState::{ConClose, ConDisconnect, ConGetName, ConPassword, ConPlaying};
use crate::structs::*;
use crate::telnet::{IAC, TELOPT_ECHO, WILL, WONT};
use crate::util::{hmhr, hshr, hssh, sana, touch, CMP, NRM, SECS_PER_MUD_HOUR};

mod act_comm;
mod act_informative;
mod act_item;
mod act_movement;
mod act_offensive;
mod act_other;
mod act_social;
mod act_wizard;
mod alias;
mod ban;
mod boards;
mod castle;
mod class;
mod config;
mod constants;
mod db;
mod depot;
mod fight;
mod graph;
mod handler;
mod house;
mod interpreter;
mod limits;
mod magic;
mod mail;
mod mobact;
mod modify;
mod objsave;
mod screen;
mod shops;
mod spec_assign;
mod spec_procs;
mod spell_parser;
mod spells;
mod structs;
mod telnet;
mod util;
mod weather;

/// CircleMUD server - A classic text-based multiplayer online role-playing game
#[derive(Parser, Debug)]
#[command(name = "mudr")]
#[command(about = "CircleMUD server")]
#[command(long_about = None)]
struct Args {
    /// Enable syntax check mode
    #[arg(short = 'c', long = "check")]
    syntax_check: bool,

    /// Specify library directory (defaults to 'lib')
    #[arg(short = 'd', long = "dir", default_value = "lib")]
    directory: String,

    /// Start in mini-MUD mode
    #[arg(short = 'm', long = "mini")]
    mini_mud: bool,

    /// Write log to file instead of stderr
    #[arg(short = 'o', long = "log")]
    log_file: Option<String>,

    /// Quick boot (doesn't scan rent for object limits)
    #[arg(short = 'q', long = "quick")]
    quick_boot: bool,

    /// Restrict MUD -- no new players allowed
    #[arg(short = 'r', long = "restrict")]
    restrict: bool,

    /// Suppress special procedure assignments
    #[arg(short = 's', long = "no-specials")]
    no_specials: bool,

    /// Port number to listen on (must be > 1024)
    #[arg(value_name = "PORT")]
    port: Option<u16>,
}

pub const PAGE_LENGTH: i32 = 22;
pub const PAGE_WIDTH: i32 = 80;

pub const TO_ROOM: i32 = 1;
pub const TO_VICT: i32 = 2;
pub const TO_NOTVICT: i32 = 3;
pub const TO_CHAR: i32 = 4;
pub const TO_SLEEP: i32 = 128; /* to char, even if sleeping */

#[derive(Debug)]
pub enum ConnectionType {
    Telnet(TcpStream),
    WebSocket(WebSocket<TcpStream>),
}

pub struct TextData {
    id: DepotId,
    pub text: String,
}

impl Default for TextData {
    fn default() -> Self {
        Self {
            id: Default::default(),
            text: Default::default(),
        }
    }
}

impl Depot<TextData> {
    pub fn add_text(&mut self, text: String) -> DepotId {
        self.push(TextData {
            id: DepotId::default(),
            text,
        })
    }
}

impl HasId for TextData {
    fn id(&self) -> DepotId {
        self.id
    }

    fn set_id(&mut self, id: DepotId) {
        self.id = id;
    }
}

pub struct DescriptorData {
    id: DepotId,
    connection: Option<ConnectionType>,
    // connection type (telnet or websocket)
    host: Rc<str>,
    // hostname
    bad_pws: u8,
    /* number of bad pw attemps this login	*/
    idle_tics: u8,
    /* tics idle at password prompt		*/
    connected: ConState,
    // mode of 'connectedness'
    desc_num: usize,
    // unique num assigned to desc
    login_time: Instant,
    /* when the person connected		*/
    showstr_head: Option<Rc<str>>,
    /* for keeping track of an internal str	*/
    showstr_vector: Vec<Rc<str>>,
    /* for paging through texts		*/
    showstr_count: i32,
    /* number of pages to page through	*/
    showstr_page: i32,
    /* which page are we currently showing?	*/
    str: Option<DepotId>,
    /* for the modify-str system		*/
    pub max_str: usize,
    /*		-			*/
    mail_to: i64,
    /* name for mail system			*/
    has_prompt: bool,
    /* is the user at a prompt?             */
    inbuf: String,
    /* buffer for raw input		*/
    last_input: String,
    /* the last input			*/
    history: [String; HISTORY_SIZE],
    /* History of commands, for ! mostly.	*/
    history_pos: usize,
    /* Circular array position.		*/
    output: Vec<u8>,
    input: LinkedList<TxtBlock>,
    character: Option<DepotId>,
    /* linked to char			*/
    original: Option<DepotId>,
    /* original char if switched		*/
    snooping: Option<DepotId>,
    /* Who is this char snooping	*/
    snoop_by: Option<DepotId>,
    /* And who is snooping this char	*/
    websocket_input_buffer: Vec<u8>,
    /* Buffer for WebSocket input messages */
}

impl HasId for DescriptorData {
    fn id(&self) -> DepotId {
        self.id
    }

    fn set_id(&mut self, id: DepotId) {
        self.id = id;
    }
}

impl Default for DescriptorData {
    fn default() -> Self {
        DescriptorData {
            id: Default::default(),
            connection: None,
            host: Rc::from(""),
            bad_pws: 0,
            idle_tics: 0,
            connected: ConGetName,
            desc_num: 0,
            login_time: Instant::now(),
            showstr_head: None,
            showstr_vector: vec![],
            showstr_count: 0,
            showstr_page: 0,
            websocket_input_buffer: vec![],
            str: None,
            max_str: 0,
            mail_to: 0,
            has_prompt: false,
            inbuf: String::new(),
            last_input: "".to_string(),
            history: [(); HISTORY_SIZE].map(|_| String::new()),
            history_pos: 0,
            output: vec![],
            input: LinkedList::new(),
            character: None,
            original: None,
            snooping: None,
            snoop_by: None,
        }
    }
}

pub struct Game {
    mother_desc: Option<TcpListener>,
    websocket_listener: Option<TcpListener>,
    descriptors: Depot<DescriptorData>,
    descriptor_list: Vec<DepotId>,
    last_desc: usize,
    circle_shutdown: bool,
    /* clean shutdown */
    circle_reboot: bool,
    /* reboot the game after a shutdown */
    max_players: i32,
    /* max descriptors available */
    // tics: i32,
    /* for extern checkpointing */
    // byte reread_wizlist;		/* signal: SIGUSR1 */
    // byte emergency_unban;		/* signal: SIGUSR2 */
    mins_since_crashsave: u32,
    config: Config,
}

/***********************************************************************
*  main game loop and related stuff                                    *
***********************************************************************/

fn main() -> ExitCode {
    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let mut port = DFLT_PORT;

    let mut game = Game {
        descriptors: Depot::new(),
        descriptor_list: vec![],
        last_desc: 0,
        circle_shutdown: false,
        circle_reboot: false,
        mother_desc: None,
        websocket_listener: None,
        // tics: 0,
        mins_since_crashsave: 0,
        config: Config {
            nameserver_is_slow: false,
            track_through_doors: true,
        },
        max_players: 0,
    };
    let mut texts: Depot<TextData> = Depot::new();
    let mut objs: Depot<ObjData> = Depot::new();
    let mut chars: Depot<CharData> = Depot::new();

    let mut db = DB::new(&mut texts);
    
    // Parse command line arguments using clap
    let args = Args::parse();
    
    // Apply parsed arguments
    let mut logname: Option<&str> = LOGNAME;
    let custom_logfile;
    if let Some(ref log_file) = args.log_file {
        custom_logfile = log_file.clone();
        logname = Some(&custom_logfile);
    }
    
    let dir = args.directory;
    
    if let Some(arg_port) = args.port {
        if arg_port <= 1024 {
            error!("SYSERR: Illegal port number {}.", arg_port);
            process::exit(1);
        }
        port = arg_port;
    }
    
    // Apply boolean flags
    if args.syntax_check {
        db.scheck = true;
        info!("Syntax check mode enabled.");
    }
    
    if args.mini_mud {
        db.mini_mud = true;
        db.no_rent_check = true;
        info!("Running in minimized mode & with no rent check.");
    }
    
    if args.quick_boot {
        db.no_rent_check = true;
        info!("Quick boot mode -- rent check supressed.");
    }
    
    if args.restrict {
        db.circle_restrict = 1;
        info!("Restricting game -- no new players allowed.");
    }
    
    if args.no_specials {
        db.no_specials = true;
        info!("Suppressing assignment of special routines.");
    }

    /* All arguments have been parsed, try to open log file. */
    setup_log(logname);

    /*
     * Moved here to distinguish command line options and to show up
     * in the log if stderr is redirected to a file.
     */
    info!("{}", CIRCLEMUD_VERSION);

    env::set_current_dir(Path::new(&dir)).unwrap_or_else(|error| {
        eprint!(
            "SYSERR: Fatal error changing to data directory {}/{}: {}",
            env::current_dir().unwrap().display(),
            dir,
            error
        );
        process::exit(1);
    });

    info!("Using {} as data directory.", dir);

    if db.scheck {
        boot_world(&mut game, &mut db, &mut chars, &mut texts);
    } else {
        info!("Running game on port {}.", port);
        game.mother_desc = Some(init_socket(port));
        game.websocket_listener = Some(init_socket(port + 1));
        info!("WebSocket server listening on port {}.", port + 1);
        game.init_game(&mut chars, &mut db, &mut texts, &mut objs, port);
    }

    info!("Clearing game world.");
    db.destroy_db(&mut game.descriptors, &mut chars, &mut objs);

    if !db.scheck {
        info!("Clearing other memory.");
        db.free_player_index(); /* db.rs */
        free_messages(&mut db); /* fight.rs */
        db.mails.clear_free_list(); /* mail.rs */
        db.free_text_files(); /* db.rs */
        board_clear_all(&mut db.boards, &mut texts); /* boards.rs */
        db.cmd_sort_info.clear(); /* act.informative.rs */
        free_social_messages(&mut db); /* act.social.rs */
        db.free_help(); /* db.rs */
        free_invalid_list(&mut db); /* ban.rs */
    }

    info!("Done.");
    ExitCode::SUCCESS
}

impl Game {
    /* Init sockets, run game, and cleanup sockets */
    fn init_game(
        &mut self,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        texts: &mut Depot<TextData>,
        objs: &mut Depot<ObjData>,
        _port: u16,
    ) {
        /* We don't want to restart if we crash before we get up. */
        touch(Path::new(KILLSCRIPT_FILE)).expect("Cannot create KILLSCRIPT path");

        info!("Finding player limit.");
        self.max_players = get_max_players();

        info!("Opening mother connection.");
        db.boot_db(self, chars, texts, objs);

        // info!("Signal trapping.");
        // signal_setup();

        /* If we made it this far, we will be able to restart without problem. */
        fs::remove_file(Path::new(KILLSCRIPT_FILE)).unwrap();

        info!("Entering game loop.");

        self.game_loop(chars, db, texts, objs);

        crash_save_all(self, chars, db, objs);

        info!("Closing all sockets.");
        let ids = self.descriptor_list.clone();
        ids.iter()
            .for_each(|d| self.close_socket(chars, db, texts, objs, *d));

        self.mother_desc = None;

        info!("Saving current MUD time.");
        save_mud_time(&db.time_info);

        if self.circle_reboot {
            info!("Rebooting.");
            process::exit(52); /* what's so great about HHGTTG, anyhow? */
        }
        info!("Normal termination of game.");
    }
}

/*
 * init_socket sets up the mother descriptor - creates the socket, sets
 * its options up, binds it, and listens.
 */
fn init_socket(port: u16) -> TcpListener {
    let socket_addr = SocketAddr::new(get_bind_addr(), port);
    let listener = TcpListener::bind(socket_addr).unwrap_or_else(|error| {
        error!("SYSERR: Error creating socket {}", error);
        process::exit(1);
    });
    listener
        .set_nonblocking(true)
        .expect("Non blocking has issue");
    listener
}

fn get_max_players() -> i32 {
    return MAX_PLAYING;
}

/*
 * game_loop contains the main loop which drives the entire MUD.  It
 * cycles once every 0.10 seconds and is responsible for accepting new
 * new connections, polling existing connections for input, dequeueing
 * output and sending it out to players, and calling "heartbeat" functions
 * such as mobile_activity().
 */
impl Game {
    pub fn desc(&self, desc_id: DepotId) -> &DescriptorData {
        self.descriptors.get(desc_id)
    }

    pub fn desc_mut(&mut self, desc_id: DepotId) -> &mut DescriptorData {
        self.descriptors.get_mut(desc_id)
    }

    fn game_loop(
        &mut self,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        texts: &mut Depot<TextData>,
        objs: &mut Depot<ObjData>,
    ) {
        let opt_time = Duration::from_micros(OPT_USEC as u64);
        let mut process_time;
        let mut before_sleep;
        let mut timeout;
        let mut comm = String::new();
        let mut pulse: u128 = 0;
        let mut missed_pulses;
        let mut aliased = false;

        let mut last_time = Instant::now();

        /* The Main Loop.  The Big Cheese.  The Top Dog.  The Head Honcho.  The.. */
        while !self.circle_shutdown {
            /* Sleep if we don't have any connections */
            if self.descriptor_list.is_empty() {
                debug!("No connections.  Going to sleep.");
                last_time = Instant::now();
            }

            /*
             * At this point, we have completed all input, output and heartbeat
             * activity from the previous iteration, so we have to put ourselves
             * to sleep until the next 0.1 second tick.  The first step is to
             * calculate how long we took processing the previous iteration.
             */
            before_sleep = Instant::now();
            process_time = before_sleep - last_time;

            /*
             * If we were asleep for more than one pass, count missed pulses and sleep
             * until we're resynchronized with the next upcoming pulse.
             */
            if process_time.as_micros() < OPT_USEC {
                missed_pulses = 0;
            } else {
                missed_pulses = process_time.as_micros() / OPT_USEC;
                let secs = process_time.as_micros() / 1000000;
                let usecs = process_time.as_micros() % 1000000;
                process_time = process_time + Duration::new(secs as u64, usecs as u32);
            }

            /* Calculate the time we should wake up */
            let now = Instant::now();
            last_time = before_sleep + opt_time - process_time;
            if last_time > now {
                /* Now keep sleeping until that time has come */
                timeout = last_time - Instant::now();
                thread::sleep(timeout);
            }
            last_time = now;

            /* If there are new telnet connections waiting, accept them. */
            if let Some(ref listener) = self.mother_desc {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        info!("New telnet connection {}.  Waking up.", addr);
                        self.new_descriptor(chars, db, stream, addr);
                    }
                    Err(e) => match e.kind() {
                        ErrorKind::WouldBlock => (),
                        _ => error!("SYSERR: Could not get telnet client {e:?}"),
                    },
                }
            }

            /* If there are new WebSocket connections waiting, accept them. */
            if let Some(ref listener) = self.websocket_listener {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        info!("New WebSocket connection {}.  Waking up.", addr);
                        self.new_websocket_descriptor(chars, db, stream, addr);
                    }
                    Err(e) => match e.kind() {
                        ErrorKind::WouldBlock => (),
                        _ => error!("SYSERR: Could not get WebSocket client {e:?}"),
                    },
                }
            }

            /* Process descriptors with input pending */
            let mut buf = [0u8];
            for d_id in self.descriptor_list.clone() {
                // First, try to poll WebSocket for new messages
                self.poll_websocket_input(d_id);
                
                let has_input = match &self.desc(d_id).connection {
                    Some(ConnectionType::Telnet(stream)) => {
                        match stream.peek(&mut buf) {
                            Ok(size) if size != 0 => true,
                            Ok(_) => false,
                            Err(err) if err.kind() == ErrorKind::WouldBlock => false,
                            Err(err) => {
                                error!("Error while peeking TCP Stream: {} ({})", err, err.kind());
                                false
                            }
                        }
                    }
                    Some(ConnectionType::WebSocket(_)) => {
                        // Check if WebSocket has buffered input
                        !self.desc(d_id).websocket_input_buffer.is_empty()
                    }
                    None => false,
                };
                
                if has_input {
                    process_input(&mut self.descriptors, d_id);
                }
            }

            /* Process commands we just read from process_input */
            for d_id in self.descriptor_list.clone() {
                /*
                 * Not combined to retain --(d->wait) behavior. -gg 2/20/98
                 * If no wait state, no subtraction.  If there is a wait
                 * state then 1 is subtracted. Therefore we don't go less
                 * than 0 ever and don't require an 'if' bracket. -gg 2/27/99
                 */
                {
                    if self.desc(d_id).character.is_some() {
                        let character_id = self.desc(d_id).character.unwrap();
                        let character = chars.get(character_id);
                        let wait_state = character.get_wait_state();
                        if wait_state > 0 {
                            let character = chars.get_mut(character_id);
                            character.decr_wait_state(1);
                        }
                        let character = chars.get(character_id);
                        if character.get_wait_state() != 0 {
                            continue;
                        }
                    }

                    if !get_from_q(&mut self.desc_mut(d_id).input, &mut comm, &mut aliased) {
                        continue;
                    }

                    if self.desc(d_id).character.borrow().is_some() {
                        /* Reset the idle timer & pull char back from void if necessary */
                        let character_id = self.desc(d_id).character.unwrap();
                        let character = chars.get_mut(character_id);
                        character.char_specials.timer = 0;
                        if self.desc(d_id).state() == ConPlaying
                            && character.get_was_in() != NOWHERE
                        {
                            if character.in_room != NOWHERE {
                                db.char_from_room(objs, character);
                            }
                            let character = chars.get(character_id);
                            db.char_to_room(chars, objs, character_id, character.get_was_in());
                            let character = chars.get_mut(character_id);
                            character.set_was_in(NOWHERE);
                            let character = chars.get(character_id);
                            act(&mut self.descriptors, 
                                chars,
                                db,
                                "$n has returned.",
                                true,
                                Some(character),
                                None,
                                None,
                                TO_ROOM,
                            );
                        }
                        let character = chars.get_mut(character_id);
                        character.set_wait_state(1);
                    }

                    self.desc_mut(d_id).has_prompt = false;
                }

                if self.desc(d_id).str.is_some() {
                    /* Writing boards, mail, etc. */
                    string_add(self, chars, db, texts, d_id, &comm);
                } else if self.desc(d_id).showstr_count != 0 {
                    /* Reading something w/ pager */
                    show_string(&mut self.descriptors, chars, d_id, &comm);
                } else if self.desc(d_id).state() != ConPlaying {
                    /* In menus, etc. */
                    nanny(self, db, chars, texts, objs, d_id, &comm);
                } else {
                    /* else: we're playing normally. */
                    if aliased {
                        /* To prevent recursive aliases. */
                        self.desc_mut(d_id).has_prompt = true; /* To get newline before next cmd output. */
                    } else if perform_alias(self, chars, d_id, &mut comm) {
                        /* Run it through aliasing system */
                        get_from_q(&mut self.desc_mut(d_id).input, &mut comm, &mut aliased);
                    }
                    /* Send it to interpreter */
                    let chid = self.desc(d_id).character.unwrap();
                    command_interpreter(self, db, chars, texts, objs, chid, &comm);
                }
            }

            /* Send queued output out to the operating system (ultimately to user). */
            for d_id in self.descriptor_list.clone() {
                let desc = self.desc_mut(d_id);
                if !desc.output.is_empty() {
                    process_output(&mut self.descriptors, chars, d_id);
                    let desc = self.desc_mut(d_id);
                    if desc.output.is_empty() {
                        desc.has_prompt = true;
                    }
                }
            }

            /* Print prompts for other descriptors who had no other output */
            for d_id in self.descriptor_list.clone() {
                let d = self.desc_mut(d_id);
                if !d.has_prompt && d.output.is_empty() {
                    let text = &d.make_prompt(chars);
                    let d = self.desc_mut(d_id);
                    if let Some(ConnectionType::Telnet(ref mut stream)) = d.connection {
                        write_to_descriptor(stream, text.as_bytes());
                    }
                    d.has_prompt = true;
                }
            }

            /* Kick out folks in the ConClose or ConDisconnect state */
            let desc_ids = self.descriptor_list.clone();
            for id in desc_ids {
                let d = self.desc(id);
                if d.state() == ConClose || d.state() == ConDisconnect {
                    self.close_socket(chars, db, texts, objs, id);
                }
            }

            /*
             * Now, we execute as many pulses as necessary--just one if we haven't
             * missed any pulses, or make up for lost time if we missed a few
             * pulses by sleeping for too long.
             */
            missed_pulses += 1;

            if missed_pulses <= 0 {
                error!(
                    "SYSERR: **BAD** MISSED_PULSES NOT POSITIVE {}, TIME GOING BACKWARDS!!",
                    missed_pulses,
                );
                missed_pulses = 1;
            }

            /* If we missed more than 30 seconds worth of pulses, just do 30 secs */
            if missed_pulses > 30 * PASSES_PER_SEC {
                error!(
                    "SYSERR: Missed {} seconds worth of pulses.",
                    missed_pulses / PASSES_PER_SEC,
                );
                missed_pulses = 30 * PASSES_PER_SEC;
            }

            /* Now execute the heartbeat functions */
            while missed_pulses != 0 {
                pulse += 1;
                self.heartbeat(chars, db, texts, objs, pulse);
                missed_pulses -= 1;
            }

            /* Check for any signals we may have received. */
            // if (reread_wizlist) {
            //     reread_wizlist = FALSE;
            //     mudlog(CMP, LVL_IMMORT, TRUE, "Signal received - rereading wizlists.");
            //     reboot_wizlists();
            // }
            // if (emergency_unban) {
            //     emergency_unban = FALSE;
            //     mudlog(BRF, LVL_IMMORT, TRUE, "Received SIGUSR2 - completely unrestricting game (emergent)");
            //     ban_list = NULL;
            //     circle_restrict = 0;
            //     num_invalid = 0;
            // }

            /* Roll pulse over after 10 hours */
            if pulse >= (10 * 60 * 60 * PASSES_PER_SEC) {
                pulse = 0;
            }
        }
    }

    fn heartbeat(
        &mut self,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        texts: &mut Depot<TextData>,
        objs: &mut Depot<ObjData>,
        pulse: u128,
    ) {
        if pulse % PULSE_ZONE == 0 {
            self.zone_update(db, chars, objs);
        }

        if pulse % PULSE_IDLEPWD == 0 {
            /* 15 seconds */
            self.check_idle_passwords();
        }

        if pulse % PULSE_MOBILE == 0 {
            self.mobile_activity(chars, db, texts, objs);
        }

        if pulse % PULSE_VIOLENCE == 0 {
            self.perform_violence(chars, db, texts, objs);
        }

        if pulse as u64 % (SECS_PER_MUD_HOUR * PASSES_PER_SEC as u64) == 0 {
            self.weather_and_time(chars, db, 1);
            affect_update(self, chars, db, objs);
            self.point_update(chars, db, texts, objs);
            //fflush(player_fl);
        }

        if AUTO_SAVE && (pulse % PULSE_AUTOSAVE) != 0 {
            /* 1 minute */
            self.mins_since_crashsave += 1;
            if self.mins_since_crashsave >= AUTOSAVE_TIME as u32 {
                self.mins_since_crashsave = 0;
                crash_save_all(self, chars, db, objs);
                house_save_all(chars, db, objs);
            }
        }

        if pulse % PULSE_USAGE == 0 {
            self.record_usage();
        }

        if pulse % PULSE_TIMESAVE == 0 {
            save_mud_time(&db.time_info);
        }

        /* Every pulse! Don't want them to stink the place up... */
        self.extract_pending_chars(chars, db, texts, objs);
    }
}

/* ******************************************************************
*  general utility stuff (for local use)                            *
****************************************************************** */

/*
 *  new code to calculate time differences, which works on systems
 *  for which tv_usec is unsigned (and thus comparisons for something
 *  being < 0 fail).  Based on code submitted by ss@sirocco.cup.hp.com.
 */
impl Game {
    fn record_usage(&self) {
        let mut sockets_connected = 0;
        let mut sockets_playing = 0;

        for &d_id in &self.descriptor_list {
            let d = self.desc(d_id);
            sockets_connected += 1;
            if d.state() == ConPlaying {
                sockets_playing += 1;
            }
        }

        info!(
            "nusage: {} sockets connected, {} sockets playing",
            sockets_connected, sockets_playing
        );
    }
}

impl DescriptorData {

    /*
     * Turn off echoing (works for both telnet and WebSocket)
     */
    fn echo_off(&mut self) {
        match &self.connection {
            Some(ConnectionType::Telnet(_)) => {
                self.output.extend_from_slice(&[IAC, WILL, TELOPT_ECHO]);
            }
            Some(ConnectionType::WebSocket(_)) => {
                // Send special WebSocket control message
                self.output.extend_from_slice(b"\x1b[ECHO_OFF]");
            }
            None => {}
        }
    }

    /*
     * Turn on echoing (works for both telnet and WebSocket)
     */
    fn echo_on(&mut self) {
        match &self.connection {
            Some(ConnectionType::Telnet(_)) => {
                self.output.extend_from_slice(&[IAC, WONT, TELOPT_ECHO]);
            }
            Some(ConnectionType::WebSocket(_)) => {
                // Send special WebSocket control message
                self.output.extend_from_slice(b"\x1b[ECHO_ON]");
            }
            None => {}
        }
    }

    fn make_prompt(&mut self, chars: &Depot<CharData>) -> String {
        let mut prompt = "".to_string();

        /* Note, prompt is truncated at MAX_PROMPT_LENGTH chars (structs.h) */

        if self.str.borrow().is_some() {
            prompt.push_str("] ");
        } else if self.showstr_count != 0 {
            prompt.push_str(&*format!(
                "\r\n[ Return to continue, (q)uit, (r)efresh, (b)ack, or page number ({}/{}) ]",
                self.showstr_page, self.showstr_count
            ));
        } else if self.connected == ConPlaying && !chars.get(self.character.unwrap()).is_npc() {
            let character_id = self.character.unwrap();
            let character = chars.get(character_id);
            if character.get_invis_lev() != 0 && prompt.len() < MAX_PROMPT_LENGTH {
                let il = character.get_invis_lev();
                prompt.push_str(format!("i{} ", il).as_str());
            }

            if character.prf_flagged(PrefFlags::DISPHP) && prompt.len() < MAX_PROMPT_LENGTH {
                let hit = character.get_hit();
                prompt.push_str(format!("{}H ", hit).as_str());
            }

            if character.prf_flagged(PrefFlags::DISPMANA) && prompt.len() < MAX_PROMPT_LENGTH {
                let mana = character.get_mana();
                prompt.push_str(format!("{}M ", mana).as_str());
            }

            if character.prf_flagged(PrefFlags::DISPMOVE) && prompt.len() < MAX_PROMPT_LENGTH {
                let _move = character.get_move();
                prompt.push_str(format!("{}V ", _move).as_str());
            }

            prompt.push_str("> ");
        } else if self.connected == ConPlaying && chars.get(self.character.unwrap()).is_npc() {
            prompt
                .push_str(format!("{}s>", chars.get(self.character.unwrap()).get_name()).as_str());
        }

        prompt
    }
}
fn write_to_q(txt: &str, queue: &mut LinkedList<TxtBlock>, aliased: bool) {
    let newt = TxtBlock {
        text: String::from(txt),
        aliased,
    };

    queue.push_back(newt);
}

fn get_from_q(queue: &mut LinkedList<TxtBlock>, dest: &mut String, aliased: &mut bool) -> bool {
    match queue.pop_front() {
        None => false,
        Some(elt) => {
            *dest = elt.text;
            *aliased = elt.aliased;
            true
        }
    }
}

impl DescriptorData {
    /* Empty the queues before closing connection */
    fn flush_queues(&mut self) {
        self.output.clear();
        self.inbuf.clear();
        self.input.clear();
    }

    /* Add a new string to a player's output queue. */
    fn write_to_output(&mut self, txt: &str) -> usize {
        let payload = txt.as_bytes();
        self.output.extend_from_slice(payload);
        payload.len()
    }
}

/* ******************************************************************
*  socket handling                                                  *
****************************************************************** */

/*
 * get_bind_addr: Return a struct in_addr that should be used in our
 * call to bind().  If the user has specified a desired binding
 * address, we try to bind to it; otherwise, we bind to INADDR_ANY.
 * Note that inet_aton() is preferred over inet_addr() so we use it if
 * we can.  If neither is available, we always bind to INADDR_ANY.
 */

fn get_bind_addr() -> IpAddr {
    let bind_addr;
    let mut use_any = true;
    /* If DLFT_IP is unspecified, use INADDR_ANY */
    match DFLT_IP {
        None => bind_addr = "0.0.0.0".parse::<IpAddr>().unwrap(),
        Some(ip) => {
            match ip.parse::<IpAddr>() {
                Err(err) => {
                    error!(
                        "SYSERR: DFLT_IP of {} appears to be an invalid IP address: {}",
                        DFLT_IP.unwrap(),
                        err
                    );
                    /* If the parsing fails, use INADDR_ANY */
                    bind_addr = "0.0.0.0".parse::<IpAddr>().unwrap();
                }
                Ok(parsed_ip) => {
                    use_any = false;
                    bind_addr = parsed_ip;
                }
            }
        }
    }

    /* Put the address that we've finally decided on into the logs */
    if use_any {
        info!("Binding to all IP interfaces on this host.");
    } else {
        info!("Binding only to IP address {}", bind_addr);
    }
    bind_addr
}

impl Game {
    fn new_descriptor(
        &mut self,
        chars: &Depot<CharData>,
        db: &DB,
        mut stream: TcpStream,
        addr: SocketAddr,
    ) {
        stream
            .set_nonblocking(true)
            .expect("Error with setting nonblocking");

        /* make sure we have room for it */
        if self.descriptor_list.len() >= self.max_players as usize {
            write_to_descriptor(
                &mut stream,
                "Sorry, CircleMUD is full right now... please try again later!\r\n".as_bytes(),
            );
            stream.shutdown(Shutdown::Both).ok();
            return;
        }
        /* create a new descriptor */
        /* initialize descriptor data */
        let mut newd = DescriptorData::default();
        newd.connection = Some(ConnectionType::Telnet(stream));

        /* find the sitename */
        if !self.config.nameserver_is_slow {
            let r = dns_lookup::lookup_addr(&addr.ip());
            if r.is_err() {
                error!("Error resolving address: {}", r.err().unwrap());
                newd.host = Rc::from(addr.ip().to_string());
            } else {
                newd.host = Rc::from(r.unwrap());
            }
        } else {
            newd.host = Rc::from(addr.ip().to_string());
        }

        /* determine if the site is banned */
        if isbanned(db, &newd.host) == BanType::All {
            if let Some(ConnectionType::Telnet(ref mut stream)) = newd.connection {
                stream.shutdown(Shutdown::Both)
                    .expect("shutdowning socket which is banned");
            }
            self.mudlog(
                chars,
                CMP,
                LVL_GOD as i32,
                true,
                format!("Connection attempt denied from [{}]", newd.host).as_str(),
            );
            return;
        }

        /*
         * This isn't exactly optimal but allows us to make a design choice.
         * Do we embed the history in descriptor_data or keep it dynamically
         * allocated and allow a user defined history size?
         */
        // TODO CREATE(newd -> history, char *, HISTORY_SIZE);
        self.last_desc += 1;
        if self.last_desc == 1000 {
            self.last_desc = 1;
        }
        newd.desc_num = self.last_desc;

        newd.write_to_output(&db.greetings);

        /* append to list */
        let newd_id = self.descriptors.push(newd);
        self.descriptor_list.push(newd_id);
    }

    fn new_websocket_descriptor(
        &mut self,
        chars: &Depot<CharData>,
        db: &DB,
        stream: TcpStream,
        addr: SocketAddr,
    ) {
        // Set non-blocking for the initial stream
        stream.set_nonblocking(true).expect("Error with setting nonblocking");

        /* make sure we have room for it */
        if self.descriptor_list.len() >= self.max_players as usize {
            // For WebSocket, we can't easily write a rejection message before handshake
            // Just close the connection
            return;
        }

        // Perform WebSocket handshake (this would need to be async in a real implementation)
        // For now, we'll create a placeholder - in practice you'd use tokio-tungstenite
        // This is a simplified synchronous approach
        match self.perform_websocket_handshake(stream) {
            Ok(ws_stream) => {
                let mut newd = DescriptorData::default();
                newd.connection = Some(ConnectionType::WebSocket(ws_stream));

                /* find the sitename */
                if !self.config.nameserver_is_slow {
                    let r = dns_lookup::lookup_addr(&addr.ip());
                    if r.is_err() {
                        error!("Error resolving address: {}", r.err().unwrap());
                        newd.host = Rc::from(addr.ip().to_string());
                    } else {
                        newd.host = Rc::from(r.unwrap());
                    }
                } else {
                    newd.host = Rc::from(addr.ip().to_string());
                }

                /* determine if the site is banned */
                if isbanned(db, &newd.host) == BanType::All {
                    self.mudlog(
                        chars,
                        CMP,
                        LVL_GOD as i32,
                        true,
                        format!("WebSocket connection attempt denied from [{}]", newd.host).as_str(),
                    );
                    return;
                }

                self.last_desc += 1;
                if self.last_desc == 1000 {
                    self.last_desc = 1;
                }
                newd.desc_num = self.last_desc;

                newd.write_to_output(&db.greetings);

                let desc_id = self.descriptors.push(newd);
                self.descriptor_list.push(desc_id);
                
                info!("WebSocket connection established for {}", addr);
            }
            Err(e) => {
                error!("WebSocket handshake failed: {}", e);
            }
        }
    }

    fn perform_websocket_handshake(&self, stream: TcpStream) -> Result<WebSocket<TcpStream>, Box<dyn std::error::Error>> {
        match accept(stream) {
            Ok(websocket) => Ok(websocket),
            Err(e) => Err(e.into())
        }
    }

    fn poll_websocket_input(&mut self, d_id: DepotId) {
        if let Some(ConnectionType::WebSocket(ref mut ws)) = self.desc_mut(d_id).connection.as_mut() {
            match ws.read() {
                Ok(Message::Text(text)) => {
                    self.desc_mut(d_id).websocket_input_buffer.extend_from_slice(text.as_bytes());
                }
                Ok(Message::Binary(data)) => {
                    self.desc_mut(d_id).websocket_input_buffer.extend_from_slice(&data);
                }
                Ok(Message::Close(_)) => {
                    // Connection closed - this will be handled elsewhere
                }
                Ok(_) => {
                    // Ignore ping/pong frames
                }
                Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available - this is normal for non-blocking sockets
                }
                Err(_) => {
                    // Other errors - connection may be broken
                }
            }
        }
    }
}

/*
 * Send all of the output that we've accumulated for a player out to
 * the player's descriptor.
 *
 * 32 byte GARBAGE_SPACE in MAX_SOCK_BUF used for:
 *	 2 bytes: prepended \r\n
 *	14 bytes: overflow message
 *	 2 bytes: extra \r\n for non-comapct
 *      14 bytes: unused
 */
    fn process_output(descs: &mut Depot<DescriptorData>, chars: &Depot<CharData>, desc_id: DepotId) -> i32 {
        /* we may need this \r\n for later -- see below */
        let mut i = "\r\n".as_bytes().to_vec();
        let mut result;

        let desc = descs.get_mut(desc_id);
        /* now, append the 'real' output */
        i.append(&mut desc.output);

        /* add the extra CRLF if the person isn't in compact mode */
        if desc.connected == ConPlaying
            && desc.character.is_some()
            && !chars.get(desc.character.unwrap()).is_npc()
            && chars.get(desc.character.unwrap()).prf_flagged(PrefFlags::COMPACT)
        {
            i.extend_from_slice("\r\n".as_bytes());
        }

        /* add a prompt */
        i.extend_from_slice(desc.make_prompt(chars).as_bytes());

        /*
         * now, send the output.  If this is an 'interruption', use the prepended
         * CRLF, otherwise send the straight output sans CRLF.
         */
        if desc.has_prompt {
            desc.has_prompt = false;
            result = match &mut desc.connection {
                Some(ConnectionType::Telnet(ref mut stream)) => write_to_descriptor(stream, &i),
                Some(ConnectionType::WebSocket(ref mut ws)) => {
                    // Send WebSocket text message
                    match ws.send(Message::Text(String::from_utf8_lossy(&i).to_string())) {
                        Ok(_) => i.len() as i32,
                        Err(_) => -1,
                    }
                }
                None => -1,
            };
            if result >= 2 {
                result -= 2;
            }
        } else {
            result = match &mut desc.connection {
                Some(ConnectionType::Telnet(ref mut stream)) => write_to_descriptor(stream, &i[2..]),
                Some(ConnectionType::WebSocket(ref mut ws)) => {
                    // Send WebSocket text message (skip the first 2 bytes)
                    match ws.send(Message::Text(String::from_utf8_lossy(&i[2..]).to_string())) {
                        Ok(_) => (i.len() - 2) as i32,
                        Err(_) => -1,
                    }
                }
                None => -1,
            };
        }

        if result < 0 {
            /* Oops, fatal error. Bye! */
            if let Some(ConnectionType::Telnet(ref mut stream)) = desc.connection {
                let _ = stream.shutdown(Shutdown::Both);
            }
            return -1;
        } else if result == 0 {
            /* Socket buffer full. Try later. */
            return 0;
        }

        /* Handle snooping: prepend "% " and send to snooper. */
        if desc.snoop_by.is_some() {
            let snooper_id =  descs.get_mut(desc_id).snoop_by.unwrap();
            let snooper =  descs.get_mut(snooper_id);
            snooper.write_to_output(format!("% {}%%", result).as_str());
        }
        let desc =  descs.get_mut(desc_id);

        /* The common case: all saved output was handed off to the kernel buffer. */
        let exp_len = (i.len() - 2) as i32;
        if result >= exp_len {
            // already cleared by append ...
            // descs.get_mut(desc_id).output.clear();
        } else {
            /* Not all data in buffer sent.  result < output buffersize. */
            desc.output = i.split_off(result as usize);
        }
        result
    }

/*
 * write_to_descriptor takes a descriptor, and text to write to the
 * descriptor.  It keeps calling the system-level write() until all
 * the text has been delivered to the OS, or until an error is
 * encountered.
 *
 * Returns:
 * >=0  If all is well and good.
 *  -1  If an error was encountered, so that the player should be cut off.
 */
fn write_to_descriptor(stream: &mut TcpStream, txt: &[u8]) -> i32 {
    let mut txt = txt;
    let mut total = txt.len();
    let mut write_total = 0;

    while total > 0 {
        match stream.write(txt.as_ref()) {
            Err(err) => {
                /* Fatal error.  Disconnect the player. */
                error!("SYSERR: Write to socket {}", err);
                return -1;
            }
            Ok(0) => return write_total, /* Temporary failure -- socket buffer full. */
            Ok(bytes_written) => {
                txt = &txt[bytes_written..];
                total -= bytes_written;
                write_total += bytes_written as i32;
            }
        }
    }
    write_total
}

/*
 * Same information about perform_socket_write applies here. I like
 * standards, there are so many of them. -gg 6/30/98
 */
fn perform_socket_read(d: &mut DescriptorData) -> std::io::Result<usize> {
    let input = &mut d.inbuf;
    let mut buf = [0u8; 4096];

    let read_result = match &mut d.connection {
        Some(ConnectionType::Telnet(ref mut stream)) => stream.read(&mut buf),
        Some(ConnectionType::WebSocket(_)) => {
            // Read from the buffered WebSocket input
            if d.websocket_input_buffer.is_empty() {
                Ok(0) // No buffered data
            } else {
                let len = d.websocket_input_buffer.len().min(buf.len());
                buf[..len].copy_from_slice(&d.websocket_input_buffer[..len]);
                d.websocket_input_buffer.drain(..len);
                Ok(len)
            }
        }
        None => return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "No connection")),
    };

    match read_result {
        Err(err) => {
            error!("{:?}", err);
            Err(err)
        }
        Ok(r) => match std::str::from_utf8(&buf[..r]) {
            Err(err) => {
                if err.valid_up_to() == 0 && buf[0] == IAC && r == 3 {
                    // this is a telnet command, no worries we can ignore that read
                } else {
                    error!(
                        "UTF-8 ERROR read={} invalid={:?} err={:?}",
                        r,
                        buf[err.valid_up_to()],
                        err
                    );
                }
                Ok(0)
            }
            Ok(s) => {
                input.push_str(s);
                Ok(r)
            }
        },
    }
}

/*
 * ASSUMPTION: There will be no newlines in the raw input buffer when this
 * function is called.  We must maintain that before returning.
 *
 * Ever wonder why 'tmp' had '+8' on it?  The crusty old code could write
 * MAX_INPUT_LENGTH+1 bytes to 'tmp' if there was a '$' as the final
 * character in the input buffer.  This would also cause 'space_left' to
 * drop to -1, which wasn't very happy in an unsigned variable.  Argh.
 * So to fix the above, 'tmp' lost the '+8' since it doesn't need it
 * and the code has been changed to reserve space by accepting one less
 * character. (Do you really need 256 characters on a line?)
 * -gg 1/21/2000
 */
    fn process_input(descs: &mut Depot<DescriptorData>, d_id: DepotId) -> i32 {
        let buf_length;
        let mut failed_subst;
        let mut bytes_read;
        let mut read_point = 0;
        let mut nl_pos: Option<usize> = None;
        let mut tmp = String::new();
        let desc = descs.get_mut(d_id);

        /* first, find the point where we left off reading data */
        buf_length = desc.inbuf.len();
        let mut space_left = MAX_RAW_INPUT_LENGTH - buf_length - 1;

        loop {
            if space_left <= 0 {
                warn!("WARNING: process_input: about to close connection: input overflow");
                return -1;
            }

            match perform_socket_read(desc) {
                Err(_) => return -1, /* Error, disconnect them. */
                Ok(0) => return 0,   /* Just blocking, no problems. */
                Ok(size) => bytes_read = size,
            }

            /* at this point, we know we got some data from the read */

            /* search for a newline in the data we just read */
            for i in read_point..read_point + bytes_read {
                let x = desc.inbuf.chars().nth(i).unwrap();

                if nl_pos.is_some() {
                    break;
                }
                if isnewl!(x) {
                    nl_pos = Some(i);
                }
            }

            read_point += bytes_read;
            space_left -= bytes_read;
            if nl_pos.is_some() {
                break;
            }
        }

        /*
         * okay, at this point we have at least one newline in the string; now we
         * can copy the formatted data to a new array for further processing.
         */

        let mut read_point = 0;

        let ptr = 0usize;
        while nl_pos.is_some() {
            tmp.truncate(0);
            space_left = MAX_INPUT_LENGTH - 1;

            /* The '> 1' reserves room for a '$ => $$' expansion. */
            let desc =  descs.get_mut(d_id);
            for ptr in 0..desc.inbuf.len() {
                let x = desc.inbuf.chars().nth(ptr).unwrap();
                if space_left <= 1 || ptr >= nl_pos.unwrap() {
                    break;
                }
                if x == 8 as char /* \b */ || x == 127 as char {
                    /* handle backspacing or delete key */
                    if !tmp.is_empty() {
                        tmp.pop();
                        if !tmp.is_empty() && tmp.chars().last().unwrap() == '$' {
                            tmp.pop();
                            space_left += 2;
                        } else {
                            space_left += 1;
                        }
                    }
                } else if x.is_ascii() && !x.is_control() {
                    tmp.push(x);
                    if x == '$' {
                        tmp.push(x);
                        space_left -= 2;
                    } else {
                        space_left -= 1;
                    }
                }
            }

            if (space_left <= 0) && (ptr < nl_pos.unwrap()) {
                let write_result = match &mut desc.connection {
                    Some(ConnectionType::Telnet(ref mut stream)) => write_to_descriptor(stream, tmp.as_bytes()),
                    Some(ConnectionType::WebSocket(ref mut ws)) => {
                        match ws.send(Message::Text(tmp.clone())) {
                            Ok(_) => tmp.len() as i32,
                            Err(_) => -1,
                        }
                    }
                    None => -1,
                };
                if write_result < 0 {
                    return -1;
                }
            }

            if desc.snoop_by.is_some() {
                let snooper_id = desc.snoop_by.unwrap();
                let snooper =  descs.get_mut(snooper_id);
                snooper.write_to_output(format!("% {}\r\n", tmp).as_str());
            }
            failed_subst = false;
            let desc =  descs.get_mut(d_id);

            if tmp == "!" {
                /* Redo last command. */
                tmp = desc.last_input.clone();
            } else if tmp.starts_with('!') && tmp.len() > 1 {
                let mut commandln = &tmp[1..];
                let starting_pos = desc.history_pos;
                let mut cnt = if desc.history_pos == 0 {
                    HISTORY_SIZE - 1
                } else {
                    desc.history_pos - 1
                };

                commandln = commandln.trim_start();
                while cnt != starting_pos {
                    if !desc.history[cnt].is_empty()
                        && is_abbrev(commandln, desc.history[cnt].as_str())
                    {
                        tmp = desc.history[cnt].clone();
                        desc.last_input = tmp.clone();
                        desc.write_to_output(format!("{}\r\n", tmp).as_str());
                        break;
                    }
                    if cnt == 0 {
                        /* At top, loop to bottom. */
                        cnt = HISTORY_SIZE;
                    }
                    cnt -= 1;
                }
            } else if tmp.starts_with('^') {
                let orig = desc.last_input.clone();
                failed_subst = desc.perform_subst(orig.as_str(), &mut tmp);
                if !failed_subst {
                    desc.last_input = tmp.to_string();
                }
            } else {
                desc.last_input = tmp.to_string();
                let pos = desc.history_pos;
                desc.history[pos] = tmp.to_string();
                desc.history_pos = desc.history_pos + 1;
                if desc.history_pos >= HISTORY_SIZE {
                    desc.history_pos = 0;
                }
            }

            if !failed_subst {
                write_to_q(tmp.as_str(), &mut desc.input, false);
            }

            /* find the end of this line */
            while nl_pos.unwrap() < desc.inbuf.len()
                && isnewl!(desc.inbuf.chars().nth(nl_pos.unwrap()).unwrap())
            {
                nl_pos = Some(nl_pos.unwrap() + 1);
            }

            /* see if there's another newline in the input buffer */
            read_point = nl_pos.unwrap();
            nl_pos = None;
            for i in read_point..desc.inbuf.len() {
                if isnewl!(desc.inbuf.chars().nth(i).unwrap()) {
                    nl_pos = Some(i);
                    break;
                }
            }
        }
        let desc =  descs.get_mut(d_id);

        desc.inbuf.drain(..read_point);

        return 1;
    }

/* perform substitution for the '^..^' csh-esque syntax orig is the
 * orig string, i.e. the one being modified.  subst contains the
 * substition string, i.e. "^telm^tell"
 */
impl DescriptorData {
    fn perform_subst(&mut self, orig: &str, subst: &mut String) -> bool {
        /*
         * first is the position of the beginning of the first string (the one
         * to be replaced
         */
        let first = subst.as_str()[1..].to_string();
        let second = first.find('^');
        /* now find the second '^' */

        if second.is_none() {
            self.write_to_output("Invalid substitution.\r\n");
            return true;
        }
        /* terminate "first" at the position of the '^' and make 'second' point
         * to the beginning of the second string */
        let (first, mut second) = first.split_at(second.unwrap());
        second = &second[1..];

        /* now, see if the contents of the first string appear in the original */
        let strpos = orig.find(first);
        if strpos.is_none() {
            self.write_to_output("Invalid substitution.\r\n");
            return true;
        }
        let strpos = strpos.unwrap();
        /* now, we construct the new string for output. */

        /* first, everything in the original, up to the string to be replaced */
        let mut newsub = String::new();
        newsub.push_str(&orig[0..strpos]);

        /* now, the replacement string */
        newsub.push_str(second);
        /* now, if there's anything left in the original after the string to
         * replaced, copy that too. */

        if strpos + first.len() < orig.len() {
            newsub.push_str(&orig[orig.len() - strpos - first.len()..]);
        }

        *subst = newsub;

        return false;
    }
}
impl Game {
    pub fn close_socket(
        &mut self,
        chars: &mut Depot<CharData>,
        db: &mut DB,
        texts: &mut Depot<TextData>,
        objs: &mut Depot<ObjData>,
        d_id: DepotId,
    ) {
        self.descriptor_list.retain(|&i| i != d_id);
        let desc = self.descriptors.get_mut(d_id);

        if let Some(ConnectionType::Telnet(ref mut stream)) = desc.connection {
            stream
                .shutdown(Shutdown::Both)
                .expect("SYSERR while closing socket");
        }
        desc.flush_queues();

        /* Forget snooping */
        if desc.snooping.is_some() {
            let snooping_id = desc.snooping.unwrap();
            self.desc_mut(snooping_id).snoop_by = None;
        }
        let desc = self.descriptors.get_mut(d_id);
        if desc.snoop_by.is_some() {
            let snooper_id = self.desc(d_id).snoop_by.unwrap();
            let snooper = self.desc_mut(snooper_id);
            snooper.write_to_output("Your victim is no longer among us.\r\n");
            let desc = self.descriptors.get_mut(d_id);
            desc.snoop_by = None;
        }
        let desc = self.descriptors.get_mut(d_id);
        match desc.character.as_ref() {
            Some(character_id) => {
                /* If we're switched, this resets the mobile taken. */
                chars.get_mut(*character_id).desc = None;

                match desc.state() {
                    ConPlaying | ConDisconnect => {
                        let original = desc.original;
                        let link_challenged_id = if original.is_some() {
                            original.unwrap()
                        } else {
                            desc.character.unwrap()
                        };

                        /* We are guaranteed to have a person. */
                        act(&mut self.descriptors, 
                            chars,
                            db,
                            "$n has lost $s link.",
                            true,
                            Some(chars.get(link_challenged_id)),
                            None,
                            None,
                            TO_ROOM,
                        );
                        save_char(&mut self.descriptors, db, chars, texts, objs, link_challenged_id);
                        self.mudlog(
                            chars,
                            NRM,
                            max(
                                LVL_IMMORT as i32,
                                chars.get(link_challenged_id).get_invis_lev() as i32,
                            ),
                            true,
                            format!(
                                "Closing link to: {}.",
                                chars.get(link_challenged_id).get_name()
                            )
                            .as_str(),
                        );
                    }
                    _ => {
                        let name = chars.get(desc.character.unwrap()).get_name();
                        let char_id = desc.character.unwrap();
                        self.mudlog(
                            chars,
                            CMP,
                            LVL_IMMORT as i32,
                            true,
                            format!(
                                "Losing player: {}.",
                                if name.is_empty() {
                                    name.as_ref()
                                } else {
                                    "<null>"
                                }
                            )
                            .as_str(),
                        );
                        db.free_char(&mut self.descriptors, chars, objs, char_id);
                    }
                }
            }
            None => {
                self.mudlog(
                    chars,
                    CMP,
                    LVL_IMMORT as i32,
                    true,
                    "Losing descriptor without char.",
                );
            }
        }
        let desc = self.descriptors.get_mut(d_id);
        /* JE 2/22/95 -- part of my unending quest to make switch stable */
        if desc.original.is_some() && chars.get(desc.original.unwrap()).desc.is_some() {
            chars.get_mut(desc.original.unwrap()).desc = None;
        }

        self.descriptors.take(d_id);
    }

    fn check_idle_passwords(&mut self) {
        //struct descriptor_data * d, * next_d;
        for d_id in self.descriptor_list.clone() {
            let desc = self.desc_mut(d_id);
            if desc.state() != ConPassword && desc.state() != ConGetName {
                continue;
            }
            if desc.idle_tics == 0 {
                desc.idle_tics = 1;
            } else {
                desc.echo_on();
                desc.write_to_output("\r\nTimed out... goodbye.\r\n");
                desc.set_state(ConClose);
            }
        }
    }
}
/* ******************************************************************
 *  signal-handling functions (formerly signals.c).  UNIX only.      *
 ****************************************************************** */

// # if defined(CIRCLE_UNIX) | | defined(CIRCLE_MACINTOSH)
//
// RETSIGTYPE reread_wizlists(int sig)
// {
// reread_wizlist = TRUE;
// }
//
//
// RETSIGTYPE unrestrict_game(int sig)
// {
// emergency_unban = TRUE;
// }
//
// # ifdef CIRCLE_UNIX
//
// /* clean up our zombie kids to avoid defunct processes */
// RETSIGTYPE reap(int sig)
// {
// while (waitpid( - 1, NULL, WNOHANG) > 0);
//
// my_signal(SIGCHLD, reap);
// }
//
// /* Dying anyway... */
// RETSIGTYPE checkpointing(int sig)
// {
// if ( ! tics) {
// log("SYSERR: CHECKPOINT shutdown: tics not updated. (Infinite loop suspected)");
// abort();
// } else
// tics = 0;
// }
//
//
// /* Dying anyway... */
// RETSIGTYPE hupsig(int sig)
// {
// log("SYSERR: Received SIGHUP, SIGINT, or SIGTERM.  Shutting down...");
// exit(1);            /* perhaps something more elegant should
// 				 * substituted */
// }
//
// # endif    /* CIRCLE_UNIX */
/*
 * This is an implementation of signal() using sigaction() for portability.
 * (sigaction() is POSIX; signal() is not.)  Taken from Stevens' _Advanced
 * Programming in the UNIX Environment_.  We are specifying that all system
 * calls _not_ be automatically restarted for uniformity, because BSD systems
 * do not restart select(), even if SA_RESTART is used.
 *
 * Note that NeXT 2.x is not POSIX and does not have sigaction; therefore,
 * I just define it to be the old signal.  If your system doesn't have
 * sigaction either, you can use the same fix.
 *
 * SunOS Release 4.0.2 (sun386) needs this too, according to Tim Aldric.
 */

// # ifndef POSIX
// # define my_signal(signo, func) signal(signo, func)
// # else
// sigfunc *my_signal(int signo, sigfunc *func)
// {
// struct sigaction sact, oact;
//
// sact.sa_handler = func;
// sigemptyset( & sact.sa_mask);
// sact.sa_flags = 0;
// # ifdef SA_INTERRUPT
// sact.sa_flags |= SA_INTERRUPT; /* SunOS */
// # endif
//
// if (sigaction(signo, & sact, & oact) < 0)
// return (SIG_ERR);
//
// return (oact.sa_handler);
// }
// # endif                /* POSIX */
//
//
// void signal_setup(void)
// {
// # ifndef CIRCLE_MACINTOSH
// struct itimerval itime;
// struct timeval interval;
//
// /* user signal 1: reread wizlists.  Used by autowiz system. */
// my_signal(SIGUSR1, reread_wizlists);
//
// /*
//  * user signal 2: unrestrict game.  Used for emergencies if you lock
//  * yourself out of the MUD somehow.  (Duh...)
//  */
// my_signal(SIGUSR2, unrestrict_game);
//
// /*
//  * set up the deadlock-protection so that the MUD aborts itself if it gets
//  * caught in an infinite loop for more than 3 minutes.
//  */
// interval.tv_sec = 180;
// interval.tv_usec = 0;
// itime.it_interval = interval;
// itime.it_value = interval;
// setitimer(ITIMER_VIRTUAL, & itime, NULL);
// my_signal(SIGVTALRM, checkpointing);
//
// /* just to be on the safe side: */
// my_signal(SIGHUP, hupsig);
// my_signal(SIGCHLD, reap);
// # endif /* CIRCLE_MACINTOSH */
// my_signal(SIGINT, hupsig);
// my_signal(SIGTERM, hupsig);
// my_signal(SIGPIPE, SIG_IGN);
// my_signal(SIGALRM, SIG_IGN);
// }
//
// # endif    /* CIRCLE_UNIX || CIRCLE_MACINTOSH */
/* ****************************************************************
 *       Public routines for system-to-player-communication        *
 **************************************************************** */

    pub fn send_to_char(descs: &mut Depot<DescriptorData>, ch: &CharData, messg: &str) -> usize {
        if ch.desc.is_some() && messg != "" {
            let desc = descs.get_mut(ch.desc.unwrap());
            desc.write_to_output(messg)
        } else {
            0
        }
    }
impl Game {

    pub fn send_to_all(&mut self, messg: &str) {
        if messg.is_empty() {
            return;
        }

        for d_id in self.descriptor_list.clone() {
            let d = self.desc_mut(d_id);
            if d.state() != ConPlaying {
                continue;
            }
            d.write_to_output(messg);
        }
    }

    fn send_to_outdoor(&mut self, chars: &Depot<CharData>, db: &DB, messg: &str) {
        if messg.is_empty() {
            return;
        }

        for desc_id in self.descriptor_list.clone() {
            let desc = self.desc(desc_id);
            if desc.state() != ConPlaying || desc.character.borrow().is_none() {
                continue;
            }
            let character_id = desc.character.unwrap();
            let character = chars.get(character_id);
            if !character.awake() || !db.outside(character) {
                continue;
            }
            let desc = self.desc_mut(desc_id);

            desc.write_to_output(messg);
        }
    }
}
    pub fn send_to_room(descs: &mut Depot<DescriptorData>, chars: &Depot<CharData>, db: &DB, room: RoomRnum, msg: &str) {
        for &chid in &db.world[room as usize].peoples {
            let ch = chars.get(chid);
            if ch.desc.is_none() {
                continue;
            }
            let desc = descs.get_mut(ch.desc.unwrap());
            desc.write_to_output(msg);
        }
    }


const ACTNULL: &str = "<NULL>";

/* higher-level communication: the act() function */
fn perform_act(
    descs: &mut Depot<DescriptorData>,
    chars: &Depot<CharData>,
    db: &DB,
    orig: &str,
    ch: Option<&CharData>,
    obj: Option<&ObjData>,
    vict_obj: Option<VictimRef>,
    to: &CharData,
) {
    let mut uppercasenext = false;
    let mut orig = orig.to_string();
    let mut i: Rc<str>;
    let mut buf = String::new();

    loop {
        if orig.starts_with('$') {
            orig.remove(0);
            match if orig.len() != 0 {
                orig.chars().next().unwrap()
            } else {
                '\0'
            } {
                'n' => {
                    i = pers(descs, chars, db, ch.unwrap(), to);
                }
                'N' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Char(p)) = vict_obj {
                            pers(descs, chars, db, p, to)
                        } else {
                            Rc::from("<INV_CHAR_REF>")
                        }
                    };
                }
                'm' => {
                    i = Rc::from(hmhr(ch.unwrap()));
                }
                'M' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Char(p)) = vict_obj {
                            Rc::from(hmhr(p))
                        } else {
                            Rc::from("<INV_CHAR_DATA>")
                        }
                    };
                }
                's' => {
                    i = Rc::from(hshr(ch.unwrap()));
                }
                'S' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Char(p)) = vict_obj {
                            Rc::from(hshr(p))
                        } else {
                            Rc::from("<INV_CHAR_DATA>")
                        }
                    };
                }
                'e' => {
                    i = Rc::from(hssh(ch.unwrap()));
                }
                'E' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Char(p)) = vict_obj {
                            Rc::from(hssh(p))
                        } else {
                            Rc::from("<INV_CHAR_DATA>")
                        }
                    };
                }
                'o' => {
                    i = if obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        objn(descs, chars, db, obj.unwrap(), to)
                    };
                }
                'O' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Obj(p)) = vict_obj {
                            objn(descs, chars, db, p, to)
                        } else {
                            Rc::from("<INV_OBJ_DATA>")
                        }
                    };
                }
                'p' => {
                    i = if obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        Rc::from(objs(descs, chars, db, obj.unwrap(), to))
                    };
                }
                'P' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Obj(p)) = vict_obj {
                            Rc::from(objs(descs, chars, db, p, to))
                        } else {
                            Rc::from("<INV_OBJ_REF>")
                        }
                    };
                }
                'a' => {
                    i = if obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        Rc::from(sana(obj.unwrap()))
                    };
                }
                'A' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Obj(p)) = vict_obj {
                            Rc::from(sana(p))
                        } else {
                            Rc::from("<INV_OBJ_REF>")
                        }
                    };
                }
                'T' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Str(ref p)) = vict_obj {
                            Rc::from(p.as_ref())
                        } else {
                            Rc::from("<INV_STR_REF>")
                        }
                    };
                }
                'F' => {
                    i = if vict_obj.is_none() {
                        Rc::from(ACTNULL)
                    } else {
                        if let Some(VictimRef::Str(ref p)) = vict_obj {
                            fname(p)
                        } else {
                            Rc::from("<INV_STR_REF>")
                        }
                    };
                }
                /* uppercase previous word */
                'u' => {
                    let pos = buf.rfind(' ');
                    let posi;
                    if pos.is_none() {
                        posi = 0;
                    } else {
                        posi = pos.unwrap();
                    }
                    let sec_part = buf.split_off(posi);
                    buf.push_str(sec_part.to_uppercase().as_str());
                    i = Rc::from("");
                }
                /* uppercase next word */
                'U' => {
                    uppercasenext = true;
                    i = Rc::from("");
                }
                '$' => {
                    i = Rc::from("$");
                }
                _ => {
                    error!("SYSERR: Illegal $-code to act(): {}", orig);
                    error!("SYSERR: {}", orig);
                    i = Rc::from("");
                }
            }
            for c in i.chars() {
                if uppercasenext && !c.is_whitespace() {
                    buf.push(c.to_ascii_uppercase());
                    uppercasenext = false;
                } else {
                    buf.push(c);
                }
            }
            orig.remove(0);
        } else {
            if orig.len() == 0 {
                break;
            }
            let k = orig.remove(0);

            if uppercasenext && !k.is_whitespace() {
                buf.push(k.to_ascii_uppercase());
                uppercasenext = false;
            } else {
                buf.push(k);
            }
        }
    }

    // TODO orig.pop();
    buf.push_str("\r\n");

    let desc_id = to.desc.unwrap();
    let desc = descs.get_mut(desc_id);
    desc.write_to_output(format!("{}", buf).as_str());
}

macro_rules! sendok {
    ($ch:expr, $to_sleeping:expr) => {
        (($ch).desc.is_some()
            && ($to_sleeping != 0 || ($ch).awake())
            && (($ch).is_npc() || !($ch).plr_flagged(PLR_WRITING)))
    };
}

#[derive(Clone, Copy)]
pub enum VictimRef<'a> {
    Char(&'a CharData),
    Obj(&'a ObjData),
    Str(&'a str),
}

pub fn act(
    descs: &mut Depot<DescriptorData>,
    chars: &Depot<CharData>,
    db: &DB,
    str: &str,
    hide_invisible: bool,
    ch: Option<&CharData>,
    obj: Option<&ObjData>,
    vict_obj: Option<VictimRef>,
    _type: i32,
) {
    if str.is_empty() {
        return;
    }

    /*
     * Warning: the following TO_SLEEP code is a hack.
     *
     * I wanted to be able to tell act to deliver a message regardless of sleep
     * without adding an additional argument.  TO_SLEEP is 128 (a single bit
     * high up).  It's ONLY legal to combine TO_SLEEP with one other TO_x
     * command.  It's not legal to combine TO_x's with each other otherwise.
     * TO_SLEEP only works because its value "happens to be" a single bit;
     * do not change it to something else.  In short, it is a hack.
     */

    /* check if TO_SLEEP is there, and remove it if it is. */
    let mut _type = _type;
    let to_sleeping = _type & TO_SLEEP;
    if to_sleeping != 0 {
        _type &= !TO_SLEEP;
    }

    if _type == TO_CHAR {
        if ch.is_some() && sendok!(ch.unwrap(), to_sleeping) {
            perform_act(
                descs,
                chars,
                db,
                str,
                ch,
                obj,
                vict_obj,
                ch.as_ref().unwrap(),
            );
        }
        return;
    }

    if _type == TO_VICT {
        if vict_obj.is_some() {
            if let Some(VictimRef::Char(to_ch)) = vict_obj {
                if sendok!(to_ch, to_sleeping) {
                    perform_act(descs, chars, db, str, ch, obj, vict_obj, to_ch);
                }
            } else {
                error!("Invalid CharData ref for victim! in act");
            }
        }
        return;
    }
    /* ASSUMPTION: at this point we know type must be TO_NOTVICT or TO_ROOM */
    let char_list;
    if ch.is_some() && ch.unwrap().in_room() != NOWHERE {
        char_list = &db.world[ch.unwrap().in_room() as usize].peoples;
    } else if obj.is_some() && obj.as_ref().unwrap().in_room() != NOWHERE {
        char_list = &db.world[obj.as_ref().unwrap().in_room() as usize].peoples;
    } else {
        error!("SYSERR: no valid target to act()!");
        return;
    }

    for &to_id in char_list {
        let to = chars.get(to_id);
        if !sendok!(to, to_sleeping) || (ch.is_some() && to_id == ch.unwrap().id()) {
            continue;
        }
        if hide_invisible && ch.is_some() && !can_see(descs, chars, db, to, ch.unwrap()) {
            continue;
        }
        if _type != TO_ROOM && vict_obj.is_none() {
            continue;
        }
        let same_chr;
        if vict_obj.is_some() {
            if let Some(VictimRef::Char(p)) = vict_obj {
                same_chr = to_id == p.id();
            } else {
                error!("Error in act: invalid CharData ref");
                continue;
            }
        } else {
            same_chr = false;
        }
        if _type != TO_ROOM && same_chr {
            continue;
        }

        perform_act(descs, chars, db, str, ch, obj, vict_obj, to);
    }
}

fn setup_log(logfile: Option<&str>) {
    let stdout = ConsoleAppender::builder().build();

    let mut config_builder = log4rs::config::Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)));

    if logfile.is_some() {
        let file = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
            .build(logfile.unwrap())
            .unwrap();

        config_builder = config_builder.appender(Appender::builder().build("file", Box::new(file)));
    }
    let config = config_builder
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))
        .unwrap();

    log4rs::init_config(config).unwrap();
}
