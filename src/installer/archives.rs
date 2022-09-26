use std::path::PathBuf;
use std::fs::File;
use std::process::{Command, Stdio};

use zip::ZipArchive;
use tar::Archive as TarArchive;
// use sevenz_rust::SevenZReader as SevenzArchive;

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
    Zip(PathBuf, ZipArchive<File>),
    Tar(PathBuf, TarArchive<File>),
    TarXz(PathBuf, TarArchive<XzReader<File>>),
    TarGz(PathBuf, TarArchive<GzReader<File>>),
    TarBz2(PathBuf, TarArchive<Bz2Reader<File>>),
    SevenZ(PathBuf/*, SevenzArchive<File>*/)
}

impl Archive {
    pub fn open<T: Into<PathBuf>>(path: T) -> anyhow::Result<Self> {
        let path: PathBuf = path.into();
        let file = File::open(&path)?;

        let path_str = path.to_string_lossy();

        if &path_str[path_str.len() - 4..] == ".zip" {
            Ok(Archive::Zip(path, ZipArchive::new(file)?))
        }

        else if &path_str[path_str.len() - 7..] == ".tar.xz" {
            Ok(Archive::TarXz(path, TarArchive::new(XzReader::new(file))))
        }

        else if &path_str[path_str.len() - 7..] == ".tar.gz" {
            Ok(Archive::TarGz(path, TarArchive::new(GzReader::new(file))))
        }

        else if &path_str[path_str.len() - 8..] == ".tar.bz2" {
            Ok(Archive::TarBz2(path, TarArchive::new(Bz2Reader::new(file))))
        }

        else if &path_str[path_str.len() - 3..] == ".7z" {
            Ok(Archive::SevenZ(path.clone()/*, SevenzArchive::open(path, &[])?*/))
        }

        else if &path_str[path_str.len() - 4..] == ".tar" {
            Ok(Archive::Tar(path, TarArchive::new(file)))
        }

        else {
            Err(anyhow::anyhow!("Archive format is not supported: {}", path.to_string_lossy()))
        }
    }

    /// Tar archives may forbid you to extract them if you call this method
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
            }

            Archive::Tar(_, tar) => {
                for entry in tar.entries().unwrap() {
                    if let Ok(entry) = entry {
                        entries.push(Entry {
                            name: entry.path().unwrap().to_str().unwrap().to_string(),
                            size: Size::Compressed(entry.size())
                        });
                    }
                }
            }

            Archive::TarXz(_, tar) => {
                for entry in tar.entries().unwrap() {
                    if let Ok(entry) = entry {
                        entries.push(Entry {
                            name: entry.path().unwrap().to_str().unwrap().to_string(),
                            size: Size::Compressed(entry.size())
                        });
                    }
                }
            }

            Archive::TarGz(_, tar) => {
                for entry in tar.entries().unwrap() {
                    if let Ok(entry) = entry {
                        entries.push(Entry {
                            name: entry.path().unwrap().to_str().unwrap().to_string(),
                            size: Size::Compressed(entry.size())
                        });
                    }
                }
            }

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

            #[allow(unused_must_use)]
            Archive::SevenZ(path) => {
                /*let (send, recv) = std::sync::mpsc::channel();

                sz.for_each_entries(move |entry, _| {
                    send.send(Entry {
                        name: entry.name.clone(),
                        size: Size::Both {
                            compressed: entry.compressed_size,
                            uncompressed: entry.size
                        }
                    });

                    Ok(true)
                });

                while let Ok(entry) = recv.recv() {
                    entries.push(entry);
                }*/

                let output = Command::new("7z")
                    .arg("l")
                    .arg(path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output()
                    .unwrap();

                let output = String::from_utf8(output.stdout).unwrap();

                let output = output.split("-------------------").collect::<Vec<&str>>();
                let output = output[1..output.len() - 1].join("-------------------");

                for line in output.split("\n").collect::<Vec<&str>>() {
                    if &line[..1] != "-" && &line[..2] != " -" {
                        let words = line.split("  ").filter_map(|word| {
                            let word = word.trim();

                            if word == "" {
                                None
                            } else {
                                Some(word)
                            }
                        }).collect::<Vec<&str>>();

                        entries.push(Entry {
                            name: words[words.len() - 1].to_string(),
                            size: Size::Uncompressed(words[1].parse::<u64>().unwrap())
                        });
                    }
                }
            }
        }

        entries
    }

    pub fn extract<T: Into<PathBuf>>(&mut self, folder: T) -> anyhow::Result<()> {
        match self {
            Archive::Zip(_, zip) => {
                zip.extract(folder.into())?;
            }

            Archive::Tar(_, tar) => {
                tar.unpack(folder.into())?;
            }

            Archive::TarXz(_, tar) => {
                tar.unpack(folder.into())?;
            }

            Archive::TarGz(_, tar) => {
                tar.unpack(folder.into())?;
            }

            Archive::TarBz2(_, tar) => {
                tar.unpack(folder.into())?;
            }

            Archive::SevenZ(archive) => {
                // sevenz_rust::decompress_file(archive, folder.into())?;

                Command::new("7z")
                    .arg("x")
                    .arg(archive)
                    .arg(format!("-o{:?}", folder.into()))
                    .arg("-aoa")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .output()?;
            }
        }

        Ok(())
    }
}
