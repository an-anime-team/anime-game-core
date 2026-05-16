use std::path::{Path, PathBuf};

use crate::version::Version;
use crate::traits::prelude::*;
use super::api;
use super::consts::*;
use super::version_diff::*;

fn get_version_from_game_files(
    file: &Path,
    stored_version: &Option<Version>
) -> anyhow::Result<Option<Version>> {
    crate::version_detect::get_version_from_game_files::<45000, 10000>(
        file,
        stored_version,
        0..=0u8,
        0..=0u8
    )
}

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

        let stored_version_path = self.path.join(".version");
        let stored_version = crate::version_detect::parse_dotversion(&stored_version_path);

        if let Some(version_from_files) = get_version_from_game_files(
            self.path
                .join(self.edition.data_folder())
                .join("globalgamemanagers")
                .as_ref(),
            &stored_version
        )? {
            tracing::info!(
                version = version_from_files.to_string(),
                "Found game version from game files"
            );
            return Ok(version_from_files);
        }

        if let Some(stored_version) = stored_version {
            tracing::info!(version = stored_version.to_string(), "Found stored version");
            return Ok(stored_version);
        }

        if let Some(game_scan_version) = crate::version_detect::get_version_game_scan(
            self.path.join(self.edition.exe_name()).as_ref(),
            self.edition.game_scan_url(),
            self.edition.api_game_id()
        )? {
            tracing::info!(
                version = game_scan_version.to_string(),
                "Found game version through game scan API"
            );
            return Ok(game_scan_version);
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

                                    uri: diff.game_pkgs[0].url.clone(), /* TODO: can be a hard
                                                                         * issue in future */
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

                            uri: diff.game_pkgs[0].url.clone(), /* TODO: can be a hard issue in
                                                                 * future */
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
                temp_folder: None
            })
        }
    }
}
