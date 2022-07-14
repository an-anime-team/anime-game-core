use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::path::Path;

use super::voice_data::package::VoicePackage;
use super::consts::{get_voice_package_path, get_voice_packages_path};
use super::version::Version;
use super::api::API;

#[cfg(feature = "install")]
use super::installer::diff::{VersionDiff, TryGetDiff};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    path: String
}

impl Game {
    pub fn new<T: ToString>(path: T) -> Self {
        Game {
            path: path.to_string()
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Checks if the game is installed
    pub fn is_installed(&self) -> bool {
        Path::new(&self.path).exists()
    }

    /// Try to get latest game version
    pub fn try_get_latest_version() -> Option<Version> {
        match API::try_fetch_json() {
            Ok(response) => Version::from_str(response.data.game.latest.version),
            Err(_) => None
        }
    }

    /// Try to get installed game version
    pub fn try_get_version(&self) -> Result<Version, Error> {
        fn bytes_to_num(bytes: &Vec<u8>) -> u8 {
            let mut num: u8 = 0;
        
            for i in 0..bytes.len() {
                num += u8::from(bytes[i] - 48) * u8::pow(10, (bytes.len() - i - 1).try_into().unwrap());
            }
        
            num
        }

        match File::open(format!("{}/GenshinImpact_Data/globalgamemanagers", &self.path)) {
            Ok(file) => {
                // [0..9, .]
                let allowed: [u8; 11] = [48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 46];

                let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
                let mut version_ptr: usize = 0;
                let mut correct = true;

                for byte in file.bytes() {
                    match byte {
                        Ok(byte) => {
                            match byte {
                                0 => {
                                    version = [vec![], vec![], vec![]];
                                    version_ptr = 0;
                                    correct = true;
                                },

                                46 => {
                                    version_ptr += 1;

                                    if version_ptr > 2 {
                                        correct = false;
                                    }
                                },

                                95 => {
                                    if correct && version[0].len() > 0 && version[1].len() > 0 && version[2].len() > 0 {
                                        return Ok(Version::new(
                                            bytes_to_num(&version[0]),
                                            bytes_to_num(&version[1]),
                                            bytes_to_num(&version[2])
                                        ))
                                    }
        
                                    correct = false;
                                },

                                _ => {
                                    if correct && allowed.contains(&byte) {
                                        version[version_ptr].push(byte);
                                    }
            
                                    else {
                                        correct = false;
                                    }
                                }
                            }
                        },
                        Err(_) => {}
                    }
                }

                Err(Error::new(ErrorKind::NotFound, "Version's bytes sequence wasn't found"))
            },
            Err(err) => Err(err)
        }
    }

    /// Get list of installed voice packages
    pub fn get_voice_packages(&self) -> Result<Vec<VoicePackage>, Error> {
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
}

// TODO: game predownloading

#[cfg(feature = "install")]
impl TryGetDiff for Game {
    fn try_get_diff(&self) -> Result<VersionDiff, Error> {
        let response = API::try_fetch_json()?;

        if self.is_installed() {
            match self.try_get_version() {
                Ok(current) => {
                    if response.data.game.latest.version == current {
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
                                })
                            }
                        }

                        Ok(VersionDiff::Outdated {
                            current,
                            latest: Version::from_str(response.data.game.latest.version).unwrap()
                        })
                    }
                },
                Err(err) => Err(err)
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
