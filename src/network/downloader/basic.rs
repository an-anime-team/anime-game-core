use std::path::{Path, PathBuf};
use std::fs::File;
use std::thread::JoinHandle;
use std::cell::Cell;

use std::io::Write;

use super::{
    DownloaderExt,
    UpdaterExt
};

// TODO: multi-thread Downloader implementation

/// Default downloading chunk size, in bytes
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 16;

/// Default value for continue downloading option
pub const DEFAULT_CONTINUE_DOWNLOADING: bool = false;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
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
    type Updater = Updater<Error>;

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

        let (updates_sender, updates_receiver) = flume::unbounded();

        Ok(Updater {
            worker_result: None,
            updates_receiver,

            current_progress: Cell::new(0), // TODO: downloaded content size
            content_size_hint: Cell::new(response.size_hint()),

            content_size: self.content_size()?,
            // download_path: download_path.as_ref().to_path_buf(),

            worker: Some(std::thread::spawn(move || -> Result<(), Self::Error> {
                let mut buffer = vec![0; chunk_size];
                let mut i = 0;

                while let Some(Ok((byte, _))) = response.next() {
                    buffer[i] = byte;

                    i += 1;

                    if i == chunk_size {
                        file.write_all(&buffer)?;

                        if let Err(err) = updates_sender.send((i, response.size_hint())) {
                            return Err(Error::FlumeSendError(err.to_string()));
                        }

                        i = 0;
                    }
                }

                file.write_all(&buffer[..i])?;

                if let Err(err) = updates_sender.send((i, response.size_hint())) {
                    return Err(Error::FlumeSendError(err.to_string()));
                }

                Ok(())
            }))
        })
    }
}

pub struct Updater<Error> {
    worker: Option<JoinHandle<Result<(), Error>>>,
    worker_result: Option<Result<(), Error>>,
    updates_receiver: flume::Receiver<(usize, (usize, Option<usize>))>,

    current_progress: Cell<usize>,
    content_size_hint: Cell<(usize, Option<usize>)>,

    content_size: Option<usize>,
    // download_path: PathBuf
}

impl<Error> Updater<Error> {
    fn update(&self) {
        if let Ok((downloaded, content_size_hint)) = self.updates_receiver.recv() {
            self.current_progress.set(self.current_progress.take() + downloaded);
            self.content_size_hint.set(content_size_hint);
        }
    }
}

impl<Error> UpdaterExt<Error> for Updater<Error> {
    fn status(&mut self) -> Result<bool, &Error> {
        self.update();

        if let Some(worker) = self.worker.take() {
            if !worker.is_finished() {
                self.worker = Some(worker);

                return Ok(false);
            }

            self.worker_result = Some(worker.join().expect("Failed to join downloader thread"));
        }

        match &self.worker_result {
            Some(Ok(_)) => Ok(true),
            Some(Err(err)) => Err(err),

            None => unreachable!()
        }
    }

    fn wait(mut self) -> Result<(), Error> {
        if let Some(worker) = self.worker.take() {
            return worker.join().expect("Failed to join downloader thread");
        }

        else if let Some(result) = self.worker_result.take() {
            return result;
        }

        unreachable!()
    }

    #[inline]
    fn current_size(&self) -> usize {
        // self.download_path.exists()
        //     .then(|| self.download_path.metadata()
        //         .map(|metadata| metadata.len())
        //         .unwrap_or(0))
        //     .unwrap_or(0)

        self.update();

        self.current_progress.get()
    }

    #[inline]
    fn total_size(&self) -> usize {
        self.update();

        let size_hint = self.content_size_hint.get();

        self.content_size.unwrap_or(size_hint.1.unwrap_or(size_hint.0))
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

        while let Ok(false) = updater.status() {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        updater.wait()?;

        assert_eq!(format!("{}", blake3::hash(&std::fs::read("dxvk-2.2.tar.gz")?)), "42e236e952d6ed3e8537ea359ae98a0fefb7ffd502d06865dd32f2c0976d4da2");

        std::fs::remove_file("dxvk-2.2.tar.gz")?;

        Ok(())
    }
}
