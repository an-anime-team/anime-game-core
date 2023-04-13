use std::fs::{read_to_string, remove_file};
use std::path::PathBuf;

use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::version::Version;

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
    repairer::IntegrityFile,
    external::hpatchz
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    InstallerUpdate(InstallerUpdate),

    ApplyingHdiffStarted,
    ApplyingHdiffProgress(u64, u64),
    ApplyingHdiffFinished,

    RemovingOutdatedStarted,
    RemovingOutdatedProgress(u64, u64),
    RemovingOutdatedFinished
}

impl From<InstallerUpdate> for Update {
    #[inline]
    fn from(update: InstallerUpdate) -> Self {
        Self::InstallerUpdate(update)
    }
}

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffDownloadError {
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

impl From<minreq::Error> for DiffDownloadError {
    fn from(error: minreq::Error) -> Self {
        DiffDownloadError::DownloadingError(DownloadingError::Minreq(error.to_string()))
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
        download_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        /// 
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        unpacking_path: Option<PathBuf>,

        // Optional path to the .version file
        version_file_path: Option<PathBuf>
    },

    /// Component should be updated before using it
    Diff {
        current: Version,
        latest: Version,
        url: String,
        download_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        /// 
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        unpacking_path: Option<PathBuf>,

        // Optional path to the .version file
        version_file_path: Option<PathBuf>
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
        download_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        /// 
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        unpacking_path: Option<PathBuf>,

        // Optional path to the .version file
        version_file_path: Option<PathBuf>
    }
}

impl VersionDiff {
    /// Get currently installed game version
    /// 
    /// Returns `None` on `VersionDiff::NotInstalled`
    #[inline]
    pub fn current(&self) -> Option<Version> {
        match self {
            Self::Latest(current) |
            Self::Predownload { current, .. } |
            Self::Diff { current, .. } |
            Self::Outdated { current, .. } => Some(*current),

            Self::NotInstalled { .. } => None
        }
    }

    /// Get latest available game version
    #[inline]
    pub fn latest(&self) -> Version {
        match self {
            Self::Latest(latest) |
            Self::Predownload { latest, .. } |
            Self::Diff { latest, .. } |
            Self::Outdated { latest, .. } |
            Self::NotInstalled { latest, .. } => *latest
        }
    }

    /// Returns (download_size, unpacked_size) pair if it exists in current enum value
    #[inline]
    pub fn size(&self) -> Option<(u64, u64)> {
        match self {
            // Can't be downloaded
            Self::Latest(_) |
            Self::Outdated { .. } => None,

            // Can be downloaded
            Self::Predownload { download_size, unpacked_size, .. } |
            Self::Diff { download_size, unpacked_size, .. } |
            Self::NotInstalled { download_size, unpacked_size, .. } => Some((*download_size, *unpacked_size))
        }
    }

    /// Returns the path this difference should be installed to if it exists in current enum value
    #[inline]
    pub fn unpacking_path(&self) -> Option<PathBuf> {
        match self {
            // Can't be downloaded
            Self::Latest(_) |
            Self::Outdated { .. } => None,

            // Can be downloaded
            Self::Predownload { unpacking_path, .. } |
            Self::Diff { unpacking_path, .. } |
            Self::NotInstalled { unpacking_path, .. } => unpacking_path.clone()
        }
    }

    /// Get filename from downloading URI
    /// 
    /// Returns `None` on `VersionDiff::Latest` and `VersionDiff::Outdated` (so diffs that can't be downloaded)
    pub fn file_name(&self) -> Option<String> {
        match self {
            Self::Latest(_) | Self::Outdated { .. } => None,

            Self::Predownload { url: diff_url, .. } |
            Self::Diff { url: diff_url, .. } |
            Self::NotInstalled { url: diff_url, .. } => Some(String::from({
                match diff_url.rfind('/') {
                    Some(index) => {
                        let file = &diff_url[index + 1..];

                        if file == "" { "index.html" } else { file }
                    },
                    None => "index.html"
                }
            }))
        }
    }

    /// Try to download archive with the difference into the specified folder
    #[cfg(feature = "install")]
    #[tracing::instrument(level = "debug", skip(progress))]
    pub fn download_in<T, Fp>(&mut self, folder: T, progress: Fp) -> Result<(), DiffDownloadError>
    where
        T: Into<PathBuf> + std::fmt::Debug,
        // (curr, total)
        Fp: Fn(u64, u64) + Send + 'static
    {
        tracing::debug!("Downloading version difference");

        let url;

        match self {
            // Can't be downloaded
            Self::Latest(_) => return Err(DiffDownloadError::AlreadyLatest),
            Self::Outdated { .. } => return Err(DiffDownloadError::Outdated),

            // Can be downloaded
            Self::Predownload { url: diff_url, .. } |
            Self::Diff { url: diff_url, .. } |
            Self::NotInstalled { url: diff_url, .. } => url = diff_url.clone()
        }

        let mut downloader = Downloader::new(url)?;

        match downloader.download(folder.into().join(downloader.get_filename()), progress) {
            Ok(_) => Ok(()),
            Err(err) => {
                tracing::error!("Failed to download version difference: {err}");

                Err(err.into())
            }
        }
    }

    /// Try to download archive with the difference by specified path, including filename
    #[cfg(feature = "install")]
    #[tracing::instrument(level = "debug", skip(progress))]
    pub fn download_to<T, Fp>(&mut self, path: T, progress: Fp) -> Result<(), DiffDownloadError>
    where
        T: Into<PathBuf> + std::fmt::Debug,
        // (curr, total)
        Fp: Fn(u64, u64) + Send + 'static
    {
        tracing::debug!("Downloading version difference");

        let url;

        match self {
            // Can't be downloaded
            Self::Latest(_) => return Err(DiffDownloadError::AlreadyLatest),
            Self::Outdated { .. } => return Err(DiffDownloadError::Outdated),

            // Can be downloaded
            Self::Predownload { url: diff_url, .. } |
            Self::Diff { url: diff_url, .. } |
            Self::NotInstalled { url: diff_url, .. } => url = diff_url.clone()
        }

        let mut downloader = Downloader::new(url)?;

        if let Err(err) = downloader.download(path, progress) {
            tracing::error!("Failed to download version difference: {err}");

            return Err(err.into());
        }

        Ok(())
    }

    /// Try to install the difference
    /// 
    /// This method can return `Err(DiffDownloadError::PathNotSpecified)` when `unpacking_path` is not specified.
    /// It's recommended to use `unpacking_path` before this method to be sure that current enum knows
    /// where the difference should be installed
    #[cfg(feature = "install")]
    pub fn install<F>(&self, updater: F) -> Result<(), DiffDownloadError>
    where F: Fn(Update) + Clone + Send + 'static
    {
        match self {
            // Can't be downloaded
            Self::Latest(_) => Err(DiffDownloadError::AlreadyLatest),
            Self::Outdated { .. } => Err(DiffDownloadError::Outdated),

            // Can be downloaded
            Self::Predownload { unpacking_path, .. } |
            Self::Diff { unpacking_path, .. } |
            Self::NotInstalled { unpacking_path, .. } => {
                match unpacking_path {
                    Some(unpacking_path) => self.install_to_by(unpacking_path, None, updater),
                    None => Err(DiffDownloadError::PathNotSpecified)
                }
            }
        }
    }

    /// Try to install the difference by specified location
    #[cfg(feature = "install")]
    pub fn install_to<T, F>(&self, path: T, updater: F) -> Result<(), DiffDownloadError>
    where
        T: Into<PathBuf> + std::fmt::Debug,
        F: Fn(Update) + Clone + Send + 'static
    {
        match self {
            // Can't be downloaded
            Self::Latest(_) => Err(DiffDownloadError::AlreadyLatest),
            Self::Outdated { .. } => Err(DiffDownloadError::Outdated),

            // Can be downloaded
            Self::Predownload { .. } |
            Self::Diff { .. } |
            Self::NotInstalled { .. } => self.install_to_by(path, None, updater)
        }
    }

    /// Try to install the difference by specified location and temp folder
    /// 
    /// Same as `install_to` method if `temp_path` specified as `None` (uses default system temp folder)
    #[cfg(feature = "install")]
    #[tracing::instrument(level = "debug", skip(updater))]
    pub fn install_to_by<T, F>(&self, path: T, temp_path: Option<T>, updater: F) -> Result<(), DiffDownloadError>
    where
        T: Into<PathBuf> + std::fmt::Debug,
        F: Fn(Update) + Clone + Send + 'static
    {
        tracing::debug!("Installing version difference");

        let url;
        let download_size;
        let unpacked_size;
        let new_version;
        let version_path;

        match self {
            // Can't be downloaded
            Self::Latest(_) => return Err(DiffDownloadError::AlreadyLatest),
            Self::Outdated { .. } => return Err(DiffDownloadError::Outdated),

            // Can be downloaded
            Self::Predownload { url: diff_url, download_size: down_size, unpacked_size: unp_size, latest, version_file_path, .. } |
            Self::Diff { url: diff_url, download_size: down_size, unpacked_size: unp_size, latest, version_file_path, .. } |
            Self::NotInstalled { url: diff_url, download_size: down_size, unpacked_size: unp_size, latest, version_file_path, .. } => {
                url = diff_url.clone();
                download_size = *down_size;
                unpacked_size = *unp_size;
                new_version = *latest;
                version_path = version_file_path.clone();
            }
        }

        match Installer::new(url) {
            Ok(mut installer) => {
                // Set temp folder if specified
                if let Some(temp_path) = temp_path {
                    installer.set_temp_folder(temp_path);
                }

                let path = path.into();

                // Check available free space for archive itself
                match free_space::available(&installer.temp_folder) {
                    Some(space) => {
                        // We can possibly store downloaded archive + unpacked data on the same disk
                        let required = if free_space::is_same_disk(&installer.temp_folder, &path) {
                            download_size + unpacked_size
                        } else {
                            download_size
                        };

                        if space < required {
                            return Err(DownloadingError::NoSpaceAvailable(installer.temp_folder, required, space).into());
                        }
                    },
                    None => return Err(DownloadingError::PathNotMounted(installer.temp_folder).into())
                }

                // Check available free space for unpacked archvie data
                match free_space::available(&path) {
                    Some(space) => {
                        // We can possibly store downloaded archive + unpacked data on the same disk
                        let required = if free_space::is_same_disk(&path, &installer.temp_folder) {
                            unpacked_size + download_size
                        } else {
                            unpacked_size
                        };

                        if space < required {
                            return Err(DownloadingError::NoSpaceAvailable(path, required, space).into());
                        }
                    },
                    None => return Err(DownloadingError::PathNotMounted(path).into())
                }

                // Install data
                let installer_updater = updater.clone();

                installer.install(&path, move |update| (installer_updater)(Update::InstallerUpdate(update)));

                // Create .version file here even if hdiff patching is failed because
                // it's easier to explain user why he should run files repairer than
                // why he should re-download entire game update because something is failed
                #[allow(unused_must_use)]
                {
                    let version_path = version_path.unwrap_or(path.join(".version"));

                    std::fs::write(version_path, new_version.version);
                }

                tracing::debug!("Applying hdiff patches");

                // Apply hdiff patches
                // We're ignoring Err because in practice it means that hdifffiles.txt is missing
                if let Ok(files) = read_to_string(path.join("hdifffiles.txt")) {
                    let files = files.lines().collect::<Vec<&str>>();
                    let hdiffs = files.len() as u64;

                    (updater)(Update::ApplyingHdiffStarted);

                    // {"remoteName": "AnimeGame_Data/StreamingAssets/Audio/GeneratedSoundBanks/Windows/Japanese/1001.pck"}
                    for (i, file) in files.into_iter().enumerate() {
                        let relative_file = &file[16..file.len() - 2];

                        let file = path.join(relative_file);
                        let patch = path.join(format!("{relative_file}.hdiff"));
                        let output = path.join(format!("{relative_file}.hdiff_patched"));

                        // If failed to apply the patch
                        #[allow(unused_mut, unused_assignments)]
                        if let Err(err) = hpatchz::patch(&file, &patch, &output) {
                            tracing::warn!("Failed to apply hdiff patch for {:?}: {err}", file);

                            let mut integrity_files: Option<anyhow::Result<Option<IntegrityFile>>> = None;

                            #[cfg(feature = "genshin")] {
                                integrity_files = Some(crate::genshin::repairer::try_get_integrity_file(relative_file, None));
                            }

                            #[cfg(feature = "honkai")] {
                                integrity_files = Some(crate::honkai::repairer::try_get_integrity_file(relative_file, None));
                            }

                            if let Some(integrity_files) = integrity_files {
                                tracing::debug!("Trying to repair corrupted file");

                                // If we were able to get API response - it shouldn't be impossible
                                // to also get integrity files list from the same API
                                match integrity_files {
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

                                        return Err(DiffDownloadError::HdiffPatch(err.to_string()))
                                    }

                                    Err(repair_fail) => {
                                        tracing::error!("Failed to repair corrupted file: {repair_fail}");

                                        return Err(DiffDownloadError::HdiffPatch(err.to_string()))
                                    }
                                }
                            }

                            #[allow(unused_must_use)] {
                                remove_file(&patch);
                            }
                        }

                        // If patch was successfully applied
                        else {
                            // FIXME: handle errors properly
                            remove_file(&file).expect(&format!("Failed to remove hdiff patch: {:?}", file));
                            remove_file(&patch).expect(&format!("Failed to remove hdiff patch: {:?}", patch));

                            std::fs::rename(&output, &file).expect(&format!("Failed to rename hdiff patch: {:?}", file));
                        }

                        (updater)(Update::ApplyingHdiffProgress(i as u64 + 1, hdiffs));
                    }

                    remove_file(path.join("hdifffiles.txt"))
                        .expect("Failed to remove hdifffiles.txt");

                    (updater)(Update::ApplyingHdiffFinished);
                }

                tracing::debug!("Deleting outdated files");

                // Remove outdated files
                // We're ignoring Err because in practice it means that deletefiles.txt is missing
                if let Ok(files) = read_to_string(path.join("deletefiles.txt")) {
                    let files = files.lines().collect::<Vec<&str>>();
                    let files_len = files.len() as u64;

                    (updater)(Update::RemovingOutdatedStarted);

                    // AnimeGame_Data/Plugins/metakeeper.dll
                    for (i, file) in files.into_iter().enumerate() {
                        let file = path.join(file);

                        remove_file(&file).expect(&format!("Failed to remove outdated file: {:?}", file));

                        (updater)(Update::RemovingOutdatedProgress(i as u64 + 1, files_len));
                    }

                    remove_file(path.join("deletefiles.txt"))
                        .expect("Failed to remove deletefiles.txt");

                    (updater)(Update::RemovingOutdatedFinished);
                }

                Ok(())
            }

            Err(err) => Err(err.into())
        }
    }
}

pub trait TryGetDiff {
    /// Try to get difference between currently installed version and the latest available
    fn try_get_diff(&self) -> anyhow::Result<VersionDiff>;
}
