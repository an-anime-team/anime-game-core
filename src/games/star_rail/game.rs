use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::file_strings::FileStringsIterator;
use crate::traits::game::GameExt;
use crate::version::Version;

use super::api;
use super::consts::*;
use super::version_diff::*;

use super::voice_data::locale::VoiceLocale;
use super::voice_data::package::VoicePackage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    path: PathBuf,
    edition: GameEdition,
}

impl GameExt for Game {
    type Edition = GameEdition;

    #[inline]
    fn new(path: impl Into<PathBuf>, edition: GameEdition) -> Self {
        Self {
            path: path.into(),
            edition,
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

        let stored_version = std::fs::read(self.path.join(".version"))
            .map(|version| Version::new(version[0], version[1], version[2]))
            .ok();

        let data_folder = self.edition.data_folder();
        let file_path = self.path.join(data_folder).join("data.unity3d");

        let re = Regex::new(r"^([0-9])\.([0-9])\.([0-9])")?;
        let mut version: Option<Version> = None;

        for line in FileStringsIterator::new(file_path)? {
            if let Some(caps) = re.captures(&line) {
                let major = caps.get(1).unwrap().as_str().parse::<u8>()?;
                let minor = caps.get(2).unwrap().as_str().parse::<u8>()?;
                let patch = caps.get(3).unwrap().as_str().parse::<u8>()?;
                version = Some(Version::new(major, minor, patch));
                break;
            }
        }

        if let Some(found_version) = version {
            // Prioritize version stored in the .version file
            // because it's parsed from the API directly
            if let Some(stored_version) = stored_version {
                if stored_version > found_version {
                    return Ok(stored_version);
                }
            }

            return Ok(found_version);
        }

        if let Some(stored_version) = stored_version {
            return Ok(stored_version);
        }

        tracing::error!("Version's bytes sequence wasn't found");

        anyhow::bail!("Version's bytes sequence wasn't found");
    }
}

impl Game {
    /// Get list of installed voice packages
    pub fn get_voice_packages(&self) -> anyhow::Result<Vec<VoicePackage>> {
        let content = std::fs::read_dir(get_voice_packages_path(&self.path, self.edition))?;

        let packages = content
            .into_iter()
            .flatten()
            .flat_map(|entry| {
                VoiceLocale::from_str(entry.file_name().to_string_lossy())
                    .map(|locale| get_voice_package_path(&self.path, self.edition, locale))
                    .map(|path| VoicePackage::new(path, self.edition))
            })
            .flatten()
            .collect();

        Ok(packages)
    }

    #[tracing::instrument(level = "debug", ret)]
    pub fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        tracing::debug!("Trying to find version diff for the game");

        let response = api::request(self.edition)?;

        if self.is_installed() {
            let current = match self.get_version() {
                Ok(version) => version,
                Err(err) => {
                    if self.path.exists() && self.path.metadata()?.len() == 0 {
                        let downloaded_size = response
                            .main
                            .major
                            .game_pkgs
                            .iter()
                            .flat_map(|pkg| pkg.size.parse::<u64>())
                            .sum();

                        let unpacked_size = response
                            .main
                            .major
                            .game_pkgs
                            .iter()
                            .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                            .sum::<u64>()
                            - downloaded_size;

                        return Ok(VersionDiff::NotInstalled {
                            latest: Version::from_str(&response.main.major.version).unwrap(),
                            edition: self.edition,

                            downloaded_size,
                            unpacked_size,

                            segments_uris: response
                                .main
                                .major
                                .game_pkgs
                                .into_iter()
                                .map(|segment| segment.url)
                                .collect(),

                            installation_path: Some(self.path.clone()),
                            version_file_path: None,
                            temp_folder: None,
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
                                let downloaded_size = diff
                                    .game_pkgs
                                    .iter()
                                    .flat_map(|pkg| pkg.size.parse::<u64>())
                                    .sum();

                                let unpacked_size = diff
                                    .game_pkgs
                                    .iter()
                                    .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                                    .sum::<u64>()
                                    - downloaded_size;

                                return Ok(VersionDiff::Predownload {
                                    current,
                                    latest: Version::from_str(predownload_major.version).unwrap(),

                                    uri: diff.game_pkgs[0].url.clone(), // TODO: can be a hard issue in future
                                    edition: self.edition,

                                    downloaded_size,
                                    unpacked_size,

                                    installation_path: Some(self.path.clone()),
                                    version_file_path: None,
                                    temp_folder: None,
                                });
                            }
                        }
                    }
                }

                Ok(VersionDiff::Latest {
                    version: current,
                    edition: self.edition,
                })
            } else {
                tracing::debug!(
                    "Game is outdated: {} -> {}",
                    current,
                    response.main.major.version
                );

                for diff in response.main.patches {
                    if diff.version == current {
                        let downloaded_size = diff
                            .game_pkgs
                            .iter()
                            .flat_map(|pkg| pkg.size.parse::<u64>())
                            .sum();

                        let unpacked_size = diff
                            .game_pkgs
                            .iter()
                            .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                            .sum::<u64>()
                            - downloaded_size;

                        return Ok(VersionDiff::Diff {
                            current,
                            latest: Version::from_str(response.main.major.version).unwrap(),

                            uri: diff.game_pkgs[0].url.clone(), // TODO: can be a hard issue in future
                            edition: self.edition,

                            downloaded_size,
                            unpacked_size,

                            installation_path: Some(self.path.clone()),
                            version_file_path: None,
                            temp_folder: None,
                        });
                    }
                }

                Ok(VersionDiff::Outdated {
                    current,
                    latest: Version::from_str(response.main.major.version).unwrap(),
                    edition: self.edition,
                })
            }
        } else {
            tracing::debug!("Game is not installed");

            let downloaded_size = response
                .main
                .major
                .game_pkgs
                .iter()
                .flat_map(|pkg| pkg.size.parse::<u64>())
                .sum();

            let unpacked_size = response
                .main
                .major
                .game_pkgs
                .iter()
                .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                .sum::<u64>()
                - downloaded_size;

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(&response.main.major.version).unwrap(),
                edition: self.edition,

                downloaded_size,
                unpacked_size,

                segments_uris: response
                    .main
                    .major
                    .game_pkgs
                    .into_iter()
                    .map(|segment| segment.url)
                    .collect(),

                installation_path: Some(self.path.clone()),
                version_file_path: None,
                temp_folder: None,
            })
        }
    }
}
