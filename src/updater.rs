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
    fn current(&self) -> usize;

    /// Get total progress
    fn total(&self) -> usize;

    #[inline]
    /// Get progress
    fn progress(&self) -> f64 {
        self.current() as f64 / self.total() as f64
    }
}
