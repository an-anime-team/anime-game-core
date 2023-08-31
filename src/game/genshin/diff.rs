use std::sync::Arc;

use crate::updater::*;

use crate::filesystem::DriverExt;
use crate::game::diff::DiffExt;
use crate::game::hoyoverse_diffs;
use crate::network::downloader::DownloaderExt;

use crate::network::downloader::basic::{
    Downloader,
    Error as DownloaderError
};

use crate::archive;

// TODO: unify this diff implementation for all the hoyo games under hoyoverse_diffs module

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Downloader error: {0}")]
    DownloaderError(#[from] DownloaderError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(#[from] flume::SendError<(Status, u64, u64)>),

    #[error("Unable to extract archive")]
    UnableToExtractArchive
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    PreparingTransition,
    Downloading,
    Unpacking,
    RunTransitionCode,
    FinishingTransition,
    RunPostTransitionCode,
    ApplyingHdiffPatches,
    DeletingObsoleteFiles,
    Finished
}

impl DiffExt for Diff {
    type Updater = BasicUpdater<Status, Error>;

    #[inline]
    fn is_installable(&self) -> bool {
        matches!(self, Diff::Available { .. })
    }

    fn install(self) -> Option<Self::Updater> {
        let Diff::Available { download_uri, driver, transition_name } = self else {
            return None;
        };

        Some(BasicUpdater::spawn(|sender| {
            Box::new(move || -> Result<(), Error> {
                let downloader = Downloader::new(download_uri);

                // Create transition

                let transition_path = driver.create_transition(&transition_name)?;

                // Download update archive

                let archive = transition_path.join(downloader.file_name());

                let mut updater = downloader.download(&archive)?;

                while !updater.is_finished() {
                    sender.send((
                        Status::Downloading,
                        updater.current(),
                        updater.total()
                    ))?;
                }

                // Extract archive

                let Some(mut updater) = archive::extract(&archive, &transition_path) else {
                    return Err(Error::UnableToExtractArchive);
                };

                while let Ok(false) = updater.status() {
                    sender.send((
                        Status::Unpacking,
                        updater.current(),
                        updater.total()
                    ))?;
                }

                std::fs::remove_file(archive)?;

                // Run transition code

                sender.send((Status::RunTransitionCode, 0, 1))?;

                let updater = sender.clone();

                hoyoverse_diffs::apply_update(driver.clone(), &transition_path, move |status| {
                    let result = match status {
                        hoyoverse_diffs::Status::ApplyingHdiffStarted => updater.send((Status::ApplyingHdiffPatches, 0, 1)),
                        hoyoverse_diffs::Status::ApplyingHdiffFinished => updater.send((Status::ApplyingHdiffPatches, 1, 1)),

                        hoyoverse_diffs::Status::ApplyingHdiffProgress(current, total) =>
                            updater.send((Status::ApplyingHdiffPatches, current, total)),

                        hoyoverse_diffs::Status::DeletingObsoleteStarted => updater.send((Status::DeletingObsoleteFiles, 0, 1)),
                        hoyoverse_diffs::Status::DeletingObsoleteFinished => updater.send((Status::RunTransitionCode, 1, 1)),

                        hoyoverse_diffs::Status::DeletingObsoleteProgress(current, total) =>
                            updater.send((Status::RunTransitionCode, current, total))
                    };

                    result.expect("Failed to send flume message from the transition code updater");
                })?;

                // Finish transition

                sender.send((Status::FinishingTransition, 0, 1))?;

                driver.finish_transition(&transition_name)?;

                // Run post-transition code

                sender.send((Status::RunPostTransitionCode, 0, 1))?;

                // TODO: re-use code defined above
                let updater = sender.clone();

                hoyoverse_diffs::post_transition(driver, move |status| {
                    let result = match status {
                        hoyoverse_diffs::Status::ApplyingHdiffStarted => updater.send((Status::ApplyingHdiffPatches, 0, 1)),
                        hoyoverse_diffs::Status::ApplyingHdiffFinished => updater.send((Status::ApplyingHdiffPatches, 1, 1)),

                        hoyoverse_diffs::Status::ApplyingHdiffProgress(current, total) =>
                            updater.send((Status::ApplyingHdiffPatches, current, total)),

                        hoyoverse_diffs::Status::DeletingObsoleteStarted => updater.send((Status::DeletingObsoleteFiles, 0, 1)),
                        hoyoverse_diffs::Status::DeletingObsoleteFinished => updater.send((Status::RunTransitionCode, 1, 1)),

                        hoyoverse_diffs::Status::DeletingObsoleteProgress(current, total) =>
                            updater.send((Status::RunTransitionCode, current, total))
                    };

                    result.expect("Failed to send flume message from the transition code updater");
                })?;

                // Finish diff

                sender.send((Status::Finished, 0, 1))?;

                Ok(())
            })
        }))
    }
}
