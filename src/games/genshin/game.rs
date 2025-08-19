use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::sophon;
use crate::version::Version;
use crate::traits::prelude::*;
use super::api;
use super::consts::*;
use super::version_diff::*;
use super::voice_data::locale::VoiceLocale;
use super::voice_data::package::VoicePackage;

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

    #[inline]
    fn is_installed(&self) -> bool {
        self.path
            .join(self.edition.data_folder())
            .join("globalgamemanagers")
            .exists()
    }

    #[tracing::instrument(level = "trace", ret)]
    /// Try to get latest game version
    fn get_latest_version(edition: GameEdition) -> anyhow::Result<Version> {
        tracing::trace!("Trying to get latest game version");

        let version = api::request(edition)?.main.major.version;

        Version::from_str(&version).ok_or_else(|| {
            anyhow::anyhow!("api returned invalid game version format").context(version)
        })
    }

    #[tracing::instrument(level = "debug", ret)]
    fn get_version(&self) -> anyhow::Result<Version> {
        tracing::debug!("Trying to get installed game version");

        #[inline]
        fn bytes_to_num(bytes: &[u8]) -> u8 {
            bytes.iter().fold(0u8, |acc, &x| acc * 10 + (x - b'0'))
        }

        let stored_version = std::fs::read(self.path.join(".version"))
            .map(|version| Version::new(version[0], version[1], version[2]))
            .ok();

        let path = self
            .path
            .join(self.edition.data_folder())
            .join("globalgamemanagers");

        let file = File::open(path)?;

        let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
        let mut version_ptr: usize = 0;
        let mut correct = true;

        for byte in file.bytes().skip(4000).take(10000).flatten() {
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
                    if correct
                        && !version[0].is_empty()
                        && !version[1].is_empty()
                        && !version[2].is_empty()
                    {
                        let found_version = Version::new(
                            bytes_to_num(&version[0]),
                            bytes_to_num(&version[1]),
                            bytes_to_num(&version[2])
                        );

                        // Little workaround for the minor game patch versions (notably 1.0.1)
                        // Prioritize version stored in the .version file
                        // because it's parsed from the API directly
                        if let Some(stored_version) = stored_version {
                            if stored_version > found_version {
                                return Ok(stored_version);
                            }
                        }

                        return Ok(found_version);
                    }

                    correct = false;
                }

                _ => {
                    if correct && b"0123456789".contains(&byte) {
                        version[version_ptr].push(byte);
                    }
                    else {
                        correct = false;
                    }
                }
            }
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

        let game_edition = self.edition;

        let client = reqwest::blocking::Client::new();

        let game_branches = sophon::get_game_branches_info(&client, game_edition.into())?;

        let latest_version = game_branches
            .latest_version_by_id(self.edition.game_id())
            .ok_or_else(|| {
                anyhow::anyhow!("failed to find the latest game version")
                    .context(format!("game id: {}", game_edition.game_id()))
            })?;

        let branch_info = game_branches
            .get_game_by_id(self.edition.game_id(), latest_version)
            .ok_or_else(|| {
                anyhow::anyhow!("failed to get the game version information")
                    .context(format!("game id: {}", game_edition.game_id()))
                    .context(format!("game version: {latest_version}"))
            })?;

        if self.is_installed() {
            let current = match self.get_version() {
                Ok(version) => version,

                Err(err) => {
                    if self.path.exists() && self.path.metadata()?.len() == 0 {
                        let game_downloads = sophon::installer::get_game_download_sophon_info(
                            &client,
                            &branch_info.main,
                            game_edition.into()
                        )?;

                        let download_info = game_downloads
                            .get_manifests_for("game")
                            .cloned()
                            .ok_or_else(|| anyhow::anyhow!("failed to get game manifest"))?;

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
                // (which is not possible in reality) we should just say thath the game
                // is latest
                if let Some(predownload_info) = &branch_info.pre_download {
                    if predownload_info
                        .diff_tags
                        .iter()
                        .any(|pre_diff| *pre_diff == current)
                    {
                        let diffs = sophon::updater::get_game_diffs_sophon_info(
                            &client,
                            predownload_info,
                            game_edition.into()
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

                            download_info: sophon::api_schemas::DownloadOrDiff::Patch(diff_info),
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
                tracing::debug!(
                    current_version = current.to_string(),
                    latest_version = latest_version.to_string(),
                    "Game is outdated"
                );

                let diffs = sophon::updater::get_game_diffs_sophon_info(
                    &client,
                    &branch_info.main,
                    game_edition.into()
                )?;

                if branch_info.main.diff_tags.iter().any(|tag| *tag == current) {
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

            let game_downloads = sophon::installer::get_game_download_sophon_info(
                &client,
                &branch_info.main,
                game_edition.into()
            )?;

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
