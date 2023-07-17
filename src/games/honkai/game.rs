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
    path: PathBuf
}

impl GameExt for Game {
    type Edition = ();

    #[inline]
    fn new(path: impl Into<PathBuf>, _edition: ()) -> Self {
        Self {
            path: path.into()
        }
    }

    #[inline]
    fn path(&self) -> &Path {
        self.path.as_path()
    }

    #[inline]
    fn edition(&self) -> Self::Edition {
        ()
    }

    #[tracing::instrument(level = "trace", ret)]
    /// Try to get latest game version
    fn get_latest_version(edition: Self::Edition) -> anyhow::Result<Version> {
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

        let file = File::open(self.path.join(DATA_FOLDER_NAME).join("globalgamemanagers"))?;

        // [0..9]
        let allowed = [48, 49, 50, 51, 52, 53, 54, 55, 56, 57];

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

        tracing::error!("Version's bytes sequence wasn't found");

        anyhow::bail!("Version's bytes sequence wasn't found");
    }
}

impl Game {
    #[tracing::instrument(level = "debug", ret)]
    pub fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        tracing::debug!("Trying to find version diff for the game");

        let latest = api::request()?.data.game.latest;

        if self.is_installed() {
            let current = self.get_version()?;

            // Hon-kai doesn't have game predownloading feature for launcher,
            // and just makes players completely reinstall the game, as I know
            if latest.version == current {
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
