use std::path::{Path, PathBuf};

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Transition {
    original_path: PathBuf,
    transition_path: PathBuf,
    name: String
}

impl Transition {
    #[inline]
    pub fn get(name: impl AsRef<str>, path: impl Into<PathBuf>) -> std::io::Result<Self> {
        Self::get_in(name, path, std::env::temp_dir().join(".transitions"))
    }

    pub fn get_in(name: impl AsRef<str>, path: impl Into<PathBuf>, transitions_storage: impl AsRef<Path>) -> std::io::Result<Self> {
        let transition_path = transitions_storage
            .as_ref()
            .join(get_uuid(name.as_ref()));

        if !transition_path.exists() {
            std::fs::create_dir_all(&transition_path)?;
        }

        Ok(Self {
            original_path: path.into(),
            transition_path,
            name: name.as_ref().to_string()
        })
    }

    #[inline]
    pub fn original_path(&self) -> &Path {
        &self.original_path
    }

    #[inline]
    pub fn transition_path(&self) -> &Path {
        &self.transition_path
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn finish(&self) -> std::io::Result<()> {
        move_files(&self.transition_path, &self.original_path)?;

        std::fs::remove_dir_all(&self.transition_path)
    }
}
