use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::sync::Arc;

use crate::game::DriverExt;
use crate::updater::UpdaterExt;

use crate::network::downloader::DownloaderExt;

use crate::network::downloader::basic::{
    Downloader,
    Error as DownloaderError
};

// Verify game files integrity:
// 
// <impl VerifyIntegrityExt>::verify_files() -> BasicVerifierUpdater -> Vec<<impl VerifyIntegrityResultExt>>
// <impl VerifyIntegrityResultExt>::repair() -> Downloader::Updater
// 
// Verify specific file integrity:
// 
// <impl VerifyIntegrityResultExt>::verify() -> impl VerifyIntegrityResultExt

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

    /// Repair game file (or files)
    fn repair(self) -> Result<Self::Updater, Self::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(#[from] flume::SendError<PathBuf>),

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
pub enum Status {
    Starting,
    Working(PathBuf),
    Finished
}

pub struct BasicVerifierUpdater {
    status_updater: Option<JoinHandle<Result<Vec<PathBuf>, Error>>>,
    status_updater_result: Option<Result<Vec<PathBuf>, Error>>,

    updater: flume::Receiver<PathBuf>,
    current_file: Cell<Option<PathBuf>>,

    current: Cell<u64>,
    total: u64
}

impl BasicVerifierUpdater {
    pub fn new<Verifier>(driver: Arc<dyn DriverExt>, files: Vec<PathBuf>, verifier: Verifier) -> Self
    where
        Verifier: Fn(Arc<dyn DriverExt>, &PathBuf) -> Result<bool, String> + Send + 'static
    {
        let (send, recv) = flume::unbounded();

        Self {
            updater: recv,
            current_file: Cell::new(None),

            current: Cell::new(0),
            total: files.len() as u64,

            status_updater_result: None,

            status_updater: Some(std::thread::spawn(move || -> Result<Vec<PathBuf>, Error> {
                let mut broken = Vec::new();

                for file in files {
                    // TODO: don't like to call clone here all the time
                    send.send(file.clone())?;

                    match verifier(driver.clone(), &file) {
                        Ok(true) => (),

                        Ok(false) => broken.push(file),

                        Err(error) => return Err(Error::FileVerifyingError {
                            file,
                            error
                        })
                    }
                }

                Ok(broken)
            }))
        }
    }

    fn update(&self) {
        let mut current = self.current.get();

        while let Ok(file) = self.updater.try_recv() {
            current += 1;

            self.current_file.set(Some(file));
            self.current.set(current);
        }
    }
}

impl UpdaterExt for BasicVerifierUpdater {
    type Error = Error;
    type Status = Status;
    type Result = Vec<PathBuf>;

    #[inline]
    fn status(&mut self) -> Result<Self::Status, &Self::Error> {
        self.update();

        if let Some(status_updater) = self.status_updater.take() {
            if !status_updater.is_finished() {
                self.status_updater = Some(status_updater);

                // TODO: don't like to call clone here all the time
                return Ok(match self.current_file.take() {
                    Some(file) => {
                        self.current_file.set(Some(file.clone()));

                        Status::Working(file)
                    }

                    None => Status::Starting
                });
            }

            self.status_updater_result = Some(status_updater.join().expect("Failed to join thread"));
        }

        match &self.status_updater_result {
            Some(Ok(_)) => Ok(Status::Finished),
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
        matches!(self.status(), Ok(Status::Finished) | Err(_))
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

pub struct BasicRepairerUpdater {
    status_updater: Option<JoinHandle<Result<(), Error>>>,
    status_updater_result: Option<Result<(), Error>>,

    updater: flume::Receiver<PathBuf>,
    current_file: Cell<Option<PathBuf>>,

    current: Cell<u64>,
    total: u64
}

impl BasicRepairerUpdater {
    pub fn new(driver: Arc<dyn DriverExt>, files: Vec<PathBuf>, base_download_uri: String) -> Self {
        let (send, recv) = flume::unbounded();

        Self {
            updater: recv,
            current_file: Cell::new(None),

            current: Cell::new(0),
            total: files.len() as u64,

            status_updater_result: None,

            status_updater: Some(std::thread::spawn(move || -> Result<(), Error> {
                // TODO: list original files hashes or something to make repair transitions unique
                let transition_folder = driver.create_transition("action:repair")?;

                for file in files {
                    let updater = Downloader::new(format!("{base_download_uri}/{}", file.to_string_lossy()))
                        .download(transition_folder.join(&file))?;

                    send.send(file)?;

                    // TODO: use updater to show downloading progress better

                    updater.wait()?;
                }

                driver.finish_transition("action:repair")?;

                Ok(())
            }))
        }
    }

    fn update(&self) {
        let mut current = self.current.get();

        while let Ok(file) = self.updater.try_recv() {
            current += 1;

            self.current_file.set(Some(file));
            self.current.set(current);
        }
    }
}

impl UpdaterExt for BasicRepairerUpdater {
    type Error = Error;
    type Status = Status;
    type Result = ();

    #[inline]
    fn status(&mut self) -> Result<Self::Status, &Self::Error> {
        self.update();

        if let Some(status_updater) = self.status_updater.take() {
            if !status_updater.is_finished() {
                self.status_updater = Some(status_updater);

                // TODO: don't like to call clone here all the time
                return Ok(match self.current_file.take() {
                    Some(file) => {
                        self.current_file.set(Some(file.clone()));

                        Status::Working(file)
                    }

                    None => Status::Starting
                });
            }

            self.status_updater_result = Some(status_updater.join().expect("Failed to join thread"));
        }

        match &self.status_updater_result {
            Some(Ok(_)) => Ok(Status::Finished),
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
        matches!(self.status(), Ok(Status::Finished) | Err(_))
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
