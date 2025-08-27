pub const KNRM: &str = "\x1B[0m";
pub const KRED: &str = "\x1B[31m";
pub const KGRN: &str = "\x1B[32m";
pub const KYEL: &str = "\x1B[33m";
// pub const KBLU: &str = "\x1B[34m";
pub const KMAG: &str = "\x1B[35m";
pub const KCYN: &str = "\x1B[36m";
// pub const KWHT: &str = "\x1B[37m";
pub const KNUL: &str = "";

/* conditional color.  pass it a pointer to a char_data and a color level. */
pub const C_OFF: u8 = 0;
pub const C_SPR: u8 = 1;
pub const C_NRM: u8 = 2;
pub const C_CMP: u8 = 3;
// #define _clrlevel(ch) (!IS_NPC(ch) ? (PRF_FLAGGED((ch), PRF_COLOR_1) ? 1 : 0) + \
// (PRF_FLAGGED((ch), PRF_COLOR_2) ? 2 : 0) : 0)
#[macro_export]
macro_rules! _clrlevel {
    ($ch:expr) => {
        (if !($ch).is_npc() {
            (if ($ch).prf_flagged(crate::structs::PRF_COLOR_1) { 1 } else { 0 })
                + (if ($ch).prf_flagged(crate::structs::PRF_COLOR_2) { 2 } else { 0 })
        } else {
            0
        })
    };
}
//#define clr(ch,lvl) (_clrlevel(ch) >= (lvl))
#[macro_export]
macro_rules! clr {
    ($ch:expr,$lvl:expr) => {
        (_clrlevel!($ch) >= ($lvl))
    };
}

#[macro_export]
macro_rules! CCNRM {
    ($ch:expr,$lvl:expr) => {
        (if clr!(($ch), ($lvl)) { KNRM } else { KNUL })
    };
}
#[macro_export]
macro_rules! CCRED {
    ($ch:expr,$lvl:expr) => {
        (if clr!(($ch), ($lvl)) { KRED } else { KNUL })
    };
}
#[macro_export]
macro_rules! CCGRN {
    ($ch:expr,$lvl:expr) => {
        (if clr!(($ch), ($lvl)) { KGRN } else { KNUL })
    };
}
#[macro_export]
macro_rules! CCYEL {
    ($ch:expr,$lvl:expr) => {
        (if clr!(($ch), ($lvl)) { KYEL } else { KNUL })
    };
}
#[macro_export]
macro_rules! CCBLU {
    ($ch:expr,$lvl:expr) => {
        (if clr!(($ch), ($lvl)) { KBLU } else { KNUL })
    };
}
#[macro_export]
macro_rules! CCMAG {
    ($ch:expr,$lvl:expr) => {
        (if clr!(($ch), ($lvl)) { KMAG } else { KNUL })
    };
}
#[macro_export]
macro_rules! CCCYN {
    ($ch:expr,$lvl:expr) => {
        (if clr!(($ch), ($lvl)) { KCYN } else { KNUL })
    };
}
#[macro_export]
macro_rules! CCWHT {
    ($ch:expr,$lvl:expr) => {
        (if clr!(($ch), ($lvl)) { KWHT } else { KNUL })
    };
}

#[macro_export]
macro_rules! COLOR_LEV {
    ($ch:expr) => {
        (_clrlevel!($ch))
    };
}

// #define QNRM CCNRM(ch,C_SPR)
// #define QRED CCRED(ch,C_SPR)
// #define QGRN CCGRN(ch,C_SPR)
// #define QYEL CCYEL(ch,C_SPR)
// #define QBLU CCBLU(ch,C_SPR)
// #define QMAG CCMAG(ch,C_SPR)
// #define QCYN CCCYN(ch,C_SPR)
// #define QWHT CCWHT(ch,C_SPR)
