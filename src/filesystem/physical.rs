use std::path::PathBuf;
use std::ffi::OsStr;
use std::io::Result;

use super::DriverExt;

pub struct Driver {
    parent_path: PathBuf
}

impl Driver {
    #[inline]
    pub fn new(folder: impl Into<PathBuf>) -> Self {
        Self {
            parent_path: folder.into()
        }
    }
}

impl DriverExt for Driver {
    #[inline]
    fn exists(&self, name: &OsStr) -> bool {
        self.parent_path.join(name).exists()
    }

    #[inline]
    fn metadata(&self, name: &OsStr) -> Result<std::fs::Metadata> {
        self.parent_path.join(name).metadata()
    }

    #[inline]
    fn read(&self, name: &OsStr) -> Result<Vec<u8>> {
        std::fs::read(self.parent_path.join(name))
    }

    #[inline]
    fn read_dir(&self, name: &OsStr) -> Result<std::fs::ReadDir> {
        std::fs::read_dir(self.parent_path.join(name))
    }
}

impl<T> From<T> for Driver where T: Into<PathBuf> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
