use std::path::Path;

use crate::updater::UpdaterExt;

pub mod basic;

pub trait DownloaderExt {
    type Error;
    type Updater: UpdaterExt;

    /// Create downloader instance for given URI
    fn new(uri: impl AsRef<str>) -> Self;

    /// Get total download size of the URI content
    /// 
    /// Return `None` if content size is unknown.
    /// `Err` if failed to request URI's HEAD
    fn content_size(&self) -> Result<Option<u64>, Self::Error>;

    /// Get file name from the URI
    fn file_name(&self) -> String;

    /// Continue downloading if download path already has a file
    fn continue_downloading(self, continue_downloading: bool) -> Self;

    /// Download URI content and save it as `download_path`
    /// 
    /// Return status updater, or `Err` if failed to initiate downloading
    fn download(&self, download_path: impl AsRef<Path>) -> Result<Self::Updater, Self::Error>;
}
