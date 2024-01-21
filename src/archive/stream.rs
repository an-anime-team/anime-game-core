use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use stream_unpack::zip::structures::CompressionMethod;
use stream_unpack::zip::structures::central_directory::CentralDirectory;
use stream_unpack::zip::{read_cd, ZipUnpacker, DecoderError, ZipDecodedData, ZipPosition};

use crate::updater::UpdaterExt;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(#[from] flume::SendError<usize>),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to fetch data: {0}")]
    Minreq(#[from] minreq::Error),

    #[error("No Content-Length in response")]
    NoSize,

    #[error("Invalid Content-Length in response: {0}")]
    InvalidSize(#[from] std::num::ParseIntError),

    #[error("Failed to process central directory: {0}")]
    CentralDirectory(#[from] read_cd::CentralDirectoryReadError),

    #[error("This archive contains elements which currently cannot be stream unpacked")]
    UnsupportedArchive,

    #[error("Could not map position to archive volumes")]
    InvalidPosition,

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
    central_directory: CentralDirectory,
    is_cut: bool
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

        // None - no compression - is supported
        let is_supported = central_directory.headers_ref().iter()
            .flat_map(|h| &h.compression_method)
            .all(CompressionMethod::is_supported);

        if !is_supported {
            return Err(Error::UnsupportedArchive);
        }

        Ok(Self {
            archives,
            central_directory,
            is_cut
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

    pub fn stream_unpack(self, folder: impl AsRef<Path>, status_file: impl AsRef<str>) -> Result<StreamArchiveUpdater, Error> {
        let folder = folder.as_ref().to_owned();
        let status_file = status_file.as_ref().to_owned();

        let (send, recv) = flume::unbounded();

        Ok(StreamArchiveUpdater { 
            incrementer: recv,

            current: Cell::new(0),
            total: self.total_size(),

            status_updater_result: None,

            status_updater: Some(std::thread::spawn(move || -> Result<(), Error> {
                let sorted_cd = self.central_directory.sort();
                let disk_sizes = self.archives.iter().map(|a| a.size).collect::<Vec<_>>();
                let file = RefCell::new(None);

                let status_file_path = folder.join(status_file);
                let status_file_exists = status_file_path.exists();
                let status_file = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(&status_file_path)?;

                let (pos, mut unpacker) = if !status_file_exists {
                    let pos = sorted_cd.headers_ref()[0].header_position();
                    StreamArchiveUpdater::write_status_file(&status_file, pos)?;

                    (pos, ZipUnpacker::new(sorted_cd, disk_sizes.clone()))
                } else {
                    let pos = StreamArchiveUpdater::read_status_file(&status_file)?;

                    (pos, ZipUnpacker::resume(sorted_cd, disk_sizes.clone(), pos)?)
                };

                unpacker.set_callback(|data| {
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
                                    .truncate(true)
                                    .open(path)?
                                );
                            } else {
                                std::fs::create_dir_all(path)?;
                            }

                            StreamArchiveUpdater::write_status_file(&status_file, h.header_position())?;
                        },

                        ZipDecodedData::FileData(data) => {
                            file.borrow().as_ref().unwrap().write_all(data)?;
                        }
                    }

                    Ok(())
                });

                let start_pos = if !self.is_cut {
                    pos
                } else {
                    Self::offset_to_position(&disk_sizes, pos.offset)?
                };

                if start_pos != ZipPosition::default() {
                    let completed = disk_sizes.iter().take(start_pos.disk).sum::<usize>() + start_pos.offset;
                    send.send(completed)?;
                }

                let mut buf = Vec::with_capacity((1 << 16) + 4096);
                for (i, archive) in self.archives.iter().enumerate().skip(start_pos.disk) {
                    let mut request = minreq::get(&archive.uri);
                    if pos.disk == i && pos.offset != 0 {
                        request = request.with_header("range", format!("bytes={}-", pos.offset));
                    }
                    let response = request.send_lazy()?;

                    for byte in response {
                        let (byte, _) = byte?;

                        buf.push(byte);

                        if buf.len() >= (1 << 16) {
                            let (advanced, reached_end) = unpacker.update(&buf)?;

                            if advanced != 0 {
                                buf.drain(..advanced);
                                send.send(advanced)?;
                            }

                            if reached_end {
                                break;
                            }
                        }
                    }
                }

                if !buf.is_empty() {
                    unpacker.update(&buf)?;
                    send.send(buf.len())?;
                }

                drop(unpacker);

                drop(status_file);
                std::fs::remove_file(status_file_path)?;

                Ok(())
            }))
        })
    }

    pub fn offset_to_position(disk_sizes: impl AsRef<[usize]>, offset: usize) -> Result<ZipPosition, Error> {
        let disk_sizes = disk_sizes.as_ref();

        let mut left = offset;
        for (i, size) in disk_sizes.iter().enumerate() {
            if left < *size {
                return Ok(ZipPosition::new(i, left));
            } else {
                left -= *size;
            }
        }

        Err(Error::InvalidPosition)
    }
}

pub struct StreamArchiveUpdater {
    status_updater: Option<JoinHandle<Result<(), Error>>>,
    status_updater_result: Option<Result<(), Error>>,

    incrementer: flume::Receiver<usize>,

    current: Cell<usize>,
    total: usize
}

impl StreamArchiveUpdater {
    fn read_status_file(file: &std::fs::File) -> Result<ZipPosition, std::io::Error> {
        let mut bytes = [0u8; 12];
        file.read_exact_at(&mut bytes, 0)?;

        Ok(ZipPosition::new(
            u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize,
            u64::from_le_bytes(bytes[4..12].try_into().unwrap()) as usize
        ))
    }

    fn write_status_file(file: &std::fs::File, pos: ZipPosition) -> Result<(), std::io::Error> {
        file.write_all_at(&((pos.disk as u32).to_le_bytes()), 0)?;
        file.write_all_at(&((pos.offset as u64).to_le_bytes()), 4)?;

        Ok(())
    }
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
