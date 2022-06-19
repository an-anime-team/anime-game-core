use std::env::temp_dir;

use uuid::Uuid;

use super::downloader::Downloader;
use super::archives::Archive;

#[derive(Debug, Clone)]
pub enum Update {
    /// (temp path)
    DownloadingStarted(String),
    /// (current bytes, total bytes)
    DownloadingProgress(u64, u64),
    DownloadingFinished,
    DownloadingError(curl::Error),

    /// (unpacking path)
    UnpackingStarted(String),
    /// (current bytes, total bytes)
    UnpackingProgress(u64, u64),
    UnpackingFinished,
    UnpackingError
}

#[derive(Debug)]
pub struct Installer {
    downloader: Downloader,
    url: String
}

impl Installer {
    pub fn new<T: ToString>(url: T) -> Result<Self, curl::Error> {
        match Downloader::new(url.to_string()) {
            Ok(downloader) => Ok(Self { downloader, url: url.to_string() }),
            Err(err) => Err(err)
        }
    }

    /// Get name of downloading file from uri
    /// 
    /// - `https://example.com/example.zip` -> `example.zip`
    /// - `https://example.com` -> `index.html`
    pub fn get_filename(&self) -> &str {
        match self.url.rfind('/') {
            Some(index) => {
                let file = &self.url[index + 1..];

                if file == "" { "index.html" } else { file }
            },
            None => "index.html"
        }
    }

    // TODO: ability to specify temp folder
    fn get_temp_path(&self) -> String {
        let temp_file = temp_dir().to_str().unwrap().to_string();

        format!("{}/.{}-{}", temp_file, Uuid::new_v4().to_string(), self.get_filename())
    }

    pub fn install<T, F>(&mut self, unpack_to: T, updater: F)
    where
        T: ToString,
        F: Fn(Update) + Clone + Send + 'static
    {
        let temp_path = self.get_temp_path();
        let download_progress_updater = updater.clone();

        (updater)(Update::DownloadingStarted(temp_path.clone()));

        match self.downloader.download_to(&temp_path, move |curr, total| {
            (download_progress_updater)(Update::DownloadingProgress(curr, total));
        }) {
            Ok(_) => {
                (updater)(Update::DownloadingFinished);

                match Archive::open(&temp_path) {
                    Some(mut archive) => {
                        (updater)(Update::UnpackingStarted(unpack_to.to_string()));

                        archive.extract(unpack_to);

                        (updater)(Update::UnpackingFinished);

                        // TODO error handling
                        std::fs::remove_file(temp_path).expect("Failed to remove temporary file");
                    },
                    None => {
                        (updater)(Update::UnpackingError);
                    }
                }
            },
            Err(err) => {
                (updater)(Update::DownloadingError(err));
            }
        }
    }
}
