use std::io::Error;

use serde_json::{from_str, Value};

use crate::api::API;
use crate::curl::fetch;
use crate::installer::downloader::Downloader;
use crate::voice_data::locale::VoiceLocale;

// {"remoteName": "UnityPlayer.dll", "md5": "8c8c3d845b957e4cb84c662bed44d072", "fileSize": 33466104}
#[derive(Debug, Clone)]
pub struct IntegrityFile {
    pub path: String,
    pub md5: String,
    pub size: u64,
    base_url: String
}

impl IntegrityFile {
    /// Compare files' sizes and (if needed) hashes
    pub fn verify<T: ToString>(&self, game_path: T) -> bool {
        let file_path = format!("{}/{}", game_path.to_string(), self.path);

        // Compare files' sizes. If they're different - they 100% different
        match std::fs::metadata(&file_path) {
            Ok(metadata) => {
                if metadata.len() != self.size {
                    false
                }

                else {
                    // And if files' sizes are same we should compare their hashes
                    match std::fs::read(&file_path) {
                        Ok(hash) => format!("{:x}", md5::compute(hash)) == self.md5,
                        Err(_) => false
                    }
                }
            },
            Err(_) => false
        }
    }

    /// Replace remote file with the latest one
    /// 
    /// This method doesn't compare them, so you should do it manually
    pub fn repair<T: ToString>(&self, game_path: T) -> Result<(), Error> {
        let mut downloader = Downloader::new(format!("{}/{}", self.base_url, self.path))?;

        Ok(downloader.download_to(format!("{}/{}", game_path.to_string(), self.path), |_, _| {})?)
    }
}

fn try_get_some_integrity_files<T: ToString>(file_name: T) -> Result<Vec<IntegrityFile>, Error> {
    let response = API::try_fetch_json()?;

    let decompressed_path = response.data.game.latest.decompressed_path;
    
    let mut pkg_version = fetch(format!("{}/{}", &decompressed_path, file_name.to_string()))?;
    let pkg_version = pkg_version.get_body()?;

    let mut files = Vec::new();

    for line in String::from_utf8_lossy(&pkg_version).lines() {
        if let Ok(value) = from_str::<Value>(line) {
            files.push(IntegrityFile {
                path: value["remoteName"].as_str().unwrap().to_string(),
                md5: value["md5"].as_str().unwrap().to_string(),
                size: value["fileSize"].as_u64().unwrap(),
                base_url: decompressed_path.clone()
            });
        }
    }

    Ok(files)
}

/// Try to list latest game files
pub fn try_get_integrity_files() -> Result<Vec<IntegrityFile>, Error> {
    Ok(try_get_some_integrity_files("pkg_version")?)
}

/// Try to list latest voice package files
pub fn try_get_voice_integrity_files(locale: VoiceLocale) -> Result<Vec<IntegrityFile>, Error> {
    Ok(try_get_some_integrity_files(format!("Audio_{}_pkg_version", locale.to_folder()))?)
}
