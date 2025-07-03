use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Condvar, Mutex, MutexGuard};

use crossbeam_deque::{Injector, Worker};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use super::api_schemas::game_branches::PackageInfo;
use super::api_schemas::sophon_manifests::{DownloadInfo, SophonDownloadInfo, SophonDownloads};
use super::protos::SophonManifest::{
    SophonManifestAssetChunk, SophonManifestAssetProperty, SophonManifestProto
};
use super::{
    add_user_write_permission_to_file, api_get_request, check_file, file_md5_hash_str,
    get_protobuf_from_url, ArtifactDownloadState, DownloadQueue, GameEdition, SophonError,
    ThreadQueue
};
use crate::prelude::{free_space, prettify_bytes};

fn sophon_download_info_url(package_info: &PackageInfo, edition: GameEdition) -> String {
    format!(
        "{}/downloader/sophon_chunk/api/getBuild?branch={}&password={}&package_id={}",
        edition.api_host(),
        package_info.branch,
        package_info.password,
        package_info.package_id
    )
}

#[inline]
pub fn get_game_download_sophon_info(
    client: &Client,
    package_info: &PackageInfo,
    edition: GameEdition
) -> Result<SophonDownloads, SophonError> {
    let url = sophon_download_info_url(package_info, edition);

    api_get_request(client, url)
}

pub fn get_download_manifest(
    client: &Client,
    download_info: &SophonDownloadInfo
) -> Result<SophonManifestProto, SophonError> {
    let url_prefix = &download_info.manifest_download.url_prefix;
    let url_suffix = &download_info.manifest_download.url_suffix;
    let manifest_id = &download_info.manifest.id;

    get_protobuf_from_url(
        client,
        format!("{url_prefix}{url_suffix}/{manifest_id}"),
        download_info.manifest_download.compression == 1
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    CheckingFreeSpace(PathBuf),

    /// `(temp path)`
    DownloadingStarted(PathBuf),

    DownloadingProgressBytes {
        downloaded_bytes: u64,
        total_bytes: u64
    },

    DownloadingProgressFiles {
        downloaded_files: u64,
        total_files: u64
    },

    DownloadingFinished,
    DownloadingError(SophonError),

    FileHashCheckFailed(PathBuf)
}

#[derive(Debug)]
struct ChunkInfo<'a> {
    chunk_manifest: &'a SophonManifestAssetChunk,
    download_info: &'a DownloadInfo,
    used_in_files: Vec<&'a String>
}

impl ChunkInfo<'_> {
    fn download_url(&self) -> String {
        self.download_info
            .download_url(&self.chunk_manifest.ChunkName)
    }

    /// returns the expected size and md5 hash that will be used to download and check this chunk
    #[inline(always)]
    fn chunk_file_info(&self) -> (u64, &str) {
        if self.is_compressed() {
            (
                self.chunk_manifest.ChunkSize,
                &self.chunk_manifest.ChunkCompressedHashMd5
            )
        }
        else {
            (
                self.chunk_manifest.ChunkSizeDecompressed,
                &self.chunk_manifest.ChunkDecompressedHashMd5
            )
        }
    }

    fn is_compressed(&self) -> bool {
        self.download_info.compression == 1
    }

    fn ondisk_filename(&self) -> String {
        if self.is_compressed() {
            format!("{}.chunk.zstd", self.chunk_manifest.ChunkName)
        }
        else {
            format!("{}.chunk", self.chunk_manifest.ChunkName)
        }
    }
}

#[derive(Debug)]
struct FileInfo<'a> {
    file_manifest: &'a SophonManifestAssetProperty,
    /// hashmap value is referring to whether the chunk was downloaded successfully or not
    chunks: Vec<&'a String>
}

impl FileInfo<'_> {
    fn is_file_ready(&self, states: &Mutex<HashMap<&String, ArtifactDownloadState>>) -> bool {
        let states_lock = states.lock().unwrap();
        for chunk_id in &self.chunks {
            match states_lock.get(*chunk_id) {
                Some(ArtifactDownloadState::Failed)
                | Some(ArtifactDownloadState::Downloading(_)) => {
                    return false;
                }
                None | Some(ArtifactDownloadState::Downloaded) => {}
            }
        }
        true
    }

    /// Path to a target file on filesystem
    fn target_file_path(&self, game_dir: impl AsRef<Path>) -> PathBuf {
        game_dir.as_ref().join(&self.file_manifest.AssetName)
    }

    /// Path to a temporary file to store the in-progress file
    fn tmp_filename(&self) -> String {
        format!("{}.tmp", self.file_manifest.AssetHashMd5)
    }
}

#[derive(Debug)]
struct DownloadIndex<'a> {
    chunks: HashMap<&'a String, ChunkInfo<'a>>,
    files: HashMap<&'a String, FileInfo<'a>>,
    total_bytes: u64,
    downloaded_bytes: AtomicU64,
    downloaded_files: AtomicU64,
    download_states: Mutex<HashMap<&'a String, ArtifactDownloadState>>,
    download_states_notifier: Condvar
}

impl<'a> DownloadIndex<'a> {
    fn new(download_info: &'a SophonDownloadInfo, manifest: &'a SophonManifestProto) -> Self {
        let mut chunks = HashMap::new();
        let mut files = HashMap::with_capacity(manifest.Assets.len());

        for file_manifest in &manifest.Assets {
            let file_chunks = file_manifest
                .AssetChunks
                .iter()
                .map(|smac| &smac.ChunkName)
                .collect::<Vec<_>>();
            for chunk_manifest in &file_manifest.AssetChunks {
                let chunk_info =
                    chunks
                        .entry(&chunk_manifest.ChunkName)
                        .or_insert_with(|| ChunkInfo {
                            chunk_manifest,
                            download_info: &download_info.chunk_download,
                            used_in_files: vec![]
                        });
                chunk_info.used_in_files.push(&file_manifest.AssetName);
            }

            files.insert(
                &file_manifest.AssetName,
                FileInfo {
                    file_manifest,
                    chunks: file_chunks
                }
            );
        }

        Self {
            download_states: Mutex::new(HashMap::from_iter(
                chunks
                    .keys()
                    .map(|id| (*id, ArtifactDownloadState::default()))
            )),
            download_states_notifier: Condvar::new(),
            total_bytes: Self::total_bytes_calculate(&chunks),
            chunks,
            files,
            downloaded_bytes: AtomicU64::new(0),
            downloaded_files: AtomicU64::new(0)
        }
    }

    /// [`DownloadingIndex`] without a file index. Used for predownloads, where only the downloaded
    /// chunks matter.
    fn new_chunks_only(
        download_info: &'a SophonDownloadInfo,
        manifest: &'a SophonManifestProto
    ) -> Self {
        let chunks = manifest
            .Assets
            .iter()
            .flat_map(|smap| &smap.AssetChunks)
            .map(|chunk| {
                (
                    &chunk.ChunkName,
                    ChunkInfo {
                        download_info: &download_info.chunk_download,
                        chunk_manifest: chunk,
                        used_in_files: vec![]
                    }
                )
            })
            .collect::<HashMap<_, _>>();

        Self {
            download_states: Mutex::new(HashMap::from_iter(
                chunks
                    .keys()
                    .map(|id| (*id, ArtifactDownloadState::default()))
            )),
            download_states_notifier: Condvar::new(),
            total_bytes: Self::total_bytes_calculate(&chunks),
            chunks,
            files: HashMap::new(),
            downloaded_bytes: AtomicU64::new(0),
            downloaded_files: AtomicU64::new(0)
        }
    }

    fn total_bytes_calculate(chunks: &HashMap<&'a String, ChunkInfo<'a>>) -> u64 {
        chunks
            .values()
            .map(|chunk| chunk.chunk_manifest.ChunkSize)
            .sum()
    }

    #[inline(always)]
    fn total_files(&self) -> u64 {
        self.files.len() as u64
    }

    fn msg_files(&self) -> Update {
        Update::DownloadingProgressFiles {
            downloaded_files: self
                .downloaded_files
                .load(std::sync::atomic::Ordering::Acquire),
            total_files: self.total_files()
        }
    }

    fn msg_bytes(&self) -> Update {
        Update::DownloadingProgressBytes {
            downloaded_bytes: self
                .downloaded_bytes
                .load(std::sync::atomic::Ordering::Acquire),
            total_bytes: self.total_bytes
        }
    }

    fn add_files(&self, amount: u64) {
        self.downloaded_files
            .fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }

    fn add_bytes(&self, amount: u64) {
        self.downloaded_bytes
            .fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }

    /// Process chunk download failure. Either pushes the chunk onto the retries queue or sends the
    /// chunk download fail update message using the updater. Refer to [Self::count_chunk_fail] for
    /// more info.
    fn process_download_fail<'b>(
        &self,
        chunk: &'a ChunkInfo<'a>,
        retries_queue: &'b Injector<&'a ChunkInfo<'a>>,
        updater: impl Fn(Update) + 'b
    ) {
        match self.count_download_fail(&chunk.chunk_manifest.ChunkName) {
            Ok(()) => retries_queue.push(chunk),
            Err(msg) => (updater)(msg)
        }
    }

    /// A download attempt or check failed, decrement the retry count or report the chunk as
    /// completely failed.
    /// The error type is a message to emit in case of a completely faield chunk download.
    /// If this returns Ok, push the chunk on the retries queue.
    /// If this returns Err, emit the fail message for this chunk and stop retrying.
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
    /// if anything is still downloading, uses the [`Condvar`] to wait until the states are updated
    /// by patching/checking threads and checks again.
    fn wait_downloading(&self) -> bool {
        let mut guard = self
            .download_states
            .lock()
            .expect("Something poisoned the mutex");
        if Self::any_downloading(&guard) {
            tracing::debug!(
                "Some artifacts still being downloaded or checked, waiting for updates"
            );
            // unlocks the mutex during wait, see [`Condvar::wait`]
            guard = self
                .download_states_notifier
                .wait(guard)
                .expect("Something poisoned teh mutex");
            Self::any_downloading(&guard)
        }
        else {
            tracing::debug!("All artifacts marked as downloaded or failed, breaking the loop");
            false
        }
    }

    fn any_downloading(guard: &MutexGuard<HashMap<&'a String, ArtifactDownloadState>>) -> bool {
        guard
            .values()
            .any(|state| matches!(state, ArtifactDownloadState::Downloading(_)))
    }
}

type DownloadChunkQueue<'a, 'b, I> = DownloadQueue<'b, &'a ChunkInfo<'a>, I>;

#[derive(Debug)]
pub struct SophonInstaller {
    pub client: reqwest::blocking::Client,
    pub manifest: SophonManifestProto,
    pub download_info: SophonDownloadInfo,
    pub check_free_space: bool,
    pub temp_folder: PathBuf
}

impl SophonInstaller {
    pub fn new(
        client: Client,
        download_info: &SophonDownloadInfo,
        temp_dir: impl AsRef<Path>
    ) -> Result<Self, SophonError> {
        let manifest = get_download_manifest(&client, download_info)?;

        Ok(Self {
            client,
            manifest,
            download_info: download_info.clone(),
            check_free_space: true,
            temp_folder: temp_dir.as_ref().to_owned()
        })
    }

    pub fn install(
        &self,
        output_folder: &Path,
        thread_count: usize,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        let download_size = self.download_info.stats.compressed_size.parse().unwrap();
        let installed_size = self.download_info.stats.uncompressed_size.parse().unwrap();

        tracing::trace!("Checking free space availability");

        if self.check_free_space {
            (updater)(Update::CheckingFreeSpace(self.temp_folder.clone()));

            Self::free_space_check(updater.clone(), &self.temp_folder, download_size)?;

            (updater)(Update::CheckingFreeSpace(output_folder.to_owned()));

            let output_size_to_check = if free_space::is_same_disk(&self.temp_folder, output_folder)
            {
                download_size + installed_size
            }
            else {
                installed_size
            };

            Self::free_space_check(updater.clone(), output_folder, output_size_to_check)?;
        }

        tracing::trace!("Downloading files");

        (updater)(Update::DownloadingStarted(self.temp_folder.clone()));

        self.create_temp_dirs()?;

        self.install_multithreaded(thread_count, output_folder, updater.clone());

        (updater)(Update::DownloadingFinished);

        Ok(())
    }

    fn install_multithreaded(
        &self,
        thread_count: usize,
        output_folder: impl AsRef<Path>,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) {
        tracing::debug!("Starting mutlithreaded download and install");

        let download_index = DownloadIndex::new(&self.download_info, &self.manifest);
        tracing::info!(
            "{} Chunks to download, {} Files to install, {} total bytes",
            download_index.chunks.len(),
            download_index.files.len(),
            prettify_bytes(download_index.total_bytes)
        );

        (updater)(download_index.msg_files());
        (updater)(download_index.msg_bytes());

        let retries_queue = Injector::<&ChunkInfo>::new();
        let file_assembly_queue = Injector::<&FileInfo>::new();

        let game_folder = output_folder.as_ref();

        let worker_threads = std::iter::repeat_with(Worker::new_fifo)
            .take(thread_count)
            .collect::<Vec<_>>();
        let stealers = worker_threads
            .iter()
            .map(Worker::stealer)
            .collect::<Vec<_>>();

        tracing::debug!("Spawning worker threads");
        std::thread::scope(|scope| {
            let updater_clone = updater.clone();
            let download_queue = DownloadQueue {
                tasks_iter: download_index.chunks.values().peekable(),
                retries_queue: &retries_queue
            };
            scope.spawn(|| {
                let _span = tracing::trace_span!("Download thread").entered();
                (updater_clone)(Update::DownloadingStarted(self.temp_folder.clone()));
                self.artifact_download_loop(
                    download_queue,
                    Some(&file_assembly_queue),
                    &download_index,
                    updater_clone
                );
            });

            for (i, worker_queue) in worker_threads.into_iter().enumerate() {
                let updater_clone = updater.clone();
                let thread_queue = ThreadQueue {
                    global: &file_assembly_queue,
                    local: worker_queue,
                    stealers: &stealers
                };
                let index_ref = &download_index;
                scope.spawn(move || {
                    let _span = tracing::debug_span!("Patching thread", thread_id = i).entered();
                    self.file_assembly_loop(game_folder, updater_clone, index_ref, thread_queue);
                });
            }
        });

        (updater)(Update::DownloadingFinished);
    }

    pub fn pre_download(
        &self,
        thread_count: usize,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        if self.check_free_space {
            (updater)(Update::CheckingFreeSpace(self.temp_folder.clone()));

            let download_size = self.download_info.stats.compressed_size.parse().unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_size)?;
        }

        self.create_temp_dirs()?;

        self.predownload_multithreaded(thread_count, updater);

        Ok(())
    }

    fn predownload_multithreaded(
        &self,
        _thread_count: usize,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) {
        tracing::debug!("Starting multithreaded predownload");

        let download_index = DownloadIndex::new_chunks_only(&self.download_info, &self.manifest);
        tracing::info!(
            "{} Chunks to download, {} total bytes",
            download_index.chunks.len(),
            prettify_bytes(download_index.total_bytes)
        );

        (updater)(download_index.msg_files());
        (updater)(download_index.msg_bytes());

        let retries_queue = Injector::<&ChunkInfo>::new();

        let download_queue = DownloadQueue {
            tasks_iter: download_index.chunks.values().peekable(),
            retries_queue: &retries_queue
        };

        tracing::debug!("Starting download");
        self.artifact_download_loop(download_queue, None, &download_index, updater);
    }

    /// Loops over the tasks and retries and tries to download them, pushing onto the file assembly queue
    /// if the download succeedes. If both the tasks iterator and the retries queue don't return
    /// anything, checks if they are empty and then checks if there are any unfinished chunks and waits
    /// for either all chunks to finish applying or a new retry being pushed onto the queue.
    fn artifact_download_loop<'a, 'b, I: Iterator<Item = &'a ChunkInfo<'a>> + 'b>(
        &self,
        mut task_queue: DownloadChunkQueue<'a, 'b, I>,
        assembly_queue: Option<&'b Injector<&'a FileInfo<'a>>>,
        download_index: &'a DownloadIndex<'a>,
        updater: impl Fn(Update) + 'b
    ) {
        loop {
            if let Some(task) = task_queue.next() {
                // Check if the file already exists on disk and if it does, skip re-downloading it
                let artifact_path = self.tmp_artifact_file_path(task);

                let res = if artifact_path.exists() {
                    tracing::debug!(artifact = ?artifact_path, "Artifact already exists, skipping download");
                    Ok(())
                }
                else {
                    self.download_artifact(task)
                };

                let (chunk_size, chunk_hash) = task.chunk_file_info();

                let res = res.and_then(|_| {
                    if !check_file(&artifact_path, chunk_size, chunk_hash)? {
                        Err(SophonError::ChunkHashMismatch {
                            expected: chunk_hash.to_owned(),
                            got: file_md5_hash_str(&artifact_path)?
                        })
                    }
                    else {
                        Ok(())
                    }
                });

                match res {
                    Ok(()) => {
                        download_index.artifact_success(&task.chunk_manifest.ChunkName);
                        download_index.add_bytes(chunk_size);
                        (updater)(download_index.msg_bytes());
                        if let Some(file_queue) = assembly_queue {
                            for file_id in &task.used_in_files {
                                let file_info = download_index
                                    .files
                                    .get(file_id)
                                    .expect("Missing files in index!");
                                if file_info.is_file_ready(&download_index.download_states) {
                                    file_queue.push(file_info);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        tracing::error!(
                            chunk_name = task.chunk_manifest.ChunkName,
                            ?err,
                            "Failed to download chunk",
                        );
                        let _ = std::fs::remove_file(&artifact_path);
                        (updater)(Update::DownloadingError(err));
                        download_index.process_download_fail(
                            task,
                            task_queue.retries_queue,
                            &updater
                        );
                    }
                }
                download_index.download_states_notifier.notify_all();
            }
            else if task_queue.is_empty() && !download_index.wait_downloading() {
                break;
            }
        }
        // Wake up any threads that might still be waiting
        download_index.download_states_notifier.notify_all();
    }

    // instrumenting to maybe try and see how much time it takes to download, hash check, and apply
    #[tracing::instrument(level = "trace", ret, skip(self, task), fields(chunk = task.chunk_manifest.ChunkName))]
    fn download_artifact(&self, task: &ChunkInfo) -> Result<(), SophonError> {
        let download_url = task.download_url();
        let out_filename = self.tmp_artifact_file_path(task);

        //let (chunk_size, _) = task.chunk_file_info();

        let mut resp = self.client.get(download_url).send()?.error_for_status()?;

        // In theory, can catch the size mismatch before writing to the disk?
        // Commented out because I don't think it's necessary and the error case might not help
        // that much
        /*
        if let Some(length) = resp.content_length() {
            if length != chunk_size {
                return Err(SophonError::IoError(format!(
                    "Content length mismatch: expected {chunk_size}, got {length}"
                )));
            }
        }
        */

        let mut out_file = BufWriter::new(File::create(out_filename)?);
        let _written = resp.copy_to(&mut out_file)?;
        out_file.flush()?;

        /*
        if written != chunk_size {
            return Err(SophonError::IoError(format!(
                "Written data length mistamch, expected {chunk_size}, got {written}"
            )));
        }
        */

        Ok(())
    }

    fn file_assembly_loop<'a, 'b>(
        &self,
        game_folder: &'b Path,
        updater: impl Fn(Update) + 'b,
        download_index: &'b DownloadIndex<'a>,
        queue: ThreadQueue<'b, &'a FileInfo<'a>>
    ) {
        let mut do_this_task_last = None;
        loop {
            if let Some(task) = queue.next_job() {
                if task.file_manifest.AssetName.ends_with("globalgamemanagers") {
                    do_this_task_last = Some(task);
                    continue;
                }
                self.file_assembly_handler(task, download_index, game_folder, &updater);
            }
            else if !download_index.wait_downloading() {
                break;
            }
        }
        if let Some(last_task) = do_this_task_last {
            self.file_assembly_handler(last_task, download_index, game_folder, updater);
        }
    }

    fn file_assembly_handler<'a>(
        &self,
        task: &'a FileInfo<'a>,
        downloading_index: &DownloadIndex<'a>,
        game_folder: &Path,
        updater: impl Fn(Update)
    ) {
        let target_path = task.target_file_path(game_folder);
        let tmp_file = self.tmp_downloading_file_path(task);

        let res = if let Ok(true) = check_file(
            &target_path,
            task.file_manifest.AssetSize,
            &task.file_manifest.AssetHashMd5
        ) {
            tracing::debug!(file = ?target_path, "File appears to be already downloaded");
            Ok(())
        }
        else {
            self.file_assembly(&tmp_file, &target_path, task, downloading_index)
        };

        match res {
            Ok(()) => {
                tracing::debug!("Successfully downloaded `{}`", task.file_manifest.AssetName);
                downloading_index.add_files(1);
                downloading_index.download_states_notifier.notify_all();
                (updater)(downloading_index.msg_files());
            }
            Err(e) => {
                tracing::error!(
                    error = ?e,
                    file = task.file_manifest.AssetName,
                    "File assembly failed"
                );
                (updater)(Update::DownloadingError(e));
                self.cleanup_on_fail(task);
            }
        }
    }

    fn cleanup_on_fail(&self, task: &FileInfo) {
        let _ = std::fs::remove_file(self.tmp_downloading_file_path(task));
    }

    fn file_assembly(
        &self,
        tmp_file: &Path,
        target_path: &Path,
        task: &FileInfo,
        downloading_index: &DownloadIndex
    ) -> Result<(), SophonError> {
        let mut chunks = task
            .chunks
            .iter()
            .map(|chunk_id| {
                downloading_index
                    .chunks
                    .get(chunk_id)
                    .expect("Chunk missing from index")
            })
            .collect::<Vec<_>>();
        chunks.sort_by_key(|chunk_info| &chunk_info.chunk_manifest.ChunkOnFileOffset);

        let output_file = File::create(tmp_file)?;
        output_file.set_len(task.file_manifest.AssetSize)?;
        let mut output_file = BufWriter::new(output_file);

        for chunk_info in chunks {
            self.write_chunk_to_file(chunk_info, &mut output_file)?;
        }

        output_file.flush()?;
        drop(output_file);

        if !check_file(
            tmp_file,
            task.file_manifest.AssetSize,
            &task.file_manifest.AssetHashMd5
        )? {
            return Err(SophonError::FileHashMismatch {
                path: tmp_file.to_owned(),
                expected: task.file_manifest.AssetHashMd5.clone(),
                got: file_md5_hash_str(tmp_file)?
            });
        }

        add_user_write_permission_to_file(target_path)?;
        std::fs::copy(tmp_file, target_path)?;
        std::fs::remove_file(tmp_file)?;

        Ok(())
    }

    fn write_chunk_to_file<W: Write>(
        &self,
        chunk_info: &ChunkInfo,
        dest_file: &mut W
    ) -> std::io::Result<u64> {
        let chunk_path = self.tmp_artifact_file_path(chunk_info);
        if chunk_info.is_compressed() {
            Self::write_artifact_to_file_zstd(dest_file, &chunk_path)
        }
        else {
            Self::write_artifact_to_file(dest_file, &chunk_path)
        }
    }

    fn write_artifact_to_file<W: Write>(
        dest_file: &mut W,
        artifact_path: &Path
    ) -> std::io::Result<u64> {
        let mut artifact_file = File::open(artifact_path)?;
        std::io::copy(&mut artifact_file, dest_file)
    }

    fn write_artifact_to_file_zstd<W: Write>(
        dest_file: &mut W,
        artifact_path: &Path
    ) -> std::io::Result<u64> {
        let artifact_file = File::open(artifact_path)?;
        let mut zstd_decoder = zstd::Decoder::new(artifact_file)?;
        std::io::copy(&mut zstd_decoder, dest_file)
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

    #[inline]
    pub fn with_free_space_check(mut self, check: bool) -> Self {
        self.check_free_space = check;

        self
    }

    #[inline]
    pub fn with_temp_folder(mut self, temp_folder: PathBuf) -> Self {
        self.temp_folder = temp_folder;

        self
    }

    /// Folder to temporarily store files being downloaded
    #[inline]
    pub fn downloading_temp(&self) -> PathBuf {
        self.temp_folder.join("downloading")
    }

    fn tmp_downloading_file_path(&self, file_info: &FileInfo) -> PathBuf {
        self.downloading_temp().join(file_info.tmp_filename())
    }

    /// Folder to temporarily store chunks
    #[inline]
    fn chunk_temp_folder(&self) -> PathBuf {
        self.downloading_temp().join("chunks")
    }

    fn tmp_artifact_file_path(&self, chunk_info: &ChunkInfo) -> PathBuf {
        self.chunk_temp_folder().join(chunk_info.ondisk_filename())
    }

    /// Create all needed sub-directories in the temp folder
    fn create_temp_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.downloading_temp())?;
        std::fs::create_dir_all(self.chunk_temp_folder())?;

        Ok(())
    }
}
