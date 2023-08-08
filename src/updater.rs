pub trait UpdaterExt<Error> {
    /// Check task's status
    fn status(&mut self) -> Result<bool, &Error>;

    /// Wait for task to complete
    fn wait(self) -> Result<(), Error>;

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
