use std::cell::Cell;
use std::thread::JoinHandle;

pub trait UpdaterExt {
    type Status;
    type Error;
    type Result;

    /// Check task's status
    fn status(&mut self) -> Result<Self::Status, &Self::Error>;

    /// Wait for task to complete
    fn wait(self) -> Result<Self::Result, Self::Error>;

    /// Check if the task is finished or returned an error
    fn is_finished(&mut self) -> bool;

    /// Get current progress
    fn current(&self) -> u64;

    /// Get total progress
    fn total(&self) -> u64;

    #[inline]
    /// Get progress
    fn progress(&self) -> f64 {
        self.current() as f64 / self.total() as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status<T: std::fmt::Debug + Clone + Copy> {
    Pending,
    Working(T),
    Finished
}

pub struct BasicUpdater<T: std::fmt::Debug + Clone + Copy, Err> {
    status: Cell<Status<T>>,
    current: Cell<u64>,
    total: Cell<u64>,

    worker: Option<JoinHandle<Result<(), Err>>>,
    worker_result: Option<Result<(), Err>>,

    updater: flume::Receiver<(T, u64, u64)>
}

impl<T, Err> BasicUpdater<T, Err>
where
    T: std::fmt::Debug + Clone + Copy + PartialEq + Eq,
    Err: Send + 'static
{
    pub fn spawn(spawn_worker: impl FnOnce(flume::Sender<(T, u64, u64)>) -> Box<dyn FnOnce() -> Result<(), Err> + Send>) -> Self {
        let (sender, receiver) = flume::unbounded();

        Self {
            status: Cell::new(Status::Pending),
            current: Cell::new(0),
            total: Cell::new(1),

            worker: Some(std::thread::spawn(spawn_worker(sender))),
            worker_result: None,

            updater: receiver
        }
    }

    fn update(&self) {
        if self.status.get() != Status::Finished {
            while let Ok((status, current, total)) = self.updater.try_recv() {
                self.status.set(Status::Working(status));
                self.current.set(current);
                self.total.set(total);
            }
        }
    }
}

impl<T, Err> UpdaterExt for BasicUpdater<T, Err>
where
    T: std::fmt::Debug + Clone + Copy + PartialEq + Eq,
    Err: Send + 'static
{
    type Error = Err;
    type Status = Status<T>;
    type Result = ();

    fn status(&mut self) -> Result<Self::Status, &Self::Error> {
        self.update();

        if let Some(worker) = self.worker.take() {
            if !worker.is_finished() {
                self.worker = Some(worker);

                return Ok(self.status.get());
            }

            self.worker_result = Some(worker.join().expect("Failed to join updater thread"));
        }

        self.status.set(Status::Finished);

        match &self.worker_result {
            Some(Ok(_)) => Ok(self.status.get()),
            Some(Err(err)) => Err(err),

            None => unreachable!()
        }
    }

    fn wait(mut self) -> Result<Self::Result, Self::Error> {
        if let Some(worker) = self.worker.take() {
            return worker.join().expect("Failed to join updater thread");
        }

        else if let Some(result) = self.worker_result.take() {
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
        self.update();

        self.total.get()
    }
}
