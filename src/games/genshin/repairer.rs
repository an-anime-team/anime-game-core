use std::path::{Path, PathBuf};

use cached::proc_macro::cached;

use crate::repairer::IntegrityFile;
use crate::sophon;

use super::consts::GameEdition;
use super::voice_data::locale::VoiceLocale;

fn try_get_some_integrity_files(
    game_edition: GameEdition,
    matching_field: &str,
    timeout: Option<u64>
) -> anyhow::Result<Vec<IntegrityFile>> {
    let client = reqwest::blocking::Client::new();

    let game_branches = sophon::get_game_branches_info(client.clone(), game_edition.into())?;

    let latest_ver = game_branches.latest_version_by_id(game_edition.game_id()).unwrap();
    let game_branch_info = game_branches.get_game_by_id(game_edition.game_id(), latest_ver).unwrap();

    let downloads = sophon::installer::get_game_download_sophon_info(
        client.clone(),
        &game_branch_info.main,
        game_edition.into()
    )?;

    let download_manifest = sophon::installer::get_download_manifest(
        client.clone(),
        downloads.manifests.iter()
            .find(|sdi| sdi.matching_field == matching_field).unwrap()
    )?;

    let mut files = Vec::new();

    for asset_info in &download_manifest.Assets {
        files.push(asset_info.into())
    }

    Ok(files)
}

/// Try to list latest game files.
#[cached(result)]
pub fn try_get_integrity_files(
    game_edition: GameEdition,
    timeout: Option<u64>
) -> anyhow::Result<Vec<IntegrityFile>> {
    try_get_some_integrity_files(game_edition, "game", timeout)
}

/// Try to list latest voice package files.
#[cached(result)]
pub fn try_get_voice_integrity_files(
    game_edition: GameEdition,
    locale: VoiceLocale,
    timeout: Option<u64>
) -> anyhow::Result<Vec<IntegrityFile>> {
    try_get_some_integrity_files(game_edition, locale.to_code(), timeout)
}

/// Try to get specific integrity file.
///
/// `relative_path` must be relative to the game's root folder, so if your file
/// is e.g. `/path/to/[AnimeGame]/[AnimeGame_Data]/level0`, then root folder is
/// `/path/to/[AnimeGame]`, and `relative_path` must be `[AnimeGame_Data]/level0`.
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

    for lang in VoiceLocale::list() {
        if let Ok(files) = try_get_voice_integrity_files(game_edition, *lang, timeout) {
            for file in files {
                if file.path == relative_path {
                    return Ok(Some(file));
                }
            }
        }
    }

    Ok(None)
}

/// Try to get list of files that are not more used by the game and can be
/// deleted.
///
/// ⚠️ Be aware that the game can create its own files after downloading, so
/// "unused files" may not be really unused. It's strongly recommended to use
/// this function only with manual control from user's side, in example to show
/// him paths to these files and let him choose what to do with them.
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
        String::from("ScreenShot"),
    ];

    crate::repairer::try_get_unused_files(game_dir, used_files, skip_names)
}

/// Try to get list of files that are not more used by the game and can be
/// deleted.
///
/// ⚠️ Be aware that the game can create its own files after downloading, so
/// "unused files" may not be really unused. It's strongly recommended to use
/// this function only with manual control from user's side, in example to show
/// him paths to these files and let him choose what to do with them.
pub fn try_get_unused_voice_files(
    game_edition: GameEdition,
    game_dir: impl Into<PathBuf>,
    locale: VoiceLocale,
    timeout: Option<u64>
) -> anyhow::Result<Vec<PathBuf>> {
    let used_files = try_get_voice_integrity_files(game_edition, locale, timeout)?
        .into_iter()
        .map(|file| file.path)
        .collect::<Vec<PathBuf>>();

    crate::repairer::try_get_unused_files(game_dir, used_files, [])
}
