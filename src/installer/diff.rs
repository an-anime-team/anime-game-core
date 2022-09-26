use std::fs::{read_to_string, remove_file};
use std::path::PathBuf;

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
    external::hpatchz
};

#[derive(Debug, Clone)]
pub enum DiffDownloadError {
    /// Your installation is already up to date and not needed to be updated
    AlreadyLatest,

    /// Current version is too outdated and can't be updated.
    /// It means that you have to download everything from zero
    Outdated,

    /// Failed to fetch remove data. Redirected from `Downloader`
    DownloadingError(DownloadingError),

    // Failed to apply hdiff patch
    HdiffPatch(String),

    /// Installation path wasn't specified. This could happen when you
    /// try to call `install` method on `VersionDiff` that was generated
    /// in `VoicePackage::list_latest`. This method couldn't know
    /// your game installation path and thus indicates that it doesn't know
    /// where this package needs to be installed
    PathNotSpecified
}

impl From<DownloadingError> for DiffDownloadError {
    fn from(err: DownloadingError) -> Self {
        Self::DownloadingError(err)
    }
}

impl From<curl::Error> for DiffDownloadError {
    fn from(err: curl::Error) -> Self {
        Self::DownloadingError(err.into())
    }
}

impl Into<std::io::Error> for DiffDownloadError {
    fn into(self) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, match self {
            Self::DownloadingError(err) => return err.into(),

            Self::AlreadyLatest => "Component version is already latest".to_string(),
            Self::Outdated => "Components version is too outdated and can't be updated".to_string(),
            Self::HdiffPatch(err) => format!("Failed to apply hdiff patch: {err}"),
            Self::PathNotSpecified => "Path to the component's downloading folder is not specified".to_string()
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
        unpacking_path: Option<PathBuf>
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
        unpacking_path: Option<PathBuf>
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
        unpacking_path: Option<PathBuf>
    }
}

impl VersionDiff {
    /// Get currently installed game version
    /// 
    /// Returns `None` on `VersionDiff::NotInstalled`
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

    /// Try to download archive with the difference by specified path
    #[cfg(feature = "install")]
    pub fn download_to<T, Fp>(&mut self, path: T, progress: Fp) -> Result<(), DiffDownloadError>
    where
        T: Into<PathBuf>,
        // (curr, total)
        Fp: Fn(u64, u64) + Send + 'static
    {
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

        match downloader.download_to(path, progress) {
            Ok(_) => Ok(()),
            Err(err) => Err(err.into())
        }
    }

    /// Try to install the difference
    /// 
    /// This method can return `Err(DiffDownloadError::PathNotSpecified)` when `unpacking_path` is not specified.
    /// It's recommended to use `unpacking_path` before this method to be sure that current enum knows
    /// where the difference should be installed
    #[cfg(feature = "install")]
    pub fn install<F>(&self, updater: F) -> Result<(), DiffDownloadError>
    where F: Fn(InstallerUpdate) + Clone + Send + 'static
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
        T: Into<PathBuf>,
        F: Fn(InstallerUpdate) + Clone + Send + 'static
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
    pub fn install_to_by<T, F>(&self, path: T, temp_path: Option<T>, updater: F) -> Result<(), DiffDownloadError>
    where
        T: Into<PathBuf>,
        F: Fn(InstallerUpdate) + Clone + Send + 'static
    {
        let url;
        let download_size;
        let unpacked_size;

        match self {
            // Can't be downloaded
            Self::Latest(_) => return Err(DiffDownloadError::AlreadyLatest),
            Self::Outdated { .. } => return Err(DiffDownloadError::Outdated),

            // Can be downloaded
            Self::Predownload { url: diff_url, download_size: down_size, unpacked_size: unp_size, .. } |
            Self::Diff { url: diff_url, download_size: down_size, unpacked_size: unp_size, .. } |
            Self::NotInstalled { url: diff_url, download_size: down_size, unpacked_size: unp_size, .. } => {
                url = diff_url.clone();
                download_size = *down_size;
                unpacked_size = *unp_size;
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
                installer.install(&path, updater);

                // Apply hdiff patches
                // We're ignoring Err because in practice it means that hdifffiles.txt is missing
                if let Ok(files) = read_to_string(path.join("hdifffiles.txt")) {
                    // {"remoteName": "AnimeGame_Data/StreamingAssets/Audio/GeneratedSoundBanks/Windows/Japanese/1001.pck"}
                    for file in files.lines().collect::<Vec<&str>>() {
                        let relative_file = &file[16..file.len() - 2];

                        let file = path.join(relative_file);
                        let patch = path.join(format!("{relative_file}.hdiff"));
                        let output = path.join(format!("{relative_file}.hdiff_patched"));

                        if let Err(err) = hpatchz::patch(&file, &patch, &output) {
                            return Err(DiffDownloadError::HdiffPatch(err.to_string()));
                        }

                        // FIXME: handle errors properly
                        remove_file(&file).expect(&format!("Failed to remove hdiff patch: {:?}", file));
                        remove_file(&patch).expect(&format!("Failed to remove hdiff patch: {:?}", patch));

                        std::fs::rename(&output, &file).expect(&format!("Failed to rename hdiff patch: {:?}", file));
                    }

                    remove_file(path.join("hdifffiles.txt"))
                        .expect("Failed to remove hdifffiles.txt");
                }

                // Remove outdated files
                // We're ignoring Err because in practice it means that deletefiles.txt is missing
                if let Ok(files) = read_to_string(path.join("deletefiles.txt")) {
                    // AnimeGame_Data/Plugins/metakeeper.dll
                    for file in files.lines().collect::<Vec<&str>>() {
                        let file = path.join(file);

                        remove_file(&file).expect(&format!("Failed to remove outdated file: {:?}", file));
                    }

                    remove_file(path.join("deletefiles.txt"))
                        .expect("Failed to remove deletefiles.txt");
                }
                
                Ok(())
            },
            Err(err) => Err(err.into())
        }
    }
}

pub trait TryGetDiff {
    /// Try to get difference between currently installed version and the latest available
    fn try_get_diff(&self) -> anyhow::Result<VersionDiff>;
}
