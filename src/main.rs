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
use std::any::Any;
use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
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

use log::{debug, error, info, warn, LevelFilter};

use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::Appender;
use log4rs::config::Root;
use log4rs::encode::pattern::PatternEncoder;

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
use crate::util::{clone_vec, hmhr, hshr, hssh, sana, touch, CMP, NRM, SECS_PER_MUD_HOUR};

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
    stream: RefCell<TcpStream>,
    // file descriptor for socket
    host: RefCell<String>,
    // hostname
    bad_pws: Cell<u8>,
    /* number of bad pw attemps this login	*/
    idle_tics: Cell<u8>,
    /* tics idle at password prompt		*/
    connected: Cell<ConState>,
    // mode of 'connectedness'
    desc_num: Cell<usize>,
    // unique num assigned to desc
    login_time: Instant,
    /* when the person connected		*/
    showstr_head: RefCell<Option<Rc<str>>>,
    /* for keeping track of an internal str	*/
    showstr_vector: RefCell<Vec<Rc<str>>>,
    /* for paging through texts		*/
    showstr_count: Cell<i32>,
    /* number of pages to page through	*/
    showstr_page: Cell<i32>,
    /* which page are we currently showing?	*/
    str: RefCell<Option<Rc<RefCell<String>>>>,
    /* for the modify-str system		*/
    pub max_str: Cell<usize>,
    /*		-			*/
    mail_to: Cell<i64>,
    /* name for mail system			*/
    has_prompt: Cell<bool>,
    /* is the user at a prompt?             */
    inbuf: RefCell<String>,
    /* buffer for raw input		*/
    last_input: RefCell<String>,
    /* the last input			*/
    history: RefCell<[String; HISTORY_SIZE]>,
    /* History of commands, for ! mostly.	*/
    history_pos: Cell<usize>,
    /* Circular array position.		*/
    output: RefCell<String>,
    input: RefCell<LinkedList<TxtBlock>>,
    character: RefCell<Option<Rc<CharData>>>,
    /* linked to char			*/
    original: RefCell<Option<Rc<CharData>>>,
    /* original char if switched		*/
    snooping: RefCell<Option<Rc<DescriptorData>>>,
    /* Who is this char snooping	*/
    snoop_by: RefCell<Option<Rc<DescriptorData>>>,
    /* And who is snooping this char	*/
}

pub struct Game {
    db: DB,
    mother_desc: Option<TcpListener>,
    descriptor_list: RefCell<Vec<Rc<DescriptorData>>>,
    last_desc: Cell<usize>,
    circle_shutdown: Cell<bool>,
    /* clean shutdown */
    circle_reboot: Cell<bool>,
    /* reboot the game after a shutdown */
    max_players: i32,
    /* max descriptors available */
    // tics: i32,
    /* for extern checkpointing */
    // byte reread_wizlist;		/* signal: SIGUSR1 */
    // byte emergency_unban;		/* signal: SIGUSR2 */
    mins_since_crashsave: Cell<u32>,
    config: Config,
}

/***********************************************************************
*  main game loop and related stuff                                    *
***********************************************************************/

fn main() -> ExitCode {
    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let mut dir = DFLT_DIR.to_string();
    let mut port = DFLT_PORT;

    let mut game = Game {
        descriptor_list: RefCell::new(Vec::new()),
        last_desc: Cell::new(0),
        circle_shutdown: Cell::new(false),
        circle_reboot: Cell::new(false),
        db: DB::new(),
        mother_desc: None,
        // tics: 0,
        mins_since_crashsave: Cell::new(0),
        config: Config {
            nameserver_is_slow: Cell::new(false),
            track_through_doors: Cell::new(true),
        },
        max_players: 0,
    };
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
                game.db.circle_restrict.set(1);
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
        game.db.mails.borrow_mut().clear_free_list(); /* mail.rs */
        game.db.free_text_files(); /* db.rs */
        board_clear_all(&mut game.db.boards.borrow_mut()); /* boards.rs */
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
        clone_vec(&self.descriptor_list)
            .iter()
            .for_each(|d| self.close_socket(d));

        self.mother_desc = None;

        info!("Saving current MUD time.");
        save_mud_time(&self.db.time_info.borrow());

        if self.circle_reboot.get() {
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
        while !self.circle_shutdown.get() {
            /* Sleep if we don't have any connections */
            if self.descriptor_list.borrow().is_empty() {
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
            for d in self.descriptor_list.borrow().iter() {
                let res = RefCell::borrow(&d.stream).peek(&mut buf);
                if res.is_ok() && res.unwrap() != 0 {
                    process_input(d);
                }
            }

            /* Process commands we just read from process_input */
            for d in clone_vec(&self.descriptor_list).iter() {
                /*
                 * Not combined to retain --(d->wait) behavior. -gg 2/20/98
                 * If no wait state, no subtraction.  If there is a wait
                 * state then 1 is subtracted. Therefore we don't go less
                 * than 0 ever and don't require an 'if' bracket. -gg 2/27/99
                 */
                if d.character.borrow().is_some() {
                    let ohc = d.character.borrow();
                    let character = ohc.as_ref().unwrap();
                    let wait_state = character.get_wait_state();
                    if wait_state > 0 {
                        character.decr_wait_state(1);
                    }

                    if character.get_wait_state() != 0 {
                        continue;
                    }
                }

                if !get_from_q(&mut d.input.borrow_mut(), &mut comm, &mut aliased) {
                    continue;
                }

                if d.character.borrow().is_some() {
                    /* Reset the idle timer & pull char back from void if necessary */
                    let ohc = d.character.borrow();
                    let character = ohc.as_ref().unwrap();
                    character.char_specials.borrow().timer.set(0);
                    if d.state() == ConPlaying && character.get_was_in() != NOWHERE {
                        if character.in_room.get() != NOWHERE {
                            self.db.char_from_room(character);
                        }
                        self.db
                            .char_to_room(Some(character), character.get_was_in());
                        character.set_was_in(NOWHERE);
                        self.db.act(
                            "$n has returned.",
                            true,
                            d.character.borrow().as_ref(),
                            None,
                            None,
                            TO_ROOM,
                        );
                    }
                    character.set_wait_state(1);
                }
                d.has_prompt.set(false);

                if d.str.borrow().is_some() {
                    /* Writing boards, mail, etc. */
                    string_add(&self.db, d, &comm);
                } else if d.showstr_count.get() != 0 {
                    /* Reading something w/ pager */
                    show_string(d, &comm);
                } else if d.state() != ConPlaying {
                    /* In menus, etc. */
                    nanny(self, d.clone(), &comm);
                } else {
                    /* else: we're playing normally. */
                    if aliased {
                        /* To prevent recursive aliases. */
                        d.has_prompt.set(true); /* To get newline before next cmd output. */
                    } else if perform_alias(d, &mut comm) {
                        /* Run it through aliasing system */
                        get_from_q(&mut d.input.borrow_mut(), &mut comm, &mut aliased);
                    }
                    /* Send it to interpreter */
                    command_interpreter(self, d.character.borrow().as_ref().unwrap(), &comm);
                }
            }

            /* Send queued output out to the operating system (ultimately to user). */
            for d in self.descriptor_list.borrow().iter() {
                if !d.output.borrow().is_empty() {
                    process_output(d);
                    if d.output.borrow().is_empty() {
                        d.has_prompt.set(true);
                    }
                }
            }

            /* Print prompts for other descriptors who had no other output */
            for d in self.descriptor_list.borrow().iter() {
                if !d.has_prompt.get() && d.output.borrow().is_empty() {
                    let text = &make_prompt(d);
                    write_to_descriptor(&mut d.stream.borrow_mut(), text);
                    d.has_prompt.set(true);
                }
            }

            /* Kick out folks in the ConClose or ConDisconnect state */
            for d in clone_vec(&self.descriptor_list).iter() {
                if d.state() == ConClose || d.state() == ConDisconnect {
                    self.close_socket(d);
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
            self.db.zone_update(self);
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
            affect_update(&self.db);
            self.point_update();
            //fflush(player_fl);
        }

        if AUTO_SAVE && (pulse % PULSE_AUTOSAVE) != 0 {
            /* 1 minute */
            self.mins_since_crashsave
                .set(self.mins_since_crashsave.get() + 1);
            if self.mins_since_crashsave.get() >= AUTOSAVE_TIME as u32 {
                self.mins_since_crashsave.set(0);
                crash_save_all(self);
                house_save_all(&self.db);
            }
        }

        if pulse % PULSE_USAGE == 0 {
            self.record_usage();
        }

        if pulse % PULSE_TIMESAVE == 0 {
            save_mud_time(&self.db.time_info.borrow());
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

        for d in self.descriptor_list.borrow().iter() {
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
fn echo_off(d: &DescriptorData) {
    let mut off_string = String::new();
    off_string.push(char::from(IAC));
    off_string.push(char::from(WILL));
    off_string.push(char::from(TELOPT_ECHO));

    write_to_output(d, &off_string);
}

/*
 * Turn on echoing (specific to telnet client)
 */
fn echo_on(d: &DescriptorData) {
    let mut off_string = String::new();
    off_string.push(char::from(IAC));
    off_string.push(char::from(WONT));
    off_string.push(char::from(TELOPT_ECHO));

    write_to_output(d, &off_string);
}

fn make_prompt(d: &DescriptorData) -> String {
    let mut prompt = "".to_string();
    let mut_d = d;

    /* Note, prompt is truncated at MAX_PROMPT_LENGTH chars (structs.h) */

    if mut_d.str.borrow().is_some() {
        prompt.push_str("] ");
    } else if mut_d.showstr_count.get() != 0 {
        prompt.push_str(&*format!(
            "\r\n[ Return to continue, (q)uit, (r)efresh, (b)ack, or page number ({}/{}) ]",
            mut_d.showstr_page.get(),
            mut_d.showstr_count.get()
        ));
    } else if mut_d.connected.get() == ConPlaying
        && !mut_d.character.borrow().as_ref().unwrap().is_npc()
    {
        let ohc = d.character.borrow();
        let character = ohc.as_ref().unwrap();
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
    } else if mut_d.connected.get() == ConPlaying
        && mut_d.character.borrow().as_ref().unwrap().is_npc()
    {
        prompt.push_str(&*format!(
            "{}s>",
            mut_d.character.borrow().as_ref().unwrap().get_name()
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
    let elt = queue.pop_front();
    if elt.is_none() {
        return false;
    }
    let elt = elt.unwrap();
    *dest = elt.text;
    *aliased = elt.aliased;
    return true;
}

/* Empty the queues before closing connection */
fn flush_queues(d: &Rc<DescriptorData>) {
    d.output.borrow_mut().clear();
    d.inbuf.borrow_mut().clear();
    d.input.borrow_mut().clear();
}

/* Add a new string to a player's output queue. */
fn write_to_output(t: &DescriptorData, txt: &str) -> usize {
    t.output.borrow_mut().push_str(txt);
    txt.as_bytes().len()
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
    if DFLT_IP.is_none() {
        bind_addr = "0.0.0.0".parse::<IpAddr>().unwrap();
    } else {
        let r = DFLT_IP.unwrap().parse::<IpAddr>();
        if r.is_err() {
            error!(
                "SYSERR: DFLT_IP of {} appears to be an invalid IP address",
                DFLT_IP.unwrap()
            );
            /* If the parsing fails, use INADDR_ANY */
            bind_addr = "0.0.0.0".parse::<IpAddr>().unwrap();
        } else {
            use_any = false;
            bind_addr = r.unwrap();
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
    fn new_descriptor(&self, mut stream: TcpStream, addr: SocketAddr) {
        stream
            .set_nonblocking(true)
            .expect("Error with setting nonblocking");

        /* make sure we have room for it */
        if self.descriptor_list.borrow().len() >= self.max_players as usize {
            write_to_descriptor(
                &mut stream,
                "Sorry, CircleMUD is full right now... please try again later!\r\n",
            );
            stream.shutdown(Shutdown::Both).ok();
            return;
        }
        /* create a new descriptor */
        /* initialize descriptor data */
        let newd = DescriptorData {
            stream: RefCell::new(stream),
            host: RefCell::new(String::new()),
            bad_pws: Cell::new(0),
            idle_tics: Cell::new(0),
            connected: Cell::new(ConGetName),
            desc_num: Cell::new(0),
            login_time: Instant::now(),
            showstr_head: RefCell::new(None),
            showstr_vector: RefCell::new(vec![]),
            showstr_count: Cell::from(0),
            showstr_page: Cell::from(0),
            str: RefCell::new(None),
            max_str: Cell::new(0),
            mail_to: Cell::new(0),
            has_prompt: Cell::new(false),
            inbuf: RefCell::from(String::new()),
            last_input: RefCell::new("".to_string()),
            history: RefCell::new([(); HISTORY_SIZE].map(|_| String::new())),
            history_pos: Cell::new(0),
            output: RefCell::new(String::new()),
            input: RefCell::new(LinkedList::new()),
            character: RefCell::new(None),
            original: RefCell::new(None),
            snooping: RefCell::new(None),
            snoop_by: RefCell::new(None),
        };

        /* find the sitename */
        if !self.config.nameserver_is_slow.get() {
            let r = dns_lookup::lookup_addr(&addr.ip());
            if r.is_err() {
                error!("Error resolving address: {}", r.err().unwrap());
                *RefCell::borrow_mut(&newd.host) = addr.ip().to_string();
            } else {
                *RefCell::borrow_mut(&newd.host) = r.unwrap();
            }
        } else {
            *RefCell::borrow_mut(&newd.host) = addr.ip().to_string();
        }

        /* determine if the site is banned */
        if isbanned(&self.db, &mut newd.host.borrow_mut()) == BAN_ALL {
            newd.stream
                .borrow_mut()
                .shutdown(Shutdown::Both)
                .expect("shutdowning socket which is banned");
            self.mudlog(
                CMP,
                LVL_GOD as i32,
                true,
                format!("Connection attempt denied from [{}]", newd.host.borrow()).as_str(),
            );
        }

        /*
         * This isn't exactly optimal but allows us to make a design choice.
         * Do we embed the history in descriptor_data or keep it dynamically
         * allocated and allow a user defined history size?
         */
        // TODO CREATE(newd -> history, char *, HISTORY_SIZE);
        self.last_desc.set(self.last_desc.get() + 1);
        if self.last_desc.get() == 1000 {
            self.last_desc.set(1);
        }
        newd.desc_num.set(self.last_desc.get());

        /* append to list */
        let rc = Rc::new(newd);
        self.descriptor_list.borrow_mut().push(rc.clone());

        write_to_output(rc.as_ref(), &self.db.greetings);
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
fn process_output(t: &DescriptorData) -> i32 {
    /* we may need this \r\n for later -- see below */
    let mut i = "\r\n".to_string();
    /* now, append the 'real' output */
    i.push_str(&RefCell::borrow(&t.output));

    /* add the extra CRLF if the person isn't in compact mode */
    if t.connected.get() == ConPlaying
        && t.character.borrow().is_some()
        && !t.character.borrow().as_ref().unwrap().is_npc()
        && t.character
            .borrow()
            .as_ref()
            .unwrap()
            .prf_flagged(PRF_COMPACT)
    {
        i.push_str("\r\n");
    }

    /* add a prompt */
    i.push_str(&make_prompt(t));
    let mut result;

    /*
     * now, send the output.  If this is an 'interruption', use the prepended
     * CRLF, otherwise send the straight output sans CRLF.
     */
    if t.has_prompt.get() {
        t.has_prompt.set(false);
        result = write_to_descriptor(&mut RefCell::borrow_mut(&t.stream), &i);
        if result >= 2 {
            result -= 2;
        }
    } else {
        result = write_to_descriptor(&mut RefCell::borrow_mut(&t.stream), &i[2..]);
    }

    if result < 0 {
        /* Oops, fatal error. Bye! */
        let _ = RefCell::borrow(&t.stream).shutdown(Shutdown::Both);
        return -1;
    } else if result == 0 {
        /* Socket buffer full. Try later. */
        return 0;
    }

    /* Handle snooping: prepend "% " and send to snooper. */
    if t.snoop_by.borrow().is_some() {
        write_to_output(
            t.snoop_by.borrow().as_ref().unwrap(),
            format!("% {}%%", result).as_str(),
        );
    }

    /* The common case: all saved output was handed off to the kernel buffer. */
    let exp_len = (i.as_bytes().len() - 2) as i32;
    if result >= exp_len {
        RefCell::borrow_mut(&t.output).clear();
    } else {
        /* Not all data in buffer sent.  result < output buffersize. */
        let _ = t.output.borrow_mut().split_off(result as usize);
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
fn write_to_descriptor(stream: &mut TcpStream, text: &str) -> i32 {
    let mut txt = text;
    let mut total = txt.as_bytes().len();
    let mut write_total = 0;

    while total > 0 {
        let bytes_written = stream.write(txt.as_ref());

        if bytes_written.is_err() {
            /* Fatal error.  Disconnect the player. */
            error!("SYSERR: Write to socket {}", bytes_written.err().unwrap());
            return -1;
        } else {
            let bytes_written = bytes_written.unwrap();
            if bytes_written == 0 {
                /* Temporary failure -- socket buffer full. */
                return write_total;
            } else {
                txt = &txt[bytes_written..];
                total -= bytes_written;
                write_total += bytes_written as i32;
            }
        }
    }

    return write_total;
}

/*
 * Same information about perform_socket_write applies here. I like
 * standards, there are so many of them. -gg 6/30/98
 */
fn perform_socket_read(d: &DescriptorData) -> std::io::Result<usize> {
    let mut stream = d.stream.borrow_mut();
    let mut input = d.inbuf.borrow_mut();

    let mut buf = [0u8; 4096];

    let r = stream.read(&mut buf);
    if r.is_err() {
        error!("{:?}", r);
        return r;
    }

    let r = r.unwrap();
    let s = std::str::from_utf8(&buf[..r]);
    if s.is_err() {
        error!("UTF-8 ERROR {:?}", r);
        return Ok(0);
    }
    input.push_str(s.unwrap());
    return Ok(r);
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
fn process_input(t: &Rc<DescriptorData>) -> i32 {
    let buf_length;
    let mut failed_subst;
    let mut bytes_read;
    let mut read_point = 0;
    let mut nl_pos: Option<usize> = None;
    let mut tmp = String::new();

    /* first, find the point where we left off reading data */
    buf_length = t.inbuf.borrow().len();
    let mut space_left = MAX_RAW_INPUT_LENGTH - buf_length - 1;

    loop {
        if space_left <= 0 {
            warn!("WARNING: process_input: about to close connection: input overflow");
            return -1;
        }

        bytes_read = perform_socket_read(t);

        if bytes_read.is_err() {
            /* Error, disconnect them. */
            return -1;
        }
        let bytes_read = bytes_read.unwrap();
        if bytes_read == 0 {
            /* Just blocking, no problems. */
            return 0;
        }

        /* at this point, we know we got some data from the read */

        /* search for a newline in the data we just read */
        for i in read_point..read_point + bytes_read {
            let x = t.inbuf.borrow().chars().nth(i).unwrap();

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
        for ptr in 0..t.inbuf.borrow().len() {
            let x = t.inbuf.borrow().chars().nth(ptr).unwrap();
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
            if write_to_descriptor(&mut RefCell::borrow_mut(&t.stream), tmp.as_str()) < 0 {
                return -1;
            }
        }

        if t.snoop_by.borrow().is_some() {
            write_to_output(
                t.snoop_by.borrow().as_ref().unwrap(),
                format!("% {}\r\n", tmp).as_str(),
            );
        }
        failed_subst = false;

        if tmp == "!" {
            /* Redo last command. */
            tmp = t.last_input.borrow().clone();
        } else if tmp.starts_with('!') && tmp.len() > 1 {
            let mut commandln = &tmp[1..];
            let starting_pos = t.history_pos.get();
            let mut cnt = if t.history_pos.get() == 0 {
                HISTORY_SIZE - 1
            } else {
                t.history_pos.get() - 1
            };

            commandln = commandln.trim_start();
            while cnt != starting_pos {
                if !t.history.borrow()[cnt].is_empty()
                    && is_abbrev(commandln, t.history.borrow()[cnt].as_str())
                {
                    tmp = t.history.borrow()[cnt].clone();
                    *t.last_input.borrow_mut() = tmp.clone();
                    write_to_output(t, format!("{}\r\n", tmp).as_str());
                    break;
                }
                if cnt == 0 {
                    /* At top, loop to bottom. */
                    cnt = HISTORY_SIZE;
                }
                cnt -= 1;
            }
        } else if tmp.starts_with('^') {
            failed_subst = perform_subst(t, t.last_input.borrow().as_ref(), &mut tmp);
            if !failed_subst {
                *t.last_input.borrow_mut() = tmp.to_string();
            }
        } else {
            *t.last_input.borrow_mut() = tmp.to_string();
            t.history.borrow_mut()[t.history_pos.get()] = tmp.to_string();
            t.history_pos.set(t.history_pos.get() + 1);
            if t.history_pos.get() >= HISTORY_SIZE {
                t.history_pos.set(0);
            }
        }

        if !failed_subst {
            write_to_q(tmp.as_str(), &mut t.input.borrow_mut(), false);
        }

        /* find the end of this line */
        while nl_pos.unwrap() < t.inbuf.borrow().len()
            && isnewl!(t.inbuf.borrow().chars().nth(nl_pos.unwrap()).unwrap())
        {
            nl_pos = Some(nl_pos.unwrap() + 1);
        }

        /* see if there's another newline in the input buffer */
        read_point = nl_pos.unwrap();
        nl_pos = None;
        for i in read_point..t.inbuf.borrow().len() {
            if isnewl!(t.inbuf.borrow().chars().nth(i).unwrap()) {
                nl_pos = Some(i);
                break;
            }
        }
    }
    t.inbuf.borrow_mut().drain(..read_point);

    return 1;
}

/* perform substitution for the '^..^' csh-esque syntax orig is the
 * orig string, i.e. the one being modified.  subst contains the
 * substition string, i.e. "^telm^tell"
 */
fn perform_subst(t: &Rc<DescriptorData>, orig: &str, subst: &mut String) -> bool {
    /*
     * first is the position of the beginning of the first string (the one
     * to be replaced
     */
    let first = subst.as_str()[1..].to_string();
    let second = first.find('^');
    /* now find the second '^' */

    if second.is_none() {
        write_to_output(t, "Invalid substitution.\r\n");
        return true;
    }
    /* terminate "first" at the position of the '^' and make 'second' point
     * to the beginning of the second string */
    let (first, mut second) = first.split_at(second.unwrap());
    second = &second[1..];

    /* now, see if the contents of the first string appear in the original */
    let strpos = orig.find(first);
    if strpos.is_none() {
        write_to_output(t, "Invalid substitution.\r\n");
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

impl Game {
    pub fn close_socket(&self, d: &Rc<DescriptorData>) {
        self.descriptor_list
            .borrow_mut()
            .retain(|c| !Rc::ptr_eq(c, d));

        d.stream
            .borrow_mut()
            .shutdown(Shutdown::Both)
            .expect("SYSERR while closing socket");
        flush_queues(d);

        /* Forget snooping */
        if d.snooping.borrow().is_some() {
            *d.snooping.borrow().as_ref().unwrap().snoop_by.borrow_mut() = None;
        }

        if d.snoop_by.borrow().is_some() {
            write_to_output(
                d.snoop_by.borrow().as_ref().unwrap(),
                "Your victim is no longer among us.\r\n",
            );
            *d.snoop_by.borrow_mut() = None;
        }

        if d.character.borrow().is_some() {
            /* If we're switched, this resets the mobile taken. */
            *d.character.borrow().as_ref().unwrap().desc.borrow_mut() = None;

            if d.state() == ConPlaying || d.state() == ConDisconnect {
                let original = d.original.borrow();
                let link_challenged = if original.is_some() {
                    original.as_ref().unwrap().clone()
                } else {
                    d.character.borrow().as_ref().unwrap().clone()
                };

                /* We are guaranteed to have a person. */
                self.db.act(
                    "$n has lost $s link.",
                    true,
                    Some(&link_challenged),
                    None,
                    None,
                    TO_ROOM,
                );
                self.db.save_char(&link_challenged);
                self.mudlog(
                    NRM,
                    max(LVL_IMMORT as i32, link_challenged.get_invis_lev() as i32),
                    true,
                    format!("Closing link to: {}.", link_challenged.get_name()).as_str(),
                );
            } else {
                let name = d.character.borrow().as_ref().unwrap().get_name();
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
                free_char(d.character.borrow().as_ref().unwrap());
            }
        } else {
            self.mudlog(
                CMP,
                LVL_IMMORT as i32,
                true,
                "Losing descriptor without char.",
            );
        }

        /* JE 2/22/95 -- part of my unending quest to make switch stable */
        if d.original.borrow().is_some()
            && d.original
                .borrow()
                .as_ref()
                .unwrap()
                .desc
                .borrow()
                .is_some()
        {
            *d.original.borrow().as_ref().unwrap().desc.borrow_mut() = None;
        }
    }

    fn check_idle_passwords(&self) {
        //struct descriptor_data * d, * next_d;
        for d in self.descriptor_list.borrow().iter() {
            if d.state() != ConPassword && d.state() != ConGetName {
                continue;
            }
            if d.idle_tics.get() == 0 {
                d.idle_tics.set(1);
            } else {
                echo_on(d.as_ref());
                write_to_output(d, "\r\nTimed out... goodbye.\r\n");
                d.set_state(ConClose);
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

pub fn send_to_char(ch: &CharData, messg: &str) -> usize {
    if ch.desc.borrow().is_some() && messg != "" {
        return write_to_output(ch.desc.borrow().as_ref().unwrap(), messg);
    }
    0
}

impl Game {
    pub fn send_to_all(&self, messg: &str) {
        if messg.is_empty() {
            return;
        }

        for i in self.descriptor_list.borrow().iter() {
            if i.state() != ConPlaying {
                continue;
            }
            write_to_output(i, messg);
        }
    }

    fn send_to_outdoor(&self, messg: &str) {
        if messg.is_empty() {
            return;
        }

        for i in self.descriptor_list.borrow().iter() {
            if i.state() != ConPlaying || i.character.borrow().is_none() {
                continue;
            }
            let character = i.character.borrow();
            if !character.as_ref().unwrap().awake() || !self.db.outside(character.as_ref().unwrap())
            {
                continue;
            }

            write_to_output(i, messg);
        }
    }
}

impl DB {
    pub fn send_to_room(&self, room: RoomRnum, msg: &str) {
        for i in self.world.borrow()[room as usize].peoples.borrow().iter() {
            if i.desc.borrow().is_none() {
                continue;
            }
            write_to_output(i.desc.borrow().as_ref().unwrap(), msg);
        }
    }
}

const ACTNULL: &str = "<NULL>";

impl DB {
    /* higher-level communication: the act() function */
    fn perform_act(
        &self,
        orig: &str,
        ch: Option<&Rc<CharData>>,
        obj: Option<&Rc<ObjData>>,
        vict_obj: Option<&dyn Any>,
        to: &Rc<CharData>,
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
                        i = self.pers(ch.unwrap(), to);
                    }
                    'N' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            let p = vict_obj.unwrap().downcast_ref::<Rc<CharData>>();
                            if p.is_some() {
                                self.pers(p.unwrap(), to)
                            } else {
                                Rc::from("<INV_CHAR_REV>")
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
                            let p = vict_obj.unwrap().downcast_ref::<Rc<CharData>>();
                            if p.is_some() {
                                Rc::from(hmhr(p.unwrap()))
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
                            let p = vict_obj.unwrap().downcast_ref::<Rc<CharData>>();
                            if p.is_some() {
                                Rc::from(hshr(p.unwrap()))
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
                            let p = vict_obj.unwrap().downcast_ref::<Rc<CharData>>();
                            if p.is_some() {
                                Rc::from(hssh(p.unwrap()))
                            } else {
                                Rc::from("<INV_CHAR_DATA>")
                            }
                        };
                    }
                    'o' => {
                        i = if obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            self.objn(obj.unwrap(), to)
                        };
                    }
                    'O' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            let p = vict_obj.unwrap().downcast_ref::<Rc<ObjData>>();
                            if p.is_some() {
                                self.objn(p.unwrap(), to)
                            } else {
                                Rc::from("<INV_OBJ_DATA>")
                            }
                        };
                    }
                    'p' => {
                        i = if obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            Rc::from(self.objs(obj.unwrap(), to))
                        };
                    }
                    'P' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            let p = vict_obj.unwrap().downcast_ref::<Rc<ObjData>>();
                            if p.is_some() {
                                Rc::from(self.objs(p.unwrap(), to))
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
                            let p = vict_obj.unwrap().downcast_ref::<Rc<ObjData>>();
                            if p.is_some() {
                                Rc::from(sana(p.unwrap()))
                            } else {
                                Rc::from("<INV_OBJ_REF>")
                            }
                        };
                    }
                    'T' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            let p = vict_obj.unwrap().downcast_ref::<String>();
                            if p.is_some() {
                                Rc::from(p.unwrap().as_str())
                            } else {
                                Rc::from("<INV_STR_REF>")
                            }
                        };
                    }
                    'F' => {
                        i = if vict_obj.is_none() {
                            Rc::from(ACTNULL)
                        } else {
                            let p = vict_obj.unwrap().downcast_ref::<String>();
                            if p.is_some() {
                                fname(p.unwrap())
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

        write_to_output(
            to.desc.borrow().as_ref().unwrap().as_ref(),
            format!("{}", buf).as_str(),
        );
    }
}

macro_rules! sendok {
    ($ch:expr, $to_sleeping:expr) => {
        (($ch).desc.borrow().is_some()
            && ($to_sleeping != 0 || ($ch).awake())
            && (($ch).is_npc() || !($ch).plr_flagged(PLR_WRITING)))
    };
}

impl DB {
    pub fn act(
        &self,
        str: &str,
        hide_invisible: bool,
        ch: Option<&Rc<CharData>>,
        obj: Option<&Rc<ObjData>>,
        vict_obj: Option<&dyn Any>,
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
            if ch.is_some() && sendok!(ch.as_ref().unwrap(), to_sleeping) {
                self.perform_act(str, ch, obj, vict_obj, ch.unwrap());
            }
            return;
        }

        if _type == TO_VICT {
            if vict_obj.is_some() {
                let to = vict_obj.unwrap().downcast_ref::<Rc<CharData>>();
                if to.is_some() {
                    if sendok!(to.unwrap(), to_sleeping) {
                        self.perform_act(str, ch, obj, vict_obj, to.unwrap());
                    }
                } else {
                    error!("Invalid CharData ref for victim! in act");
                }
            }
            return;
        }
        /* ASSUMPTION: at this point we know type must be TO_NOTVICT or TO_ROOM */
        let w = self.world.borrow();
        let char_list;
        if ch.is_some() && ch.as_ref().unwrap().in_room() != NOWHERE {
            char_list = &w[ch.as_ref().unwrap().in_room() as usize].peoples;
        } else if obj.is_some() && obj.unwrap().in_room() != NOWHERE {
            char_list = &w[obj.unwrap().in_room() as usize].peoples;
        } else {
            error!("SYSERR: no valid target to act()!");
            return;
        }

        for to in char_list.borrow().iter() {
            if !sendok!(to.as_ref(), to_sleeping)
                || (ch.is_some() && Rc::ptr_eq(to, ch.as_ref().unwrap()))
            {
                continue;
            }
            if hide_invisible && ch.is_some() && !self.can_see(to.as_ref(), ch.as_ref().unwrap()) {
                continue;
            }
            if vict_obj.is_none() {
                continue;
            }
            let same_chr;
            if vict_obj.is_some() {
                let p = vict_obj.unwrap().downcast_ref::<Rc<CharData>>();
                if p.is_some() {
                    same_chr = Rc::ptr_eq(to, p.as_ref().unwrap());
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

            self.perform_act(str, Some(ch.as_ref().unwrap().borrow()), obj, vict_obj, to);
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
