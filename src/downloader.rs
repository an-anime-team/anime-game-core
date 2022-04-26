pub struct Stream {
    response: minreq::ResponseLazy,
    fetched: usize,
    total: usize
}

impl Stream {
    pub const CHUNK_SIZE: usize = 1024;

    pub fn new(response: minreq::ResponseLazy) -> Stream {
        let mut total = response.size_hint().0;

        /*if let Some(len) = response.headers.get("content-length") {
            total = len.parse().unwrap();
        }*/
        
        Stream {
            response,
            fetched: 0,
            total
        }
    }

    // TODO: improve speed
    pub fn get_chunk(&mut self) -> Option<Vec<u8>> {
        let mut chunk = Vec::with_capacity(Self::CHUNK_SIZE);

        for _ in 0..Self::CHUNK_SIZE {
            match self.response.next() {
                Some(Ok((byte, _))) => chunk.push(byte),
                Some(Err(_)) => break,
                None => break
            }
        }

        self.fetched += chunk.len();

        if chunk.len() > 0 { Some(chunk) } else { None }
    }

    pub fn get_fetched(&self) -> usize {
        self.fetched
    }

    pub fn get_total(&self) -> usize {
        self.total
    }

    pub fn get_progress(&self) -> Option<f64> {
        match self.total {
            0 => None,
            total => Some((self.fetched as f64) / (total as f64))
        }
    }
}

pub fn download(uri: &str) -> Result<Stream, minreq::Error> {
    match minreq::get(uri).send_lazy() {
        Ok(response) => Ok(Stream::new(response)),
        Err(err) => Err(err)
    }
}
