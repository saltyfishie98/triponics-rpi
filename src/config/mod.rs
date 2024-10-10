use crate::{log, AtomicFixedString};
use std::{
    fs::OpenOptions,
    io::{BufReader, Write},
    path::PathBuf,
};

pub trait ConfigFile {
    const FILENAME: &'static str;
    type Config: serde::Serialize + serde::de::DeserializeOwned + Default + std::fmt::Debug;

    fn config_filepath() -> PathBuf {
        local::filepath(Self::FILENAME)
    }

    fn save_config(config: Self::Config) -> std::io::Result<()> {
        let filepath = Self::config_filepath();

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

        log::debug!(
            "new config file: \"{}\"\n{:?}",
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

    fn load_config() -> Result<Self::Config, AtomicFixedString> {
        let filepath = Self::config_filepath();

        if !filepath.exists() {
            Self::save_config(Self::Config::default()).unwrap();
            Ok(Self::Config::default())
        } else {
            let file = OpenOptions::new().read(true).open(&filepath).unwrap();
            let reader = BufReader::new(file);

            let out = serde_json::from_reader(reader).map_err(|e| -> AtomicFixedString {
                format!(
                    "error in config file '{}', reason {e}",
                    filepath.to_str().unwrap()
                )
                .into()
            })?;

            log::debug!(
                "existing config file: \"{}\"\n{:?}",
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
