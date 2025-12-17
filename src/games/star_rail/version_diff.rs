use std::path::{Path, PathBuf};
use std::os::unix::prelude::PermissionsExt;

use serde::{Serialize, Deserialize};
use thiserror::Error;

use super::consts::GameEdition;

use crate::{sophon::{self, SophonError, api_schemas::{DownloadOrDiff, sophon_diff::SophonDiff, sophon_manifests::SophonDownloadInfo}, installer::SophonInstaller, updater::SophonPatcher}, version::Version};
use crate::traits::version_diff::VersionDiffExt;

#[cfg(feature = "install")]
use crate::{
    installer::{
        downloader::{Downloader, DownloadingError},
        installer::Update as InstallerUpdate,
        free_space,
        archives::Archive
    },
    external::hpatchz
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffUpdate {
    CheckingFreeSpace(PathBuf),

    InstallerUpdate(InstallerUpdate),

    SophonInstallerUpdate(sophon::installer::Update),
    SophonPatcherUpdate(sophon::updater::Update),

    ApplyingHdiffStarted,
    ApplyingHdiffProgress(u64, u64),
    ApplyingHdiffFinished,

    RemovingOutdatedStarted,
    RemovingOutdatedProgress(u64, u64),
    RemovingOutdatedFinished
}

impl From<sophon::updater::Update> for DiffUpdate {
    fn from(v: sophon::updater::Update) -> Self {
        Self::SophonPatcherUpdate(v)
    }
}

impl From<sophon::installer::Update> for DiffUpdate {
    fn from(v: sophon::installer::Update) -> Self {
        Self::SophonInstallerUpdate(v)
    }
}

impl From<InstallerUpdate> for DiffUpdate {
    #[inline]
    fn from(update: InstallerUpdate) -> Self {
        Self::InstallerUpdate(update)
    }
}

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffDownloadingError {
    /// Your installation is already up to date and not needed to be updated
    #[error("Component version is already latest")]
    AlreadyLatest,

    /// Current version is too outdated and can't be updated.
    /// It means that you have to download everything from zero
    #[error("Components version is too outdated and can't be updated")]
    Outdated,

    /// When there's multiple urls and you can't save them as a single file
    #[error("Component has multiple downloading urls and can't be saved as a single file")]
    MultipleSegments,

    /// Failed to fetch remove data. Redirected from `Downloader`
    #[error("{0}")]
    DownloadingError(#[from] DownloadingError),

    /// Failed to apply hdiff patch
    #[error("Failed to apply hdiff patch: {0}")]
    HdiffPatch(String),

    /// Installation path wasn't specified. This could happen when you
    /// try to call `install` method on `VersionDiff` that was generated
    /// in `VoicePackage::list_latest`. This method couldn't know
    /// your game installation path and thus indicates that it doesn't know
    /// where this package needs to be installed
    #[error("Path to the component's downloading folder is not specified")]
    PathNotSpecified,

    /// Sophon download/patch error
    #[error("{0}")]
    SophonError(#[from] SophonError)
}

impl From<reqwest::Error> for DiffDownloadingError {
    fn from(error: reqwest::Error) -> Self {
        SophonError::Reqwest(error.to_string()).into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionDiff {
    /// Latest version
    Latest {
        version: Version,
        edition: GameEdition
    },

    /// Component's update can be predownloaded, but you still can use it
    Predownload {
        current: Version,
        latest: Version,

        download_info: DownloadOrDiff,
        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        ///
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        installation_path: Option<PathBuf>,

        /// Optional path to the `.version` file
        version_file_path: Option<PathBuf>,

        /// Temp folder path
        temp_folder: Option<PathBuf>
    },

    /// Component should be updated before using it
    Diff {
        current: Version,
        latest: Version,

        diff: SophonDiff,
        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        ///
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        installation_path: Option<PathBuf>,

        /// Optional path to the `.version` file
        version_file_path: Option<PathBuf>,

        /// Temp folder path
        temp_folder: Option<PathBuf>
    },

    /// Difference can't be calculated because installed version is too old
    Outdated {
        current: Version,
        latest: Version,
        edition: GameEdition
    },

    /// Component is not yet installed
    NotInstalled {
        latest: Version,
        download_info: SophonDownloadInfo,
        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        ///
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        installation_path: Option<PathBuf>,

        /// Optional path to the `.version` file
        version_file_path: Option<PathBuf>,

        /// Temp folder path
        temp_folder: Option<PathBuf>
    }
}

impl VersionDiff {
    /// Get `.version` file path
    pub fn version_file_path(&self) -> Option<PathBuf> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { version_file_path, .. } |
            Self::Diff { version_file_path, .. } |
            Self::NotInstalled { version_file_path, .. } => version_file_path.to_owned()
        }
    }

    /// Return currently selected temp folder path
    ///
    /// Default is `std::env::temp_dir()` value
    pub fn temp_folder(&self) -> PathBuf {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => std::env::temp_dir(),

            // Can be installed
            Self::Predownload { temp_folder, .. } |
            Self::Diff { temp_folder, .. } |
            Self::NotInstalled { temp_folder, .. } => match temp_folder {
                Some(path) => path.to_owned(),
                None => std::env::temp_dir()
            }
        }
    }

    pub fn with_temp_folder(mut self, temp: PathBuf) -> Self {
        match &mut self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => self,

            // Can be installed
            Self::Predownload { temp_folder, .. } => {
                *temp_folder = Some(temp);

                self
            }

            Self::Diff { temp_folder, .. } => {
                *temp_folder = Some(temp);

                self
            }

            Self::NotInstalled { temp_folder, .. } => {
                *temp_folder = Some(temp);

                self
            }
        }
    }

    fn download_game(
        &self,
        download_info: &SophonDownloadInfo,
        thread_count: usize,
        path: impl AsRef<Path>,
        updater: impl Fn(<Self as VersionDiffExt>::Update) + Clone + Send + 'static
    ) -> Result<(), <Self as VersionDiffExt>::Error> {
        tracing::debug!(
            path = ?path.as_ref(),
            info = ?download_info,
            "Downloading game"
        );

        let client = reqwest::blocking::Client::new();

        let installer = SophonInstaller::new(client, download_info, self.temp_folder())?;

        installer.install(path.as_ref(), thread_count, move |msg| {
            (updater)(msg.into());
        })?;

        // Create `.version` file here even if hdiff patching is failed because
        // it's easier to explain user why he should run files repairer than
        // why he should re-download entire game update because something is failed
        #[allow(unused_must_use)]
        {
            let version_path = self
                .version_file_path()
                .unwrap_or(path.as_ref().join(".version"));

            std::fs::write(version_path, self.latest().version);
        }

        tracing::debug!(
            temp = ?installer.downloading_temp(),
            "Removing game downloading cache"
        );

        let _ = std::fs::remove_dir_all(installer.downloading_temp());

        Ok(())
    }

    fn patch_game(
        &self,
        from: Version,
        thread_count: usize,
        diff: &SophonDiff,
        path: impl AsRef<Path>,
        updater: impl Fn(<Self as VersionDiffExt>::Update) + Clone + Send + 'static
    ) -> Result<(), <Self as VersionDiffExt>::Error> {
        tracing::debug!(
            path = ?path.as_ref(),
            from_version = from.to_string(),
            ?diff,
            "Patching game files"
        );

        let client = reqwest::blocking::Client::new();

        let patcher = SophonPatcher::new(client, diff, self.temp_folder())?;

        patcher.update(&path, from, thread_count, move |msg| {
            (updater)(msg.into());
        })?;

        // Create `.version` file here even if hdiff patching is failed because
        // it's easier to explain user why he should run files repairer than
        // why he should re-download entire game update because something is failed
        #[allow(unused_must_use)]
        {
            let version_path = self
                .version_file_path()
                .unwrap_or(path.as_ref().join(".version"));

            std::fs::write(version_path, self.latest().version);
        }

        tracing::debug!(
            temp = ?patcher.files_temp(),
            "Removing patching cache"
        );

        let _ = std::fs::remove_dir_all(patcher.files_temp());

        Ok(())
    }

    fn pre_download(
        &self,
        download_or_patch_info: &DownloadOrDiff,
        from: Version,
        thread_count: usize,
        updater: impl Fn(<Self as VersionDiffExt>::Update) + Clone + Send + 'static
    ) -> Result<(), <Self as VersionDiffExt>::Error> {
        tracing::debug!(
            from_version = from.to_string(),
            diff = ?download_or_patch_info,
            "Predownloading game update"
        );

        let client = reqwest::blocking::Client::new();

        match download_or_patch_info {
            DownloadOrDiff::Download(download_info) => {
                let installer = SophonInstaller::new(client, download_info, self.temp_folder())?;

                installer.pre_download(thread_count, move |msg| {
                    (updater)(msg.into());
                })?;
            }

            DownloadOrDiff::Patch(diff_info) => {
                let patcher = SophonPatcher::new(client, diff_info, self.temp_folder())?;

                patcher.pre_download(from, thread_count, move |msg| {
                    (updater)(msg.into());
                })?;
            }
        }

        Ok(())
    }

    /// Get the matching field value for this diff. Returns none in case of
    /// [`VersionDiff::Latest`] or [`VersionDiff::Outdated`]
    pub fn matching_field(&self) -> Option<&str> {
        match self {
            Self::Latest {
                ..
            }
            | Self::Outdated {
                ..
            } => None,
            Self::Predownload {
                download_info, ..
            } => match download_info {
                DownloadOrDiff::Patch(SophonDiff {
                    matching_field, ..
                })
                | DownloadOrDiff::Download(SophonDownloadInfo {
                    matching_field, ..
                }) => Some(matching_field)
            },
            Self::Diff {
                diff, ..
            } => Some(&diff.matching_field),
            Self::NotInstalled {
                download_info, ..
            } => Some(&download_info.matching_field)
        }
    }
}

impl VersionDiffExt for VersionDiff {
    type Error = DiffDownloadingError;
    type Update = DiffUpdate;
    type Edition = GameEdition;

    fn edition(&self) -> GameEdition {
        match self {
            Self::Latest { edition, .. } |
            Self::Predownload { edition, .. } |
            Self::Diff { edition, .. } |
            Self::Outdated { edition, .. } |
            Self::NotInstalled { edition, .. } => *edition
        }
    }

    fn current(&self) -> Option<Version> {
        match self {
            Self::Latest { version: current, .. } |
            Self::Predownload { current, .. } |
            Self::Diff { current, .. } |
            Self::Outdated { current, .. } => Some(*current),

            Self::NotInstalled { .. } => None
        }
    }

    fn latest(&self) -> Version {
        match self {
            Self::Latest { version: latest, .. } |
            Self::Predownload { latest, .. } |
            Self::Diff { latest, .. } |
            Self::Outdated { latest, .. } |
            Self::NotInstalled { latest, .. } => *latest
        }
    }

    fn downloaded_size(&self) -> Option<u64> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { downloaded_size, .. } |
            Self::Diff { downloaded_size, .. } |
            Self::NotInstalled { downloaded_size, .. } => Some(*downloaded_size)
        }
    }

    fn unpacked_size(&self) -> Option<u64> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { unpacked_size, .. } |
            Self::Diff { unpacked_size, .. } |
            Self::NotInstalled { unpacked_size, .. } => Some(*unpacked_size)
        }
    }

    fn installation_path(&self) -> Option<&Path> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { installation_path, .. } |
            Self::Diff { installation_path, .. } |
            Self::NotInstalled { installation_path, .. } => match installation_path {
                Some(path) => Some(path.as_path()),
                None => None
            }
        }
    }

    fn downloading_uri(&self) -> Option<String> {
        None
    }

    // no singular file to download
    fn download_as(
        &mut self,
        _path: impl AsRef<Path>,
        _progress: impl Fn(u64, u64) + Send + 'static
    ) -> Result<(), Self::Error> {
        tracing::debug!("Downloading version difference");

        match self {
            // Can't be downloaded
            Self::Latest {
                ..
            } => Err(Self::Error::AlreadyLatest),
            Self::Outdated {
                ..
            } => Err(Self::Error::Outdated),

            // Can be downloaded
            // Self::Predownload { uri, .. } |
            // Self::Diff { uri, .. } => uri,

            // Can be installed but amogus
            // Self::NotInstalled { .. } => return Err(Self::Error::MultipleSegments),
            _ => Err(Self::Error::MultipleSegments)
        }
    }

    // Reimplemented for the edge case of predownloading. Since self.file_name()
    // returns None, the implementation provided in trait definition won't work.
    //
    // Implemented based on observation that the method is only called for
    // predownloads in the launcher itself.
    fn download_to(
        &mut self,
        folder: impl AsRef<Path>,
        progress: impl Fn(u64, u64) + Send + 'static
    ) -> Result<(), Self::Error> {
        if matches!(self, Self::Predownload { .. }) {
            // non-sync and non-clone progress callback was provided, so have
            // to do stuff like this
            let (sender, recver) = std::sync::mpsc::channel();

            let proxy_thread_handle = std::thread::spawn(move || {
                while let Ok((downloaded, total)) = recver.recv() {
                    (progress)(downloaded, total);
                }
            });

            self.install_to(folder, 1, move |msg| match msg {
                DiffUpdate::SophonPatcherUpdate(
                    sophon::updater::Update::DownloadingProgressBytes {
                        downloaded_bytes,
                        total_bytes
                    }
                ) => {
                    let _ = sender.send((downloaded_bytes, total_bytes));
                }

                DiffUpdate::SophonInstallerUpdate(
                    sophon::installer::Update::DownloadingProgressBytes {
                        downloaded_bytes,
                        total_bytes
                    }
                ) => {
                    let _ = sender.send((downloaded_bytes, total_bytes));
                }

                _ => ()
            })?;

            proxy_thread_handle
                .join()
                .expect("failed to join game downloader thread");
        }

        Ok(())
    }

    fn install_to(&self, path: impl AsRef<Path>, thread_count: usize, updater: impl Fn(Self::Update) + Clone + Send + 'static) -> Result<(), Self::Error> {
        tracing::debug!("Installing version difference");

        match self {
            // Can't be installed
            Self::Latest {
                ..
            } => Err(Self::Error::AlreadyLatest),
            Self::Outdated {
                ..
            } => Err(Self::Error::Outdated),

            // Can be installed
            Self::Diff {
                diff,
                current,
                ..
            } => self.patch_game(*current, thread_count, diff, path, updater),
            Self::NotInstalled {
                download_info, ..
            } => self.download_game(download_info, thread_count, path, updater),

            // Predownload without applying
            Self::Predownload {
                download_info,
                current,
                ..
            } => self.pre_download(download_info, *current, thread_count, updater)
        }
    }
}
