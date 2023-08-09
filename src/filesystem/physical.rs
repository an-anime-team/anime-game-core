use std::path::PathBuf;
use std::ffi::OsStr;
use std::io::Result;

use super::DriverExt;

use super::get_uuid;

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
        loop {
            let path = self.parent_path
                .join(".transitions")
                .join(get_uuid(name));

            if !path.exists() {
                std::fs::create_dir_all(&path)?;
                std::fs::write(path.join(".transition"), name)?;

                return Ok(path);
            }
        }
    }

    fn get_transition(&self, name: &str) -> Option<PathBuf> {
        let path = self.parent_path
            .join(".transitions")
            .join(get_uuid(name));

        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    fn list_transitions(&self) -> Vec<(String, PathBuf)> {
        self.parent_path.join(".transitions").read_dir()
            .map(|files| {
                files.flatten()
                    .filter(|file| file.path().is_dir() && file.path().join(".transition").exists())
                    .flat_map(|file| {
                        if let Ok(name) = std::fs::read_to_string(file.path().join(".transition")) {
                            Some((name, file.path()))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or(vec![])
    }

    fn finish_transition(&self, name: &str) -> Result<()> {
        fn move_files(from: PathBuf, to: PathBuf) -> Result<()> {
            if !to.exists() {
                std::fs::create_dir_all(&to);
            }

            for file in from.read_dir()?.flatten() {
                if let Some(file_name) = from.file_name() {
                    if file.path().is_dir() {
                        move_files(file.path(), to.join(file_name))?;

                        std::fs::remove_dir_all(file.path());
                    }

                    else {
                        std::fs::copy(file.path(), to.join(file_name))?;
                        std::fs::remove_file(file.path())?;
                    }
                }
            }

            std::fs::remove_dir_all(from);

            Ok(())
        }

        let path = self.parent_path
            .join(".transitions")
            .join(get_uuid(name));

        if !path.exists() {
            return Ok(());
        }

        std::fs::remove_file(path.join(".transition"))?;

        move_files(path, self.parent_path)
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
