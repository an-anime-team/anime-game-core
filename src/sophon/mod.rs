use std::{fs::File, os::unix::fs::PermissionsExt};
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use protobuf::Message;
use reqwest::blocking::Client;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use thiserror::Error;

use api_schemas::ApiResponse;
use api_schemas::game_branches::GameBranches;

#[cfg(feature = "genshin")]
use crate::genshin;

use crate::prettify_bytes::prettify_bytes;

pub mod api_schemas;
pub mod installer;
pub mod protos;
pub mod updater;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum GameEdition {
    Global,
    China
}

#[cfg(feature = "genshin")]
impl From<genshin::consts::GameEdition> for GameEdition {
    fn from(value: genshin::consts::GameEdition) -> Self {
        match value {
            genshin::consts::GameEdition::China => Self::China,
            genshin::consts::GameEdition::Global => Self::Global
        }
    }
}

impl GameEdition {
    #[inline]
    pub fn branches_host(&self) -> &str {
        match self {
            Self::Global => concat!("https://", "s", "g-hy", "p-api.", "h", "oy", "over", "se", ".com"),
            Self::China => concat!("https://", "hy", "p-api.", "mi", "h", "oyo", ".com")
        }
    }

    #[inline]
    pub fn api_host(&self) -> &str {
        match self {
            Self::Global => concat!("https://", "s", "g-pu", "blic-api.", "h", "oy", "over", "se", ".com"),
            Self::China => concat!("https://", "api-t", "ak", "umi.", "mi", "h", "oyo", ".com")
        }
    }

    #[inline]
    pub fn launcher_id(&self) -> &str {
        match self {
            Self::Global => "VYTpXlbWo8",
            Self::China => "jGHBHlcOq1"
        }
    }
}

#[inline(always)]
fn get_game_branches_url(edition: GameEdition) -> String {
    format!("{}/hyp/hyp-connect/api/getGameBranches?launcher_id={}", edition.branches_host(), edition.launcher_id())
}

#[inline(always)]
pub fn get_game_branches_info(
    client: Client,
    edition: GameEdition
) -> Result<GameBranches, SophonError> {
    api_get_request(client, &get_game_branches_url(edition))
}

fn api_get_request<T: DeserializeOwned>(client: Client, url: &str) -> Result<T, SophonError> {
    let response = client.get(url).send()?.error_for_status()?;

    Ok(response.json::<ApiResponse<T>>()?.data)
}

fn api_post_request<T: DeserializeOwned>(client: Client, url: &str) -> Result<T, SophonError> {
    let response = client.post(url).send()?.error_for_status()?;

    Ok(response.json::<ApiResponse<T>>()?.data)
}

fn get_protobuf_from_url<T: Message>(
    url: &str,
    client: Client,
    compression: bool,
) -> Result<T, SophonError> {
    let response = client.get(url).send()?.error_for_status()?;

    let compressed_manifest = response.bytes()?;

    let protobuf_bytes = if compression {
        zstd::decode_all(&*compressed_manifest).unwrap()
    } else {
        compressed_manifest.into()
    };

    let parsed_manifest = T::parse_from_bytes(&protobuf_bytes).unwrap();

    Ok(parsed_manifest)
}

fn ensure_parent(path: impl AsRef<Path>) -> std::io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}

fn md5_hash_str(data: &[u8]) -> String {
    format!("{:x}", Md5::digest(data))
}

fn bytes_check_md5(data: &[u8], expected_hash: &str) -> bool {
    let computed_hash = md5_hash_str(data);

    expected_hash == computed_hash
}

// MD5 hash calculation without reading the whole file contents into RAM
fn file_md5_hash_str(file_path: impl AsRef<Path>) -> std::io::Result<String> {
    let mut file = File::open(&file_path)?;
    let mut md5 = Md5::new();

    std::io::copy(&mut file, &mut md5)?;

    Ok(format!("{:x}", md5.finalize()))
}

fn check_file(
    file_path: impl AsRef<Path>,
    expected_size: u64,
    expected_md5: &str
) -> std::io::Result<bool> {
    if !std::fs::exists(&file_path)? {
        return Ok(false);
    }

    let file_size = std::fs::metadata(&file_path)?.len();

    if file_size != expected_size {
        return Ok(false);
    }

    let file_md5 = file_md5_hash_str(&file_path)?;

    Ok(file_md5 == expected_md5)
}

fn add_user_write_permission_to_file(path: impl AsRef<Path>) -> std::io::Result<()> {
    if !path.as_ref().exists() {
        return Ok(());
    }

    let mut permissions = std::fs::metadata(&path)?.permissions();
    if permissions.readonly() {
        let perm_mode = permissions.mode();
        let user_write_mode = perm_mode | 0o200;
        permissions.set_mode(user_write_mode);
        std::fs::set_permissions(path, permissions)?;
    }

    Ok(())
}

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SophonError {
    /// Specified downloading path is not available in system
    ///
    /// `(path)`
    #[error("Path is not mounted: {0:?}")]
    PathNotMounted(PathBuf),

    /// No free space available under specified path
    #[error("No free space available for specified path: {0:?} (requires {}, available {})", prettify_bytes(*.required), prettify_bytes(*.available))]
    NoSpaceAvailable {
        path: PathBuf,
        required: u64,
        available: u64,
    },

    /// Failed to create or open output file
    #[error("Failed to create output file {path:?}: {message}")]
    OutputFileError {
        path: PathBuf,
        message: String
    },

    /// Failed to create or open temporary output file
    #[error("Failed to create temporary output file {path:?}: {message}")]
    TempFileError {
        path: PathBuf,
        message: String
    },

    /// Couldn't get metadata of existing output file
    ///
    /// This metadata supposed to be used to continue downloading of the file
    #[error("Failed to read metadata of the output file {path:?}: {message}")]
    OutputFileMetadataError {
        path: PathBuf,
        message: String
    },

    /// reqwest error
    #[error("reqwest error: {0}")]
    Reqwest(String),

    #[error("Chunk hash mismatch: expected `{expected}`, got `{got}`")]
    ChunkHashMismatch {
        expected: String,
        got: String
    },

    #[error("File {path:?} hash mismatch: expected `{expected}`, got `{got}`")]
    FileHashMismatch {
        path: PathBuf,
        expected: String,
        got: String,
    },

    #[error("IO error: {0}")]
    IoError(String)
}

impl From<reqwest::Error> for SophonError {
    #[inline(always)]
    fn from(error: reqwest::Error) -> Self {
        Self::Reqwest(error.to_string())
    }
}

impl From<std::io::Error> for SophonError {
    #[inline(always)]
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value.to_string())
    }
}
