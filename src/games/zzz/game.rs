use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::version::Version;
use crate::traits::prelude::*;

use super::api;
use super::consts::*;
use super::version_diff::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    path: PathBuf,
    edition: GameEdition
}

impl GameExt for Game {
    type Edition = GameEdition;

    #[inline]
    fn new(path: impl Into<PathBuf>, edition: GameEdition) -> Self {
        Self {
            path: path.into(),
            edition
        }
    }

    #[inline]
    fn path(&self) -> &Path {
        self.path.as_path()
    }

    #[inline]
    fn edition(&self) -> GameEdition {
        self.edition
    }

    #[tracing::instrument(level = "trace", ret)]
    /// Try to get latest game version
    fn get_latest_version(edition: GameEdition) -> anyhow::Result<Version> {
        tracing::trace!("Trying to get latest game version");

        // I assume game's API can't return incorrect version format right? Right?
        Ok(Version::from_str(api::request(edition)?.main.major.version).unwrap())
    }

    #[tracing::instrument(level = "debug", ret)]
    fn get_version(&self) -> anyhow::Result<Version> {
        tracing::debug!("Trying to get installed game version");

        fn bytes_to_num(bytes: &Vec<u8>) -> u8 {
            bytes.iter().fold(0u8, |acc, &x| acc * 10 + (x - b'0'))
        }

        let stored_version = std::fs::read(self.path.join(".version"))
            .map(|version| Version::new(version[0], version[1], version[2]))
            .ok();

        let file = File::open(self.path.join(self.edition.data_folder()).join("globalgamemanagers"))?;

        let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
        let mut version_ptr: usize = 0;
        let mut correct = true;

        for byte in file.bytes().skip(4000).take(10000).flatten() {
            match byte {
                0 => {
                    if correct && !version[0].is_empty() && !version[1].is_empty() && !version[2].is_empty() {
                        let found_version = Version::new(
                            bytes_to_num(&version[0]),
                            bytes_to_num(&version[1]),
                            bytes_to_num(&version[2])
                        );

                        // Little workaround for the minor game patch versions (notably 1.0.1)
                        // Prioritize version stored in the .version file
                        // because it's parsed from the API directly
                        if let Some(stored_version) = stored_version {
                            if stored_version > found_version {
                                return Ok(stored_version);
                            }
                        }

                        return Ok(found_version);
                    }

                    version = [vec![], vec![], vec![]];
                    version_ptr = 0;
                    correct = true;
                }

                b'.' => {
                    version_ptr += 1;

                    if version_ptr > 2 {
                        correct = false;
                    }
                }

                _ => {
                    if correct && b"0123456789".contains(&byte) {
                        version[version_ptr].push(byte);
                    }

                    else {
                        correct = false;
                    }
                }
            }
        }

        if let Some(stored_version) = stored_version {
            return Ok(stored_version);
        }

        tracing::error!("Version's bytes sequence wasn't found");

        anyhow::bail!("Version's bytes sequence wasn't found");
    }
}

impl Game {
    #[tracing::instrument(level = "debug", ret)]
    pub fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        tracing::debug!("Trying to find version diff for the game");

        let response = api::request(self.edition)?;

        if self.is_installed() {
            let current = match self.get_version() {
                Ok(version) => version,
                Err(err) => {
                    if self.path.exists() && self.path.metadata()?.len() == 0 {
                        let downloaded_size = response.main.major.game_pkgs.iter()
                            .flat_map(|pkg| pkg.size.parse::<u64>())
                            .sum();

                        let unpacked_size = response.main.major.game_pkgs.iter()
                            .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                            .sum::<u64>() - downloaded_size;

                        return Ok(VersionDiff::NotInstalled {
                            latest: Version::from_str(&response.main.major.version).unwrap(),

                            edition: self.edition,

                            downloaded_size,
                            unpacked_size,

                            segments_uris: response.main.major.game_pkgs.into_iter()
                                .map(|segment| segment.url)
                                .collect(),

                            installation_path: Some(self.path.clone()),
                            version_file_path: None,
                            temp_folder: None
                        });
                    }

                    return Err(err);
                }
            };

            if current >= response.main.major.version {
                tracing::debug!("Game version is latest");

                // If we're running latest game version the diff we need to download
                // must always be `predownload.diffs[0]`, but just to be safe I made
                // a loop through possible variants, and if none of them was correct
                // (which is not possible in reality) we should just say thath the game
                // is latest
                if let Some(predownload_info) = response.pre_download {
                    if let Some(predownload_major) = predownload_info.major {
                        for diff in predownload_info.patches {
                            if diff.version == current {
                                let downloaded_size = diff.game_pkgs.iter()
                                    .flat_map(|pkg| pkg.size.parse::<u64>())
                                    .sum();

                                let unpacked_size = diff.game_pkgs.iter()
                                    .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                                    .sum::<u64>() - downloaded_size;

                                return Ok(VersionDiff::Predownload {
                                    current,
                                    latest: Version::from_str(predownload_major.version).unwrap(),

                                    uri: diff.game_pkgs[0].url.clone(), // TODO: can be a hard issue in future
                                    edition: self.edition,

                                    downloaded_size,
                                    unpacked_size,

                                    installation_path: Some(self.path.clone()),
                                    version_file_path: None,
                                    temp_folder: None
                                });
                            }
                        }
                    }
                }

                Ok(VersionDiff::Latest {
                    version: current,
                    edition: self.edition
                })
            }

            else {
                tracing::debug!("Game is outdated: {} -> {}", current, response.main.major.version);

                for diff in response.main.patches {
                    if diff.version == current {
                        let downloaded_size = diff.game_pkgs.iter()
                            .flat_map(|pkg| pkg.size.parse::<u64>())
                            .sum();

                        let unpacked_size = diff.game_pkgs.iter()
                            .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                            .sum::<u64>() - downloaded_size;

                        return Ok(VersionDiff::Diff {
                            current,
                            latest: Version::from_str(response.main.major.version).unwrap(),

                            uri: diff.game_pkgs[0].url.clone(), // TODO: can be a hard issue in future
                            edition: self.edition,

                            downloaded_size,
                            unpacked_size,

                            installation_path: Some(self.path.clone()),
                            version_file_path: None,
                            temp_folder: None
                        });
                    }
                }

                Ok(VersionDiff::Outdated {
                    current,
                    latest: Version::from_str(response.main.major.version).unwrap(),
                    edition: self.edition
                })
            }
        }

        else {
            tracing::debug!("Game is not installed");

            let downloaded_size = response.main.major.game_pkgs.iter()
                .flat_map(|pkg| pkg.size.parse::<u64>())
                .sum();

            let unpacked_size = response.main.major.game_pkgs.iter()
                .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                .sum::<u64>() - downloaded_size;

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(&response.main.major.version).unwrap(),

                edition: self.edition,

                downloaded_size,
                unpacked_size,

                segments_uris: response.main.major.game_pkgs.into_iter()
                    .map(|segment| segment.url)
                    .collect(),

                installation_path: Some(self.path.clone()),
                version_file_path: None,
                temp_folder: None
            })
        }
    }
}
