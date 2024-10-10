use crate::log;
use std::{
    fs::OpenOptions,
    io::{BufReader, Write},
};

pub trait ConfigFile {
    const FILENAME: &'static str;
    type Config: serde::Serialize + serde::de::DeserializeOwned + Default + std::fmt::Debug;

    fn save_config(config: Self::Config) -> std::io::Result<()> {
        let filepath = local::filepath(Self::FILENAME);

        let mut file = {
            if !filepath.exists() {
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .truncate(true)
                    .open(&filepath)
                    .unwrap()
            } else {
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&filepath)
                    .unwrap()
            }
        };

        log::info!(
            "new config file: \"{}\"\n{:#?}",
            filepath.as_path().to_str().unwrap(),
            config
        );

        file.write_all(
            format!(
                "{}\n",
                serde_json::to_string_pretty(&serde_json::to_value(config).unwrap(),).unwrap()
            )
            .as_bytes(),
        )
    }

    fn load_config() -> serde_json::Result<Self::Config> {
        let filepath = local::filepath(Self::FILENAME);

        if !filepath.exists() {
            Self::save_config(Self::Config::default()).unwrap();
            Ok(Self::Config::default())
        } else {
            let file = OpenOptions::new().read(true).open(&filepath).unwrap();
            let reader = BufReader::new(file);

            let out = serde_json::from_reader(reader)?;

            log::info!(
                "existing config file: \"{}\"\n{:#?}",
                filepath.as_path().to_str().unwrap(),
                out
            );

            Ok(out)
        }
    }
}

mod local {
    use std::path::PathBuf;

    pub fn filepath(name: &str) -> PathBuf {
        let mut dir = crate::data_directory().to_path_buf();
        dir.push("config");
        std::fs::create_dir_all(&dir).unwrap();

        dir.push(format!("{}.json", name));
        dir
    }
}
