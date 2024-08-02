use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::version::Version;
use crate::traits::game::GameExt;

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
    fn edition(&self) -> Self::Edition {
        self.edition
    }

    #[tracing::instrument(level = "trace", ret)]
    /// Try to get latest game version
    fn get_latest_version(edition: Self::Edition) -> anyhow::Result<Version> {
        tracing::trace!("Trying to get latest game version");

        // I assume game's API can't return incorrect version format right? Right?
        Ok(Version::from_str(api::request(edition)?.data.game.latest.version).unwrap())
    }

    #[tracing::instrument(level = "debug", ret)]
    fn get_version(&self) -> anyhow::Result<Version> {
        tracing::debug!("Trying to get installed game version");

        fn bytes_to_num(bytes: &Vec<u8>) -> u8 {
            bytes.iter().fold(0u8, |acc, &x| acc * 10 + (x - '0' as u8))
        }

        let stored_version = std::fs::read(self.path.join(".version"))
            .map(|version| Version::new(version[0], version[1], version[2]))
            .ok();

        let file = File::open(self.path.join(self.edition.data_folder()).join("globalgamemanagers"))?;

        let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
        let mut version_ptr: usize = 0;
        let mut correct = true;

        for byte in file.bytes().skip(4000).take(10000) {
            if let Ok(byte) = byte {
                match byte {
                    0 => {
                        if correct && version_ptr == 2 && version[0].len() > 0 && version[1].len() > 0 && version[2].len() > 0 {
                            let found_version = Version::new(
                                bytes_to_num(&version[0]),
                                bytes_to_num(&version[1]),
                                bytes_to_num(&version[2])
                            );
    
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

                    46 => {
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

        let latest = api::request(self.edition)?.data.game.latest;

        if self.is_installed() {
            let current = self.get_version()?;

            if current >= latest.version {
                tracing::debug!("Game version is latest");

                Ok(VersionDiff::Latest(current))
            }

            else {
                tracing::debug!("Game is outdated: {} -> {}", current, latest.version);

                Ok(VersionDiff::Diff {
                    current,
                    latest: Version::from_str(latest.version).unwrap(),
                    url: latest.path,

                    downloaded_size: latest.package_size.parse::<u64>().unwrap(),
                    unpacked_size: latest.size.parse::<u64>().unwrap(),

                    installation_path: Some(self.path.clone()),
                    version_file_path: None,
                    temp_folder: None
                })
            }
        }

        else {
            tracing::debug!("Game is not installed");

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(&latest.version).unwrap(),
                url: latest.path,

                downloaded_size: latest.package_size.parse::<u64>().unwrap(),
                unpacked_size: latest.size.parse::<u64>().unwrap(),

                installation_path: Some(self.path.clone()),
                version_file_path: None,
                temp_folder: None
            })
        }
    }
}
