use std::path::{Path, PathBuf};

use cached::proc_macro::cached;

use super::api;
use super::consts::GameEdition;

use crate::repairer::IntegrityFile;

fn try_get_some_integrity_files<T: AsRef<str>>(game_edition: GameEdition, file_name: T, timeout: Option<u64>) -> anyhow::Result<Vec<IntegrityFile>> {
    let decompressed_path = api::request(game_edition)?.main.major.res_list_url;

    let pkg_version = minreq::get(format!("{decompressed_path}/{}", file_name.as_ref()))
        .with_timeout(timeout.unwrap_or(*crate::REQUESTS_TIMEOUT))
        .send()?;

    let mut files = Vec::new();

    for line in String::from_utf8_lossy(pkg_version.as_bytes()).lines() {
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
#[cached(result)]
pub fn try_get_integrity_files(game_edition: GameEdition, timeout: Option<u64>) -> anyhow::Result<Vec<IntegrityFile>> {
    try_get_some_integrity_files(game_edition, "pkg_version", timeout)
}

/// Try to get specific integrity file
/// 
/// `relative_path` must be relative to the game's root folder, so
/// if your file is e.g. `/path/to/[AnimeGame]/[AnimeGame_Data]/level0`, then root folder is `/path/to/[AnimeGame]`,
/// and `relative_path` must be `[AnimeGame_Data]/level0`
pub fn try_get_integrity_file(game_edition: GameEdition, relative_path: impl AsRef<Path>, timeout: Option<u64>) -> anyhow::Result<Option<IntegrityFile>> {
    let relative_path = relative_path.as_ref();

    if let Ok(files) = try_get_integrity_files(game_edition, timeout) {
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
pub fn try_get_unused_files(game_edition: GameEdition, game_dir: impl Into<PathBuf>, timeout: Option<u64>) -> anyhow::Result<Vec<PathBuf>> {
    let used_files = try_get_integrity_files(game_edition, timeout)?
        .into_iter()
        .map(|file| file.path)
        .collect::<Vec<PathBuf>>();

    let skip_names = [
        String::from("webCaches"),
        String::from("SDKCaches"),
        String::from("GeneratedSoundBanks"),
        String::from("ScreenShot"),
    ];

    crate::repairer::try_get_unused_files(game_dir, used_files, skip_names)
}
