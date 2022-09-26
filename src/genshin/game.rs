use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::version::Version;
use crate::traits::game::GameBasics;

use super::api;
use super::voice_data::package::VoicePackage;
use super::consts::*;

#[cfg(feature = "install")]
use crate::installer::diff::{VersionDiff, TryGetDiff};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    path: PathBuf
}

impl GameBasics for Game {
    fn new<T: Into<PathBuf>>(path: T) -> Self {
        Self {
            path: path.into()
        }
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Try to get latest game version
    fn try_get_latest_version() -> anyhow::Result<Version> {
        // I assume game's API can't return incorrect version format right? Right?
        Ok(Version::from_str(api::try_fetch_json()?.data.game.latest.version).unwrap())
    }

    fn try_get_version(&self) -> anyhow::Result<Version> {
        fn bytes_to_num(bytes: &Vec<u8>) -> u8 {
            let mut num: u8 = 0;
        
            for i in 0..bytes.len() {
                num += u8::from(bytes[i] - 48) * u8::pow(10, (bytes.len() - i - 1).try_into().unwrap());
            }
        
            num
        }

        let file = File::open(self.path.join(unsafe { DATA_FOLDER_NAME }).join("globalgamemanagers"))?;

        // [0..9, .]
        let allowed: [u8; 11] = [48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 46];

        let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
        let mut version_ptr: usize = 0;
        let mut correct = true;

        for byte in file.bytes().skip(4000).take(10000) {
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

                    95 => {
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

        Err(anyhow::anyhow!("Version's bytes sequence wasn't found"))
    }
}

impl Game {
    /// Get list of installed voice packages
    pub fn get_voice_packages(&self) -> anyhow::Result<Vec<VoicePackage>> {
        let content = std::fs::read_dir(get_voice_packages_path(&self.path))?;

        let packages = content.into_iter()
            .filter_map(|result| result.ok())
            .filter_map(|entry| {
                let path = get_voice_package_path(&self.path, entry.file_name().to_string_lossy());

                VoicePackage::new(path)
            })
            .collect();

        Ok(packages)
    }

    /// Try to get game's edition from its folder
    /// 
    /// Will return `None` if the game is not installed
    pub fn get_edition(&self) -> Option<GameEdition> {
        if !Path::new(&self.path).exists() {
            return None;
        }

        for edition in [GameEdition::Global, GameEdition::China] {
            if self.path.join(get_data_folder_name(edition)).exists() {
                return Some(edition);
            }
        }

        // Should be unreachable!() instead of None to catch possible future errors
        unreachable!()
    }
}

#[cfg(feature = "install")]
impl TryGetDiff for Game {
    fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        let response = api::try_fetch_json()?;

        if self.is_installed() {
            let current = self.try_get_version()?;

            if response.data.game.latest.version == current {
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
                                unpacking_path: Some(self.path.clone())
                            });
                        }
                    }
                }

                Ok(VersionDiff::Latest(current))
            }

            else {
                for diff in response.data.game.diffs {
                    if diff.version == current {
                        return Ok(VersionDiff::Diff {
                            current,
                            latest: Version::from_str(response.data.game.latest.version).unwrap(),
                            url: diff.path,
                            download_size: diff.size.parse::<u64>().unwrap(),
                            unpacked_size: diff.package_size.parse::<u64>().unwrap(),
                            unpacking_path: Some(self.path.clone())
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
            let latest = response.data.game.latest;

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(&latest.version).unwrap(),
                url: latest.path,
                download_size: latest.size.parse::<u64>().unwrap(),
                unpacked_size: latest.package_size.parse::<u64>().unwrap(),
                unpacking_path: Some(self.path.clone())
            })
        }
    }
}
