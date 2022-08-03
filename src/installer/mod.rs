pub mod downloader;
pub mod archives;
pub mod installer;
pub mod diff;
pub mod free_space;

pub mod prelude {
    pub use super::downloader::Downloader;
    pub use super::archives::Archive;
    pub use super::installer::{
        Installer,
        Update as InstallerUpdate
    };
    pub use super::diff::*;
    pub use super::free_space;
}
