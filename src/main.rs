/* ************************************************************************
*   File: comm.c                                        Part of CircleMUD *
*  Usage: Communication, socket handling, main(), central game loop       *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

mod ban;
mod class;
mod config;
mod constants;
mod db;
mod interpreter;
mod modify;
mod structs;
mod telnet;
mod util;

use crate::config::*;
use crate::constants::*;
use crate::db::*;
use crate::interpreter::nanny;
use crate::structs::ConState::ConPlaying;
use crate::structs::*;
use crate::telnet::{IAC, TELOPT_ECHO, WILL, WONT};
use env_logger::Env;
use log::{debug, error, info, warn};
use std::cell::RefCell;
use std::collections::LinkedList;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};
use std::{env, fs, process, thread};

pub const PAGE_LENGTH: i32 = 22;
pub const PAGE_WIDTH: i32 = 80;

pub struct DescriptorData<'a> {
    stream: TcpStream,
    // file descriptor for socket
    host: String,
    // hostname
    bad_pws: u8,
    /* number of bad pw attemps this login	*/
    idle_tics: u8,
    /* tics idle at password prompt		*/
    connected: ConState,
    // mode of 'connectedness'
    desc_num: u32,
    // unique num assigned to desc
    login_time: Instant,
    /* when the person connected		*/
    showstr_head: &'a str,
    /* for keeping track of an internal str	*/
    showstr_vector: Vec<&'a str>,
    /* for paging through texts		*/
    showstr_count: i32,
    /* number of pages to page through	*/
    showstr_page: i32,
    /* which page are we currently showing?	*/
    str: Option<&'a str>,
    /* for the modify-str system		*/
    // size_t max_str;	        /*		-			*/
    // long	mail_to;		/* name for mail system			*/
    has_prompt: bool,
    /* is the user at a prompt?             */
    inbuf: String,
    /* buffer for raw input		*/
    // char	last_input[MAX_INPUT_LENGTH]; /* the last input			*/
    // char small_outbuf[SMALL_BUFSIZE];  /* standard output buffer		*/
    history: Vec<String>,
    /* History of commands, for ! mostly.	*/
    // int	history_pos;		/* Circular array position.		*/
    output: Option<String>,
    // int  bufptr;			/* ptr to end of current output		*/
    // int	bufspace;		/* space left in the output buffer	*/
    // struct txt_block *large_outbuf; /* ptr to large buffer, if we need it */
    input: LinkedList<TxtBlock>,
    character: Option<CharData<'a>>,
    /* linked to char			*/
    // struct char_data *original;	/* original char if switched		*/
    // struct descriptor_data *snooping; /* Who is this char snooping	*/
    // struct descriptor_data *snoop_by; /* And who is snooping this char	*/
    // struct descriptor_data *next; /* link to next descriptor		*/
}

/* local globals */
pub struct MainGlobals<'a> {
    db: Option<Rc<RefCell<DB<'a>>>>,
    mother_desc: Option<Box<TcpListener>>,
    descriptor_list: Vec<Rc<RefCell<DescriptorData<'a>>>>,
    last_desc: u32,
    // struct txt_block *bufpool = 0;	/* pool of large output buffers */
    // int buf_largecount = 0;		/* # of large buffers which exist */
    // int buf_overflows = 0;		/* # of overflows of output */
    // int buf_switches = 0;		/* # of switches from small to large buf */
    circle_shutdown: bool,
    /* clean shutdown */
    // int circle_reboot = 0;		/* reboot the game after a shutdown */
    // int no_specials = 0;		/* Suppress ass. of special routines */
    // int max_players = 0;		/* max descriptors available */
    // int tics = 0;			/* for extern checkpointing */
    // struct timeval null_time;	/* zero-valued time structure */
    // byte reread_wizlist;		/* signal: SIGUSR1 */
    // byte emergency_unban;		/* signal: SIGUSR2 */
    /* Where to send the log messages. */
    // const char *text_overflow = "**OVERFLOW**\r\n";
}

/***********************************************************************
*  main game loop and related stuff                                    *
***********************************************************************/

fn main() -> ExitCode {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let dir = DFLT_DIR;
    let port = DFLT_PORT;

    let game = Rc::new(RefCell::new(MainGlobals {
        descriptor_list: Vec::new(),
        last_desc: 0,
        circle_shutdown: false,
        db: None,
        mother_desc: None,
    }));
    //let mut scheck: bool = false; /* for syntax checking mode */
    // ush_int port;
    // int pos = 1;

    // dir = DFLT_DIR;
    //
    // while ((pos < argc) && (*(argv[pos]) == '-')) {
    // switch (*(argv[pos] + 1)) {
    // case 'o':
    // if (*(argv[pos] + 2))
    // LOGNAME = argv[pos] + 2;
    // else if (++pos < argc)
    // LOGNAME = argv[pos];
    // else {
    // puts("SYSERR: File name to log to expected after option -o.");
    // exit(1);
    // }
    // break;
    // case 'd':
    // if (*(argv[pos] + 2))
    // dir = argv[pos] + 2;
    // else if (++pos < argc)
    // dir = argv[pos];
    // else {
    // puts("SYSERR: Directory arg expected after option -d.");
    // exit(1);
    // }
    // break;
    // case 'm':
    // mini_mud = 1;
    // no_rent_check = 1;
    // puts("Running in minimized mode & with no rent check.");
    // break;
    // case 'c':
    // scheck = 1;
    // puts("Syntax check mode enabled.");
    // break;
    // case 'q':
    // no_rent_check = 1;
    // puts("Quick boot mode -- rent check supressed.");
    // break;
    // case 'r':
    // circle_restrict = 1;
    // puts("Restricting game -- no new players allowed.");
    // break;
    // case 's':
    // no_specials = 1;
    // puts("Suppressing assignment of special routines.");
    // break;
    // case 'h':
    // /* From: Anil Mahajan <amahajan@proxicom.com> */
    // printf("Usage: %s [-c] [-m] [-q] [-r] [-s] [-d pathname] [port #]\n"
    // "  -c             Enable syntax check mode.\n"
    // "  -d <directory> Specify library directory (defaults to 'lib').\n"
    // "  -h             Print this command line argument help.\n"
    // "  -m             Start in mini-MUD mode.\n"
    // "  -o <file>      Write log to <file> instead of stderr.\n"
    // "  -q             Quick boot (doesn't scan rent for object limits)\n"
    // "  -r             Restrict MUD -- no new players allowed.\n"
    // "  -s             Suppress special procedure assignments.\n",
    // argv[0]
    // );
    // exit(0);
    // default:
    // printf("SYSERR: Unknown option -%c in argument string.\n", *(argv[pos] + 1));
    // break;
    // }
    // pos++;
    // }
    //
    // if (pos < argc) {
    // if (!isdigit(*argv[pos])) {
    // printf("Usage: %s [-c] [-m] [-q] [-r] [-s] [-d pathname] [port #]\n", argv[0]);
    // exit(1);
    // } else if ((port = atoi(argv[pos])) <= 1024) {
    // printf("SYSERR: Illegal port number %d.\n", port);
    // exit(1);
    // }
    // }

    /* All arguments have been parsed, try to open log file. */
    // setup_log(LOGNAME);

    /*
     * Moved here to distinguish command line options and to show up
     * in the log if stderr is redirected to a file.
     */
    info!("{}", CIRCLEMUD_VERSION);

    env::set_current_dir(Path::new(dir)).unwrap_or_else(|error| {
        eprint!(
            "SYSERR: Fatal error changing to data directory {}/{}: {}",
            env::current_dir().unwrap().display(),
            dir,
            error
        );
        process::exit(1);
    });

    info!("Using {} as data directory.", dir);

    // if scheck {
    //     boot_world();
    // } else {
    info!("Running game on port {}.", port);
    RefCell::borrow_mut(&game).mother_desc = Some(init_socket(port));
    init_game(game, port);
    // }

    info!("Clearing game world.");
    // destroy_db();

    // if !scheck {
    //     log("Clearing other memory.");
    //     free_player_index();    /* db.c */
    //     free_messages();        /* fight.c */
    //     clear_free_list();        /* mail.c */
    //     free_text_files();        /* db.c */
    //     Board_clear_all();        /* boards.c */
    //     free(cmd_sort_info);    /* act.informative.c */
    //     free_social_messages();    /* act.social.c */
    //     free_help();        /* db.c */
    //     Free_Invalid_List();    /* ban.c */
    // }

    info!("Done.");
    ExitCode::SUCCESS
}

/* Init sockets, run game, and cleanup sockets */
fn init_game(globals: Rc<RefCell<MainGlobals>>, _port: u16) {
    //socket_t mother_desc;

    /* We don't want to restart if we crash before we get up. */
    util::touch(Path::new(KILLSCRIPT)).expect("Cannot create KILLSCRIPT path");

    // log("Finding player limit.");
    // max_players = get_max_players();

    info!("Opening mother connection.");

    RefCell::borrow_mut(&globals).db = Some(Rc::from(RefCell::new(boot_db(Rc::clone(&globals)))));

    // info!("Signal trapping.");
    // signal_setup();

    /* If we made it this far, we will be able to restart without problem. */
    fs::remove_file(Path::new(KILLSCRIPT)).unwrap();

    info!("Entering game loop.");

    game_loop(globals);

    //Crash_save_all();

    info!("Closing all sockets.");
    // DESCRIPTOR_LIST.iter_mut().for_each(|descriptor_data| {
    //     close_socket(descriptor_data);
    // });
    //
    // CLOSE_SOCKET(mother_desc);
    // fclose(player_fl);

    info!("Saving current MUD time.");
    // save_mud_time(&time_info);

    // if (circle_reboot) {
    //     log("Rebooting.");
    //     exit(52);            /* what's so great about HHGTTG, anyhow? */
    // }
    info!("Normal termination of game.");
}

/*
 * init_socket sets up the mother descriptor - creates the socket, sets
 * its options up, binds it, and listens.
 */
fn init_socket(port: u16) -> Box<TcpListener> {
    let socket_addr = SocketAddr::new(("127.0.0.1".parse()).unwrap(), port);
    let listener = TcpListener::bind(socket_addr).unwrap_or_else(|error| {
        error!("SYSERR: Error creating socket {}", error);
        process::exit(1);
    });
    listener
        .set_nonblocking(true)
        .expect("Non blocking has issue");
    Box::new(listener)
    //
    // if ((s = socket(PF_INET, SOCK_STREAM, 0)) < 0) {
    // perror("");
    // exit(1);
    // }
    //
    // # if defined(SO_REUSEADDR) & & ! defined(CIRCLE_MACINTOSH)
    // opt = 1;
    // if (setsockopt(s, SOL_SOCKET, SO_REUSEADDR, (char * ) & opt, sizeof(opt)) < 0){
    // perror("SYSERR: setsockopt REUSEADDR");
    // exit(1);
    // }
    // # endif
    //
    // set_sendbuf(s);
    //
    // /*
    //  * The GUSI sockets library is derived from BSD, so it defines
    //  * SO_LINGER, even though setsockopt() is unimplimented.
    //  *	(from Dean Takemori <dean@UHHEPH.PHYS.HAWAII.EDU>)
    //  */
    // # if defined(SO_LINGER) & & ! defined(CIRCLE_MACINTOSH)
    // {
    // struct linger ld;
    //
    // ld.l_onoff = 0;
    // ld.l_linger = 0;
    // if (setsockopt(s, SOL_SOCKET, SO_LINGER, (char * ) & ld, sizeof(ld)) < 0)
    // perror("SYSERR: setsockopt SO_LINGER");    /* Not fatal I suppose. */
    // }
    // # endif
    //
    // /* Clear the structure */
    // memset((char * ) & sa, 0, sizeof(sa));
    //
    // sa.sin_family = AF_INET;
    // sa.sin_port = htons(port);
    // sa.sin_addr = * (get_bind_addr());
    //
    // if (bind(s, ( struct sockaddr * ) & sa, sizeof(sa)) < 0) {
    // perror("SYSERR: bind");
    // CLOSE_SOCKET(s);
    // exit(1);
    // }
    // nonblock(s);
    // listen(s, 5);
    // return (s);
}

// int get_max_players(void)
// {
// # ifndef CIRCLE_UNIX
// return (max_playing);
// # else
//
// int max_descs = 0;
// const char * method;
//
// /*
//  * First, we'll try using getrlimit/setrlimit.  This will probably work
//  * on most systems.  HAS_RLIMIT is defined in sysdep.h.
//  */
// # ifdef HAS_RLIMIT
// {
// struct rlimit limit;
//
// /* find the limit of file descs */
// method = "rlimit";
// if (getrlimit(RLIMIT_NOFILE, & limit) < 0) {
// perror("SYSERR: calling getrlimit");
// exit(1);
// }
//
// /* set the current to the maximum */
// limit.rlim_cur = limit.rlim_max;
// if (setrlimit(RLIMIT_NOFILE, & limit) < 0) {
// perror("SYSERR: calling setrlimit");
// exit(1);
// }
// # ifdef RLIM_INFINITY
// if (limit.rlim_max == RLIM_INFINITY)
// max_descs = max_playing + NUM_RESERVED_DESCS;
// else
// max_descs = MIN(max_playing + NUM_RESERVED_DESCS, limit.rlim_max);
// # else
// max_descs = MIN(max_playing + NUM_RESERVED_DESCS, limit.rlim_max);
// # endif
// }
//
// # elif defined (OPEN_MAX) | | defined(FOPEN_MAX)
// # if ! defined(OPEN_MAX)
// #define OPEN_MAX FOPEN_MAX
// # endif
// method = "OPEN_MAX";
// max_descs = OPEN_MAX; /* Uh oh.. rlimit didn't work, but we have
// 				 * OPEN_MAX */
// # elif defined (_SC_OPEN_MAX)
// /*
//  * Okay, you don't have getrlimit() and you don't have OPEN_MAX.  Time to
//  * try the POSIX sysconf() function.  (See Stevens' _Advanced Programming
//  * in the UNIX Environment_).
//  */
// method = "POSIX sysconf";
// errno = 0;
// if ((max_descs = sysconf(_SC_OPEN_MAX)) < 0) {
// if (errno == 0)
// max_descs = max_playing + NUM_RESERVED_DESCS;
// else {
// perror("SYSERR: Error calling sysconf");
// exit(1);
// }
// }
// # else
// /* if everything has failed, we'll just take a guess */
// method = "random guess";
// max_descs = max_playing + NUM_RESERVED_DESCS;
// # endif
//
// /* now calculate max _players_ based on max descs */
// max_descs = MIN(max_playing, max_descs - NUM_RESERVED_DESCS);
//
// if (max_descs < = 0) {
// log("SYSERR: Non-positive max player limit!  (Set at %d using %s).",
// max_descs, method);
// exit(1);
// }
// log("   Setting player limit to %d using %s.", max_descs, method);
// return (max_descs);
// # endif /* CIRCLE_UNIX */
// }
//

/*
 * game_loop contains the main loop which drives the entire MUD.  It
 * cycles once every 0.10 seconds and is responsible for accepting new
 * new connections, polling existing connections for input, dequeueing
 * output and sending it out to players, and calling "heartbeat" functions
 * such as mobile_activity().
 */
fn game_loop(main_globals: Rc<RefCell<MainGlobals>>) {
    let opt_time = Duration::from_micros(OPT_USEC as u64);
    let mut process_time;
    let mut temp_time;
    let mut before_sleep;
    // let mut now;
    let mut timeout;
    let mut comm = String::new();
    // struct descriptor_data * d, * next_d;
    let mut pulse: u32 = 0;
    let mut missed_pulses;
    //        let mut maxdesc;
    let mut aliased = false;

    /* initialize various time values */
    // null_time.tv_sec = 0;
    // null_time.tv_usec = 0;
    // FD_ZERO( & null_set);

    let mut last_time = Instant::now();

    /* The Main Loop.  The Big Cheese.  The Top Dog.  The Head Honcho.  The.. */
    while !RefCell::borrow(&main_globals).circle_shutdown {
        /* Sleep if we don't have any connections */
        if RefCell::borrow_mut(&main_globals)
            .descriptor_list
            .is_empty()
        {
            debug!("No connections.  Going to sleep.");
            // match listener.accept() {
            //     Ok((_socket, addr)) => {
            //         log("New connection.  Waking up.");
            //         last_time = Local::now();
            //     },
            //     Err(e) => { log("SYSERR: Could not get client {e:?}") }
            // }
            // FD_ZERO(&input_set);
            // FD_SET(mother_desc, &input_set);
            // if (select(mother_desc + 1, &input_set, (fd_set *) 0, (fd_set *) 0, NULL) < 0) {
            //     if (errno == EINTR)
            //     log("Waking up to process signal.");
            //     else
            //     perror("SYSERR: Select coma");
            // } else
            // log("New connection.  Waking up.");
            // last_time = Local::now();
            // gettimeofday(&last_time, ( struct timezone
            // * ) 0);
        }
        /* Set up the input, output, and exception sets for select(). */
        // FD_ZERO(&input_set);
        // FD_ZERO(&output_set);
        // FD_ZERO(&exc_set);
        // FD_SET(mother_desc, &input_set);
        //
        // maxdesc = mother_desc;
        // for (d = descriptor_list; d; d = d -> next) {
        //     # ifndef
        //     CIRCLE_WINDOWS
        //     if (d -> descriptor > maxdesc)
        //     maxdesc = d -> descriptor;
        //     # endif
        //     FD_SET(d -> descriptor, &input_set);
        //     FD_SET(d -> descriptor, &output_set);
        //     FD_SET(d -> descriptor, &exc_set);
        // }

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
        //missed_pulses;
        if (process_time.as_micros() as u32) < OPT_USEC {
            missed_pulses = 0;
        } else {
            missed_pulses = process_time.as_micros() as u32 / OPT_USEC;
            process_time = process_time
                + Duration::new(0, 1000 * (process_time.as_micros() as u32 % OPT_USEC));
        }

        /* Calculate the time we should wake up */
        temp_time = opt_time - process_time;
        last_time = before_sleep + temp_time;

        /* Now keep sleeping until that time has come */
        timeout = last_time - Instant::now();

        thread::sleep(timeout);

        /* If there are new connections waiting, accept them. */
        let accept_result = RefCell::borrow(&main_globals)
            .mother_desc
            .as_ref()
            .unwrap()
            .accept();
        match accept_result {
            Ok((socket, addr)) => {
                info!("New connection {}.  Waking up.", addr);
                new_descriptor(&main_globals, socket, addr);
            }
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => (),
                _ => error!("SYSERR: Could not get client {e:?}"),
            },
        }

        /* Process descriptors with input pending */
        for d in &RefCell::borrow(&main_globals).descriptor_list {
            let mut buf = [0 as u8];
            let res = RefCell::borrow(d).stream.peek(&mut buf);
            if res.is_ok() && res.unwrap() != 0 {
                process_input(d);
            }
        }

        let my_main_globals = RefCell::borrow(&main_globals);
        /* Process commands we just read from process_input */
        for d in &my_main_globals.descriptor_list {
            /*
             * Not combined to retain --(d->wait) behavior. -gg 2/20/98
             * If no wait state, no subtraction.  If there is a wait
             * state then 1 is subtracted. Therefore we don't go less
             * than 0 ever and don't require an 'if' bracket. -gg 2/27/99
             */
            if RefCell::borrow(d).character.is_some() {
                let wait_state =
                    get_wait_state!(RefCell::borrow_mut(d).character.as_mut().unwrap());
                if wait_state > 0 {
                    get_wait_state!(RefCell::borrow_mut(d).character.as_mut().unwrap()) -= 1;
                }

                if get_wait_state!(RefCell::borrow_mut(d).character.as_mut().unwrap()) != 0 {
                    continue;
                }
            }

            if !get_from_q(&mut RefCell::borrow_mut(d).input, &mut comm, &mut aliased) {
                continue;
            }

            if RefCell::borrow(d).character.is_some() {
                /* Reset the idle timer & pull char back from void if necessary */
                RefCell::borrow_mut(d)
                    .character
                    .as_mut()
                    .unwrap()
                    .char_specials
                    .timer = 0;
                if state!(RefCell::borrow_mut(d)) == ConPlaying
                /* && GET_WAS_IN(d -> character) != NOWHERE */
                {
                    // if (IN_ROOM(d -> character) != NOWHERE)
                    // char_from_room(d -> character);
                    // char_to_room(d -> character, GET_WAS_IN(d -> character));
                    // GET_WAS_IN(d -> character) = NOWHERE;
                    // act("$n has returned.", TRUE, d -> character, 0, 0, TO_ROOM);
                }
                get_wait_state!(RefCell::borrow_mut(d).character.as_mut().unwrap()) = 1;
            }
            RefCell::borrow_mut(d).has_prompt = false;

            // if RefCell::borrow(d).str.is_some() {
            //     /* Writing boards, mail, etc. */
            //     string_add(d, comm);
            // }
            // else
            // if  RefCell::borrow(d).showstr_count {
            //     /* Reading something w/ pager */
            //     show_string(d, comm);
            // } else
            if state!(RefCell::borrow(d)) != ConPlaying {
                /* In menus, etc. */
                nanny(main_globals.clone(), d.clone(), comm.as_str());
            } else {
                /* else: we're playing normally. */
                // if (aliased)        /* To prevent recursive aliases. */
                // d -> has_prompt = TRUE;    /* To get newline before next cmd output. */
                // else if (perform_alias(d, comm, sizeof(comm)))    /* Run it through aliasing system */
                // get_from_q(&d ->input, comm, &aliased);
                // command_interpreter(d -> character, comm); /* Send it to interpreter */
            }
        }

        /* Send queued output out to the operating system (ultimately to user). */
        for d in &RefCell::borrow(&main_globals).descriptor_list {
            if RefCell::borrow(d).output.is_some() {
                process_output(d);
                if RefCell::borrow(d).output.is_some()
                    && RefCell::borrow(d).output.as_ref().unwrap().len() != 0
                {
                    RefCell::borrow_mut(d).has_prompt = true;
                }
            }
        }

        /* Print prompts for other descriptors who had no other output */
        for d in &RefCell::borrow(&main_globals).descriptor_list {
            let mut mut_d = RefCell::borrow_mut(d);
            if !mut_d.has_prompt
                && (mut_d.output.is_none() || mut_d.output.as_ref().unwrap().len() != 0)
            {
                let text = &make_prompt(&mut mut_d);
                write_to_descriptor(&mut mut_d.stream, text);
                mut_d.has_prompt = true;
            }
        }

        /* Kick out folks in the ConClose or ConDisconnect state */
        // for (d = descriptor_list; d; d = next_d) {
        //     next_d = d -> next;
        //     if (STATE(d) == ConClose | | STATE(d) == ConDisconnect)
        //     close_socket(d);
        // }

        /*
         * Now, we execute as many pulses as necessary--just one if we haven't
         * missed any pulses, or make up for lost time if we missed a few
         * pulses by sleeping for too long.
         */
        let mut missed_pulses = missed_pulses + 1;

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
            heartbeat(pulse);
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

fn heartbeat(_pulse: u32) {
    // static int
    // mins_since_crashsave = 0;
    //
    // if (!(pulse % PULSE_ZONE))
    // zone_update();
    //
    // if (!(pulse % PULSE_IDLEPWD))        /* 15 seconds */
    // check_idle_passwords();
    //
    // if (!(pulse % PULSE_MOBILE))
    // mobile_activity();
    //
    // if (!(pulse % PULSE_VIOLENCE))
    // perform_violence();
    //
    // if (!(pulse % (SECS_PER_MUD_HOUR * PASSES_PER_SEC))) {
    //     weather_and_time(1);
    //     affect_update();
    //     point_update();
    //     fflush(player_fl);
    // }
    //
    // if (auto_save & &!(pulse % PULSE_AUTOSAVE)) {
    //     /* 1 minute */
    //     if ( + + mins_since_crashsave > = autosave_time) {
    //         mins_since_crashsave = 0;
    //         Crash_save_all();
    //         House_save_all();
    //     }
    // }
    //
    // if (!(pulse % PULSE_USAGE))
    // record_usage();
    //
    // if (!(pulse % PULSE_TIMESAVE))
    // save_mud_time(&time_info);
    //
    // /* Every pulse! Don't want them to stink the place up... */
    // extract_pending_chars();
}

/* ******************************************************************
*  general utility stuff (for local use)                            *
****************************************************************** */

/*
 *  new code to calculate time differences, which works on systems
 *  for which tv_usec is unsigned (and thus comparisons for something
 *  being < 0 fail).  Based on code submitted by ss@sirocco.cup.hp.com.
 */
//
// void record_usage(void)
// {
// int sockets_connected = 0, sockets_playing = 0;
// struct descriptor_data * d;
//
// for (d = descriptor_list; d; d = d -> next) {
// sockets_connected + +;
// if (STATE(d) == ConPlaying)
// sockets_playing + +;
// }
//
// log("nusage: %-3d sockets connected, %-3d sockets playing",
// sockets_connected, sockets_playing);
//
// #ifdef RUSAGE    /* Not RUSAGE_SELF because it doesn't guarantee prototype. */
// {
// struct rusage ru;
//
// getrusage(RUSAGE_SELF, & ru);
// log("rusage: user time: %ld sec, system time: %ld sec, max res size: %ld",
// ru.ru_utime.tv_sec, ru.ru_stime.tv_sec, ru.ru_maxrss);
// }
// # endif
//
// }

/*
 * Turn off echoing (specific to telnet client)
 */
fn echo_off(d: &mut DescriptorData) {
    let mut off_string = "".to_string();
    off_string.push(char::from(IAC));
    off_string.push(char::from(WILL));
    off_string.push(char::from(TELOPT_ECHO));
    off_string.push(char::from(0));

    write_to_output(d, off_string.as_str());
}

/*
 * Turn on echoing (specific to telnet client)
 */
fn echo_on(d: &mut DescriptorData) {
    let mut off_string = "".to_string();
    off_string.push(char::from(IAC));
    off_string.push(char::from(WONT));
    off_string.push(char::from(TELOPT_ECHO));
    off_string.push(char::from(0));

    write_to_output(d, off_string.as_str());
}

fn make_prompt(d: &DescriptorData) -> String {
    let mut prompt = "".to_string();
    let mut_d = d;

    /* Note, prompt is truncated at MAX_PROMPT_LENGTH chars (structs.h) */

    if mut_d.str.is_some() {
        prompt.push_str("] "); /* strcpy: OK (for 'MAX_PROMPT_LENGTH >= 3') */
    } else if mut_d.showstr_count != 0 {
        prompt.push_str(&*format!(
            "\r\n[ Return to continue, (q)uit, (r)efresh, (b)ack, or page number ({}/{}) ]",
            mut_d.showstr_page, mut_d.showstr_count
        ));
    } else if mut_d.connected == ConPlaying {
        let character = mut_d.character.as_ref().unwrap();
        if !is_npc!(character) {
            if get_invis_lev!(character) != 0 && prompt.len() < MAX_PROMPT_LENGTH as usize {
                let il = get_invis_lev!(character);
                prompt.push_str(&*format!("i{} ", il));
            }

            if prf_flagged!(character, PRF_DISPHP) && prompt.len() < MAX_PROMPT_LENGTH as usize {
                let hit = get_hit!(character);
                prompt.push_str(&*format!("{}H ", hit));
            }

            if prf_flagged!(character, PRF_DISPMANA) && prompt.len() < MAX_PROMPT_LENGTH as usize {
                let mana = get_mana!(character);
                prompt.push_str(&*format!("{}M ", mana));
            }

            if prf_flagged!(character, PRF_DISPMOVE) && prompt.len() < MAX_PROMPT_LENGTH as usize {
                let _move = get_move!(character);
                prompt.push_str(&*format!("{}V ", _move));
            }

            prompt.push_str("> ");
        } else if is_npc!(character) {
            let borrowed_d = mut_d;
            prompt.push_str(&*format!("{}s>", get_name!(character)));
        }
    }

    prompt
}

/*
 * NOTE: 'txt' must be at most MAX_INPUT_LENGTH big.
 */
fn write_to_q(txt: &str, queue: &mut LinkedList<TxtBlock>, aliased: bool) {
    let newt = TxtBlock {
        text: String::from(txt),
        aliased,
    };

    queue.push_back(newt);
}

/*
 * NOTE: 'dest' must be at least MAX_INPUT_LENGTH big.
 */
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
// void flush_queues(struct descriptor_data * d)
// {
// if (d -> large_outbuf) {
// d -> large_outbuf -> next = bufpool;
// bufpool = d -> large_outbuf;
// }
// while (d -> input.head) {
// struct txt_block * tmp = d -> input.head;
// d -> input.head = d -> input.head -> next;
// free(tmp -> text);
// free(tmp);
// }
// }

/* Add a new string to a player's output queue. */
fn write_to_output(t: &mut DescriptorData, txt: &str) {
    // static char txt[MAX_STRING_LENGTH];
    // size_t wantsize;
    // int size;

    /* if we're in the overflow state already, ignore this new output */
    // if (t -> bufspace == 0)
    // return (0);

    // wantsize = size = vsnprintf(txt, sizeof(txt), format, args);
    /* If exceeding the size of the buffer, truncate it for the overflow message */
    // if (size < 0 || wantsize > = sizeof(txt)) {
    // size = sizeof(txt) - 1;
    // strcpy(txt + size - strlen(text_overflow), text_overflow);    /* strcpy: OK */
    // }

    /*
     * If the text is too big to fit into even a large buffer, truncate
     * the new text to make it fit.  (This will switch to the overflow
     * state automatically because t->bufspace will end up 0.)
     */
    // if (size + t -> bufptr + 1 > LARGE_BUFSIZE) {
    // size = LARGE_BUFSIZE - t -> bufptr - 1;
    // txt[size] = '\0';
    // buf_overflows + +;
    // }

    /*
     * If we have enough space, just write to buffer and that's it! If the
     * text just barely fits, then it's switched to a large buffer instead.
     */
    // if (t -> bufspace > size) {
    // strcpy(t -> output + t -> bufptr, txt); /* strcpy: OK (size checked above) */
    // t -> bufspace -= size;
    // t-> bufptr += size;
    // return (t -> bufspace);
    // }

    // buf_switches + +;

    /* if the pool has a buffer in it, grab it */
    // if (bufpool != NULL) {
    // t -> large_outbuf = bufpool;
    // bufpool = bufpool ->next;
    // } else {            /* else create a new one */
    // CREATE(t -> large_outbuf, struct txt_block, 1);
    // CREATE(t -> large_outbuf -> text, char, LARGE_BUFSIZE);
    // buf_largecount + +;
    // }

    // strcpy(t -> large_outbuf -> text, t ->output); /* strcpy: OK (size checked previously) */
    // t -> output = t -> large_outbuf-> text; /* make big buffer primary */
    // strcat(t -> output, txt); /* strcat: OK (size checked) */
    // /* set the pointer for the next write */
    // t -> bufptr = strlen(t ->output);
    //
    // /* calculate how much space is left in the buffer */
    // t -> bufspace = LARGE_BUFSIZE - 1 - t -> bufptr;
    //
    // return (t -> bufspace);
    if t.output.is_none() {
        t.output = Some(txt.to_string());
    } else {
        t.output.as_mut().unwrap().push_str(txt);
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

// struct in_addr * get_bind_addr()
// {
// static struct in_addr bind_addr;
//
// /* Clear the structure */
// memset((char *) & bind_addr, 0, sizeof(bind_addr));
//
// /* If DLFT_IP is unspecified, use INADDR_ANY */
// if (DFLT_IP == NULL) {
// bind_addr.s_addr = htonl(INADDR_ANY);
// } else {
// /* If the parsing fails, use INADDR_ANY */
// if ( ! parse_ip(DFLT_IP, & bind_addr)) {
// log("SYSERR: DFLT_IP of %s appears to be an invalid IP address", DFLT_IP);
// bind_addr.s_addr = htonl(INADDR_ANY);
// }
// }
//
// /* Put the address that we've finally decided on into the logs */
// if (bind_addr.s_addr == htonl(INADDR_ANY))
// log("Binding to all IP interfaces on this host.");
// else
// log("Binding only to IP address %s", inet_ntoa(bind_addr));
//
// return ( &bind_addr);
// }

/* Sets the kernel's send buffer size for the descriptor */
// int set_sendbuf(socket_t s)
// {
// # if defined(SO_SNDBUF) & & ! defined(CIRCLE_MACINTOSH)
// int opt = MAX_SOCK_BUF;
//
// if (setsockopt(s, SOL_SOCKET, SO_SNDBUF, (char * ) & opt, sizeof(opt)) < 0) {
// perror("SYSERR: setsockopt SNDBUF");
// return ( - 1);
// }
//
// # if 0
// if (setsockopt(s, SOL_SOCKET, SO_RCVBUF, (char * ) & opt, sizeof(opt)) < 0) {
// perror("SYSERR: setsockopt RCVBUF");
// return ( - 1);
// }
// # endif
//
// # endif
//
// return (0);
// }

fn new_descriptor(main_globals: &Rc<RefCell<MainGlobals>>, socket: TcpStream, addr: SocketAddr) {
    // socket_t
    // desc;
    // sockets_connected = 0;
    // socklen_t
    // i;
    // static last_desc = 0; /* last descriptor number */
    // struct descriptor_data *newd;
    // struct sockaddr_in peer;
    // struct hostent * from;

    /* accept the new connection */
    //     i = sizeof(peer);
    // if ((desc = accept(s, ( struct sockaddr * ) & peer, & i)) == INVALID_SOCKET) {
    // perror("SYSERR: accept");
    // return ( - 1);
    // }
    /* keep it from blocking */
    socket
        .set_nonblocking(true)
        .expect("Error with setting nonblocking");
    /* set the send buffer size */
    // if (set_sendbuf(desc) < 0) {
    // CLOSE_SOCKET(desc);
    // return (0);
    // }

    /* make sure we have room for it */
    // for (newd = descriptor_list; newd; newd = newd -> next)
    // sockets_connected + +;

    // if (sockets_connected > = max_players) {
    // write_to_descriptor(desc, "Sorry, CircleMUD is full right now... please try again later!\r\n");
    // CLOSE_SOCKET(desc);
    // return (0);
    // }
    /* create a new descriptor */
    let newd = Rc::new(RefCell::new(DescriptorData {
        stream: socket,
        host: "".parse().unwrap(),
        bad_pws: 0,
        idle_tics: 0,
        connected: ConState::ConGetName,
        desc_num: RefCell::borrow(main_globals).last_desc,
        login_time: Instant::now(),
        showstr_head: "",
        showstr_vector: vec![],
        showstr_count: 0,
        showstr_page: 0,
        str: None,
        has_prompt: false,
        inbuf: String::new(),
        history: vec![],
        output: None,
        input: LinkedList::new(),
        character: None,
    }));

    /* find the sitename */
    if !NAMESERVER_IS_SLOW {
        let r = dns_lookup::lookup_addr(&addr.ip());
        if r.is_err() {
            error!("Error resolving address: {}", r.err().unwrap());
            RefCell::borrow_mut(&newd).host = addr.ip().to_string();
        } else {
            RefCell::borrow_mut(&newd).host = r.unwrap();
        }
    } else {
        RefCell::borrow_mut(&newd).host = addr.ip().to_string();
    }

    /* determine if the site is banned */
    // if (isbanned(newd -> host) == BAN_ALL) {
    //     CLOSE_SOCKET(desc);
    //     mudlog(CMP, LVL_GOD, TRUE, "Connection attempt denied from [%s]", newd -> host);
    //     free(newd);
    //     return (0);
    // }

    /* initialize descriptor data */
    //newd -> descriptor = desc;
    RefCell::borrow_mut(&newd).idle_tics = 0;
    //newd -> output = newd -> small_outbuf;
    //newd -> bufspace = SMALL_BUFSIZE - 1;
    RefCell::borrow_mut(&newd).login_time = Instant::now();
    //*newd -> output = '\0';
    //newd -> bufptr = 0;
    RefCell::borrow_mut(&newd).has_prompt = true; /* prompt is part of greetings */
    RefCell::borrow_mut(&newd).connected = ConState::ConGetName;

    /*
     * This isn't exactly optimal but allows us to make a design choice.
     * Do we embed the history in descriptor_data or keep it dynamically
     * allocated and allow a user defined history size?
     */
    //CREATE(newd -> history, char *, HISTORY_SIZE);
    RefCell::borrow_mut(&newd).history = Vec::new();
    RefCell::borrow_mut(main_globals).last_desc += 1;
    if RefCell::borrow_mut(main_globals).last_desc == 1000 {
        RefCell::borrow_mut(main_globals).last_desc = 1;
    }
    RefCell::borrow_mut(&newd).desc_num = RefCell::borrow_mut(main_globals).last_desc;

    /* prepend to list */
    // newd -> next = descriptor_list;
    // descriptor_list = newd;
    RefCell::borrow_mut(main_globals)
        .descriptor_list
        .push(newd.clone());

    write_to_output(
        &mut RefCell::borrow_mut(&newd),
        format!(
            "{}",
            RefCell::borrow(RefCell::borrow(main_globals).db.as_ref().unwrap())
                .greetings
                .borrow()
        )
        .as_str(),
    );
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
fn process_output(t: &Rc<RefCell<DescriptorData>>) -> i32 {
    //char i[MAX_SOCK_BUF], * osb = i + 2;

    let mut result = 0;

    /* we may need this \r\n for later -- see below */
    let mut i = "\r\n".to_string();
    //strcpy(i, "\r\n"); /* strcpy: OK (for 'MAX_SOCK_BUF >= 3') */
    /* now, append the 'real' output */
    i.push_str(&RefCell::borrow(t).output.as_ref().unwrap());

    /* if we're in the overflow state, notify the user */
    // if (t -> bufspace == 0)
    // strcat(osb, "**OVERFLOW**\r\n"); /* strcpy: OK (osb:MAX_SOCK_BUF-2 reserves space) */
    /* add the extra CRLF if the person isn't in compact mode */
    if RefCell::borrow(t).connected == ConPlaying
        && RefCell::borrow(t).character.is_some()
        && !is_npc!(RefCell::borrow(t).character.as_ref().unwrap())
        && prf_flagged!(RefCell::borrow(t).character.as_ref().unwrap(), PRF_COMPACT)
    {
        i.push_str("\r\n");
    }

    /* add a prompt */
    i.push_str(&make_prompt(&mut RefCell::borrow_mut(t)));

    /*
     * now, send the output.  If this is an 'interruption', use the prepended
     * CRLF, otherwise send the straight output sans CRLF.
     */
    if RefCell::borrow(&t).has_prompt {
        RefCell::borrow_mut(&t).has_prompt = false;
        result = write_to_descriptor(&mut RefCell::borrow_mut(&t).stream, &i);
        if result >= 2 {
            result -= 2;
        }
    } else {
        result = write_to_descriptor(&mut RefCell::borrow_mut(&t).stream, &i[2..]);
    }

    if result < 0 {
        /* Oops, fatal error. Bye! */
        RefCell::borrow(&t)
            .stream
            .shutdown(Shutdown::Both)
            .expect("SYSERR: cannot close socket");
        return -1;
    } else if result == 0 {
        /* Socket buffer full. Try later. */
        return 0;
    }

    /* Handle snooping: prepend "% " and send to snooper. */
    // if (RefCell::borrow(&t).snoop_by) {
    //     write_to_output(RefCell::borrow(&t).snoop_by, "%% %*s%%%%", result, t ->output);
    // }

    // /* The common case: all saved output was handed off to the kernel buffer. */
    // if (result > = t ->bufptr) {
    // /*
    //  * if we were using a large buffer, put the large buffer on the buffer pool
    //  * and switch back to the small one
    //  */
    // if (t -> large_outbuf) {
    // t -> large_outbuf -> next = bufpool;
    // bufpool = t -> large_outbuf;
    // t -> large_outbuf = NULL;
    // t -> output = t -> small_outbuf;
    // }
    // /* reset total bufspace back to that of a small buffer */
    // t -> bufspace = SMALL_BUFSIZE - 1;
    // t ->bufptr = 0;
    // * (t -> output) = '\0';

    RefCell::borrow_mut(&t).output = Some("".to_string());
    /*
     * If the overflow message or prompt were partially written, try to save
     * them. There will be enough space for them if this is true.  'result'
     * is effectively unsigned here anyway.
     */
    // if ((unsigned int)result < strlen(osb)) {
    // size_t savetextlen = strlen(osb + result);
    //
    // strcat(t -> output, osb + result);
    // t -> bufptr -= savetextlen;
    // t -> bufspace += savetextlen;
    // }
    //}
    // else {
    // /* Not all data in buffer sent.  result < output buffersize. */
    //
    // strcpy(t -> output, t -> output + result); /* strcpy: OK (overlap) */
    // t -> bufptr -= result;
    // t-> bufspace += result;
    // }
    result
}

/*
 * perform_socket_write: takes a descriptor, a pointer to text, and a
 * text length, and tries once to send that text to the OS.  This is
 * where we stuff all the platform-dependent stuff that used to be
 * ugly #ifdef's in write_to_descriptor().
 *
 * This function must return:
 *
 * -1  If a fatal error was encountered in writing to the descriptor.
 *  0  If a transient failure was encountered (e.g. socket buffer full).
 * >0  To indicate the number of bytes successfully written, possibly
 *     fewer than the number the caller requested be written.
 *
 * Right now there are two versions of this function: one for Windows,
 * and one for all other platforms.
 */

// # if defined(CIRCLE_WINDOWS)
//
// ssize_t perform_socket_write(socket_t desc, const char * txt, size_t length)
// {
// ssize_t result;
//
// result = send(desc, txt, length, 0);
//
// if (result > 0) {
// /* Write was sucessful */
// return (result);
// }
//
// if (result == 0) {
// /* This should never happen! */
// log("SYSERR: Huh??  write() returned 0???  Please report this!");
// return ( - 1);
// }
//
// /* result < 0: An error was encountered. */
//
// /* Transient error? */
// if (WSAGetLastError() == WSAEWOULDBLOCK | | WSAGetLastError() == WSAEINTR)
// return (0);
//
// /* Must be a fatal error. */
// return ( - 1);
// }
//
// # else
//
// # if defined(CIRCLE_ACORN)
// # define write    socketwrite
// # endif
//
// /* perform_socket_write for all Non-Windows platforms */
// ssize_t perform_socket_write(socket_t desc, const char * txt, size_t length)
// {
// ssize_t result;
//
// result = write(desc, txt, length);
//
// if (result > 0) {
// /* Write was successful. */
// return (result);
// }
//
// if (result == 0) {
// /* This should never happen! */
// log("SYSERR: Huh??  write() returned 0???  Please report this!");
// return ( - 1);
// }
//
// /*
//  * result < 0, so an error was encountered - is it transient?
//  * Unfortunately, different systems use different constants to
//  * indicate this.
//  */
//
// # ifdef EAGAIN        /* POSIX */
// if (errno == EAGAIN)
// return (0);
// # endif
//
// # ifdef EWOULDBLOCK    /* BSD */
// if (errno == EWOULDBLOCK)
// return (0);
// # endif
//
// # ifdef EDEADLK        /* Macintosh */
// if (errno == EDEADLK)
// return (0);
// # endif
//
// /* Looks like the error was fatal.  Too bad. */
// return (- 1);
// }
//
// # endif /* CIRCLE_WINDOWS */
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
    let mut total = txt.len();
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
fn perform_socket_read(d: &mut DescriptorData) -> std::io::Result<usize> {
    let stream = &mut d.stream;
    let input = &mut d.inbuf;

    let mut buf = [0 as u8; 4096];

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
    //
    // # if defined(CIRCLE_ACORN)
    // ret = recv(desc, read_point, space_left, MSG_DONTWAIT);
    // # elif defined(CIRCLE_WINDOWS)
    // ret = recv(desc, read_point, space_left, 0);
    // # else
    // ret = read(desc, read_point, space_left);
    // # endif
    //
    // /* Read was successful. */
    // if (ret > 0)
    // return (ret);
    //
    // /* read() returned 0, meaning we got an EOF. */
    // if (ret == 0) {
    // log("WARNING: EOF on socket read (connection broken by peer)");
    // return ( - 1);
    // }
    //
    // /*
    //  * read returned a value < 0: there was an error
    //  */
    //
    // # if defined(CIRCLE_WINDOWS)    /* Windows */
    // if (WSAGetLastError() == WSAEWOULDBLOCK | | WSAGetLastError() == WSAEINTR)
    // return (0);
    // # else
    //
    // # ifdef EINTR        /* Interrupted system call - various platforms */
    // if (errno == EINTR)
    // return (0);
    // # endif
    //
    // # ifdef EAGAIN        /* POSIX */
    // if (errno == EAGAIN)
    // return (0);
    // # endif
    //
    // # ifdef EWOULDBLOCK    /* BSD */
    // if (errno == EWOULDBLOCK)
    // return (0);
    // # endif /* EWOULDBLOCK */
    //
    // # ifdef EDEADLK        /* Macintosh */
    // if (errno == EDEADLK)
    // return (0);
    // # endif
    //
    // # ifdef ECONNRESET
    // if (errno == ECONNRESET)
    // return ( - 1);
    // # endif
    //
    // #endif /* CIRCLE_WINDOWS */
    //
    // /*
    //  * We don't know what happened, cut them off. This qualifies for
    //  * a SYSERR because we have no idea what happened at this point.
    //  */
    // perror("SYSERR: perform_socket_read: about to lose connection");
    // return ( - 1);
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
fn process_input(t: &Rc<RefCell<DescriptorData>>) -> i32 {
    let buf_length;
    let mut failed_subst;
    let mut bytes_read;
    let mut space_left;
    let mut read_point = 0;
    let mut write_point = "".to_string();
    let mut nl_pos: Option<usize> = None;
    let mut tmp = String::new();

    /* first, find the point where we left off reading data */
    let mut mut_t = RefCell::borrow_mut(t);
    buf_length = mut_t.inbuf.len();
    read_point = buf_length;
    space_left = MAX_RAW_INPUT_LENGTH - buf_length - 1;

    loop {
        if space_left <= 0 {
            warn!("WARNING: process_input: about to close connection: input overflow");
            return -1;
        }

        bytes_read = perform_socket_read(&mut mut_t);
        info!("{} {} {:?}", buf_length, mut_t.inbuf.capacity(), bytes_read);

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
            let x = mut_t.inbuf.chars().nth(i).unwrap();

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

    let ptr = 0 as usize;
    while nl_pos.is_some() {
        tmp.truncate(0);
        space_left = MAX_INPUT_LENGTH - 1;

        /* The '> 1' reserves room for a '$ => $$' expansion. */
        for ptr in 0..mut_t.inbuf.len() {
            let x = mut_t.inbuf.chars().nth(ptr).unwrap();
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
            let buffer = format!("Line too long.  Truncated to:\r\n{}\r\n", tmp);

            if write_to_descriptor(&mut RefCell::borrow_mut(t).stream, tmp.as_str()) < 0 {
                return -1;
            }
        }

        // if (t -> snoop_by)
        // write_to_output(t ->snoop_by, "%% %s\r\n", tmp);
        failed_subst = 0;

        if tmp == "!" {
            /* Redo last command. */
            //strcpy(tmp, t -> last_input); /* strcpy: OK (by mutual MAX_INPUT_LENGTH) */
        } else if tmp.starts_with('!') && tmp.len() > 1 {
            // char * commandln = (tmp + 1);
            // int
            // starting_pos = t -> history_pos,
            // cnt = (t -> history_pos == 0?
            // HISTORY_SIZE - 1: t -> history_pos - 1);
            //
            // skip_spaces(&commandln);
            // for (; cnt != starting_pos; cnt - -) {
            //     if (t -> history[cnt] & &is_abbrev(commandln, t -> history[cnt])) {
            //         strcpy(tmp, t -> history[cnt]); /* strcpy: OK (by mutual MAX_INPUT_LENGTH) */
            //         strcpy(t -> last_input, tmp); /* strcpy: OK (by mutual MAX_INPUT_LENGTH) */
            //         write_to_output(t, "%s\r\n", tmp);
            //         break;
            //     }
            //     if (cnt == 0) {
            //         /* At top, loop to bottom. */
            //         cnt = HISTORY_SIZE;
            //     }
            // }
        } else if tmp.starts_with('^') {
            // if (!(failed_subst = perform_subst(t, t -> last_input, tmp)))
            // strcpy(t -> last_input, tmp);    /* strcpy: OK (by mutual MAX_INPUT_LENGTH) */
        } else {
            // strcpy(t -> last_input, tmp); /* strcpy: OK (by mutual MAX_INPUT_LENGTH) */
            // if (t -> history
            // [t -> history_pos])
            // free(t -> history[t -> history_pos]); /* Clear the old line. */
            // t -> history
            // [t -> history_pos] = strdup(tmp); /* Save the new. */
            // if ( + + t -> history_pos > = HISTORY_SIZE)    /* Wrap to top. */
            // t -> history_pos = 0;
        }

        // if (!failed_subst)
        write_to_q(tmp.as_str(), &mut mut_t.input, false);

        /* find the end of this line */
        while nl_pos.unwrap() < mut_t.inbuf.len()
            && isnewl!(mut_t.inbuf.chars().nth(nl_pos.unwrap()).unwrap())
        {
            nl_pos = Some(nl_pos.unwrap() + 1);
        }

        /* see if there's another newline in the input buffer */
        read_point = nl_pos.unwrap();
        nl_pos = None;
        for i in read_point..mut_t.inbuf.len() {
            if isnewl!(mut_t.inbuf.chars().nth(i).unwrap()) {
                nl_pos = Some(i);
                break;
            }
        }
    }
    mut_t.inbuf.drain(..read_point);

    return 1;
}

/* perform substitution for the '^..^' csh-esque syntax orig is the
 * orig string, i.e. the one being modified.  subst contains the
 * substition string, i.e. "^telm^tell"
 */
// int perform_subst(struct descriptor_data * t, char *orig, char *subst)
// {
// char newsub[MAX_INPUT_LENGTH + 5];
//
// char *first, * second, * strpos;
//
// /*
//  * first is the position of the beginning of the first string (the one
//  * to be replaced
//  */
// first = subst + 1;
//
// /* now find the second '^' */
// if ( ! (second = strchr(first, '^'))) {
// write_to_output(t, "Invalid substitution.\r\n");
// return (1);
// }
// /* terminate "first" at the position of the '^' and make 'second' point
//  * to the beginning of the second string */
// * (second + + ) = '\0';
//
// /* now, see if the contents of the first string appear in the original */
// if ( ! (strpos = strstr(orig, first))) {
// write_to_output(t, "Invalid substitution.\r\n");
// return (1);
// }
// /* now, we construct the new string for output. */
//
// /* first, everything in the original, up to the string to be replaced */
// strncpy(newsub, orig, strpos - orig); /* strncpy: OK (newsub:MAX_INPUT_LENGTH+5 > orig:MAX_INPUT_LENGTH) */
// newsub[strpos - orig] = '\0';
//
// /* now, the replacement string */
// strncat(newsub, second, MAX_INPUT_LENGTH - strlen(newsub) - 1); /* strncpy: OK */
//
// /* now, if there's anything left in the original after the string to
//  * replaced, copy that too. */
// if (((strpos - orig) + strlen(first)) < strlen(orig))
// strncat(newsub, strpos + strlen(first), MAX_INPUT_LENGTH - strlen(newsub) - 1); /* strncpy: OK */
//
// /* terminate the string in case of an overflow from strncat */
// newsub[MAX_INPUT_LENGTH - 1] = '\0';
// strcpy(subst, newsub); /* strcpy: OK (by mutual MAX_INPUT_LENGTH) */
//
// return (0);
// }

// void close_socket(struct descriptor_data * d)
// {
// struct descriptor_data * temp;
//
// REMOVE_FROM_LIST(d, descriptor_list, next);
// CLOSE_SOCKET(d -> descriptor);
// flush_queues(d);
//
// /* Forget snooping */
// if (d ->snooping)
// d -> snooping -> snoop_by = NULL;
//
// if (d -> snoop_by) {
// write_to_output(d -> snoop_by, "Your victim is no longer among us.\r\n");
// d-> snoop_by -> snooping = NULL;
// }
//
// if (d -> character) {
// /* If we're switched, this resets the mobile taken. */
// d -> character -> desc = NULL;
//
// /* Plug memory leak, from Eric Green. */
// if (! IS_NPC(d -> character) & & PLR_FLAGGED(d -> character, PLR_MAILING) & & d-> str) {
// if ( * (d -> str))
// free( * (d -> str));
// free(d -> str);
// }
//
// if (STATE(d) == ConPlaying | | STATE(d) == ConDisconnect) {
// struct char_data * link_challenged = d -> original ? d-> original: d -> character;
//
// /* We are guaranteed to have a person. */
// act("$n has lost $s link.", TRUE, link_challenged, 0, 0, TO_ROOM);
// save_char(link_challenged);
// mudlog(NRM, MAX(LVL_IMMORT, GET_INVIS_LEV(link_challenged)), TRUE, "Closing link to: %s.", GET_NAME(link_challenged));
// } else {
// mudlog(CMP, LVL_IMMORT, TRUE, "Losing player: %s.", GET_NAME(d -> character) ? GET_NAME(d-> character): "<null>");
// free_char(d-> character);
// }
// } else
// mudlog(CMP, LVL_IMMORT, TRUE, "Losing descriptor without char.");
//
// /* JE 2/22/95 -- part of my unending quest to make switch stable */
// if (d -> original & & d-> original -> desc)
// d -> original -> desc = NULL;
//
// /* Clear the command history. */
// if (d -> history) {
// int cnt;
// for (cnt = 0; cnt < HISTORY_SIZE; cnt + + )
// if (d-> history[cnt])
// free(d ->history[cnt]);
// free(d ->history);
// }
//
// if (d -> showstr_head)
// free(d ->showstr_head);
// if (d -> showstr_count)
// free(d -> showstr_vector);
//
// free(d);
// }

// void check_idle_passwords(void)
// {
// struct descriptor_data * d, * next_d;
//
// for (d = descriptor_list; d; d = next_d) {
// next_d = d -> next;
// if (STATE(d) != ConPassword & & STATE(d) != ConGetName)
// continue;
// if ( ! d ->idle_tics) {
// d -> idle_tics + +;
// continue;
// } else {
// echo_on(d);
// write_to_output(d, "\r\nTimed out... goodbye.\r\n");
// STATE(d) = ConClose;
// }
// }
// }

/*
 * I tried to universally convert Circle over to POSIX compliance, but
 * alas, some systems are still straggling behind and don't have all the
 * appropriate defines.  In particular, NeXT 2.x defines O_NDELAY but not
 * O_NONBLOCK.  Krusty old NeXT machines!  (Thanks to Michael Jones for
 * this and various other NeXT fixes.)
 */

// # if defined(CIRCLE_WINDOWS)
//
// void nonblock(socket_t s)
// {
// unsigned long val = 1;
// ioctlsocket(s, FIONBIO, & val);
// }
//
// # elif defined(CIRCLE_AMIGA)
//
// void nonblock(socket_t s)
// {
// long val = 1;
// IoctlSocket(s, FIONBIO, & val);
// }
//
// # elif defined(CIRCLE_ACORN)
//
// void nonblock(socket_t s)
// {
// int val = 1;
// socket_ioctl(s, FIONBIO, & val);
// }
//
// # elif defined(CIRCLE_VMS)
//
// void nonblock(socket_t s)
// {
// int val = 1;
//
// if (ioctl(s, FIONBIO, & val) < 0) {
// perror("SYSERR: Fatal error executing nonblock (comm.c)");
// exit(1);
// }
// }
//
// # elif defined(CIRCLE_UNIX) | | defined(CIRCLE_OS2) | | defined(CIRCLE_MACINTOSH)
//
// # ifndef O_NONBLOCK
// # define O_NONBLOCK O_NDELAY
// # endif
//
// void nonblock(socket_t s)
// {
// int flags;
//
// flags = fcntl(s, F_GETFL, 0);
// flags |= O_NONBLOCK;
// if (fcntl(s, F_SETFL, flags) < 0) {
// perror("SYSERR: Fatal error executing nonblock (comm.c)");
// exit(1);
// }
// }
//
// # endif  /* CIRCLE_UNIX || CIRCLE_OS2 || CIRCLE_MACINTOSH */
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

// size_t send_to_char(struct char_data * ch, const char * messg, ...)
// {
// if (ch -> desc & & messg & & * messg) {
// size_t left;
// va_list args;
//
// va_start(args, messg);
// left = vwrite_to_output(ch -> desc, messg, args);
// va_end(args);
// return left;
// }
// return 0;
// }

// void send_to_all(const char * messg, ...)
// {
// struct descriptor_data * i;
// va_list args;
//
// if (messg == NULL)
// return;
//
// for (i = descriptor_list; i; i = i -> next) {
// if (STATE(i) != ConPlaying)
// continue;
//
// va_start(args, messg);
// vwrite_to_output(i, messg, args);
// va_end(args);
// }
// }

// void send_to_outdoor(const char * messg, ...)
// {
// struct descriptor_data * i;
//
// if ( ! messg || ! * messg)
// return;
//
// for (i = descriptor_list; i; i = i -> next) {
// va_list args;
//
// if (STATE(i) != ConPlaying | | i -> character == NULL)
// continue;
// if ( ! AWAKE(i -> character) | | ! OUTSIDE(i -> character))
// continue;
//
// va_start(args, messg);
// vwrite_to_output(i, messg, args);
// va_end(args);
// }
// }

// void send_to_room(room_rnum room, const char * messg, ...)
// {
// struct char_data * i;
// va_list args;
//
// if (messg == NULL)
// return;
//
// for (i = world[room].people; i; i = i -> next_in_room) {
// if ( ! i -> desc)
// continue;
//
// va_start(args, messg);
// vwrite_to_output(i ->desc, messg, args);
// va_end(args);
// }
// }

// const char * ACTNULL = "<NULL>";
//
// # define CHECK_NULL(pointer, expression) \
// if ((pointer) == NULL) i = ACTNULL; else i = (expression);

/* higher-level communication: the act() function */
// void perform_act(const char * orig, struct char_data * ch, struct obj_data * obj,
// const void * vict_obj, const struct char_data * to)
// {
// const char * i = NULL;
// char lbuf[MAX_STRING_LENGTH], * buf, * j;
// bool uppercasenext = FALSE;
//
// buf = lbuf;
//
// for (; ; ) {
// if ( * orig == '$') {
// switch ( * ( + + orig)) {
// case 'n':
// i = PERS(ch, to);
// break;
// case 'N':
// CHECK_NULL(vict_obj, PERS(( const struct char_data * ) vict_obj, to));
// break;
// case 'm':
// i = HMHR(ch);
// break;
// case 'M':
// CHECK_NULL(vict_obj, HMHR(( const struct char_data * ) vict_obj));
// break;
// case 's':
// i = HSHR(ch);
// break;
// case 'S':
// CHECK_NULL(vict_obj, HSHR(( const struct char_data * ) vict_obj));
// break;
// case 'e':
// i = HSSH(ch);
// break;
// case 'E':
// CHECK_NULL(vict_obj, HSSH(( const struct char_data * ) vict_obj));
// break;
// case 'o':
// CHECK_NULL(obj, OBJN(obj, to));
// break;
// case 'O':
// CHECK_NULL(vict_obj, OBJN(( const struct obj_data * ) vict_obj, to));
// break;
// case 'p':
// CHECK_NULL(obj, OBJS(obj, to));
// break;
// case 'P':
// CHECK_NULL(vict_obj, OBJS(( const struct obj_data * ) vict_obj, to));
// break;
// case 'a':
// CHECK_NULL(obj, SANA(obj));
// break;
// case 'A':
// CHECK_NULL(vict_obj, SANA(( const struct obj_data * ) vict_obj));
// break;
// case 'T':
// CHECK_NULL(vict_obj, (const char * ) vict_obj);
// break;
// case 'F':
// CHECK_NULL(vict_obj, fname(( const char * ) vict_obj));
// break;
// /* uppercase previous word */
// case 'u':
// for (j = buf; j > lbuf & & ! isspace((int) *(j - 1)); j - -);
// if (j != buf)
// * j = UPPER( * j);
// i = "";
// break;
// /* uppercase next word */
// case 'U':
// uppercasenext = TRUE;
// i = "";
// break;
// case '$':
// i = "$";
// break;
// default:
// log("SYSERR: Illegal $-code to act(): %c", * orig);
// log("SYSERR: %s", orig);
// i = "";
// break;
// }
// while (( * buf = * (i ++ )))
// {
// if (uppercasenext & & ! isspace((int) * buf))
// {
// * buf = UPPER( * buf);
// uppercasenext = FALSE;
// }
// buf + +;
// }
// orig + +;
// } else if ( ! ( * (buf + +) = * (orig + + ))) {
// break;
// } else if (uppercasenext & & ! isspace((int) * (buf - 1))) {
// * (buf - 1) = UPPER( *(buf - 1));
// uppercasenext = FALSE;
// }
// }
//
// * ( - - buf) = '\r';
// *( + + buf) = '\n';
// * (+ + buf) = '\0';
//
// write_to_output(to-> desc, "%s", CAP(lbuf));
// }
//
//
// # define SENDOK(ch)    ((ch) -> desc & & (to_sleeping | | AWAKE(ch)) & & \
// (IS_NPC(ch) | | ! PLR_FLAGGED((ch), PLR_WRITING)))
//
// void act(const char * str, int hide_invisible, struct char_data * ch,
// struct obj_data * obj, const void * vict_obj, int type )
// {
// const struct char_data * to;
// int to_sleeping;
//
// if ( ! str | | ! * str)
// return;
//
// /*
//  * Warning: the following TO_SLEEP code is a hack.
//  *
//  * I wanted to be able to tell act to deliver a message regardless of sleep
//  * without adding an additional argument.  TO_SLEEP is 128 (a single bit
//  * high up).  It's ONLY legal to combine TO_SLEEP with one other TO_x
//  * command.  It's not legal to combine TO_x's with each other otherwise.
//  * TO_SLEEP only works because its value "happens to be" a single bit;
//  * do not change it to something else.  In short, it is a hack.
//  */
//
// /* check if TO_SLEEP is there, and remove it if it is. */
// if ((to_sleeping = ( type & TO_SLEEP)))
// type &= ~TO_SLEEP;
//
// if (type == TO_CHAR) {
// if (ch & & SENDOK(ch))
// perform_act(str, ch, obj, vict_obj, ch);
// return;
// }
//
// if ( type == TO_VICT) {
// if ((to = ( const struct char_data *) vict_obj) != NULL & & SENDOK(to))
// perform_act(str, ch, obj, vict_obj, to);
// return;
// }
// /* ASSUMPTION: at this point we know type must be TO_NOTVICT or TO_ROOM */
//
// if (ch & & IN_ROOM(ch) != NOWHERE)
// to = world[IN_ROOM(ch)].people;
// else if (obj & & IN_ROOM(obj) != NOWHERE)
// to = world[IN_ROOM(obj)].people;
// else {
// log("SYSERR: no valid target to act()!");
// return;
// }
//
// for (; to; to = to -> next_in_room) {
// if ( ! SENDOK(to) | | (to == ch))
// continue;
// if (hide_invisible & & ch & & ! CAN_SEE(to, ch))
// continue;
// if ( type != TO_ROOM & & to == vict_obj)
// continue;
// perform_act(str, ch, obj, vict_obj, to);
// }
// }
