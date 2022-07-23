use std::io::Write;
use std::path::Path;
use std::fs::File;

use curl::easy::Easy;

/// Default amount of bytes `Downloader::download_to` method will store in memory
/// before writing them onto the disk
pub const DEFAULT_DOWNLOADING_CHUNK: usize = 64 * 1024 * 1024;

#[derive(Debug)]
pub struct Downloader {
    length: Option<u64>,
    curl: Easy,

    /// Amount of bytes `download_to` method will store in memory before
    /// writing them onto the disk. Uses `DEFAULT_DOWNLOADING_CHUNK` value
    /// by default
    pub downloading_chunk: usize
}

impl Downloader {
    /// Try to open downloading stream
    /// 
    /// Will return `Error` if the URL is not valid
    pub fn new<T: ToString>(uri: T) -> Result<Self, curl::Error> {
        let mut curl = Easy::new();

        curl.url(&uri.to_string())?;

        curl.follow_location(true)?;
        curl.progress(true)?;

        curl.nobody(true)?;

        if let Ok(length) = curl.content_length_download() {
            if length >= 0.0 {
                return Ok(Self {
                    length: Some(length.ceil() as u64),
                    curl,
                    downloading_chunk: DEFAULT_DOWNLOADING_CHUNK
                });
            }
        }

        else if let Ok(length) = curl.download_size() {
            if length >= 0.0 {
                return Ok(Self {
                    length: Some(length.ceil() as u64),
                    curl,
                    downloading_chunk: DEFAULT_DOWNLOADING_CHUNK
                });
            }
        }
        
        let (send, recv) = std::sync::mpsc::channel::<u64>();

        curl.header_function(move |header| {
            let header = String::from_utf8_lossy(header);

            // Content-Length: 8899
            if header.len() > 16 && &header[..16] == "Content-Length: " {
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
            curl,
            downloading_chunk: DEFAULT_DOWNLOADING_CHUNK
        })
    }

    /// Get content length
    pub fn length(&self) -> Option<u64> {
        self.length
    }

    /// Set downloading chunk size
    pub fn set_downloading_chunk(mut self, size: usize) -> Self {
        self.downloading_chunk = size;

        self
    }

    // TODO: verify available free space before starting downloading
    // TODO: somehow use FnOnce instead of Fn

    pub fn download<Fd, Fp>(&mut self, mut downloader: Fd, progress: Fp) -> Result<(), curl::Error>
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

        self.curl.progress_function(move |expected_total, downloaded, _, _| {
            (progress)(downloaded.ceil() as u64, content_length.unwrap_or(expected_total.ceil() as u64));

            true
        })?;

        self.curl.perform()
    }

    pub fn download_to<T, Fp>(&mut self, path: T, progress: Fp) -> Result<(), curl::Error>
    where
        T: ToString,
        // (curr, total)
        Fp: Fn(u64, u64) + Send + 'static
    {
        let path = path.to_string();

        match File::create(Path::new(path.as_str())) {
            Ok(mut file) => {
                let downloading_chunk = self.downloading_chunk;
                let total = self.length().unwrap_or(0) as usize;

                let mut bytes = Vec::new();
                let mut curr = 0_usize;

                self.download(move |data| {
                    curr += data.len();
                    bytes.extend_from_slice(data);

                    // FIXME: (0_usize - x_usize), where x > 0, will cause usize overflow and panic
                    if bytes.len() >= downloading_chunk || total - curr <= downloading_chunk {
                        file.write_all(&bytes).expect("Failed to write data");

                        bytes.clear();
                    }

                    Ok(data.len())
                }, progress)
            },
            Err(_) => {
                // FIXME
                panic!("Failed to create output file");
            }
        }
    }
}
