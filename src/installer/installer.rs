use std::path::PathBuf;
use std::os::unix::prelude::PermissionsExt;

use serde::{Serialize, Deserialize};

use super::downloader::{Downloader, DownloadingError};
use super::archives::Archive;
use super::free_space;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    CheckingFreeSpace(PathBuf),

    /// `(temp path)`
    DownloadingStarted(PathBuf),

    /// `(current bytes, total bytes)`
    DownloadingProgress(u64, u64),

    DownloadingFinished,
    DownloadingError(DownloadingError),

    /// `(unpacking path)`
    UnpackingStarted(PathBuf),

    /// `(current bytes, total bytes)`
    UnpackingProgress(u64, u64),

    UnpackingFinished,
    UnpackingError(String)
}

impl From<DownloadingError> for Update {
    #[inline]
    fn from(err: DownloadingError) -> Self {
        Self::DownloadingError(err)
    }
}

#[derive(Debug)]
pub struct Installer {
    pub downloader: Downloader,

    /// Path to the temp folder used to store archive before unpacking
    pub temp_folder: PathBuf,

    /// Perform free space verifications before downloading file
    pub check_free_space: bool,

    /// How `Downloader` should save the file before unpacking it
    pub filename: Option<String>
}

impl Installer {
    #[inline]
    pub fn new<T: AsRef<str>>(uri: T) -> Result<Self, minreq::Error> {
        Ok(Self {
            downloader: Downloader::new(uri.as_ref())?
                .with_free_space_check(false),

            temp_folder: std::env::temp_dir(),
            check_free_space: true,
            filename: None
        })
    }

    /// Get name of downloading file from uri
    /// 
    /// - `https://example.com/example.zip` -> `example.zip`
    /// - `https://example.com` -> `index.html`
    #[inline]
    pub fn get_filename(&self) -> &str {
        self.filename.as_deref().unwrap_or(self.downloader.get_filename())
    }

    #[inline]
    fn get_temp_path(&self) -> PathBuf {
        self.temp_folder.join(self.get_filename())
    }

    #[inline]
    /// Specify path to the temp folder used to store archive before unpacking
    pub fn with_temp_folder(mut self, path: impl Into<PathBuf>) -> Self {
        self.temp_folder = path.into();

        self
    }

    #[inline]
    /// Specify whether installer should check free space availability
    pub fn with_free_space_check(mut self, check_free_space: bool) -> Self {
        self.check_free_space = check_free_space;

        self
    }

    #[inline]
    /// Specify name of the file `Downloader` will save archive as before unpacking it
    pub fn with_filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);

        self
    }

    /// Download archive from specified uri and unpack it
    pub fn install(&mut self, unpack_to: impl Into<PathBuf>, updater: impl Fn(Update) + Clone + Send + 'static) {
        tracing::trace!("Checking free space availability");

        let temp_path = self.get_temp_path();
        let unpack_to = unpack_to.into();

        // Perform free space verifications if needed
        if self.check_free_space {
            // Check available free space for archive itself
            (updater)(Update::CheckingFreeSpace(temp_path.clone()));

            let Some(space) = free_space::available(&temp_path) else {
                tracing::error!("Path is not mounted: {:?}", temp_path);

                (updater)(DownloadingError::PathNotMounted(temp_path).into());

                return;
            };

            if let Some(required) = self.downloader.length() {
                // We can possibly store downloaded archive + unpacked data on the same disk
                let required = if free_space::is_same_disk(&temp_path, &unpack_to) {
                    (required as f64 * 2.5).ceil() as u64
                } else {
                    required
                };

                if space < required {
                    tracing::error!("No free space available in the temp folder. Required: {required}. Available: {space}");

                    (updater)(DownloadingError::NoSpaceAvailable(temp_path, required, space).into());

                    return;
                }
            }

            // Check available free space for unpacked archvie data (archive size * 1.5)
            (updater)(Update::CheckingFreeSpace(unpack_to.clone()));

            let Some(space) = free_space::available(&unpack_to) else {
                tracing::error!("Path is not mounted: {:?}", temp_path);

                (updater)(DownloadingError::PathNotMounted(unpack_to).into());

                return;
            };

            if let Some(required) = self.downloader.length() {
                // We can possibly store downloaded archive + unpacked data on the same disk
                let required = if free_space::is_same_disk(&unpack_to, &temp_path) {
                    (required as f64 * 2.5).ceil() as u64
                } else {
                    (required as f64 * 1.5).ceil() as u64
                };

                if space < required {
                    tracing::error!("No free space available in the installation folder. Required: {required}. Available: {space}");

                    (updater)(DownloadingError::NoSpaceAvailable(unpack_to, required, space).into());

                    return;
                }
            }
        }

        tracing::trace!("Downloading archive");

        // Download archive
        let download_progress_updater = updater.clone();

        (updater)(Update::DownloadingStarted(temp_path.clone()));

        if let Err(err) = self.downloader.download(&temp_path, move |curr, total| (download_progress_updater)(Update::DownloadingProgress(curr, total))) {
            tracing::error!("Failed to download archive: {err}");

            (updater)(Update::DownloadingError(err));

            return;
        }

        (updater)(Update::DownloadingFinished);

        match Archive::open(&temp_path) {
            Ok(mut archive) => {
                // Temporary workaround as we can't get archive extraction process
                // directly - we'll spawn it in another thread and check this archive entries appearence in the filesystem
                let mut total = 0;
                let entries = archive.get_entries();

                for entry in &entries {
                    total += entry.size.get_size();

                    let path = unpack_to.join(&entry.name);

                    // Failed to change permissions => likely patch-related file and was made by the sudo, so root
                    #[allow(unused_must_use)]
                    if let Err(_) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)) {
                        // For weird reason we can delete files made by root, but can't modify their permissions
                        // We're not checking its result because if it's error - then it's either couldn't be removed (which is not the case)
                        // or the file doesn't exist, which we obviously can just ignore
                        std::fs::remove_file(&path);
                    }
                }

                tracing::trace!("Extracting archive");

                let unpacking_path = unpack_to.clone();
                let unpacking_updater = updater.clone();

                let handle_2 = std::thread::spawn(move || {
                    let mut entries = entries.into_iter()
                        .map(|entry| (unpacking_path.join(&entry.name), entry.size.get_size(), true))
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
                let handle_1 = std::thread::spawn(move || {
                    (updater)(Update::UnpackingStarted(unpack_to.clone()));

                    // We have to create new instance of Archive here
                    // because otherwise it may not work after get_entries method call
                    match Archive::open(&temp_path) {
                        Ok(mut archive) => match archive.extract(unpack_to) {
                            Ok(_) => {
                                // TODO error handling
                                #[allow(unused_must_use)] {
                                    std::fs::remove_file(temp_path);
                                }

                                (updater)(Update::UnpackingFinished);
                            }

                            Err(err) => (updater)(Update::UnpackingError(err.to_string()))
                        }

                        Err(err) => (updater)(Update::UnpackingError(err.to_string()))
                    }
                });

                handle_1.join().unwrap();
                handle_2.join().unwrap();
            }

            Err(err) => (updater)(Update::UnpackingError(err.to_string()))
        }
    }
}
