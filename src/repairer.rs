use std::io::Error;

use serde_json::{from_str, Value};

use crate::api::API;
use crate::curl::fetch;
use crate::installer::downloader::Downloader;

// {"remoteName": "UnityPlayer.dll", "md5": "8c8c3d845b957e4cb84c662bed44d072", "fileSize": 33466104}
#[derive(Debug, Clone)]
pub struct IntegrityFile {
    pub path: String,
    pub md5: String,
    pub size: u64,
    base_url: String
}

impl IntegrityFile {
    /// Compare file hashes
    pub fn verify<T: ToString>(&self, game_path: T) -> Result<bool, Error> {
        let hash = std::fs::read(format!("{}/{}", game_path.to_string(), self.path))?;
        let hash = format!("{:x}", md5::compute(hash));

        Ok(hash == self.md5)
    }

    /// Replace remote file with the latest one
    /// 
    /// This method doesn't compare them, so you should do it manually
    pub fn repair<T: ToString>(&self, game_path: T) -> Result<(), Error> {
        match Downloader::new(format!("{}/{}", self.base_url, self.path)) {
            Ok(mut downloader) => {
                match downloader.download_to(format!("{}/{}", game_path.to_string(), self.path), |_, _| {}) {
                    Ok(_) => Ok(()),
                    Err(err) => Err(err.into())
                }
            },
            Err(err) => Err(err.into())
        }
    }
}

/// Try to list latest game files
pub fn try_get_integrity_files() -> Result<Vec<IntegrityFile>, Error> {
    match API::try_fetch_json() {
        Ok(response) => {
            match fetch(format!("{}/pkg_version", &response.data.game.latest.decompressed_path)) {
                Ok(mut pkg_version) => {
                    match pkg_version.get_body() {
                        Ok(pkg_version) => {
                            let mut files = Vec::new();

                            for line in String::from_utf8_lossy(&pkg_version).lines() {
                                if let Ok(value) = from_str::<Value>(line) {
                                    files.push(IntegrityFile {
                                        path: value["remoteName"].as_str().unwrap().to_string(),
                                        md5: value["md5"].as_str().unwrap().to_string(),
                                        size: value["fileSize"].as_u64().unwrap(),
                                        base_url: response.data.game.latest.decompressed_path.clone()
                                    });
                                }
                            }

                            Ok(files)
                        },
                        Err(err) => Err(err.into())
                    }
                },
                Err(err) => Err(err.into())
            }
        },
        Err(err) => Err(err.into())
    }
}
