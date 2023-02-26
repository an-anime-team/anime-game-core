use md5::{Md5, Digest};

use std::path::PathBuf;
use std::collections::HashSet;

use super::installer::downloader::{Downloader, DownloadingError};

// {"remoteName": "UnityPlayer.dll", "md5": "8c8c3d845b957e4cb84c662bed44d072", "fileSize": 33466104}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct IntegrityFile {
    pub path: PathBuf,
    pub md5: String,
    pub size: u64,
    pub base_url: String
}

impl IntegrityFile {
    /// Compare files' sizes and (if needed) hashes
    #[tracing::instrument(level = "trace", ret)]
    pub fn verify<T: Into<PathBuf> + std::fmt::Debug>(&self, game_path: T) -> bool {
        tracing::trace!("Verifying file");

        let file_path: PathBuf = game_path.into().join(&self.path);

        // Compare files' sizes. If they're different - they 100% different
        match file_path.metadata() {
            Ok(metadata) => {
                if metadata.len() != self.size {
                    false
                }

                else {
                    tracing::trace!("Comparing hashes");

                    // And if files' sizes are same we should compare their hashes
                    match std::fs::read(&file_path) {
                        Ok(hash) => format!("{:x}", Md5::digest(hash)).to_lowercase() == self.md5.to_lowercase(),
                        Err(_) => false
                    }
                }
            },
            Err(_) => false
        }
    }

    /// Compare files' sizes and do not compare files' hashes. Works lots faster than `verify`
    #[tracing::instrument(level = "trace", ret)]
    pub fn fast_verify<T: Into<PathBuf> + std::fmt::Debug>(&self, game_path: T) -> bool {
        tracing::trace!("Verifying file");

        match std::fs::metadata(game_path.into().join(&self.path)) {
            Ok(metadata) => metadata.len() == self.size,
            Err(_) => false
        }
    }

    /// Replace remote file with the latest one
    /// 
    /// This method doesn't compare them, so you should do it manually
    #[tracing::instrument(level = "debug", ret)]
    pub fn repair<T: Into<PathBuf> + std::fmt::Debug>(&self, game_path: T) -> Result<(), DownloadingError> {
        tracing::debug!("Repairing file");

        let mut downloader = Downloader::new(format!("{}/{}", self.base_url, self.path.to_string_lossy()))?;

        // Obviously re-download file entirely
        downloader.continue_downloading = false;

        downloader.download_to(game_path.into().join(&self.path), |_, _| {})
    }
}

/// Calculate difference between actual files stored in `game_dir`, and files listed in `used_files`
/// 
/// Returned difference will contain files that are not used by the game and should (or just can) be deleted
/// 
/// `used_files` can be both absolute and relative to `game_dir`
pub fn try_get_unused_files<T, F, U>(game_dir: T, used_files: F, skip_names: U) -> anyhow::Result<Vec<PathBuf>>
where
    T: Into<PathBuf>,
    F: IntoIterator<Item = PathBuf>,
    U: IntoIterator<Item = String>
{
    fn list_files(path: PathBuf, skip_names: &[String]) -> std::io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for entry in std::fs::read_dir(&path)? {
            let entry = entry?;
            let entry_path = path.join(entry.file_name());

            let mut should_skip = false;

            for skip in skip_names {
                if entry.file_name().to_string_lossy().contains(skip) {
                    should_skip = true;

                    break;
                }
            }

            if !should_skip {
                if entry.file_type()?.is_dir() {
                    files.append(&mut list_files(entry_path, skip_names)?);
                }
    
                else {
                    files.push(entry_path);
                }
            }
        }

        Ok(files)
    }

    let used_files = used_files.into_iter()
        .map(|path| path.into())
        .collect::<HashSet<PathBuf>>();

    let skip_names = skip_names.into_iter()
        .collect::<Vec<String>>();

    let game_dir = game_dir.into();

    Ok(list_files(game_dir.clone(), skip_names.as_slice())?
        .into_iter()
        .filter(move |path| {
            // File persist in used_files => unused
            if used_files.contains(path) {
                return false;
            }

            // File persist in used_files => unused
            if let Ok(path) = path.strip_prefix(&game_dir) {
                if used_files.contains(path) {
                    return false;
                }
            }

            // File not persist in used_files => not unused
            return true;
        })
        .collect())
}
