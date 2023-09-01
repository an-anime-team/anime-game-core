use std::cell::Cell;
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::sync::Arc;

use crate::game::DriverExt;
use crate::updater::UpdaterExt;

use crate::network::downloader::DownloaderExt;

use crate::network::downloader::basic::{
    Downloader,
    Error as DownloaderError
};

pub trait VerifyIntegrityExt {
    type Error;
    type Updater: UpdaterExt;

    /// Verify installed game files and return
    /// list of broken/outdated/absent files
    fn verify_files(&self) -> Result<Self::Updater, Self::Error>;
}

pub trait RepairFilesExt {
    type Error;
    type Updater: UpdaterExt;

    /// Repair game files
    fn repair_files(&self, files: impl AsRef<[PathBuf]>) -> Result<Self::Updater, Self::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to send message through the flume channel: {0}")]
    FlumeVerifierSendError(#[from] flume::SendError<()>),

    #[error("Failed to send message through the flume channel: {0}")]
    FlumeRepairerSendError(#[from] flume::SendError<BasicRepairerUpdaterStatus>),

    #[error("Failed to verify {file} integrity: {error}")]
    FileVerifyingError {
        file: PathBuf,
        error: String
    },

    #[error("Failed to download file: {0}")]
    DownloaderError(#[from] DownloaderError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BasicRepairerUpdaterStatus {
    PreparingTransition,
    RepairingFiles,
    FinishingTransition,
    Finished
}

pub struct BasicRepairerUpdater {
    status_updater: Option<JoinHandle<Result<(), Error>>>,
    status_updater_result: Option<Result<(), Error>>,

    updater: flume::Receiver<BasicRepairerUpdaterStatus>,
    status: Cell<Option<BasicRepairerUpdaterStatus>>,

    current: Cell<u64>,
    total: u64
}

impl BasicRepairerUpdater {
    pub fn new(driver: Arc<dyn DriverExt>, files: Vec<PathBuf>, base_download_uri: String) -> Self {
        let (send, recv) = flume::unbounded();

        Self {
            updater: recv,
            status: Cell::new(None),

            current: Cell::new(0),
            total: files.len() as u64,

            status_updater_result: None,

            status_updater: Some(std::thread::spawn(move || -> Result<(), Error> {
                // I don't need to send this message but do it just for consistency
                send.send(BasicRepairerUpdaterStatus::PreparingTransition)?;

                // TODO: list original files hashes or something to make repair transitions unique
                let transition_folder = driver.create_transition("action:repair")?;

                send.send(BasicRepairerUpdaterStatus::RepairingFiles)?;

                for file in files {
                    let updater = Downloader::new(format!("{base_download_uri}/{}", file.to_string_lossy()))
                        .download(transition_folder.join(&file))?;

                    // TODO: use updater to show downloading progress better

                    updater.wait()?;
                }

                send.send(BasicRepairerUpdaterStatus::FinishingTransition)?;

                driver.finish_transition("action:repair")?;

                // I don't need to send this message but do it just for consistency
                send.send(BasicRepairerUpdaterStatus::Finished)?;

                Ok(())
            }))
        }
    }

    fn update(&self) {
        let mut current = self.current.get();

        while let Ok(status) = self.updater.try_recv() {
            current += 1;

            self.status.set(Some(status));
            self.current.set(current);
        }
    }
}

impl UpdaterExt for BasicRepairerUpdater {
    type Error = Error;
    type Status = BasicRepairerUpdaterStatus;
    type Result = ();

    #[inline]
    fn status(&mut self) -> Result<Self::Status, &Self::Error> {
        self.update();

        if let Some(status_updater) = self.status_updater.take() {
            if !status_updater.is_finished() {
                self.status_updater = Some(status_updater);

                // TODO: don't like to call clone here all the time
                return Ok(match self.status.take() {
                    Some(status) => {
                        self.status.set(Some(status.clone()));

                        status
                    }

                    None => BasicRepairerUpdaterStatus::PreparingTransition
                });
            }

            self.status_updater_result = Some(status_updater.join().expect("Failed to join thread"));
        }

        match &self.status_updater_result {
            Some(Ok(_)) => Ok(BasicRepairerUpdaterStatus::Finished),
            Some(Err(err)) => Err(err),

            None => unreachable!()
        }
    }

    #[inline]
    fn wait(mut self) -> Result<Self::Result, Self::Error> {
        if let Some(worker) = self.status_updater.take() {
            return worker.join().expect("Failed to join thread");
        }

        else if let Some(result) = self.status_updater_result.take() {
            return result;
        }

        unreachable!()
    }

    #[inline]
    fn is_finished(&mut self) -> bool {
        matches!(self.status(), Ok(BasicRepairerUpdaterStatus::Finished) | Err(_))
    }

    #[inline]
    fn current(&self) -> u64 {
        self.update();

        self.current.get()
    }

    #[inline]
    fn total(&self) -> u64 {
        self.total
    }
}
