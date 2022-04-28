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
    response: minreq::ResponseLazy,
    total: usize,
    on_update: Box<dyn Fn(StreamUpdate)>
}

impl Stream {
    pub const CHUNK_SIZE: usize = 1024;

    pub fn new(response: minreq::ResponseLazy) -> Stream {
        let total = response.size_hint().0;

        /*if let Some(len) = response.headers.get("content-length") {
            total = len.parse().unwrap();
        }*/
        
        Stream {
            response,
            total,
            on_update: Box::new(|_| {})
        }
    }

    pub fn on_update<T: Fn(StreamUpdate) + 'static>(&mut self, callback: T) {
        self.on_update = Box::new(callback);
    }

    pub fn download<T: ToString>(&mut self, path: T, method: Downloaders) -> Result<Duration, Error> {
        let instant = Instant::now();

        match method {
            Downloaders::Aria2c => todo!(),
            Downloaders::Curl => {
                todo!()
            },
            Downloaders::Native => {
                match File::create(path.to_string()) {
                    Ok(mut file) => {
                        (self.on_update)(StreamUpdate::Start);

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

pub fn download<T: Into<minreq::URL>>(uri: T) -> Result<Stream, minreq::Error> {
    match minreq::get(uri).send_lazy() {
        Ok(response) => Ok(Stream::new(response)),
        Err(err) => Err(err)
    }
}
