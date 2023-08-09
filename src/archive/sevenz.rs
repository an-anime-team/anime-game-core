use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{
    ArchiveExt,
    BasicUpdater,
    BasicEntry
};

/// Get 7z binary if some is available
fn get_sevenz() -> Option<&'static str> {
    for binary in ["7z", "7za"] {
        let result = Command::new(binary)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();

        if result.is_ok() {
            return Some(binary);
        }
    }

    None
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("7z package is not installed")]
    SevenZNotAvailable,

    #[error("{0}")]
    Io(#[from] std::io::Error)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    path: PathBuf
}

impl ArchiveExt for Archive {
    type Error = Error;
    type Entry = BasicEntry;
    type Updater = BasicUpdater;

    #[inline]
    fn open(file: impl AsRef<Path>) -> Result<Self, Self::Error> where Self: Sized {
        Ok(Self {
            path: file.as_ref().to_path_buf()
        })
    }

    // TODO: cache

    fn entries(&self) -> Result<Vec<Self::Entry>, Self::Error> {
        let Some(sevenz) = get_sevenz() else {
            return Err(Error::SevenZNotAvailable);
        };

        let output = Command::new(sevenz)
            .arg("l")
            .arg(&self.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()?;

        let output = String::from_utf8_lossy(&output.stdout);

        let output = output.split("-------------------").skip(1).collect::<Vec<_>>();
        let output = output[..output.len() - 1].join("-------------------");

        Ok(output.split('\n')
            .filter(|line| &line[..1] != "-" && &line[..2] != " -")
            .map(|line| {
                line.split("  ").filter_map(|word| {
                    let word = word.trim();

                    if word.is_empty() {
                        None
                    } else {
                        Some(word)
                    }
                })
            })
            .flat_map(|mut words| {
                let size = words.nth(1).map(|size| size.parse());
                let path = words.last().map(PathBuf::from);

                if let (Some(path), Some(Ok(size))) = (path, size) {
                    Some(BasicEntry {
                        path,
                        size
                    })
                } else {
                    None
                }
            })
            .collect())
    }

    fn extract(&self, folder: impl AsRef<Path>) -> Result<Self::Updater, Self::Error> {
        let Some(sevenz) = get_sevenz() else {
            return Err(Error::SevenZNotAvailable);
        };

        let files = self.entries()?
            .into_iter()
            .map(|entry| folder.as_ref().join(entry.path))
            .collect();

        // Workaround to allow 7z to overwrite files
        // Somehow it manages to forbid itself to do this
        Command::new("chmod")
            .arg("-R")
            .arg("755")
            .arg(folder.as_ref())
            .output()?;

        let child = Command::new(sevenz)
            .arg("x")
            .arg(&self.path)
            .arg(format!("-o{}", folder.as_ref().to_string_lossy()))
            .arg("-aoa")
            .spawn()?;

        Ok(BasicUpdater::new(child, files))
    }
}
