use std::process::Command;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ArchiveType {
    Zip,
    Tar
}

pub struct Stream {
    path: String,
    archive_type: ArchiveType,
    on_start: Box<dyn FnOnce()>,
    on_unpacked: Box<dyn FnOnce()>,
    on_finish: Box<dyn FnOnce()>
}

impl Stream {
    pub fn new(path: String, archive_type: ArchiveType) -> Stream {
        Stream {
            path,
            archive_type,
            on_start: Box::new(|| {}),
            on_unpacked: Box::new(|| {}),
            on_finish: Box::new(|| {})
        }
    }

    pub fn on_start<T: FnOnce() + 'static>(&mut self, callback: T) {
        self.on_start = Box::new(callback);
    }

    pub fn on_unpacked<T: FnOnce() + 'static>(&mut self, callback: T) {
        self.on_unpacked = Box::new(callback);
    }

    pub fn on_finish<T: FnOnce() + 'static>(&mut self, callback: T) {
        self.on_finish = Box::new(callback);
    }

    pub fn unpack(&self) {
        let command = match self.archive_type {
            ArchiveType::Zip => {
                Command::new("unzip")
            },
            ArchiveType::Tar => {
                Command::new("tar")
            },
        };


    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: String,
    pub compressed_size: u32,
    pub uncompressed_size: u32
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveInfo {
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub files: Vec<ArchiveEntry>
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archive {
    path: String,
    archive_type: ArchiveType
}

impl Archive {
    pub fn new<T: ToString>(path: T) -> Option<Archive> {
        let path = path.to_string();

        if path.len() > 4 && &path[path.len() - 4..] == ".zip" {
            Some(Archive {
                path,
                archive_type: ArchiveType::Zip
            })
        }

        else if path.len() > 4 && &path[path.len() - 7..path.len() - 2] == ".tar." {
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
                        let lines = lines.split_whitespace().collect::<Vec<&str>>();

                        todo!();

                        // let files = Vec::new();

                        //     8905  Defl:N     2464  72% 04-27-2022 20:33 41521ee0  Cargo.lock
                        for line in &lines[2..lines.len() - 2] {

                        }

                        None
                    },
                    Err(_) => None
                }
            },
            ArchiveType::Tar => {
                // Command::new("tar").arg("-tvf").arg(self.path.clone()).output()

                None
            }
        }
    }
}
