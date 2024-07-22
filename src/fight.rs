/* ************************************************************************
*   File: fight.rs                                      Part of CircleMUD *
*  Usage: Combat system                                                   *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use log::error;
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
use crate::depot::{Depot, DepotId, HasId};
use crate::handler::{affect_from_char, affect_remove, affected_by_spell, obj_to_obj, object_list_new_owner};
use crate::limits::gain_exp;
use crate::mobact::{forget, remember};
use crate::screen::{C_CMP, C_SPR, KNRM, KNUL, KRED, KYEL};
use crate::shops::ok_damage_shopkeeper;
use crate::spells::{
    AttackHitType, SKILL_BACKSTAB, SPELL_INVISIBLE, SPELL_SLEEP, TYPE_HIT, TYPE_SUFFERING,
    TYPE_UNDEFINED,
};
use crate::structs::{
    CharData, MeRef, MessageList, MessageType, MsgType, ObjData, AFF_GROUP, AFF_HIDE,
    AFF_INVISIBLE, AFF_SANCTUARY, AFF_SLEEP, ITEM_CONTAINER, ITEM_NODONATE, ITEM_WEAPON,
    ITEM_WEAR_TAKE, LVL_IMMORT, MOB_MEMORY, MOB_NOTDEADYET, MOB_SPEC, MOB_WIMPY, NOTHING, NOWHERE,
    NUM_OF_DIRS, NUM_WEARS, PLR_KILLER, PLR_NOTDEADYET, PLR_THIEF, POS_DEAD, POS_FIGHTING,
    POS_INCAP, POS_MORTALLYW, POS_STANDING, POS_STUNNED, PRF_COLOR_1, PRF_COLOR_2, PULSE_VIOLENCE,
    ROOM_PEACEFUL, WEAR_WIELD,
};
use crate::util::{dice, rand_number, stop_follower, BRF};
use crate::{act, send_to_char, send_to_room, DescriptorData, TextData, VictimRef};
use crate::{
    _clrlevel, clr, Game, CCNRM, CCRED, CCYEL, TO_CHAR, TO_NOTVICT, TO_ROOM, TO_SLEEP, TO_VICT,
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
    pub fn appear(descs: &mut Depot<DescriptorData>, chars: &mut Depot<CharData>, db: &DB,objs: &mut Depot<ObjData>,  chid: DepotId) {
        let ch = chars.get_mut(chid);
        if affected_by_spell(ch, SPELL_INVISIBLE as i16) {
            affect_from_char( objs,ch, SPELL_INVISIBLE as i16);
        }
        ch.remove_aff_flags(AFF_INVISIBLE | AFF_HIDE);
        let ch = chars.get(chid);
        if ch.get_level() < LVL_IMMORT as u8 {
            act(descs, chars, 
                db,
                "$n slowly fades into existence.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
        } else {
            act(descs, chars, 
                db,
                "You feel a strange presence as $n appears, seemingly from nowhere.",
                false,
                Some(ch),
                None,
                None,
                TO_ROOM,
            );
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
                format!("SYSERR: Error #4 reading combat message file{}", MESS_FILE).as_str(),
            );
            a_type = buf.trim().parse::<i32>().expect(
                format!("SYSERR: Error #5 reading combat message file{}", MESS_FILE).as_str(),
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
                format!("SYSERR: Error #6 reading combat message file{}", MESS_FILE).as_str(),
            );

            while r != 0 && (buf.starts_with('\n') || buf.starts_with('*')) {
                r = reader.read_line(&mut buf).expect(
                    format!("SYSERR: Error #7 reading combat message file{}", MESS_FILE).as_str(),
                );
            }
        }
    }
}

pub fn update_pos(victim: &mut CharData) {
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

pub fn check_killer(chid: DepotId, vict_id: DepotId, game: &mut Game, chars: &mut Depot<CharData>, db: &DB) {
    let ch = chars.get(chid);
    let vict = chars.get(vict_id);
    if vict.plr_flagged(PLR_KILLER) || vict.plr_flagged(PLR_THIEF) {
        return;
    }
    if ch.plr_flagged(PLR_KILLER) || ch.is_npc() || vict.is_npc() || chid == vict_id {
        return;
    }
    let ch = chars.get_mut(chid);
    ch.set_plr_flag_bit(PLR_KILLER);

    send_to_char(&mut game.descriptors, ch, "If you want to be a PLAYER KILLER, so be it...\r\n");
    let ch = chars.get(chid);
    let vict = chars.get(vict_id);
    game.mudlog(chars,
        BRF,
        LVL_IMMORT as i32,
        true,
        format!(
            "PC Killer bit set on {} for initiating attack on {} at {}.",
            ch.get_name(),
            vict.get_name(),
            db.world[vict.in_room() as usize].name
        )
        .as_str(),
    );
}

/* start one char fighting another (yes, it is horrible, I know... )  */
impl Game {
    pub(crate) fn set_fighting(&mut self, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId, victid: DepotId) {
        if chid == victid {
            return;
        }

        let ch = chars.get(chid);
        if ch.fighting_id().is_some() {
            error!("Unexpected error in set_fighting!");
            return;
        }

        db.combat_list.push(chid);
        let ch = chars.get_mut(chid);

        if ch.aff_flagged(AFF_SLEEP) {
            affect_from_char( objs,ch, SPELL_SLEEP as i16);
        }
        let ch= chars.get_mut(chid);

        ch.set_fighting(Some(victid));
        ch.set_pos(POS_FIGHTING);

        if !PK_ALLOWED {
            check_killer(chid, victid, self, chars, db);
        }
    }
}
impl DB {
    /* remove a char from the list of fighting chars */
    pub fn stop_fighting(&mut self, ch: &mut CharData) {
        self.combat_list.retain(|&c| c != ch.id());
        ch.set_fighting(None);
        ch.set_pos(POS_STANDING);

        update_pos(ch);
    }
}
    pub fn make_corpse( chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId) {
        let ch = chars.get(chid);
        let mut corpse = ObjData::default();

        corpse.item_number = NOTHING;
        corpse.set_in_room(NOWHERE);
        corpse.name = Rc::from("corpse");

        let buf2 = format!("The corpse of {} is lying here.", ch.get_name());
        corpse.description = Rc::from(buf2.as_str());

        let buf2 = format!("the corpse of {}", ch.get_name());
        corpse.short_description = Rc::from(buf2.as_str());

        corpse.set_obj_type(ITEM_CONTAINER);
        corpse.set_obj_wear(ITEM_WEAR_TAKE);
        corpse.set_obj_extra(ITEM_NODONATE);
        corpse.set_obj_val(0, 0); /* You can't store stuff in a corpse */
        corpse.set_obj_val(3, 1); /* corpse identifier */
        corpse.set_obj_weight(ch.get_weight() as i32 + ch.is_carrying_w());
        corpse.set_obj_rent(100000);
        if ch.is_npc() {
            corpse.set_obj_timer(MAX_NPC_CORPSE_TIME);
        } else {
            corpse.set_obj_timer(MAX_PC_CORPSE_TIME);
        }


        let corpse_id = objs.push(corpse);
        db.object_list.push(corpse_id);

        /* transfer character's inventory to the corpse */
        let ch = chars.get(chid);
        for o in ch.carrying.clone() {
            objs.get_mut(corpse_id).contains.push(o);
        }
        for oid in objs.get(corpse_id).contains.clone() {
            objs.get_mut(oid).in_obj = Some(corpse_id);
            object_list_new_owner(chars, objs,oid, None);
        }
        /* transfer character's equipment to the corpse */
        for i in 0..NUM_WEARS {
            let ch = chars.get(chid);
            if ch.get_eq(i).is_some() {
                let oid = db.unequip_char(chars, objs,chid, i).unwrap();
                obj_to_obj(chars, objs,oid, corpse_id);
            }
        }
        let ch = chars.get(chid);
        /* transfer gold */
        if ch.get_gold() > 0 {
            /*
             * following 'if' clause added to fix gold duplication loophole
             * The above line apparently refers to the old "partially log in,
             * kill the game character, then finish login sequence" duping
             * bug. The duplication has been fixed (knock on wood) but the
             * test below shall live on, for a while. -gg 3/3/2002
             */
            if ch.is_npc() || ch.desc.is_some() {
                let money_id = db.create_money(objs,ch.get_gold());
                obj_to_obj(chars, objs,money_id.unwrap(), corpse_id);
            }
            let ch = chars.get_mut(chid);
            ch.set_gold(0);
        }
        let ch = chars.get_mut(chid);
        ch.carrying.clear();
        ch.set_is_carrying_w(0);
        ch.set_is_carrying_n(0);
        let ch_in_room = ch.in_room();
        db.obj_to_room(objs.get_mut(corpse_id), ch_in_room);
    }


/* When ch kills victim */
pub fn change_alignment(chars: &mut Depot<CharData>, chid: DepotId, victim_id: DepotId) {
    /*
     * new alignment change algorithm: if you kill a monster with alignment A,
     * you move 1/16th of the way to having alignment -A.  Simple and fast.
     */
    let ch = chars.get(chid);
    let victim = chars.get(victim_id);
    let alignment = ch.get_alignment() + (-victim.get_alignment() - ch.get_alignment()) / 16;
    chars.get_mut(chid).set_alignment(alignment);
}

    pub fn death_cry(descs: &mut Depot<DescriptorData>, chars: &Depot<CharData>, db: &DB, ch: &CharData) {
        act(descs, chars, 
            db,
            "Your blood freezes as you hear $n's death cry.",
            false,
            Some(ch),
            None,
            None,
            TO_ROOM,
        );
        let ch_in_room = ch.in_room();
        for door in 0..NUM_OF_DIRS {
            if db.can_go(ch, door) {
                send_to_room(
                    descs,
                    chars, db,
                    db.world[ch_in_room as usize].dir_option[door]
                        .as_ref()
                        .unwrap()
                        .to_room,
                    "Your blood freezes as you hear someone's death cry.\r\n",
                );
            }
        }
    }

    pub fn raw_kill(descs: &mut Depot<DescriptorData>, chars: &mut Depot<CharData>, db: &mut DB,objs: &mut Depot<ObjData>,  chid: DepotId) {
        let ch = chars.get_mut(chid);
        if ch.fighting_id().is_some() {
            db.stop_fighting(ch);
        }
        let ch = chars.get_mut(chid);
        let mut list = ch.affected.clone();
        list.retain(|af| {
            affect_remove(objs,ch, *af);
            false
        });
        let ch = chars.get_mut(chid);
        ch.affected = list;
        let ch = chars.get(chid);
        death_cry(descs, chars, db, ch);
        make_corpse(chars, db, objs,chid);
        db.extract_char(chars, chid);
    }


pub fn die(chid: DepotId, game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, ) {
    let ch = chars.get(chid);
    gain_exp(chid, -(ch.get_exp() / 2), game, chars, db, texts,objs);
    let ch = chars.get_mut(chid);
    if !ch.is_npc() {
        ch.remove_plr_flag(PLR_KILLER | PLR_THIEF);
    }
    raw_kill(&mut game.descriptors, chars,db, objs,chid);
}

pub fn perform_group_gain(
    chid: DepotId,
    base: i32,
    victim_id: DepotId,
    game: &mut Game,
    chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
) {
    let ch = chars.get(chid);
    let share = min(MAX_EXP_GAIN, max(1, base));

    if share > 1 {
        send_to_char(&mut game.descriptors, 
            ch,
            format!(
                "You receive your share of experience -- {} points.\r\n",
                share
            )
            .as_str(),
        );
    } else {
        send_to_char(&mut game.descriptors, 
            ch,
            "You receive your share of experience -- one measly little point!\r\n",
        );
    }
    gain_exp(chid, share, game, chars, db, texts,objs);
    change_alignment(chars,  chid, victim_id);
}

pub fn group_gain(chid: DepotId, victim_id: DepotId, game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, ) {
    let ch = chars.get(chid);
    let victim = chars.get(victim_id);
    let k_id;
    if ch.master.is_none() {
        k_id = chid;
    } else {
        k_id = ch.master.unwrap();
    }
    let k = chars.get(k_id);
    let mut tot_members;
    if k.aff_flagged(AFF_GROUP) && k.in_room() == ch.in_room() {
        tot_members = 1;
    } else {
        tot_members = 0;
    }

    for f in k.followers.iter() {
        let follower = chars.get(f.follower);
        if follower.aff_flagged(AFF_GROUP) && follower.in_room() == ch.in_room() {
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
        perform_group_gain(k_id, base, victim_id, game, chars, db, texts,objs);
    }
    let k = chars.get(k_id);
    for f in k.followers.clone() {
        let follower = chars.get(f.follower);
        let ch = chars.get(chid);
        if follower.aff_flagged(AFF_GROUP) && follower.in_room() == ch.in_room() {
            perform_group_gain(f.follower, base, victim_id, game, chars, db, texts,objs);
        }
    }
}

pub fn solo_gain(chid: DepotId, victim_id: DepotId, game: &mut Game, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, ) {
    let ch = chars.get(chid);
    let victim = chars.get(victim_id);
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
        send_to_char(&mut game.descriptors, 
            ch,
            format!("You receive {} experience points.\r\n", exp).as_str(),
        );
    } else {
        send_to_char(&mut game.descriptors, ch, "You receive one lousy experience point.\r\n");
    }
    gain_exp(chid, exp, game, chars, db, texts,objs);
    change_alignment(chars,  chid, victim_id);
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
    buf
}

    /* message for doing damage with a weapon */
    pub fn dam_message(
        descs: &mut Depot<DescriptorData>, chars: &Depot<CharData>, 
        db: &DB,
        dam: i32,
        ch: &CharData,
        victim: &CharData,
        mut w_type: i32,
    ) {
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
        act(descs, chars, 
            db,
            &buf,
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(victim)),
            TO_NOTVICT,
        );

        /* damage message to damager */
        send_to_char(descs, ch, CCYEL!(ch, C_CMP));
        let buf = replace_string(
            DAM_WEAPONS[msgnum].to_char,
            ATTACK_HIT_TEXT[w_type].singular,
            ATTACK_HIT_TEXT[w_type].plural,
        );
        act(descs, chars, 
            db,
            &buf,
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(victim)),
            TO_CHAR,
        );
        send_to_char(descs, ch, CCNRM!(ch, C_CMP));

        /* damage message to damagee */
        send_to_char(descs, victim, CCRED!(victim, C_CMP));
        let buf = replace_string(
            DAM_WEAPONS[msgnum].to_victim,
            ATTACK_HIT_TEXT[w_type].singular,
            ATTACK_HIT_TEXT[w_type].plural,
        );
        act(descs, chars, 
            db,
            &buf,
            false,
            Some(ch),
            None,
            Some(VictimRef::Char(victim)),
            TO_VICT | TO_SLEEP,
        );
        send_to_char(descs, victim, CCNRM!(victim, C_CMP));
    }

    /*
     *  message for doing damage with a spell or skill
     *  C3.0: Also used for weapon damage on miss and death blows
     */
    pub fn skill_message(
        descs: &mut Depot<DescriptorData>, chars: &Depot<CharData>, 
        db: &DB,objs: & Depot<ObjData>, 
        dam: i32,
        ch: &CharData,
        vict: &CharData,
        attacktype: i32,
    ) -> i32 {
        let weap_id = ch.get_eq(WEAR_WIELD as i8);
        let weap = weap_id.map(|id| objs.get(id));

        for i in 0..db.fight_messages.len() {
            if db.fight_messages[i].a_type == attacktype {
                let nr = dice(1, db.fight_messages[i].messages.len() as i32) as usize;

                if !vict.is_npc() && vict.get_level() >= LVL_IMMORT as u8 {
                    let attacker_msg: &Rc<str> = &db.fight_messages[i].messages[nr]
                        .god_msg
                        .attacker_msg;
                    let victim_msg: &Rc<str> =
                        &db.fight_messages[i].messages[nr].god_msg.victim_msg;
                    let room_msg: &Rc<str> =
                        &db.fight_messages[i].messages[nr].god_msg.room_msg;
                    act(descs, chars, 
                        db,
                        attacker_msg,
                        false,
                        Some(ch),
                        weap,
                        Some(VictimRef::Char(vict)),
                        TO_CHAR,
                    );
                    act(descs, chars, 
                        db,
                        victim_msg,
                        false,
                        Some(ch),
                        weap,
                        Some(VictimRef::Char(vict)),
                        TO_VICT,
                    );
                    act(descs, chars, 
                        db,
                        room_msg,
                        false,
                        Some(ch),
                        weap,
                        Some(VictimRef::Char(vict)),
                        TO_NOTVICT,
                    );
                } else if dam != 0 {
                    /*
                     * Don't send redundant color codes for TYPE_SUFFERING & other types
                     * of damage without attacker_msg.
                     */
                    if vict.get_pos() == POS_DEAD {
                        let attacker_msg: &Rc<str> = &db.fight_messages[i].messages[nr]
                            .die_msg
                            .attacker_msg;
                        let victim_msg: &Rc<str> =
                            &db.fight_messages[i].messages[nr].die_msg.victim_msg;
                        let room_msg: &Rc<str> =
                            &db.fight_messages[i].messages[nr].die_msg.room_msg;
                        if !attacker_msg.is_empty() {
                            send_to_char(descs, ch, CCYEL!(ch, C_CMP));
                            act(descs, chars, 
                                db,
                                attacker_msg,
                                false,
                                Some(ch),
                                weap,
                                Some(VictimRef::Char(vict)),
                                TO_CHAR,
                            );
                            send_to_char(descs, ch, CCNRM!(ch, C_CMP));
                        }
                        send_to_char(descs, vict, CCRED!(vict, C_CMP));
                        act(descs, chars, 
                            db,
                            victim_msg,
                            false,
                            Some(ch),
                            weap,
                            Some(VictimRef::Char(vict)),
                            TO_VICT | TO_SLEEP,
                        );
                        send_to_char(descs, vict, CCNRM!(vict, C_CMP));

                        act(descs, chars, 
                            db,
                            room_msg,
                            false,
                            Some(ch),
                            weap,
                            Some(VictimRef::Char(vict)),
                            TO_NOTVICT,
                        );
                    } else {
                        let attacker_msg: &Rc<str> = &db.fight_messages[i].messages[nr]
                            .hit_msg
                            .attacker_msg;
                        let victim_msg: &Rc<str> =
                            &db.fight_messages[i].messages[nr].hit_msg.victim_msg;
                        let room_msg: &Rc<str> =
                            &db.fight_messages[i].messages[nr].hit_msg.room_msg;
                        if !attacker_msg.is_empty() {
                            send_to_char(descs, ch, CCYEL!(ch, C_CMP));
                            act(descs, chars, 
                                db,
                                attacker_msg,
                                false,
                                Some(ch),
                                weap,
                                Some(VictimRef::Char(vict)),
                                TO_CHAR,
                            );
                            send_to_char(descs, ch, CCNRM!(ch, C_CMP));
                        }
                        send_to_char(descs, vict, CCRED!(vict, C_CMP));
                        act(descs, chars, 
                            db,
                            victim_msg,
                            false,
                            Some(ch),
                            weap,
                            Some(VictimRef::Char(vict)),
                            TO_VICT | TO_SLEEP,
                        );
                        send_to_char(descs, vict, CCNRM!(vict, C_CMP));

                        act(descs, chars, 
                            db,
                            room_msg,
                            false,
                            Some(ch),
                            weap,
                            Some(VictimRef::Char(vict)),
                            TO_NOTVICT,
                        );
                    }
                } else if !std::ptr::eq(ch, vict) {
                    let attacker_msg: &Rc<str> = &db.fight_messages[i].messages[nr]
                        .miss_msg
                        .attacker_msg;
                    let victim_msg: &Rc<str> = &db.fight_messages[i].messages[nr]
                        .miss_msg
                        .victim_msg;
                    let room_msg: &Rc<str> =
                        &db.fight_messages[i].messages[nr].miss_msg.room_msg;
                    /* Dam == 0 */
                    if !attacker_msg.is_empty() {
                        send_to_char(descs, ch, CCYEL!(ch, C_CMP));
                        act(descs, chars, 
                            db,
                            attacker_msg,
                            false,
                            Some(ch),
                            weap,
                            Some(VictimRef::Char(vict)),
                            TO_CHAR,
                        );
                        send_to_char(descs, ch, CCNRM!(ch, C_CMP));
                    }
                    send_to_char(descs, vict, CCRED!(vict, C_CMP));
                    act(descs, chars, 
                        db,
                        victim_msg,
                        false,
                        Some(ch),
                        weap,
                        Some(VictimRef::Char(vict)),
                        TO_VICT | TO_SLEEP,
                    );
                    send_to_char(descs, vict, CCNRM!(vict, C_CMP));

                    act(descs, chars, 
                        db,
                        room_msg,
                        false,
                        Some(ch),
                        weap,
                        Some(VictimRef::Char(vict)),
                        TO_NOTVICT,
                    );
                }
                return 1;
            }
        }
        return 0;
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
        chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, 
        chid: DepotId,
        victim_id: DepotId,
        dam: i32,
        attacktype: i32,
    ) -> i32 {
        let ch = chars.get(chid);
        let victim = chars.get(victim_id);
        let mut dam = dam;

        if victim.get_pos() <= POS_DEAD {
            /* This is "normal"-ish now with delayed extraction. -gg 3/15/2001 */
            if victim.plr_flagged(PLR_NOTDEADYET) || victim.mob_flagged(MOB_NOTDEADYET) {
                return -1;
            }

            error!(
                "SYSERR: Attempt to damage corpse '{}' in room #{} by '{}'.",
                victim.get_name(),
                db.get_room_vnum(victim.in_room()),
                ch.get_name()
            );
            die(victim_id, self,chars, db, texts,objs);
            return -1; /* -je, 7/7/92 */
        }

        /* peaceful rooms */
        if chid != victim_id && db.room_flagged(ch.in_room(), ROOM_PEACEFUL) {
            send_to_char(&mut self.descriptors, 
                ch,
                "This room just has such a peaceful, easy feeling...\r\n",
            );
            return 0;
        }

        /* shopkeeper protection */
        if !ok_damage_shopkeeper(self, chars, db, texts,objs,chid, victim_id) {
            return 0;
        }

        /* You can't damage an immortal! */
        let victim = chars.get(victim_id);
        if !victim.is_npc() && victim.get_level() >= LVL_IMMORT as u8 {
            dam = 0;
        }

        if victim_id != chid {
            /* Start the attacker fighting the victim */
            let ch = chars.get(chid);
            if ch.get_pos() > POS_STUNNED && ch.fighting_id().is_none() {
                self.set_fighting(chars, db, objs,chid, victim_id);
            }

            /* Start the victim fighting the attacker */
            let victim = chars.get(victim_id);
            if victim.get_pos() > POS_STUNNED && victim.fighting_id().is_none() {
                self.set_fighting(chars, db, objs,victim_id, chid);
                let ch = chars.get(chid);
                let victim = chars.get(victim_id);
                if victim.mob_flagged(MOB_MEMORY) && !ch.is_npc() {
                    remember(chars,   victim_id, chid);
                }
            }
        }

        /* If you attack a pet, it hates your guts */
        let victim = chars.get(victim_id);
        if victim.master.is_some() && victim.master.unwrap() == chid {
            stop_follower(&mut self.descriptors, chars, db, objs,victim_id);
        }

        /* If the attacker is invisible, he becomes visible */
        let ch = chars.get(chid);
        if ch.aff_flagged(AFF_INVISIBLE | AFF_HIDE) {
            appear(&mut self.descriptors, chars, db, objs,chid);
        }

        /* Cut damage in half if victim has sanct, to a minimum 1 */
        let victim = chars.get(victim_id);
        if victim.aff_flagged(AFF_SANCTUARY) && dam >= 2 {
            dam /= 2;
        }

        /* Check for PK if this is not a PK MUD */
        if PK_ALLOWED {
            check_killer(chid, victim_id, self, chars, db);
            let ch = chars.get(chid);
            if ch.plr_flagged(PLR_KILLER) && chid != victim_id {
                dam = 0;
            }
        }

        /* Set the maximum damage per round and subtract the hit points */
        dam = max(min(dam, 100), 0);
        let victim = chars.get_mut(victim_id);
        victim.decr_hit(dam as i16);

        /* Gain exp for the hit */
        if chid != victim_id {
            gain_exp(chid, victim.get_level() as i32 * dam, self, chars, db, texts,objs);
        }
        let victim = chars.get_mut(victim_id);
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
        let ch = chars.get(chid);
        let victim = chars.get(victim_id);
        if !is_weapon!(attacktype) {
            skill_message(&mut self.descriptors, chars, db, objs,dam, ch, victim, attacktype);
        } else {
            if victim.get_pos() == POS_DEAD || dam == 0 {
                if skill_message(&mut self.descriptors, chars, db, objs,dam, ch, victim, attacktype) == 0 {
                    dam_message(&mut self.descriptors, chars, db, dam, ch, victim, attacktype);
                }
            } else {
                dam_message(&mut self.descriptors, chars, db, dam, ch, victim, attacktype);
            }
        }

        /* Use game.send_to_char -- act() doesn't send message if you are DEAD. */
        let victim = chars.get(victim_id);
        match victim.get_pos() {
            POS_MORTALLYW => {
                act(&mut self.descriptors, chars, 
                    db,
                    "$n is mortally wounded, and will die soon, if not aided.",
                    true,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(&mut self.descriptors, 
                    victim,
                    "You are mortally wounded, and will die soon, if not aided.\r\n",
                );
            }

            POS_INCAP => {
                act(&mut self.descriptors, chars, 
                    db,
                    "$n is incapacitated and will slowly die, if not aided.",
                    true,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(&mut self.descriptors, 
                    victim,
                    "You are incapacitated an will slowly die, if not aided.\r\n",
                );
            }
            POS_STUNNED => {
                act(&mut self.descriptors, chars, 
                    db,
                    "$n is stunned, but will probably regain consciousness again.",
                    true,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(&mut self.descriptors, 
                    victim,
                    "You're stunned, but will probably regain consciousness again.\r\n",
                );
            }
            POS_DEAD => {
                act(&mut self.descriptors, chars, 
                    db,
                    "$n is dead!  R.I.P.",
                    false,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                send_to_char(&mut self.descriptors, victim, "You are dead!  Sorry...\r\n");
            }

            _ => {
                /* >= POSITION SLEEPING */
                if dam > (victim.get_max_hit() / 4) as i32 {
                    send_to_char(&mut self.descriptors, victim, "That really did HURT!\r\n");
                }
                let victim = chars.get(victim_id);
                if victim.get_hit() < victim.get_max_hit() / 4 {
                    send_to_char(&mut self.descriptors, 
                        victim,
                        format!(
                            "{}You wish that your wounds would stop BLEEDING so much!{}\r\n",
                            CCRED!(victim, C_SPR),
                            CCNRM!(victim, C_SPR)
                        )
                        .as_str(),
                    );
                    let victim = chars.get(victim_id);
                    if chid != victim_id && victim.mob_flagged(MOB_WIMPY) {
                        do_flee(self, db, chars, texts, objs,victim_id, "", 0, 0);
                    }
                }
                let victim = chars.get(victim_id);
                if !victim.is_npc()
                    && victim.get_wimp_lev() != 0
                    && chid != victim_id
                    && victim.get_hit() < victim.get_wimp_lev() as i16
                    && victim.get_hit() > 0
                {
                    send_to_char(&mut self.descriptors, victim, "You wimp out, and attempt to flee!\r\n");
                    do_flee(self, db,chars,  texts, objs,victim_id, "", 0, 0);
                }
            }
        }

        /* Help out poor linkless people who are attacked */
        let victim = chars.get(victim_id);
        if !victim.is_npc() && victim.desc.is_none() && victim.get_pos() > POS_STUNNED {
            do_flee(self, db, chars, texts,objs, victim_id, "", 0, 0);
            let victim = chars.get(victim_id);
            if victim.fighting_id().is_none() {
                act(&mut self.descriptors, chars, 
                    db,
                    "$n is rescued by divine forces.",
                    false,
                    Some(victim),
                    None,
                    None,
                    TO_ROOM,
                );
                let victim = chars.get_mut(victim_id);
                victim.set_was_in(victim.in_room());
                db.char_from_room(objs, victim);
                db.char_to_room(chars, objs,victim_id, 0);
            }
        }

        /* stop someone from fighting if they're stunned or worse */
        let victim = chars.get_mut(victim_id);
        if victim.get_pos() <= POS_STUNNED && victim.fighting_id().is_some() {
            db.stop_fighting( victim);
        }

        /* Uh oh.  Victim died. */
        let victim = chars.get(victim_id);
        if victim.get_pos() == POS_DEAD {
            if chid != victim_id && (victim.is_npc() || victim.desc.is_some()) {
                let ch = chars.get(chid);
                if ch.aff_flagged(AFF_GROUP) {
                    group_gain(chid, victim_id, self, chars, db, texts,objs);
                } else {
                    solo_gain(chid, victim_id, self, chars, db, texts,objs);
                }
                let victim = chars.get(victim_id);
                if !victim.is_npc() {
                    let ch = chars.get(chid);
                    self.mudlog(chars,
                        BRF,
                        LVL_IMMORT as i32,
                        true,
                        format!(
                            "{} killed by {} at {}",
                            victim.get_name(),
                            ch.get_name(),
                            db.world[victim.in_room() as usize].name
                        )
                        .as_str(),
                    );
                    let ch = chars.get(chid);
                    if ch.mob_flagged(MOB_MEMORY) {
                        forget(chars,  chid, victim_id);
                    }
                }
                die(victim_id, self,chars, db, texts,objs);
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
    pub fn hit(&mut self, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>, objs: &mut Depot<ObjData>, chid: DepotId, victim_id: DepotId, _type: i32) {
        let ch = chars.get(chid);
        let victim = chars.get(victim_id);
        let wielded = ch.get_eq(WEAR_WIELD as i8);

        /* Do some sanity checking, in case someone flees, etc. */
        if ch.in_room() != victim.in_room() {
            if ch.fighting_id().is_some() && ch.fighting_id().unwrap() == victim_id {
                db.stop_fighting(chars.get_mut(chid));
                return;
            }
        }

        let w_type;
        /* Find the weapon type (for display purposes only) */
        if wielded.is_some() && objs.get(wielded.unwrap()).get_obj_type() == ITEM_WEAPON {
            w_type = objs.get(wielded.unwrap()).get_obj_val(3) + TYPE_HIT;
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
            self.damage(chars,
                db, texts,objs,
                chid,
                victim_id,
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
            if wielded.is_some() && objs.get(wielded.unwrap()).get_obj_type() == ITEM_WEAPON {
                /* Add weapon-based damage if a weapon is being wielded */
                dam += dice(
                    objs.get(wielded.unwrap()).get_obj_val(1),
                    objs.get(wielded.unwrap()).get_obj_val(2),
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
                self.damage(chars,
                    db,texts,objs,
                    chid,
                    victim_id,
                    dam * backstab_mult(ch.get_level()),
                    SKILL_BACKSTAB,
                );
            } else {
                self.damage(chars, db, texts,objs,chid, victim_id, dam, w_type);
            }
        }
    }

    /* control the fights going on.  Called every 2 seconds from comm.c. */
    pub fn perform_violence(&mut self, chars: &mut Depot<CharData>, db: &mut DB, texts: &mut Depot<TextData>,objs: &mut Depot<ObjData>, ) {
        let mut old_combat_list = vec![];
        for c in db.combat_list.iter() {
            old_combat_list.push(*c);
        }

        for chid in old_combat_list.into_iter() {
            //next_combat_list = ch->next_fighting;
            let ch = chars.get(chid);
            if ch.fighting_id().is_none()
                || ch.in_room() != chars.get(ch.fighting_id().unwrap()).in_room()
            {
                db.stop_fighting(chars.get_mut(chid));
                continue;
            }

            if ch.is_npc() {
                let ch = chars.get_mut(chid);
                if ch.get_wait_state() > 0 {
                    ch.decr_wait_state(PULSE_VIOLENCE as i32);
                    continue;
                }
                ch.set_wait_state(0);

                if ch.get_pos() < POS_FIGHTING {
                    ch.set_pos(POS_FIGHTING);
                    let ch = chars.get(chid);
                    act(&mut self.descriptors, chars, 
                        db,
                        "$n scrambles to $s feet!",
                        true,
                        Some(ch),
                        None,
                        None,
                        TO_ROOM,
                    );
                }
            }
            let ch = chars.get(chid);
            if ch.get_pos() < POS_FIGHTING {
                send_to_char(&mut self.descriptors, ch, "You can't fight while sitting!!\r\n");
                continue;
            }

            self.hit(chars, db, texts, objs,chid, ch.fighting_id().unwrap(), TYPE_UNDEFINED);
            let ch = chars.get(chid);
            if ch.mob_flagged(MOB_SPEC)
                && db.get_mob_spec(ch).is_some()
                && !ch.mob_flagged(MOB_NOTDEADYET)
            {
                let actbuf = String::new();
                db.get_mob_spec(ch).as_ref().unwrap()(
                    self,chars,
                    db,texts,objs,
                    chid,
                    MeRef::Char(chid),
                    0,
                    actbuf.as_str(),
                );
            }
        }
    }
}
