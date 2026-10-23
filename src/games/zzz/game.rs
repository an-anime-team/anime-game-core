use std::path::{Path, PathBuf};

use anyhow::Context;

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

/// Get the version of the game's main branch
///
/// Unlike the `getGamePackages` endpoint, this one is updated in time, and so
/// can be used to detect the game version which was already released
/// (see `resolve_release`). The request is optional: if the branches API is
/// not available, we will fall back to the packages data.
fn get_branch_version(edition: GameEdition) -> Option<Version> {
    match api::branches(edition) {
        Ok(branch) => branch.main.and_then(|main| Version::from_str(main.tag)),

        Err(err) => {
            tracing::warn!("Failed to get the game's branch info: {err}");

            None
        }
    }
}

/// Get the currently released game version and its patches
///
/// The `getGamePackages` endpoint may keep reporting the previous game version
/// as `main` for some time after the new version release (the new version
/// itself is only listed in `pre_download` in this case), while the
/// `getGameBranches` endpoint is updated in time. We use its `main.tag` value
/// to detect this case and treat the predownload data as the current release.
fn resolve_release<'a>(
    response: &'a api::schema::GamePackage,
    branch_version: Option<Version>
) -> (&'a api::schema::GameLatestInfo, &'a [api::schema::GamePatch]) {
    let main = (&response.main.major, response.main.patches.as_slice());

    let Some(branch_version) = branch_version else {
        return main;
    };

    let Some(main_version) = Version::from_str(&response.main.major.version) else {
        return main;
    };

    // The branch info is outdated, nothing to fix
    if branch_version <= main_version {
        return main;
    }

    let Some(predownload) = response.pre_download.as_ref() else {
        return main;
    };

    let Some(predownload_major) = predownload.major.as_ref() else {
        return main;
    };

    // The branch version doesn't match the predownload version,
    // so we can't tell which one is correct
    if Version::from_str(&predownload_major.version) != Some(branch_version) {
        return main;
    }

    tracing::debug!(
        "Game version {} is released, but the API still reports it as a predownload",
        branch_version
    );

    (predownload_major, predownload.patches.as_slice())
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

        let response = api::request(edition)?;
        let (latest, _) = resolve_release(&response, get_branch_version(edition));

        // I assume game's API can't return incorrect version format right? Right?
        Ok(Version::from_str(&latest.version).unwrap())
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

        // `getGamePackages`' `main` field may be outdated, see `resolve_release`
        let (latest, patches) = resolve_release(&response, get_branch_version(self.edition));

        let latest_version = Version::from_str(&latest.version)
            .context(format!("Failed to parse latest game version: {}", latest.version))?;

        if self.is_installed() {
            let current = match self.get_version() {
                Ok(version) => version,
                Err(err) => {
                    if self.path.exists() {
                        if !self.path.metadata()?.is_dir() {
                            anyhow::bail!("Path is not a directory: {}", self.path.display());
                        }
                        if self
                            .path
                            .read_dir()
                            .context(format!("Checking game dir: {}", self.path.display()))?
                            .count()
                            > 0
                        {
                            anyhow::bail!("Game directory is not empty")
                        }
                        let downloaded_size = latest
                            .game_pkgs
                            .iter()
                            .flat_map(|pkg| pkg.size.parse::<u64>())
                            .sum();

                        let unpacked_size = latest
                            .game_pkgs
                            .iter()
                            .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                            .sum::<u64>()
                            - downloaded_size;

                        return Ok(VersionDiff::NotInstalled {
                            latest: latest_version,

                            edition: self.edition,

                            downloaded_size,
                            unpacked_size,

                            segments_uris: latest
                                .game_pkgs
                                .iter()
                                .map(|segment| segment.url.clone())
                                .collect(),

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
                    latest_version
                );

                for diff in patches {
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
                            latest: latest_version,

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
                    latest: latest_version,
                    edition: self.edition
                })
            }
        }
        else {
            tracing::debug!("Game is not installed");

            let downloaded_size = latest
                .game_pkgs
                .iter()
                .flat_map(|pkg| pkg.size.parse::<u64>())
                .sum();

            let unpacked_size = latest
                .game_pkgs
                .iter()
                .flat_map(|pkg| pkg.decompressed_size.parse::<u64>())
                .sum::<u64>()
                - downloaded_size;

            Ok(VersionDiff::NotInstalled {
                latest: latest_version,

                edition: self.edition,

                downloaded_size,
                unpacked_size,

                segments_uris: latest
                    .game_pkgs
                    .iter()
                    .map(|segment| segment.url.clone())
                    .collect(),

                installation_path: Some(self.path.clone()),
                version_file_path: None,
                temp_folder: None
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimalized copy of the real `getGamePackages` response
    ///
    /// It reproduces the state of the API right after the 3.3.0 game version
    /// release, when the `main` field was still pointing to 3.2.0, while the
    /// 3.3.0 version was only listed as a predownload.
    const PACKAGES: &str = r#"
        {
            "retcode": 0,
            "message": "OK",
            "data": {
                "game_packages": [
                    {
                        "game": {
                            "id": "x6znKlJ0xK",
                            "biz": "nap_cn"
                        },
                        "main": {
                            "major": {
                                "version": "3.2.0",
                                "game_pkgs": [
                                    {
                                        "url": "https://example.com/3.2.0.zip",
                                        "md5": "00000000000000000000000000000000",
                                        "size": "100",
                                        "decompressed_size": "200"
                                    }
                                ],
                                "audio_pkgs": [],
                                "res_list_url": ""
                            },
                            "patches": [
                                {
                                    "version": "3.1.0",
                                    "game_pkgs": [
                                        {
                                            "url": "https://example.com/3.1.0_3.2.0.zip",
                                            "md5": "00000000000000000000000000000000",
                                            "size": "50",
                                            "decompressed_size": "100"
                                        }
                                    ],
                                    "audio_pkgs": []
                                }
                            ]
                        },
                        "pre_download": {
                            "major": {
                                "version": "3.3.0",
                                "game_pkgs": [
                                    {
                                        "url": "https://example.com/3.3.0.zip",
                                        "md5": "00000000000000000000000000000000",
                                        "size": "150",
                                        "decompressed_size": "300"
                                    }
                                ],
                                "audio_pkgs": [],
                                "res_list_url": ""
                            },
                            "patches": [
                                {
                                    "version": "3.2.0",
                                    "game_pkgs": [
                                        {
                                            "url": "https://example.com/3.2.0_3.3.0.zip",
                                            "md5": "00000000000000000000000000000000",
                                            "size": "75",
                                            "decompressed_size": "150"
                                        }
                                    ],
                                    "audio_pkgs": []
                                }
                            ]
                        }
                    }
                ]
            }
        }
    "#;

    fn game_package() -> api::schema::GamePackage {
        let response = serde_json::from_str::<api::schema::Response>(PACKAGES)
            .expect("Failed to parse the packages API response");

        response.data.game_packages.into_iter().next().unwrap()
    }

    #[test]
    fn test_resolve_release_with_released_predownload() {
        let package = game_package();

        let (latest, patches) = resolve_release(&package, Version::from_str("3.3.0"));

        assert_eq!(latest.version, "3.3.0");
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].version, "3.2.0");
    }

    #[test]
    fn test_resolve_release_with_not_released_predownload() {
        let package = game_package();

        let (latest, _) = resolve_release(&package, Version::from_str("3.2.0"));

        assert_eq!(latest.version, "3.2.0");
    }

    #[test]
    fn test_resolve_release_without_branch_info() {
        let package = game_package();

        let (latest, _) = resolve_release(&package, None);

        assert_eq!(latest.version, "3.2.0");
    }

    #[test]
    fn test_resolve_release_with_unknown_branch_version() {
        let package = game_package();

        let (latest, _) = resolve_release(&package, Version::from_str("3.4.0"));

        assert_eq!(latest.version, "3.2.0");
    }
}
