use std::path::{Path, PathBuf};

use crate::version::Version;

pub trait GameExt {
    /// Game edition
    type Edition;

    fn new(path: impl Into<PathBuf>, edition: Self::Edition) -> Self;

    fn path(&self) -> &Path;
    fn edition(&self) -> Self::Edition;

    /// Checks if the game is installed
    fn is_installed(&self) -> bool {
        self.path().exists()
    }

    fn get_latest_version(edition: Self::Edition) -> anyhow::Result<Version>;
    fn get_version(&self) -> anyhow::Result<Version>;
}
