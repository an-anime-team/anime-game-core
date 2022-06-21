use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::path::Path;

use fs_extra::dir::get_dir_content;

use super::voice_data::package::VoicePackage;
use super::consts::{get_voice_package_path, get_voice_packages_path};
use super::version::Version;
use super::api::API;
use super::installer::diff::*;

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
    pub fn get_voice_packages(&self) -> Result<Vec<VoicePackage>, fs_extra::error::Error> {
        match get_dir_content(get_voice_packages_path(&self.path)) {
            Ok(content) => {
                let mut packages = Vec::new();

                for dir in &content.directories[1..] {
                    if let Some(dir) = Path::new(dir).file_name() {
                        if let Some(package) = VoicePackage::new(get_voice_package_path(&self.path, dir.to_string_lossy())) {
                            packages.push(package);
                        }
                    }
                }

                Ok(packages)
            },
            Err(err) => Err(err)
        }
    }

    /// Try to get difference between currently installed game version and the latest available
    pub fn try_get_diff(&self) -> Result<VersionDiff, Error> {
        match API::try_fetch() {
            Ok(response) => match response.try_json::<super::json_schemas::versions::Response>() {
                Ok(response) => {
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
                                                latest: Version::from_str(response.data.game.latest.version),
                                                url: diff.path,
                                                size: diff.package_size.parse::<u64>().unwrap(),
                                                unpacking_path: self.path.clone()
                                            })
                                        }
                                    }
            
                                    Ok(VersionDiff::Outdated {
                                        current,
                                        latest: Version::from_str(response.data.game.latest.version)
                                    })
                                }
                            },
                            Err(err) => Err(err)
                        }
                    }
                    
                    else {
                        Ok(VersionDiff::NotInstalled {
                            latest: Version::from_str(&response.data.game.latest.version),
                            url: response.data.game.latest.path,
                            size: response.data.game.latest.package_size.parse::<u64>().unwrap(),
                            unpacking_path: self.path.clone()
                        })
                    }
                },
                Err(err) => Err(Error::new(ErrorKind::InvalidData, format!("Failed to decode server response: {}", err.to_string())))
            },
            Err(err) => Err(err)
        }
    }
}
