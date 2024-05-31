/* ************************************************************************
*   File: fight.rs                                      Part of CircleMUD *
*  Usage: Combat system                                                   *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023 Laurent Pautet                            *
************************************************************************ */

use log::error;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::rc::Rc;

use crate::act_offensive::do_flee;
use crate::act_social::fread_action;
use crate::class::{backstab_mult, thaco};
use crate::config::{
    MAX_EXP_GAIN, MAX_EXP_LOSS, MAX_NPC_CORPSE_TIME, MAX_PC_CORPSE_TIME, PK_ALLOWED,
};
use crate::constants::{DEX_APP, STR_APP};
use crate::db::{DB, MESS_FILE};
use crate::handler::{affect_from_char, affect_remove, affected_by_spell, object_list_new_owner};
use crate::limits::gain_exp;
use crate::mobact::{forget, remember};
use crate::screen::{C_CMP, C_SPR, KNRM, KNUL, KRED, KYEL};
use crate::shops::ok_damage_shopkeeper;
use crate::spells::{
    AttackHitType, SKILL_BACKSTAB, SPELL_INVISIBLE, SPELL_SLEEP, TYPE_HIT, TYPE_SUFFERING,
    TYPE_UNDEFINED,
};
use crate::structs::{
    CharData, MessageList, MessageType, MsgType, ObjData, AFF_GROUP, AFF_HIDE, AFF_INVISIBLE,
    AFF_SANCTUARY, AFF_SLEEP, ITEM_CONTAINER, ITEM_NODONATE, ITEM_WEAPON, ITEM_WEAR_TAKE,
    LVL_IMMORT, MOB_MEMORY, MOB_NOTDEADYET, MOB_SPEC, MOB_WIMPY, NOTHING, NOWHERE, NUM_OF_DIRS,
    NUM_WEARS, PLR_KILLER, PLR_NOTDEADYET, PLR_THIEF, POS_DEAD, POS_FIGHTING, POS_INCAP,
    POS_MORTALLYW, POS_STANDING, POS_STUNNED, PRF_COLOR_1, PRF_COLOR_2, PULSE_VIOLENCE,
    ROOM_PEACEFUL, WEAR_WIELD,
};
use crate::util::{dice, rand_number, BRF};
use crate::{
    _clrlevel, clr, send_to_char, Game, CCNRM, CCRED, CCYEL, TO_CHAR, TO_NOTVICT, TO_ROOM,
    TO_SLEEP, TO_VICT,
};

/* Weapon attack texts */
pub const ATTACK_HIT_TEXT: [AttackHitType; 15] = [
    AttackHitType {
        singular: "hit",
        plural: "hits",
    }, /* 0 */
    AttackHitType {
        singular: "sting",
        plural: "stings",
    },
    AttackHitType {
        singular: "whip",
        plural: "whips",
    },
    AttackHitType {
        singular: "slash",
        plural: "slashes",
    },
    AttackHitType {
        singular: "bite",
        plural: "bites",
    },
    AttackHitType {
        singular: "bludgeon",
        plural: "bludgeons",
    }, /* 5 */
    AttackHitType {
        singular: "crush",
        plural: "crushes",
    },
    AttackHitType {
        singular: "pound",
        plural: "pounds",
    },
    AttackHitType {
        singular: "claw",
        plural: "claws",
    },
    AttackHitType {
        singular: "maul",
        plural: "mauls",
    },
    AttackHitType {
        singular: "thrash",
        plural: "thrashes",
    }, /* 10 */
    AttackHitType {
        singular: "pierce",
        plural: "pierces",
    },
    AttackHitType {
        singular: "blast",
        plural: "blasts",
    },
    AttackHitType {
        singular: "punch",
        plural: "punches",
    },
    AttackHitType {
        singular: "stab",
        plural: "stabs",
    },
];

macro_rules! is_weapon {
    ($type:expr) => {
        (($type) >= TYPE_HIT && ($type) < TYPE_SUFFERING)
    };
}

/* The Fight related routines */
impl DB {
    pub fn appear(&self, ch: &CharData) {
        if affected_by_spell(ch, SPELL_INVISIBLE as i16) {
            affect_from_char(ch, SPELL_INVISIBLE as i16);
        }

        ch.remove_aff_flags(AFF_INVISIBLE | AFF_HIDE);

        if ch.get_level() < LVL_IMMORT as u8 {
            self.act(
                "$n slowly fades into existence.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        } else {
            self.act(
                "You feel a strange presence as $n appears, seemingly from nowhere.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        }
    }
}

pub fn compute_armor_class(ch: &CharData) -> i16 {
    let mut armorclass = ch.get_ac();

    if ch.awake() {
        armorclass += DEX_APP[ch.get_dex() as usize].defensive * 10;
    }

    return max(-100, armorclass); /* -100 is lowest */
}

pub fn free_messages(db: &mut DB) {
    db.fight_messages.clear();
}

impl DB {
    pub fn load_messages(&mut self) {
        let fl = OpenOptions::new()
            .read(true)
            .open(MESS_FILE)
            .expect(format!("SYSERR: Error #1 reading combat message file{}", MESS_FILE).as_str());
        let mut reader = BufReader::new(fl);
        let mut buf = String::new();
        let mut r = reader
            .read_line(&mut buf)
            .expect(format!("SYSERR: Error #2 reading combat message file{}", MESS_FILE).as_str());

        while r != 0 && (buf.starts_with('\n') || buf.starts_with('*')) {
            r = reader.read_line(&mut buf).expect(
                format!("SYSERR: Error #3 reading combat message file{}", MESS_FILE).as_str(),
            );
        }

        let mut a_type;
        while buf.starts_with('M') {
            buf.clear();
            reader.read_line(&mut buf).expect(
                format!("SYSERR: Error #3 reading combat message file{}", MESS_FILE).as_str(),
            );
            a_type = buf.trim().parse::<i32>().expect(
                format!("SYSERR: Error #4 reading combat message file{}", MESS_FILE).as_str(),
            );

            let fml = self
                .fight_messages
                .iter()
                .position(|fm| fm.a_type == a_type);
            let ml;
            let i;
            if fml.is_none() {
                let nml = MessageList {
                    a_type,
                    messages: vec![],
                };
                i = self.fight_messages.len();
                self.fight_messages.push(nml);
            } else {
                i = fml.unwrap();
            }
            ml = &mut self.fight_messages[i];
            let i = i as i32;
            let msg = MessageType {
                die_msg: MsgType {
                    attacker_msg: fread_action(&mut reader, i),
                    victim_msg: fread_action(&mut reader, i),
                    room_msg: fread_action(&mut reader, i),
                },
                miss_msg: MsgType {
                    attacker_msg: fread_action(&mut reader, i),
                    victim_msg: fread_action(&mut reader, i),
                    room_msg: fread_action(&mut reader, i),
                },
                hit_msg: MsgType {
                    attacker_msg: fread_action(&mut reader, i),
                    victim_msg: fread_action(&mut reader, i),
                    room_msg: fread_action(&mut reader, i),
                },
                god_msg: MsgType {
                    attacker_msg: fread_action(&mut reader, i),
                    victim_msg: fread_action(&mut reader, i),
                    room_msg: fread_action(&mut reader, i),
                },
            };
            ml.messages.push(msg);

            let mut r = reader.read_line(&mut buf).expect(
                format!("SYSERR: Error #2 reading combat message file{}", MESS_FILE).as_str(),
            );

            while r != 0 && (buf.starts_with('\n') || buf.starts_with('*')) {
                r = reader.read_line(&mut buf).expect(
                    format!("SYSERR: Error #3 reading combat message file{}", MESS_FILE).as_str(),
                );
            }
        }
    }
}

pub fn update_pos(victim: &CharData) {
    if victim.get_hit() > 0 && victim.get_pos() > POS_STUNNED {
        return;
    } else if victim.get_hit() > 0 {
        victim.set_pos(POS_STANDING);
    } else if victim.get_hit() <= -11 {
        victim.set_pos(POS_DEAD);
    } else if victim.get_hit() <= -6 {
        victim.set_pos(POS_MORTALLYW);
    } else if victim.get_hit() <= -3 {
        victim.set_pos(POS_INCAP);
    } else {
        victim.set_pos(POS_STUNNED);
    }
}

pub fn check_killer(ch: &CharData, vict: &CharData, game: &Game) {
    if vict.plr_flagged(PLR_KILLER) || vict.plr_flagged(PLR_THIEF) {
        return;
    }
    if ch.plr_flagged(PLR_KILLER) || ch.is_npc() || vict.is_npc() || std::ptr::eq(ch, vict) {
        return;
    }

    ch.set_plr_flag_bit(PLR_KILLER);

    send_to_char(ch, "If you want to be a PLAYER KILLER, so be it...\r\n");
    game.mudlog(
        BRF,
        LVL_IMMORT as i32,
        true,
        format!(
            "PC Killer bit set on {} for initiating attack on {} at {}.",
            ch.get_name(),
            vict.get_name(),
            game.db.world[vict.in_room() as usize].name
        )
        .as_str(),
    );
}

/* start one char fighting another (yes, it is horrible, I know... )  */
impl DB {
    pub(crate) fn set_fighting(&self, ch: &Rc<CharData>, vict: &Rc<CharData>, game: &Game) {
        if Rc::ptr_eq(ch, vict) {
            return;
        }

        if ch.fighting().is_some() {
            error!("Unexpected error in set_fighting!");
            return;
        }

        self.combat_list.borrow_mut().push(ch.clone());

        if ch.aff_flagged(AFF_SLEEP) {
            affect_from_char(ch, SPELL_SLEEP as i16);
        }

        ch.set_fighting(Some(vict.clone()));
        ch.set_pos(POS_FIGHTING);

        if !PK_ALLOWED {
            check_killer(ch, vict, game);
        }
    }
    
    /* remove a char from the list of fighting chars */
    pub fn stop_fighting(&mut self, ch: &Rc<CharData>) {
        self.combat_list.borrow_mut().retain(|c| !Rc::ptr_eq(c, ch));
        ch.set_fighting(None);
        ch.set_pos(POS_STANDING);

        update_pos(ch);
    }

    pub fn make_corpse(&mut self, ch: &Rc<CharData>) {
        let mut corpse = ObjData::new();

        corpse.item_number = NOTHING;
        corpse.set_in_room(NOWHERE);
        corpse.name = RefCell::from("corpse".to_string());

        let buf2 = format!("The corpse of {} is lying here.", ch.get_name());
        corpse.description = buf2;

        let buf2 = format!("the corpse of {}", ch.get_name());
        corpse.short_description = buf2;

        corpse.set_obj_type(ITEM_CONTAINER);
        corpse.set_obj_wear(ITEM_WEAR_TAKE);
        corpse.set_obj_extra(ITEM_NODONATE);
        corpse.set_obj_val(0, 0); /* You can't store stuff in a corpse */
        corpse.set_obj_val(3, 1); /* corpse identifier */
        corpse.set_obj_weight(ch.get_weight() as i32 + ch.is_carrying_w() );
        corpse.set_obj_rent(100000);
        if ch.is_npc() {
            corpse.set_obj_timer(MAX_NPC_CORPSE_TIME);
        } else {
            corpse.set_obj_timer(MAX_PC_CORPSE_TIME);
        }

        let corpse = Rc::from(corpse);
        self.object_list.push(corpse.clone());

        /* transfer character's inventory to the corpse */
        for o in ch.carrying.borrow().iter() {
            corpse.contains.borrow_mut().push(o.clone());
        }
        for o in corpse.contains.borrow().iter() {
            *o.in_obj.borrow_mut() = Some(corpse.clone());
            object_list_new_owner(&corpse, None);
        }
        /* transfer character's equipment to the corpse */
        for i in 0..NUM_WEARS {
            if ch.get_eq(i).is_some() {
                self.obj_to_obj(self.unequip_char(ch, i).as_ref().unwrap(), &corpse);
            }
        }
        /* transfer gold */
        if ch.get_gold() > 0 {
            /*
             * following 'if' clause added to fix gold duplication loophole
             * The above line apparently refers to the old "partially log in,
             * kill the game character, then finish login sequence" duping
             * bug. The duplication has been fixed (knock on wood) but the
             * test below shall live on, for a while. -gg 3/3/2002
             */
            if ch.is_npc() || ch.desc.borrow().is_some() {
                let money = self.create_money(ch.get_gold());
                self.obj_to_obj(money.as_ref().unwrap(), &corpse);
            }
            ch.set_gold(0);
        }
        ch.carrying.borrow_mut().clear();
        ch.set_is_carrying_w(0);
        ch.set_is_carrying_n(0);

        self.obj_to_room(&corpse, ch.in_room());
    }
}

/* When ch kills victim */
pub fn change_alignment(ch: &CharData, victim: &CharData) {
    /*
     * new alignment change algorithm: if you kill a monster with alignment A,
     * you move 1/16th of the way to having alignment -A.  Simple and fast.
     */
    ch.set_alignment(ch.get_alignment() + (-victim.get_alignment() - ch.get_alignment()) / 16);
}

impl DB {
    pub fn death_cry(&self, ch: &CharData) {
        self.act(
            "Your blood freezes as you hear $n's death cry.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        for door in 0..NUM_OF_DIRS {
            if self.can_go(ch, door) {
                self.send_to_room(
                    self.world[ch.in_room() as usize].dir_option[door]
                        .as_ref()
                        .unwrap()
                        .to_room
                        .get(),
                    "Your blood freezes as you hear someone's death cry.\r\n",
                );
            }
        }
    }

    pub fn raw_kill(&mut self, ch: &Rc<CharData>) {
        if ch.fighting().is_some() {
            self.stop_fighting(ch);
        }

        ch.affected.borrow_mut().retain(|af| {
            affect_remove(ch, af);
            false
        });

        self.death_cry(ch);

        self.make_corpse(ch);
        self.extract_char(ch);
    }
}

pub fn die(ch: &Rc<CharData>, game: &mut Game) {
    gain_exp(ch, -(ch.get_exp() / 2), game);
    if !ch.is_npc() {
        ch.remove_plr_flag(PLR_KILLER | PLR_THIEF);
    }
    game.db.raw_kill(ch);
}

pub fn perform_group_gain(ch: &Rc<CharData>, base: i32, victim: &Rc<CharData>, game: &Game) {
    let share = min(MAX_EXP_GAIN, max(1, base));

    if share > 1 {
        send_to_char(
            ch,
            format!(
                "You receive your share of experience -- {} points.\r\n",
                share
            )
            .as_str(),
        );
    } else {
        send_to_char(
            ch,
            "You receive your share of experience -- one measly little point!\r\n",
        );
    }
    gain_exp(ch, share, game);
    change_alignment(ch, victim);
}

pub fn group_gain(ch: &Rc<CharData>, victim: &Rc<CharData>, game: &Game) {
    let k;
    if ch.master.borrow().is_none() {
        k = ch.clone();
    } else {
        k = ch.master.borrow().as_ref().unwrap().clone();
    }
    let mut tot_members;
    if k.aff_flagged(AFF_GROUP) && k.in_room() == ch.in_room() {
        tot_members = 1;
    } else {
        tot_members = 0;
    }

    for f in k.followers.borrow().iter() {
        if f.follower.aff_flagged(AFF_GROUP) && f.follower.in_room() == ch.in_room() {
            tot_members += 1;
        }
    }

    /* round up to the next highest tot_members */
    let mut tot_gain = (victim.get_exp() / 3) + tot_members - 1;

    /* prevent illegal xp creation when killing players */
    if !victim.is_npc() {
        tot_gain = min(MAX_EXP_LOSS * 2 / 3, tot_gain);
    }

    let base;
    if tot_members >= 1 {
        base = max(1, tot_gain / tot_members);
    } else {
        base = 0;
    }

    if k.aff_flagged(AFF_GROUP) && k.in_room() == ch.in_room() {
        perform_group_gain(&k, base, victim, game);
    }

    for f in k.followers.borrow().iter() {
        if f.follower.aff_flagged(AFF_GROUP) && f.follower.in_room() == ch.in_room() {
            perform_group_gain(&f.follower, base, victim, game);
        }
    }
}

pub fn solo_gain(ch: &Rc<CharData>, victim: &Rc<CharData>, game: &Game) {
    let mut exp = min(MAX_EXP_GAIN, victim.get_exp() / 3);

    /* Calculate level-difference bonus */
    if ch.is_npc() {
        exp += max(
            0,
            exp * min(4, victim.get_level() as i32 - ch.get_level() as i32) / 8,
        );
    } else {
        exp += max(
            0,
            exp * min(8, victim.get_level() as i32 - ch.get_level() as i32) / 8,
        );
    }
    exp = max(exp, 1);

    if exp > 1 {
        send_to_char(
            ch,
            format!("You receive {} experience points.\r\n", exp).as_str(),
        );
    } else {
        send_to_char(ch, "You receive one lousy experience point.\r\n");
    }
    gain_exp(ch, exp, game);
    change_alignment(ch, victim);
}

pub fn replace_string(str: &str, weapon_singular: &str, weapon_plural: &str) -> String {
    let mut buf = String::new();

    let mut iter = str.chars();
    loop {
        let c = iter.next();
        if c.is_none() {
            break;
        }
        let mut c = c.unwrap();
        if c == '#' {
            c = iter.next().unwrap();
            match c {
                'W' => {
                    buf.push_str(weapon_plural);
                }
                'w' => {
                    buf.push_str(weapon_singular);
                }
                _ => {
                    buf.push('#');
                }
            }
        } else {
            buf.push(c);
        }
    } /* For */
    buf.clone()
}

impl DB {
    /* message for doing damage with a weapon */
    pub fn dam_message(&self, dam: i32, ch: &CharData, victim: &CharData, mut w_type: i32) {
        struct DamWeaponType {
            to_room: &'static str,
            to_char: &'static str,
            to_victim: &'static str,
        }
        const DAM_WEAPONS: [DamWeaponType; 9] = [
            /* use #w for singular (i.e. "slash") and #W for plural (i.e. "slashes") */
            DamWeaponType {
                to_room: "$n tries to #w $N, but misses.",
                to_char: "You try to #w $N, but miss.",
                to_victim: "$n tries to #w you, but misses.",
            }, /* 0: 0     */
            DamWeaponType {
                to_room: "$n tickles $N as $e #W $M.",
                to_char: "You tickle $N as you #w $M.",
                to_victim: "$n tickles you as $e #W you.",
            }, /* 1: 1..2  */
            DamWeaponType {
                to_room: "$n barely #W $N.",
                to_char: "You barely #w $N.",
                to_victim: "$n barely #W you.",
            }, /* 2: 3..4  */
            DamWeaponType {
                to_room: "$n #W $N.",
                to_char: "You #w $N.",
                to_victim: "$n #W you.",
            }, /* 3: 5..6  */
            DamWeaponType {
                to_room: "$n #W $N hard.",
                to_char: "You #w $N hard.",
                to_victim: "$n #W you hard.",
            }, /* 4: 7..10  */
            DamWeaponType {
                to_room: "$n #W $N very hard.",
                to_char: "You #w $N very hard.",
                to_victim: "$n #W you very hard.",
            }, /* 5: 11..14  */
            DamWeaponType {
                to_room: "$n #W $N extremely hard.",
                to_char: "You #w $N extremely hard.",
                to_victim: "$n #W you extremely hard.",
            }, /* 6: 15..19  */
            DamWeaponType {
                to_room: "$n massacres $N to small fragments with $s #w.",
                to_char: "You massacre $N to small fragments with your #w.",
                to_victim: "$n massacres you to small fragments with $s #w.",
            }, /* 7: 19..23 */
            DamWeaponType {
                to_room: "$n OBLITERATES $N with $s deadly #w!!",
                to_char: "You OBLITERATE $N with your deadly #w!!",
                to_victim: "$n OBLITERATES you with $s deadly #w!!",
            }, /* 8: > 23   */
        ];

        w_type -= TYPE_HIT; /* Change to base of table with text */
        let w_type = w_type as usize;
        let msgnum;
        if dam == 0 {
            msgnum = 0;
        } else if dam <= 2 {
            msgnum = 1;
        } else if dam <= 4 {
            msgnum = 2;
        } else if dam <= 6 {
            msgnum = 3;
        } else if dam <= 10 {
            msgnum = 4;
        } else if dam <= 14 {
            msgnum = 5;
        } else if dam <= 19 {
            msgnum = 6;
        } else if dam <= 23 {
            msgnum = 7;
        } else {
            msgnum = 8
        };

        /* damage message to onlookers */
        let buf = replace_string(
            DAM_WEAPONS[msgnum].to_room,
            ATTACK_HIT_TEXT[w_type].singular,
            ATTACK_HIT_TEXT[w_type].plural,
        );
        self.act(&buf, false, Some(ch), None, Some(victim), TO_NOTVICT);

        /* damage message to damager */
        send_to_char(ch, CCYEL!(ch, C_CMP));
        let buf = replace_string(
            DAM_WEAPONS[msgnum].to_char,
            ATTACK_HIT_TEXT[w_type].singular,
            ATTACK_HIT_TEXT[w_type].plural,
        );
        self.act(&buf, false, Some(ch), None, Some(victim), TO_CHAR);
        send_to_char(ch, CCNRM!(ch, C_CMP));

        /* damage message to damagee */
        send_to_char(victim, CCRED!(victim, C_CMP));
        let buf = replace_string(
            DAM_WEAPONS[msgnum].to_victim,
            ATTACK_HIT_TEXT[w_type].singular,
            ATTACK_HIT_TEXT[w_type].plural,
        );
        self.act(
            &buf,
            false,
            Some(ch),
            None,
            Some(victim),
            TO_VICT | TO_SLEEP,
        );
        send_to_char(victim, CCNRM!(victim, C_CMP));
    }

    /*
     *  message for doing damage with a spell or skill
     *  C3.0: Also used for weapon damage on miss and death blows
     */
    pub fn skill_message(
        &self,
        dam: i32,
        ch: &CharData,
        vict: &CharData,
        attacktype: i32,
    ) -> i32 {
        let weap_b = ch.get_eq(WEAR_WIELD as i8).clone();
        let weap = weap_b.as_ref();
        let weapref = if weap.is_none() { None } else { Some(weap.unwrap().as_ref())};

        for i in 0..self.fight_messages.len() {
            if self.fight_messages[i].a_type == attacktype {
                let nr = dice(1, self.fight_messages[i].messages.len() as i32) as usize;
                let msg = &self.fight_messages[i].messages[nr];

                if !vict.is_npc() && vict.get_level() >= LVL_IMMORT as u8 {
                    self.act(
                        &msg.god_msg.attacker_msg,
                        false,
                        Some(ch),
                        weapref,
                        Some(vict),
                        TO_CHAR,
                    );
                    self.act(
                        &msg.god_msg.victim_msg,
                        false,
                        Some(ch),
                        weapref,
                        Some(vict),
                        TO_VICT,
                    );
                    self.act(
                        &msg.god_msg.room_msg,
                        false,
                        Some(ch),
                        weapref,
                        Some(vict),
                        TO_NOTVICT,
                    );
                } else if dam != 0 {
                    /*
                     * Don't send redundant color codes for TYPE_SUFFERING & other types
                     * of damage without attacker_msg.
                     */
                    if vict.get_pos() == POS_DEAD {
                        if !msg.die_msg.attacker_msg.is_empty() {
                            send_to_char(ch, CCYEL!(ch, C_CMP));
                            self.act(
                                &msg.die_msg.attacker_msg,
                                false,
                                Some(ch),
                                weapref,
                                Some(vict),
                                TO_CHAR,
                            );
                            send_to_char(ch, CCNRM!(ch, C_CMP));
                        }

                        send_to_char(vict, CCRED!(vict, C_CMP));
                        self.act(
                            &msg.die_msg.victim_msg,
                            false,
                            Some(ch),
                            weapref,
                            Some(vict),
                            TO_VICT | TO_SLEEP,
                        );
                        send_to_char(vict, CCNRM!(vict, C_CMP));

                        self.act(
                            &msg.die_msg.room_msg,
                            false,
                            Some(ch),
                            weapref,
                            Some(vict),
                            TO_NOTVICT,
                        );
                    } else {
                        if !msg.hit_msg.attacker_msg.is_empty() {
                            send_to_char(ch, CCYEL!(ch, C_CMP));
                            self.act(
                                &msg.hit_msg.attacker_msg,
                                false,
                                Some(ch),
                                weapref,
                                Some(vict),
                                TO_CHAR,
                            );
                            send_to_char(ch, CCNRM!(ch, C_CMP));
                        }

                        send_to_char(vict, CCRED!(vict, C_CMP));
                        self.act(
                            &msg.hit_msg.victim_msg,
                            false,
                            Some(ch),
                            weapref,
                            Some(vict),
                            TO_VICT | TO_SLEEP,
                        );
                        send_to_char(vict, CCNRM!(vict, C_CMP));

                        self.act(
                            &msg.hit_msg.room_msg,
                            false,
                            Some(ch),
                            weapref,
                            Some(vict),
                            TO_NOTVICT,
                        );
                    }
                } else if !std::ptr::eq(ch, vict) {
                    /* Dam == 0 */
                    if !msg.miss_msg.attacker_msg.is_empty() {
                        send_to_char(ch, CCYEL!(ch, C_CMP));
                        self.act(
                            &msg.miss_msg.attacker_msg,
                            false,
                            Some(ch),
                            weapref,
                            Some(vict),
                            TO_CHAR,
                        );
                        send_to_char(ch, CCNRM!(ch, C_CMP));
                    }

                    send_to_char(vict, CCRED!(vict, C_CMP));
                    self.act(
                        &msg.miss_msg.victim_msg,
                        false,
                        Some(ch),
                        weapref,
                        Some(vict),
                        TO_VICT | TO_SLEEP,
                    );
                    send_to_char(vict, CCNRM!(vict, C_CMP));

                    self.act(
                        &msg.miss_msg.room_msg,
                        false,
                        Some(ch),
                        weapref,
                        Some(vict),
                        TO_NOTVICT,
                    );
                }
                return 1;
            }
        }
        return 0;
    }
}
impl Game {
    /*
     * Alert: As of bpl14, this function returns the following codes:
     *	< 0	Victim died.
     *	= 0	No damage.
     *	> 0	How much damage done.
     */
    pub fn damage(
        &mut self,
        ch: &Rc<CharData>,
        victim: &Rc<CharData>,
        dam: i32,
        attacktype: i32,
    ) -> i32 {
        let mut dam = dam;

        if victim.get_pos() <= POS_DEAD {
            /* This is "normal"-ish now with delayed extraction. -gg 3/15/2001 */
            if victim.plr_flagged(PLR_NOTDEADYET) || victim.mob_flagged(MOB_NOTDEADYET) {
                return -1;
            }

            error!(
                "SYSERR: Attempt to damage corpse '{}' in room #{} by '{}'.",
                victim.get_name(),
                self.db.get_room_vnum(victim.in_room()),
                ch.get_name()
            );
            die(victim, self);
            return -1; /* -je, 7/7/92 */
        }

        /* peaceful rooms */
        if !Rc::ptr_eq(ch, victim) && self.db.room_flagged(ch.in_room(), ROOM_PEACEFUL) {
            send_to_char(
                ch,
                "This room just has such a peaceful, easy feeling...\r\n",
            );
            return 0;
        }

        /* shopkeeper protection */
        if !ok_damage_shopkeeper(self, ch, victim) {
            return 0;
        }

        /* You can't damage an immortal! */
        if !victim.is_npc() && victim.get_level() >= LVL_IMMORT as u8 {
            dam = 0;
        }

        if !Rc::ptr_eq(victim, ch) {
            /* Start the attacker fighting the victim */
            if ch.get_pos() > POS_STUNNED && ch.fighting().is_none() {
                self.db.set_fighting(ch, victim, self);
            }

            /* Start the victim fighting the attacker */
            if victim.get_pos() > POS_STUNNED && victim.fighting().is_none() {
                self.db.set_fighting(victim, ch, self);
                if victim.mob_flagged(MOB_MEMORY) && !ch.is_npc() {
                    remember(victim, ch);
                }
            }
        }

        /* If you attack a pet, it hates your guts */
        if victim.master.borrow().is_some()
            && Rc::ptr_eq(victim.master.borrow().as_ref().unwrap(), ch)
        {
            self.db.stop_follower(victim);
        }

        /* If the attacker is invisible, he becomes visible */
        if ch.aff_flagged(AFF_INVISIBLE | AFF_HIDE) {
            self.db.appear(ch);
        }

        /* Cut damage in half if victim has sanct, to a minimum 1 */
        if victim.aff_flagged(AFF_SANCTUARY) && dam >= 2 {
            dam /= 2;
        }

        /* Check for PK if this is not a PK MUD */
        if PK_ALLOWED {
            check_killer(ch, victim, self);
            if ch.plr_flagged(PLR_KILLER) && !Rc::ptr_eq(ch, victim) {
                dam = 0;
            }
        }

        /* Set the maximum damage per round and subtract the hit points */
        dam = max(min(dam, 100), 0);
        victim.decr_hit(dam as i16);

        /* Gain exp for the hit */
        if !Rc::ptr_eq(ch, victim) {
            gain_exp(ch, victim.get_level() as i32 * dam, self);
        }

        update_pos(victim);

        /*
         * skill_message sends a message from the messages file in lib/misc.
         * dam_message just sends a generic "You hit $n extremely hard.".
         * skill_message is preferable to dam_message because it is more
         * descriptive.
         *
         * If we are _not_ attacking with a weapon (i.e. a spell), always use
         * skill_message. If we are attacking with a weapon: If this is a miss or a
         * death blow, send a skill_message if one exists; if not, default to a
         * dam_message. Otherwise, always send a dam_message.
         */
        if !is_weapon!(attacktype) {
            self.db.skill_message(dam, ch, victim, attacktype);
        } else {
            if victim.get_pos() == POS_DEAD || dam == 0 {
                if self.db.skill_message(dam, ch, victim, attacktype) == 0 {
                    self.db.dam_message(dam, ch, victim, attacktype);
                }
            } else {
                self.db.dam_message(dam, ch, victim, attacktype);
            }
        }

        /* Use send_to_char -- act() doesn't send message if you are DEAD. */
        match victim.get_pos() {
            POS_MORTALLYW => {
                self.db.act(
                    "$n is mortally wounded, and will die soon, if not aided.",
                    true,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(
                    victim,
                    "You are mortally wounded, and will die soon, if not aided.\r\n",
                );
            }

            POS_INCAP => {
                self.db.act(
                    "$n is incapacitated and will slowly die, if not aided.",
                    true,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(
                    victim,
                    "You are incapacitated an will slowly die, if not aided.\r\n",
                );
            }
            POS_STUNNED => {
                self.db.act(
                    "$n is stunned, but will probably regain consciousness again.",
                    true,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(
                    victim,
                    "You're stunned, but will probably regain consciousness again.\r\n",
                );
            }
            POS_DEAD => {
                self.db.act(
                    "$n is dead!  R.I.P.",
                    false,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(victim, "You are dead!  Sorry...\r\n");
            }

            _ => {
                /* >= POSITION SLEEPING */
                if dam > (victim.get_max_hit() / 4) as i32 {
                    send_to_char(victim, "That really did HURT!\r\n");
                }

                if victim.get_hit() < victim.get_max_hit() / 4 {
                    send_to_char(
                        victim,
                        format!(
                            "{}You wish that your wounds would stop BLEEDING so much!{}\r\n",
                            CCRED!(victim, C_SPR),
                            CCNRM!(victim, C_SPR)
                        )
                        .as_str(),
                    );
                    if !Rc::ptr_eq(ch, victim) && victim.mob_flagged(MOB_WIMPY) {
                        do_flee(self, victim, "", 0, 0);
                    }
                }
                if !victim.is_npc()
                    && victim.get_wimp_lev() != 0
                    && !Rc::ptr_eq(ch, victim)
                    && victim.get_hit() < victim.get_wimp_lev() as i16
                    && victim.get_hit() > 0
                {
                    send_to_char(victim, "You wimp out, and attempt to flee!\r\n");
                    do_flee(self, victim, "", 0, 0);
                }
            }
        }

        /* Help out poor linkless people who are attacked */
        if !victim.is_npc() && victim.desc.borrow().is_none() && victim.get_pos() > POS_STUNNED {
            do_flee(self, victim, "", 0, 0);
            if victim.fighting().is_none() {
                self.db.act(
                    "$n is rescued by divine forces.",
                    false,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                victim.set_was_in(victim.in_room());
                self.db.char_from_room(victim);
                self.db.char_to_room(victim, 0);
            }
        }

        /* stop someone from fighting if they're stunned or worse */
        if victim.get_pos() <= POS_STUNNED && victim.fighting().is_some() {
            self.db.stop_fighting(victim);
        }

        /* Uh oh.  Victim died. */
        if victim.get_pos() == POS_DEAD {
            if !Rc::ptr_eq(ch, victim) && (victim.is_npc() || victim.desc.borrow().is_some()) {
                if ch.aff_flagged(AFF_GROUP) {
                    group_gain(ch, victim, self);
                } else {
                    solo_gain(ch, victim, self);
                }

                if !victim.is_npc() {
                    self.mudlog(
                        BRF,
                        LVL_IMMORT as i32,
                        true,
                        format!(
                            "{} killed by {} at {}",
                            victim.get_name(),
                            ch.get_name(),
                            self.db.world[victim.in_room() as usize].name
                        )
                        .as_str(),
                    );
                    if ch.mob_flagged(MOB_MEMORY) {
                        forget(ch, victim);
                    }
                }
                die(victim, self);
                return -1;
            }
        }
        return dam;
    }
}
/*
 * Calculate the THAC0 of the attacker.
 *
 * 'victim' currently isn't used but you could use it for special cases like
 * weapons that hit evil creatures easier or a weapon that always misses
 * attacking an animal.
 */
pub fn compute_thaco(ch: &CharData, _victim: &CharData) -> i32 {
    let mut calc_thaco;

    if !ch.is_npc() {
        calc_thaco = thaco(ch.get_class(), ch.get_level());
    } else {
        /* THAC0 for monsters is set in the HitRoll */
        calc_thaco = 20;
    }
    calc_thaco -= STR_APP[ch.strength_apply_index()].tohit as i32;
    calc_thaco -= ch.get_hitroll() as i32;
    calc_thaco -= ((ch.get_int() as f32 - 13f32) / 1.5) as i32; /* Intelligence helps! */
    calc_thaco -= ((ch.get_wis() as f32 - 13f32) / 1.5) as i32; /* So does wisdom */

    return calc_thaco;
}

impl Game {
    pub fn hit(&mut self, ch: &Rc<CharData>, victim: &Rc<CharData>, _type: i32) {
        let wielded = ch.get_eq(WEAR_WIELD as i8);

        /* Do some sanity checking, in case someone flees, etc. */
        if ch.in_room() != victim.in_room() {
            if ch.fighting().is_some() && Rc::ptr_eq(ch.fighting().as_ref().unwrap(), victim) {
                self.db.stop_fighting(ch);
                return;
            }
        }

        let w_type;
        /* Find the weapon type (for display purposes only) */
        if wielded.is_some() && wielded.as_ref().unwrap().get_obj_type() == ITEM_WEAPON {
            w_type = wielded.as_ref().unwrap().get_obj_val(3) + TYPE_HIT;
        } else {
            if ch.is_npc() && ch.mob_specials.attack_type != 0 {
                w_type = ch.mob_specials.attack_type as i32 + TYPE_HIT;
            } else {
                w_type = TYPE_HIT;
            }
        }

        /* Calculate chance of hit. Lower THAC0 is better for attacker. */
        let calc_thaco = compute_thaco(ch, victim);

        /* Calculate the raw armor including magic armor.  Lower AC is better for defender. */
        let victim_ac = compute_armor_class(victim) / 10;

        /* roll the die and take your chances... */
        let diceroll = rand_number(1, 20);

        /*
         * Decide whether this is a hit or a miss.
         *
         *  Victim asleep = hit, otherwise:
         *     1   = Automatic miss.
         *   2..19 = Checked vs. AC.
         *    20   = Automatic hit.
         */
        let mut dam: i32;
        if diceroll == 20 || !victim.awake() {
            dam = 1;
        } else if diceroll == 1 {
            dam = 0;
        } else {
            dam = if calc_thaco - diceroll as i32 <= victim_ac as i32 {
                1
            } else {
                0
            };
        }

        if dam == 0 {
            /* the attacker missed the victim */
            self.damage(
                ch,
                victim,
                0,
                if _type == SKILL_BACKSTAB {
                    SKILL_BACKSTAB
                } else {
                    w_type
                },
            );
        } else {
            /* okay, we know the guy has been hit.  now calculate damage. */

            /* Start with the damage bonuses: the damroll and strength apply */
            dam = STR_APP[ch.strength_apply_index()].todam as i32;
            dam += ch.get_damroll() as i32;

            /* Maybe holding arrow? */
            if wielded.is_some() && wielded.as_ref().unwrap().get_obj_type() == ITEM_WEAPON {
                /* Add weapon-based damage if a weapon is being wielded */
                dam += dice(
                    wielded.as_ref().unwrap().get_obj_val(1),
                    wielded.as_ref().unwrap().get_obj_val(2),
                );
            } else {
                /* If no weapon, add bare hand damage instead */
                if ch.is_npc() {
                    dam += dice(
                        ch.mob_specials.damnodice as i32,
                        ch.mob_specials.damsizedice as i32,
                    );
                } else {
                    dam += rand_number(0, 2) as i32; /* Max 2 bare hand damage for players */
                }
            }

            /*
             * Include a damage multiplier if victim isn't ready to fight:
             *
             * Position sitting  1.33 x normal
             * Position resting  1.66 x normal
             * Position sleeping 2.00 x normal
             * Position stunned  2.33 x normal
             * Position incap    2.66 x normal
             * Position mortally 3.00 x normal
             *
             * Note, this is a hack because it depends on the particular
             * values of the POSITION_XXX constants.
             */
            if victim.get_pos() < POS_FIGHTING {
                dam *= 1 + (POS_FIGHTING as i32 - victim.get_pos() as i32) / 3;
            }

            /* at least 1 hp damage min per hit */
            dam = max(1, dam);

            if _type == SKILL_BACKSTAB {
                self.damage(
                    ch,
                    victim,
                    dam * backstab_mult(ch.get_level()),
                    SKILL_BACKSTAB,
                );
            } else {
                self.damage(ch, victim, dam, w_type);
            }
        }
    }

    /* control the fights going on.  Called every 2 seconds from comm.c. */
    pub fn perform_violence(&mut self) {
        let mut old_combat_list = vec![];
        for c in self.db.combat_list.borrow().iter() {
            old_combat_list.push(c.clone());
        }

        for ch in old_combat_list.iter() {
            //next_combat_list = ch->next_fighting;

            if ch.fighting().is_none() || ch.in_room() != ch.fighting().as_ref().unwrap().in_room()
            {
                self.db.stop_fighting(ch);
                continue;
            }

            if ch.is_npc() {
                if ch.get_wait_state() > 0 {
                    ch.decr_wait_state(PULSE_VIOLENCE as i32);
                    continue;
                }
                ch.set_wait_state(0);

                if ch.get_pos() < POS_FIGHTING {
                    ch.set_pos(POS_FIGHTING);
                    self.db.act(
                        "$n scrambles to $s feet!",
                        true,
                        Some(ch),
                        None,
                        None,
                        TO_ROOM,
                    );
                }
            }

            if ch.get_pos() < POS_FIGHTING {
                send_to_char(ch, "You can't fight while sitting!!\r\n");
                continue;
            }

            self.hit(ch, ch.fighting().as_ref().unwrap(), TYPE_UNDEFINED);
            if ch.mob_flagged(MOB_SPEC)
                && self.db.get_mob_spec(ch).is_some()
                && !ch.mob_flagged(MOB_NOTDEADYET)
            {
                let actbuf = String::new();
                self.db.get_mob_spec(ch).as_ref().unwrap()(self, ch, ch, 0, actbuf.as_str());
            }
        }
    }
}
