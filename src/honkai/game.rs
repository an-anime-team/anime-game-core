use std::fs::File;
use std::io::Read;

use crate::version::Version;
use crate::traits::game::GameBasics;

use super::api;
use super::consts::*;

#[cfg(feature = "install")]
use crate::installer::diff::{VersionDiff, TryGetDiff};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    path: String
}

impl GameBasics for Game {
    fn new<T: ToString>(path: T) -> Self {
        Self {
            path: path.to_string()
        }
    }

    fn path(&self) -> &str {
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

        let file = File::open(format!("{}/{}/globalgamemanagers", &self.path, unsafe { DATA_FOLDER_NAME }))?;

        // [0..9, .]
        let allowed: [u8; 11] = [48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 46];

        let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
        let mut version_ptr: usize = 0;
        let mut correct = true;

        for byte in file.bytes().skip(4000).take(10000) {
            if let Ok(byte) = byte {
                match byte {
                    0 => {
                        if correct && version_ptr == 2 && version[0].len() > 0 && version[1].len() > 0 && version[2].len() > 0 {
                            return Ok(Version::new(
                                bytes_to_num(&version[0]),
                                bytes_to_num(&version[1]),
                                bytes_to_num(&version[2])
                            ))
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

#[cfg(feature = "install")]
impl TryGetDiff for Game {
    fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        let latest = api::try_fetch_json()?.data.game.latest;

        if self.is_installed() {
            let current = self.try_get_version()?;

            if latest.version == current {
                Ok(VersionDiff::Latest(current))
            }

            else {
                Ok(VersionDiff::Diff {
                    current,
                    latest: Version::from_str(latest.version).unwrap(),
                    url: latest.path,
                    download_size: latest.size.parse::<u64>().unwrap(),
                    unpacked_size: latest.package_size.parse::<u64>().unwrap(),
                    unpacking_path: Some(self.path.clone())
                })
            }
        }

        else {
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
