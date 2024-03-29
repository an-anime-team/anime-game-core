use std::path::PathBuf;

use cached::proc_macro::cached;

use super::api;
use super::consts::API_BASE_URI;

use crate::repairer::IntegrityFile;

/// Try to list latest game files
#[cached(result)]
pub fn try_get_integrity_files() -> anyhow::Result<Vec<IntegrityFile>> {
    let decompressed_path = format!("{API_BASE_URI}/{}", api::game::request()?.default.resourcesBasePath);

    Ok(api::resource::request()?.resource.into_iter().map(|resource| IntegrityFile {
        path: resource.dest
            .strip_prefix('/')
            .unwrap_or(resource.dest.as_str())
            .into(),

        md5: resource.md5,
        size: resource.size,
        base_url: decompressed_path.clone()
    }).collect())
}

/// Try to get specific integrity file
/// 
/// `relative_path` must be relative to the game's root folder, so
/// if your file is e.g. `/path/to/[AnimeGame]/[AnimeGame_Data]/level0`, then root folder is `/path/to/[AnimeGame]`,
/// and `relative_path` must be `[AnimeGame_Data]/level0`
pub fn try_get_integrity_file<T: Into<PathBuf>>(relative_path: T) -> anyhow::Result<Option<IntegrityFile>> {
    let relative_path = relative_path.into();

    if let Ok(files) = try_get_integrity_files() {
        for file in files {
            if file.path == relative_path {
                return Ok(Some(file));
            }
        }
    }

    Ok(None)
}
