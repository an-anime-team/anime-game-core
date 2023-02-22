use std::string::FromUtf8Error;
use std::sync::mpsc;

use curl::easy::Easy;
use serde::Deserialize;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Response {
    data: Vec<u8>
}

impl<'a> Response {
    pub fn bytes(&self) -> &Vec<u8> {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Try to parse utf8 string from responded array of bytes
    pub fn try_string(&self) -> Result<String, FromUtf8Error> {
        String::from_utf8(self.data.clone())
    }

    /// Try to parse json data from responded array of bytes
    pub fn try_json<T: Deserialize<'a>>(&'a self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(self.data.as_slice())
    }
}

/// Try to fetch data from the game's api
#[cached::proc_macro::cached]
#[tracing::instrument(level = "trace")]
pub fn try_fetch(uri: &'static str) -> Result<Response, curl::Error> {
    let mut curl = Easy::new();
    let (sender, receiver) = mpsc::channel();

    curl.url(uri)?;

    #[allow(unused_must_use)]
    curl.write_function(move |data| {
        sender.send(Vec::from(data));

        Ok(data.len())
    })?;

    curl.perform()?;

    let mut result = Vec::new();

    while let Ok(mut data) = receiver.try_recv() {
        result.append(&mut data);
    }

    let response = Response {
        data: result
    };

    Ok(response)
}
