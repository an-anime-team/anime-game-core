use std::fs::{read_to_string, remove_file};
use std::io::Error;

use crate::version::Version;

#[cfg(feature = "install")]
use crate::installer::{
    downloader::Downloader,
    installer::{
        Installer,
        Update as InstallerUpdate
    }
};

#[derive(Debug, Clone)]
pub enum DiffDownloadError {
    /// Your installation is already up to date and not needed to be updated
    AlreadyLatest,

    /// Current version is too outdated and can't be updated.
    /// It means that you have to download everything from zero
    Outdated,

    /// Failed to fetch remove data. Redirected from `Downloader`
    Curl(curl::Error),

    /// Installation path wasn't specified. This could happen when you
    /// try to call `install` method on `VersionDiff` that was generated
    /// in `VoicePackage::list_latest`. This method couldn't know
    /// your game installation path and thus indicates that it doesn't know
    /// where this package needs to be installed
    PathNotSpecified
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionDiff {
    Latest(Version),
    Diff {
        current: Version,
        latest: Version,
        url: String,
        download_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        /// 
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        unpacking_path: Option<String>
    },
    /// Difference can't be calculated because installed version is too old
    Outdated {
        current: Version,
        latest: Version
    },
    NotInstalled {
        latest: Version,
        url: String,
        download_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        /// 
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        unpacking_path: Option<String>
    }
}

impl VersionDiff {
    /// Try to download archive with the difference by specified path
    #[cfg(feature = "install")]
    fn download_to<T, Fp>(&mut self, path: T, progress: Fp) -> Result<(), DiffDownloadError>
    where
        T: ToString,
        // (curr, total)
        Fp: Fn(u64, u64) + Send + 'static
    {
        let url;

        match self {
            // Can't be downloaded
            VersionDiff::Latest(_) => return Err(DiffDownloadError::AlreadyLatest),
            VersionDiff::Outdated { current: _, latest: _ } => return Err(DiffDownloadError::Outdated),

            // Can be downloaded
            VersionDiff::Diff { current: _, latest: _, url: diff_url, download_size: _, unpacked_size: _, unpacking_path: _ } => url = diff_url.clone(),
            VersionDiff::NotInstalled { latest: _, url: diff_url, download_size: _, unpacked_size: _, unpacking_path: _ } => url = diff_url.clone()
        }

        match Downloader::new(url) {
            Ok(mut downloader) => {
                match downloader.download_to(path, progress) {
                    Ok(_) => Ok(()),
                    Err(err) => Err(DiffDownloadError::Curl(err))
                }
            },
            Err(err) => Err(DiffDownloadError::Curl(err))
        }
    }

    /// Try to install the difference
    /// 
    /// This method can return `Err(DiffDownloadError::PathNotSpecified)` when `unpacking_path` is not specified.
    /// It's recommended to use `unpacking_path` before this method to be sure that current enum knows
    /// where the difference should be installed
    #[cfg(feature = "install")]
    fn install<F>(&self, updater: F) -> Result<(), DiffDownloadError>
    where F: Fn(InstallerUpdate) + Clone + Send + 'static
    {
        match self {
            // Can't be downloaded
            VersionDiff::Latest(_) => Err(DiffDownloadError::AlreadyLatest),
            VersionDiff::Outdated { current: _, latest: _ } => Err(DiffDownloadError::Outdated),

            // Can be downloaded
            VersionDiff::Diff { current: _, latest: _, url: _, download_size: _, unpacked_size: _, unpacking_path } => {
                match unpacking_path {
                    Some(unpacking_path) => self.install_to(unpacking_path, updater),
                    None => Err(DiffDownloadError::PathNotSpecified)
                }
            },
            VersionDiff::NotInstalled { latest: _, url: _, download_size: _, unpacked_size: _, unpacking_path } => {
                match unpacking_path {
                    Some(unpacking_path) => self.install_to(unpacking_path, updater),
                    None => Err(DiffDownloadError::PathNotSpecified)
                }
            }
        }
    }

    /// Try to install the difference by specified location
    #[cfg(feature = "install")]
    fn install_to<T, F>(&self, path: T, updater: F) -> Result<(), DiffDownloadError>
    where
        T: ToString,
        F: Fn(InstallerUpdate) + Clone + Send + 'static
    {
        let url;

        match self {
            // Can't be downloaded
            VersionDiff::Latest(_) => return Err(DiffDownloadError::AlreadyLatest),
            VersionDiff::Outdated { current: _, latest: _ } => return Err(DiffDownloadError::Outdated),

            // Can be downloaded
            VersionDiff::Diff { current: _, latest: _, url: diff_url, download_size: _, unpacked_size: _, unpacking_path: _ } => url = diff_url.clone(),
            VersionDiff::NotInstalled { latest: _, url: diff_url, download_size: _, unpacked_size: _, unpacking_path: _ } => url = diff_url.clone()
        }

        match Installer::new(url) {
            Ok(mut installer) => {
                installer.install(path.to_string(), updater);

                // TODO: https://gitlab.com/an-anime-team/an-anime-game-launcher/-/blob/main/src/ts/launcher/states/ApplyChanges.ts
                // TODO: update states for patches applying and removing of outdated files

                // Remove outdated files
                match read_to_string(format!("{}/deletefiles.txt", path.to_string())) {
                    Ok(files) => {
                        for file in files.split("\n").collect::<Vec<&str>>() {
                            let file: &str = file.trim_end();

                            // TODO: add errors handling
                            remove_file(file).expect("Failed to remove outdated file");
                        }

                        remove_file(format!("{}/deletefiles.txt", path.to_string())).expect("Failed to remove deletefiles.txt");

                        Ok(())
                    },
                    Err(_) => todo!() // FIXME
                }
            },
            Err(err) => Err(DiffDownloadError::Curl(err))
        }
    }

    /// Returns (download_size, unpacked_size) pair if it exists in current enum value
    pub fn get_size(&self) -> Option<(u64, u64)> {
        match self {
            // Can't be downloaded
            VersionDiff::Latest(_) => None,
            VersionDiff::Outdated { current: _, latest: _ } => None,

            // Can be downloaded
            VersionDiff::Diff { current: _, latest: _, url: diff_url, download_size, unpacked_size, unpacking_path: _ } => Some((*download_size, *unpacked_size)),
            VersionDiff::NotInstalled { latest: _, url: diff_url, download_size, unpacked_size, unpacking_path: _ } => Some((*download_size, *unpacked_size))
        }
    }

    /// Returns the path this difference should be installed to if it exists in current enum value
    pub fn unpacking_path(&self) -> Option<String> {
        match self {
            // Can't be downloaded
            VersionDiff::Latest(_) => None,
            VersionDiff::Outdated { current: _, latest: _ } => None,

            // Can be downloaded
            VersionDiff::Diff { current: _, latest: _, url: diff_url, download_size: _, unpacked_size: _, unpacking_path } => unpacking_path.clone(),
            VersionDiff::NotInstalled { latest: _, url: diff_url, download_size: _, unpacked_size: _, unpacking_path } => unpacking_path.clone()
        }
    }
}

// TODO: probably use "type Error" instead of io::Error
pub trait TryGetDiff {
    /// Try to get difference between currently installed version and the latest available
    fn try_get_diff(&self) -> Result<VersionDiff, Error>;
}
