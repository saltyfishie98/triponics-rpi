#![allow(dead_code)]

pub mod project {
    pub const NAME: &str = "triponics";
    pub const DEVICE: &str = "0";
}

pub mod gpio {
    pub const SWITCH_1: u8 = 22;
    pub const SWITCH_2: u8 = 23;
    pub const SWITCH_3: u8 = 24;

    pub const GROWLIGHT: u8 = 27;
    pub const MOTOR_PH_UP: u8 = 26;
    pub const MOTOR_PH_DOWN: u8 = 27;
}
