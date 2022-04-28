use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Instant, Duration};
use std::io::{Error, Write};
use std::fs::File;

pub enum StreamUpdate {
    Start,
    Progress(usize, usize),
    Finish
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Downloaders {
    Aria2c,
    Curl,
    Native
}

pub struct Stream {
    uri: String,
    response: minreq::ResponseLazy,
    total: usize,
    on_update: Box<dyn Fn(StreamUpdate)>,
    pub download_progress_interval: Duration
}

impl Stream {
    pub const CHUNK_SIZE: usize = 1024;

    pub fn open<T: ToString>(uri: T) -> Result<Stream, minreq::Error> {
        match minreq::get(uri.to_string()).send_lazy() {
            Ok(response) => {
                let mut total = response.size_hint().0;

                if let Some(len) = response.headers.get("content-length") {
                    total = len.parse().unwrap();
                }

                Ok(Stream {
                    uri: uri.to_string(),
                    response,
                    total,
                    on_update: Box::new(|_| {}),
                    download_progress_interval: Duration::from_millis(50)
                })
            },
            Err(err) => Err(err)
        }
    }

    pub fn on_update<T: Fn(StreamUpdate) + 'static>(&mut self, callback: T) {
        self.on_update = Box::new(callback);
    }

    pub fn download<T: ToString>(&mut self, path: T, method: Downloaders) -> Result<Duration, Error> {
        match method {
            Downloaders::Aria2c => todo!(),
            Downloaders::Curl => {
                // curl -s -L -N -C - -o <output> <uri>
                let child = Command::new("curl")
                    .args([
                        "-s", "-L", "-N", "-C", "-",
                        "-o", path.to_string().as_str(),
                        self.uri.as_str()
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();

                match child {
                    Ok(mut child) => {
                        (self.on_update)(StreamUpdate::Start);

                        let instant = Instant::now();

                        while let Ok(None) = child.try_wait() {
                            if let Ok(metadata) = Path::new(&path.to_string()).metadata() {
                                (self.on_update)(StreamUpdate::Progress(usize::try_from(metadata.len()).unwrap(), self.total));
                            }

                            std::thread::sleep(self.download_progress_interval);
                        }

                        (self.on_update)(StreamUpdate::Finish);

                        Ok(instant.elapsed())
                    },
                    Err(err) => Err(err)
                }
            },
            Downloaders::Native => {
                match File::create(path.to_string()) {
                    Ok(mut file) => {
                        (self.on_update)(StreamUpdate::Start);

                        let instant = Instant::now();
                        let mut progress = 0;

                        while let Some(mut bytes) = self.get_chunk() {
                            progress += bytes.len();
                            
                            (self.on_update)(StreamUpdate::Progress(progress, self.total));

                            file.write_all(&mut bytes);
                        }

                        (self.on_update)(StreamUpdate::Finish);
        
                        Ok(instant.elapsed())
                    },
                    Err(err) => Err(err),
                }
            }
        }
    }

    // TODO: improve speed
    fn get_chunk(&mut self) -> Option<Vec<u8>> {
        let mut chunk = Vec::with_capacity(Self::CHUNK_SIZE);

        for _ in 0..Self::CHUNK_SIZE {
            match self.response.next() {
                Some(Ok((byte, _))) => chunk.push(byte),
                Some(Err(_)) => break,
                None => break
            }
        }

        if chunk.len() > 0 { Some(chunk) } else { None }
    }

    pub fn get_total(&self) -> usize {
        self.total
    }
}
