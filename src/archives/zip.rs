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

impl ArchiveExt<flume::SendError<usize>> for Archive {
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
        let files = self.entries()?
            .into_iter()
            .map(|entry| folder.as_ref().join(entry.path))
            .collect();

        let child = Command::new("unzip")
            .arg("-q")
            .arg("-o")
            .arg(&self.path)
            .arg("-d")
            .arg(folder.as_ref())
            .spawn()?;

        Ok(BasicUpdater::new(child, files))
    }
}