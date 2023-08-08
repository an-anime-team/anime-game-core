use std::path::PathBuf;
use std::process::Child;
use std::thread::JoinHandle;
use std::cell::Cell;
use std::time::Duration;

use crate::updater::UpdaterExt;

pub const UPDATER_TIMEOUT: Duration = Duration::from_secs(1);

pub struct BasicUpdater {
    _process: Child,

    status_updater: Option<JoinHandle<Result<(), flume::SendError<usize>>>>,
    status_updater_result: Option<Result<(), flume::SendError<usize>>>,

    incrementer: flume::Receiver<usize>,

    current: Cell<usize>,
    total: usize
}

impl BasicUpdater {
    pub fn new(process: Child, mut files: Vec<PathBuf>) -> Self {
        let (send, recv) = flume::unbounded();

        Self {
            _process: process,

            incrementer: recv,

            current: Cell::new(0),
            total: files.len(),

            status_updater_result: None,

            status_updater: Some(std::thread::spawn(move || -> Result<(), flume::SendError<usize>> {
                let mut prev_files = files.len();

                while !files.is_empty() {
                    files.retain(|file| !file.exists());

                    let new_files = prev_files - files.len();

                    if new_files > 0 {
                        send.send(new_files)?;

                        prev_files -= new_files;
                    }

                    std::thread::sleep(UPDATER_TIMEOUT);
                }

                Ok(())
            }))
        }
    }
}

impl UpdaterExt<flume::SendError<usize>> for BasicUpdater {
    #[inline]
    fn status(&mut self) -> Result<bool, &flume::SendError<usize>> {
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
    fn wait(mut self) -> Result<(), flume::SendError<usize>> {
        if let Some(worker) = self.status_updater.take() {
            return worker.join().expect("Failed to join thread");
        }

        else if let Some(result) = self.status_updater_result.take() {
            return result;
        }

        unreachable!()
    }

    #[inline]
    fn current(&self) -> usize {
        let mut current = self.current.get();

        while let Ok(increment) = self.incrementer.try_recv() {
            current += increment;
        }

        self.current.set(current);

        current
    }

    #[inline]
    fn total(&self) -> usize {
        self.total
    }
}
