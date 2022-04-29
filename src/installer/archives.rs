use std::process::{Command, Stdio};
use std::time::{Instant, Duration};
use std::path::Path;

#[derive(Debug)]
pub enum StreamUpdate {
    Start(ArchiveInfo),

    /// Unpacked file, current progress, total progress
    Unpacked(ArchiveEntry, u32, u32),

    /// Successfully unpacked if `Finish(Ok(ExitStatus(unix_wait_status(0))))`
    Finish(Result<std::process::ExitStatus, std::io::Error>)
}

pub struct Stream {
    archive: Archive,
    on_update: Box<dyn Fn(StreamUpdate)>,
    pub unpack_progress_interval: Duration
}

impl Stream {
    pub fn open(path: String) -> Option<Stream> {
        match Archive::open(path) {
            Some(archive) => Some(Stream {
                archive,
                on_update: Box::new(|_| {}),
                unpack_progress_interval: Duration::from_millis(10)
            }),
            None => None
        }
    }

    pub fn from_archive(archive: Archive) -> Stream {
        Self {
            archive,
            on_update: Box::new(|_| {}),
            unpack_progress_interval: Duration::from_millis(10)
        }
    }

    pub fn on_update<T: Fn(StreamUpdate) + 'static>(&mut self, callback: T) {
        self.on_update = Box::new(callback);
    }

    pub fn unpack<T: ToString>(&self, to: T) -> Option<Duration> {
        match self.archive.get_info() {
            Some(info) => {
                let child = match info.r#type {
                    ArchiveType::Zip => {
                        Command::new("unzip")
                            .arg("-o")
                            .arg(self.archive.get_path())
                            .arg("-d")
                            .arg(to.to_string())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
        
                    },
                    ArchiveType::Tar => {
                        Command::new("tar")
                            .arg("-xvf")
                            .arg(self.archive.get_path())
                            .arg("-C")
                            .arg(to.to_string())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn()
                    }
                };
        
                match child {
                    Ok(mut child) => {
                        (self.on_update)(StreamUpdate::Start(info.clone()));

                        let instant = Instant::now();

                        let mut remained = info.files.clone();
                        let mut progress = 0u32;

                        while let Ok(None) = child.try_wait() {
                            let mut new_remained = Vec::new();

                            for entry in remained {
                                if Path::new(format!("{}/{}", to.to_string(), entry.path).as_str()).exists() {
                                    progress += entry.size.size();

                                    (self.on_update)(StreamUpdate::Unpacked(entry, progress, info.size.size()));
                                }

                                else {
                                    new_remained.push(entry);
                                }
                            }

                            remained = new_remained;

                            std::thread::sleep(self.unpack_progress_interval);
                        }

                        (self.on_update)(StreamUpdate::Finish(child.wait()));

                        Some(instant.elapsed())
                    },
                    Err(_) => None
                }
            },
            None => None
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ArchiveType {
    Zip,
    Tar
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntrySize {
    Compressed(u32),
    Uncompressed(u32),
    Both {
        compressed: u32,
        uncompressed: u32
    }
}

impl EntrySize {
    /// Returns compressed / uncompressed size, or compressed if both available
    pub fn size(&self) -> u32 {
        match self {
            EntrySize::Compressed(size) => size.clone(),
            EntrySize::Uncompressed(size) => size.clone(),
            EntrySize::Both { compressed, uncompressed: _ } => compressed.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: EntrySize
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveInfo {
    pub path: String,
    pub r#type: ArchiveType,
    pub size: EntrySize,
    pub files: Vec<ArchiveEntry>
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    path: String,
    archive_type: ArchiveType
}

impl Archive {
    pub fn open<T: ToString>(path: T) -> Option<Archive> {
        let path = path.to_string();

        if path.len() > 4 && &path[path.len() - 4..] == ".zip" {
            Some(Archive {
                path,
                archive_type: ArchiveType::Zip
            })
        }

        else if path.len() > 7 && &path[path.len() - 7..path.len() - 2] == ".tar." {
            Some(Archive {
                path,
                archive_type: ArchiveType::Tar
            })
        }
        
        else {
            None
        }
    }

    pub fn get_type(&self) -> ArchiveType {
        self.archive_type
    }

    pub fn get_path(&self) -> &str {
        self.path.as_str()
    }

    // TODO: add tests
    pub fn get_info(&self) -> Option<ArchiveInfo> {
        match self.archive_type {
            ArchiveType::Zip => {
                let output = Command::new("unzip")
                    .arg("-v")
                    .arg(self.path.clone())
                    .output();

                match output {
                    Ok(output) => {
                        // Had to write it this way
                        let lines = String::from_utf8_lossy(output.stdout.as_slice());
                        let lines = lines.split('\n').collect::<Vec<&str>>();

                        let mut total_compressed = 0;
                        let mut total_uncompressed = 0;

                        let mut files = Vec::new();

                        //     8905  Defl:N     2464  72% 04-27-2022 20:33 41521ee0  Cargo.lock
                        for line in &lines[3..lines.len() - 3] {
                            let words = line.split_whitespace().collect::<Vec<&str>>();

                            let compressed = words[2].parse().unwrap();
                            let uncompressed = words[0].parse().unwrap();

                            files.push(ArchiveEntry {
                                path: words[7].to_string(),
                                size: EntrySize::Both {
                                    compressed,
                                    uncompressed
                                }
                            });

                            total_compressed += compressed;
                            total_uncompressed += uncompressed;
                        }

                        Some(ArchiveInfo {
                            path: self.path.clone(),
                            r#type: self.archive_type,
                            size: EntrySize::Both {
                                compressed: total_compressed,
                                uncompressed: total_uncompressed
                            },
                            files
                        })
                    },
                    Err(_) => None
                }
            },

            ArchiveType::Tar => {
                let output = Command::new("tar")
                    .arg("-tvf")
                    .arg(self.path.clone())
                    .output();

                match output {
                    Ok(output) => {
                        // Had to write it this way
                        let lines = String::from_utf8_lossy(output.stdout.as_slice());
                        let lines = lines.split('\n').collect::<Vec<&str>>();

                        let mut total_uncompressed = 0;
                        let mut files = Vec::new();

                        // -rw-r--r-- observer/observer 8905 2022-04-27 20:33 Cargo.lock
                        for line in &lines[..lines.len() - 1] {
                            let words = line.split_whitespace().collect::<Vec<&str>>();

                            let uncompressed = words[2].parse().unwrap();

                            files.push(ArchiveEntry {
                                path: words[5].to_string(),
                                size: EntrySize::Uncompressed(uncompressed)
                            });

                            total_uncompressed += uncompressed;
                        }

                        Some(ArchiveInfo {
                            path: self.path.clone(),
                            r#type: self.archive_type,
                            size: EntrySize::Uncompressed(total_uncompressed),
                            files
                        })
                    },
                    Err(_) => None
                }
            }
        }
    }

    pub fn get_stream(&self) -> Stream {
        Stream::from_archive(self.clone())
    }
}
