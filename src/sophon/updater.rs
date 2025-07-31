use std::collections::HashMap;
use std::time::Duration;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::iter;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Condvar, Mutex, MutexGuard};

use crossbeam_deque::{Injector, Worker};
use reqwest::blocking::Client;
use reqwest::header::RANGE;
use serde::{Deserialize, Serialize};

use super::api_schemas::game_branches::PackageInfo;
use super::api_schemas::sophon_diff::{SophonDiff, SophonDiffs};
use super::api_schemas::sophon_manifests::DownloadInfo;
use super::protos::SophonPatch::{
    SophonPatchAssetChunk, SophonPatchAssetProperty, SophonPatchProto, SophonUnusedAssetInfo
};
use super::{
    ArtifactDownloadState, DEFAULT_CHUNK_RETRIES, DownloadQueue, GameEdition, SophonError,
    ThreadQueue, add_user_write_permission_to_file, api_post_request, check_file, ensure_parent,
    file_md5_hash_str, get_protobuf_from_url
};
use crate::external::hpatchz;
use crate::prelude::{free_space, prettify_bytes};
use crate::version::Version;

fn sophon_patch_info_url(package_info: &PackageInfo, edition: GameEdition) -> String {
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
    let url = sophon_patch_info_url(package_info, edition);

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
        format!("{url_prefix}{url_suffix}/{manifest_id}"),
        diff_info.manifest_download.compression == 1
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    CheckingFreeSpace(PathBuf),

    DeletingStarted,

    DeletingProgress {
        deleted_files: u64,
        total_unused: u64
    },

    DeletingFinished,

    /// `(temp path)`
    DownloadingStarted(PathBuf),

    DownloadingProgressBytes {
        downloaded_bytes: u64,
        total_bytes: u64
    },

    DownloadingFinished,

    PatchingStarted,

    PatchingProgress {
        patched_files: u64,
        total_files: u64
    },

    PatchingFinished,

    DownloadingError(SophonError),
    PatchingError(String),

    FileHashCheckFailed(PathBuf)
}

#[derive(Debug)]
struct FilePatchInfo<'a> {
    file_manifest: &'a SophonPatchAssetProperty,
    patch_chunk: &'a SophonPatchAssetChunk,
    patch_chunk_download_info: &'a DownloadInfo
}

impl FilePatchInfo<'_> {
    /// Path to a target file on filesystem
    fn target_file_path(&self, game_dir: impl AsRef<Path>) -> PathBuf {
        game_dir.as_ref().join(&self.file_manifest.AssetName)
    }

    fn orig_file_path(&self, game_dir: impl AsRef<Path>) -> Option<PathBuf> {
        if !self.is_patch() {
            None
        }
        else {
            Some(game_dir.as_ref().join(&self.patch_chunk.OriginalFileName))
        }
    }

    /// Path to temporary file to store before patching or as a result of a copy
    /// from patch chunk
    fn tmp_src_filename(&self) -> String {
        format!("{}.tmp", &self.file_manifest.AssetHashMd5)
    }

    /// Path to a temporary file to store patching output to
    fn tmp_out_filename(&self) -> String {
        format!("{}.tmp.out", &self.file_manifest.AssetHashMd5)
    }

    /// Get filename for whatever artifact is needed to patch this file.
    /// it's either an hdiff patch file or a plain blob that needs to be copied
    /// as the entire contents of the new file.
    fn artifact_filename(&self) -> String {
        if self.is_patch() {
            format!(
                "{}-{}.hdiff",
                self.patch_chunk.PatchName, self.file_manifest.AssetHashMd5
            )
        }
        else {
            format!("{}.bin", self.file_manifest.AssetHashMd5)
        }
    }

    /// Returns true if the file is updated by patching.
    /// Returns false if the file is simply copied from the chunk.
    const fn is_patch(&self) -> bool {
        !self.patch_chunk.OriginalFileName.is_empty()
    }

    /// Value for a Range header for downloading the file
    fn download_range(&self) -> String {
        format!(
            "bytes={}-{}",
            self.patch_chunk.PatchOffset,
            self.patch_chunk.PatchOffset + self.patch_chunk.PatchLength - 1
        )
    }

    fn download_url(&self) -> String {
        self.patch_chunk_download_info
            .download_url(&self.patch_chunk.PatchName)
    }
}

#[derive(Debug)]
struct UpdateIndex<'a> {
    unused: Option<&'a SophonUnusedAssetInfo>,
    unused_deleted: AtomicU64,
    total_bytes: u64,
    downloaded_bytes: AtomicU64,
    files_to_patch: HashMap<&'a String, FilePatchInfo<'a>>,
    files_patched: AtomicU64,
    download_states: Mutex<HashMap<&'a String, ArtifactDownloadState>>,
    download_states_notifier: Condvar
}

impl<'a> UpdateIndex<'a> {
    fn new(
        update_manifest: &'a SophonPatchProto,
        patch_chunk_download_info: &'a DownloadInfo,
        from: Version
    ) -> Self {
        let files_to_patch = update_manifest
            .PatchAssets
            .iter()
            .filter_map(|spap| {
                Some((&spap.AssetName, FilePatchInfo {
                    file_manifest: spap,
                    patch_chunk_download_info,
                    patch_chunk: spap
                        .AssetPatchChunks
                        .iter()
                        .find_map(|(fromver, pchunk)| (*fromver == from).then_some(pchunk))?
                }))
            })
            .collect::<HashMap<_, _>>();

        // use hashmap to deduplicate the chunks
        let mut patch_chunks_map = HashMap::new();
        for file_info in files_to_patch.values() {
            if !patch_chunks_map.contains_key(&file_info.patch_chunk.PatchName) {
                patch_chunks_map.insert(
                    &file_info.patch_chunk.PatchName,
                    file_info.patch_chunk.PatchSize
                );
            }
        }
        let total_bytes = patch_chunks_map.values().sum();

        Self {
            download_states: Mutex::new(
                files_to_patch
                    .keys()
                    .map(|asset_name| {
                        (
                            *asset_name,
                            ArtifactDownloadState::Downloading(DEFAULT_CHUNK_RETRIES)
                        )
                    })
                    .collect()
            ),
            download_states_notifier: Condvar::new(),
            unused: update_manifest
                .UnusedAssets
                .iter()
                .find_map(|(fromver, unused)| (*fromver == from).then_some(unused)),
            unused_deleted: AtomicU64::new(0),
            total_bytes,
            downloaded_bytes: AtomicU64::new(0),
            files_to_patch,
            files_patched: AtomicU64::new(0)
        }
    }

    #[inline]
    fn total_files(&self) -> u64 {
        self.files_to_patch.len() as u64
    }

    #[inline]
    fn total_unused(&self) -> u64 {
        self.unused.map(|una| una.Assets.len()).unwrap_or(0) as u64
    }

    #[inline]
    fn msg_bytes(&self) -> Update {
        Update::DownloadingProgressBytes {
            downloaded_bytes: self
                .downloaded_bytes
                .load(std::sync::atomic::Ordering::Acquire),
            total_bytes: self.total_bytes
        }
    }

    #[inline]
    fn msg_patched(&self) -> Update {
        Update::PatchingProgress {
            patched_files: self
                .files_patched
                .load(std::sync::atomic::Ordering::Acquire),
            total_files: self.total_files()
        }
    }

    #[inline]
    fn msg_deleted(&self) -> Update {
        Update::DeletingProgress {
            deleted_files: self
                .unused_deleted
                .load(std::sync::atomic::Ordering::Acquire),
            total_unused: self.total_unused()
        }
    }

    fn count_downloaded(&self, amount: u64) {
        self.downloaded_bytes
            .fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }

    fn count_deleted(&self, amount: u64) {
        self.unused_deleted
            .fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }

    fn count_patched(&self, amount: u64) {
        self.files_patched
            .fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }

    /// Process chunk download failure. Either pushes the chunk onto the retries
    /// queue or sends the chunk download fail update message using the
    /// updater. Refer to [Self::count_chunk_fail] for more info.
    fn process_download_fail<'b>(
        &self,
        file: &'a FilePatchInfo<'a>,
        retries_queue: &'b Injector<&'a FilePatchInfo<'a>>,
        updater: impl Fn(Update) + 'b
    ) {
        match self.count_download_fail(&file.file_manifest.AssetName) {
            Ok(()) => retries_queue.push(file),
            Err(msg) => (updater)(msg)
        }
    }

    /// A download attempt or check failed, decrement the retry count or report
    /// the chunk as completely failed.
    /// The error type is a message to emit in case of a completely faield chunk
    /// download. If this returns Ok, push the chunk on the retries queue.
    /// If this returns Err, emit the fail message for this chunk and stop
    /// retrying.
    fn count_download_fail(&self, artifact_name: &'a String) -> Result<(), Update> {
        let mut states_lock = self
            .download_states
            .lock()
            .expect("Something poisoned the lock");
        let chunk_state = states_lock
            .get_mut(artifact_name)
            .expect("Attempt to count fail of an artifact that was not in the states map");
        match chunk_state {
            ArtifactDownloadState::Downloading(0) => {
                *chunk_state = ArtifactDownloadState::Failed;
                Err(Update::DownloadingError(SophonError::ChunkDownloadFailed(
                    artifact_name.clone()
                )))
            }
            ArtifactDownloadState::Downloading(n) => {
                *n -= 1;
                Ok(())
            }
            _ => {
                unreachable!(
                    "The artifact download can't fail after the artifact is already downloaded or failed"
                )
            }
        }
    }

    fn artifact_success(&self, artifact_name: &'a String) {
        let mut guard = self
            .download_states
            .lock()
            .expect("Something poisoned the lock");
        *guard
            .get_mut(artifact_name)
            .expect("All artifacts must be added to the state tracker") =
            ArtifactDownloadState::Downloaded;
    }

    /// Returns true to continue downloading, false to exit the loop.
    /// if anything is still downloading, uses the [`Condvar`] to wait until the
    /// states are updated by patching/checking threads and checks again.
    fn wait_downloading(&self) -> bool {
        let mut guard = self
            .download_states
            .lock()
            .expect("Something poisoned the mutex");

        if !Self::any_downloading(&guard) {
            tracing::debug!("All artifacts marked as downloaded or failed, breaking the loop");

            return false;
        }

        // unlocks the mutex during wait, see [`Condvar::wait_timeout`]
        // timeout 10s
        guard = self
            .download_states_notifier
            .wait_timeout(guard, Duration::from_secs(10))
            .expect("Something poisoned the mutex")
            .0;

        Self::any_downloading(&guard)
    }

    fn any_downloading(guard: &MutexGuard<HashMap<&'a String, ArtifactDownloadState>>) -> bool {
        guard
            .values()
            .any(|state| matches!(state, ArtifactDownloadState::Downloading(_)))
    }
}

type DownloadPatchQueue<'a, 'b, I> = DownloadQueue<'b, &'a FilePatchInfo<'a>, I>;

#[derive(Debug)]
pub struct SophonPatcher {
    pub client: Client,
    pub patch_manifest: SophonPatchProto,
    pub diff_info: SophonDiff,
    pub check_free_space: bool,
    pub temp_folder: PathBuf
}

impl SophonPatcher {
    pub fn new(
        client: Client,
        diff: &SophonDiff,
        temp_dir: impl AsRef<Path>
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

    pub fn update(
        &self,
        target_dir: impl AsRef<Path>,
        from: Version,
        thread_count: usize,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        if self.check_free_space {
            let download_bytes = self
                .diff_info
                .stats
                .get(&from.to_string())
                .unwrap()
                .compressed_size
                .parse()
                .unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_bytes)?;
        }

        self.create_temp_dirs()?;

        self.update_multithreaded(thread_count, target_dir, from, updater.clone());

        Ok(())
    }

    fn update_multithreaded(
        &self,
        thread_count: usize,
        game_folder: impl AsRef<Path>,
        from: Version,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) {
        let update_index =
            UpdateIndex::new(&self.patch_manifest, &self.diff_info.diff_download, from);

        tracing::info!(
            total_bytes = prettify_bytes(update_index.total_bytes),
            total_files = update_index.total_files(),
            delete_files = update_index.total_unused(),
            "Starting multi-thread updater"
        );

        (updater)(update_index.msg_deleted());
        (updater)(update_index.msg_patched());
        (updater)(update_index.msg_bytes());

        let retries_queue = Injector::<&FilePatchInfo>::new();

        let file_patch_queue = Injector::<&FilePatchInfo>::new();

        let game_folder = game_folder.as_ref();

        let worker_queues = iter::repeat_with(Worker::new_fifo)
            .take(thread_count)
            .collect::<Vec<_>>();

        let stealers = worker_queues
            .iter()
            .map(|w| w.stealer())
            .collect::<Vec<_>>();

        tracing::debug!("Spawning worker threads");

        // Same as download/install, but the deleted files are going to be
        // deleted in the main thread.
        std::thread::scope(|scope| {
            // downlaoder thread
            let updater_clone = updater.clone();

            let download_queue = DownloadQueue {
                tasks_iter: update_index.files_to_patch.values().peekable(),
                retries_queue: &retries_queue
            };

            scope.spawn(|| {
                let _span = tracing::trace_span!("Download thread").entered();

                (updater_clone)(Update::DownloadingStarted(self.temp_folder.clone()));

                self.artifact_download_loop(
                    download_queue,
                    &file_patch_queue,
                    &update_index,
                    updater_clone
                );
            });

            // Patching threads
            for (i, worker_queue) in worker_queues.into_iter().enumerate() {
                let updater_clone = updater.clone();

                let thread_queue = ThreadQueue {
                    global: &file_patch_queue,
                    local: worker_queue,
                    stealers: &stealers
                };

                let index_ref = &update_index;
                let retries_ref = &retries_queue;

                scope.spawn(move || {
                    let _span = tracing::debug_span!("Patching thread", thread_id = i).entered();

                    self.file_patch_loop(
                        game_folder,
                        updater_clone,
                        index_ref,
                        retries_ref,
                        thread_queue
                    );
                });
            }

            // Unused file deletion - in main thread
            if let Some(unused) = &update_index.unused {
                let _deleting_unused_span =
                    tracing::trace_span!("Deleting unused", amount = unused.Assets.len()).entered();

                // Deleting unused files
                for unused_asset in &unused.Assets {
                    // Ignore any I/O errors
                    let _ = std::fs::remove_file(game_folder.join(&unused_asset.FileName));

                    update_index.count_deleted(1);

                    (updater)(update_index.msg_deleted());
                }

                (updater)(Update::DeletingFinished);
            }
        });

        (updater)(Update::PatchingFinished);
    }

    pub fn pre_download(
        &self,
        from: Version,
        thread_count: usize,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        if self.check_free_space {
            let download_bytes = self
                .diff_info
                .stats
                .get(&from.to_string())
                .unwrap()
                .compressed_size
                .parse()
                .unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_bytes)?;
        }

        self.create_temp_dirs()?;

        self.predownload_multithreaded(thread_count, from, updater.clone());

        let marker_file_path = self.files_temp().join(".predownloadcomplete");
        File::create(marker_file_path)?;

        Ok(())
    }

    fn predownload_multithreaded(
        &self,
        _thread_count: usize,
        from: Version,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) {
        tracing::debug!("Starting multithreaded update predownload process");

        let update_index =
            UpdateIndex::new(&self.patch_manifest, &self.diff_info.diff_download, from);

        tracing::info!(
            "{} files to download, {} download total",
            update_index.files_to_patch.len(),
            prettify_bytes(update_index.total_bytes)
        );

        (updater)(update_index.msg_bytes());

        let retries_queue = Injector::<&FilePatchInfo>::new();
        let patch_queue = Injector::<&FilePatchInfo>::new();

        tracing::debug!("Starting download");

        std::thread::scope(|scope| {
            let updater_clone = updater.clone();

            let download_queue = DownloadQueue {
                tasks_iter: update_index.files_to_patch.values().peekable(),
                retries_queue: &retries_queue
            };

            scope.spawn(|| {
                let _span = tracing::trace_span!("Download thread").entered();

                (updater_clone)(Update::DownloadingStarted(self.temp_folder.clone()));

                self.artifact_download_loop(
                    download_queue,
                    &patch_queue,
                    &update_index,
                    updater_clone
                );
            });

            // no reason to spawn multiple of this, teh downloader itself is
            // basically single-threaded too
            let updater_clone = updater.clone();
            scope.spawn(|| {
                let local_updater = updater_clone;
                let _span = tracing::trace_span!("Verification thread").entered();

                'worker: loop {
                    if let Some(task) = patch_queue.steal().success() {
                        // This is predownload, assume the files are downlaoded successfully, no
                        // checks done other than those in the download handler. The full update
                        // process will redownload files that are broken.
                        update_index.artifact_success(&task.file_manifest.AssetName);
                        update_index.count_downloaded(task.patch_chunk.PatchLength);
                        (local_updater)(update_index.msg_bytes());

                        update_index.download_states_notifier.notify_all();
                    }
                    else if patch_queue.is_empty() && !update_index.wait_downloading() {
                        break 'worker;
                    }
                }

                update_index.download_states_notifier.notify_all();
            });
        });

        (updater)(Update::DownloadingFinished);
    }

    /// Loops over the tasks and retries and tries to download them, pushing
    /// onto the patch queue if the download succeedes. If both the tasks
    /// iterator and the retries queues return nothing, checks if they are empty
    /// and then checks if there are any unfinished patches and waits for either
    /// all patches to finish applying or a new retry being pushed onto the
    /// queue.
    fn artifact_download_loop<'a, 'b, I: Iterator<Item = &'a FilePatchInfo<'a>> + 'b>(
        &self,
        mut task_queue: DownloadPatchQueue<'a, 'b, I>,
        patch_queue: &'b Injector<&'a FilePatchInfo<'a>>,
        update_index: &'b UpdateIndex<'a>,
        updater: impl Fn(Update) + 'b
    ) {
        loop {
            if let Some(task) = task_queue.next() {
                // Check if the file already exists on disk and if it does,
                // skip re-downloading it
                let artifact_path = self.tmp_artifact_file_path(task);

                let res = if artifact_path.exists() {
                    tracing::debug!(
                        artifact = ?artifact_path,
                        "Artifact already exists, skipping download"
                    );

                    Ok(())
                }
                else {
                    self.download_artifact(task)
                };

                match res {
                    Ok(()) => {
                        patch_queue.push(task);
                    }

                    Err(err) => {
                        tracing::error!(
                            patch_name = task.patch_chunk.PatchName,
                            ?err,
                            "Failed to download patch",
                        );

                        let _ = std::fs::remove_file(&artifact_path);

                        (updater)(Update::DownloadingError(err));

                        update_index.process_download_fail(
                            task,
                            task_queue.retries_queue,
                            &updater
                        );
                    }
                }

                update_index.download_states_notifier.notify_all();
            }
            else if task_queue.is_empty() && !update_index.wait_downloading() {
                break;
            }
        }

        // Wake up any threads that might still be waiting
        update_index.download_states_notifier.notify_all();
    }

    // instrumenting to maybe try and see how much time it takes to download, hash
    // check, and apply
    #[tracing::instrument(
        level = "trace", ret, skip(self, task),
        fields(
            file = task.file_manifest.AssetName,
            patch_chunk = task.patch_chunk.PatchName
        )
    )]
    fn download_artifact(&self, task: &FilePatchInfo) -> Result<(), SophonError> {
        let download_url = task.download_url();
        let download_range_val = task.download_range();
        let out_filename = self.tmp_artifact_file_path(task);

        let mut resp = self
            .client
            .get(download_url)
            .header(RANGE, download_range_val)
            .send()?
            .error_for_status()?;

        // Don't have a hash for the patch, can't check it here, check the hash
        // before using (or just check the resulting file, copy-over will hash
        // mismatch, patching will likely just fail, less likely succeed and
        // produce wrong file)
        if let Some(length) = resp.content_length() {
            if length != task.patch_chunk.PatchLength {
                return Err(SophonError::IoError(format!(
                    "Content length mismatch: expected {}, got {length}",
                    task.patch_chunk.PatchLength
                )));
            }
        }

        let mut out_file = BufWriter::new(File::create(out_filename)?);

        let written = resp.copy_to(&mut out_file)?;

        out_file.flush()?;

        if written != task.patch_chunk.PatchLength {
            return Err(SophonError::IoError(format!(
                "Written data length mistamch, expected {}, got {written}",
                task.patch_chunk.PatchLength
            )));
        }

        Ok(())
    }

    fn file_patch_loop<'a, 'b>(
        &self,
        game_folder: &'b Path,
        updater: impl Fn(Update) + 'b,
        update_index: &'b UpdateIndex<'a>,
        retries_queue: &'b Injector<&'a FilePatchInfo<'a>>,
        queue: ThreadQueue<'b, &'a FilePatchInfo<'a>>
    ) {
        let mut do_this_task_last = None;

        loop {
            if let Some(task) = queue.next_job() {
                if task.file_manifest.AssetName.ends_with("globalgamemanagers") {
                    do_this_task_last = Some(task);
                }

                self.file_patch_handler(task, update_index, game_folder, retries_queue, &updater);
            }
            else if !update_index.wait_downloading() {
                break;
            }
        }

        if let Some(last_task) = do_this_task_last {
            let tmp_file_path = self.files_temp().join("globalgamemanagers.tmp");
            let target_path = last_task.target_file_path(game_folder);
            if let Err(err) = Self::finalize_patch(
                &tmp_file_path,
                &target_path,
                last_task.file_manifest.AssetSize,
                &last_task.file_manifest.AssetHashMd5
            ) {
                tracing::error!(?err, "Failed to finalize last file")
            }
        }
    }

    fn file_patch_handler<'a, 'b>(
        &self,
        file_patch_task: &'a FilePatchInfo<'a>,
        update_index: &'b UpdateIndex<'a>,
        game_folder: &'b Path,
        retries_queue: &'b Injector<&'a FilePatchInfo<'a>>,
        updater: impl Fn(Update) + 'b
    ) {
        let res = {
            let target_path = file_patch_task.target_file_path(game_folder);

            if let Ok(true) = check_file(
                &target_path,
                file_patch_task.file_manifest.AssetSize,
                &file_patch_task.file_manifest.AssetHashMd5
            ) {
                tracing::debug!(
                    file = ?target_path,
                    "File appears to be already patched, marking as success"
                );

                Ok(())
            }
            else if let Some(orig_file_path) = file_patch_task.orig_file_path(game_folder) {
                self.file_patch(&orig_file_path, file_patch_task, game_folder)
            }
            else {
                self.file_copy_over(file_patch_task, game_folder)
            }
        };

        match res {
            Ok(()) => {
                tracing::debug!(
                    name = ?file_patch_task.file_manifest.AssetName,
                    "Successfully patched"
                );

                update_index.artifact_success(&file_patch_task.file_manifest.AssetName);

                update_index.download_states_notifier.notify_all();

                update_index.count_patched(1);
                update_index.count_downloaded(file_patch_task.patch_chunk.PatchLength);

                (updater)(update_index.msg_patched());
                (updater)(update_index.msg_bytes());
            }

            Err(e) => {
                tracing::error!(
                    error = ?e,
                    file = file_patch_task.file_manifest.AssetName,
                    "Patching failed"
                );

                (updater)(Update::PatchingError(e.to_string()));

                self.cleanup_on_fail(file_patch_task);

                update_index.process_download_fail(file_patch_task, retries_queue, updater);

                update_index.download_states_notifier.notify_all();
            }
        }
    }

    fn file_copy_over(
        &self,
        file_patch_task: &FilePatchInfo,
        game_folder: &Path
    ) -> Result<(), SophonError> {
        let target_path = file_patch_task.target_file_path(game_folder);

        if let Ok(true) = check_file(
            &target_path,
            file_patch_task.file_manifest.AssetSize,
            &file_patch_task.file_manifest.AssetHashMd5
        ) {
            tracing::debug!(file = ?target_path, "File appears to be already patched, marking as success");

            return Ok(());
        }

        let artifact_path = self.tmp_artifact_file_path(file_patch_task);

        Self::finalize_patch(
            &artifact_path,
            &target_path,
            file_patch_task.file_manifest.AssetSize,
            &file_patch_task.file_manifest.AssetHashMd5
        )
    }

    fn file_patch(
        &self,
        orig_file_path: &Path,
        file_patch_task: &FilePatchInfo,
        game_folder: &Path
    ) -> Result<(), SophonError> {
        if !check_file(
            orig_file_path,
            file_patch_task.patch_chunk.OriginalFileLength,
            &file_patch_task.patch_chunk.OriginalFileMd5
        )? {
            // A better way would be to mark the download as failed right away
            // instead of having this repeat all the retries. But it's easier to
            // handle this rare faulty edge case this way.
            tracing::error!(file = ?orig_file_path, "Original file doesn't pass hash check, cannot patch file");

            return Err(SophonError::FileHashMismatch {
                path: orig_file_path.to_owned(),
                expected: file_patch_task.patch_chunk.OriginalFileMd5.clone(),
                got: file_md5_hash_str(orig_file_path)?
            });
        }

        let tmp_src_path = self.tmp_src_file_path(file_patch_task);
        let tmp_out_path = self.tmp_out_file_path(file_patch_task);
        let artifact = self.tmp_artifact_file_path(file_patch_task);

        std::fs::copy(orig_file_path, &tmp_src_path)?;

        hpatchz::patch(&tmp_src_path, &artifact, &tmp_out_path)?;

        let target = if file_patch_task
            .file_manifest
            .AssetName
            .ends_with("globalgamemanagers")
        {
            self.files_temp().join("globalgamemanagers.tmp")
        }
        else {
            file_patch_task.target_file_path(game_folder)
        };

        Self::finalize_patch(
            &tmp_out_path,
            &target,
            file_patch_task.file_manifest.AssetSize,
            &file_patch_task.file_manifest.AssetHashMd5
        )?;

        // Clean up a bit after patching
        let _ = std::fs::remove_file(&tmp_src_path);
        let _ = std::fs::remove_file(&tmp_out_path);

        Ok(())
    }

    fn finalize_patch(
        file: &Path,
        target: &Path,
        size: u64,
        hash: &str
    ) -> Result<(), SophonError> {
        if check_file(file, size, hash)? {
            tracing::debug!(
                result = ?file,
                destination = ?target,
                "File hash check passed, copying into final destination"
            );
            ensure_parent(target)?;
            add_user_write_permission_to_file(target)?;
            std::fs::copy(file, target)?;
            Ok(())
        }
        else {
            Err(SophonError::FileHashMismatch {
                path: file.to_owned(),
                expected: hash.to_owned(),
                got: file_md5_hash_str(file)?
            })
        }
    }

    /// Remove all the files that might have been created. Temporary files,
    /// downloads, etc to prepare for a clean re-downlaod of the artifact and
    /// attempting to patch again
    fn cleanup_on_fail(&self, file_info: &FilePatchInfo) {
        let tmp_src = self.tmp_src_file_path(file_info);
        let tmp_out = self.tmp_out_file_path(file_info);
        let artifact = self.tmp_artifact_file_path(file_info);
        for path in [tmp_src, tmp_out, artifact] {
            // Ignore errors (missing file, permissions, etc)
            let _ = std::fs::remove_file(path);
        }
    }

    /// Folder to temporarily store files being updated (patched, created, etc).
    #[inline]
    pub fn files_temp(&self) -> PathBuf {
        self.temp_folder
            .join(format!("updating-{}", self.diff_info.matching_field))
    }

    fn tmp_src_file_path(&self, file_info: &FilePatchInfo) -> PathBuf {
        self.files_temp().join(file_info.tmp_src_filename())
    }

    fn tmp_out_file_path(&self, file_info: &FilePatchInfo) -> PathBuf {
        self.files_temp().join(file_info.tmp_out_filename())
    }

    /// Folder to temporarily store hdiff files
    #[inline]
    fn patches_temp(&self) -> PathBuf {
        self.files_temp().join("patches")
    }

    fn tmp_artifact_file_path(&self, file_info: &FilePatchInfo) -> PathBuf {
        self.patches_temp().join(file_info.artifact_filename())
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
}
