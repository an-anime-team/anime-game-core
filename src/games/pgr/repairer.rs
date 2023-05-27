use std::path::PathBuf;

use cached::proc_macro::cached;

use super::api;
use super::consts::API_BASE_URI;

use crate::repairer::IntegrityFile;

fn try_get_some_integrity_files<T: AsRef<str>>(file_name: T, timeout: Option<u64>) -> anyhow::Result<Vec<IntegrityFile>> {
    let decompressed_path = format!("{API_BASE_URI}/{}", api::game::request()?.default.resourcesBasePath);

    let pkg_version = minreq::get(format!("{decompressed_path}/{}", file_name.as_ref()))
        .with_timeout(timeout.unwrap_or(*crate::REQUESTS_TIMEOUT))
        .send()?;

    let mut files = Vec::new();

    for line in String::from_utf8_lossy(pkg_version.as_bytes()).lines() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            files.push(IntegrityFile {
                path: PathBuf::from(value["dest"].as_str().unwrap()),
                md5: value["md5"].as_str().unwrap().to_string(),
                size: value["size"].as_u64().unwrap(),
                base_url: decompressed_path.clone()
            });
        }
    }

    Ok(files)
}

/// Try to list latest game files
#[cached(result)]
pub fn try_get_integrity_files(timeout: Option<u64>) -> anyhow::Result<Vec<IntegrityFile>> {
    try_get_some_integrity_files("pkg_version", timeout)
}

/// Try to get specific integrity file
/// 
/// `relative_path` must be relative to the game's root folder, so
/// if your file is e.g. `/path/to/[AnimeGame]/[AnimeGame_Data]/level0`, then root folder is `/path/to/[AnimeGame]`,
/// and `relative_path` must be `[AnimeGame_Data]/level0`
pub fn try_get_integrity_file<T: Into<PathBuf>>(relative_path: T, timeout: Option<u64>) -> anyhow::Result<Option<IntegrityFile>> {
    let relative_path = relative_path.into();

    if let Ok(files) = try_get_integrity_files(timeout) {
        for file in files {
            if file.path == relative_path {
                return Ok(Some(file));
            }
        }
    }

    Ok(None)
}
