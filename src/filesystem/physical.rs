use std::path::PathBuf;
use std::ffi::OsStr;
use std::io::Result;

use super::DriverExt;

use super::{get_uuid, move_files};

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

    fn create_transition(&self, name: &str) -> Result<PathBuf> {
        let uuid = get_uuid(name);

        let path = self.parent_path
            .join(".transitions")
            .join(uuid);

        if !path.exists() {
            std::fs::create_dir_all(path.join("content"))?;
            std::fs::write(path.join("name"), name)?;
        }

        Ok(path.join("content"))
    }

    fn get_transition(&self, name: &str) -> Option<PathBuf> {
        let path = self.parent_path
            .join(".transitions")
            .join(get_uuid(name));

        if path.join("content").is_dir() && path.join("name").is_file() {
            Some(path.join("content"))
        } else {
            None
        }
    }

    fn list_transitions(&self) -> Vec<(String, PathBuf)> {
        self.parent_path.join(".transitions").read_dir()
            .map(|files| {
                files.flatten()
                    .filter(|file| {
                        file.path().join("content").is_dir() &&
                        file.path().join("name").is_file()
                    })
                    .flat_map(|file| {
                        if let Ok(name) = std::fs::read_to_string(file.path().join("name")) {
                            Some((name, file.path().join("content")))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or(vec![])
    }

    fn finish_transition(&self, name: &str) -> Result<()> {
        let path = self.parent_path
            .join(".transitions")
            .join(get_uuid(name));

        move_files(path.join("content"), &self.parent_path)?;

        std::fs::remove_dir_all(path)
    }

    fn remove_transition(&self, name: &str) -> Result<()> {
        let path = self.parent_path
            .join(".transitions")
            .join(get_uuid(name));

        std::fs::remove_dir_all(path)
    }
}

impl<T> From<T> for Driver where T: Into<PathBuf> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
