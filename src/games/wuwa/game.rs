use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use md5::{Md5, Digest};

use crate::version::Version;
use crate::traits::game::GameExt;

use super::api;
use super::consts::*;
use super::version_diff::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    path: PathBuf,
    edition: GameEdition,

    /// Compare files sizes instead of computing md5 hashes. `false` by default
    pub fast_verify: bool
}

impl GameExt for Game {
    type Edition = GameEdition;

    #[inline]
    fn new(path: impl Into<PathBuf>, edition: Self::Edition) -> Self {
        Self {
            path: path.into(),
            edition,
            fast_verify: false
        }
    }

    #[inline]
    fn path(&self) -> &Path {
        self.path.as_path()
    }

    #[inline]
    fn edition(&self) -> Self::Edition {
        self.edition
    }

    #[tracing::instrument(level = "trace", ret)]
    /// Try to get latest game version
    fn get_latest_version(edition: GameEdition) -> anyhow::Result<Version> {
        tracing::trace!("Trying to get latest game version");

        // I assume game's API can't return incorrect version format right? Right?
        Ok(Version::from_str(api::game::request(edition)?.default.version).unwrap())
    }

    #[tracing::instrument(level = "debug", ret)]
    fn get_version(&self) -> anyhow::Result<Version> {
        tracing::debug!("Trying to get installed game version");

        if self.path.join(".version").exists() {
            let version = std::fs::read(self.path.join(".version"))?;

            return Ok(Version::new(
                version[0],
                version[1],
                version[2]
            ));
        }

        tracing::error!("Version's bytes sequence wasn't found");
        
        anyhow::bail!("Version's bytes sequence wasn't found");
    }
}

impl Game {
    #[inline]
    pub fn with_fast_verify(self, fast_verify: bool) -> Self {
        Self {
            fast_verify,
            ..self
        }
    }

    pub fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        tracing::debug!("Trying to find version diff for the game");

        fn get_files(edition: GameEdition, game_path: &PathBuf, fast_verify: bool) -> anyhow::Result<(Vec<String>, u64)> {
            let mut files = Vec::new();
            let mut total_size = 0;

            for mut file in api::resource::request(edition)?.resource {
                // Remove "/" from the beginning of the path
                file.dest = file.dest.strip_prefix('/').unwrap().to_string();

                let file_path = game_path.join(&file.dest);

                // Add file here if it is not downloaded
                if !file_path.exists() {
                    files.push(file.dest.clone());

                    total_size += file.size;
                }

                // Or try to get downloaded file's metadata
                else if let Ok(metadata) = file_path.metadata() {
                    // And compare updated file size with downloaded one. If they're equal,
                    // then as well compare their md5 hashes if fast_verify = false
                    if metadata.len() != file.size || (!fast_verify && format!("{:x}", Md5::digest(std::fs::read(file_path)?)).to_ascii_lowercase() != file.md5.to_ascii_lowercase()) {
                        files.push(file.dest.clone());

                        // Add only files difference in size to the total download size
                        // If remote file is smaller than downloaded, then total value will decrease
                        total_size += file.size - metadata.len();
                    }
                }
            }

            // TODO:

            // Push `globalgamemanagers` to the end of the list to not to break launcher compatibility
            // let game_data_file = format!("{DATA_FOLDER_NAME}/globalgamemanagers");
            //
            // if files.contains(&game_data_file) {
            //     files.retain(|file| file != &game_data_file);
            //     files.push(game_data_file);
            // }

            Ok((files, total_size))
        }

        let latest = api::game::request(self.edition)?.default;

        if let Ok(current) = self.get_version() {
            if current >= Version::from_str(&latest.version).unwrap() {
                tracing::debug!("Game version is latest");

                Ok(VersionDiff::Latest(current))
            }

            else {
                tracing::debug!("Game is outdated: {} -> {}", current, latest.version);

                let (files, total_size) = get_files(self.edition, &self.path, self.fast_verify)?;

                Ok(VersionDiff::Outdated {
                    current,
                    latest: Version::from_str(latest.version).unwrap(),

                    unpacked_url: format!("{}/{}", api::find_cdn_uri(self.edition)?, latest.resourcesBasePath),
                    files,
                    total_size,

                    installation_path: Some(self.path.clone()),
                    version_file_path: None,

                    threads: DEFAULT_DOWNLOADER_THREADS
                })
            }
        }

        else {
            tracing::debug!("Game is not installed");

            let (files, total_size) = get_files(self.edition, &self.path, self.fast_verify)?;

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(&latest.version).unwrap(),

                unpacked_url: format!("{}/{}", api::find_cdn_uri(self.edition)?, latest.resourcesBasePath),
                files,
                total_size,

                installation_path: Some(self.path.clone()),
                version_file_path: None,

                threads: DEFAULT_DOWNLOADER_THREADS
            })
        }
    }
}
