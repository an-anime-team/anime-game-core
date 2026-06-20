use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sophon::{SophonError, reqwest};
use sophon::updater::SophonPatcher;
use sophon::api::schemas::sophon_diff::SophonDiff;
use sophon::api::schemas::DownloadOrDiff;
use sophon::api::schemas::sophon_manifests::SophonDownloadInfo;
use sophon::installer::SophonInstaller;
use thiserror::Error;

use crate::honkai::consts::GameEdition;
use crate::version::Version;
use crate::traits::version_diff::VersionDiffExt;
#[cfg(feature = "install")]
use crate::installer::downloader::DownloadingError;

#[derive(Debug)]
pub enum DiffUpdate {
    Installer(sophon::installer::Update),
    Patcher(sophon::updater::Update)
}

impl From<sophon::updater::Update> for DiffUpdate {
    fn from(v: sophon::updater::Update) -> Self {
        Self::Patcher(v)
    }
}

impl From<sophon::installer::Update> for DiffUpdate {
    fn from(v: sophon::installer::Update) -> Self {
        Self::Installer(v)
    }
}

#[derive(Error, Debug)]
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

impl From<minreq::Error> for DiffDownloadingError {
    fn from(error: minreq::Error) -> Self {
        DownloadingError::Minreq(error.to_string()).into()
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

        game_download_info: DownloadOrDiff,
        asb_download_info: DownloadOrDiff,
        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the
        /// `install` method
        ///
        /// This value can be `None`, so `install` will return
        /// `Err(DiffDownloadError::PathNotSpecified)`
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

        // `game` might not ever have an update
        game_diff: Option<SophonDiff>,
        asb_diff: Option<SophonDiff>,

        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the
        /// `install` method
        ///
        /// This value can be `None`, so `install` will return
        /// `Err(DiffDownloadError::PathNotSpecified)`
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
        game_download_info: SophonDownloadInfo,
        asb_download_info: SophonDownloadInfo,
        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the
        /// `install` method
        ///
        /// This value can be `None`, so `install` will return
        /// `Err(DiffDownloadError::PathNotSpecified)`
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
            Self::Latest {
                ..
            }
            | Self::Outdated {
                ..
            } => None,

            // Can be installed
            Self::Diff {
                version_file_path, ..
            }
            | Self::NotInstalled {
                version_file_path, ..
            }
            | Self::Predownload {
                version_file_path, ..
            } => version_file_path.to_owned()
        }
    }

    /// Return currently selected temp folder path
    ///
    /// Default is `std::env::temp_dir()` value
    pub fn temp_folder(&self) -> PathBuf {
        match self {
            // Can't be installed
            Self::Latest {
                ..
            }
            | Self::Outdated {
                ..
            } => std::env::temp_dir(),

            // Can be installed
            Self::Diff {
                temp_folder, ..
            }
            | Self::NotInstalled {
                temp_folder, ..
            }
            | Self::Predownload {
                temp_folder, ..
            } => temp_folder
                .as_ref()
                .map(PathBuf::to_owned)
                .unwrap_or_else(std::env::temp_dir)
        }
    }

    pub fn with_temp_folder(mut self, temp: PathBuf) -> Self {
        match &mut self {
            // Can't be installed
            Self::Latest {
                ..
            }
            | Self::Outdated {
                ..
            } => self,

            // Can be installed
            Self::Predownload {
                temp_folder, ..
            } => {
                *temp_folder = Some(temp);

                self
            }

            Self::Diff {
                temp_folder, ..
            } => {
                *temp_folder = Some(temp);

                self
            }

            Self::NotInstalled {
                temp_folder, ..
            } => {
                *temp_folder = Some(temp);

                self
            }
        }
    }

    fn download_game(
        &self,
        game_download_info: &SophonDownloadInfo,
        asb_download_info: &SophonDownloadInfo,
        thread_count: usize,
        path: impl AsRef<Path>,
        updater: impl Fn(<Self as VersionDiffExt>::Update) + Clone + Send + 'static
    ) -> Result<(), <Self as VersionDiffExt>::Error> {
        tracing::debug!(
            path = ?path.as_ref(),
            info = ?game_download_info,
            "Downloading game"
        );

        let client = reqwest::blocking::Client::new();

        tracing::info_span!("Downloading `asb`").in_scope(|| {
            let mut installer =
                SophonInstaller::new(client.clone(), asb_download_info, self.temp_folder())?;
            installer.chunks_in_mem = true;
            installer.chunks_queue_data_limit = Some(2048 * 1024 * 1024); // 2GiB
            installer.inplace = true;

            let updater_clone = updater.clone();
            installer.install(path.as_ref(), thread_count, move |msg| {
                (updater_clone)(msg.into());
            })?;

            tracing::debug!(
                temp = ?installer.downloading_temp(),
                "Removing game downloading cache"
            );

            let _ = std::fs::remove_dir_all(installer.downloading_temp());
            Ok::<_, SophonError>(())
        })?;

        tracing::info_span!("Downloading `game`").in_scope(|| {
            let mut installer =
                SophonInstaller::new(client, game_download_info, self.temp_folder())?;
            installer.chunks_in_mem = true;
            installer.chunks_queue_data_limit = Some(2048 * 1024 * 1024); // 2GiB
            installer.inplace = true;

            installer.install(path.as_ref(), thread_count, move |msg| {
                (updater)(msg.into());
            })?;

            tracing::debug!(
                temp = ?installer.downloading_temp(),
                "Removing game downloading cache"
            );

            let _ = std::fs::remove_dir_all(installer.downloading_temp());
            Ok::<_, SophonError>(())
        })?;

        // Create `.version` file here even if hdiff patching is failed because
        // it's easier to explain user why he should run files repairer than
        // why he should re-download entire game update because something is failed
        #[allow(unused_must_use)]
        {
            let version_path = self
                .version_file_path()
                .unwrap_or(path.as_ref().join(".version"));

            std::fs::write(version_path, self.latest().to_string());
        }

        Ok(())
    }

    fn patch_game(
        &self,
        from: Version,
        thread_count: usize,
        game_diff: &Option<SophonDiff>,
        asb_diff: &Option<SophonDiff>,
        path: impl AsRef<Path>,
        updater: impl Fn(<Self as VersionDiffExt>::Update) + Clone + Send + 'static
    ) -> Result<(), <Self as VersionDiffExt>::Error> {
        tracing::debug!(
            path = ?path.as_ref(),
            from_version = from.to_string(),
            ?game_diff,
            "Patching game files"
        );

        let client = reqwest::blocking::Client::new();

        if let Some(asb_diff) = asb_diff {
            tracing::info_span!("Updating `asb").in_scope(|| {
                let patcher =
                    SophonPatcher::new(client.clone(), asb_diff, self.temp_folder(), None)?;

                let updater_clone = updater.clone();
                patcher.update(&path, from.into(), thread_count, move |msg| {
                    (updater_clone)(msg.into());
                })?;

                tracing::debug!(
                    temp = ?patcher.files_temp(),
                    "Removing patching cache"
                );

                let _ = std::fs::remove_dir_all(patcher.files_temp());
                Ok::<_, SophonError>(())
            })?;
        }

        if let Some(game_diff) = game_diff {
            tracing::info_span!("Updating `game`").in_scope(|| {
                let patcher = SophonPatcher::new(client, game_diff, self.temp_folder(), None)?;

                patcher.update(&path, from.into(), thread_count, move |msg| {
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

                    std::fs::write(version_path, self.latest().to_string());
                }

                tracing::debug!(
                    temp = ?patcher.files_temp(),
                    "Removing patching cache"
                );

                let _ = std::fs::remove_dir_all(patcher.files_temp());
                Ok::<_, SophonError>(())
            })?;
        }

        Ok(())
    }

    fn pre_download(
        &self,
        game_download_or_patch_info: &DownloadOrDiff,
        asb_download_or_patch_info: &DownloadOrDiff,
        from: Version,
        thread_count: usize,
        updater: impl Fn(<Self as VersionDiffExt>::Update) + Clone + Send + 'static
    ) -> Result<(), <Self as VersionDiffExt>::Error> {
        tracing::debug!(
            from_version = from.to_string(),
            diff = ?game_download_or_patch_info,
            "Predownloading game update"
        );

        let client = reqwest::blocking::Client::new();

        let updater_clone = updater.clone();
        match game_download_or_patch_info {
            DownloadOrDiff::Download(download_info) => {
                let installer =
                    SophonInstaller::new(client.clone(), download_info, self.temp_folder())?;

                installer.pre_download(thread_count, move |msg| {
                    (updater_clone)(msg.into());
                })?;
            }

            DownloadOrDiff::Patch(diff_info) => {
                let patcher =
                    SophonPatcher::new(client.clone(), diff_info, self.temp_folder(), None)?;

                patcher.pre_download(from.into(), thread_count, move |msg| {
                    (updater_clone)(msg.into());
                })?;
            }
        }

        match asb_download_or_patch_info {
            DownloadOrDiff::Download(download_info) => {
                let installer = SophonInstaller::new(client, download_info, self.temp_folder())?;

                installer.pre_download(thread_count, move |msg| {
                    (updater)(msg.into());
                })?;
            }

            DownloadOrDiff::Patch(diff_info) => {
                let patcher = SophonPatcher::new(client, diff_info, self.temp_folder(), None)?;

                patcher.pre_download(from.into(), thread_count, move |msg| {
                    (updater)(msg.into());
                })?;
            }
        }

        Ok(())
    }
}

impl VersionDiffExt for VersionDiff {
    type Edition = GameEdition;
    type Error = DiffDownloadingError;
    type Update = DiffUpdate;

    #[inline]
    fn edition(&self) -> Self::Edition {
        match self {
            Self::Latest {
                edition, ..
            }
            | Self::Predownload {
                edition, ..
            }
            | Self::Diff {
                edition, ..
            }
            | Self::Outdated {
                edition, ..
            }
            | Self::NotInstalled {
                edition, ..
            } => *edition
        }
    }

    fn current(&self) -> Option<Version> {
        match self {
            Self::Latest {
                version: current, ..
            }
            | Self::Predownload {
                current, ..
            }
            | Self::Diff {
                current, ..
            }
            | Self::Outdated {
                current, ..
            } => Some(*current),

            Self::NotInstalled {
                ..
            } => None
        }
    }

    fn latest(&self) -> Version {
        match self {
            Self::Latest {
                version: latest, ..
            }
            | Self::Predownload {
                latest, ..
            }
            | Self::Diff {
                latest, ..
            }
            | Self::Outdated {
                latest, ..
            }
            | Self::NotInstalled {
                latest, ..
            } => *latest
        }
    }

    fn downloaded_size(&self) -> Option<u64> {
        match self {
            // Can't be installed
            Self::Latest {
                ..
            }
            | Self::Outdated {
                ..
            } => None,

            // Can be installed
            Self::Diff {
                downloaded_size, ..
            }
            | Self::Predownload {
                downloaded_size, ..
            }
            | Self::NotInstalled {
                downloaded_size, ..
            } => Some(*downloaded_size)
        }
    }

    fn unpacked_size(&self) -> Option<u64> {
        match self {
            // Can't be installed
            Self::Latest {
                ..
            }
            | Self::Outdated {
                ..
            } => None,

            // Can be installed
            Self::Diff {
                unpacked_size, ..
            }
            | Self::Predownload {
                unpacked_size, ..
            }
            | Self::NotInstalled {
                unpacked_size, ..
            } => Some(*unpacked_size)
        }
    }

    fn installation_path(&self) -> Option<&Path> {
        match self {
            // Can't be installed
            Self::Latest {
                ..
            }
            | Self::Outdated {
                ..
            } => None,

            // Can be installed
            Self::Diff {
                installation_path, ..
            }
            | Self::Predownload {
                installation_path, ..
            }
            | Self::NotInstalled {
                installation_path, ..
            } => match installation_path {
                Some(path) => Some(path.as_path()),
                None => None
            }
        }
    }

    fn downloading_uri(&self) -> Option<String> {
        // because sophon
        None
    }

    fn download_as(
        &mut self,
        _path: impl AsRef<Path>,
        _progress: impl Fn(u64, u64) + Send + 'static
    ) -> Result<(), Self::Error> {
        tracing::debug!("Downloading version difference");

        match self {
            Self::Latest {
                ..
            } => Err(Self::Error::AlreadyLatest),
            _ => Err(Self::Error::MultipleSegments)
        }
    }

    // Reimplemented for predownloading. Since self.file_name() returns None, the
    // implementation provided in trait definition won't work.
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
                DiffUpdate::Patcher(sophon::updater::Update::DownloadingProgressBytes {
                    downloaded_bytes,
                    total_bytes
                }) => {
                    let _ = sender.send((downloaded_bytes, total_bytes));
                }

                DiffUpdate::Installer(sophon::installer::Update::DownloadingProgressBytes {
                    downloaded_bytes,
                    total_bytes
                }) => {
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

    fn install_to(
        &self,
        path: impl AsRef<Path>,
        thread_count: usize,
        updater: impl Fn(Self::Update) + Clone + Send + 'static
    ) -> Result<(), Self::Error> {
        tracing::debug!("Installing version difference");

        match self {
            Self::Latest {
                ..
            } => Err(Self::Error::AlreadyLatest),
            Self::Outdated {
                ..
            } => Err(Self::Error::Outdated),

            Self::Diff {
                game_diff,
                asb_diff,
                current,
                ..
            } => self.patch_game(*current, thread_count, game_diff, asb_diff, path, updater),
            Self::NotInstalled {
                game_download_info,
                asb_download_info,
                ..
            } => self.download_game(
                game_download_info,
                asb_download_info,
                thread_count,
                path,
                updater
            ),

            Self::Predownload {
                game_download_info,
                asb_download_info,
                current,
                ..
            } => self.pre_download(
                game_download_info,
                asb_download_info,
                *current,
                thread_count,
                updater
            )
        }
    }
}
