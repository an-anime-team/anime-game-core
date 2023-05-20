pub mod downloader;
pub mod archives;
pub mod installer;
pub mod free_space;

pub mod prelude {
    pub use super::archives::Archive;
    pub use super::free_space;

    pub use super::downloader::{
        Downloader,
        DownloadingError
    };

    pub use super::installer::{
        Installer,
        Update as InstallerUpdate
    };
}
