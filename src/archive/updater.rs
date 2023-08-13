use std::io::{BufReader, BufRead};
use std::process::Child;
use std::thread::JoinHandle;
use std::cell::Cell;
use std::time::Duration;

use crate::updater::UpdaterExt;

pub const UPDATER_TIMEOUT: Duration = Duration::from_secs(1);

// pub type Error = flume::SendError<usize>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(#[from] flume::SendError<usize>),

    #[error("Failed to wait for updater's process end: {0}")]
    ProcessWaitError(#[from] std::io::Error),

    #[error("Failed to execute updater process")]
    ProcessError
}

pub struct BasicUpdater {
    status_updater: Option<JoinHandle<Result<(), Error>>>,
    status_updater_result: Option<Result<(), Error>>,

    incrementer: flume::Receiver<usize>,

    current: Cell<usize>,
    total: usize
}

impl BasicUpdater {
    pub fn new<F: Fn(&str) -> Option<usize> + Send + 'static>(mut process: Child, file_count: usize, out_processor: F) -> Self {
        let (send, recv) = flume::unbounded();

        Self {
            incrementer: recv,

            current: Cell::new(0),
            total: file_count,

            status_updater_result: None,

            status_updater: Some(std::thread::spawn(move || -> Result<(), Error> {
                if let Some(stdout) = &mut process.stdout {
                    let reader = BufReader::new(stdout);

                    for line in reader.lines().flatten() {
                        if let Some(count) = (out_processor)(line.as_str()) {
                            send.send(count)?;
                        }
                    }
                }

                if process.wait()?.success() {
                    Ok(())
                } else {
                    Err(Error::ProcessError)
                }
            }))
        }
    }
}

impl UpdaterExt for BasicUpdater {
    type Error = Error;
    type Status = bool;
    type Result = ();

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
