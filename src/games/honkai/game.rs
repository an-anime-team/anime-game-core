use std::path::{Path, PathBuf};

use anyhow::Context;
use sophon::reqwest;

use crate::version::Version;
use crate::traits::game::GameExt;
use super::api;
use super::consts::*;
use super::version_diff::*;

fn get_version_from_game_files(
    file: &Path,
    stored_version: &Option<Version>
) -> anyhow::Result<Option<Version>> {
    crate::version_detect::get_version_from_game_files::<4000, 10000>(
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
    fn edition(&self) -> Self::Edition {
        self.edition
    }

    #[tracing::instrument(level = "trace", ret)]
    /// Try to get latest game version
    fn get_latest_version(edition: Self::Edition) -> anyhow::Result<Version> {
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

        let client = reqwest::blocking::Client::new();

        let game_branches = sophon::api::get_game_branches_info(&client, &self.edition.into())
            .inspect_err(|err| tracing::error!(?err, "getting game branches error"))?;

        let branch_info = game_branches
            .get_game_branch_by_id_or_biz_latest(self.edition.api_game_id())
            .ok_or_else(|| {
                anyhow::anyhow!("failed to get the game version information")
                    .context(format!("game id: {}", self.edition.api_game_id()))
            })?;
        let latest_version = branch_info
            .version()
            .expect("must be a valid version")
            .into();

        if self.is_installed() {
            let current = match self.get_version() {
                Ok(version) => version,
                Err(err) => {
                    if self.path.exists() && self.path.metadata()?.len() == 0 {
                        let game_downloads = sophon::api::get_game_download_sophon_info(
                            &client,
                            branch_info
                                .main
                                .as_ref()
                                .expect("The `None` case would have been caught earlier"),
                            &self.edition.into()
                        )
                        .inspect_err(|err| tracing::error!(?err, "getting download info error"))?;

                        let download_info = game_downloads
                            .get_manifests_for("game")
                            .cloned()
                            .ok_or_else(|| anyhow::anyhow!("Failed to get game manifest"))?;

                        let downloaded_size = download_info.stats.compressed_size.parse()?;
                        let unpacked_size = download_info.stats.uncompressed_size.parse()?;

                        return Ok(VersionDiff::NotInstalled {
                            latest: latest_version,

                            edition: self.edition,

                            downloaded_size,
                            unpacked_size,
                            download_info,

                            installation_path: Some(self.path.clone()),
                            version_file_path: None,
                            temp_folder: None
                        });
                    }

                    return Err(err);
                }
            };

            if current >= latest_version {
                tracing::debug!("Game version is latest");

                // If we're running latest game version the diff we need to download
                // must always be `predownload.diffs[0]`, but just to be safe I made
                // a loop through possible variants, and if none of them was correct
                // (which is not possible in reality) we should just say that the game
                // is latest
                if let Some(predownload_info) = &branch_info.pre_download {
                    if predownload_info
                        .diff_tags
                        .iter()
                        .any(|pre_diff| *pre_diff == current)
                    {
                        let diffs = sophon::api::get_game_diffs_sophon_info(
                            &client,
                            predownload_info,
                            &self.edition.into()
                        )?;

                        let diff_info = diffs.get_manifests_for("game").unwrap().clone();

                        return Ok(VersionDiff::Predownload {
                            current,
                            latest: Version::from_str(&predownload_info.tag).unwrap(),

                            downloaded_size: diff_info
                                .stats
                                .get(&current.to_string())
                                .unwrap()
                                .compressed_size
                                .parse()
                                .unwrap(),

                            unpacked_size: diff_info
                                .stats
                                .get(&current.to_string())
                                .unwrap()
                                .uncompressed_size
                                .parse()
                                .unwrap(),

                            download_info: sophon::api::schemas::DownloadOrDiff::Patch(diff_info),
                            edition: self.edition,

                            installation_path: Some(self.path.clone()),
                            version_file_path: None,
                            temp_folder: None
                        });
                    }
                }

                Ok(VersionDiff::Latest {
                    version: current,
                    edition: self.edition
                })
            }
            else {
                tracing::debug!("Game is outdated: {} -> {}", current, latest_version);

                let diffs = sophon::api::get_game_diffs_sophon_info(
                    &client,
                    branch_info
                        .main
                        .as_ref()
                        .expect("The `None` case would have been caught earlier"),
                    &self.edition.into()
                )
                .context("Getting game diffs")?;

                if branch_info
                    .main
                    .as_ref()
                    .expect("The `None` case would have been caught earlier")
                    .diff_tags
                    .iter()
                    .any(|tag| *tag == current)
                {
                    for diff in &diffs.manifests {
                        if diff.matching_field == "game" {
                            if let Some((_, stats)) =
                                diff.stats.iter().find(|(tag, _)| **tag == current)
                            {
                                let downloaded_size = stats.compressed_size.parse()?;
                                let unpacked_size = stats.uncompressed_size.parse()?;

                                return Ok(VersionDiff::Diff {
                                    current,
                                    latest: latest_version,

                                    edition: self.edition,

                                    downloaded_size,
                                    unpacked_size,

                                    diff: diff.clone(),

                                    installation_path: Some(self.path.clone()),
                                    version_file_path: None,
                                    temp_folder: None
                                });
                            }
                        }
                    }
                }

                Ok(VersionDiff::Outdated {
                    current,
                    latest: latest_version,
                    edition: self.edition
                })
            }
        }
        else {
            tracing::debug!("Game is not installed");

            let game_downloads = sophon::api::get_game_download_sophon_info(
                &client,
                branch_info
                    .main
                    .as_ref()
                    .expect("The `None` case would have been caught earlier"),
                &self.edition.into()
            )
            .inspect_err(|err| tracing::error!(?err, "getting download info error"))?;

            let download_info = game_downloads
                .get_manifests_for("game")
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("failed to get game manifest"))?;

            let downloaded_size = download_info.stats.compressed_size.parse()?;
            let unpacked_size = download_info.stats.uncompressed_size.parse()?;

            Ok(VersionDiff::NotInstalled {
                latest: latest_version,
                edition: self.edition,

                downloaded_size,
                unpacked_size,
                download_info,

                installation_path: Some(self.path.clone()),
                version_file_path: None,
                temp_folder: None
            })
        }
    }
}
