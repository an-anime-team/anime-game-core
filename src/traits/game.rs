use std::path::PathBuf;

use crate::version::Version;

pub trait GameBasics {
    fn new<T: Into<PathBuf>>(path: T) -> Self;
    fn path(&self) -> &PathBuf;

    /// Checks if the game is installed
    fn is_installed(&self) -> bool {
        self.path().exists()
    }

    fn try_get_latest_version() -> anyhow::Result<Version>;
    fn try_get_version(&self) -> anyhow::Result<Version>;
}
