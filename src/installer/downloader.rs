use std::io::Write;
use std::path::Path;
use std::fs::File;

use curl::easy::Easy;

#[derive(Debug)]
pub struct Downloader {
    length: Option<u64>,
    curl: Easy
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
                    curl
                });
            }
        }

        else if let Ok(length) = curl.download_size() {
            if length >= 0.0 {
                return Ok(Self {
                    length: Some(length.ceil() as u64),
                    curl
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
            curl
        })
    }

    /// Try to get content length
    pub fn try_get_length(&self) -> Option<u64> {
        self.length
    }

    // TODO: verify available free space before starting downloading
    // TODO: somehow use FnOnce instead of Fn

    pub fn download<Fd, Fp>(&mut self, downloader: Fd, progress: Fp) -> Result<(), curl::Error>
    where
        // array of bytes
        Fd: Fn(&[u8]) -> Result<usize, curl::easy::WriteError> + Send + 'static,
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
                let (send, recv) = std::sync::mpsc::channel::<Vec<u8>>();

                // TODO: downloading speed may be higher than disk writing speed, so
                //       this thread can store lots of data in memory before pushing it
                //       to the disk which can lead to some issues
                //       it's better to somehow pause downloading if the queue is full

                std::thread::spawn(move || {
                    while let Ok(data) = recv.recv_timeout(std::time::Duration::from_secs(5)) {
                        file.write(&data);
                    }

                    file.flush();
                });
        
                self.download(move |data| {
                    send.send(data.to_vec());

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
