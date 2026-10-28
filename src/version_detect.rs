use std::io::{BufReader, Read, Seek};
use std::ops::RangeInclusive;
use std::fs::File;
use std::path::Path;

use anyhow::Context;
use md5::{Digest, Md5};

use crate::version::Version;

pub fn parse_dotversion(path: &Path) -> Option<Version> {
    std::fs::read(path)
        .map(|version| {
            if version.len() == 3 {
                tracing::info!("Found old format version file");
                Some(Version::new(version[0], version[1], version[2]))
            }
            else if version.len() > 3 {
                String::from_utf8(version)
                    .map(|version_str| Version::from_str(version_str.trim_end()))
                    .ok()
                    .flatten()
            }
            else {
                tracing::error!(?path, "The `.version` file is too short!");
                None
            }
        })
        .ok()
        .flatten()
}

pub fn get_version_from_game_files<const OFFSET: u64, const REGION_SIZE: usize>(
    file: &Path,
    stored_version: &Option<Version>,
    start_pattern: RangeInclusive<u8>,
    end_pattern: RangeInclusive<u8>
) -> anyhow::Result<Option<Version>> {
    tracing::debug!(?file, "Trying game files");
    fn bytes_to_num(bytes: &[u8]) -> u8 {
        bytes.iter().fold(0u8, |acc, &x| acc * 10 + (x - b'0'))
    }

    let mut file = match File::open(file) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into())
    };
    file.seek(std::io::SeekFrom::Start(OFFSET))?;
    let mut search_region = [0u8; REGION_SIZE];
    file.read_exact(&mut search_region)?;

    let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
    let mut version_ptr: usize = 0;
    let mut correct = true;

    for byte in search_region {
        match byte {
            x if end_pattern.contains(&x) => {
                if correct
                    && !version[0].is_empty()
                    && !version[1].is_empty()
                    && !version[2].is_empty()
                {
                    let found_version = Version::new(
                        bytes_to_num(&version[0]),
                        bytes_to_num(&version[1]),
                        bytes_to_num(&version[2])
                    );

                    // Little workaround for the minor game patch versions
                    // (notably 1.0.1) Prioritize version
                    // stored in the .version file
                    // because it's parsed from the API directly
                    if let Some(stored_version) = stored_version {
                        if *stored_version > found_version {
                            return Ok(Some(*stored_version));
                        }
                    }

                    return Ok(Some(found_version));
                }

                correct = false;

                if start_pattern.contains(&byte) {
                    version = [vec![], vec![], vec![]];
                    version_ptr = 0;
                    correct = true;
                }
            }

            x if start_pattern.contains(&x) => {
                version = [vec![], vec![], vec![]];
                version_ptr = 0;
                correct = true;
            }

            b'.' => {
                version_ptr += 1;

                if version_ptr > 2 {
                    correct = false;
                }
            }

            _ => {
                if correct && byte.is_ascii_digit() {
                    version[version_ptr].push(byte);
                }
                else {
                    correct = false;
                }
            }
        }
    }

    Ok(None)
}

fn file_md5(path: &Path) -> std::io::Result<String> {
    let mut file = BufReader::new(File::open(path)?);
    let mut md5 = Md5::new();

    std::io::copy(&mut file, &mut md5)?;

    Ok(format!("{:x}", md5.finalize()))
}

pub fn get_version_game_scan(
    exe_path: &Path,
    scan_url: &str,
    game_id: &str
) -> anyhow::Result<Option<Version>> {
    tracing::debug!(game_id, ?exe_path, "Trying Game Scan");
    let exe_hash = match file_md5(exe_path) {
        Ok(hash) => hash,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into())
    };
    let scan_info = minreq::get(scan_url)
        .send()
        .context("Sending game scan API request")?
        .json::<serde_json::Value>()
        .context("Parsing game scan API response")?;

    Ok(scan_info
        .get("data")
        .and_then(|v| {
            v.get("game_scan_info")?
                .as_array()?
                .iter()
                .find_map(|scan_info| {
                    if scan_info.get("game_id")?.as_str()? == game_id {
                        scan_info.get("game_exe_list")?.as_array()
                    }
                    else {
                        None
                    }
                })
        })
        .and_then(|exe_list| {
            exe_list.iter().find_map(|exe_hash_item| {
                if exe_hash_item
                    .get("md5")?
                    .as_str()?
                    .eq_ignore_ascii_case(&exe_hash)
                {
                    exe_hash_item
                        .get("version")?
                        .as_str()
                        .and_then(Version::from_str)
                }
                else {
                    None
                }
            })
        }))
}

#[cfg(feature = "sophon")]
pub fn get_version_sophon(
    exe_path: &Path,
    game_id: &str,
    edition: crate::sophon::GameEdition
) -> anyhow::Result<Option<Version>> {
    use sophon::api::ApiContext;

    use crate::sophon;
    tracing::debug!(game_id, ?exe_path, "Trying sophon");
    let Some(exe_filename) = exe_path.file_name().and_then(std::ffi::OsStr::to_str)
    else {
        return Ok(None);
    };
    let exe_hash = match file_md5(exe_path) {
        Ok(hash) => hash,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into())
    };

    let client = sophon::client_builder().build().unwrap();
    let api = ApiContext {
        client,
        edition
    };

    let game_branches = api.game_branches()?;
    let Some(package) = game_branches.as_ref().package_latest(game_id, false)
    else {
        tracing::warn!(game_id, ?api.edition, "No game branch found");
        return Ok(None);
    };

    let download_info = package.downloads()?;
    let Some(dlinfo) = download_info.as_ref().for_component("game")
    else {
        return Ok(None);
    };

    let download_manifest = dlinfo.download_manifest()?;
    let exe_hash_from_dl = download_manifest
        .value
        .assets
        .iter()
        .find_map(|asset| (asset.asset_name == exe_filename).then_some(&asset.asset_hash_md5));

    if let Some(dlhash) = exe_hash_from_dl {
        if dlhash.eq_ignore_ascii_case(&exe_hash) {
            return Ok(Version::from_str(&package.value.tag));
        }
    }
    else {
        tracing::error!("Failed to find exe hashes in sophon download api, asset not found")
    }

    // Hash does not match latest at this point, try to find in updates

    let diffs = package.diffs()?;
    let Some(diff_info) = diffs.as_ref().for_component("game")
    else {
        return Ok(None);
    };

    let patch_manifest = diff_info.patch_manifest()?;
    let Some(exe_hashes) = patch_manifest
        .value
        .patch_assets
        .iter()
        .find_map(|patch_asset| {
            (patch_asset.asset_name == exe_filename).then(|| {
                patch_asset
                    .asset_patch_chunks
                    .iter()
                    .map(|(tag, chunk)| (tag, &chunk.original_file_md5))
            })
        })
    else {
        tracing::error!("Failed to get exe hashes from sophon update api, asset not found");
        return Ok(None);
    };

    for (tag, patch_exe_hash) in exe_hashes {
        if patch_exe_hash.eq_ignore_ascii_case(&exe_hash) {
            return Ok(Version::from_str(tag));
        }
    }

    tracing::warn!("Sophon API lookup failed to find any matching exe hash");

    Ok(None)
}
