use std::time::Duration;

use crate::repairer::IntegrityFile;
use crate::curl::fetch;

use super::api;

fn try_get_some_integrity_files<T: ToString>(file_name: T, timeout: Option<Duration>) -> anyhow::Result<Vec<IntegrityFile>> {
    let decompressed_path = api::try_fetch_json()?.data.game.latest.decompressed_path;
    let pkg_version = fetch(format!("{decompressed_path}/{}", file_name.to_string()), timeout)?.get_body()?;

    let mut files = Vec::new();

    for line in String::from_utf8_lossy(&pkg_version).lines() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
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
pub fn try_get_integrity_files(timeout: Option<Duration>) -> anyhow::Result<Vec<IntegrityFile>> {
    try_get_some_integrity_files("pkg_version", timeout)
}
