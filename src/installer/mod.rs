use std::env::temp_dir;
use std::fs::remove_file;
use std::time::{Instant, Duration};
use std::io::{Error, ErrorKind};

pub mod downloader;
pub mod archives;

use crate::installer::downloader::{
    Stream as DownloaderStream,
    StreamUpdate as DownloaderStreamUpdate,
    Downloaders
};

use crate::installer::archives::{
    Archive,
    StreamUpdate as ArchiveStreamUpdate
};

pub mod prelude {
    pub use super::{Installer, InstallerUpdate};

    pub use super::downloader::{
        StreamUpdate as DownloaderUpdate,
        Downloaders
    };

    pub use super::archives::StreamUpdate as UnpackerUpdate;
}

use uuid::Uuid;

pub enum InstallerUpdate {
    Downloader(DownloaderStreamUpdate),
    Unpacker(ArchiveStreamUpdate)
}

pub struct Installer {
    downloader: DownloaderStream,
    on_update: Box<dyn Fn(InstallerUpdate)>,
    pub method: Downloaders,
    pub unpack_progress_interval: Option<Duration>
}

impl Installer {
    pub fn new<T: ToString>(uri: T) -> Result<Installer, minreq::Error> {
        match DownloaderStream::open(uri) {
            Ok(downloader) => Ok(Self {
                downloader,
                on_update: Box::new(|_| {}),
                method: Downloaders::Native,
                unpack_progress_interval: None
            }),
            Err(err) => Err(err)
        }
    }

    pub fn set_downloader(&mut self, method: Downloaders) {
        self.method = method;
    }

    pub fn set_downloader_interval(&mut self, updates_interval: Duration) {
        self.downloader.download_progress_interval = updates_interval;
    }

    pub fn set_unpacker_interval(&mut self, updates_interval: Duration) {
        self.unpack_progress_interval = Some(updates_interval);
    }

    pub fn on_update<T: Fn(InstallerUpdate) + 'static>(&mut self, callback: T) {
        self.on_update = Box::new(callback);
    }

    pub fn install<T: ToString>(&mut self, path: T) -> Result<Duration, Error> {
        let mut temp_file = temp_dir();

        temp_file.push(format!("/.{}", Uuid::new_v4().to_string()));

        self.install_to(temp_file.to_str().unwrap(), path.to_string().as_str())
    }

    pub fn install_to<T: ToString>(&mut self, temp_path: T, unpack_path: T) -> Result<Duration, Error> {
        let instant = Instant::now();

        let temp_path = temp_path.to_string();

        if let Err(err) = self.downloader.download(temp_path.clone(), self.method) {
            return Err(err);
        }

        match Archive::open(temp_path.clone()) {
            Some(archive) => {
                let mut stream = archive.get_stream();

                if let Some(interval) = self.unpack_progress_interval {
                    stream.unpack_progress_interval = interval;
                }

                if stream.unpack(unpack_path) == None {
                    return Err(Error::new(ErrorKind::InvalidInput, "Archive unpacking error"));
                }

                remove_file(temp_path);

                Ok(instant.elapsed())
            },
            None => Err(Error::new(ErrorKind::InvalidInput, "Downloaded file is not a supported archive type"))
        }
    }
}
