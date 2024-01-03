use std::path::Path;
use std::fs::File;
use std::cell::Cell;

use std::io::Write;

use crate::updater::*;

use super::DownloaderExt;

// TODO: multi-thread Downloader implementation

/// Default downloading chunk size, in bytes
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 16;

/// Default value for continue downloading option
pub const DEFAULT_CONTINUE_DOWNLOADING: bool = false;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(#[from] flume::SendError<((), u64, u64)>),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to fetch data: {0}")]
    Minreq(#[from] minreq::Error)
}

pub struct Downloader {
    uri: String,
    file_name: Cell<Option<String>>,
    content_size: Cell<Option<Option<usize>>>,

    chunk_size: usize,
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

    fn content_size(&self) -> Result<Option<usize>, Self::Error> {
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
        let mut response = minreq::get(&self.uri).send_lazy()?;

        // TODO: respect continue downloading option
        let mut file = File::create(download_path.as_ref())?;

        let chunk_size = self.chunk_size;
        let content_size = self.content_size()?;

        Ok(BasicUpdater::spawn(|updater| {
            Box::new(move || -> Result<(), Self::Error> {
                let mut buffer = vec![0; chunk_size];
                let mut i = 0;
                let mut j = 0u64;

                while let Some(Ok((byte, _))) = response.next() {
                    buffer[i] = byte;

                    i += 1;
                    j += 1;

                    if i == chunk_size {
                        file.write_all(&buffer)?;

                        let total = response.size_hint();
                        let total = content_size.unwrap_or(total.1.unwrap_or(total.0)) as u64;

                        updater.send((
                            (),
                            j,
                            total
                        ))?;

                        i = 0;
                    }
                }

                file.write_all(&buffer[..i])?;

                let total = response.size_hint();
                let total = content_size.unwrap_or(total.1.unwrap_or(total.0)) as u64;

                updater.send((
                    (),
                    j,
                    total
                ))?;

                Ok(())
            })
        }))

        // Ok(Updater {
        //     worker_result: None,
        //     updates_receiver,

        //     current_progress: Cell::new(0), // TODO: downloaded content size
        //     content_size_hint: Cell::new(response.size_hint()),

        //     content_size: self.content_size()?,
        //     // download_path: download_path.as_ref().to_path_buf(),

        //     worker: Some(std::thread::spawn())
        // })
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
        let mut updater = Downloader::new("https://github.com/doitsujin/dxvk/releases/download/v2.2/dxvk-2.2.tar.gz")
            .download("dxvk-2.2.tar.gz")?;

        while updater.is_finished() {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        updater.wait()?;

        assert_eq!(format!("{}", blake3::hash(&std::fs::read("dxvk-2.2.tar.gz")?)), "42e236e952d6ed3e8537ea359ae98a0fefb7ffd502d06865dd32f2c0976d4da2");

        std::fs::remove_file("dxvk-2.2.tar.gz")?;

        Ok(())
    }
}
