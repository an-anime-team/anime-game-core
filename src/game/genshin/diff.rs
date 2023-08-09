use std::cell::Cell;
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::filesystem::DriverExt;
use crate::game::diff::DiffExt;
use crate::updater::UpdaterExt;
use crate::network::downloader::DownloaderExt;

use crate::network::downloader::basic::{
    Downloader,
    Error as DownloaderError
};

use crate::archive;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    DownloaderError(#[from] DownloaderError),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(String)
}

#[derive(Clone)]
pub enum Diff {
    /// Version is latest
    Latest,

    /// Diff is not available
    NotAvailable,

    /// Diff is available and installable
    Available {
        download_uri: String,
        driver: Arc<dyn DriverExt>,
        transition_name: String
    }
}

impl DiffExt for Diff {
    type Updater = Updater;

    #[inline]
    fn is_installable(&self) -> bool {
        matches!(self, Diff::Available { .. })
    }

    fn install(self) -> Option<Self::Updater> {
        let Diff::Available { download_uri, driver, transition_name } = self else {
            return None;
        };

        let (sender, receiver) = flume::unbounded();

        Some(Updater {
            status: Cell::new(Status::PreparingTransition),
            current: Cell::new(0),
            total: Cell::new(1), // To prevent division by 0

            worker_result: None,
            updater: receiver,

            worker: Some(std::thread::spawn(move || -> Result<(), Error> {
                let downloader = Downloader::new(download_uri);

                // Create transition

                let path = driver.create_transition(&transition_name)?;
                let archive = path.join(downloader.file_name());

                // Download update archive

                let mut updater = downloader.download(&archive)?;

                while let Ok(false) = updater.status() {
                    let update = (
                        Status::Downloading,
                        updater.current(),
                        updater.total()
                    );

                    if let Err(err) = sender.send(update) {
                        return Err(Error::FlumeSendError(err.to_string()));
                    }
                }

                // Finish transition

                if let Err(err) = sender.send((Status::FinishingTransition, 0, 1)) {
                    return Err(Error::FlumeSendError(err.to_string()));
                }

                driver.finish_transition(&transition_name)?;

                Ok(())
            }))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    PreparingTransition,
    Downloading,
    Unpacking,
    FinishingTransition,
    ApplyingHdiffPatches,
    DeleteObsoleteFiles
}

pub struct Updater {
    status: Cell<Status>,
    current: Cell<usize>,
    total: Cell<usize>,

    worker: Option<JoinHandle<Result<(), Error>>>,
    worker_result: Option<Result<(), Error>>,
    updater: flume::Receiver<(Status, usize, usize)>
}

impl Updater {
    fn update(&self) {
        while let Ok((status, current, total)) = self.updater.try_recv() {
            self.status.set(status);
            self.current.set(current);
            self.total.set(total);
        }
    }
}

impl UpdaterExt for Updater {
    type Error = Error;
    type Status = Status;
    type Result = ();

    fn status(&mut self) -> Result<Self::Status, &Self::Error> {
        self.update();

        if let Some(worker) = self.worker.take() {
            if !worker.is_finished() {
                self.worker = Some(worker);

                return Ok(self.status.get());
            }

            self.worker_result = Some(worker.join().expect("Failed to join diff updater thread"));
        }

        match &self.worker_result {
            Some(Ok(_)) => Ok(self.status.get()),
            Some(Err(err)) => Err(err),

            None => unreachable!()
        }
    }

    fn wait(mut self) -> Result<Self::Result, Self::Error> {
        if let Some(worker) = self.worker.take() {
            return worker.join().expect("Failed to join diff updater thread");
        }

        else if let Some(result) = self.worker_result.take() {
            return result;
        }

        unreachable!()
    }

    #[inline]
    fn current(&self) -> usize {
        self.update();

        self.current.get()
    }

    #[inline]
    fn total(&self) -> usize {
        self.update();

        self.total.get()
    }
}
