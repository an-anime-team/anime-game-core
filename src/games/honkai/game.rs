use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

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

        let response = api::request(self.edition)?;

        if self.is_installed() {
            let current = self.get_version()?;

            if current >= response.main.major.version {
                tracing::debug!("Game version is latest");

                Ok(VersionDiff::Latest(current))
            }
            else {
                tracing::debug!(
                    "Game is outdated: {} -> {}",
                    current,
                    response.main.major.version
                );

                Ok(VersionDiff::Diff {
                    current,
                    latest: Version::from_str(response.main.major.version).unwrap(),

                    // TODO: can be a hard issue in future
                    url: response.main.major.game_pkgs[0].url.clone(),

                    downloaded_size: response
                        .main
                        .major
                        .game_pkgs
                        .iter()
                        .flat_map(|pkg| pkg.size.parse::<u64>())
                        .sum(),

                    unpacked_size: response
                        .main
                        .major
                        .game_pkgs
                        .iter()
                        .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                        .sum(),

                    installation_path: Some(self.path.clone()),
                    version_file_path: None,
                    temp_folder: None
                })
            }
        }
        else {
            tracing::debug!("Game is not installed");

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(&response.main.major.version).unwrap(),

                // TODO: can be a hard issue in future
                url: response.main.major.game_pkgs[0].url.clone(),

                downloaded_size: response
                    .main
                    .major
                    .game_pkgs
                    .iter()
                    .flat_map(|pkg| pkg.size.parse::<u64>())
                    .sum(),

                unpacked_size: response
                    .main
                    .major
                    .game_pkgs
                    .iter()
                    .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                    .sum(),

                installation_path: Some(self.path.clone()),
                version_file_path: None,
                temp_folder: None
            })
        }
    }
}
