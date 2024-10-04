use std::{
    fs::OpenOptions,
    io::{BufReader, Write},
};

pub trait ConfigFile {
    const FILENAME: &'static str;
    type Config: serde::Serialize + serde::de::DeserializeOwned + Default;

    fn load_config() -> serde_json::Result<Self::Config> {
        let filepath = local::filepath(Self::FILENAME);

        if !filepath.exists() {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .truncate(true)
                .open(&filepath)
                .unwrap();

            file.write_all(
                serde_json::to_string_pretty(
                    &serde_json::to_value(Self::Config::default()).unwrap(),
                )
                .unwrap()
                .as_bytes(),
            )
            .unwrap();

            Ok(Self::Config::default())
        } else {
            let file = OpenOptions::new().read(true).open(&filepath).unwrap();
            let reader = BufReader::new(file);

            serde_json::from_reader(reader)
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
