use crate::DescriptorData;
use std::cell::RefCell;
use std::rc::Rc;

pub const OPT_USEC: u32 = 100000;
pub const PASSES_PER_SEC: u32 = 1000000 / OPT_USEC;

#[derive(PartialEq, Debug)]
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
pub const MAX_PWD_LENGTH: usize = 10; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_TITLE_LENGTH: i32 = 80; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const HOST_LENGTH: i32 = 30; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const EXDSCR_LENGTH: i32 = 240; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_TONGUE: i32 = 3; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_SKILLS: i32 = 200; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_AFFECT: i32 = 32; /* Used in char_file_u *DO*NOT*CHANGE* */
pub const MAX_OBJ_AFFECT: i32 = 6; /* Used in obj_file_elem *DO*NOT*CHANGE* */
pub const MAX_NOTE_LENGTH: i32 = 1000; /* arbitrary */

/* ================== Structure for player/non-player ===================== */
pub struct CharData<'a> {
    // int pfilepos;			 /* playerfile pos		  */
    // mob_rnum nr;                          /* Mob's rnum			  */
    // room_rnum in_room;                    /* Location (real room number)	  */
    // room_rnum was_in_room;		 /* location for linkdead people  */
    pub wait: i32, /* wait for how many loops	  */
    //
    pub player: CharPlayerData, /* Normal data                   */
    // struct char_ability_data real_abils;	 /* Abilities without modifiers   */
    // struct char_ability_data aff_abils;	 /* Abils with spells/stones/etc  */
    pub points: CharPointData,
    /* Points                        */
    pub char_specials: CharSpecialData<'a>,
    /* PC/NPC specials	  */
    pub player_specials: Option<PlayerSpecialData>,
    /* PC specials		  */
    // struct mob_special_data mob_specials;	/* NPC specials		  */
    //
    // struct affected_type *affected;       /* affected by what spells       */
    // struct obj_data *equipment[NUM_WEARS];/* Equipment array               */
    //
    // struct obj_data *carrying;            /* Head of list                  */
    pub(crate) desc: Rc<RefCell<DescriptorData<'a>>>, /* NULL for mobiles              */
                                                      //
                                                      // struct CharData *next_in_room;     /* For room->people - list         */
                                                      // struct CharData *next;             /* For either monster or ppl-list  */
                                                      // struct CharData *next_fighting;    /* For fighting list               */
                                                      //
                                                      // struct follow_type *followers;        /* List of chars followers       */
                                                      // struct CharData *master;             /* Who is char following?        */
}
/* ====================================================================== */

/* Char's points.  Used in char_file_u *DO*NOT*CHANGE* */
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

/* Special playing constants shared by PCs and NPCs which aren't in pfile */
pub struct CharSpecialData<'a> {
    pub fighting: Option<Rc<RefCell<CharData<'a>>>>,
    /* Opponent				*/
    pub hunting: Option<Rc<RefCell<CharData<'a>>>>,
    /* Char hunted by this char		*/

    // byte position;		/* Standing, fighting, sleeping, etc.	*/
    //
    // int	carry_weight;		/* Carried weight			*/
    // byte carry_items;		/* Number of items carried		*/
    pub(crate) timer: i32, /* Timer for update			*/
    //
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
pub struct CharSpecialDataSaved {
    pub alignment: i32,
    // +-1000 for alignments
    pub idnum: i64,
    /* player's idnum; -1 for mobiles	*/
    pub act: i64,
    /* act flag for NPC's; player flag for PC's */
    pub affected_by: i64,
    /* Bitvector for spells/skills affected by */
    // sh_int apply_saving_throw[5]; /* Saving throw (Bonuses)		*/
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
    // long last_tell;		/* idnum of last tell from		*/
    // void *last_olc_targ;		/* olc control				*/
    // int last_olc_mode;		/* olc control				*/
}

/*
 *  If you want to add new values to the playerfile, do it here.  DO NOT
 * ADD, DELETE OR MOVE ANY OF THE VARIABLES - doing so will change the
 * size of the structure and ruin the playerfile.  However, you can change
 * the names of the spares to something more meaningful, and then use them
 * in your new code.  They will automatically be transferred from the
 * playerfile into memory when players log in.
 */
pub struct PlayerSpecialDataSaved {
    //byte skills[MAX_SKILLS+1];	/* array of skills plus skill 0		*/
    pub(crate) padding0: i8,
    /* used to be spells_to_learn		*/
    //bool talks[MAX_TONGUE];	/* PC s Tongues 0 for NPC		*/
    //int	wimp_level;		/* Below this # of hit points, flee!	*/
    pub(crate) freeze_level: i8,
    /* Level of god who froze char, if any	*/
    pub invis_level: i16,
    /* level of invisibility		*/
    //room_vnum load_room;		/* Which room to place char in		*/
    pub pref: i64, /*bitvector_t*/
                   /* preference flags for PC's.		*/
                   //ubyte bad_pws;		/* number of bad password attemps	*/
                   //sbyte conditions[3];         /* Drunk, full, thirsty			*/

                   /* spares below for future expansion.  You can change the names from
                      'sparen' to something meaningful, but don't change the order.  */

                   // ubyte spare0;
                   // ubyte spare1;
                   // ubyte spare2;
                   // ubyte spare3;
                   // ubyte spare4;
                   // ubyte spare5;
                   // int spells_to_learn;		/* How many can you learn yet this level*/
                   // int spare7;
                   // int spare8;
                   // int spare9;
                   // int spare10;
                   // int spare11;
                   // int spare12;
                   // int spare13;
                   // int spare14;
                   // int spare15;
                   // int spare16;
                   // long	spare17;
                   // long	spare18;
                   // long	spare19;
                   // long	spare20;
                   // long	spare21;
}

/* general player-related info, usually PC's and NPC's */
pub struct CharPlayerData {
    // char	passwd[MAX_PWD_LENGTH+1]; /* character's password      */
    pub name: String, /* PC / NPC s name (kill ...  )         */
    pub short_descr: String, /* for NPC 'actions'                    */
                      // char	*long_descr;   /* for 'look'			       */
                      // char	*description;  /* Extra descriptions                   */
                      // char	*title;        /* PC / NPC's title                     */
                      // byte sex;           /* PC / NPC's sex                       */
                      // byte chclass;       /* PC / NPC's class		       */
                      // byte level;         /* PC / NPC's level                     */
                      // sh_int hometown;    /* PC s Hometown (zone)                 */
                      // struct time_data time;  /* PC's AGE in days                 */
                      // ubyte weight;       /* PC / NPC's weight                    */
                      // ubyte height;       /* PC / NPC's height                    */
}

/* PC classes */
pub const CLASS_UNDEFINED: i32 = -1;
pub const CLASS_MAGIC_USER: i32 = 0;
pub const CLASS_CLERIC: i32 = 1;
pub const CLASS_THIEF: i32 = 2;
pub const CLASS_WARRIOR: i32 = 3;

pub const NUM_CLASSES: i32 = 4; /* This must be the number of classes!! */

pub struct TxtBlock {
    pub text: String,
    pub aliased: bool,
}
