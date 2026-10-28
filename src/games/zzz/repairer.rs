use std::path::{Path, PathBuf};

use cached::proc_macro::cached;
use sophon::api::ApiContext;

use super::consts::GameEdition;
use crate::repairer::IntegrityFile;

// TODO: utilize the `timeout` variable!

fn try_get_some_integrity_files<T: AsRef<str>>(
    game_edition: GameEdition,
    file_name: T,
    _timeout: Option<u64>
) -> anyhow::Result<Vec<IntegrityFile>> {
    let api = ApiContext::new(game_edition.into());

    let download_manifest = api
        .game_branches()?
        .as_ref()
        .package_latest(game_edition.api_game_id(), false)
        .ok_or_else(|| {
            anyhow::anyhow!("failed to get the game version information")
                .context(format!("game id: {}", game_edition.api_game_id()))
        })?
        .downloads()?
        .as_ref()
        .for_component("game")
        .ok_or_else(|| anyhow::anyhow!("failed to find game download info"))?
        .download_manifest()?
        .value;

    let files = download_manifest
        .assets
        .iter()
        .filter(|asset| asset.asset_name == file_name.as_ref())
        .map(IntegrityFile::from)
        .collect::<Vec<_>>();

    Ok(files)
}

/// Try to list latest game files
#[cached(result)]
pub fn try_get_integrity_files(
    game_edition: GameEdition,
    timeout: Option<u64>
) -> anyhow::Result<Vec<IntegrityFile>> {
    try_get_some_integrity_files(game_edition, "pkg_version", timeout)
}

/// Try to get specific integrity file
///
/// `relative_path` must be relative to the game's root folder, so
/// if your file is e.g. `/path/to/[AnimeGame]/[AnimeGame_Data]/level0`, then
/// root folder is `/path/to/[AnimeGame]`, and `relative_path` must be
/// `[AnimeGame_Data]/level0`
pub fn try_get_integrity_file(
    game_edition: GameEdition,
    relative_path: impl AsRef<Path>,
    timeout: Option<u64>
) -> anyhow::Result<Option<IntegrityFile>> {
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

/// Try to get list of files that are not more used by the game and can be
/// deleted
///
/// ⚠️ Be aware that the game can create its own files after downloading, so "un
/// used files" may not be really unused. It's strongly recommended to use
/// this function only with manual control from user's side, in example to show
/// him paths to these files and let him choose what to do with them
pub fn try_get_unused_files(
    game_edition: GameEdition,
    game_dir: impl Into<PathBuf>,
    timeout: Option<u64>
) -> anyhow::Result<Vec<PathBuf>> {
    let used_files = try_get_integrity_files(game_edition, timeout)?
        .into_iter()
        .map(|file| file.path)
        .collect::<Vec<PathBuf>>();

    let skip_names = [
        String::from("webCaches"),
        String::from("SDKCaches"),
        String::from("GeneratedSoundBanks"),
        String::from("ScreenShot")
    ];

    crate::repairer::try_get_unused_files(game_dir, used_files, skip_names)
}
