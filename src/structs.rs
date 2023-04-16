use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::{DescriptorData, Game};

pub type Special =
    fn(game: &Game, ch: &Rc<CharData>, me: &dyn Any, cmd: i32, argument: &str) -> bool;

pub const OPT_USEC: u128 = 100000;
pub const PASSES_PER_SEC: u128 = 1000000 / OPT_USEC;

pub const PULSE_ZONE: u128 = 10 * PASSES_PER_SEC;
pub const PULSE_MOBILE: u128 = 10 * PASSES_PER_SEC;
pub const PULSE_VIOLENCE: u128 = 2 * PASSES_PER_SEC;
pub const PULSE_AUTOSAVE: u128 = 60 * PASSES_PER_SEC;
pub const PULSE_IDLEPWD: u128 = 15 * PASSES_PER_SEC;
pub const PULSE_SANITY: u128 = 30 * PASSES_PER_SEC;
pub const PULSE_USAGE: u128 = 5 * 60 * PASSES_PER_SEC; /* 5 mins */
pub const PULSE_TIMESAVE: u128 = 30 * 60 * PASSES_PER_SEC; /* should be >= SECS_PER_MUD_HOUR */

/* Room flags: used in room_data.room_flags */
/* WARNING: In the world files, NEVER set the bits marked "R" ("Reserved") */
pub const ROOM_DARK: i64 = 1 << 0; /* Dark			*/
pub const ROOM_DEATH: i64 = 1 << 1; /* Death trap		*/
pub const ROOM_NOMOB: i64 = 1 << 2; /* MOBs not allowed		*/
pub const ROOM_INDOORS: i64 = 1 << 3; /* Indoors			*/
pub const ROOM_PEACEFUL: i64 = 1 << 4; /* Violence not allowed	*/
pub const ROOM_SOUNDPROOF: i64 = 1 << 5; /* Shouts, gossip blocked	*/
pub const ROOM_NOTRACK: i64 = 1 << 6; /* Track won't go through	*/
pub const ROOM_NOMAGIC: i64 = 1 << 7; /* Magic not allowed		*/
pub const ROOM_TUNNEL: i64 = 1 << 8; /* room for only 1 pers	*/
pub const ROOM_PRIVATE: i64 = 1 << 9; /* Can't teleport in		*/
pub const ROOM_GODROOM: i64 = 1 << 10; /* LVL_GOD+ only allowed	*/
pub const ROOM_HOUSE: i64 = 1 << 11; /* (R) Room is a house	*/
pub const ROOM_HOUSE_CRASH: i64 = 1 << 12; /* (R) House needs saving	*/
pub const ROOM_ATRIUM: i64 = 1 << 13; /* (R) The door to a house	*/
pub const ROOM_OLC: i64 = 1 << 14; /* (R) Modifyable/!compress	*/
pub const ROOM_BFS_MARK: i64 = 1 << 15; /* (R) breath-first srch mrk	*/

/* Exit info: used in room_data.dir_option.exit_info */
pub const EX_ISDOOR: i16 = 1 << 0; /* Exit is a door		*/
pub const EX_CLOSED: i16 = 1 << 1; /* The door is closed	*/
pub const EX_LOCKED: i16 = 1 << 2; /* The door is locked	*/
pub const EX_PICKPROOF: i16 = 1 << 3; /* Lock can't be picked	*/

/* Sector types: used in room_data.sector_type */
pub const SECT_INSIDE: i32 = 0; /* Indoors			*/
pub const SECT_CITY: i32 = 1; /* In a city			*/
pub const SECT_FIELD: i32 = 2; /* In a field		*/
pub const SECT_FOREST: i32 = 3; /* In a forest		*/
pub const SECT_HILLS: i32 = 4; /* In the hills		*/
pub const SECT_MOUNTAIN: i32 = 5; /* On a mountain		*/
pub const SECT_WATER_SWIM: i32 = 6; /* Swimmable water		*/
pub const SECT_WATER_NOSWIM: i32 = 7; /* Water - need a boat	*/
pub const SECT_FLYING: i32 = 8; /* Wheee!			*/
pub const SECT_UNDERWATER: i32 = 9; /* Underwater		*/

/* Player conditions */
pub const DRUNK: i32 = 0;
pub const FULL: i32 = 1;
pub const THIRST: i32 = 2;

/* Sun state for weather_data */
pub const SUN_DARK: i32 = 0;
pub const SUN_RISE: i32 = 1;
pub const SUN_LIGHT: i32 = 2;
pub const SUN_SET: i32 = 3;

/* Sky conditions for weather_data */
pub const SKY_CLOUDLESS: i32 = 0;
pub const SKY_CLOUDY: i32 = 1;
pub const SKY_RAINING: i32 = 2;
pub const SKY_LIGHTNING: i32 = 3;

/* Rent codes */
pub const RENT_UNDEF: i32 = 0;
pub const RENT_CRASH: i32 = 1;
pub const RENT_RENTED: i32 = 2;
pub const RENT_CRYO: i32 = 3;
pub const RENT_FORCED: i32 = 4;
pub const RENT_TIMEDOUT: i32 = 5;

/* object-related defines ********************************************/

/* Item types: used by obj_data.obj_flags.type_flag */
pub const ITEM_LIGHT: u8 = 1; /* Item is a light source	*/
pub const ITEM_SCROLL: u8 = 2; /* Item is a scroll		*/
pub const ITEM_WAND: u8 = 3; /* Item is a wand		*/
pub const ITEM_STAFF: u8 = 4; /* Item is a staff		*/
pub const ITEM_WEAPON: u8 = 5; /* Item is a weapon		*/
pub const ITEM_FIREWEAPON: u8 = 6; /* Unimplemented		*/
pub const ITEM_MISSILE: u8 = 7; /* Unimplemented		*/
pub const ITEM_TREASURE: u8 = 8; /* Item is a treasure, not gold	*/
pub const ITEM_ARMOR: u8 = 9; /* Item is armor		*/
pub const ITEM_POTION: u8 = 10; /* Item is a potion		*/
pub const ITEM_WORN: u8 = 11; /* Unimplemented		*/
pub const ITEM_OTHER: u8 = 12; /* Misc object			*/
pub const ITEM_TRASH: u8 = 13; /* Trash - shopkeeps won't buy	*/
pub const ITEM_TRAP: u8 = 14; /* Unimplemented		*/
pub const ITEM_CONTAINER: u8 = 15; /* Item is a container		*/
pub const ITEM_NOTE: u8 = 16; /* Item is note 		*/
pub const ITEM_DRINKCON: u8 = 17; /* Item is a drink container	*/
pub const ITEM_KEY: u8 = 18; /* Item is a key		*/
pub const ITEM_FOOD: u8 = 19; /* Item is food			*/
pub const ITEM_MONEY: u8 = 20; /* Item is money (gold)		*/
pub const ITEM_PEN: u8 = 21; /* Item is a pen		*/
pub const ITEM_BOAT: u8 = 22; /* Item is a boat		*/
pub const ITEM_FOUNTAIN: u8 = 23; /* Item is a fountain		*/

/* Take/Wear flags: used by obj_data.obj_flags.wear_flags */
pub const ITEM_WEAR_TAKE: i32 = 1 << 0; /* Item can be takes		*/
pub const ITEM_WEAR_FINGER: i32 = 1 << 1; /* Can be worn on finger	*/
pub const ITEM_WEAR_NECK: i32 = 1 << 2; /* Can be worn around neck 	*/
pub const ITEM_WEAR_BODY: i32 = 1 << 3; /* Can be worn on body 	*/
pub const ITEM_WEAR_HEAD: i32 = 1 << 4; /* Can be worn on head 	*/
pub const ITEM_WEAR_LEGS: i32 = 1 << 5; /* Can be worn on legs	*/
pub const ITEM_WEAR_FEET: i32 = 1 << 6; /* Can be worn on feet	*/
pub const ITEM_WEAR_HANDS: i32 = 1 << 7; /* Can be worn on hands	*/
pub const ITEM_WEAR_ARMS: i32 = 1 << 8; /* Can be worn on arms	*/
pub const ITEM_WEAR_SHIELD: i32 = 1 << 9; /* Can be used as a shield	*/
pub const ITEM_WEAR_ABOUT: i32 = 1 << 10; /* Can be worn about body 	*/
pub const ITEM_WEAR_WAIST: i32 = 1 << 11; /* Can be worn around waist 	*/
pub const ITEM_WEAR_WRIST: i32 = 1 << 12; /* Can be worn on wrist 	*/
pub const ITEM_WEAR_WIELD: i32 = 1 << 13; /* Can be wielded		*/
pub const ITEM_WEAR_HOLD: i32 = 1 << 14; /* Can be held		*/

/* Character equipment positions: used as index for char_data.equipment[] */
/* NOTE: Don't confuse these constants with the ITEM_ bitvectors
which control the valid places you can wear a piece of equipment */
pub const WEAR_LIGHT: i16 = 0;
pub const WEAR_FINGER_R: i16 = 1;
pub const WEAR_FINGER_L: i16 = 2;
pub const WEAR_NECK_1: i16 = 3;
pub const WEAR_NECK_2: i16 = 4;
pub const WEAR_BODY: i16 = 5;
pub const WEAR_HEAD: i16 = 6;
pub const WEAR_LEGS: i16 = 7;
pub const WEAR_FEET: i16 = 8;
pub const WEAR_HANDS: i16 = 9;
pub const WEAR_ARMS: i16 = 10;
pub const WEAR_SHIELD: i16 = 11;
pub const WEAR_ABOUT: i16 = 12;
pub const WEAR_WAIST: i16 = 13;
pub const WEAR_WRIST_R: i16 = 14;
pub const WEAR_WRIST_L: i16 = 15;
pub const WEAR_WIELD: i16 = 16;
pub const WEAR_HOLD: i16 = 17;

pub const NUM_WEARS: i8 = 18;

/* Extra object flags: used by obj_data.obj_flags.extra_flags */
pub const ITEM_GLOW: i32 = 1 << 0; /* Item is glowing		*/
pub const ITEM_HUM: i32 = 1 << 1; /* Item is humming		*/
pub const ITEM_NORENT: i32 = 1 << 2; /* Item cannot be rented	*/
pub const ITEM_NODONATE: i32 = 1 << 3; /* Item cannot be donated	*/
pub const ITEM_NOINVIS: i32 = 1 << 4; /* Item cannot be made invis	*/
pub const ITEM_INVISIBLE: i32 = 1 << 5; /* Item is invisible		*/
pub const ITEM_MAGIC: i32 = 1 << 6; /* Item is magical		*/
pub const ITEM_NODROP: i32 = 1 << 7; /* Item is cursed: can't drop	*/
pub const ITEM_BLESS: i32 = 1 << 8; /* Item is blessed		*/
pub const ITEM_ANTI_GOOD: i32 = 1 << 9; /* Not usable by good people	*/
pub const ITEM_ANTI_EVIL: i32 = 1 << 10; /* Not usable by evil people	*/
pub const ITEM_ANTI_NEUTRAL: i32 = 1 << 11; /* Not usable by neutral people	*/
pub const ITEM_ANTI_MAGIC_USER: i32 = 1 << 12; /* Not usable by mages		*/
pub const ITEM_ANTI_CLERIC: i32 = 1 << 13; /* Not usable by clerics	*/
pub const ITEM_ANTI_THIEF: i32 = 1 << 14; /* Not usable by thieves	*/
pub const ITEM_ANTI_WARRIOR: i32 = 1 << 15; /* Not usable by warriors	*/
pub const ITEM_NOSELL: i32 = 1 << 16; /* Shopkeepers won't touch it	*/

/* Modifier constants used with obj affects ('A' fields) */
pub const APPLY_NONE: i8 = 0; /* No effect			*/
pub const APPLY_STR: i8 = 1; /* Apply to strength		*/
pub const APPLY_DEX: i8 = 2; /* Apply to dexterity		*/
pub const APPLY_INT: i8 = 3; /* Apply to intelligence	*/
pub const APPLY_WIS: i8 = 4; /* Apply to wisdom		*/
pub const APPLY_CON: i8 = 5; /* Apply to constitution	*/
pub const APPLY_CHA: i8 = 6; /* Apply to charisma		*/
pub const APPLY_CLASS: i8 = 7; /* Reserved			*/
pub const APPLY_LEVEL: i8 = 8; /* Reserved			*/
pub const APPLY_AGE: i8 = 9; /* Apply to age			*/
pub const APPLY_CHAR_WEIGHT: i8 = 10; /* Apply to weight		*/
pub const APPLY_CHAR_HEIGHT: i8 = 11; /* Apply to height		*/
pub const APPLY_MANA: i8 = 12; /* Apply to max mana		*/
pub const APPLY_HIT: i8 = 13; /* Apply to max hit points	*/
pub const APPLY_MOVE: i8 = 14; /* Apply to max move points	*/
pub const APPLY_GOLD: i8 = 15; /* Reserved			*/
pub const APPLY_EXP: i8 = 16; /* Reserved			*/
pub const APPLY_AC: i8 = 17; /* Apply to Armor Class		*/
pub const APPLY_HITROLL: i8 = 18; /* Apply to hitroll		*/
pub const APPLY_DAMROLL: i8 = 19; /* Apply to damage roll		*/
pub const APPLY_SAVING_PARA: i8 = 20; /* Apply to save throw: paralz	*/
pub const APPLY_SAVING_ROD: i8 = 21; /* Apply to save throw: rods	*/
pub const APPLY_SAVING_PETRI: i8 = 22; /* Apply to save throw: petrif	*/
pub const APPLY_SAVING_BREATH: i8 = 23; /* Apply to save throw: breath	*/
pub const APPLY_SAVING_SPELL: i8 = 24; /* Apply to save throw: spells	*/

/* Container flags - value[1] */
pub const CONT_CLOSEABLE: i32 = 1 << 0; /* Container can be closed	*/
pub const CONT_PICKPROOF: i32 = 1 << 1; /* Container is pickproof	*/
pub const CONT_CLOSED: i32 = 1 << 2; /* Container is closed		*/
pub const CONT_LOCKED: i32 = 1 << 3; /* Container is locked		*/

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ConState {
    ConPlaying,
    /* Playing - Nominal state		*/
    ConClose,
    /* User disconnect, remove character.	*/
    ConGetName,
    /* By what name ..?			*/
    ConNameCnfrm,
    /* Did I get that right, x?		*/
    ConPassword,
    /* Password:				*/
    ConNewpasswd,
    /* Give me a password for x		*/
    ConCnfpasswd,
    /* Please retype password:		*/
    ConQsex,
    /* Sex?					*/
    ConQclass,
    /* Class?				*/
    ConRmotd,
    /* PRESS RETURN after MOTD		*/
    ConMenu,
    /* Your choice: (main menu)		*/
    ConExdesc,
    /* Enter a new description:		*/
    ConChpwdGetold,
    /* Changing passwd: get old		*/
    ConChpwdGetnew,
    /* Changing passwd: get new		*/
    ConChpwdVrfy,
    /* Verify new password			*/
    ConDelcnf1,
    /* Delete confirmation 1		*/
    ConDelcnf2,
    /* Delete confirmation 2		*/
    ConDisconnect,
    /* In-game link loss (leave character)	*/
}

/* Mobile flags: used by char_data.char_specials.act */
pub const MOB_SPEC: i64 = 1 << 0; /* Mob has a callable spec-proc	*/
pub const MOB_SENTINEL: i64 = 1 << 1; /* Mob should not move		*/
pub const MOB_SCAVENGER: i64 = 1 << 2; /* Mob picks up stuff on the ground	*/
pub const MOB_ISNPC: i64 = 1 << 3; /* (R) Automatically set on all Mobs	*/
pub const MOB_AWARE: i64 = 1 << 4; /* Mob can't be backstabbed		*/
pub const MOB_AGGRESSIVE: i64 = 1 << 5; /* Mob auto-attacks everybody nearby	*/
pub const MOB_STAY_ZONE: i64 = 1 << 6; /* Mob shouldn't wander out of zone	*/
pub const MOB_WIMPY: i64 = 1 << 7; /* Mob flees if severely injured	*/
pub const MOB_AGGR_EVIL: i64 = 1 << 8; /* Auto-attack any evil PC's		*/
pub const MOB_AGGR_GOOD: i64 = 1 << 9; /* Auto-attack any good PC's		*/
pub const MOB_AGGR_NEUTRAL: i64 = 1 << 10; /* Auto-attack any neutral PC's	*/
pub const MOB_MEMORY: i64 = 1 << 11; /* remember attackers if attacked	*/
pub const MOB_HELPER: i64 = 1 << 12; /* attack PCs fighting other NPCs	*/
pub const MOB_NOCHARM: i64 = 1 << 13; /* Mob can't be charmed		*/
pub const MOB_NOSUMMON: i64 = 1 << 14; /* Mob can't be summoned		*/
pub const MOB_NOSLEEP: i64 = 1 << 15; /* Mob can't be slept		*/
pub const MOB_NOBASH: i64 = 1 << 16; /* Mob can't be bashed (e.g. trees)	*/
pub const MOB_NOBLIND: i64 = 1 << 17; /* Mob can't be blinded		*/
pub const MOB_NOTDEADYET: i64 = 1 << 18; /* (R) Mob being extracted.		*/

/* Preference flags: used by char_data.player_specials.pref */
pub const PRF_BRIEF: i64 = 1 << 0; /* Room descs won't normally be shown	*/
pub const PRF_COMPACT: i64 = 1 << 1; /* No extra CRLF pair before prompts	*/
pub const PRF_DEAF: i64 = 1 << 2; /* Can't hear shouts			*/
pub const PRF_NOTELL: i64 = 1 << 3; /* Can't receive tells		*/
pub const PRF_DISPHP: i64 = 1 << 4; /* Display hit points in prompt	*/
pub const PRF_DISPMANA: i64 = 1 << 5; /* Display mana points in prompt	*/
pub const PRF_DISPMOVE: i64 = 1 << 6; /* Display move points in prompt	*/
pub const PRF_AUTOEXIT: i64 = 1 << 7; /* Display exits in a room		*/
pub const PRF_NOHASSLE: i64 = 1 << 8; /* Aggr mobs won't attack		*/
pub const PRF_QUEST: i64 = 1 << 9; /* On quest				*/
pub const PRF_SUMMONABLE: i64 = 1 << 10; /* Can be summoned			*/
pub const PRF_NOREPEAT: i64 = 1 << 11; /* No repetition of comm commands	*/
pub const PRF_HOLYLIGHT: i64 = 1 << 12; /* Can see in dark			*/
pub const PRF_COLOR_1: i64 = 1 << 13; /* Color (low bit)			*/
pub const PRF_COLOR_2: i64 = 1 << 14; /* Color (high bit)			*/
pub const PRF_NOWIZ: i64 = 1 << 15; /* Can't hear wizline			*/
pub const PRF_LOG1: i64 = 1 << 16; /* On-line System Log (low bit)	*/
pub const PRF_LOG2: i64 = 1 << 17; /* On-line System Log (high bit)	*/
pub const PRF_NOAUCT: i64 = 1 << 18; /* Can't hear auction channel		*/
pub const PRF_NOGOSS: i64 = 1 << 19; /* Can't hear gossip channel		*/
pub const PRF_NOGRATZ: i64 = 1 << 20; /* Can't hear grats channel		*/
pub const PRF_ROOMFLAGS: i64 = 1 << 21; /* Can see room flags (ROOM_x)	*/
pub const PRF_DISPAUTO: i64 = 1 << 22; /* Show prompt HP, MP, MV when < 30%.	*/

/* Variables for the output buffering system */
pub const MAX_SOCK_BUF: i32 = 12 * 1024; /* Size of kernel's sock buf   */
pub const MAX_PROMPT_LENGTH: i32 = 96; /* Max length of prompt        */
pub const GARBAGE_SPACE: i32 = 32; /* Space for **OVERFLOW** etc  */
pub const SMALL_BUFSIZE: i32 = 1024; /* Static output buffer size   */
/* Max amount of output that can be buffered */
pub const LARGE_BUFSIZE: i32 = MAX_SOCK_BUF - GARBAGE_SPACE - MAX_PROMPT_LENGTH;
pub const HISTORY_SIZE: i32 = 5; /* Keep last 5 commands. */
pub const MAX_STRING_LENGTH: i32 = 8192;
pub const MAX_INPUT_LENGTH: usize = 256; /* Max length per *line* of input */
pub const MAX_RAW_INPUT_LENGTH: usize = 512; /* Max size of *raw* input */
pub const MAX_MESSAGES: i32 = 60;
pub const MAX_NAME_LENGTH: usize = 20; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_PWD_LENGTH: usize = 16; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_TITLE_LENGTH: usize = 80; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const HOST_LENGTH: usize = 30; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const EXDSCR_LENGTH: usize = 240; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_TONGUE: usize = 3; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_SKILLS: usize = 200; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_AFFECT: usize = 32; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_OBJ_AFFECT: i32 = 6; /* Used in ObjFileElem *DO*NOT*CHANGE* */
pub const MAX_NOTE_LENGTH: i32 = 1000; /* arbitrary */

/* ================== Structure for player/non-player ===================== */
pub struct CharData {
    pub(crate) pfilepos: RefCell<i32>,
    /* playerfile pos		  */
    pub nr: MobRnum,
    /* Mob's rnum			  */
    pub in_room: Cell<RoomRnum>,
    /* Location (real room number)	  */
    pub was_in_room: Cell<RoomRnum>,
    /* location for linkdead people  */
    pub wait: Cell<i32>,
    /* wait for how many loops	  */
    pub player: RefCell<CharPlayerData>,
    /* Normal data                   */
    pub real_abils: RefCell<CharAbilityData>,
    /* Abilities without modifiers   */
    pub aff_abils: RefCell<CharAbilityData>,
    /* Abils with spells/stones/etc  */
    pub points: RefCell<CharPointData>,
    /* Points                        */
    pub char_specials: RefCell<CharSpecialData>,
    /* PC/NPC specials	  */
    pub player_specials: RefCell<PlayerSpecialData>,
    /* PC specials		  */
    pub mob_specials: MobSpecialData,
    /* NPC specials		  */
    pub affected: RefCell<Vec<AffectedType>>,
    /* affected by what spells       */
    pub equipment: RefCell<[Option<Rc<ObjData>>; NUM_WEARS as usize]>,
    /* Equipment array               */
    pub carrying: RefCell<Vec<Rc<ObjData>>>,
    /* Head of list                  */
    pub desc: RefCell<Option<Rc<DescriptorData>>>,
    /* NULL for mobiles              */
    pub next_in_room: RefCell<Option<Rc<CharData>>>,
    /* For room->people - list         */
    pub next: RefCell<Option<Rc<CharData>>>,
    /* For either monster or ppl-list  */
    // pub next_fighting: RefCell<Option<Rc<CharData>>>,
    /* For fighting list               */
    pub followers: RefCell<Vec<FollowType>>,
    /* List of chars followers       */
    pub master: RefCell<Option<Rc<CharData>>>,
    /* Who is char following?        */
}
/* ====================================================================== */

/* Char's points.  Used in char_file_u *DO*NOT*CHANGE* */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CharPointData {
    pub mana: i16,
    pub max_mana: i16,
    /* Max mana for PC/NPC			   */
    pub hit: i16,
    pub max_hit: i16,
    /* Max hit for PC/NPC                      */
    pub movem: i16,
    pub max_move: i16,
    /* Max move for PC/NPC                     */
    pub armor: i16,
    /* Internal -100..100, external -10..10 AC */
    pub gold: i32,
    /* Money carried                           */
    pub bank_gold: i32,
    /* Gold the char has in a bank account	   */
    pub exp: i32,
    /* The experience of the player            */
    pub hitroll: i8,
    /* Any bonus or penalty to the hit roll    */
    pub damroll: i8,
    /* Any bonus or penalty to the damage roll */
}

/* Structure used for chars following other chars */
#[derive(Clone)]
pub struct FollowType {
    pub follower: Rc<CharData>,
}

/* Special playing constants shared by PCs and NPCs which aren't in pfile */
pub struct CharSpecialData {
    pub fighting: Option<Rc<CharData>>,
    /* Opponent				*/
    pub hunting: Option<Rc<CharData>>,
    /* Char hunted by this char		*/
    pub position: u8,
    /* Standing, fighting, sleeping, etc.	*/
    pub carry_weight: i32,
    /* Carried weight			*/
    pub carry_items: u8,
    /* Number of items carried		*/
    pub timer: Cell<i32>,
    /* Timer for update			*/
    pub saved: CharSpecialDataSaved,
    /* constants saved in plrfile	*/
}

/*
 * CharSpecialDataSaved: specials which both a PC and an NPC have in
 * common, but which must be saved to the playerfile for PC's.
 *
 * WARNING:  Do not change this structure.  Doing so will ruin the
 * playerfile.  If you want to add to the playerfile, use the spares
 * in player_special_data.
 */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct CharSpecialDataSaved {
    pub alignment: i32,
    // +-1000 for alignments
    pub idnum: i64,
    /* player's idnum; -1 for mobiles	*/
    pub act: i64,
    /* act flag for NPC's; player flag for PC's */
    pub affected_by: i64,
    /* Bitvector for spells/skills affected by */
    pub apply_saving_throw: [i16; 5],
    /* Saving throw (Bonuses)		*/
}

/*
 * Specials needed only by PCs, not NPCs.  Space for this structure is
 * not allocated in memory for NPCs, but it is for PCs and the portion
 * of it labelled 'saved' is saved in the playerfile.  This structure can
 * be changed freely; beware, though, that changing the contents of
 * player_special_data_saved will corrupt the playerfile.
 */
pub struct PlayerSpecialData {
    pub saved: PlayerSpecialDataSaved,
    // char	*poofin;		/* Description on arrival of a god.     */
    // char	*poofout;		/* Description upon a god's exit.       */
    // struct alias_data *aliases;	/* Character's aliases			*/
    pub last_tell: i64,
    /* idnum of last tell from		*/
    // void *last_olc_targ;		/* olc control				*/
    // int last_olc_mode;		/* olc control				*/
}

/* Specials used by NPCs, not PCs */
pub struct MobSpecialData {
    pub memory: RefCell<Vec<i64>>,
    /* List of attackers to remember	       */
    pub attack_type: u8,
    /* The Attack Type Bitvector for NPC's     */
    pub default_pos: u8,
    /* Default position for NPC                */
    pub damnodice: u8,
    /* The number of damage dice's	       */
    pub damsizedice: u8,
    /* The size of the damage dice's           */
}

/*
 *  If you want to add new values to the playerfile, do it here.  DO NOT
 * ADD, DELETE OR MOVE ANY OF THE VARIABLES - doing so will change the
 * size of the structure and ruin the playerfile.  However, you can change
 * the names of the spares to something more meaningful, and then use them
 * in your new code.  They will automatically be transferred from the
 * playerfile into memory when players log in.
 */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct PlayerSpecialDataSaved {
    pub skills: [i8; MAX_SKILLS + 1],
    /* array of skills plus skill 0		*/
    pub(crate) padding0: i8,
    /* used to be spells_to_learn		*/
    pub talks: [bool; MAX_TONGUE],
    /* PC s Tongues 0 for NPC		*/
    pub wimp_level: i32,
    /* Below this # of hit points, flee!	*/
    pub freeze_level: i8,
    /* Level of god who froze char, if any	*/
    pub invis_level: i16,
    /* level of invisibility		*/
    pub load_room: RoomVnum,
    /* Which room to place char in		*/
    pub pref: i64,
    /*bitvector_t*/
    /* preference flags for PC's.		*/
    pub bad_pws: u8,
    /* number of bad password attemps	*/
    pub conditions: [i16; 3],
    /* Drunk, full, thirsty			*/

    /* spares below for future expansion.  You can change the names from
       'sparen' to something meaningful, but don't change the order.  */
    pub(crate) spare0: u8,
    pub(crate) spare1: u8,
    pub(crate) spare2: u8,
    pub(crate) spare3: u8,
    pub(crate) spare4: u8,
    pub(crate) spare5: u8,
    pub spells_to_learn: i32,
    /* How many can you learn yet this level*/
    pub(crate) spare7: i32,
    pub(crate) spare8: i32,
    pub(crate) spare9: i32,
    pub(crate) spare10: i32,
    pub(crate) spare11: i32,
    pub(crate) spare12: i32,
    pub(crate) spare13: i32,
    pub(crate) spare14: i32,
    pub(crate) spare15: i32,
    pub(crate) spare16: i32,
    pub(crate) spare17: i64,
    pub(crate) spare18: i64,
    pub(crate) spare19: i64,
    pub(crate) spare20: i64,
    pub(crate) spare21: i64,
}

/* This structure is purely intended to be an easy way to transfer */
/* and return information about time (real or mudwise).            */
pub struct TimeInfoData {
    pub hours: i32,
    pub day: i32,
    pub month: i32,
    pub year: i16,
}

/* general player-related info, usually PC's and NPC's */
pub struct CharPlayerData {
    pub passwd: [u8; MAX_PWD_LENGTH],
    /* character's password      */
    pub name: String,
    /* PC / NPC s name (kill ...  )         */
    pub short_descr: String,
    /* for NPC 'actions'                    */
    pub long_descr: String,
    /* for 'look'			       */
    pub description: String,
    /* Extra descriptions                   */
    pub title: Option<String>,
    /* PC / NPC's title                     */
    pub sex: u8,
    /* PC / NPC's sex                       */
    pub chclass: i8,
    /* PC / NPC's class		       */
    pub level: u8,
    /* PC / NPC's level                     */
    pub hometown: i16,
    /* PC s Hometown (zone)                 */
    pub time: TimeData,
    /* PC's AGE in days                 */
    pub weight: u8,
    /* PC / NPC's weight                    */
    pub height: u8,
    /* PC / NPC's height                    */
}

/* These data contain information about a players time data */
#[derive(Copy, Clone)]
pub struct TimeData {
    pub(crate) birth: u64,
    /* This represents the characters age                */
    pub(crate) logon: u64,
    /* Time of the last logon (used to calculate played) */
    pub(crate) played: i32,
    /* This is the total accumulated time played in secs */
}

pub const NOWHERE: i16 = -1;
pub const NOTHING: i16 = -1;
pub const NOBODY: i16 = -1;

/* PC classes */
pub const CLASS_UNDEFINED: i8 = -1;
pub const CLASS_MAGIC_USER: i8 = 0;
pub const CLASS_CLERIC: i8 = 1;
pub const CLASS_THIEF: i8 = 2;
pub const CLASS_WARRIOR: i8 = 3;

pub const NUM_CLASSES: i32 = 4; /* This must be the number of classes!! */

pub struct TxtBlock {
    pub text: String,
    pub aliased: bool,
}

/* Extra description: used in objects, mobiles, and rooms */
// struct ExtraDescrData {
//     keyword: String,                 /* Keyword in look/examine          */
//     description: String,             /* What to see                      */
//     next: Option<ExtraDescrData>, /* Next in list                     */
// }

/* object-related structures ******************************************/

/* object flags; used in obj_data */
pub struct ObjFlagData {
    pub value: [Cell<i32>; 4],
    /* Values of the item (see list)    */
    pub type_flag: u8,
    /* Type of item			    */
    pub wear_flags: i32,
    /* Where you can wear it	    */
    pub(crate) extra_flags: Cell<i32>,
    /* If it hums, glows, etc.	    */
    pub weight: Cell<i32>,
    /* Weigt what else                  */
    pub cost: i32,
    /* Value when sold (gp.)            */
    pub cost_per_day: i32,
    /* Cost to keep pr. real day        */
    pub timer: Cell<i32>,
    /* Timer for object                 */
    pub bitvector: Cell<i64>,
    /* To set chars bits                */
}

/* Used in ObjFileElem *DO*NOT*CHANGE* */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct ObjAffectedType {
    pub(crate) location: u8,
    /* Which ability to change (APPLY_XXX) */
    pub(crate) modifier: i8,
    /* How much it changes by              */
}

/* ================== Memory Structure for Objects ================== */
pub struct ObjData {
    pub item_number: ObjVnum,
    /* Where in data-base			*/
    pub in_room: Cell<RoomRnum>,
    /* In what room -1 when conta/carr	*/
    pub obj_flags: ObjFlagData,
    /* Object information               */
    pub affected: [Cell<ObjAffectedType>; MAX_OBJ_AFFECT as usize],
    /* affects */
    pub(crate) name: RefCell<String>,
    /* Title of object :get etc.        */
    pub description: String,
    /* When in room                     */
    pub(crate) short_description: String,
    /* when worn/carry/in cont.         */
    pub action_description: String,
    /* What to write when used          */
    pub ex_descriptions: Vec<ExtraDescrData>,
    /* extra descriptions     */
    pub carried_by: RefCell<Option<Rc<CharData>>>,
    /* Carried by :NULL in room/conta   */
    pub worn_by: RefCell<Option<Rc<CharData>>>,
    /* Worn by?			      */
    pub worn_on: Cell<i16>,
    /* Worn where?		      */
    pub in_obj: RefCell<Option<Rc<ObjData>>>,
    /* In what object NULL when none    */
    pub contains: RefCell<Vec<Rc<ObjData>>>,
    /* Contains objects                 */
    pub next_content: RefCell<Option<Rc<ObjData>>>,
    /* For 'contains' lists             */
    pub next: RefCell<Option<Rc<ObjData>>>,
    /* For the object list              */
}
/* ======================================================================= */

/* ==================== File Structure for Player ======================= */
/*             BEWARE: Changing it will ruin the playerfile		  */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct CharFileU {
    /* char_player_data */
    pub name: [u8; MAX_NAME_LENGTH + 1],
    pub description: [u8; EXDSCR_LENGTH],
    pub title: [u8; MAX_TITLE_LENGTH + 1],
    pub sex: u8,
    pub chclass: i8,
    pub level: u8,
    pub hometown: i16,
    pub birth: u64,
    /* Time of birth of character     */
    pub played: i32,
    /* Number of secs played in total */
    pub weight: u8,
    pub height: u8,

    pub pwd: [u8; MAX_PWD_LENGTH],
    /* character's password */
    pub char_specials_saved: CharSpecialDataSaved,
    pub player_specials_saved: PlayerSpecialDataSaved,
    pub abilities: CharAbilityData,
    pub points: CharPointData,
    pub affected: [AffectedType; MAX_AFFECT],

    pub last_logon: u64,
    /* Time (in secs) of last logon */
    pub host: [u8; HOST_LENGTH + 1],
    /* host of last logon */
}
/* ====================================================================== */

/* Char's abilities.  Used in char_file_u *DO*NOT*CHANGE* */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct CharAbilityData {
    pub(crate) str: i8,
    pub(crate) str_add: i8,
    /* 000 - 100 if strength 18             */
    pub(crate) intel: i8,
    pub(crate) wis: i8,
    pub(crate) dex: i8,
    pub(crate) con: i8,
    pub(crate) cha: i8,
}

/* An affect structure.  Used in char_file_u *DO*NOT*CHANGE* */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct AffectedType {
    pub _type: i16,
    /* The type of spell that caused this      */
    pub duration: i16,
    /* For how long its effects will last      */
    pub modifier: i8,
    /* This is added to apropriate ability     */
    pub location: u8,
    /* Tells which ability to change(APPLY_XXX)*/
    pub bitvector: i64,
    /* Tells which bits to set (AFF_XXX) */
}

/* ====================== File Element for Objects ======================= */
/*                 BEWARE: Changing it will ruin rent files		   */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct ObjFileElem {
    pub item_number: ObjVnum,
    pub location: i16,
    pub value: [i32; 4],
    pub extra_flags: i32,
    pub weight: i32,
    pub timer: i32,
    pub bitvector: i64,
    pub affected: [ObjAffectedType; MAX_OBJ_AFFECT as usize],
}

/* header block for rent files.  BEWARE: Changing it will ruin rent files  */
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct RentInfo {
    pub time: i32,
    pub rentcode: i32,
    pub net_cost_per_diem: i32,
    pub gold: i32,
    pub account: i32,
    pub nitems: i32,
    pub spare0: i32,
    pub spare1: i32,
    pub spare2: i32,
    pub spare3: i32,
    pub spare4: i32,
    pub spare5: i32,
    pub spare6: i32,
    pub spare7: i32,
}

pub type IDXTYPE = i16;

/* Various virtual (human-reference) number types. */
pub type RoomVnum = IDXTYPE;
pub type ObjVnum = IDXTYPE;
pub type MobVnum = IDXTYPE;
pub type ZoneVnum = IDXTYPE;
pub type ShopVnum = IDXTYPE;

/* Various real (array-reference) number types. */
pub type RoomRnum = IDXTYPE;
pub type ObjRnum = IDXTYPE;
pub type MobRnum = IDXTYPE;
pub type ZoneRnum = IDXTYPE;
pub type ShopRnum = IDXTYPE;

pub const SEX_NEUTRAL: u8 = 0;
pub const SEX_MALE: u8 = 1;
pub const SEX_FEMALE: u8 = 2;

/*
 * **DO**NOT** blindly change the number of levels in your MUD merely by
 * changing these numbers and without changing the rest of the code to match.
 * Other changes throughout the code are required.  See coding.doc for
 * details.
 *
 * LVL_IMPL should always be the HIGHEST possible immortal level, and
 * LVL_IMMORT should always be the LOWEST immortal level.  The number of
 * mortal levels will always be LVL_IMMORT - 1.
 */
pub const LVL_IMPL: i16 = 34;
pub const LVL_GRGOD: i16 = 33;
pub const LVL_GOD: i16 = 32;
pub const LVL_IMMORT: i16 = 31;

/* Level of the 'freeze' command */
pub const LVL_FREEZE: u8 = LVL_GRGOD as u8;

pub const NUM_OF_DIRS: usize = 6; /* number of directions in a room (nsewud) */
pub const MAGIC_NUMBER: u8 = 0x06; /* Arbitrary number that won't be in a string */

/* Affect bits: used in char_data.char_specials.saved.affected_by */
/* WARNING: In the world files, NEVER set the bits marked "R" ("Reserved") */
pub const AFF_BLIND: i64 = 1 << 0; /* (R) Char is blind		*/
pub const AFF_INVISIBLE: i64 = 1 << 1; /* Char is invisible		*/
pub const AFF_DETECT_ALIGN: i64 = 1 << 2; /* Char is sensitive to align*/
pub const AFF_DETECT_INVIS: i64 = 1 << 3; /* Char can see invis chars  */
pub const AFF_DETECT_MAGIC: i64 = 1 << 4; /* Char is sensitive to magic*/
pub const AFF_SENSE_LIFE: i64 = 1 << 5; /* Char can sense hidden life*/
pub const AFF_WATERWALK: i64 = 1 << 6; /* Char can walk on water	*/
pub const AFF_SANCTUARY: i64 = 1 << 7; /* Char protected by sanct.	*/
pub const AFF_GROUP: i64 = 1 << 8; /* (R) Char is grouped	*/
pub const AFF_CURSE: i64 = 1 << 9; /* Char is cursed		*/
pub const AFF_INFRAVISION: i64 = 1 << 10; /* Char can see in dark	*/
pub const AFF_POISON: i64 = 1 << 11; /* (R) Char is poisoned	*/
pub const AFF_PROTECT_EVIL: i64 = 1 << 12; /* Char protected from evil  */
pub const AFF_PROTECT_GOOD: i64 = 1 << 13; /* Char protected from good  */
pub const AFF_SLEEP: i64 = 1 << 14; /* (R) Char magically asleep	*/
pub const AFF_NOTRACK: i64 = 1 << 15; /* Char can't be tracked	*/
pub const AFF_UNUSED16: i64 = 1 << 16; /* Room for future expansion	*/
pub const AFF_UNUSED17: i64 = 1 << 17; /* Room for future expansion	*/
pub const AFF_SNEAK: i64 = 1 << 18; /* Char can move quietly	*/
pub const AFF_HIDE: i64 = 1 << 19; /* Char is hidden		*/
pub const AFF_UNUSED20: i64 = 1 << 20; /* Room for future expansion	*/
pub const AFF_CHARM: i64 = 1 << 21; /* Char is charmed		*/

/* Player flags: used by char_data.char_specials.act */
pub const PLR_KILLER: i64 = 1 << 0; /* Player is a player-killer		*/
pub const PLR_THIEF: i64 = 1 << 1; /* Player is a player-thief		*/
pub const PLR_FROZEN: i64 = 1 << 2; /* Player is frozen			*/
pub const PLR_DONTSET: i64 = 1 << 3; /* Don't EVER set (ISNPC bit)	*/
pub const PLR_WRITING: i64 = 1 << 4; /* Player writing (board/mail/olc)	*/
pub const PLR_MAILING: i64 = 1 << 5; /* Player is writing mail		*/
pub const PLR_CRASH: i64 = 1 << 6; /* Player needs to be crash-saved	*/
pub const PLR_SITEOK: i64 = 1 << 7; /* Player has been site-cleared	*/
pub const PLR_NOSHOUT: i64 = 1 << 8; /* Player not allowed to shout/goss	*/
pub const PLR_NOTITLE: i64 = 1 << 9; /* Player not allowed to set title	*/
pub const PLR_DELETED: i64 = 1 << 10; /* Player deleted - space reusable	*/
pub const PLR_LOADROOM: i64 = 1 << 11; /* Player uses nonstandard loadroom	*/
pub const PLR_NOWIZLIST: i64 = 1 << 12; /* Player shouldn't be on wizlist	*/
pub const PLR_NODELETE: i64 = 1 << 13; /* Player shouldn't be deleted	*/
pub const PLR_INVSTART: i64 = 1 << 14; /* Player should enter game wizinvis	*/
pub const PLR_CRYO: i64 = 1 << 15; /* Player is cryo-saved (purge prog)	*/
pub const PLR_NOTDEADYET: i64 = 1 << 16; /* (R) Player being extracted.	*/

/* Positions */
pub const POS_DEAD: u8 = 0; /* dead			*/
pub const POS_MORTALLYW: u8 = 1; /* mortally wounded	*/
pub const POS_INCAP: u8 = 2; /* incapacitated	*/
pub const POS_STUNNED: u8 = 3; /* stunned		*/
pub const POS_SLEEPING: u8 = 4; /* sleeping		*/
pub const POS_RESTING: u8 = 5; /* resting		*/
pub const POS_SITTING: u8 = 6; /* sitting		*/
pub const POS_FIGHTING: u8 = 7; /* fighting		*/
pub const POS_STANDING: u8 = 8; /* standing		*/

/* room-related structures ************************************************/

pub struct RoomDirectionData {
    pub general_description: String,
    /* When look DIR.			*/
    pub keyword: String,
    /* for open/close			*/
    pub exit_info: Cell<i16>,
    /* Exit info			*/
    pub key: ObjVnum,
    /* Key's number (-1 for no key)		*/
    pub to_room: Cell<RoomRnum>,
    /* Where direction leads (NOWHERE)	*/
}

/* ================== Memory Structure for room ======================= */
pub struct RoomData {
    pub number: RoomVnum,
    /* Rooms number	(vnum)		      */
    pub zone: ZoneRnum,
    /* Room zone (for resetting)          */
    pub sector_type: i32,
    /* sector type (move/hide)            */
    pub name: String,
    /* Rooms name 'You are ...'           */
    pub description: String,
    /* Shown when entered                 */
    pub ex_descriptions: Vec<ExtraDescrData>,
    /* for examine/look       */
    pub dir_option: [Option<Rc<RoomDirectionData>>; NUM_OF_DIRS],
    /* Directions */
    pub room_flags: Cell<i32>,
    /* DEATH,DARK ... etc */
    pub light: Cell<u8>,
    /* Number of lightsources in room     */
    pub func: Option<Special>,
    pub contents: RefCell<Vec<Rc<ObjData>>>,
    /* List of items in room              */
    pub peoples: RefCell<Vec<Rc<CharData>>>,
    /* List of NPC / PC in room           */
}
/* ====================================================================== */

/* Extra description: used in objects, mobiles, and rooms */
pub struct ExtraDescrData {
    pub keyword: String,
    /* Keyword in look/examine          */
    pub description: String,
    /* What to see                      */
    //  pub next: Option<Box<ExtraDescrData>>,
    /* Next in list                     */
}

pub struct MsgType {
    pub attacker_msg: Rc<str>,
    /* message to attacker */
    pub victim_msg: Rc<str>,
    /* message to victim   */
    pub room_msg: Rc<str>,
    /* message to room     */
}

pub struct MessageType {
    pub die_msg: MsgType,
    /* messages when death			*/
    pub miss_msg: MsgType,
    /* messages when miss			*/
    pub hit_msg: MsgType,
    /* messages when hit			*/
    pub god_msg: MsgType,
    /* messages when hit on god		*/
}

pub struct MessageList {
    pub a_type: i32,
    /* Attack type				*/
    //number_of_attacks;	/* How many attack messages to chose from. */
    pub messages: Vec<MessageType>,
    /* List of messages.			*/
}

pub struct DexSkillType {
    pub p_pocket: i16,
    pub p_locks: i16,
    pub traps: i16,
    pub sneak: i16,
    pub hide: i16,
}

pub struct DexAppType {
    pub reaction: i16,
    pub miss_att: i16,
    pub defensive: i16,
}

pub struct StrAppType {
    pub tohit: i16,
    /* To Hit (THAC0) Bonus/Penalty        */
    pub todam: i16,
    /* Damage Bonus/Penalty                */
    pub carry_w: i16,
    /* Maximum weight that can be carrried */
    pub wield_w: i16,
    /* Maximum weight that can be wielded  */
}

pub struct WisAppType {
    pub(crate) bonus: u8,
    /* how many practices player gains per lev */
}

pub struct IntAppType {
    pub learn: u8,
    /* how many % a player learns a spell/skill */
}

pub struct ConAppType {
    pub hitp: i16,
    pub shock: i16,
}

pub struct WeatherData {
    pub pressure: i32,
    /* How is the pressure ( Mb ) */
    pub change: i32,
    /* How fast and what way does it change. */
    pub sky: i32,
    /* How is the sky. */
    pub sunlight: i32,
    /* And how much sun. */
}

/*
 * Element in monster and object index-tables.
 *
 * NOTE: Assumes sizeof(mob_vnum) >= sizeof(ObjVnum)
 */
pub struct IndexData {
    pub vnum: MobVnum,
    /* virtual number of this mob/obj		*/
    pub number: Cell<i32>,
    /* number of existing units of this mob/obj	*/
    pub func: Option<Special>,
}

impl ExtraDescrData {
    pub fn new() -> ExtraDescrData {
        ExtraDescrData {
            keyword: "".to_string(),
            description: "".to_string(),
        }
    }
}

pub struct GuildInfoType {
    pub pc_class: i8,
    pub guild_room: RoomVnum,
    pub direction: i32,
}

/* Some different kind of liquids for use in values of drink containers */
pub const LIQ_WATER: i32 = 0;
pub const LIQ_BEER: i32 = 1;
pub const LIQ_WINE: i32 = 2;
pub const LIQ_ALE: i32 = 3;
pub const LIQ_DARKALE: i32 = 4;
pub const LIQ_WHISKY: i32 = 5;
pub const LIQ_LEMONADE: i32 = 6;
pub const LIQ_FIREBRT: i32 = 7;
pub const LIQ_LOCALSPC: i32 = 8;
pub const LIQ_SLIME: i32 = 9;
pub const LIQ_MILK: i32 = 10;
pub const LIQ_TEA: i32 = 11;
pub const LIQ_COFFE: i32 = 12;
pub const LIQ_BLOOD: i32 = 13;
pub const LIQ_SALTWATER: i32 = 14;
pub const LIQ_CLEARWATER: i32 = 15;
