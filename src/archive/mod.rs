use std::path::Path;

pub mod updater;
pub mod entry;

pub mod sevenz;
pub mod zip;
pub mod tar;

use crate::updater::UpdaterExt;

use updater::BasicUpdater;
use entry::BasicEntry;

use sevenz::Archive as SevenZip;
use zip::Archive as Zip;
use tar::Archive as Tar;

pub trait ArchiveExt {
    type Error;
    type Entry;
    type Updater: UpdaterExt;

    fn open(file: impl AsRef<Path>) -> Result<Self, Self::Error> where Self: Sized;
    fn entries(&self) -> Result<Vec<Self::Entry>, Self::Error>;
    fn extract(&self, folder: impl AsRef<Path>) -> Result<Self::Updater, Self::Error>;
}

/// Automatically identify archive format based on its extension,
/// and return its entries if this format is supported
pub fn entries(archive: impl AsRef<Path>) -> Option<Vec<BasicEntry>> {
    let archive = archive.as_ref();

    if !archive.is_file() {
        None
    }

    else if archive.ends_with(".tar.xz") || archive.ends_with(".tar.gz") || archive.ends_with(".tar.bz2") || archive.ends_with(".tar") {
        Tar::open(archive)
            .and_then(|archive| archive.entries())
            .ok()
    }

    else if archive.ends_with(".zip") {
        Zip::open(archive)
            .and_then(|archive| archive.entries())
            .ok()
    }

    else if archive.ends_with(".7z") {
        SevenZip::open(archive)
            .and_then(|archive| archive.entries())
            .ok()
    }

    else {
        None
    }
}

/// Automatically identify archive format based on its extension,
/// and extract its entries if this format is supported
pub fn extract(archive: impl AsRef<Path>, extract_to: impl AsRef<Path>) -> Option<BasicUpdater> {
    let archive = archive.as_ref();
    let path = archive.to_string_lossy();

    if !archive.is_file() {
        None
    }

    else if path.ends_with(".tar.xz") || path.ends_with(".tar.gz") || path.ends_with(".tar.bz2") || path.ends_with(".tar") {
        Tar::open(archive)
            .and_then(|archive| archive.extract(extract_to))
            .ok()
    }

    else if path.ends_with(".zip") {
        Zip::open(archive)
            .and_then(|archive| archive.extract(extract_to))
            .ok()
    }

    else if path.ends_with(".7z") {
        SevenZip::open(archive)
            .and_then(|archive| archive.extract(extract_to))
            .ok()
    }

    else {
        None
    }
}
