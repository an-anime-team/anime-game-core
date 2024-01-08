use std::path::{Path, PathBuf};
use std::fs::File;
use std::cell::Cell;
use std::io::{Seek, Write};

use crate::updater::*;

use super::DownloaderExt;

// TODO: multi-thread Downloader implementation

/// Default downloading chunk size, in bytes
pub const DEFAULT_CHUNK_SIZE: u64 = 1024 * 16;

/// Default value for continue downloading option
pub const DEFAULT_CONTINUE_DOWNLOADING: bool = true;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(#[from] flume::SendError<((), u64, u64)>),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to fetch data: {0}")]
    Minreq(#[from] minreq::Error),

    #[error("Failed to create output file {0:?}: {1}")]
    /// Failed to create or open output file
    /// 
    /// `(path, error message)`
    OutputFileError(PathBuf, String),

    #[error("Failed to read metadata of the output file {0:?}: {1}")]
    /// Couldn't get metadata of existing output file
    /// 
    /// This metadata supposed to be used to continue downloading of the file
    /// 
    /// `(path, error message)`
    OutputFileMetadataError(PathBuf, String)
}

pub struct Downloader {
    uri: String,
    file_name: Cell<Option<String>>,
    content_size: Cell<Option<Option<u64>>>,

    chunk_size: u64,
    continue_downloading: bool
}

impl DownloaderExt for Downloader {
    type Error = Error;
    type Updater = BasicUpdater<(), (), Error>;

    #[inline]
    fn new(uri: impl AsRef<str>) -> Self {
        Self {
            uri: uri.as_ref().to_string(),
            file_name: Cell::new(None),
            content_size: Cell::new(None),

            chunk_size: DEFAULT_CHUNK_SIZE,
            continue_downloading: DEFAULT_CONTINUE_DOWNLOADING
        }
    }

    fn content_size(&self) -> Result<Option<u64>, Self::Error> {
        if let Some(content_size) = self.content_size.take() {
            self.content_size.set(Some(content_size));

            Ok(content_size)
        }

        else {
            let content_size = minreq::head(&self.uri)
                .send()?.headers.get("content-length")
                .map(|value| value.parse().ok())
                .unwrap_or(None);

            self.content_size.set(Some(content_size));

            Ok(content_size)
        }
    }

    fn file_name(&self) -> String {
        if let Some(file_name) = self.file_name.take() {
            self.file_name.set(Some(file_name.clone()));

            file_name
        }

        else {
            let file_name = self.uri
                .replace('\\', "/")
                .replace("://", "");

            let file_name = file_name
                .split('?').next()
                .and_then(|uri| uri.split('/')
                    .filter(|part| !part.is_empty())
                    .skip(1)
                    .last())
                .unwrap_or("index.html")
                .to_string();

            self.file_name.set(Some(file_name.clone()));

            file_name
        }
    }

    #[inline]
    fn continue_downloading(self, continue_downloading: bool) -> Self {
        Self {
            continue_downloading,
            ..self
        }
    }

    fn download(&self, download_path: impl AsRef<Path>) -> Result<Self::Updater, Self::Error> {
        let download_path = download_path.as_ref();

        let chunk_size = self.chunk_size as usize;
        let content_size = self.content_size()?;

        let mut downloaded = 0;

        // Open or create output file
        let file = if download_path.exists() && self.continue_downloading {
            tracing::debug!("Opening output file");

            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(download_path);

            // Continue downloading if the file exists and can be opened
            if let Ok(file) = &mut file {
                match file.metadata() {
                    Ok(metadata) => {
                        // Stop the process if the file is already downloaded
                        if let Some(length) = self.content_size()? {
                            match metadata.len().cmp(&length) {
                                std::cmp::Ordering::Less => (),

                                // Return finished updater
                                std::cmp::Ordering::Equal => return Ok(BasicUpdater::finished((), content_size.unwrap_or(1))),

                                // Trim downloaded file to prevent future issues (e.g. with extracting the archive)
                                std::cmp::Ordering::Greater => {
                                    if let Err(err) = file.set_len(length) {
                                        return Err(Self::Error::OutputFileError(download_path.to_path_buf(), err.to_string()));
                                    }

                                    // Return finished updater
                                    return Ok(BasicUpdater::finished((), content_size.unwrap_or(1)));
                                }
                            }
                        }

                        if let Err(err) = file.seek(std::io::SeekFrom::Start(metadata.len())) {
                            return Err(Self::Error::OutputFileError(download_path.to_path_buf(), err.to_string()));
                        }

                        downloaded = metadata.len();
                    }

                    Err(err) => return Err(Self::Error::OutputFileMetadataError(download_path.to_path_buf(), err.to_string()))
                }
            }

            file
        } else {
            tracing::debug!("Creating output file");

            let base_folder = download_path.parent().unwrap();

            if !base_folder.exists() {
                if let Err(err) = std::fs::create_dir_all(base_folder) {
                    return Err(Self::Error::OutputFileError(download_path.to_path_buf(), err.to_string()));
                }
            }

            File::create(download_path)
        };

        match file {
            Ok(mut file) => {
                let response = minreq::head(&self.uri)
                    .with_header("range", format!("bytes={downloaded}-"))
                    .send()?;

                // Request content range (downloaded + remained content size)
                // 
                // If finished or overcame: bytes */10611646760
                // If not finished: bytes 10611646759-10611646759/10611646760
                // 
                // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Range
                if let Some(range) = response.headers.get("content-range") {
                    // Finish downloading if header says that we've already downloaded all the data
                    if range.contains("*/") {
                        return Ok(BasicUpdater::finished((), content_size.unwrap_or(downloaded)));
                    }
                }

                let mut response = minreq::get(&self.uri)
                    .with_header("range", format!("bytes={downloaded}-"))
                    .send_lazy()?;

                // HTTP 416 = provided range is overcame actual content length (means file is downloaded)
                // I check this here because HEAD request can return 200 OK while GET - 416
                // 
                // https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/416
                if response.status_code == 416 {
                    return Ok(BasicUpdater::finished((), content_size.unwrap_or(downloaded)));
                }

                Ok(BasicUpdater::spawn(|updater| {
                    Box::new(move || -> Result<(), Self::Error> {
                        let mut buffer = vec![0; chunk_size];
                        let mut i = 0;

                        while let Some(Ok((byte, _))) = response.next() {
                            buffer[i] = byte;

                            i += 1;
                            downloaded += 1;

                            if i == chunk_size {
                                file.write_all(&buffer)?;

                                let total = response.size_hint();
                                let total = content_size.unwrap_or(total.1.unwrap_or(total.0) as u64);

                                updater.send((
                                    (),
                                    downloaded,
                                    total
                                ))?;

                                i = 0;
                            }
                        }

                        file.write_all(&buffer[..i])?;

                        let total = response.size_hint();
                        let total = content_size.unwrap_or(total.1.unwrap_or(total.0) as u64);

                        updater.send((
                            (),
                            downloaded,
                            total
                        ))?;

                        Ok(())
                    })
                }))
            }

            Err(err) => Err(Self::Error::OutputFileError(download_path.to_path_buf(), err.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name() -> Result<(), Error> {
        assert_eq!(Downloader::new("https://example.com").file_name(), "index.html");
        assert_eq!(Downloader::new("https://example.com/").file_name(), "index.html");
        assert_eq!(Downloader::new("https://example.com\\").file_name(), "index.html");
        assert_eq!(Downloader::new("https://example.com/?example=123").file_name(), "index.html");

        assert_eq!(Downloader::new("https://example.com/example.zip").file_name(), "example.zip");
        assert_eq!(Downloader::new("https://example.com/example.zip/").file_name(), "example.zip");
        assert_eq!(Downloader::new("https://example.com/example.zip\\").file_name(), "example.zip");

        assert_eq!(Downloader::new("https://example.com/example.zip/?token=example").file_name(), "example.zip");

        assert_eq!(
            Downloader::new("https://github.com/GloriousEggroll/wine-ge-custom/releases/download/GE-Proton8-13/wine-lutris-GE-Proton8-13-x86_64.tar.xz").file_name(),
            "wine-lutris-GE-Proton8-13-x86_64.tar.xz"
        );

        Ok(())
    }

    #[test]
    fn content_size() -> Result<(), Error> {
        assert_eq!(
            Downloader::new("https://github.com/doitsujin/dxvk/releases/download/v2.2/dxvk-2.2.tar.gz").content_size()?,
            Some(7935306)
        );

        Ok(())
    }

    #[test]
    fn download_file() -> Result<(), Error> {
        let updater = Downloader::new("https://github.com/doitsujin/dxvk/releases/download/v2.2/dxvk-2.2.tar.gz")
            .download("dxvk-2.2.tar.gz")?;

        updater.wait()?;

        assert_eq!(format!("{}", blake3::hash(&std::fs::read("dxvk-2.2.tar.gz")?)), "42e236e952d6ed3e8537ea359ae98a0fefb7ffd502d06865dd32f2c0976d4da2");

        std::fs::remove_file("dxvk-2.2.tar.gz")?;

        Ok(())
    }

    #[test]
    fn continue_file_downloading() -> Result<(), Error> {
        let mut updater = Downloader::new("https://github.com/doitsujin/dxvk/releases/download/v2.2/dxvk-2.2.tar.gz")
            .continue_downloading(true)
            .download("dxvk-2.2.tar.gz")?;

        while !updater.is_finished() {
            if updater.progress() >= 0.5 {
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        std::fs::copy("dxvk-2.2.tar.gz", "dxvk-2.2.tar.gz.part")?;

        assert!(format!("{}", blake3::hash(&std::fs::read("dxvk-2.2.tar.gz.part")?)) != "42e236e952d6ed3e8537ea359ae98a0fefb7ffd502d06865dd32f2c0976d4da2");

        updater.wait()?;

        assert_eq!(format!("{}", blake3::hash(&std::fs::read("dxvk-2.2.tar.gz")?)), "42e236e952d6ed3e8537ea359ae98a0fefb7ffd502d06865dd32f2c0976d4da2");

        std::fs::remove_file("dxvk-2.2.tar.gz")?;

        let updater = Downloader::new("https://github.com/doitsujin/dxvk/releases/download/v2.2/dxvk-2.2.tar.gz")
            .continue_downloading(true)
            .download("dxvk-2.2.tar.gz.part")?;

        updater.wait()?;

        assert_eq!(format!("{}", blake3::hash(&std::fs::read("dxvk-2.2.tar.gz.part")?)), "42e236e952d6ed3e8537ea359ae98a0fefb7ffd502d06865dd32f2c0976d4da2");

        std::fs::remove_file("dxvk-2.2.tar.gz.part")?;

        Ok(())
    }
}
