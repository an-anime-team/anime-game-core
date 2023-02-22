use std::io::{Write, Seek};
use std::path::PathBuf;
use std::fs::File;

use curl::easy::Easy;

use super::free_space;

/// Default amount of bytes `Downloader::download_to` method will store in memory
/// before writing them onto the disk
pub const DEFAULT_DOWNLOADING_CHUNK: usize = 1024 * 1024;

/// Default amount of progress updates that will be skipped each time
/// before calling progress function
pub const DEFAULT_DOWNLOADING_UPDATES_FREQUENCE: usize = 4000;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum DownloadingError {
    /// Specified downloading path is not available in system
    /// 
    /// `(path)`
    PathNotMounted(PathBuf),

    /// No free space available under specified path
    /// 
    /// `(path, required, available)`
    NoSpaceAvailable(PathBuf, u64, u64),

    /// Failed to create or open output file
    /// 
    /// `(path, error message)`
    OutputFileError(PathBuf, String),

    /// Couldn't get metadata of existing output file
    /// 
    /// This metadata supposed to be used to continue downloading of the file
    /// 
    /// `(path, error message)`
    OutputFileMetadataError(PathBuf, String),

    /// Curl downloading error
    Curl(String)
}

impl From<curl::Error> for DownloadingError {
    fn from(err: curl::Error) -> Self {
        Self::Curl(err.to_string())
    }
}

impl Into<std::io::Error> for DownloadingError {
    fn into(self) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, match self {
            Self::PathNotMounted(path) => format!("Path is not mounted: {:?}", path),
            Self::NoSpaceAvailable(path, required, available) => format!("No free space availale for specified path: {:?} (requires {required}, available {available})", path),
            Self::OutputFileError(path, err) => format!("Failed to create output file {:?}: {err}", path),
            Self::OutputFileMetadataError(path, err) => format!("Failed to read metadata of the output file {:?}: {err}", path),
            Self::Curl(curl) => format!("Curl error: {curl}")
        })
    }
}

#[derive(Debug)]
pub struct Downloader {
    length: Option<u64>,
    uri: String,
    curl: Easy,

    /// Amount of bytes `download_to` method will store in memory before
    /// writing them onto the disk. Uses `DEFAULT_DOWNLOADING_CHUNK` value
    /// by default
    pub downloading_chunk: usize,

    /// Amount of progress updates that will be skipped each time
    /// before calling progress function
    pub downloading_updates_frequence: usize,

    /// If true, then Downloader will try to continue downloading of the file.
    /// Otherwise it will re-download the file entirely
    pub continue_downloading: bool
}

impl Downloader {
    /// Try to open downloading stream
    /// 
    /// Will return `Error` if the URL is not valid
    #[tracing::instrument(level = "trace")]
    pub fn new<T: ToString + std::fmt::Debug>(uri: T) -> Result<Self, curl::Error> {
        let mut curl = Easy::new();

        curl.url(&uri.to_string())?;

        curl.follow_location(true)?;
        curl.progress(true)?;

        curl.nobody(true)?;

        if let Ok(length) = curl.content_length_download() {
            if length >= 0.0 {
                return Ok(Self {
                    length: Some(length.ceil() as u64),
                    uri: uri.to_string(),
                    curl,
                    downloading_chunk: DEFAULT_DOWNLOADING_CHUNK,
                    downloading_updates_frequence: DEFAULT_DOWNLOADING_UPDATES_FREQUENCE,
                    continue_downloading: true
                });
            }
        }

        else if let Ok(length) = curl.download_size() {
            if length >= 0.0 {
                return Ok(Self {
                    length: Some(length.ceil() as u64),
                    uri: uri.to_string(),
                    curl,
                    downloading_chunk: DEFAULT_DOWNLOADING_CHUNK,
                    downloading_updates_frequence: DEFAULT_DOWNLOADING_UPDATES_FREQUENCE,
                    continue_downloading: true
                });
            }
        }
        
        let (send, recv) = std::sync::mpsc::channel::<u64>();

        curl.header_function(move |header| {
            let header = String::from_utf8_lossy(header);

            // Content-Length: 8899
            #[allow(unused_must_use)]
            if header.len() > 16 && header[..16].to_lowercase() == "content-length: " {
                send.send(header[16..header.len() - 2].parse::<u64>().unwrap());
            }

            true
        })?;

        curl.perform()?;

        let mut content_length = 0;

        while let Ok(len) = recv.try_recv() {
            if len > 0 {
                content_length = len;
            }
        }

        Ok(Self {
            length: match content_length {
                0 => None,
                len => Some(len)
            },
            uri: uri.to_string(),
            curl,
            downloading_chunk: DEFAULT_DOWNLOADING_CHUNK,
            downloading_updates_frequence: DEFAULT_DOWNLOADING_UPDATES_FREQUENCE,
            continue_downloading: true
        })
    }

    /// Get content length
    pub fn length(&self) -> Option<u64> {
        self.length
    }

    /// Set downloading chunk size
    pub fn set_downloading_chunk(&mut self, size: usize) {
        self.downloading_chunk = size;
    }

    /// Set downloading speed limit, bytes per second
    pub fn set_downloading_speed(&mut self, speed: u64) -> Result<(), curl::Error> {
        Ok(self.curl.max_recv_speed(speed)?)
    }

    /// Get name of downloading file from uri
    /// 
    /// - `https://example.com/example.zip` -> `example.zip`
    /// - `https://example.com` -> `index.html`
    pub fn get_filename(&self) -> &str {
        match self.uri.rfind('/') {
            Some(index) => {
                let file = &self.uri[index + 1..];

                if file == "" { "index.html" } else { file }
            },
            None => "index.html"
        }
    }

    #[tracing::instrument(level = "trace", skip(downloader, progress))]
    pub fn download<Fd, Fp>(&mut self, mut downloader: Fd, progress: Fp) -> Result<(), DownloadingError>
    where
        // array of bytes
        Fd: FnMut(&[u8]) -> Result<usize, curl::easy::WriteError> + Send + 'static,
        // (curr, total)
        Fp: Fn(u64, u64) + Send + 'static
    {
        self.curl.nobody(false)?;

        self.curl.write_function(move |data| {
            (downloader)(data)
        })?;

        let content_length = self.length.clone();

        let downloading_chunk = self.downloading_chunk as u64;
        let updates_frequence = self.downloading_updates_frequence;

        let mut i = 0_usize;

        self.curl.progress_function(move |expected_total, downloaded, _, _| {
            let curr = downloaded as u64;
            let total = content_length.unwrap_or(expected_total as u64);

            i += 1;

            if i == updates_frequence || total.checked_sub(curr).unwrap_or(0) <= downloading_chunk {
                (progress)(curr, total);

                i = 0;
            }

            true
        })?;

        match self.curl.perform() {
            Ok(_) => Ok(()),
            Err(err) => Err(DownloadingError::Curl(err.to_string()))
        }
    }

    #[tracing::instrument(level = "trace", skip(progress))]
    pub fn download_to<T, Fp>(&mut self, path: T, progress: Fp) -> Result<(), DownloadingError>
    where
        T: Into<PathBuf> + std::fmt::Debug,
        // (curr, total)
        Fp: Fn(u64, u64) + Send + 'static
    {
        let path: PathBuf = path.into();

        // Check available free space
        match free_space::available(&path) {
            Some(space) => {
                if let Some(required) = self.length() {
                    if space < required {
                        return Err(DownloadingError::NoSpaceAvailable(path, required, space));
                    }
                }
            },
            None => return Err(DownloadingError::PathNotMounted(path))
        }

        // Set downloading from beginning
        if let Err(err) = self.curl.resume_from(0) {
            return Err(DownloadingError::Curl(err.to_string()));
        }

        // Current downloading progress
        let mut curr = 0_usize;

        // Open or create output file
        let file = if path.exists() && self.continue_downloading {
            let mut file = std::fs::OpenOptions::new().read(true).write(true).open(&path);

            // Continue downloading if the file exists and can be opened
            if let Ok(file) = &mut file {
                match file.metadata() {
                    Ok(metadata) => {
                        if let Err(err) = self.curl.resume_from(metadata.len()) {
                            return Err(DownloadingError::Curl(err.to_string()));
                        }

                        if let Err(err) = file.seek(std::io::SeekFrom::Start(metadata.len())) {
                            return Err(DownloadingError::OutputFileError(path, err.to_string()));
                        }

                        curr = metadata.len() as usize;
                    },
                    Err(err) => return Err(DownloadingError::OutputFileMetadataError(path, err.to_string()))
                }
            }

            file
        } else {
            let base_folder = path.parent().unwrap();

            #[allow(unused_must_use)]
            if !base_folder.exists() {
                std::fs::create_dir_all(base_folder);
            }

            File::create(&path)
        };

        // Download data
        match file {
            Ok(mut file) => {
                let downloading_chunk = self.downloading_chunk;
                let total = self.length().unwrap_or(0) as usize;

                let mut bytes = Vec::new();

                let downloader = move |data: &[u8]| {
                    curr += data.len();
                    bytes.extend_from_slice(data);

                    if bytes.len() >= downloading_chunk || total.checked_sub(curr).unwrap_or(0) <= downloading_chunk {
                        file.write_all(&bytes).expect("Failed to write data");

                        bytes.clear();
                    }

                    Ok(data.len())
                };

                // I sadly couldn't write it better as move |..| and progress have different types
                // and I cant' use them in if-else statement
                if curr > 0 {
                    self.download(downloader, move |c, t| progress(c + curr as u64, t))
                } else {
                    self.download(downloader, progress)
                }
            },
            Err(err) => Err(DownloadingError::OutputFileError(path, err.to_string()))
        }
    }
}
