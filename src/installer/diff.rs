use std::fs::{read_to_string, remove_file};

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
    AlreadyLatest,
    Outdated,
    Curl(curl::Error)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionDiff {
    Latest(Version),
    Diff {
        current: Version,
        latest: Version,
        url: String,
        size: u64,
        unpacking_path: String
    },
    /// Difference can't be calculated because installed version is too old
    Outdated {
        current: Version,
        latest: Version
    },
    NotInstalled {
        latest: Version,
        url: String,
        size: u64,
        unpacking_path: String
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
            VersionDiff::Diff { current: _, latest: _, url: diff_url, size: _, unpacking_path: _ } => url = diff_url.clone(),
            VersionDiff::NotInstalled { latest: _, url: diff_url, size: _, unpacking_path: _ } => url = diff_url.clone()
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
    #[cfg(feature = "install")]
    fn install<F>(&self, updater: F) -> Result<(), DiffDownloadError>
    where F: Fn(InstallerUpdate) + Clone + Send + 'static
    {
        match self {
            // Can't be downloaded
            VersionDiff::Latest(_) => return Err(DiffDownloadError::AlreadyLatest),
            VersionDiff::Outdated { current: _, latest: _ } => return Err(DiffDownloadError::Outdated),

            // Can be downloaded
            VersionDiff::Diff { current: _, latest: _, url: _, size: _, unpacking_path } => self.install_to(unpacking_path, updater),
            VersionDiff::NotInstalled { latest: _, url: _, size: _, unpacking_path } => self.install_to(unpacking_path, updater)
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
            VersionDiff::Diff { current: _, latest: _, url: diff_url, size: _, unpacking_path: _ } => url = diff_url.clone(),
            VersionDiff::NotInstalled { latest: _, url: diff_url, size: _, unpacking_path: _ } => url = diff_url.clone()
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
}
