use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::version::Version;
use crate::traits::version_diff::VersionDiffExt;

#[cfg(feature = "install")]
use crate::{
    installer::{
        downloader::{Downloader, DownloadingError},
        installer::{
            Installer,
            Update as InstallerUpdate
        },
        free_space
    },
    external::hpatchz
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffUpdate {
    InstallerUpdate(InstallerUpdate),

    ApplyingHdiffStarted,
    ApplyingHdiffProgress(u64, u64),
    ApplyingHdiffFinished,

    RemovingOutdatedStarted,
    RemovingOutdatedProgress(u64, u64),
    RemovingOutdatedFinished
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
    PathNotSpecified
}

impl From<minreq::Error> for DiffDownloadingError {
    fn from(error: minreq::Error) -> Self {
        DownloadingError::Minreq(error.to_string()).into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionDiff {
    /// Latest version
    Latest(Version),

    /// Component's update can be predownloaded, but you still can use it
    Predownload {
        current: Version,
        latest: Version,
        url: String,
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
        url: String,
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
        latest: Version
    },

    /// Component is not yet installed
    NotInstalled {
        latest: Version,
        url: String,
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
            Self::Latest(_) |
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
            Self::Latest(_) |
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
            Self::Latest(_) |
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
}

impl VersionDiffExt for VersionDiff {
    type Error = DiffDownloadingError;
    type Update = DiffUpdate;

    fn current(&self) -> Option<Version> {
        match self {
            Self::Latest(current) |
            Self::Predownload { current, .. } |
            Self::Diff { current, .. } |
            Self::Outdated { current, .. } => Some(*current),

            Self::NotInstalled { .. } => None
        }
    }

    fn latest(&self) -> Version {
        match self {
            Self::Latest(latest) |
            Self::Predownload { latest, .. } |
            Self::Diff { latest, .. } |
            Self::Outdated { latest, .. } |
            Self::NotInstalled { latest, .. } => *latest
        }
    }

    fn downloaded_size(&self) -> Option<u64> {
        match self {
            // Can't be installed
            Self::Latest(_) |
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
            Self::Latest(_) |
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
            Self::Latest(_) |
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
        match self {
            // Can't be installed
            Self::Latest(_) |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { url, .. } |
            Self::Diff { url, .. } |
            Self::NotInstalled { url, .. } => Some(url.to_owned())
        }
    }

    fn download_as(&mut self, path: impl AsRef<Path>, progress: impl Fn(u64, u64) + Send + 'static) -> Result<(), Self::Error> {
        tracing::debug!("Downloading version difference");

        let mut downloader = Downloader::new(match self {
            // Can't be downloaded
            Self::Latest(_) => return Err(Self::Error::AlreadyLatest),
            Self::Outdated { .. } => return Err(Self::Error::Outdated),

            // Can be downloaded
            Self::Predownload { url: diff_url, .. } |
            Self::Diff { url: diff_url, .. } |
            Self::NotInstalled { url: diff_url, .. } => diff_url
        })?;

        if let Err(err) = downloader.download(path.as_ref(), progress) {
            tracing::error!("Failed to download version difference: {err}");

            return Err(err.into());
        }

        Ok(())
    }

    fn install_to(&self, path: impl AsRef<Path>, updater: impl Fn(Self::Update) + Clone + Send + 'static) -> Result<(), Self::Error> {
        tracing::debug!("Installing version difference");

        match self {
            // Can't be downloaded
            Self::Latest(_) => return Err(Self::Error::AlreadyLatest),
            Self::Outdated { .. } => return Err(Self::Error::Outdated),

            _ => ()
        }

        let path = path.as_ref();

        let url = self.downloading_uri().expect("Failed to retreive downloading url");
        let downloaded_size = self.downloaded_size().expect("Failed to retreive downloaded size");
        let unpacked_size = self.unpacked_size().expect("Failed to retreive unpacked size");

        let mut installer = Installer::new(url)?
            // Set custom temp folder location
            .with_temp_folder(self.temp_folder())

            // Don't perform space checks in the Installer because we're doing it here
            .with_free_space_check(false);

        // Check available free space for archive itself
        let Some(space) = free_space::available(&installer.temp_folder) else {
            tracing::error!("Path is not mounted: {:?}", installer.temp_folder);

            return Err(DownloadingError::PathNotMounted(installer.temp_folder).into());
        };

        // We can possibly store downloaded archive + unpacked data on the same disk
        let required = if free_space::is_same_disk(&installer.temp_folder, path) {
            downloaded_size + unpacked_size
        } else {
            downloaded_size
        };

        if space < required {
            tracing::error!("No free space available in the temp folder. Required: {required}. Available: {space}");

            return Err(DownloadingError::NoSpaceAvailable(installer.temp_folder, required, space).into());
        }

        // Check available free space for unpacked archvie data
        let Some(space) = free_space::available(&path) else {
            tracing::error!("Path is not mounted: {:?}", installer.temp_folder);

            return Err(DownloadingError::PathNotMounted(path.to_path_buf()).into());
        };

        // We can possibly store downloaded archive + unpacked data on the same disk
        let required = if free_space::is_same_disk(&path, &installer.temp_folder) {
            unpacked_size + downloaded_size
        } else {
            unpacked_size
        };

        if space < required {
            tracing::error!("No free space available in the installation folder. Required: {required}. Available: {space}");

            return Err(DownloadingError::NoSpaceAvailable(path.to_path_buf(), required, space).into());
        }

        // Install data
        let installer_updater = updater.clone();

        installer.install(path, move |update| (installer_updater)(update.into()));

        // Create `.version` file here even if hdiff patching is failed because
        // it's easier to explain user why he should run files repairer than
        // why he should re-download entire game update because something is failed
        #[allow(unused_must_use)] {
            let version_path = self.version_file_path()
                .unwrap_or_else(|| path.join(".version"));

            std::fs::write(version_path, self.latest().version);
        }

        // Apply hdiff patches
        // We're ignoring Err because in practice it means that hdifffiles.txt is missing
        if let Ok(files) = std::fs::read_to_string(path.join("hdifffiles.txt")) {
            tracing::debug!("Applying hdiff patches");

            (updater)(Self::Update::ApplyingHdiffStarted);

            let files = files.lines().collect::<Vec<&str>>();
            let hdiffs = files.len() as u64;

            // {"remoteName": "AnimeGame_Data/StreamingAssets/Audio/GeneratedSoundBanks/Windows/Japanese/1001.pck"}
            for (i, file) in files.into_iter().enumerate() {
                let relative_file = &file[16..file.len() - 2];

                let file = path.join(relative_file);
                let patch = path.join(format!("{relative_file}.hdiff"));
                let output = path.join(format!("{relative_file}.hdiff_patched"));

                // If failed to apply the patch
                if let Err(err) = hpatchz::patch(&file, &patch, &output) {
                    tracing::warn!("Failed to apply hdiff patch for {:?}: {err}", file);
                    tracing::debug!("Trying to repair corrupted file");

                    // If we were able to get API response - it shouldn't be impossible
                    // to also get integrity files list from the same API
                    match super::repairer::try_get_integrity_file(relative_file, Some(*crate::REQUESTS_TIMEOUT)) {
                        Ok(Some(integrity)) => {
                            if !integrity.fast_verify(&path) {
                                if let Err(err) = integrity.repair(&path) {
                                    tracing::error!("Failed to repair corrupted file: {err}");

                                    return Err(err.into());
                                }
                            }
                        }

                        Ok(None) => {
                            tracing::error!("Failed to repair corrupted file: not found");

                            return Err(Self::Error::HdiffPatch(err.to_string()))
                        }

                        Err(repair_fail) => {
                            tracing::error!("Failed to repair corrupted file: {repair_fail}");

                            return Err(Self::Error::HdiffPatch(err.to_string()))
                        }
                    }

                    #[allow(unused_must_use)] {
                        std::fs::remove_file(&patch);
                    }
                }

                // If patch was successfully applied
                else {
                    // FIXME: handle errors properly
                    std::fs::remove_file(&file)
                        .expect(&format!("Failed to remove hdiff patch: {:?}", file));

                    std::fs::remove_file(&patch)
                        .expect(&format!("Failed to remove hdiff patch: {:?}", patch));

                    std::fs::rename(&output, &file)
                        .expect(&format!("Failed to rename hdiff patch: {:?}", file));
                }

                (updater)(Self::Update::ApplyingHdiffProgress(i as u64 + 1, hdiffs));
            }

            std::fs::remove_file(path.join("hdifffiles.txt"))
                .expect("Failed to remove hdifffiles.txt");

            (updater)(Self::Update::ApplyingHdiffFinished);
        }

        tracing::debug!("Deleting outdated files");

        // Remove outdated files
        // We're ignoring Err because in practice it means that deletefiles.txt is missing
        if let Ok(files) = std::fs::read_to_string(path.join("deletefiles.txt")) {
            let files = files.lines().collect::<Vec<&str>>();
            let files_len = files.len() as u64;

            (updater)(Self::Update::RemovingOutdatedStarted);

            // AnimeGame_Data/Plugins/metakeeper.dll
            for (i, file) in files.into_iter().enumerate() {
                let file = path.join(file);

                std::fs::remove_file(&file)
                    .expect(&format!("Failed to remove outdated file: {:?}", file));

                (updater)(Self::Update::RemovingOutdatedProgress(i as u64 + 1, files_len));
            }

            std::fs::remove_file(path.join("deletefiles.txt"))
                .expect("Failed to remove deletefiles.txt");

            (updater)(Self::Update::RemovingOutdatedFinished);
        }

        Ok(())
    }
}
