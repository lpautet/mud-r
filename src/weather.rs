/* ************************************************************************
*   File: weather.c                                     Part of CircleMUD *
*  Usage: functions handling time and the weather                         *
*                                                                         *
*  All rights reserved.  See license.doc for complete information.        *
*                                                                         *
*  Copyright (C) 1993, 94 by the Trustees of the Johns Hopkins University *
*  CircleMUD is based on DikuMUD, Copyright (C) 1990, 1991.               *
************************************************************************ */

use crate::structs::{
    SKY_CLOUDLESS, SKY_CLOUDY, SKY_LIGHTNING, SKY_RAINING, SUN_DARK, SUN_LIGHT, SUN_RISE, SUN_SET,
};
use crate::util::dice;
use crate::MainGlobals;
use std::cmp::{max, min};

impl MainGlobals {
    pub(crate) fn weather_and_time(&self, mode: i32) {
        self.another_hour(mode);
        if mode != 0 {
            self.weather_change();
        }
    }

    fn another_hour(&self, mode: i32) {
        let mut time_info = self.db.time_info.borrow_mut();
        let mut weather_info = self.db.weather_info.borrow_mut();
        time_info.hours += 1;

        if mode != 0 {
            match time_info.hours {
                5 => {
                    weather_info.sunlight = SUN_RISE;
                    self.send_to_outdoor("The sun rises in the east.\r\n");
                }
                6 => {
                    weather_info.sunlight = SUN_LIGHT;
                    self.send_to_outdoor("The day has begun.\r\n");
                }
                21 => {
                    weather_info.sunlight = SUN_SET;
                    self.send_to_outdoor("The sun slowly disappears in the west.\r\n");
                }
                22 => {
                    weather_info.sunlight = SUN_DARK;
                    self.send_to_outdoor("The night has begun.\r\n");
                }
                _ => {}
            }
        }
        if time_info.hours > 23 {
            /* Changed by HHS due to bug ??? */
            time_info.hours -= 24;
            time_info.day += 1;

            if time_info.day > 34 {
                time_info.day = 0;
                time_info.month += 1;

                if time_info.month > 16 {
                    time_info.month = 0;
                    time_info.year += 1;
                }
            }
        }
    }

    fn weather_change(&self) {
        let time_info = self.db.time_info.borrow_mut();
        let mut weather_info = self.db.weather_info.borrow_mut();

        let diff;
        if (time_info.month >= 9) && (time_info.month <= 16) {
            diff = if weather_info.pressure > 985 { -2 } else { 2 };
        } else {
            diff = if weather_info.pressure > 1015 { -2 } else { 2 };
        }

        weather_info.change += dice(1, 4) * diff + dice(2, 6) - dice(2, 6);

        weather_info.change = min(weather_info.change, 12);
        weather_info.change = max(weather_info.change, -12);

        weather_info.pressure += weather_info.change;

        weather_info.pressure = min(weather_info.pressure, 1040);
        weather_info.pressure = max(weather_info.pressure, 960);

        let mut change = 0;

        match weather_info.sky {
            SKY_CLOUDLESS => {
                if weather_info.pressure < 990 {
                    change = 1;
                } else if weather_info.pressure < 1010 {
                    if dice(1, 4) == 1 {
                        change = 1;
                    }
                }
            }
            SKY_CLOUDY => {
                if weather_info.pressure < 970 {
                    change = 2;
                } else if weather_info.pressure < 990 {
                    if dice(1, 4) == 1 {
                        change = 2;
                    } else {
                        change = 0;
                    }
                } else if weather_info.pressure > 1030 {
                    if dice(1, 4) == 1 {
                        change = 3;
                    }
                }
            }
            SKY_RAINING => {
                if weather_info.pressure < 970 {
                    if dice(1, 4) == 1 {
                        change = 4;
                    } else {
                        change = 0;
                    }
                } else if weather_info.pressure > 1030 {
                    change = 5;
                } else if weather_info.pressure > 1010 {
                    if dice(1, 4) == 1 {
                        change = 5;
                    }
                }
            }
            SKY_LIGHTNING => {
                if weather_info.pressure > 1010 {
                    change = 6;
                } else if weather_info.pressure > 990 {
                    if dice(1, 4) == 1 {
                        change = 6;
                    }
                }
            }

            _ => {
                change = 0;
                weather_info.sky = SKY_CLOUDLESS;
            }
        }

        match change {
            0 => {}
            1 => {
                self.send_to_outdoor("The sky starts to get cloudy.\r\n");
                weather_info.sky = SKY_CLOUDY;
            }

            2 => {
                self.send_to_outdoor("It starts to rain.\r\n");
                weather_info.sky = SKY_RAINING;
            }

            3 => {
                self.send_to_outdoor("The clouds disappear.\r\n");
                weather_info.sky = SKY_CLOUDLESS;
            }

            4 => {
                self.send_to_outdoor("Lightning starts to show in the sky.\r\n");
                weather_info.sky = SKY_LIGHTNING;
            }

            5 => {
                self.send_to_outdoor("The rain stops.\r\n");
                weather_info.sky = SKY_CLOUDY;
            }

            6 => {
                self.send_to_outdoor("The lightning stops.\r\n");
                weather_info.sky = SKY_RAINING;
            }
            _ => {}
        }
    }
}
