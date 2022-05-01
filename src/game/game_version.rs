use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::time::Duration;

use crate::json_schemas;
use crate::Version;
use crate::installer::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DiffError {
    AlreadyLatest,
    RemoteNotAvailable,
    CanNotCalculate
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    from: Version,
    to: Version,
    uri: String,
    size: u64,
    path_to_game: String
}

impl Diff {
    pub fn get_from_version(&self) -> Version {
        self.from
    }
    
    pub fn get_to_version(&self) -> Version {
        self.to
    }

    pub fn get_uri(&self) -> String {
        self.uri.clone()
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    #[cfg(feature = "install")]
    pub fn get_installer(&self) -> Result<Installer, minreq::Error> {
        Installer::new(self.uri.clone())
    }

    #[cfg(feature = "install")]
    pub fn download(&self, params: InstallerParams) -> Result<Duration, Error> {
        self.download_to(self.path_to_game.clone(), params)
    }

    #[cfg(feature = "install")]
    pub fn download_to<T: ToString>(&self, path: T, params: InstallerParams) -> Result<Duration, Error> {
        let path = path.to_string();
        let uri = self.uri.clone();

        match Installer::new(uri) {
            Ok(mut installer) => {
                installer.on_update(params.on_update);

                installer.set_downloader(params.downloader);
                installer.set_downloader_interval(params.downloader_updates_interval);
                installer.set_unpacker_interval(params.unpacker_updates_interval);

                installer.install(path)
            },
            Err(err) => Err(Error::new(ErrorKind::AddrNotAvailable, format!("Installer init error: {:?}", err)))
        }
    }
}

pub struct GameVersion {
    path: String,
    remote: Option<json_schemas::versions::Response>
}

impl GameVersion {
    pub fn new(path: String, remote: Option<json_schemas::versions::Response>) -> GameVersion {
        GameVersion {
            path,
            remote
        }
    }

    /// Try to parse installed game version
    pub fn installed(&self) -> Result<Version, Error> {
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
                                            Self::bytes_to_num(&version[0]),
                                            Self::bytes_to_num(&version[1]),
                                            Self::bytes_to_num(&version[2])
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

    /// Get latest available game version
    /// 
    /// Returns `None` if remote server is not available
    pub fn latest(&self) -> Option<Version> {
        match &self.remote {
            Some(remote) => Some(Version::from_str(remote.data.game.latest.version.as_str())),
            None => None
        }
    }

    /// Check if the current game version is latest available
    /// 
    /// Returns `None` if this can't be checked (remote server is not available or failed to parse currently installed game version)
    pub fn is_latest(&self) -> Option<bool> {
        match self.installed() {
            Ok(installed) => {
                match self.latest() {
                    Some(latest) => Some(installed == latest),
                    None => None
                }
            },
            Err(_) => None
        }
    }

    /// Get difference between currently installed and latest available game versions
    pub fn diff(&self) -> Result<Diff, DiffError> {
        match &self.remote {
            Some(remote) => {
                let curr = self.installed().unwrap();
                let latest = self.latest().unwrap();

                if curr == latest {
                    Err(DiffError::AlreadyLatest)
                }

                else {
                    for diff in &remote.data.game.diffs {
                        if diff.version == curr {
                            return Ok(Diff {
                                from: Version::from_str(diff.version.clone()),
                                to: latest,
                                uri: diff.path.clone(),
                                size: diff.size.parse().unwrap(),
                                path_to_game: self.path.clone()
                            })
                        }
                    }

                    Err(DiffError::CanNotCalculate)
                }
            },
            None => Err(DiffError::RemoteNotAvailable)
        }
    }

    fn bytes_to_num(bytes: &Vec<u8>) -> u8 {
        let mut num: u8 = 0;
    
        for i in 0..bytes.len() {
            num += u8::from(bytes[i] - 48) * u8::pow(10, (bytes.len() - i - 1).try_into().unwrap());
        }
    
        num
    }
}
