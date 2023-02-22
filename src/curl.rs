use std::sync::mpsc;
use std::time::Duration;

use curl::easy::Easy;

#[derive(Debug)]
pub struct Response {
    pub url: String,
    pub status: Option<u16>,
    pub header: Vec<String>,
    pub curl: Easy
}

impl Response {
    /// Whether this request was successful or not
    /// 
    /// By successful it means that the status code is in range of 200-299
    /// 
    /// https://developer.mozilla.org/en-US/docs/Web/API/Response/ok
    pub fn is_ok(&self) -> bool {
        match self.status {
            Some(code) => code >= 200 && code <= 299,
            None => false
        }
    }

    /// Get body of this request
    #[tracing::instrument(level = "trace")]
    pub fn get_body(&mut self) -> Result<Vec<u8>, curl::Error> {
        self.curl.nobody(false)?;

        // TODO: downloading speed may be higher than disk writing speed, so
        //       this thread can store lots of data in memory before pushing it
        //       to the disk which can lead to some issues
        //       it's better to somehow pause downloading if the queue is full

        let (send, recv) = mpsc::channel();

        #[allow(unused_must_use)]
        self.curl.write_function(move |data| {
            send.send(data.to_vec());

            Ok(data.len())
        })?;

        self.curl.perform()?;

        let mut body = Vec::new();

        while let Ok(mut data) = recv.try_recv() {
            body.append(&mut data);
        }

        Ok(body)
    }
}

/// Try to fetch remote data
#[tracing::instrument(level = "trace")]
pub fn fetch<T: ToString + std::fmt::Debug>(url: T, timeout: Option<Duration>) -> Result<Response, curl::Error> {
    let mut curl = Easy::new();

    curl.url(&url.to_string())?;
    curl.follow_location(true)?;
    curl.nobody(true)?;

    if let Some(timeout) = timeout {
        curl.timeout(timeout)?;
    }

    let (send, recv) = mpsc::channel();

    #[allow(unused_must_use)]
    curl.header_function(move |data| {
        send.send(String::from_utf8_lossy(data).to_string());

        true
    })?;

    curl.perform()?;

    let mut header = Vec::new();
    let mut status = None;

    while let Ok(data) = recv.try_recv() {
        header.push(data.clone());

        if data.len() > 9 && &data[..5] == "HTTP/" {
            let code = data.split(" ").collect::<Vec<&str>>()[1];

            status = Some(code.parse::<u16>().unwrap());
        }
    }

    Ok(Response {
        url: url.to_string(),
        status,
        header,
        curl
    })
}
