use std::path::{Path, PathBuf};

use crate::version::Version;

pub trait GameExt {
    fn new<T: Into<PathBuf>>(path: T) -> Self;
    fn path(&self) -> &Path;

    /// Checks if the game is installed
    fn is_installed(&self) -> bool {
        self.path().exists()
    }

    fn get_latest_version() -> anyhow::Result<Version>;
    fn get_version(&self) -> anyhow::Result<Version>;
}
