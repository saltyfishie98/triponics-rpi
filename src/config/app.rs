use std::{fs::OpenOptions, io::Write};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AppConfig {
    pub mqtt: mqtt::Config,
}
impl AppConfig {
    pub fn load() -> Self {
        let mut json_path = std::env::current_dir().unwrap();
        json_path.push("data");
        std::fs::create_dir_all(json_path.clone()).unwrap();

        json_path.push("config.json");

        let file = if json_path.exists() {
            std::fs::File::open(json_path).unwrap()
        } else {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .truncate(true)
                .open(json_path.clone())
                .unwrap();

            file.write_all(include_str!("../../data/config.json").as_bytes())
                .unwrap();

            std::fs::File::open(json_path).unwrap()
        };

        serde_json::from_reader(file).unwrap()
    }
}

pub mod mqtt {
    use crate::{helper::AtomicFixedString, mqtt};

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    pub struct Config {
        pub topic_source: AtomicFixedString,
        pub create_options: mqtt::ClientCreateOptions,
        pub connect_options: mqtt::ClientConnectOptions,
    }
}
