use std::path::PathBuf;

use crate::updater::UpdaterExt;

pub trait VerifyIntegrityExt {
    type Error;
    type Updater: UpdaterExt;

    /// Verify installed game files and return
    /// list of broken/outdated/absent files
    fn verify_files(&self) -> Result<Self::Updater, Self::Error>;
}

pub trait RepairFilesExt {
    type Error;
    type Updater: UpdaterExt;

    /// Repair game files
    fn repair_files(&self, files: impl AsRef<[PathBuf]>) -> Result<Self::Updater, Self::Error>;
}
