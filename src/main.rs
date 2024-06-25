/* ************************************************************************
*   File: main.rs                                       Part of CircleMUD *
*  Usage: Communication, socket handling, main(), central game loop       *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */
use std::borrow::Borrow;
use std::cell::RefCell;
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

use depot::{Depot, DepotId, HasId};
use log::{debug, error, info, warn, LevelFilter};

use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::Appender;
use log4rs::config::Root;
use log4rs::encode::pattern::PatternEncoder;
use util::clone_vec2;

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

pub const PAGE_LENGTH: i32 = 22;
pub const PAGE_WIDTH: i32 = 80;

pub const TO_ROOM: i32 = 1;
pub const TO_VICT: i32 = 2;
pub const TO_NOTVICT: i32 = 3;
pub const TO_CHAR: i32 = 4;
pub const TO_SLEEP: i32 = 128; /* to char, even if sleeping */

pub struct DescriptorData {
    id: DepotId,
    stream: Option<TcpStream>,
    // file descriptor for socket
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
    str: Option<Rc<RefCell<String>>>,
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
    output: String,
    input: LinkedList<TxtBlock>,
    character: Option<DepotId>,
    /* linked to char			*/
    original: Option<DepotId>,
    /* original char if switched		*/
    snooping: Option<DepotId>,
    /* Who is this char snooping	*/
    snoop_by: Option<DepotId>,
    /* And who is snooping this char	*/
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
            stream: None,
            host: Rc::from(EMPTY_STRING),
            bad_pws: 0,
            idle_tics: 0,
            connected: ConGetName,
            desc_num: 0,
            login_time: Instant::now(),
            showstr_head: None,
            showstr_vector: vec![],
            showstr_count: 0,
            showstr_page: 0,
            str: None,
            max_str: 0,
            mail_to: 0,
            has_prompt: false,
            inbuf: String::new(),
            last_input: "".to_string(),
            history: [(); HISTORY_SIZE].map(|_| String::new()),
            history_pos: 0,
            output: String::new(),
            input: LinkedList::new(),
            character: None,
            original: None,
            snooping: None,
            snoop_by: None,
        }
    }
}

pub struct Game {
    db: DB,
    mother_desc: Option<TcpListener>,
    descriptor_list: Depot<DescriptorData>,
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

    let mut dir = DFLT_DIR.to_string();
    let mut port = DFLT_PORT;

    let mut game = Box::new(Game {
        descriptor_list: Depot::new(),
        last_desc: 0,
        circle_shutdown: false,
        circle_reboot: false,
        db: DB::new(),
        mother_desc: None,
        // tics: 0,
        mins_since_crashsave: 0,
        config: Config {
            nameserver_is_slow: false,
            track_through_doors: true,
        },
        max_players: 0,
    });
    let mut logname: Option<&str> = LOGNAME;

    let mut pos = 1;
    let args: Vec<String> = env::args().collect();
    let mut arg;
    let mut custom_logfile;
    loop {
        if pos >= args.len() {
            break;
        }
        arg = args[pos].clone();
        if !arg.starts_with('-') {
            break;
        }

        arg.remove(0);
        match arg.chars().next().unwrap() {
            'o' => {
                arg.remove(0);
                if arg.len() != 0 {
                    custom_logfile = arg.to_string();
                    logname = Some(&custom_logfile);
                } else if {
                    pos += 1;
                    pos < args.len()
                } {
                    custom_logfile = args[pos].to_string();
                    logname = Some(&custom_logfile);
                } else {
                    error!("SYSERR: File name to log to expected after option -o.");
                    process::exit(1);
                }
            }
            'd' => {
                arg.remove(0);
                if arg.len() != 0 {
                    dir = arg.clone();
                } else if {
                    pos += 1;
                    pos < args.len()
                } {
                    dir = args[pos].clone();
                } else {
                    error!("SYSERR: Directory arg expected after option -d.");
                    process::exit(1);
                }
            }
            'm' => {
                game.db.mini_mud = true;
                game.db.no_rent_check = true;
                info!("Running in minimized mode & with no rent check.");
            }
            'c' => {
                game.db.scheck = true;
                info!("Syntax check mode enabled.");
            }
            'q' => {
                game.db.no_rent_check = true;
                info!("Quick boot mode -- rent check supressed.");
            }
            'r' => {
                game.db.circle_restrict = 1;
                info!("Restricting game -- no new players allowed.");
            }
            's' => {
                game.db.no_specials = true;
                info!("Suppressing assignment of special routines.");
            }
            'h' => {
                /* From: Anil Mahajan <amahajan@proxicom.com> */
                println!(
                    "Usage: {} [-c] [-m] [-q] [-r] [-s] [-d pathname] [port #]\n\
                  -c             Enable syntax check mode.\n\
                  -d <directory> Specify library directory (defaults to 'lib').\n\
                  -h             Print this command line argument help.\n\
                  -m             Start in mini-MUD mode.\n\
                  -o <file>      Write log to <file> instead of stderr.\n\
                  -q             Quick boot (doesn't scan rent for object limits)\n\
                  -r             Restrict MUD -- no new players allowed.\n\
                  -s             Suppress special procedure assignments.\n",
                    args[0]
                );
                process::exit(0);
            }
            _ => {
                eprint!("SYSERR: Unknown option -{} in argument string.\n", arg);
                break;
            }
        }
        pos += 1;
    }

    //game.db.mini_mud = true;

    if pos < args.len() {
        if !args[pos].chars().next().unwrap().is_digit(10) {
            println!(
                "Usage: {} [-c] [-m] [-q] [-r] [-s] [-d pathname] [port #]\n",
                args[0]
            );
            process::exit(1);
        }
        let r = args[pos].parse::<u16>();
        if r.is_err() || {
            port = r.unwrap();
            port <= 1024
        } {
            println!("SYSERR: Illegal port number {}.\n", port);
            process::exit(1);
        }
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

    if game.db.scheck {
        boot_world(&mut game);
    } else {
        info!("Running game on port {}.", port);
        game.mother_desc = Some(init_socket(port));
        game.init_game(port);
    }

    info!("Clearing game world.");
    game.db.destroy_db();

    if !game.db.scheck {
        info!("Clearing other memory.");
        game.db.free_player_index(); /* db.rs */
        free_messages(&mut game.db); /* fight.rs */
        game.db.mails.clear_free_list(); /* mail.rs */
        game.db.free_text_files(); /* db.rs */
        board_clear_all(&mut game.db.boards); /* boards.rs */
        game.db.cmd_sort_info.clear(); /* act.informative.rs */
        free_social_messages(&mut game.db); /* act.social.rs */
        game.db.free_help(); /* db.rs */
        free_invalid_list(&mut game.db); /* ban.rs */
    }

    info!("Done.");
    ExitCode::SUCCESS
}

impl Game {
    /* Init sockets, run game, and cleanup sockets */
    fn init_game(&mut self, _port: u16) {
        /* We don't want to restart if we crash before we get up. */
        touch(Path::new(KILLSCRIPT_FILE)).expect("Cannot create KILLSCRIPT path");

        info!("Finding player limit.");
        self.max_players = get_max_players();

        info!("Opening mother connection.");
        DB::boot_db(self);

        // info!("Signal trapping.");
        // signal_setup();

        /* If we made it this far, we will be able to restart without problem. */
        fs::remove_file(Path::new(KILLSCRIPT_FILE)).unwrap();

        info!("Entering game loop.");

        self.game_loop();

        crash_save_all(self);

        info!("Closing all sockets.");
        let ids = self.descriptor_list.ids();
        ids.iter().for_each(|d| self.close_socket(*d));

        self.mother_desc = None;

        info!("Saving current MUD time.");
        save_mud_time(&self.db.time_info);

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
        self.descriptor_list.get(desc_id)
    }

    pub fn desc_mut(&mut self, desc_id: DepotId) -> &mut DescriptorData {
        self.descriptor_list.get_mut(desc_id)
    }

    fn game_loop(&mut self) {
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

            /* If there are new connections waiting, accept them. */
            let accept_result = self.mother_desc.as_ref().unwrap().accept();
            match accept_result {
                Ok((stream, addr)) => {
                    info!("New connection {}.  Waking up.", addr);
                    self.new_descriptor(stream, addr);
                }
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => (),
                    _ => error!("SYSERR: Could not get client {e:?}"),
                },
            }

            /* Process descriptors with input pending */
            let mut buf = [0u8];
            for d_id in self.descriptor_list.ids() {
                match self.desc(d_id).stream.as_ref().unwrap().peek(&mut buf) {
                    Ok(size) if size != 0 => {
                        self.process_input(d_id);
                    }
                    Ok(_) => (),
                    Err(err) if err.kind() == ErrorKind::WouldBlock => (),
                    Err(err) => error!("Error while peeking TCP Stream: {} ({})", err, err.kind()),
                }
            }

            /* Process commands we just read from process_input */
            let ids = self.descriptor_list.ids();
            for d_id in ids {
                /*
                 * Not combined to retain --(d->wait) behavior. -gg 2/20/98
                 * If no wait state, no subtraction.  If there is a wait
                 * state then 1 is subtracted. Therefore we don't go less
                 * than 0 ever and don't require an 'if' bracket. -gg 2/27/99
                 */
                {
                    if self.descriptor_list.get_mut(d_id).character.is_some() {
                        let character_id = self.descriptor_list.get_mut(d_id).character.unwrap();
                        let character = self.db.ch(character_id);
                        let wait_state = character.get_wait_state();
                        if wait_state > 0 {
                            let character = self.db.ch_mut(character_id);
                            character.decr_wait_state(1);
                        }
                        let character = self.db.ch(character_id);
                        if character.get_wait_state() != 0 {
                            continue;
                        }
                    }

                    if !get_from_q(&mut self.descriptor_list.get_mut(d_id).input, &mut comm, &mut aliased) {
                        continue;
                    }

                    if self.descriptor_list.get_mut(d_id).character.borrow().is_some() {
                        /* Reset the idle timer & pull char back from void if necessary */
                        let character_id = self.descriptor_list.get(d_id).character.borrow().unwrap();
                        let character = self.db.ch_mut(character_id);
                        character.char_specials.timer = 0;
                        if self.descriptor_list.get_mut(d_id).state() == ConPlaying && character.get_was_in() != NOWHERE {
                            if character.in_room != NOWHERE {
                                self.db.char_from_room(character_id);
                            }
                            let character = self.db.ch(character_id);
                            self.db.char_to_room(character_id, character.get_was_in());
                            let character = self.db.ch_mut(character_id);
                            character.set_was_in(NOWHERE);
                            self.act(
                                "$n has returned.",
                                true,
                                Some(character_id),
                                None,
                                None,
                                TO_ROOM,
                            );
                        }
                        let character = self.db.ch_mut(character_id);
                        character.set_wait_state(1);
                    }
                    
                    self.descriptor_list.get_mut(d_id).has_prompt = false;
                }

                if self.descriptor_list.get_mut(d_id).str.is_some() {
                    /* Writing boards, mail, etc. */
                    string_add(self, d_id, &comm);
                } else if self.descriptor_list.get_mut(d_id).showstr_count != 0 {
                    /* Reading something w/ pager */
                    show_string(self, d_id, &comm);
                } else if self.descriptor_list.get_mut(d_id).state() != ConPlaying {
                    /* In menus, etc. */
                    nanny(self, d_id, &comm);
                } else {
                    /* else: we're playing normally. */
                    if aliased {
                        /* To prevent recursive aliases. */
                        self.descriptor_list.get_mut(d_id).has_prompt = true; /* To get newline before next cmd output. */
                    } else if perform_alias(&self.db, self.descriptor_list.get_mut(d_id), &mut comm) {
                        /* Run it through aliasing system */
                        get_from_q(
                            &mut self.descriptor_list.get_mut(d_id).input,
                            &mut comm,
                            &mut aliased,
                        );
                    }
                    /* Send it to interpreter */
                    let chid = self
                        .descriptor_list
                        .get_mut(d_id)
                        .character
                        .as_ref()
                        .unwrap()
                        .clone();
                    command_interpreter(self, chid, &comm);
                }
            }

            /* Send queued output out to the operating system (ultimately to user). */
            for d_id in self.descriptor_list.ids() {
                if !self.desc(d_id).output.is_empty() {
                    self.process_output(d_id);
                    if self.desc(d_id).output.is_empty() {
                        self.desc_mut(d_id).has_prompt = true;
                    }
                }
            }

            /* Print prompts for other descriptors who had no other output */
            for d_id in self.descriptor_list.ids() {
                let d = self.descriptor_list.get_mut(d_id);
                if !d.has_prompt && d.output.is_empty() {
                    let text = &make_prompt(self, d_id);
                    let d = self.descriptor_list.get_mut(d_id);
                    write_to_descriptor(d.stream.as_mut().unwrap(), text);
                    d.has_prompt = true;
                }
            }

            /* Kick out folks in the ConClose or ConDisconnect state */
            let ids = self.descriptor_list.ids();
            for id in ids {
                let d = self.descriptor_list.get(id);
                if d.state() == ConClose || d.state() == ConDisconnect {
                    self.close_socket(id);
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
                self.heartbeat(pulse);
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
}

impl Game {
    fn heartbeat(&mut self, pulse: u128) {
        if pulse % PULSE_ZONE == 0 {
            self.zone_update();
        }

        if pulse % PULSE_IDLEPWD == 0 {
            /* 15 seconds */
            self.check_idle_passwords();
        }

        if pulse % PULSE_MOBILE == 0 {
            self.mobile_activity();
        }

        if pulse % PULSE_VIOLENCE == 0 {
            self.perform_violence();
        }

        if pulse as u64 % (SECS_PER_MUD_HOUR * PASSES_PER_SEC as u64) == 0 {
            self.weather_and_time(1);
            affect_update(self);
            self.point_update();
            //fflush(player_fl);
        }

        if AUTO_SAVE && (pulse % PULSE_AUTOSAVE) != 0 {
            /* 1 minute */
            self.mins_since_crashsave += 1;
            if self.mins_since_crashsave >= AUTOSAVE_TIME as u32 {
                self.mins_since_crashsave = 0;
                crash_save_all(self);
                house_save_all(&mut self.db);
            }
        }

        if pulse % PULSE_USAGE == 0 {
            self.record_usage();
        }

        if pulse % PULSE_TIMESAVE == 0 {
            save_mud_time(&self.db.time_info);
        }

        /* Every pulse! Don't want them to stink the place up... */
        self.extract_pending_chars();
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

        for d in self.descriptor_list.iter() {
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
/*
 * Turn off echoing (specific to telnet client)
 */
impl Game {
    fn echo_off(&mut self, desc_id: DepotId) {
        let mut off_string = String::new();
        off_string.push(char::from(IAC));
        off_string.push(char::from(WILL));
        off_string.push(char::from(TELOPT_ECHO));

        self.write_to_output(desc_id, &off_string);
    }

    /*
     * Turn on echoing (specific to telnet client)
     */
    fn echo_on(&mut self, desc_id: DepotId) {
        let mut off_string = String::new();
        off_string.push(char::from(IAC));
        off_string.push(char::from(WONT));
        off_string.push(char::from(TELOPT_ECHO));

        self.write_to_output(desc_id, &off_string);
    }
}

fn make_prompt(game: &Game, d_id: DepotId) -> String {
    let d = game.desc(d_id);
    let mut prompt = "".to_string();

    /* Note, prompt is truncated at MAX_PROMPT_LENGTH chars (structs.h) */

    if d.str.borrow().is_some() {
        prompt.push_str("] ");
    } else if d.showstr_count != 0 {
        prompt.push_str(&*format!(
            "\r\n[ Return to continue, (q)uit, (r)efresh, (b)ack, or page number ({}/{}) ]",
            d.showstr_page, d.showstr_count
        ));
    } else if d.connected == ConPlaying && !game.db.ch(d.character.borrow().unwrap()).is_npc() {
        let ohc = d.character.borrow();
        let character_id = ohc.unwrap();
        let character = game.db.ch(character_id);
        if character.get_invis_lev() != 0 && prompt.len() < MAX_PROMPT_LENGTH as usize {
            let il = character.get_invis_lev();
            prompt.push_str(&*format!("i{} ", il));
        }

        if character.prf_flagged(PRF_DISPHP) && prompt.len() < MAX_PROMPT_LENGTH as usize {
            let hit = character.get_hit();
            prompt.push_str(&*format!("{}H ", hit));
        }

        if character.prf_flagged(PRF_DISPMANA) && prompt.len() < MAX_PROMPT_LENGTH as usize {
            let mana = character.get_mana();
            prompt.push_str(&*format!("{}M ", mana));
        }

        if character.prf_flagged(PRF_DISPMOVE) && prompt.len() < MAX_PROMPT_LENGTH as usize {
            let _move = character.get_move();
            prompt.push_str(&*format!("{}V ", _move));
        }

        prompt.push_str("> ");
    } else if d.connected == ConPlaying && game.db.ch(d.character.unwrap()).is_npc() {
        prompt.push_str(&*format!(
            "{}s>",
            game.db.ch(d.character.unwrap()).get_name()
        ));
    }

    prompt
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

/* Empty the queues before closing connection */
fn flush_queues(d: &mut DescriptorData) {
    d.output.clear();
    d.inbuf.clear();
    d.input.clear();
}

impl Game {
    /* Add a new string to a player's output queue. */
    fn write_to_output(&mut self, desc_id: DepotId, txt: &str) -> usize {
        self.desc_mut(desc_id).output.push_str(txt);
        txt.as_bytes().len()
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

static EMPTY_STRING: &'static str = "";

impl Game {
    fn new_descriptor(&mut self, mut stream: TcpStream, addr: SocketAddr) {
        stream
            .set_nonblocking(true)
            .expect("Error with setting nonblocking");

        /* make sure we have room for it */
        if self.descriptor_list.len() >= self.max_players as usize {
            write_to_descriptor(
                &mut stream,
                "Sorry, CircleMUD is full right now... please try again later!\r\n",
            );
            stream.shutdown(Shutdown::Both).ok();
            return;
        }
        /* create a new descriptor */
        /* initialize descriptor data */
        let mut newd = DescriptorData::default();
        newd.stream = Some(stream);

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
        if isbanned(&self.db, &newd.host) == BAN_ALL {
            newd.stream
                .as_mut()
                .unwrap()
                .shutdown(Shutdown::Both)
                .expect("shutdowning socket which is banned");
            self.mudlog(
                CMP,
                LVL_GOD as i32,
                true,
                format!("Connection attempt denied from [{}]", newd.host).as_str(),
            );
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

        /* append to list */
        let desc_id = self.descriptor_list.push(newd);
        let txt = self.db.greetings.clone();
        self.write_to_output(desc_id, txt.as_ref());

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
impl Game {
    fn process_output(&mut self, desc_id: DepotId) -> i32 {
        /* we may need this \r\n for later -- see below */
        let mut i = "\r\n".to_string();
        let mut result;

        {
        let t = self.desc(desc_id);
        /* now, append the 'real' output */
        i.push_str(&t.output);

        /* add the extra CRLF if the person isn't in compact mode */
        if t.connected == ConPlaying
            && t.character.is_some()
            && !self.db.ch(t.character.unwrap()).is_npc()
            && self.db.ch(t.character
                .unwrap())
                .prf_flagged(PRF_COMPACT)
        {
            i.push_str("\r\n");
        }

        /* add a prompt */
        i.push_str(&make_prompt(self, desc_id));

        /*
         * now, send the output.  If this is an 'interruption', use the prepended
         * CRLF, otherwise send the straight output sans CRLF.
         */
        let t = self.desc_mut(desc_id);
        if t.has_prompt {
            t.has_prompt = false;
            result = write_to_descriptor(t.stream.as_mut().unwrap(), &i);
            if result >= 2 {
                result -= 2;
            }
        } else {
            result = write_to_descriptor(t.stream.as_mut().unwrap(), &i[2..]);
        }

        if result < 0 {
            /* Oops, fatal error. Bye! */
            let _ = t.stream.as_mut().unwrap().shutdown(Shutdown::Both);
            return -1;
        } else if result == 0 {
            /* Socket buffer full. Try later. */
            return 0;
        }
    }

        /* Handle snooping: prepend "% " and send to snooper. */
        if self.desc(desc_id).snoop_by.is_some() {
            let snooper_id = self.desc_mut(desc_id).snoop_by.unwrap();
            self.write_to_output(
                snooper_id,
                format!("% {}%%", result).as_str(),
            );
        }
    

        /* The common case: all saved output was handed off to the kernel buffer. */
        let exp_len = (i.as_bytes().len() - 2) as i32;
        if result >= exp_len {
            self.desc_mut(desc_id).output.clear();
        } else {
            /* Not all data in buffer sent.  result < output buffersize. */
            let _ = self.desc_mut(desc_id).output.split_off(result as usize);
        }
        result
    }
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
fn write_to_descriptor(stream: &mut TcpStream, text: &str) -> i32 {
    let mut txt = text;
    let mut total = txt.as_bytes().len();
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
    let stream = d.stream.as_mut().unwrap();
    let input = &mut d.inbuf;

    let mut buf = [0u8; 4096];

    match stream.read(&mut buf) {
        Err(err) => {
            error!("{:?}", err);
            return Err(err);
        }
        Ok(r) => match std::str::from_utf8(&buf[..r]) {
            Err(err) => {
                error!("UTF-8 ERROR {:?}", err);
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
impl Game {
    fn process_input(&mut self, d_id: DepotId) -> i32 {
        let buf_length;
        let mut failed_subst;
        let mut bytes_read;
        let mut read_point = 0;
        let mut nl_pos: Option<usize> = None;
        let mut tmp = String::new();

        /* first, find the point where we left off reading data */
        buf_length = self.desc(d_id).inbuf.len();
        let mut space_left = MAX_RAW_INPUT_LENGTH - buf_length - 1;

        loop {
            if space_left <= 0 {
                warn!("WARNING: process_input: about to close connection: input overflow");
                return -1;
            }

            match perform_socket_read(self.desc_mut(d_id)) {
                Err(_) => return -1, /* Error, disconnect them. */
                Ok(0) => return 0,   /* Just blocking, no problems. */
                Ok(size) => bytes_read = size,
            }

            /* at this point, we know we got some data from the read */

            /* search for a newline in the data we just read */
            for i in read_point..read_point + bytes_read {
                let x = self.desc(d_id).inbuf.chars().nth(i).unwrap();

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
            for ptr in 0..self.desc(d_id).inbuf.len() {
                let x = self.desc(d_id).inbuf.chars().nth(ptr).unwrap();
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
                if write_to_descriptor(self.desc_mut(d_id).stream.as_mut().unwrap(), tmp.as_str()) < 0 {
                    return -1;
                }
            }

            if self.desc(d_id).snoop_by.is_some() {
                let desc_id = self.desc_mut(d_id).snoop_by.unwrap();
                self.write_to_output(
                    desc_id,
                    format!("% {}\r\n", tmp).as_str(),
                );
            }
            failed_subst = false;

            if tmp == "!" {
                /* Redo last command. */
                tmp = self.desc_mut(d_id).last_input.clone();
            } else if tmp.starts_with('!') && tmp.len() > 1 {
                let mut commandln = &tmp[1..];
                let starting_pos = self.desc(d_id).history_pos;
                let mut cnt = if self.desc(d_id).history_pos == 0 {
                    HISTORY_SIZE - 1
                } else {
                    self.desc_mut(d_id).history_pos - 1
                };

                commandln = commandln.trim_start();
                while cnt != starting_pos {
                    if !self.desc(d_id).history[cnt].is_empty()
                        && is_abbrev(commandln, self.desc(d_id).history[cnt].as_str())
                    {
                        tmp = self.desc(d_id).history[cnt].clone();
                        self.desc_mut(d_id).last_input = tmp.clone();
                        self.write_to_output(d_id, format!("{}\r\n", tmp).as_str());
                        break;
                    }
                    if cnt == 0 {
                        /* At top, loop to bottom. */
                        cnt = HISTORY_SIZE;
                    }
                    cnt -= 1;
                }
            } else if tmp.starts_with('^') {
                let orig = self.desc(d_id).last_input.clone();
                failed_subst = self.perform_subst(d_id, orig.as_str(), &mut tmp);
                if !failed_subst {
                    self.desc_mut(d_id).last_input = tmp.to_string();
                }
            } else {
                self.desc_mut(d_id).last_input = tmp.to_string();
                let pos = self.desc_mut(d_id).history_pos;
                self.desc_mut(d_id).history[pos] = tmp.to_string();
                self.desc_mut(d_id).history_pos = self.desc(d_id).history_pos + 1;
                if self.desc_mut(d_id).history_pos >= HISTORY_SIZE {
                    self.desc_mut(d_id).history_pos = 0;
                }
            }

            if !failed_subst {
                write_to_q(tmp.as_str(), &mut self.desc_mut(d_id).input, false);
            }

            /* find the end of this line */
            while nl_pos.unwrap() < self.desc(d_id).inbuf.len()
                && isnewl!(self.desc(d_id).inbuf.chars().nth(nl_pos.unwrap()).unwrap())
            {
                nl_pos = Some(nl_pos.unwrap() + 1);
            }

            /* see if there's another newline in the input buffer */
            read_point = nl_pos.unwrap();
            nl_pos = None;
            for i in read_point..self.desc(d_id).inbuf.len() {
                if isnewl!(self.desc(d_id).inbuf.chars().nth(i).unwrap()) {
                    nl_pos = Some(i);
                    break;
                }
            }
        }
        self.desc_mut(d_id).inbuf.drain(..read_point);

        return 1;
    }
/* perform substitution for the '^..^' csh-esque syntax orig is the
 * orig string, i.e. the one being modified.  subst contains the
 * substition string, i.e. "^telm^tell"
 */
fn perform_subst(&mut self, desc_id: DepotId, orig: &str, subst: &mut String) -> bool {
    /*
     * first is the position of the beginning of the first string (the one
     * to be replaced
     */
    let first = subst.as_str()[1..].to_string();
    let second = first.find('^');
    /* now find the second '^' */

    if second.is_none() {
        self.write_to_output(desc_id, "Invalid substitution.\r\n");
        return true;
    }
    /* terminate "first" at the position of the '^' and make 'second' point
     * to the beginning of the second string */
    let (first, mut second) = first.split_at(second.unwrap());
    second = &second[1..];

    /* now, see if the contents of the first string appear in the original */
    let strpos = orig.find(first);
    if strpos.is_none() {
        self.write_to_output(desc_id, "Invalid substitution.\r\n");
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
    pub fn close_socket(&mut self, d: DepotId) {
        let mut d = self.descriptor_list.remove(d);

        d.stream
            .as_mut()
            .unwrap()
            .shutdown(Shutdown::Both)
            .expect("SYSERR while closing socket");
        flush_queues(&mut d);

        /* Forget snooping */
        if d.snooping.is_some() {
            self.desc_mut(d.snooping.unwrap()).snoop_by = None;
        }

        if d.snoop_by.is_some() {
            self.write_to_output(
                d.snoop_by.unwrap(),
                "Your victim is no longer among us.\r\n",
            );
            d.snoop_by = None;
        }

        match d.character.as_ref() {
            Some(character_id) => {
                /* If we're switched, this resets the mobile taken. */
                self.db.ch_mut(*character_id).desc = None;

                match d.state() {
                    ConPlaying | ConDisconnect => {
                        let original = d.original;
                        let link_challenged_id = if original.is_some() {
                            original.as_ref().unwrap().clone()
                        } else {
                            d.character.borrow().as_ref().unwrap().clone()
                        };

                        /* We are guaranteed to have a person. */
                        self.act(
                            "$n has lost $s link.",
                            true,
                            Some(link_challenged_id),
                            None,
                            None,
                            TO_ROOM,
                        );
                        self.save_char(link_challenged_id);
                        self.mudlog(
                            NRM,
                            max(LVL_IMMORT as i32, self.db.ch(link_challenged_id).get_invis_lev() as i32),
                            true,
                            format!("Closing link to: {}.", self.db.ch(link_challenged_id).get_name()).as_str(),
                        );
                    }
                    _ => {
                        let name = self.db.ch(d.character.borrow().unwrap()).get_name();
                        self.mudlog(
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
                        self.db.free_char(d.character.unwrap());
                    }
                }
            }
            None => {
                self.mudlog(
                    CMP,
                    LVL_IMMORT as i32,
                    true,
                    "Losing descriptor without char.",
                );
            }
        }

        /* JE 2/22/95 -- part of my unending quest to make switch stable */
        if d.original.is_some()
            && self.db.ch(d.original
                .borrow()
                .unwrap())
                .desc
                .borrow()
                .is_some()
        {
            self.db.ch_mut(d.original.unwrap()).desc = None;
        }
    }

    fn check_idle_passwords(&mut self) {
        //struct descriptor_data * d, * next_d;
        for d_id in self.descriptor_list.ids() {
            if self.desc(d_id).state() != ConPassword && self.desc(d_id).state() != ConGetName {
                continue;
            }
            if self.desc(d_id).idle_tics == 0 {
                self.desc_mut(d_id).idle_tics = 1;
            } else {
                self.echo_on(d_id);
                self.write_to_output(d_id, "\r\nTimed out... goodbye.\r\n");
                self.desc_mut(d_id).set_state(ConClose);
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

impl Game {
    pub fn send_to_char(&mut self, chid: DepotId, messg: &str) -> usize {
        if self.db.ch(chid).desc.is_some() && messg != "" {
            let desc_id = self.db.ch(chid).desc.unwrap();
            return self.write_to_output(
                desc_id,
                messg,
            );
        }
        0
    }
}

impl Game {
    pub fn send_to_all(&mut self, messg: &str) {
        if messg.is_empty() {
            return;
        }

        for d_id in self.descriptor_list.ids() {
            let t = self.descriptor_list.get_mut(d_id);
            if t.state() != ConPlaying {
                continue;
            }
            self.write_to_output(d_id, messg);
        }
    }

    fn send_to_outdoor(&mut self, messg: &str) {
        if messg.is_empty() {
            return;
        }

        for d_id in self.descriptor_list.ids() {
            let i = self.descriptor_list.get_mut(d_id);
            if i.state() != ConPlaying || i.character.borrow().is_none() {
                continue;
            }
            let character_id = i.character.borrow();
            let character = character_id.map(|i| self.db.ch(i));
            if !character.as_ref().unwrap().awake() || !self.db.outside(character.as_ref().unwrap())
            {
                continue;
            }

            self.write_to_output(d_id, messg);
        }
    }

    pub fn send_to_room(&mut self, room: RoomRnum, msg: &str) {
        let list = clone_vec2(&self.db.world[room as usize].peoples);
        for i_id in list {
            let i = self.db.ch(i_id);
            if i.desc.is_none() {
                continue;
            }
            let desc_id = i.desc.unwrap();
            self.write_to_output(desc_id, msg);
        }
    }
}

const ACTNULL: &str = "<NULL>";

impl Game {
    /* higher-level communication: the act() function */
    fn perform_act(
        &mut self,
        orig: &str,
        chid: Option<DepotId>,
        oid: Option<DepotId>,
        vict_obj: Option<VictimRef>,
        to_id: DepotId,
    ) {

        let mut uppercasenext = false;
        let mut orig = orig.to_string();
        let mut i: Rc<str>;
        let mut buf = String::new();

        let obj = oid.map(|e|   self.db.obj(e));

        loop {
            if orig.starts_with('$') {
                orig.remove(0);
                match if orig.len() != 0 {
                    orig.chars().next().unwrap()
                } else {
                    '\0'
                } {
                    'n' => {
                        i = self.pers(self.db.ch(chid.unwrap()), self.db.ch(to_id));
                    }
                    'N' => {
                        i = if vict_obj.clone().is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            if let Some(VictimRef::Char(p)) = vict_obj {
                                self.pers(self.db.ch(p), self.db.ch(to_id))
                            } else {
                                Rc::from("<INV_CHAR_REF>")
                            }
                        };
                    }
                    'm' => {
                        i = Rc::from(hmhr(self.db.ch(chid.unwrap())));
                    }
                    'M' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            if let Some(VictimRef::Char(p)) = vict_obj {
                                Rc::from(hmhr(self.db.ch(p)))
                            } else {
                                Rc::from("<INV_CHAR_DATA>")
                            }
                        };
                    }
                    's' => {
                        i = Rc::from(hshr(self.db.ch(chid.unwrap())));
                    }
                    'S' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            if let Some(VictimRef::Char(p)) = vict_obj {
                                Rc::from(hshr(self.db.ch(p)))
                            } else {
                                Rc::from("<INV_CHAR_DATA>")
                            }
                        };
                    }
                    'e' => {
                        i = Rc::from(hssh(self.db.ch(chid.unwrap())));
                    }
                    'E' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            if let Some(VictimRef::Char(p)) = vict_obj {
                                Rc::from(hssh(self.db.ch(p)))
                            } else {
                                Rc::from("<INV_CHAR_DATA>")
                            }
                        };
                    }
                    'o' => {
                        i = if obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            self.objn(obj.unwrap(), self.db.ch(to_id))
                        };
                    }
                    'O' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            if let Some(VictimRef::Obj(p)) = vict_obj {
                                self.objn(self.db.obj(p), self.db.ch(to_id))
                            } else {
                                Rc::from("<INV_OBJ_DATA>")
                            }
                        };
                    }
                    'p' => {
                        i = if obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            Rc::from(self.objs(obj.unwrap(), self.db.ch(to_id)))
                        };
                    }
                    'P' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            if let Some(VictimRef::Obj(p)) = vict_obj {
                                Rc::from(self.objs(self.db.obj(p), self.db.ch(to_id)))
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
                                Rc::from(sana(self.db.obj(p)))
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

        let desc_id = self.db.ch(to_id).desc.unwrap();
        self.write_to_output(
            desc_id,
            format!("{}", buf).as_str(),
        );
    }
}

macro_rules! sendok {
    ($ch:expr, $to_sleeping:expr) => {
        (($ch).desc.is_some()
            && ($to_sleeping != 0 || ($ch).awake())
            && (($ch).is_npc() || !($ch).plr_flagged(PLR_WRITING)))
    };
}

#[derive(Clone)]
pub enum VictimRef {
    Char(DepotId),
    Obj(DepotId),
    Str(Rc<str>),
}

impl Game {
    pub fn act(
        &mut self,
        str: &str,
        hide_invisible: bool,
        chid: Option<DepotId>,
        oid: Option<DepotId>,
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
            if chid.is_some() && sendok!(self.db.ch(chid.unwrap()), to_sleeping) {
                self.perform_act(str, chid, oid, vict_obj, chid.unwrap());
            }
            return;
        }

        if _type == TO_VICT {
            if vict_obj.is_some() {
                if let Some(VictimRef::Char(to_id)) = vict_obj {
                    if sendok!(self.db.ch(to_id), to_sleeping) {
                        self.perform_act(str, chid, oid, vict_obj.clone(), to_id);
                    }
                } else {
                    error!("Invalid CharData ref for victim! in act");
                }
            }
            return;
        }
        /* ASSUMPTION: at this point we know type must be TO_NOTVICT or TO_ROOM */
        let char_list;
        if chid.is_some() && self.db.ch(chid.unwrap()).in_room() != NOWHERE {
            char_list = &self.db.world[self.db.ch(chid.unwrap()).in_room() as usize].peoples;
        } else if oid.is_some() && self.db.obj(oid.unwrap()).in_room() != NOWHERE {
            char_list = &self.db.world[self.db.obj(oid.unwrap()).in_room() as usize].peoples;
        } else {
            error!("SYSERR: no valid target to act()!");
            return;
        }

        let list = clone_vec2(char_list);
        for to_id in list {
            let to = self.db.ch(to_id);
            if !sendok!(to, to_sleeping)
                || (chid.is_some() && to_id ==  chid.unwrap())
            {
                continue;
            }
            if hide_invisible && chid.is_some() && !self.can_see(to, self.db.ch(chid.unwrap())) {
                continue;
            }
            if _type != TO_ROOM && vict_obj.is_none() {
                continue;
            }
            let same_chr;
            if vict_obj.is_some() {
                if let Some(VictimRef::Char(p_id)) = vict_obj {
                    same_chr = to_id == p_id;
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

            self.perform_act(str, chid, oid, vict_obj.clone(), to.id());
        }
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
