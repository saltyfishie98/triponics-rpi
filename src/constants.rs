#![allow(dead_code)]

pub mod project {
    pub const NAME: &str = "triponics";
    pub const DEVICE: &str = "0";
}

pub mod mqtt_prefix {
    pub const STATUS: &str = "status";
    pub const DATABASE: &str = "data";
    pub const REQUEST: &str = "request";
    pub const RESPONSE: &str = "response";
}
