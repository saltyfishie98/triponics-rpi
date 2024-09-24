use std::sync::LazyLock;

pub fn data_directory() -> &'static std::path::Path {
    static DATA_DIRECTORY: LazyLock<std::path::PathBuf> = LazyLock::new(|| {
        let mut cwd = std::env::current_dir().unwrap();
        cwd.push("data");
        cwd
    });
    DATA_DIRECTORY.as_path()
}

pub fn timezone_offset() -> &'static time::UtcOffset {
    static TIMEZONE_OFFSET: LazyLock<time::UtcOffset> = LazyLock::new(|| {
        time::UtcOffset::current_local_offset().unwrap_or(time::macros::offset!(+8))
    });
    &TIMEZONE_OFFSET
}
