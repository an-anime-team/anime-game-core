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
    url: String,

    /// Path to the temp folder used to store archive before unpacking
    pub temp_folder: String
}

impl Installer {
    pub fn new<T: ToString>(url: T) -> Result<Self, curl::Error> {
        match Downloader::new(url.to_string()) {
            Ok(downloader) => Ok(Self {
                downloader,
                url: url.to_string(),
                temp_folder: temp_dir().to_str().unwrap().to_string()
            }),
            Err(err) => Err(err)
        }
    }

    /// Specify path to the temp folder used to store archive before unpacking
    pub fn set_temp_folder<T: ToString>(mut self, path: T) -> Self {
        self.temp_folder = path.to_string();

        self
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

    fn get_temp_path(&self) -> String {
        format!("{}/.{}-{}", self.temp_folder, Uuid::new_v4().to_string(), self.get_filename())
    }

    /// Download archive from specified uri and unpack it
    /// 
    /// This does not freeze current thread
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

                let unpack_to = unpack_to.to_string();

                match Archive::open(&temp_path) {
                    Some(mut archive) => {
                        // Temporary workaround as we can't get archive extraction process
                        // directly - we'll spawn it in another thread and check this archive entries appearence in the filesystem
                        let mut total = 0;
                        let entries = archive.get_entries();

                        for entry in &entries {
                            total += entry.size.get_size();
                        }

                        let unpacking_path = unpack_to.clone();
                        let unpacking_updater = updater.clone();

                        std::thread::spawn(move || {
                            let mut entries = entries.into_iter()
                                .map(|entry| (format!("{}/{}", unpacking_path, entry.name), entry.size.get_size(), true))
                                .collect::<Vec<_>>();

                            let mut unpacked = 0;

                            loop {
                                std::thread::sleep(std::time::Duration::from_millis(250));

                                let mut empty = true;

                                for (path, size, remained) in &mut entries {
                                    if *remained {
                                        empty = false;

                                        if std::path::Path::new(path).exists() {
                                            *remained = false;

                                            unpacked += *size;
                                        }
                                    }
                                }

                                (unpacking_updater)(Update::UnpackingProgress(unpacked, total));

                                if empty {
                                    break;
                                }
                            }
                        });

                        // Run archive extraction in another thread to not to freeze the current one
                        std::thread::spawn(move || {
                            (updater)(Update::UnpackingStarted(unpack_to.clone()));

                            // We have to create new instance of Archive here
                            // because otherwise it may not work after get_entries method call
                            Archive::open(&temp_path).unwrap().extract(unpack_to);

                            (updater)(Update::UnpackingFinished);

                            // TODO error handling
                            std::fs::remove_file(temp_path).expect("Failed to remove temporary file");
                        });
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
