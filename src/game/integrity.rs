use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::sync::Arc;

use crate::game::DriverExt;
use crate::updater::UpdaterExt;

// Verify game files integrity:
// 
// <impl VerifyIntegrityExt>::verify_files() -> BasicUpdater -> Vec<<impl VerifyIntegrityResultExt>>
// <impl VerifyIntegrityResultExt>::repair() -> downloader::Updater
// 
// Verify specific file integrity:
// 

pub trait VerifyIntegrityExt {
    type Error;
    type Updater: UpdaterExt;

    /// Verify installed game files and return
    /// list of broken/outdated/absent files
    fn verify_files(&self) -> Result<Self::Updater, Self::Error>;
}

pub trait VerifyIntegrityResultExt {
    type Error;
    type Updater: UpdaterExt;

    /// Verify game file. Return `None` if file is correct
    fn verify(driver: Arc<dyn DriverExt>, file: &Path) -> Result<Option<Self>, Self::Error> where Self: Sized;

    /// Repair game file
    fn repair(self) -> Result<Self::Updater, Self::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(#[from] flume::SendError<()>),

    #[error("Failed to verify {file} integrity: {error}")]
    FileVerifyingError {
        file: PathBuf,
        error: String
    }
}

pub struct BasicUpdater {
    status_updater: Option<JoinHandle<Result<Vec<PathBuf>, Error>>>,
    status_updater_result: Option<Result<Vec<PathBuf>, Error>>,

    incrementer: flume::Receiver<()>,

    current: Cell<u64>,
    total: u64
}

impl BasicUpdater {
    pub fn new<Verifier>(driver: Arc<dyn DriverExt>, files: Vec<PathBuf>, verifier: Verifier) -> Self
    where
        Verifier: Fn(Arc<dyn DriverExt>, &PathBuf) -> Result<bool, String> + Send + 'static
    {
        let (send, recv) = flume::unbounded();

        Self {
            incrementer: recv,

            current: Cell::new(0),
            total: files.len() as u64,

            status_updater_result: None,

            status_updater: Some(std::thread::spawn(move || -> Result<Vec<PathBuf>, Error> {
                let mut broken = Vec::new();

                for file in files {
                    match verifier(driver.clone(), &file) {
                        Ok(true) => (),

                        Ok(false) => broken.push(file),

                        Err(error) => return Err(Error::FileVerifyingError {
                            file,
                            error
                        })
                    }

                    send.send(())?;
                }

                Ok(broken)
            }))
        }
    }
}

impl UpdaterExt for BasicUpdater {
    type Error = Error;
    type Status = bool;
    type Result = Vec<PathBuf>;

    #[inline]
    fn status(&mut self) -> Result<Self::Status, &Self::Error> {
        if let Some(status_updater) = self.status_updater.take() {
            if !status_updater.is_finished() {
                self.status_updater = Some(status_updater);

                return Ok(false);
            }

            self.status_updater_result = Some(status_updater.join().expect("Failed to join thread"));
        }

        match &self.status_updater_result {
            Some(Ok(_)) => Ok(true),
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
        matches!(self.status(), Ok(true) | Err(_))
    }

    #[inline]
    fn current(&self) -> u64 {
        let mut current = self.current.get();

        while let Ok(()) = self.incrementer.try_recv() {
            current += 1;
        }

        self.current.set(current);

        current
    }

    #[inline]
    fn total(&self) -> u64 {
        self.total
    }
}
