use std::path::Path;

use crate::version::Version;

pub trait GameBasics {
    fn new<T: ToString>(path: T) -> Self;
    fn path(&self) -> &str;

    /// Checks if the game is installed
    fn is_installed(&self) -> bool {
        Path::new(self.path()).exists()
    }

    fn try_get_latest_version() -> anyhow::Result<Version>;
    fn try_get_version(&self) -> anyhow::Result<Version>;
}
