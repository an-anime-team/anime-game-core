use std::io::{Write, Seek};
use std::path::PathBuf;
use std::fs::File;

use serde::{Serialize, Deserialize};
use thiserror::Error;

use super::free_space;
use crate::prettify_bytes::prettify_bytes;

/// Default amount of bytes `Downloader::download` method will send to `downloader` function
pub const DEFAULT_CHUNK_SIZE: usize = 128 * 1024; // 128 KB

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadingError {
    /// Specified downloading path is not available in system
    /// 
    /// `(path)`
    #[error("Path is not mounted: {0:?}")]
    PathNotMounted(PathBuf),

    /// No free space available under specified path
    /// 
    /// `(path, required, available)`
    #[error("No free space availale for specified path: {0:?} (requires {}, available {})", prettify_bytes(*.1), prettify_bytes(*.2))]
    NoSpaceAvailable(PathBuf, u64, u64),

    /// Failed to create or open output file
    /// 
    /// `(path, error message)`
    #[error("Failed to create output file {0:?}: {1}")]
    OutputFileError(PathBuf, String),

    /// Couldn't get metadata of existing output file
    /// 
    /// This metadata supposed to be used to continue downloading of the file
    /// 
    /// `(path, error message)`
    #[error("Failed to read metadata of the output file {0:?}: {1}")]
    OutputFileMetadataError(PathBuf, String),

    /// minreq error
    #[error("minreq error: {0}")]
    Minreq(String)
}

impl From<minreq::Error> for DownloadingError {
    fn from(error: minreq::Error) -> Self {
        DownloadingError::Minreq(error.to_string())
    }
}

#[derive(Debug)]
pub struct Downloader {
    uri: String,
    length: Option<u64>,

    /// Amount of bytes `Downloader::download` method will send to `downloader` function
    pub chunk_size: usize,

    /// If true, then `Downloader` will try to continue downloading of the file.
    /// Otherwise it will re-download the file entirely
    pub continue_downloading: bool
}

impl Downloader {
    pub fn new<T: AsRef<str>>(uri: T) -> Result<Self, minreq::Error> {
        let uri = uri.as_ref();

        let header = minreq::head(uri)
            .with_timeout(*crate::REQUESTS_TIMEOUT)
            .send()?;

        let length = header.headers.get("content-length")
            .map(|len| len.parse().expect("Requested site's content-length is not a number"));

        Ok(Self {
            uri: uri.to_owned(),
            length,

            chunk_size: DEFAULT_CHUNK_SIZE,
            continue_downloading: true
        })
    }

    #[inline]
    /// Get content length
    pub fn length(&self) -> Option<u64> {
        self.length
    }

    /// Get name of downloading file from uri
    /// 
    /// - `https://example.com/example.zip` -> `example.zip`
    /// - `https://example.com` -> `index.html`
    pub fn get_filename(&self) -> &str {
        if let Some(pos) = self.uri.replace('\\', "/").rfind(|c| c == '/') {
            if pos < self.uri.len() - 1 {
                return &self.uri[pos + 1..];
            }
        }

        "index.html"
    }

    #[tracing::instrument(level = "debug", skip(progress), ret)]
    pub fn download<T, Fp>(&mut self, path: T, progress: Fp) -> Result<(), DownloadingError>
    where
        T: Into<PathBuf> + std::fmt::Debug,
        // `(curr, total)`
        Fp: Fn(u64, u64) + Send + 'static
    {
        tracing::debug!("Checking free space availability");

        let path = path.into();

        // Check available free space
        match free_space::available(&path) {
            Some(space) => {
                if let Some(required) = self.length() {
                    if space < required {
                        return Err(DownloadingError::NoSpaceAvailable(path, required, space));
                    }
                }
            }

            None => return Err(DownloadingError::PathNotMounted(path))
        }

        let mut downloaded = 0;

        // Open or create output file
        let file = if path.exists() && self.continue_downloading {
            tracing::debug!("Opening output file");

            let mut file = std::fs::OpenOptions::new().read(true).write(true).open(&path);

            // Continue downloading if the file exists and can be opened
            if let Ok(file) = &mut file {
                match file.metadata() {
                    Ok(metadata) => {
                        if let Err(err) = file.seek(std::io::SeekFrom::Start(metadata.len())) {
                            return Err(DownloadingError::OutputFileError(path, err.to_string()));
                        }

                        downloaded = metadata.len() as usize;
                    }

                    Err(err) => return Err(DownloadingError::OutputFileMetadataError(path, err.to_string()))
                }
            }

            file
        } else {
            tracing::debug!("Creating output file");

            let base_folder = path.parent().unwrap();

            if !base_folder.exists() {
                if let Err(err) = std::fs::create_dir_all(base_folder) {
                    return Err(DownloadingError::OutputFileError(path, err.to_string()));
                }
            }

            File::create(&path)
        };

        // Download data
        match file {
            Ok(mut file) => {
                let mut chunk = Vec::with_capacity(self.chunk_size);

                let request = minreq::head(&self.uri)
                    .with_header("range", format!("bytes={downloaded}-"))
                    .send()?;

                // Request content range (downloaded + remained content size)
                // 
                // If finished or overcame: bytes */10611646760
                // If not finished: bytes 10611646759-10611646759/10611646760
                // 
                // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Range
                if let Some(range) = request.headers.get("content-range") {
                    // Finish downloading if header says that we've already downloaded all the data
                    if range.contains("*/") {
                        (progress)(self.length.unwrap_or(downloaded as u64), self.length.unwrap_or(downloaded as u64));

                        return Ok(());
                    }
                }

                let request = minreq::get(&self.uri)
                    .with_header("range", format!("bytes={downloaded}-"))
                    .send_lazy()?;

                // HTTP 416 = provided range is overcame actual content length (means file is downloaded)
                // I check this here because HEAD request can return 200 OK while GET - 416
                // 
                // https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/416
                if request.status_code == 416 {
                    (progress)(self.length.unwrap_or(downloaded as u64), self.length.unwrap_or(downloaded as u64));

                    return Ok(());
                }

                for byte in request {
                    let (byte, expected_len) = byte?;

                    chunk.push(byte);

                    if chunk.len() == self.chunk_size {
                        if let Err(err) = file.write_all(&chunk) {
                            return Err(DownloadingError::OutputFileError(path, err.to_string()));
                        }

                        chunk.clear();

                        downloaded += self.chunk_size;

                        (progress)(downloaded as u64, self.length.unwrap_or(expected_len as u64));
                    }
                }

                if !chunk.is_empty() {
                    if let Err(err) = file.write_all(&chunk) {
                        return Err(DownloadingError::OutputFileError(path, err.to_string()));
                    }

                    downloaded += chunk.len();

                    (progress)(downloaded as u64, downloaded as u64); // may not be true..?
                }

                Ok(())
            }

            Err(err) => Err(DownloadingError::OutputFileError(path, err.to_string()))
        }
    }
}
