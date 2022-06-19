use std::path::Path;
use std::fs::File;

use zip::ZipArchive;
use tar::Archive as TarArchive;
use xz::read::XzDecoder as XzReader;
use bzip2::read::BzDecoder as Bz2Reader;
use flate2::read::GzDecoder as GzReader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Compressed(u64),
    Uncompressed(u64),
    Both {
        compressed: u64,
        uncompressed: u64
    }
}

impl Size {
    pub fn get_size(&self) -> u64 {
        match self {
            Size::Compressed(size) => *size,
            Size::Uncompressed(size) => *size,
            Size::Both { compressed, uncompressed: _ } => *compressed
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub size: Size
}

pub enum Archive {
    Zip(String, ZipArchive<File>),
    Tar(String, TarArchive<File>),
    TarXz(String, TarArchive<XzReader<File>>),
    TarGz(String, TarArchive<GzReader<File>>),
    TarBz2(String, TarArchive<Bz2Reader<File>>)
}

impl Archive {
    pub fn open<T: ToString>(path: T) -> Option<Self> {
        match File::open(Path::new(path.to_string().as_str())) {
            Ok(file) => {
                if let Ok(zip) = ZipArchive::new(file) {
                    Some(Archive::Zip(path.to_string(), zip))
                }

                else {
                    let mut tar = TarArchive::new(File::open(Path::new(path.to_string().as_str())).unwrap());

                    if let Ok(_) = tar.entries() {
                        let path = path.to_string();
                        let file = File::open(Path::new(path.to_string().as_str())).unwrap();

                        if &path[path.len() - 7..] == ".tar.xz" {
                            Some(Archive::TarXz(path.to_string(), TarArchive::new(XzReader::new(file))))
                        }

                        else if &path[path.len() - 7..] == ".tar.gz" {
                            Some(Archive::TarGz(path.to_string(), TarArchive::new(GzReader::new(file))))
                        }

                        else if &path[path.len() - 8..] == ".tar.bz2" {
                            Some(Archive::TarBz2(path.to_string(), TarArchive::new(Bz2Reader::new(file))))
                        }

                        else {
                            Some(Archive::Tar(path.to_string(), tar))
                        }
                    }
                    
                    else {
                        None
                    }
                }
            },
            Err(_) => None
        }
    }

    pub fn get_entries(&mut self) -> Vec<Entry> {
        let mut entries = Vec::new();

        match self {
            Archive::Zip(_, zip) => {
                for i in 0..zip.len() {
                    let entry = zip.by_index(i).unwrap();

                    entries.push(Entry {
                        name: entry.name().to_string(),
                        size: Size::Both {
                            compressed: entry.compressed_size(),
                            uncompressed: entry.size()
                        }
                    });
                }
            },
            Archive::Tar(_, tar) => {
                for entry in tar.entries().unwrap() {
                    if let Ok(entry) = entry {
                        entries.push(Entry {
                            name: entry.path().unwrap().to_str().unwrap().to_string(),
                            size: Size::Compressed(entry.size())
                        });
                    }
                }
            },
            Archive::TarXz(_, tar) => {
                for entry in tar.entries().unwrap() {
                    if let Ok(entry) = entry {
                        entries.push(Entry {
                            name: entry.path().unwrap().to_str().unwrap().to_string(),
                            size: Size::Compressed(entry.size())
                        });
                    }
                }
            },
            Archive::TarGz(_, tar) => {
                for entry in tar.entries().unwrap() {
                    if let Ok(entry) = entry {
                        entries.push(Entry {
                            name: entry.path().unwrap().to_str().unwrap().to_string(),
                            size: Size::Compressed(entry.size())
                        });
                    }
                }
            },
            Archive::TarBz2(_, tar) => {
                for entry in tar.entries().unwrap() {
                    if let Ok(entry) = entry {
                        entries.push(Entry {
                            name: entry.path().unwrap().to_str().unwrap().to_string(),
                            size: Size::Compressed(entry.size())
                        });
                    }
                }
            }
        }

        entries
    }

    // TODO: progress callback
    // TODO: errors
    pub fn extract<T: ToString>(&mut self, folder: T) {
        match self {
            Archive::Zip(_, zip) => {
                zip.extract(folder.to_string()).expect("Failed to extract zip archive");
            },
            Archive::Tar(_, tar) => {
                tar.unpack(folder.to_string()).expect("Failed to extract tar archive");
            },
            Archive::TarXz(_, tar) => {
                tar.unpack(folder.to_string()).expect("Failed to extract tar xz archive");
            },
            Archive::TarGz(_, tar) => {
                tar.unpack(folder.to_string()).expect("Failed to extract tar gz archive");
            },
            Archive::TarBz2(_, tar) => {
                tar.unpack(folder.to_string()).expect("Failed to extract tar bz2 archive");
            }
        }
    }
}
