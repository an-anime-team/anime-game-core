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
        let uuid = get_uuid(name);

        let path = self.parent_path
            .join(".transitions")
            .join(&uuid);

        if !path.exists() {
            std::fs::create_dir_all(&path)?;
            std::fs::write(path.join(format!(".{uuid}")), name)?;
        }

        Ok(path)
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
                    .filter(|file| {
                        let uuid = format!(".{}", file.file_name().to_string_lossy());

                        file.path().is_dir() &&
                        file.path().join(uuid).exists()
                    })
                    .flat_map(|file| {
                        let uuid = format!(".{}", file.file_name().to_string_lossy());

                        if let Ok(name) = std::fs::read_to_string(file.path().join(uuid)) {
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
                std::fs::create_dir_all(&to)?;
            }

            for file in from.read_dir()?.flatten() {
                let file = file.path();

                if let Some(file_name) = file.file_name() {
                    if file.is_dir() {
                        move_files(file.clone(), to.join(file_name))?;

                        std::fs::remove_dir_all(&file)?;
                    }

                    else {
                        std::fs::copy(&file, to.join(file_name))?;
                        std::fs::remove_file(&file)?;
                    }
                }
            }

            std::fs::remove_dir_all(from)?;

            Ok(())
        }

        let uuid = get_uuid(name);

        let path = self.parent_path
            .join(".transitions")
            .join(&uuid);

        if !path.exists() {
            return Ok(());
        }

        move_files(path, self.parent_path.clone())?;

        std::fs::remove_file(self.parent_path.join(uuid))
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
