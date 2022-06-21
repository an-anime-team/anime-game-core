use std::string::FromUtf8Error;
use std::sync::mpsc;

use curl::easy::Easy;
use serde::Deserialize;

use super::consts::API_URI;

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

pub struct API;

impl<'a> API {
    /// Remote server can not be available
    /// TODO: cache response
    pub fn try_fetch() -> Result<Response, curl::Error> {
        let mut curl = Easy::new();
        let (sender, receiver) = mpsc::channel();

        curl.url(API_URI)?;

        curl.write_function(move |data| {
            sender.send(Vec::from(data));

            Ok(data.len())
        })?;

        curl.perform()?;

        let mut result = Vec::new();

        while let Ok(mut data) = receiver.try_recv() {
            result.append(&mut data);
        }

        Ok(Response {
            data: result
        })
    }

    // TODO: add try_fetch_json
}
