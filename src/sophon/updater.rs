use std::{collections::{HashMap, HashSet}, sync::{atomic::AtomicU64, Mutex}};
use std::io::{Read, Seek, Take};
use std::fs::File;
use std::path::{Path, PathBuf};

use crossbeam_deque::{Injector, Steal};
use serde::{Serialize, Deserialize};
use reqwest::blocking::Client;

use crate::{
    external::hpatchz,
    prelude::free_space,
    sophon::{add_user_write_permission_to_file, bytes_check_md5, check_file, ChunkState},
    version::Version,
};

// I ain't refactoring it.
use super::{
    api_post_request, api_schemas::{
        game_branches::PackageInfo,
        sophon_diff::{SophonDiff, SophonDiffs}, sophon_manifests::DownloadInfo,
    }, file_md5_hash_str, get_protobuf_from_url, md5_hash_str, protos::{SophonManifest::SophonManifestAssetChunk, SophonPatch::{
        SophonPatchAssetChunk, SophonPatchAssetProperty, SophonPatchProto, SophonUnusedAssetFile, SophonUnusedAssetInfo
    }}, GameEdition, SophonError
};

fn sophon_patch_info_url(
    package_info: &PackageInfo,
    edition: GameEdition
) -> String {
    format!(
        "{}/downloader/sophon_chunk/api/getPatchBuild?branch={}&password={}&package_id={}",
        edition.api_host(),
        package_info.branch,
        package_info.password,
        package_info.package_id
    )
}

#[inline]
pub fn get_game_diffs_sophon_info(
    client: &Client,
    package_info: &PackageInfo,
    edition: GameEdition
) -> Result<SophonDiffs, SophonError> {
    let url = sophon_patch_info_url(
        package_info,
        edition
    );

    api_post_request(client, &url)
}

pub fn get_patch_manifest(
    client: &Client,
    diff_info: &SophonDiff
) -> Result<SophonPatchProto, SophonError> {
    let url_prefix = &diff_info.manifest_download.url_prefix;
    let url_suffix = &diff_info.manifest_download.url_suffix;
    let manifest_id = &diff_info.manifest.id;

    get_protobuf_from_url(
        client,
        format!("{}{}/{}", url_prefix, url_suffix, manifest_id),
        diff_info.manifest_download.compression == 1
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    CheckingFreeSpace(PathBuf),

    DeletingStarted,

    DeletingProgress {
        deleted_files: u64,
        total_unused: u64,
    },

    DeletingFinished,

    /// `(temp path)`
    DownloadingStarted(PathBuf),

    DownloadingProgressBytes {
        downloaded_bytes: u64,
        total_bytes: u64,
    },

    DownloadingFinished,

    PatchingStarted,

    PatchingProgress {
        patched_files: u64,
        total_files: u64,
    },

    PatchingFinished,

    DownloadingError(SophonError),
    PatchingError(String),

    FileHashCheckFailed(PathBuf)
}

struct PatchChunkInfo<'a> {
    /// Grabbed from any file, used for download and check information.
    patch_chunk_manifest: &'a SophonPatchAssetChunk,
    download_info: &'a DownloadInfo,
    used_in_files: Vec<&'a String>
}

impl PatchChunkInfo<'_> {
    fn chunk_path(&self, dir: impl AsRef<Path>) -> PathBuf {
        dir.as_ref().join(&self.patch_chunk_manifest.PatchName)
    }

    fn download_url(&self) -> String {
        self.download_info.download_url(&self.patch_chunk_manifest.PatchName)
    }
}

struct FilePatchInfo<'a> {
    file_manifest: &'a SophonPatchAssetProperty,
    patch_chunk: Option<&'a SophonPatchAssetChunk>,
}

impl FilePatchInfo<'_> {
    /// Path to temporary file to store before patching or as a result of a copy from patch chunk
    fn temp_file_path(&self, temp_dir: impl AsRef<Path>) -> PathBuf {
        temp_dir.as_ref().join(format!("{}.tmp", &self.file_manifest.AssetHashMd5))
    }
    
    /// Path to a temporary file to store patchign output to
    fn temp_file_out_path(&self, temp_dir: impl AsRef<Path>) -> PathBuf {
        temp_dir.as_ref().join(format!("{}.out.tmp", &self.file_manifest.AssetHashMd5))
    }

    /// Path to a target file on filesystem
    fn target_file_path(&self, game_dir: impl AsRef<Path>) -> PathBuf {
        game_dir.as_ref().join(&self.file_manifest.AssetName)
    }

    /// Path to the patch chunk
    fn patch_chunk_path(&self, dir: impl AsRef<Path>) -> Option<PathBuf> {
        Some(dir.as_ref().join(&self.patch_chunk?.PatchName))
    }

    /// Path to the temporary location the patch is stored at
    fn patch_tmp_path(&self, tmpdir: impl AsRef<Path>) -> Option<PathBuf> {
        Some(tmpdir.as_ref().join(format!("{}-{}.hdiff", self.patch_chunk?.PatchName, self.file_manifest.AssetHashMd5)))
    }
}

struct UpdateIndex<'a> {
    unused: &'a SophonUnusedAssetInfo,
    unused_deleted: AtomicU64,
    patch_chunks: HashMap<&'a String, PatchChunkInfo<'a>>,
    total_bytes: u64,
    downloaded_bytes: AtomicU64,
    files_to_patch: HashMap<&'a String, FilePatchInfo<'a>>,
    files_patched: AtomicU64,
}

impl<'a> UpdateIndex<'a> {
    fn new(update_manifest: &'a SophonPatchProto, patch_chunk_download_info: &'a DownloadInfo, from: Version) -> Self {
        let files_to_patch = update_manifest.PatchAssets.iter()
            .map(|spap| {
                (
                    &spap.AssetName,
                    FilePatchInfo {
                        file_manifest: spap,
                        patch_chunk: spap.AssetPatchChunks.iter().find_map(|(fromver, pchunk)| (*fromver == from).then_some(pchunk))
                    }
                )
            }).collect::<HashMap<_, _>>();
        let mut patch_chunks = HashMap::new();
        for file_patch_info in files_to_patch.values() {
            if let Some(patch_info) = &file_patch_info.patch_chunk {
                let patch_chunk_index = patch_chunks
                    .entry(&patch_info.PatchName)
                    .or_insert(PatchChunkInfo {
                        patch_chunk_manifest: patch_info,
                        download_info: patch_chunk_download_info,
                        used_in_files: vec![]
                    });
                patch_chunk_index.used_in_files.push(&file_patch_info.file_manifest.AssetName);
            }
        }

        let total_bytes = Self::total_bytes_calculate(&patch_chunks);

        Self {
            unused: update_manifest.UnusedAssets
                .iter()
                .find_map(|(fromver, unused)| (*fromver == from).then_some(unused)).expect("Unused files are expected"),
            unused_deleted: AtomicU64::new(0),
            patch_chunks,
            total_bytes,
            downloaded_bytes: AtomicU64::new(0),
            files_to_patch,
            files_patched: AtomicU64::new(0),
        }
    }

    // Only build an index of chunks for downlaoding
    fn new_download_only(update_manifest: &'a SophonPatchProto, patch_chunk_download_info: &'a DownloadInfo, from: Version) -> Self {
        let files_to_patch = update_manifest.PatchAssets.iter()
            .map(|spap| {
                    FilePatchInfo {
                        file_manifest: spap,
                        patch_chunk: spap.AssetPatchChunks.iter().find_map(|(fromver, pchunk)| (*fromver == from).then_some(pchunk))
                    }
            }).collect::<Vec<_>>();
        let mut patch_chunks = HashMap::new();
        for file_patch_info in files_to_patch.iter() {
            if let Some(patch_info) = &file_patch_info.patch_chunk {
                let patch_chunk_index = patch_chunks
                    .entry(&patch_info.PatchName)
                    .or_insert(PatchChunkInfo {
                        patch_chunk_manifest: patch_info,
                        download_info: patch_chunk_download_info,
                        used_in_files: vec![]
                    });
                //patch_chunk_index.used_in_files.push(&file_patch_info.file_manifest.AssetName);
            }
        }

        let total_bytes = Self::total_bytes_calculate(&patch_chunks);

        Self {
            unused: update_manifest.UnusedAssets
                .iter()
                .find_map(|(fromver, unused)| (*fromver == from).then_some(unused)).expect("Unused files are expected"),
            unused_deleted: AtomicU64::new(0),
            patch_chunks,
            total_bytes,
            downloaded_bytes: AtomicU64::new(0),
            files_to_patch: HashMap::new(),
            files_patched: AtomicU64::new(0),
        }

    }

    fn total_bytes_calculate(patch_chunks: &HashMap<&'a String, PatchChunkInfo<'a>>) -> u64 {
        patch_chunks.values().map(|pci| pci.patch_chunk_manifest.PatchSize).sum()
    }

    #[inline]
    fn total_files(&self) -> u64 {
        self.files_to_patch.len() as u64
    }

    #[inline]
    fn total_unused(&self) -> u64 {
        self.unused.Assets.len() as u64
    }

    #[inline]
    fn msg_bytes(&self) -> Update {
        Update::DownloadingProgressBytes {
            downloaded_bytes: self.downloaded_bytes.load(std::sync::atomic::Ordering::Acquire),
            total_bytes: self.total_bytes
        }
    }

    #[inline]
    fn msg_files(&self) -> Update {
        Update::PatchingProgress {
            patched_files: self.files_patched.load(std::sync::atomic::Ordering::Acquire),
            total_files: self.total_files()
        }
    }

    #[inline]
    fn msg_deleted(&self) -> Update {
        Update::DeletingProgress {
            deleted_files: self.unused_deleted.load(std::sync::atomic::Ordering::Acquire),
            total_unused: self.total_unused()
        }
    }

    fn count_downloaded(&self, amount: u64) {
        self.downloaded_bytes.fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }

    fn count_deleted(&self, amount: u64) {
        self.unused_deleted.fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }

    fn count_patched(&self, amount: u64) {
        self.files_patched.fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Debug, Clone)]
struct PatchingStats {
    total_bytes: u64,
    downloaded_bytes: u64,
    total_files: u64,
    patched_files: u64,
    total_unused: u64,
    deleted_files: u64,
    downloaded_patch_chunks: HashSet<String>
}

impl PatchingStats {
    fn new(total_bytes: u64, total_files: u64, total_unused: u64, total_patch_chunks: usize) -> Self {
        Self {
            total_bytes,
            total_files,
            total_unused,
            downloaded_bytes: 0,
            patched_files: 0,
            deleted_files: 0,
            downloaded_patch_chunks: HashSet::with_capacity(total_patch_chunks)
        }
    }

    #[inline]
    const fn msg_bytes(&self) -> Update {
        Update::DownloadingProgressBytes {
            downloaded_bytes: self.downloaded_bytes,
            total_bytes: self.total_bytes
        }
    }

    #[inline]
    const fn msg_files(&self) -> Update {
        Update::PatchingProgress {
            patched_files: self.patched_files,
            total_files: self.total_files
        }
    }

    #[inline]
    const fn msg_deleted(&self) -> Update {
        Update::DeletingProgress {
            deleted_files: self.deleted_files,
            total_unused: self.total_unused
        }
    }

    fn count_patch_chunk(&mut self, patch_chunk_info: &SophonPatchAssetChunk) {
        if !self.downloaded_patch_chunks.contains(&patch_chunk_info.PatchName) {
            self.downloaded_bytes += patch_chunk_info.PatchSize;

            self.downloaded_patch_chunks.insert(patch_chunk_info.PatchName.clone());
        }
    }
}

#[derive(Debug)]
pub struct SophonPatcher {
    pub client: Client,
    pub patch_manifest: SophonPatchProto,
    pub diff_info: SophonDiff,
    pub check_free_space: bool,
    pub temp_folder: PathBuf,
}

impl SophonPatcher {
    pub fn new(
        client: Client,
        diff: &SophonDiff,
        temp_dir: impl AsRef<Path>,
    ) -> Result<Self, SophonError> {
        Ok(Self {
            patch_manifest: get_patch_manifest(&client, diff)?,
            client,
            diff_info: diff.clone(),
            check_free_space: true,
            temp_folder: temp_dir.as_ref().to_owned()
        })
    }

    #[inline]
    pub fn with_free_space_check(mut self, check: bool) -> Self {
        self.check_free_space = check;

        self
    }

    #[inline]
    pub fn with_temp_folder(mut self, temp_folder: impl Into<PathBuf>) -> Self {
        self.temp_folder = temp_folder.into();

        self
    }

    /// Folder to temporarily store files being updated (patched, created, etc).
    #[inline]
    pub fn files_temp(&self) -> PathBuf {
        self.temp_folder.join("updating")
    }

    /// Folder to temporarily store hdiff files
    #[inline]
    fn patches_temp(&self) -> PathBuf {
        self.files_temp().join("patches")
    }

    /// Folder to temporarily store downloaded patch chunks
    #[inline]
    fn patch_chunk_temp_folder(&self) -> PathBuf {
        self.files_temp().join("patch_chunks")
    }

    /// Create all needed sub-directories in the temp folder
    fn create_temp_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.files_temp())?;
        std::fs::create_dir_all(self.patches_temp())?;
        std::fs::create_dir_all(self.patch_chunk_temp_folder())?;

        Ok(())
    }

    fn free_space_check(
        updater: impl Fn(Update) + Clone + Send + 'static,
        path: impl AsRef<Path>,
        required: u64
    ) -> Result<(), SophonError> {
        (updater)(Update::CheckingFreeSpace(path.as_ref().to_owned()));

        match free_space::available(&path) {
            Some(space) if space >= required => Ok(()),

            Some(space) => {
                let err = SophonError::NoSpaceAvailable {
                    path: path.as_ref().to_owned(),
                    required,
                    available: space
                };

                (updater)(Update::DownloadingError(err.clone()));

                Err(err)
            }

            None => {
                let err = SophonError::PathNotMounted(path.as_ref().to_owned());

                (updater)(Update::DownloadingError(err.clone()));

                Err(err)
            }
        }
    }

    fn predownload_multithreaded(&self, thread_count: usize, from: Version, updater: impl Fn(Update) + Clone + Send + 'static) {
        tracing::debug!("Starting multithreaded update predownload process");

        let update_index = UpdateIndex::new_download_only(&self.patch_manifest, &self.diff_info.diff_download, from);
        tracing::info!("{} Patch Chunks to download", update_index.patch_chunks.len());
        let chunk_states: Mutex<HashMap<&String, ChunkState>> = Mutex::new(HashMap::from_iter(update_index.patch_chunks.keys().map(|id| (*id, ChunkState::default()))));

        (updater)(update_index.msg_bytes());

        let download_queue: Injector<&PatchChunkInfo> = Injector::new();
        for patch_chunk_info in update_index.patch_chunks.values() {
            download_queue.push(patch_chunk_info);
        }

        let download_check_queue: Injector<&PatchChunkInfo> = Injector::new();

        tracing::debug!("Spawning worker threads");
        std::thread::scope(|scope| {
            for _ in 0..thread_count {
                let updater_clone = updater.clone();
                scope.spawn(|| {
                    let local_updater = move |msg| {
                        (updater_clone)(msg);
                    };
                    // The queues are tried in the inverse order of the pipeline, kind of like a
                    // LIFO queue but shared across threads. Here's teh order the threads do the
                    // tasks in:
                    // 1. Apply patch to a file, if one is available
                    // 2. Check a downloaded patch chunk. If it is correct, schedule the files it
                    //    is used for to be patched. Otherwise, delete the downloaded chunk and
                    //    re-schedule the chunk.
                    // 3. Download a patch chunk.
                    'worker: loop {
                        if let Steal::Success(download_check_task) = download_check_queue.steal() {
                            self.download_check_handler(download_check_task, &chunk_states, &update_index, &local_updater, None, &download_queue);
                            continue;
                        }
                        if let Steal::Success(download_task) = download_queue.steal() {
                            self.download_handler(download_task, &chunk_states, &download_queue, &download_check_queue, &local_updater);
                            continue;
                        }
                        if download_check_queue.is_empty() && download_queue.is_empty() {
                            tracing::debug!("All queues are empty, thread exiting");
                            break 'worker;
                        }
                    }
                });
            }
        });

    }

    fn update_multithreaded(&self, thread_count: usize, game_folder: impl AsRef<Path>, from: Version, updater: impl Fn(Update) + Clone + Send + 'static) {
        tracing::debug!("Starting multithreaded download and update process");

        let update_index = UpdateIndex::new(&self.patch_manifest, &self.diff_info.diff_download, from);
        tracing::info!("{} Patch Chunks to download, {} Files to patch, {} Files to delete", update_index.patch_chunks.len(), update_index.files_to_patch.len(), update_index.unused.Assets.len());
        let chunk_states: Mutex<HashMap<&String, ChunkState>> = Mutex::new(HashMap::from_iter(update_index.patch_chunks.keys().map(|id| (*id, ChunkState::default()))));

        (updater)(update_index.msg_deleted());
        (updater)(update_index.msg_files());
        (updater)(update_index.msg_bytes());

        let download_queue: Injector<&PatchChunkInfo> = Injector::new();
        for patch_chunk_info in update_index.patch_chunks.values() {
            download_queue.push(patch_chunk_info);
        }

        let download_check_queue: Injector<&PatchChunkInfo> = Injector::new();

        let file_patch_queue: Injector<&FilePatchInfo> = Injector::new();

        let game_folder = game_folder.as_ref();

        tracing::debug!("Spawning worker threads");
        // Same as download/install, but the deleted files are going to be deleted in the main
        // thread.
        std::thread::scope(|scope| {
            for _ in 0..thread_count {
                let updater_clone = updater.clone();
                scope.spawn(|| {
                    let local_updater = move |msg| {
                        (updater_clone)(msg);
                    };
                    // The queues are tried in the inverse order of the pipeline, kind of like a
                    // LIFO queue but shared across threads. Here's teh order the threads do the
                    // tasks in:
                    // 1. Apply patch to a file, if one is available
                    // 2. Check a downloaded patch chunk. If it is correct, schedule the files it
                    //    is used for to be patched. Otherwise, delete the downloaded chunk and
                    //    re-schedule the chunk.
                    // 3. Download a patch chunk.
                    'worker: loop {
                        if let Steal::Success(file_patch_task) = file_patch_queue.steal() {
                            self.file_patch_handler(file_patch_task, game_folder, &local_updater);
                            continue;
                        }
                        if let Steal::Success(download_check_task) = download_check_queue.steal() {
                            self.download_check_handler(download_check_task, &chunk_states, &update_index, &local_updater, Some(&file_patch_queue), &download_queue);
                            continue;
                        }
                        if let Steal::Success(download_task) = download_queue.steal() {
                            self.download_handler(download_task, &chunk_states, &download_queue, &download_check_queue, &local_updater);
                            continue;
                        }
                        if file_patch_queue.is_empty() && download_check_queue.is_empty() && download_queue.is_empty() {
                            tracing::debug!("All queues are empty, thread exiting");
                            break 'worker;
                        }
                    }
                });
            }
            // Deleting unused files
            for unused_asset in &update_index.unused.Assets {
                // Ignore any I/O errors
                let _ = std::fs::remove_file(game_folder.join(&unused_asset.FileName));
                update_index.count_deleted(1);
                (updater)(update_index.msg_deleted());
            }
        });
    }

    fn file_patch_handler<'b>(
        &self,
        file_patch_task: &FilePatchInfo,
        game_folder: &Path,
        updater: impl Fn(Update) + 'b
    ) {
        let tmp_file_path = file_patch_task.temp_file_path(self.files_temp());
        let target_file_path = file_patch_task.target_file_path(game_folder);

        // copy file to tmp location if it is going to be actually patched
        if let Some(patch_info) = file_patch_task.patch_chunk {
            if !patch_info.OriginalFileName.is_empty() {
                let src_file_path = game_folder.join(&patch_info.OriginalFileName);
                if let Err(e) = std::fs::copy(&src_file_path, &tmp_file_path) {
                    tracing::error!("Failed to copy file for patching: {e}");
                    (updater)(Update::PatchingError(format!("Failed to create temporary copy of a file: {e}")));
                    return;
                }
            }
        }

        let res = self.file_patch(file_patch_task, &tmp_file_path);

        match res {
            Ok(()) => {
                tracing::debug!("Successfully patched `{}`", file_patch_task.file_manifest.AssetName);
                if let Err(e) = self.move_patched_file(&tmp_file_path, &target_file_path) {
                    tracing::error!("Failed to move patched file into final destination: {e}");
                }
            }
            Err(e) => {
                tracing::error!("Patching for file `{}` failed: {e}", file_patch_task.file_manifest.AssetName);
                (updater)(Update::PatchingError(e.to_string()));
                let _ = std::fs::remove_file(&tmp_file_path);
            }
        }
    }

    fn move_patched_file(&self, tmp_file_path: impl AsRef<Path>, target_file_path: impl AsRef<Path>) -> Result<(), SophonError> {
        std::fs::copy(&tmp_file_path, &target_file_path)?;
        std::fs::remove_file(&tmp_file_path)?;
        Ok(())
    }

    fn file_patch(&self, file_patch_task: &FilePatchInfo, tmp_file_path: impl AsRef<Path>) -> Result<(), SophonError> {
        tracing::trace!("Handling file {}", file_patch_task.file_manifest.AssetName);

        //let target_file_path = target_dir.as_ref().join(&file_patch_task.file_manifest.AssetName);
        let patch_chunk_opt = &file_patch_task.patch_chunk;

        if let Some(patch_chunk) = patch_chunk_opt {
            if patch_chunk.OriginalFileName.is_empty() {
                tracing::trace!("Copying new file `{}`", file_patch_task.file_manifest.AssetName);

                self.copy_over_file_temp(
                    patch_chunk,
                    file_patch_task,
                    &tmp_file_path
                )?;
            }

            else {
                tracing::trace!("Patching `{}`", file_patch_task.file_manifest.AssetName);

                self.apply_file_patch(&tmp_file_path, patch_chunk, file_patch_task)?;
            }

            if !check_file(
                &tmp_file_path,
                file_patch_task.file_manifest.AssetSize,
                &file_patch_task.file_manifest.AssetHashMd5
            )? {
                let file_hash = file_md5_hash_str(&tmp_file_path)?;
                return Err(SophonError::FileHashMismatch {
                    path: tmp_file_path.as_ref().to_owned(),
                    expected: file_patch_task.file_manifest.AssetHashMd5.clone(),
                    got: file_hash
                });
            }
        }

        // Assume files that don't need updating don't need to be checked.
        // In case those files are brok then a repair can be done separately.
        /*
        else {
            tracing::trace!("Just checking file `{}`", target_file_path.display());
            let is_file_valid = check_file(
                &target_file_path,
                file_patch_task.file_manifest.AssetSize,
                &file_patch_task.file_manifest.AssetHashMd5
            )?;

            if !is_file_valid {
                (updater)(Update::FileHashCheckFailed(target_file_path));
            }
        }
        */

        Ok(())
    }

    fn copy_over_file_temp(&self, patch_chunk: &SophonPatchAssetChunk, file_info: &FilePatchInfo, tmp_file_path: impl AsRef<Path>) -> std::io::Result<()> {
        let patch_chunk_path = self.patch_chunk_temp_folder().join(&patch_chunk.PatchName);
        extract_patch_chunk_region_to_file(patch_chunk_path, tmp_file_path, patch_chunk)
    }

    fn apply_file_patch(&self, tmp_file_path: impl AsRef<Path>, patch_chunk_info: &SophonPatchAssetChunk, file_info: &FilePatchInfo) -> Result<(), SophonError> {
        let is_src_valid = check_file(&tmp_file_path, patch_chunk_info.OriginalFileLength, &patch_chunk_info.OriginalFileMd5)?;

        if !is_src_valid {
            let invalid_file_md5 = file_md5_hash_str(&tmp_file_path)?;
            return Err(SophonError::FileHashMismatch {
                path: tmp_file_path.as_ref().to_owned(),
                expected: patch_chunk_info.OriginalFileMd5.clone(),
                got: invalid_file_md5,
            });
        }

        // Safe to unwrap, guaranteed to return `Some` here
        let extracted_patch_path = file_info.patch_tmp_path(self.patches_temp()).unwrap();
        let patch_chunk_file = file_info.patch_chunk_path(self.patch_chunk_temp_folder()).unwrap();

        extract_patch_chunk_region_to_file(&patch_chunk_file, &extracted_patch_path, patch_chunk_info)?;

        let tmp_out_path = file_info.temp_file_out_path(self.files_temp());

        if let Err(err) = hpatchz::patch(tmp_file_path.as_ref(), &extracted_patch_path, &tmp_out_path) {
            tracing::error!("Patching error: {err}");
            return Err(SophonError::PatchingError(err.to_string()));
        }

        tracing::trace!("Checking after patching");

        let is_out_file_valid = check_file(
            &tmp_out_path,
            file_info.file_manifest.AssetSize,
            &file_info.file_manifest.AssetHashMd5
        )?;

        if !is_out_file_valid {
            let invalid_file_hash = file_md5_hash_str(&tmp_out_path)?;

            return Err(SophonError::FileHashMismatch {
                path: tmp_out_path,
                expected: file_info.file_manifest.AssetHashMd5.clone(),
                got: invalid_file_hash
            });
        }

        tracing::trace!("File valid, replacing tmp file with it");

        std::fs::copy(&tmp_out_path, &tmp_file_path)?;
        std::fs::remove_file(&tmp_file_path)?;

        Ok(())
    }

    fn download_check_handler<'a, 'b>(
        &self,
        download_check_task: &'a PatchChunkInfo<'a>,
        chunk_states: &'b Mutex<HashMap<&'a String, ChunkState>>,
        update_index: &'a UpdateIndex<'a>,
        updater: impl Fn(Update) + 'b,
        file_patch_queue: Option<&'b Injector<&'a FilePatchInfo<'a>>>,
        download_queue: &'b Injector<&'a PatchChunkInfo<'a>>)
    {
        let res = self.check_downloaded_chunk(download_check_task);
        match res {
            Ok(true) => {
                tracing::trace!("Successfully downloaded chunk `{}`", download_check_task.patch_chunk_manifest.PatchName);
                {
                    let mut states_lock = chunk_states.lock().unwrap();
                    let chunk_state = states_lock.get_mut(&download_check_task.patch_chunk_manifest.PatchName).unwrap();
                    *chunk_state = ChunkState::Downloaded;
                }
                if let Some(file_patch_queue) = file_patch_queue {
                    for file_name in &download_check_task.used_in_files {
                        if let Some(file_info) = update_index.files_to_patch.get(*file_name) {
                            tracing::trace!("File `{}` is ready to patch, pushing to queue", file_name);
                            file_patch_queue.push(file_info);
                        }
                    }
                }
            },
            Ok(false) => {
                tracing::trace!("Chunk `{}` failed size+hash check", download_check_task.patch_chunk_manifest.PatchName);
                self.fail_chunk(download_check_task, chunk_states, download_queue, updater);
            },
            Err(err) => {
                tracing::error!("I/O error checking chunk `{}`: {err}", download_check_task.patch_chunk_manifest.PatchName);
                (updater)(Update::DownloadingError(err.into()));
                self.fail_chunk(download_check_task, chunk_states, download_queue, updater);
            }
        }
    }

    fn check_downloaded_chunk(&self, chunk_info: &PatchChunkInfo) -> std::io::Result<bool> {
        let chunk_path = chunk_info.chunk_path(self.patch_chunk_temp_folder());
        check_file(&chunk_path, chunk_info.patch_chunk_manifest.PatchSize, &chunk_info.patch_chunk_manifest.PatchMd5)
    }

    fn fail_chunk<'a>(&self, chunk_info: &'a PatchChunkInfo<'a>, states: &Mutex<HashMap<&'a String, ChunkState>>, download_queue: &Injector<&'a PatchChunkInfo<'a>>, updater: impl Fn(Update)) {
        // Check/download failed, file corrupt, so try to delete it.
        let chunk_path = chunk_info.chunk_path(&self.patch_chunk_temp_folder());
        let _ = std::fs::remove_file(&chunk_path);
        {
            let mut states_lock = states.lock().unwrap();
            let chunk_state = states_lock.get_mut(&chunk_info.patch_chunk_manifest.PatchName).unwrap();
            match chunk_state {
                ChunkState::Downloading(0) => {
                    *chunk_state = ChunkState::Failed;
                    (updater)(Update::DownloadingError(SophonError::ChunkDownloadFailed(chunk_info.patch_chunk_manifest.PatchName.clone())))
                },
                ChunkState::Downloading(n) => {
                    *n -= 1;
                    download_queue.push(chunk_info);
                },
                // Why is chunk being checked if it's not being
                // downloaded?
                _ => { unreachable!() }
            }
        }
    }

    fn download_handler<'a, 'b>(
        &self,
        download_task: &'a PatchChunkInfo<'a>,
        chunk_states: &'b Mutex<HashMap<&'a String, ChunkState>>,
        download_queue: &'b Injector<&'a PatchChunkInfo<'a>>,
        download_check_queue: &'b Injector<&'a PatchChunkInfo<'a>>,
        updater: impl Fn(Update) + 'b
    ) {
        let res = self.download_patch_chunk_nocheck(download_task);
        match res {
            Ok(()) => {
                download_check_queue.push(download_task);
            },
            Err(err) => {
                tracing::error!("Error downloading patch chunk `{}`: {err}", download_task.patch_chunk_manifest.PatchName);
                (updater)(Update::DownloadingError(err));
                self.fail_chunk(download_task, chunk_states, download_queue, updater);
            }
        }
    }

    fn download_patch_chunk_nocheck(&self, download_task: &PatchChunkInfo) -> Result<(), SophonError> {
        let chunk_path = download_task.chunk_path(self.patch_chunk_temp_folder());

        if check_file(&chunk_path, download_task.patch_chunk_manifest.PatchSize, &download_task.patch_chunk_manifest.PatchMd5)? {
            Ok(())
        } else {
            let chunk_url = download_task.download_url();

            let response = self.client.get(&chunk_url)
                .send()?
                .error_for_status()?;

            let chunk_bytes = response.bytes()?;

            std::fs::write(&chunk_path, &chunk_bytes)?;

            Ok(())
        }

    }

    pub fn pre_download(
        &self,
        from: Version,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {

        if self.check_free_space {
            let download_bytes = self.diff_info.stats.get(&from.to_string()).unwrap()
                .compressed_size.parse().unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_bytes)?;
        }

        self.create_temp_dirs()?;

        (updater)(Update::DownloadingStarted(self.temp_folder.clone()));

        self.predownload_multithreaded(14, from, updater.clone());

        (updater)(Update::DownloadingFinished);

        Ok(())
    }

    pub fn sophon_apply_patches(
        &self,
        target_dir: impl AsRef<Path>,
        from: Version,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        if self.check_free_space {
            let download_bytes = self.diff_info.stats.get(&from.to_string()).unwrap()
                .compressed_size.parse().unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_bytes)?;
        }

        self.create_temp_dirs()?;

        (updater)(Update::PatchingStarted);

        self.update_multithreaded(14, target_dir, from, updater.clone());

        (updater)(Update::PatchingFinished);

        Ok(())
    }

    fn patch_file_updater_handler(
        &self,
        file_patch_info: &SophonPatchAssetProperty,
        target_dir: impl AsRef<Path>,
        installed_ver: Version,
        progress: &mut PatchingStats,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) {
        let result = self.sophon_patch_file(
            file_patch_info,
            &target_dir,
            installed_ver,
            progress,
            updater.clone()
        );

        if let Err(err) = result {
            tracing::error!("Error {err:?} with `{}`", file_patch_info.AssetName);

            (updater)(Update::DownloadingError(err));

            tracing::trace!("the asset patching info: {file_patch_info:?}");
        }

        else {
            progress.patched_files += 1;

            (updater)(progress.msg_files());
        }
    }

    fn sophon_patch_file(
        &self,
        patch_info: &SophonPatchAssetProperty,
        target_dir: impl AsRef<Path>,
        installed_ver: Version,
        progress: &mut PatchingStats,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        tracing::trace!("Handling file {}", patch_info.AssetName);

        let target_file_path = target_dir.as_ref().join(&patch_info.AssetName);
        let asset_info = patch_info.AssetPatchChunks.get(&installed_ver.to_string());

        if let Some(patch_chunk) = asset_info {
            if patch_chunk.OriginalFileName.is_empty() {
                tracing::trace!("Copying new file `{}`", patch_info.AssetName);

                self.copy_over_file(
                    target_file_path,
                    patch_chunk,
                    patch_info.AssetSize,
       &patch_info.AssetHashMd5,
                    progress,
                    updater.clone()
                )?;
            }

            else {
                let source_file_path = target_dir.as_ref().join(&patch_chunk.OriginalFileName);

                if source_file_path == target_file_path {
                    tracing::trace!("Patching `{}`", target_file_path.display());
                }

                else {
                    tracing::trace!(
                        "Patching `{}` => `{}`",
                        source_file_path.display(),
                        target_file_path.display()
                    )
                }

                self.actually_patch_file(
                    target_file_path,
                    source_file_path,
                    patch_info,
                    patch_chunk,
                    progress,
                    updater.clone()
                )?;
            }
        }

        else {
            tracing::trace!("Just checking file `{}`", target_file_path.display());
            let valid_file = check_file(
                &target_file_path,
                patch_info.AssetSize,
                &patch_info.AssetHashMd5
            )?;

            if !valid_file {
                (updater)(Update::FileHashCheckFailed(target_file_path));
            }
        }

        Ok(())
    }

    fn actually_patch_file(
        &self,
        to: impl AsRef<Path>,
        from: impl AsRef<Path>,
        asset_info: &SophonPatchAssetProperty,
        patch_chunk: &SophonPatchAssetChunk,
        progress: &mut PatchingStats,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        let valid_file = check_file(
            &from,
            patch_chunk.OriginalFileLength,
            &patch_chunk.OriginalFileMd5,
        )?;

        if !valid_file {
            (updater)(Update::FileHashCheckFailed(from.as_ref().to_owned()));
        }

        let patch_chunk_file = self.download_patch_chunk(patch_chunk, progress, updater.clone())?;

        let patch_path_tmp = self.patches_temp().join(
            format!("{}-{}.hdiff",patch_chunk.OriginalFileMd5, asset_info.AssetHashMd5)
        );

        extract_patch_chunk_region_to_file(patch_chunk_file, &patch_path_tmp, patch_chunk)?;

        let tmp_out_file_path = self.files_temp()
            .join(format!("{}.tmp", &asset_info.AssetHashMd5));

        if let Err(err) = hpatchz::patch(from.as_ref(), &patch_path_tmp, &tmp_out_file_path) {
            (updater)(Update::PatchingError(err.to_string()));

            return Ok(());
        }

        tracing::trace!("Checking post-patch");

        let valid_file = check_file(
            &tmp_out_file_path,
            asset_info.AssetSize,
            &asset_info.AssetHashMd5
        )?;

        if !valid_file {
            (updater)(Update::FileHashCheckFailed(tmp_out_file_path.clone()));

            let file_hash = file_md5_hash_str(&tmp_out_file_path)?;

            return Err(SophonError::FileHashMismatch {
                path: tmp_out_file_path,
                expected: asset_info.AssetHashMd5.clone(),
                got: file_hash
            });
        }

        // Delete original if patching is also a move
        if asset_info.AssetName != patch_chunk.OriginalFileName {
            tracing::trace!("Pactching is a move, deleting original");

            std::fs::remove_file(from)?;
        }

        tracing::trace!("Tweaking permissions for copy");

        add_user_write_permission_to_file(&to)?;

        tracing::trace!("Copying `{tmp_out_file_path:?}` to `{:?}`", to.as_ref());

        std::fs::copy(&tmp_out_file_path, &to)?;

        // tmp file was checked, doesn't need to be checked after copy
        tracing::trace!("Removing tmp output {tmp_out_file_path:?}");

        std::fs::remove_file(&tmp_out_file_path)?;

        tracing::trace!("Removing tmp patch {patch_path_tmp:?}");

        std::fs::remove_file(&patch_path_tmp)?;

        Ok(())
    }

    fn copy_over_file(
        &self,
        out_file_path: impl AsRef<Path>,
        patch_chunk: &SophonPatchAssetChunk,
        expected_size: u64,
        expected_md5: &str,
        progress: &mut PatchingStats,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        let patch_chunk_path = self.download_patch_chunk(patch_chunk, progress, updater.clone())?;

        let tmp_file_path = self.files_temp().join(format!("{}.tmp", expected_md5));

        extract_patch_chunk_region_to_file(&patch_chunk_path, &tmp_file_path, patch_chunk)?;

        if !check_file(&tmp_file_path, expected_size, expected_md5)? {
            (updater)(Update::FileHashCheckFailed(tmp_file_path.clone()));

            let file_hash = file_md5_hash_str(&tmp_file_path)?;

            Err(SophonError::FileHashMismatch {
                path: tmp_file_path,
                expected: expected_md5.to_owned(),
                got: file_hash
            })
        }

        else {
            std::fs::copy(&tmp_file_path, &out_file_path)?;
            std::fs::remove_file(tmp_file_path)?;

            Ok(())
        }
    }

    fn remove_unused_files(
        &self,
        unused_assets: &[SophonUnusedAssetFile],
        target_dir: impl AsRef<Path>,
        progress: &mut PatchingStats,
        updater: impl Fn(Update) + Send + 'static
    ) -> Result<(), SophonError> {
        for unused in unused_assets {
            tracing::trace!("Unused file `{}`", &unused.FileName);

            let file_path = target_dir.as_ref().join(&unused.FileName);

            if check_file(&file_path, unused.FileSize, &unused.FileMd5)? && std::fs::exists(&file_path)? {
                let _ = std::fs::remove_file(file_path);
            }

            progress.deleted_files += 1;

            (updater)(progress.msg_deleted());
        }

        Ok(())
    }

    #[inline]
    fn patch_chunk_download_url(&self, patch_chunk_id: &str) -> String {
        format!("{}{}/{patch_chunk_id}",self.diff_info.diff_download.url_prefix, self.diff_info.diff_download.url_suffix)
    }

    // Assumes patch chunks are not compressed, ignores the `compression` field.
    // As such, no split is needed. Just download the patch.
    // Checks teh temp dir for an already downloaded patch chunk and returns that path if it passes
    // the checks.
    fn download_patch_chunk(
        &self,
        patch_chunk_info: &SophonPatchAssetChunk,
        progress: &mut PatchingStats,
        updater: impl Fn(Update) + Send + 'static,
    ) -> Result<PathBuf, SophonError> {
        let patch_path = self.patch_chunk_temp_folder().join(format!("{}.patch_chunk", patch_chunk_info.PatchName));

        let valid_file = check_file(
            &patch_path,
            patch_chunk_info.PatchSize,
            &patch_chunk_info.PatchMd5,
        )?;

        if valid_file {
            progress.count_patch_chunk(patch_chunk_info);

            (updater)(progress.msg_bytes());

            Ok(patch_path)
        }

        else {
            let patch_chunk_url = self.patch_chunk_download_url(&patch_chunk_info.PatchName);

            let response = self.client.get(&patch_chunk_url)
                .send()?
                .error_for_status()?;

            let patch_chunk_bytes = response.bytes()?;

            if patch_chunk_bytes.len() as u64 == patch_chunk_info.PatchSize && bytes_check_md5(&patch_chunk_bytes, &patch_chunk_info.PatchMd5) {
                std::fs::write(&patch_path, &patch_chunk_bytes)?;

                progress.count_patch_chunk(patch_chunk_info);

                (updater)(progress.msg_bytes());

                Ok(patch_path)
            }

            else {
                Err(SophonError::ChunkHashMismatch {
                    expected: patch_chunk_info.PatchMd5.clone(),
                    got: md5_hash_str(&patch_chunk_bytes),
                })
            }
        }
    }
}

fn extract_patch_chunk_region(
    patch_chunk: impl AsRef<Path>,
    offset: u64,
    length: u64
) -> std::io::Result<Take<File>> {
    let mut file = File::open(patch_chunk)?;

    file.seek(std::io::SeekFrom::Start(offset))?;

    Ok(file.take(length))
}

fn extract_patch_chunk_region_to_file(
    patch_chunk_file: impl AsRef<Path>,
    out_path: impl AsRef<Path>,
    patch_chunk: &SophonPatchAssetChunk
) -> std::io::Result<()> {
    let mut patch_data = extract_patch_chunk_region(
        patch_chunk_file,
        patch_chunk.PatchOffset,
        patch_chunk.PatchLength,
    )?;

    let mut patch_file = File::create(out_path)?;

    std::io::copy(&mut patch_data, &mut patch_file)?;

    Ok(())
}
