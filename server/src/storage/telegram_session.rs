use std::path::{Path, PathBuf};

/// Return the path to the Telegram session file.
pub fn session_path(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join("telegram.session")
}

/// Check whether a Telegram session file exists on disk.
pub fn session_exists(data_dir: &str) -> bool {
    session_path(data_dir).exists()
}
