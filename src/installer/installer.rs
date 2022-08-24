use std::env::temp_dir;
use std::os::unix::prelude::PermissionsExt;

use uuid::Uuid;

use super::downloader::{Downloader, DownloadingError};
use super::archives::Archive;
use super::free_space;

#[derive(Debug, Clone)]
pub enum Update {
    CheckingFreeSpace(String),

    /// `(temp path)`
    DownloadingStarted(String),

    /// `(current bytes, total bytes)`
    DownloadingProgress(u64, u64),

    DownloadingFinished,
    DownloadingError(DownloadingError),

    /// `(unpacking path)`
    UnpackingStarted(String),

    /// `(current bytes, total bytes)`
    UnpackingProgress(u64, u64),

    UnpackingFinished,
    UnpackingError
}

impl From<DownloadingError> for Update {
    fn from(err: DownloadingError) -> Self {
        Self::DownloadingError(err)
    }
}

#[derive(Debug)]
pub struct Installer {
    pub downloader: Downloader,
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
    pub fn install<T, F>(&mut self, unpack_to: T, updater: F)
    where
        T: ToString,
        F: Fn(Update) + Clone + Send + 'static
    {
        let temp_path = self.get_temp_path();
        let unpack_to = unpack_to.to_string();

        // Check available free space for archive itself
        (updater)(Update::CheckingFreeSpace(temp_path.clone()));

        match free_space::available(&temp_path) {
            Some(space) => {
                if let Some(required) = self.downloader.length() {
                    // We can possibly store downloaded archive + unpacked data on the same disk
                    let required = if free_space::is_same_disk(&temp_path, &unpack_to) {
                        (required as f64 * 2.5).ceil() as u64
                    } else {
                        required
                    };

                    if space < required {
                        (updater)(DownloadingError::NoSpaceAvailable(temp_path, required, space).into());

                        return;
                    }
                }
            },
            None => {
                (updater)(DownloadingError::PathNotMounted(temp_path).into());

                return;
            }
        }

        // Check available free space for unpacked archvie data (archive size * 1.5)
        (updater)(Update::CheckingFreeSpace(unpack_to.clone()));

        match free_space::available(&unpack_to) {
            Some(space) => {
                if let Some(required) = self.downloader.length() {
                    // We can possibly store downloaded archive + unpacked data on the same disk
                    let required = if free_space::is_same_disk(&unpack_to, &temp_path) {
                        (required as f64 * 2.5).ceil() as u64
                    } else {
                        (required as f64 * 1.5).ceil() as u64
                    };

                    if space < required {
                        (updater)(DownloadingError::NoSpaceAvailable(unpack_to, required, space).into());

                        return;
                    }
                }
            },
            None => {
                (updater)(DownloadingError::PathNotMounted(unpack_to).into());

                return;
            }
        }

        // Download archive
        let download_progress_updater = updater.clone();

        (updater)(Update::DownloadingStarted(temp_path.clone()));

        match self.downloader.download_to(&temp_path, move |curr, total| {
            (download_progress_updater)(Update::DownloadingProgress(curr, total));
        }) {
            Ok(_) => {
                (updater)(Update::DownloadingFinished);

                match Archive::open(&temp_path) {
                    Some(mut archive) => {
                        // Temporary workaround as we can't get archive extraction process
                        // directly - we'll spawn it in another thread and check this archive entries appearence in the filesystem
                        let mut total = 0;
                        let entries = archive.get_entries();

                        for entry in &entries {
                            total += entry.size.get_size();

                            let path = format!("{}/{}", &unpack_to, entry.name);

                            // Failed to change permissions => likely patch-related file and was made by the sudo, so root
                            #[allow(unused_must_use)]
                            if let Err(_) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)) {
                                // For weird reason we can delete files made by root, but can't modify their permissions
                                // We're not checking its result because if it's error - then it's either couldn't be removed (which is not the case)
                                // or the file doesn't exist, which we obviously can just ignore
                                std::fs::remove_file(&path);
                            }
                        }

                        let unpacking_path = unpack_to.clone();
                        let unpacking_updater = updater.clone();

                        let now = std::time::SystemTime::now();

                        let handle_2 = std::thread::spawn(move || {
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

                                        let path = std::path::Path::new(path);

                                        if let Ok(metadata) = path.metadata() {
                                            match metadata.modified() {
                                                Ok(time) => {
                                                    // Mark file as downloaded only if it was modified recently
                                                    if time > now {
                                                        *remained = false;

                                                        unpacked += *size;
                                                    }
                                                },

                                                // Some systems may not have this parameter
                                                Err(_) => {
                                                    *remained = false;

                                                    unpacked += *size;
                                                }
                                            }
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
                        let handle_1 = std::thread::spawn(move || {
                            (updater)(Update::UnpackingStarted(unpack_to.clone()));

                            // We have to create new instance of Archive here
                            // because otherwise it may not work after get_entries method call
                            Archive::open(&temp_path).unwrap().extract(unpack_to);

                            (updater)(Update::UnpackingFinished);

                            // TODO error handling
                            std::fs::remove_file(temp_path).expect("Failed to remove temporary file");
                        });

                        handle_1.join().unwrap();
                        handle_2.join().unwrap();
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
