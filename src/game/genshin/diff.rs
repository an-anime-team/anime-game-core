use std::path::PathBuf;
use std::rc::Rc;
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
    DownloaderError(#[from] DownloaderError)
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
        driver: Rc<dyn DriverExt>
    }
}

impl DiffExt for Diff {
    type Updater = Updater;

    #[inline]
    fn is_installable(&self) -> bool {
        matches!(self, Diff::Available { .. })
    }

    fn install(self) -> Option<Self::Updater> {
        let Diff::Available { download_uri, driver } = self else {
            return None;
        };

        Some(Updater {
            current_task: Task::Download,

            worker: std::thread::spawn(move || -> Result<(), Error> {
                Downloader::new(download_uri).download()

                Ok(())
            })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    Download,
    Unpack,
    DeleteObsoleteFiles,
    ApplyHdiffPatches
}

pub struct Updater {
    current_task: Task,

    worker: JoinHandle<Result<(), Error>>
}

impl Updater {
    #[inline]
    pub fn current_task(&self) -> Task {
        self.current_task
    }
}

impl UpdaterExt for Updater {
    type Status = ();

    type Error = ();

    type Result = ();

    fn status(&mut self) -> Result<Self::Status, &Self::Error> {
        todo!()
    }

    fn wait(self) -> Result<Self::Result, Self::Error> {
        todo!()
    }

    fn current(&self) -> usize {
        todo!()
    }

    fn total(&self) -> usize {
        todo!()
    }
}
