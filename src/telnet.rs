/*
 * Definitions for the TELNET protocol.
 */
pub const IAC: u8 = 0xff; /* interpret as command: */
pub const DONT: u8 = 254; /* you are not to use option */
pub const DO: u8 = 253; /* please, you use option */
pub const WONT: u8 = 0xfc; /* I won't use option */
pub const WILL: u8 = 0xfb; /* I will use option */
pub const SB: u8 = 250; /* interpret as subnegotiation */
pub const GA: u8 = 249; /* you may reverse the line */
pub const EL: u8 = 248; /* erase the current line */
pub const EC: u8 = 247; /* erase the current character */
pub const AYT: u8 = 246; /* are you there */
pub const AO: u8 = 245; /* abort output--but let prog finish */
pub const IP: u8 = 244; /* interrupt process--permanently */
pub const BREAK: u8 = 243; /* break */
pub const DM: u8 = 242; /* data mark--for connect. cleaning */
pub const NOP: u8 = 241; /* nop */
pub const SE: u8 = 240; /* end sub negotiation */
pub const EOR: u8 = 239; /* end of record (transparent mode) */
pub const ABORT: u8 = 238; /* Abort process */
pub const SUSP: u8 = 237; /* Suspend process */
pub const xEOF: u8 = 236; /* End of file: EOF is already used... */

pub const SYNCH: u8 = 242; /* for telfunc calls */

// #ifdef TELCMDS
// char *telcmds[] = {
// "EOF", "SUSP", "ABORT", "EOR",
// "SE", "NOP", "DMARK", "BRK", "IP", "AO", "AYT", "EC",
// "EL", "GA", "SB", "WILL", "WONT", "DO", "DONT", "IAC", 0,
// };
// #else
// extern char *telcmds[];
// #endif
//
// #define	TELCMD_FIRST	xEOF
// #define	TELCMD_LAST	IAC
// #define	TELCMD_OK(x)	((unsigned int)(x) <= TELCMD_LAST && \
// (unsigned int)(x) >= TELCMD_FIRST)
// #define	TELCMD(x)	telcmds[(x)-TELCMD_FIRST]

/* telnet options */
pub const TELOPT_BINARY: u8 = 0; /* 8-bit data path */
pub const TELOPT_ECHO: u8 = 0x01; /* echo */
pub const TELOPT_RCP: u8 = 2; /* prepare to reconnect */
pub const TELOPT_SGA: u8 = 3; /* suppress go ahead */
pub const TELOPT_NAMS: u8 = 4; /* approximate message size */
pub const TELOPT_STATUS: u8 = 5; /* give status */
pub const TELOPT_TM: u8 = 6; /* timing mark */
pub const TELOPT_RCTE: u8 = 7; /* remote controlled transmission and echo */
pub const TELOPT_NAOL: u8 = 8; /* negotiate about output line width */
pub const TELOPT_NAOP: u8 = 9; /* negotiate about output page size */
pub const TELOPT_NAOCRD: u8 = 10; /* negotiate about CR disposition */
pub const TELOPT_NAOHTS: u8 = 11; /* negotiate about horizontal tabstops */
pub const TELOPT_NAOHTD: u8 = 12; /* negotiate about horizontal tab disposition */
pub const TELOPT_NAOFFD: u8 = 13; /* negotiate about formfeed disposition */
pub const TELOPT_NAOVTS: u8 = 14; /* negotiate about vertical tab stops */
pub const TELOPT_NAOVTD: u8 = 15; /* negotiate about vertical tab disposition */
pub const TELOPT_NAOLFD: u8 = 16; /* negotiate about output LF disposition */
pub const TELOPT_XASCII: u8 = 17; /* extended ascic character set */
pub const TELOPT_LOGOUT: u8 = 18; /* force logout */
pub const TELOPT_BM: u8 = 19; /* byte macro */
pub const TELOPT_DET: u8 = 20; /* data entry terminal */
pub const TELOPT_SUPDUP: u8 = 21; /* supdup protocol */
pub const TELOPT_SUPDUPOUTPUT: u8 = 22; /* supdup output */
pub const TELOPT_SNDLOC: u8 = 23; /* send location */
pub const TELOPT_TTYPE: u8 = 24; /* terminal type */
pub const TELOPT_EOR: u8 = 25; /* end or record */
pub const TELOPT_TUID: u8 = 26; /* TACACS user identification */
pub const TELOPT_OUTMRK: u8 = 27; /* output marking */
pub const TELOPT_TTYLOC: u8 = 28; /* terminal location number */
pub const TELOPT_3270REGIME: u8 = 29; /* 3270 regime */
pub const TELOPT_X3PAD: u8 = 30; /* X.3 PAD */
pub const TELOPT_NAWS: u8 = 31; /* window size */
pub const TELOPT_TSPEED: u8 = 32; /* terminal speed */
pub const TELOPT_LFLOW: u8 = 33; /* remote flow control */
pub const TELOPT_LINEMODE: u8 = 34; /* Linemode option */
pub const TELOPT_XDISPLOC: u8 = 35; /* X Display Location */
pub const TELOPT_OLD_ENVIRON: u8 = 36; /* Old - Environment variables */
pub const TELOPT_AUTHENTICATION: u8 = 37; /*; Authenticate */
pub const TELOPT_ENCRYPT: u8 = 38; /* Encryption option */
pub const TELOPT_NEW_ENVIRON: u8 = 39; /* New - Environment variables */
pub const TELOPT_EXOPL: u8 = 255; /* extended-options-list */
