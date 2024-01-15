use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use stream_unpack::zip::structures::CompressionMethod;
use stream_unpack::zip::structures::central_directory::CentralDirectory;
use stream_unpack::zip::{read_cd, ZipUnpacker, DecoderError, ZipDecodedData};

use crate::updater::UpdaterExt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to fetch data: {0}")]
    Minreq(#[from] minreq::Error),

    #[error("No Content-Length in response")]
    NoSize,

    #[error("Invalid Content-Length in response: {0}")]
    InvalidSize(#[from] std::num::ParseIntError),

    #[error("Failed to process central directory: {0}")]
    CentralDirectory(#[from] read_cd::CentralDirectoryReadError),

    #[error("Decoder error: {0}")]
    Decoder(#[from] DecoderError)
}

struct StreamArchiveDisk {
    uri: String,
    size: usize
}

impl StreamArchiveDisk {
    fn from_uri(uri: impl AsRef<str>) -> Result<Self, Error> {
        let uri = uri.as_ref();

        let size = minreq::head(uri)
            .send()?.headers.get("content-length")
            .ok_or(Error::NoSize)?
            .parse()?;

        Ok(Self {
            uri: uri.to_owned(),
            size
        })
    }
}

pub struct StreamArchive {
    archives: Vec<StreamArchiveDisk>,
    central_directory: CentralDirectory
}

impl StreamArchive {
    pub fn from_uris(uris: &[&str], is_cut: bool) -> Result<Self, Error> {
        let archives = uris.iter()
            .map(StreamArchiveDisk::from_uri)
            .collect::<Result<Vec<_>, _>>()?;

        let central_directory = read_cd::from_provider(
            archives.iter().map(|d| d.size).collect::<Vec<_>>(), 
            is_cut, 
            |pos, length| {
                let start = pos.offset;
                let end = start + length - 1;

                let bytes = minreq::get(&archives[pos.disk].uri)
                    .with_header("range", format!("bytes={start}-{end}"))
                    .send()?.into_bytes();

                Ok(bytes)
            }
        )?;

        Ok(Self {
            archives,
            central_directory
        })
    }

    pub fn from_uri(uri: &str) -> Result<Self, Error> {
        Self::from_uris(&[uri], false)
    }

    pub fn total_size(&self) -> usize {
        self.archives.iter()
            .map(|a| a.size)
            .sum()
    }

    pub fn uncompressed_size(&self) -> usize {
        self.central_directory.headers_ref().iter()
            .map(|h| h.uncompressed_size as usize)
            .sum()
    }

    pub fn can_stream_unpack(&self) -> bool {
        // None - no compression - is supported
        self.central_directory.headers_ref().iter()
            .flat_map(|h| &h.compression_method)
            .all(CompressionMethod::is_supported)
    }

    pub fn stream_unpack(self, folder: impl AsRef<Path>) -> Result<StreamArchiveUpdater, Error> {
        let folder = folder.as_ref().to_owned();

        let (send, recv) = flume::unbounded();

        Ok(StreamArchiveUpdater { 
            incrementer: recv,

            current: Cell::new(0),
            total: self.uncompressed_size(),

            status_updater_result: None,

            status_updater: Some(std::thread::spawn(move || -> Result<(), Error> {
                let file = RefCell::new(None);

                let mut unpacker = ZipUnpacker::new(
                    self.central_directory.sort(), 
                    self.archives.iter().map(|a| a.size).collect(), 
                    |data| {
                        match data {
                            ZipDecodedData::FileHeader(h, _) => {
                                let mut path = PathBuf::from(&folder);
                                path.push(&h.filename);

                                if !h.is_directory() {
                                    std::fs::create_dir_all(path.parent().unwrap())?;

                                    *file.borrow_mut() = Some(
                                        OpenOptions::new()
                                        .create(true)
                                        .write(true)
                                        .open(path)?
                                    );
                                } else {
                                    std::fs::create_dir_all(path)?;
                                }
                            },

                            ZipDecodedData::FileData(data) => {
                                file.borrow().as_ref().unwrap().write_all(data)?;

                                send.send(data.len())?;
                            }
                        }

                        Ok(())
                    }
                );

                let mut buf = Vec::with_capacity((1 << 16) + 4096);
                for archive in self.archives {
                    let response = minreq::get(archive.uri).send_lazy()?;

                    for byte in response {
                        let (byte, _) = byte?;

                        buf.push(byte);

                        if buf.len() >= (1 << 16) {
                            let (advanced, reached_end) = unpacker.update(&buf)?;
                            buf.drain(..advanced);

                            if reached_end {
                                break;
                            }
                        }
                    }
                }

                if !buf.is_empty() {
                    unpacker.update(buf)?;
                }

                todo!()
            }))
        })
    }
}

pub struct StreamArchiveUpdater {
    status_updater: Option<JoinHandle<Result<(), Error>>>,
    status_updater_result: Option<Result<(), Error>>,

    incrementer: flume::Receiver<usize>,

    current: Cell<usize>,
    total: usize
}


impl UpdaterExt for StreamArchiveUpdater {
    type Error = Error;
    type Status = bool;
    type Result = ();

    #[inline]
    fn status(&mut self) -> Result<Self::Status, &Self::Error> {
        if let Some(status_updater) = self.status_updater.take() {
            if !status_updater.is_finished() {
                self.status_updater = Some(status_updater);

                return Ok(false);
            }

            self.status_updater_result = Some(status_updater.join().expect("Failed to join thread"));
        }

        match &self.status_updater_result {
            Some(Ok(_)) => Ok(true),
            Some(Err(err)) => Err(err),

            None => unreachable!()
        }
    }

    #[inline]
    fn wait(mut self) -> Result<Self::Result, Self::Error> {
        if let Some(worker) = self.status_updater.take() {
            return worker.join().expect("Failed to join thread");
        }

        else if let Some(result) = self.status_updater_result.take() {
            return result;
        }

        unreachable!()
    }

    #[inline]
    fn is_finished(&mut self) -> bool {
        matches!(self.status(), Ok(true) | Err(_))
    }

    #[inline]
    fn current(&self) -> u64 {
        let mut current = self.current.get();

        while let Ok(increment) = self.incrementer.try_recv() {
            current += increment;
        }

        self.current.set(current);

        current as u64
    }

    #[inline]
    fn total(&self) -> u64 {
        self.total as u64
    }
}
