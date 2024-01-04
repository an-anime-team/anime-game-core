use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{
    ArchiveExt,
    BasicUpdater,
    BasicEntry
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    path: PathBuf
}

impl ArchiveExt for Archive {
    type Error = std::io::Error;
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
        let output = Command::new("unzip")
            .arg("-l")
            .arg(&self.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()?;

        let output = String::from_utf8_lossy(&output.stdout);

        Ok(output.trim()
            .split('\n')
            .skip(3)
            .take_while(|line| !line.starts_with("---------"))
            .map(|line| line.split("  ").filter_map(|word| {
                let word = word.trim();

                if word.is_empty() {
                    None
                } else {
                    Some(word)
                }
            }))
            .flat_map(|mut words| {
                let size = words.next().map(|size| size.parse());
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
            .collect::<Vec<_>>())
    }

    fn extract(&self, folder: impl AsRef<Path>) -> Result<Self::Updater, Self::Error> {
        let files = HashMap::<String, u64>::from_iter(self.entries()?
            .into_iter()
            .map(|entry| (
                entry.path.to_string_lossy().to_string(),
                entry.size
            )));

        let total_size = files.values().sum::<u64>();

        let child = Command::new("unzip")
            .stdout(Stdio::piped())
            .arg("-o")
            .arg(&self.path)
            .arg("-d")
            .arg(folder.as_ref())
            .spawn()?;

        let prefix = format!("{}/", folder.as_ref().to_string_lossy());

        Ok(BasicUpdater::new(child, total_size, move |line| {
            // Strip 'Archive: ...' and other top-level info messages
            if let Some(line) = line.strip_prefix("  ") {
                // inflating: sus/3x.webp
                // linking: sus/3x.symlink          -> 3x.webp
                if let Some((_, file)) = line.split_once(": ") {
                    // Remove output directory prefix
                    let file = file.strip_prefix(&prefix)
                        .unwrap_or(file);

                    return files.get(file.trim_end()).copied();
                }
            }

            None
        }))
    }
}
