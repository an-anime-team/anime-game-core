use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::version::Version;
use crate::traits::game::GameExt;

use super::api;
use super::consts::*;

#[cfg(feature = "install")]
use crate::installer::diff::{VersionDiff, TryGetDiff};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    path: PathBuf
}

impl GameExt for Game {
    #[inline]
    fn new<T: Into<PathBuf>>(path: T) -> Self {
        Self {
            path: path.into()
        }
    }

    #[inline]
    fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Try to get latest game version
    #[tracing::instrument(level = "trace", ret)]
    fn get_latest_version() -> anyhow::Result<Version> {
        tracing::trace!("Trying to get latest game version");

        // I assume game's API can't return incorrect version format right? Right?
        Ok(Version::from_str(api::request()?.data.game.latest.version).unwrap())
    }

    #[tracing::instrument(level = "debug", ret)]
    fn get_version(&self) -> anyhow::Result<Version> {
        tracing::debug!("Trying to get installed game version");

        fn bytes_to_num(bytes: &Vec<u8>) -> u8 {
            let mut num: u8 = 0;
        
            for i in 0..bytes.len() {
                num += u8::from(bytes[i] - 48) * u8::pow(10, (bytes.len() - i - 1).try_into().unwrap());
            }
        
            num
        }

        let file = File::open(self.path.join(GameEdition::selected().data_folder()).join("data.unity3d"))?;

        // [0..9]
        let allowed = [48, 49, 50, 51, 52, 53, 54, 55, 56, 57];

        let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
        let mut version_ptr: usize = 0;
        let mut correct = true;

        for byte in file.bytes().skip(2000).take(10000) {
            if let Ok(byte) = byte {
                match byte {
                    0 => {
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

                    38 => {
                        if correct && version[0].len() > 0 && version[1].len() > 0 && version[2].len() > 0 {
                            return Ok(Version::new(
                                bytes_to_num(&version[0]),
                                bytes_to_num(&version[1]),
                                bytes_to_num(&version[2])
                            ))
                        }

                        correct = false;
                    }

                    _ => {
                        if correct && allowed.contains(&byte) {
                            version[version_ptr].push(byte);
                        }

                        else {
                            correct = false;
                        }
                    }
                }
            }
        }

        tracing::error!("Version's bytes sequence wasn't found");

        anyhow::bail!("Version's bytes sequence wasn't found");
    }
}

#[cfg(feature = "install")]
impl TryGetDiff for Game {
    #[tracing::instrument(level = "debug", ret)]
    fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        tracing::debug!("Trying to find version diff for the game");

        let response = api::request()?;

        if self.is_installed() {
            let current = match self.get_version() {
                Ok(version) => version,
                Err(err) => {
                    if self.path.exists() && self.path.metadata()?.len() == 0 {
                        let latest = response.data.game.latest;

                        return Ok(VersionDiff::NotInstalled {
                            latest: Version::from_str(&latest.version).unwrap(),
                            url: latest.path,
                            download_size: latest.size.parse::<u64>().unwrap(),
                            unpacked_size: latest.package_size.parse::<u64>().unwrap(),
                            unpacking_path: Some(self.path.clone()),
                            version_file_path: None
                        });
                    }

                    return Err(err);
                }
            };

            if response.data.game.latest.version == current {
                tracing::debug!("Game version is latest");

                // If we're running latest game version the diff we need to download
                // must always be `predownload.diffs[0]`, but just to be safe I made
                // a loop through possible variants, and if none of them was correct
                // (which is not possible in reality) we should just say thath the game
                // is latest
                if let Some(predownload) = response.data.pre_download_game {
                    for diff in predownload.diffs {
                        if diff.version == current {
                            return Ok(VersionDiff::Predownload {
                                current,
                                latest: Version::from_str(predownload.latest.version).unwrap(),
                                url: diff.path,
                                download_size: diff.size.parse::<u64>().unwrap(),
                                unpacked_size: diff.package_size.parse::<u64>().unwrap(),
                                unpacking_path: Some(self.path.clone()),
                                version_file_path: None
                            });
                        }
                    }
                }

                Ok(VersionDiff::Latest(current))
            }

            else {
                tracing::debug!("Game is outdated: {} -> {}", current, response.data.game.latest.version);

                for diff in response.data.game.diffs {
                    if diff.version == current {
                        return Ok(VersionDiff::Diff {
                            current,
                            latest: Version::from_str(response.data.game.latest.version).unwrap(),
                            url: diff.path,
                            download_size: diff.size.parse::<u64>().unwrap(),
                            unpacked_size: diff.package_size.parse::<u64>().unwrap(),
                            unpacking_path: Some(self.path.clone()),
                            version_file_path: None
                        });
                    }
                }

                Ok(VersionDiff::Outdated {
                    current,
                    latest: Version::from_str(response.data.game.latest.version).unwrap()
                })
            }
        }

        else {
            tracing::debug!("Game is not installed");

            let latest = response.data.game.latest;

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(&latest.version).unwrap(),
                url: latest.path,
                download_size: latest.size.parse::<u64>().unwrap(),
                unpacked_size: latest.package_size.parse::<u64>().unwrap(),
                unpacking_path: Some(self.path.clone()),
                version_file_path: None
            })
        }
    }
}
