use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::iter::Peekable;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use api_schemas::game_branches::GameBranches;
use api_schemas::ApiResponse;
use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use md5::{Digest, Md5};
use protobuf::Message;
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "genshin")]
use crate::genshin;
use crate::prettify_bytes::prettify_bytes;

pub mod api_schemas;
pub mod installer;
pub mod protos;
pub mod repairer;
pub mod updater;

const DEFAULT_CHUNK_RETRIES: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ArtifactDownloadState {
    // Chunk successfully downloaded
    Downloaded,
    // Download failed, run out of retries
    Failed,
    // Amount of retries left, 0 means last retry is being run
    Downloading(u8)
}

impl Default for ArtifactDownloadState {
    #[inline(always)]
    fn default() -> Self {
        Self::Downloading(DEFAULT_CHUNK_RETRIES)
    }
}

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
            Self::Global => {
                concat!("https://", "s", "g-hy", "p-api.", "h", "oy", "over", "se", ".com")
            }
            Self::China => concat!("https://", "hy", "p-api.", "mi", "h", "oyo", ".com")
        }
    }

    #[inline]
    pub fn api_host(&self) -> &str {
        match self {
            Self::Global => concat!(
                "https://",
                "s",
                "g-pu",
                "blic-api.",
                "h",
                "oy",
                "over",
                "se",
                ".com"
            ),
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

struct ThreadQueue<'a, T> {
    global: &'a Injector<T>,
    local: Worker<T>,
    stealers: &'a [Stealer<T>]
}

impl<'a, T> ThreadQueue<'a, T> {
    /// based on the example from crossbeam deque
    fn next_job(&self) -> Option<T> {
        self.local.pop().or_else(|| {
            std::iter::repeat_with(|| {
                self.global
                    .steal_batch_and_pop(&self.local)
                    .or_else(|| self.stealers.iter().map(|s| s.steal()).collect())
            })
            .find(|s| !s.is_retry())
            .and_then(Steal::success)
        })
    }
}

#[derive(Debug)]
struct DownloadQueue<'b, T, I: Iterator<Item = T> + 'b> {
    tasks_iter: Peekable<I>,
    retries_queue: &'b Injector<T>
}

impl<'b, I, T> DownloadQueue<'b, T, I>
where
    I: Iterator<Item = T> + 'b
{
    fn is_empty(&mut self) -> bool {
        self.tasks_iter.peek().is_none() && self.retries_queue.is_empty()
    }
}

impl<'b, I, T> Iterator for DownloadQueue<'b, T, I>
where
    I: Iterator<Item = T> + 'b
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.tasks_iter.next().or_else(|| {
            std::iter::repeat_with(|| self.retries_queue.steal())
                .find(|s| !s.is_retry())
                .and_then(Steal::success)
        })
    }
}

#[inline(always)]
fn get_game_branches_url(edition: GameEdition) -> String {
    format!(
        "{}/hyp/hyp-connect/api/getGameBranches?launcher_id={}",
        edition.branches_host(),
        edition.launcher_id()
    )
}

#[inline(always)]
pub fn get_game_branches_info(
    client: &Client,
    edition: GameEdition
) -> Result<GameBranches, SophonError> {
    api_get_request(client, get_game_branches_url(edition))
}

fn api_get_request<T: DeserializeOwned>(
    client: &Client,
    url: impl AsRef<str>
) -> Result<T, SophonError> {
    let response = client.get(url.as_ref()).send()?.error_for_status()?;

    Ok(response.json::<ApiResponse<T>>()?.data)
}

fn api_post_request<T: DeserializeOwned>(
    client: &Client,
    url: impl AsRef<str>
) -> Result<T, SophonError> {
    let response = client.post(url.as_ref()).send()?.error_for_status()?;

    Ok(response.json::<ApiResponse<T>>()?.data)
}

fn get_protobuf_from_url<T: Message>(
    client: &Client,
    url: impl AsRef<str>,
    compression: bool
) -> Result<T, SophonError> {
    let response = client.get(url.as_ref()).send()?.error_for_status()?;

    let compressed_manifest = response.bytes()?;

    let protobuf_bytes = if compression {
        zstd::decode_all(&*compressed_manifest).unwrap()
    }
    else {
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
    let Ok(fs_metadata) = std::fs::metadata(&file_path)
    else {
        return Ok(false);
    };

    let file_size = fs_metadata.len();

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

fn file_region_hash_md5(file: &mut File, offset: u64, length: u64) -> std::io::Result<String> {
    file.seek(SeekFrom::Start(offset))?;

    let mut region_reader = file.take(length);
    let mut hasher = Md5::new();

    std::io::copy(&mut region_reader, &mut hasher)?;

    Ok(format!("{:x}", hasher.finalize()))
}

// TODO:
// - Cull some variants of SophonError, especially those that are unused
// - Make some better variants describign where the error happened, perhaps steal anyhow's context
//   idea but simpler, especially useful for I/O errors.
// - Cull unused installer/update messages

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
        available: u64
    },

    /// Failed to create or open output file
    #[error("Failed to create output file {path:?}: {message}")]
    OutputFileError { path: PathBuf, message: String },

    /// Failed to create or open temporary output file
    #[error("Failed to create temporary output file {path:?}: {message}")]
    TempFileError { path: PathBuf, message: String },

    /// Couldn't get metadata of existing output file
    ///
    /// This metadata supposed to be used to continue downloading of the file
    #[error("Failed to read metadata of the output file {path:?}: {message}")]
    OutputFileMetadataError { path: PathBuf, message: String },

    /// reqwest error
    #[error("reqwest error: {0}")]
    Reqwest(String),

    #[error("Chunk hash mismatch: expected `{expected}`, got `{got}`")]
    ChunkHashMismatch { expected: String, got: String },

    #[error("File {path:?} hash mismatch: expected `{expected}`, got `{got}`")]
    FileHashMismatch {
        path: PathBuf,
        expected: String,
        got: String
    },

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Failed to download chunk {0}, out of retries")]
    ChunkDownloadFailed(String),

    #[error("Failed to apply hdiff patch: {0}")]
    PatchingError(String)
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
