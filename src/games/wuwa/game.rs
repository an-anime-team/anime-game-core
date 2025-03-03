use std::path::{Path, PathBuf};

use md5::{Md5, Digest};

use crate::version::Version;
use crate::traits::game::GameExt;

use super::api;
use super::consts::*;
use super::version_diff::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    path: PathBuf,
    edition: GameEdition,

    /// Compare files sizes instead of computing md5 hashes. `false` by default
    pub fast_verify: bool
}

impl GameExt for Game {
    type Edition = GameEdition;

    #[inline]
    fn new(path: impl Into<PathBuf>, edition: Self::Edition) -> Self {
        Self {
            path: path.into(),
            edition,
            fast_verify: false
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
    fn get_latest_version(edition: GameEdition) -> anyhow::Result<Version> {
        tracing::trace!("Trying to get latest game version");

        // I assume game's API can't return incorrect version format right? Right?
        Ok(Version::from_str(api::game::request(edition)?.default.version).unwrap())
    }

    #[tracing::instrument(level = "debug", ret)]
    fn get_version(&self) -> anyhow::Result<Version> {
        tracing::debug!("Trying to get installed game version");

        if self.path.join(".version").exists() {
            let version = std::fs::read(self.path.join(".version"))?;

            return Ok(Version::new(
                version[0],
                version[1],
                version[2]
            ));
        }

        tracing::error!("Version's bytes sequence wasn't found");
        
        anyhow::bail!("Version's bytes sequence wasn't found");
    }
}

impl Game {
    #[inline]
    pub fn with_fast_verify(self, fast_verify: bool) -> Self {
        Self {
            fast_verify,
            ..self
        }
    }

    pub fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        tracing::debug!("Trying to find version diff for the game");

        let response = api::game::request(self.edition)?;
        let cdn_url = api::find_cdn_uri(self.edition)?;

        let latest_version = Version::from_str(&response.default.version).unwrap();
        let resources_base_path = &response.default.resourcesBasePath;
        let unpacked_url = format!("{cdn_url}/{resources_base_path}");

        let resource_files = api::resource::request(self.edition)?;
        let total_size: u64 = resource_files.resource.iter()
            .map(|file| file.size)
            .sum();

        let files: Vec<String> = resource_files.resource.iter()
            .map(|file| file.dest.strip_prefix('/').unwrap_or(&file.dest).to_string())
            .collect();

        if self.is_installed() {
            let current = match self.get_version() {
                Ok(version) => version,
                Err(err) => {
                    // Handle empty install folder case
                    if self.path.exists() && self.path.metadata()?.len() == 0 {
                        tracing::debug!("Game folder exists but appears empty");

                        return Ok(VersionDiff::NotInstalled {
                            latest: latest_version,
                            unpacked_url,
                            files,
                            total_size,
                            installation_path: Some(self.path.clone()),
                            version_file_path: None,
                            threads: DEFAULT_DOWNLOADER_THREADS
                        });
                    }

                    return Err(err)
                }
            };

            if current >= latest_version {
                tracing::debug!("Game version is latest");

                Ok(VersionDiff::Latest(current))
            }

            else {
                tracing::debug!("Game is outdated: {} -> {}", current, latest_version);

                Ok(VersionDiff::Outdated {
                    current,
                    latest: latest_version,
                    unpacked_url,
                    files,
                    total_size,
                    installation_path: Some(self.path.clone()),
                    version_file_path: None,
                    threads: DEFAULT_DOWNLOADER_THREADS
                })
            }
        }

        else {
            tracing::debug!("Game is not installed");

            Ok(VersionDiff::NotInstalled {
                latest: latest_version,
                unpacked_url,
                files,
                total_size,

                installation_path: Some(self.path.clone()),
                version_file_path: None,

                threads: DEFAULT_DOWNLOADER_THREADS
            })
        }
    }
}
