/* ************************************************************************
*   File: weather.rs                                    Part of CircleMUD *
*  Usage: functions handling time and the weather                         *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
*  Rust port Copyright (C) 2023, 2024 Laurent Pautet                      *
************************************************************************ */

use crate::depot::Depot;
use crate::structs::{SkyCondition, SunState};
use crate::util::dice;
use crate::{CharData, Game, DB};
use std::cmp::{max, min};

impl Game {
    pub(crate) fn weather_and_time(&mut self, chars: &Depot<CharData>, db: &mut DB, mode: i32) {
        self.another_hour(chars, db, mode);
        if mode != 0 {
            self.weather_change(chars, db);
        }
    }

    fn another_hour(&mut self, chars: &Depot<CharData>, db: &mut DB, mode: i32) {
        db.time_info.hours += 1;

        if mode != 0 {
            match db.time_info.hours {
                5 => {
                    db.weather_info.sunlight = SunState::Rise;
                    self.send_to_outdoor(chars, db, "The sun rises in the east.\r\n");
                }
                6 => {
                    db.weather_info.sunlight = SunState::Light;
                    self.send_to_outdoor(chars, db, "The day has begun.\r\n");
                }
                21 => {
                    db.weather_info.sunlight = SunState::Set;
                    self.send_to_outdoor(chars, db, "The sun slowly disappears in the west.\r\n");
                }
                22 => {
                    db.weather_info.sunlight = SunState::Dark;
                    self.send_to_outdoor(chars, db, "The night has begun.\r\n");
                }
                _ => {}
            }
        }

        if db.time_info.hours > 23 {
            /* Changed by HHS due to bug ??? */
            db.time_info.hours -= 24;
            db.time_info.day += 1;

            if db.time_info.day > 34 {
                db.time_info.day = 0;
                db.time_info.month += 1;

                if db.time_info.month > 16 {
                    db.time_info.month = 0;
                    db.time_info.year += 1;
                }
            }
        }
    }

    fn weather_change(&mut self, chars: &Depot<CharData>, db: &mut DB) {
        let diff = if (db.time_info.month >= 9) && (db.time_info.month <= 16) {
            if db.weather_info.pressure > 985 {
                -2
            } else {
                2
            }
        } else if db.weather_info.pressure > 1015 {
            -2
        } else {
            2
        };

        db.weather_info.change += dice(1, 4) * diff + dice(2, 6) - dice(2, 6);

        db.weather_info.change = min(db.weather_info.change, 12);
        db.weather_info.change = max(db.weather_info.change, -12);

        db.weather_info.pressure += db.weather_info.change;

        db.weather_info.pressure = min(db.weather_info.pressure, 1040);
        db.weather_info.pressure = max(db.weather_info.pressure, 960);

        let mut change = 0;

        #[allow(clippy::if_same_then_else)]
        match db.weather_info.sky {
            SkyCondition::Cloudless => {
                if db.weather_info.pressure < 990 {
                    change = 1;
                } else if db.weather_info.pressure < 1010
                    && dice(1, 4) == 1 {
                        change = 1;
                    }
            }
            SkyCondition::Cloudy => {
                if db.weather_info.pressure < 970 {
                    change = 2;
                } else if db.weather_info.pressure < 990 {
                    if dice(1, 4) == 1 {
                        change = 2;
                    } else {
                        change = 0;
                    }
                } else if db.weather_info.pressure > 1030
                    && dice(1, 4) == 1 {
                        change = 3;
                    }
            }
            SkyCondition::Raining => {
                if db.weather_info.pressure < 970 {
                    if dice(1, 4) == 1 {
                        change = 4;
                    } else {
                        change = 0;
                    }
                } else if db.weather_info.pressure > 1030 {
                    change = 5;
                } else if db.weather_info.pressure > 1010
                    && dice(1, 4) == 1 {
                        change = 5;
                    }
            }
            SkyCondition::Lightning => {
                if db.weather_info.pressure > 1010 {
                    change = 6;
                } else if db.weather_info.pressure > 990
                    && dice(1, 4) == 1 {
                        change = 6;
                    }
            }
        }

        match change {
            0 => {}
            1 => {
                self.send_to_outdoor(chars, db, "The sky starts to get cloudy.\r\n");
                db.weather_info.sky = SkyCondition::Cloudy;
            }

            2 => {
                self.send_to_outdoor(chars, db, "It starts to rain.\r\n");
                db.weather_info.sky = SkyCondition::Raining;
            }

            3 => {
                self.send_to_outdoor(chars, db, "The clouds disappear.\r\n");
                db.weather_info.sky = SkyCondition::Cloudless;
            }

            4 => {
                self.send_to_outdoor(chars, db, "Lightning starts to show in the sky.\r\n");
                db.weather_info.sky = SkyCondition::Lightning;
            }

            5 => {
                self.send_to_outdoor(chars, db, "The rain stops.\r\n");
                db.weather_info.sky = SkyCondition::Cloudy;
            }

            6 => {
                self.send_to_outdoor(chars, db, "The lightning stops.\r\n");
                db.weather_info.sky = SkyCondition::Raining;
            }
            _ => {}
        }
    }
}
