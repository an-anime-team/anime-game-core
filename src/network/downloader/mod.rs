use std::path::Path;

pub mod basic;

pub trait DownloaderExt {
    type Error;
    type Updater: UpdaterExt<Self::Error>;

    /// Create downloader instance for given URI
    fn new(uri: impl AsRef<str>) -> Self;

    /// Get total download size of the URI content
    /// 
    /// Return `None` if content size is unknown.
    /// `Err` if failed to request URI's HEAD
    fn content_size(&self) -> Result<Option<usize>, Self::Error>;

    /// Get file name from the URI
    fn file_name(&self) -> String;

    /// Continue downloading if download path already has a file
    fn continue_downloading(self, continue_downloading: bool) -> Self;

    /// Download URI content and save it as `download_path`
    /// 
    /// Return status updater, or `Err` if failed to initiate downloading
    fn download(&self, download_path: impl AsRef<Path>) -> Result<Self::Updater, Self::Error>;
}

pub trait UpdaterExt<Error> {
    /// Check downloader status
    fn status(&mut self) -> Result<bool, &Error>;

    /// Wait for downloading task to complete
    fn wait(self) -> Result<(), Error>;

    /// Get current downloading progress
    fn current_size(&self) -> usize;

    /// Get total downloading content size
    fn total_size(&self) -> usize;

    #[inline]
    /// Get downloading progress
    fn progress(&self) -> f64 {
        self.current_size() as f64 / self.total_size() as f64
    }
}
