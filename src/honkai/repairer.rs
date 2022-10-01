use std::time::Duration;
use std::path::PathBuf;

use cached::proc_macro::cached;

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
                path: PathBuf::from(value["remoteName"].as_str().unwrap()),
                md5: value["md5"].as_str().unwrap().to_string(),
                size: value["fileSize"].as_u64().unwrap(),
                base_url: decompressed_path.clone()
            });
        }
    }

    Ok(files)
}

/// Try to list latest game files
#[cached(result=true)]
pub fn try_get_integrity_files(timeout: Option<Duration>) -> anyhow::Result<Vec<IntegrityFile>> {
    try_get_some_integrity_files("pkg_version", timeout)
}

/// Try to get specific integrity file
/// 
/// `relative_path` must be relative to the game's root folder, so
/// if your file is e.g. `/path/to/[AnimeGame]/[AnimeGame_Data]/level0`, then root folder is `/path/to/[AnimeGame]`,
/// and `relative_path` must be `[AnimeGame_Data]/level0`
pub fn try_get_integrity_file<T: Into<PathBuf>>(relative_path: T, timeout: Option<Duration>) -> anyhow::Result<Option<IntegrityFile>> {
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

/// Try to get list of files that are not more used by the game and can be deleted
/// 
/// ⚠️ Be aware that the game can create its own files after downloading, so "unused files" may not be really unused.
/// It's strongly recommended to use this function only with manual control from user's side, in example to show him
/// paths to these files and let him choose what to do with them
pub fn try_get_unused_files<T: Into<PathBuf>>(game_dir: T, timeout: Option<Duration>) -> anyhow::Result<Vec<PathBuf>> {
    let used_files = try_get_integrity_files(timeout)?
        .into_iter()
        .map(|file| file.path)
        .collect::<Vec<PathBuf>>();

    let skip_names = [
        String::from("webCaches"),
        String::from("SDKCaches")
    ];

    crate::repairer::try_get_unused_files(game_dir, used_files, skip_names)
}
