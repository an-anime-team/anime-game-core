use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Arc};

use serde::{Serialize, Deserialize};

use md5::{Md5, Digest};

use crate::game::GameExt;
use crate::game::integrity::*;

use crate::game::version::{
    // Version,
    Error as VersionError
};

use crate::updater::*;

use crate::network::api::ApiExt;
use crate::network::downloader::DownloaderExt;
use crate::network::downloader::basic::{
    Downloader,
    Error as DownloaderError
};

use super::Game;
use super::Api;

/// Number of threads used to verify game files
pub const VERIFY_THREADS_NUM: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to fetch data: {0}")]
    Minreq(#[from] minreq::Error),

    #[error("Failed to fetch data: {0}")]
    MinreqRef(#[from] &'static minreq::Error),

    #[error("Failed to send verifier message through the flume channel: {0}")]
    FlumeVerifierSendError(#[from] flume::SendError<((), u64, u64)>),

    #[error("Failed to send repairer message through the flume channel: {0}")]
    FlumeRepairerSendError(#[from] flume::SendError<(RepairerStatus, u64, u64)>),

    #[error("Failed to parse version: {0}")]
    VersionParseError(#[from] VersionError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to start downloader: {0}")]
    DownloaderError(#[from] DownloaderError)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairerStatus {
    PreparingTransition,
    RepairingFiles,
    FinishingTransition
}

pub type VerifyUpdater = BasicUpdater<(), Vec<PathBuf>, Error>;
pub type RepairUpdater = BasicUpdater<RepairerStatus, (), Error>;

impl VerifyIntegrityExt for Game {
    type Error = Error;
    type Updater = VerifyUpdater;

    fn verify_files(&self) -> Result<Self::Updater, Self::Error> {
        let api = match Api::fetch(self.edition) {
            Ok(api) => api.data.game.latest.clone(),

            Err(err) => return Err(Error::MinreqRef(err))
        };

        let decompressed_path = api.decompressed_path;
        // let version = api.version.parse::<Version>()?;

        // Should I use transitions for files verification?
        // let game_folder = self.driver.create_transition(&format!("action:verify_files-component:game_{}-version:v{version}", self.edition.to_str()))?;

        #[derive(Serialize, Deserialize)]
        #[allow(non_snake_case)]
        struct PkgVersionFile {
            pub remoteName: String,
            pub md5: String,
            pub fileSize: u64
        }

        let files = Arc::new(Mutex::new(minreq::get(format!("{decompressed_path}/pkg_version"))
            .send()?
            .as_str()?
            .lines()
            .flat_map(serde_json::from_str::<PkgVersionFile>)
            .collect::<Vec<_>>()));

        let driver = self.get_driver();

        Ok(BasicUpdater::spawn(|updater| {
            Box::new(move || {
                let mut workers = Vec::new();
                let mut broken = Vec::new();

                let current = Arc::new(AtomicU64::new(0));
                let total = files.lock().unwrap().len() as u64;

                for i in 1..=VERIFY_THREADS_NUM {
                    let driver = driver.clone();
                    let updater = updater.clone();

                    let current = current.clone();
                    let files = files.clone();

                    workers.push(std::thread::spawn(move || -> Result<Vec<PathBuf>, Error> {
                        let mut broken = Vec::new();

                        while let Ok(mut files) = files.try_lock() {
                            let file = files.pop();

                            // Drop mutex lock to allow other workers to access it
                            drop(files);

                            let Some(file) = file else {
                                break;
                            };

                            tracing::trace!("[verifier {i}] Processing {:?}", file.remoteName);

                            let path = PathBuf::from(file.remoteName);

                            let verified = driver.exists(path.as_os_str()) &&
                                driver.metadata(path.as_os_str())?.len() == file.fileSize &&
                                format!("{:x}", Md5::digest(driver.read(path.as_os_str())?)).to_ascii_lowercase() == file.md5;

                            if !verified {
                                broken.push(path);
                            }

                            updater.send(((), current.fetch_add(1, Ordering::Relaxed), total))?;
                        }

                        Ok(broken)
                    }));
                }

                for worker in workers {
                    broken.append(&mut worker.join().expect("Failed to join worker thread")?);
                }

                Ok(broken)
            })
        }))
    }
}

impl RepairFilesExt for Game {
    type Error = Error;
    type Updater = RepairUpdater;

    fn repair_files(&self, files: impl AsRef<[PathBuf]>) -> Result<Self::Updater, Self::Error> {
        let api = match Api::fetch(self.edition) {
            Ok(api) => api.data.game.latest.clone(),

            Err(err) => return Err(Error::MinreqRef(err))
        };

        // Ok(BasicRepairerUpdater::new(self.get_driver(), files.as_ref().to_vec(), api.decompressed_path))

        let driver = self.get_driver();
        let files = files.as_ref().to_vec();
        let base_download_uri = api.decompressed_path;

        Ok(BasicUpdater::spawn(|updater| {
            Box::new(move || -> Result<(), Error> {
                // I don't need to send this message but do it just for consistency
                updater.send((RepairerStatus::PreparingTransition, 0, 1))?;

                // TODO: list original files hashes or something to make repair transitions unique
                let transition_folder = driver.create_transition("action:repair")?;

                let total = files.len() as u64;

                updater.send((RepairerStatus::RepairingFiles, 0, total))?;

                for file in files {
                    // TODO: use updater to show downloading progress better
                    Downloader::new(format!("{base_download_uri}/{}", file.to_string_lossy()))
                        .download(transition_folder.join(&file))?
                        .wait()?;

                    updater.send((RepairerStatus::RepairingFiles, 0, total))?;
                }

                updater.send((RepairerStatus::FinishingTransition, 0, 1))?;

                driver.finish_transition("action:repair")?;

                Ok(())
            })
        }))
    }
}
