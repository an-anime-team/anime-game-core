use std::{collections::{HashMap, HashSet}, sync::{atomic::AtomicU64, Mutex}};
use std::io::{Read, Write, Seek, SeekFrom};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::os::unix::fs::FileExt;

use crossbeam_deque::{Injector, Steal};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

// I ain't refactoring all this.
use super::{
    api_get_request, api_schemas::{
        game_branches::PackageInfo,
        sophon_manifests::{DownloadInfo, SophonDownloadInfo, SophonDownloads},
    }, bytes_check_md5, check_file, ensure_parent, file_md5_hash_str, get_protobuf_from_url, md5_hash_str, protos::SophonManifest::{
        SophonManifestAssetChunk, SophonManifestAssetProperty, SophonManifestProto,
    }, GameEdition, SophonError
};

use crate::prelude::free_space;

const DEFAULT_CHUNK_RETRIES: u8 = 4;

fn sophon_download_info_url(
    package_info: &PackageInfo,
    edition: GameEdition,
) -> String {
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
    let url = sophon_download_info_url(
        package_info,
        edition
    );

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
        format!("{}{}/{}", url_prefix, url_suffix, manifest_id),
        download_info.manifest_download.compression == 1
    )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub total_bytes: u64,
    pub total_files: u64,
    pub downloaded_bytes: AtomicU64,
    pub downloaded_files: AtomicU64,
    pub downloaded_chunks: HashSet<String>
}

impl DownloadProgress {
    fn new_from_manifest(manifest: &SophonManifestProto) -> Self {
        Self {
            total_bytes: manifest.total_bytes_compressed(),
            total_files: manifest.total_files(),
            downloaded_bytes: 0.into(),
            downloaded_files: 0.into(),
            downloaded_chunks: HashSet::with_capacity(manifest.total_chunks() as usize)
        }
    }

    fn msg_files(&self) -> Update {
        Update::DownloadingProgressFiles {
            downloaded_files: self.downloaded_files.load(std::sync::atomic::Ordering::Acquire),
            total_files: self.total_files
        }
    }

    fn msg_bytes(&self) -> Update {
        Update::DownloadingProgressBytes {
            downloaded_bytes: self.downloaded_bytes.load(std::sync::atomic::Ordering::Acquire),
            total_bytes: self.total_bytes
        }
    }

    fn count_chunk(&mut self, chunk_info: &SophonManifestAssetChunk) {
        if !self.downloaded_chunks.contains(&chunk_info.ChunkName) {
            self.downloaded_bytes.fetch_add(chunk_info.ChunkSize, std::sync::atomic::Ordering::AcqRel);

            self.downloaded_chunks.insert(chunk_info.ChunkName.clone());
        }
    }

    fn add_files(&self, amount: u64) {
        self.downloaded_files.fetch_add(amount, std::sync::atomic::Ordering::AcqRel);
    }

    fn add_bytes(&self, amount: u64) {
        self.downloaded_bytes.fetch_add(amount, std::sync::atomic::Ordering::AcqRel);
    }
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
    used_in_files: Vec<&'a String>,
}

impl ChunkInfo<'_> {
    fn download_url(&self) -> String {
        self.download_info.download_url(&self.chunk_manifest.ChunkName)
    }

    /// returns the expected size, expected hash and a filename extension to be used for
    /// downloading and checking this chunk
    #[inline(always)]
    fn chunk_file_info(&self) -> (u64, &str, &'static str) {
        if self.download_info.compression == 1 {
            (
                self.chunk_manifest.ChunkSize,
                &self.chunk_manifest.ChunkCompressedHashMd5,
                ".chunk.zstd"
            )
        } else {
            (
                self.chunk_manifest.ChunkSizeDecompressed,
                &self.chunk_manifest.ChunkDecompressedHashMd5,
                ".chunk"
            )
        }
    }

    fn chunk_path(&self, dir: &Path) -> PathBuf {
        let (_, _, file_ext) = self.chunk_file_info();
        dir.join(format!("{}{}", self.chunk_manifest.ChunkName, file_ext))
    }
}

#[derive(Debug)]
struct FileInfo<'a> {
    file_manifest: &'a SophonManifestAssetProperty,
    /// hashmap value is referring to whether the chunk was downloaded successfully or not
    chunks: Vec<&'a String>,
}

impl FileInfo<'_> {
    fn is_file_ready(&self, states: &Mutex<HashMap<&String, ChunkState>>) -> bool {
        let states_lock = states.lock().unwrap();
        for chunk_id in &self.chunks {
            match states_lock.get(*chunk_id) {
                Some(ChunkState::Failed) | Some(ChunkState::Downloading(_)) => { return false; },
                None | Some(ChunkState::Downloaded) => {}
            }
        }
        true
    }
}

#[derive(Debug)]
struct DownloadingIndex<'a> {
    chunks: HashMap<&'a String, ChunkInfo<'a>>,
    files: HashMap<&'a String, FileInfo<'a>>
}

impl<'a> DownloadingIndex<'a> {
    fn new(download_info: &'a SophonDownloadInfo, manifest: &'a SophonManifestProto) -> Self {
        let mut chunks = HashMap::new();
        let mut files = HashMap::with_capacity(manifest.Assets.len());

        for file_manifest in &manifest.Assets {
            let file_chunks = file_manifest.AssetChunks.iter().map(|smac| &smac.ChunkName).collect::<Vec<_>>();
            for chunk_manifest in &file_manifest.AssetChunks {
                let chunk_info = chunks.entry(&chunk_manifest.ChunkName).or_insert_with(|| {
                    ChunkInfo {
                        chunk_manifest,
                        download_info: &download_info.chunk_download,
                        used_in_files: vec![],
                    }
                });
                chunk_info.used_in_files.push(&file_manifest.AssetName);
            }

            files.insert(&file_manifest.AssetName, FileInfo {
                file_manifest,
                chunks: file_chunks
            });
        }

        Self {
            chunks,
            files
        }
    }

    /// [`DownloadingIndex`] without a file index. Used for predownloads, where only the downloaded
    /// chunks matter.
    fn new_chunks_only(download_info: &'a SophonDownloadInfo, manifest: &'a SophonManifestProto) -> Self {
        let chunks = manifest.Assets
            .iter()
            .flat_map(|smap| &smap.AssetChunks)
            .map(|chunk| (
                    &chunk.ChunkName,
                    ChunkInfo {
                        download_info: &download_info.chunk_download,
                        chunk_manifest: chunk,
                        used_in_files: vec![],
                    }
        )).collect();

        Self {
            chunks,
            files: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ChunkState {
    // Chunk successfully downloaded
    Downloaded,
    // Download failed, run out of retries
    Failed,
    // Amount of retries left, 0 means last retry is being run
    Downloading(u8)
}

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

    /// Folder to temporarily store chunks
    #[inline]
    fn chunk_temp_folder(&self) -> PathBuf {
        self.downloading_temp().join("chunks")
    }

    /// Create all needed sub-directories in the temp folder
    fn create_temp_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.downloading_temp())?;
        std::fs::create_dir_all(self.chunk_temp_folder())?;

        Ok(())
    }

    fn fail_chunk<'a, 'b>(&self, chunk_info: &'a ChunkInfo<'a>, states: &'b Mutex<HashMap<&'a String, ChunkState>>, download_queue: &'b Injector<&'a ChunkInfo<'a>>, updater: impl Fn(Update) + 'b) {
        // Check failed, file corrupt
        let chunk_path = chunk_info.chunk_path(&self.chunk_temp_folder());
        let _ = std::fs::remove_file(&chunk_path);
        {
            let mut states_lock = states.lock().unwrap();
            let chunk_state = states_lock.get_mut(&chunk_info.chunk_manifest.ChunkName).unwrap();
            match chunk_state {
                ChunkState::Downloading(0) => {
                    *chunk_state = ChunkState::Failed;
                    (updater)(Update::DownloadingError(SophonError::ChunkDownloadFailed(chunk_info.chunk_manifest.ChunkName.clone())))
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

    // jtcf: I do not like the amount of repetition in the multithreading code, but making a
    // pseudo-runtime builder/factory/whatever to make it use modules proved to be harder than I
    // anticipated. So unfortunately, have to repeat code.

    fn predownload_multithreaded(&self, thread_count: usize, updater: impl Fn(Update) + Clone + Send + 'static) {
        tracing::debug!("Starting multithreaded predownload");

        let download_index = DownloadingIndex::new_chunks_only(&self.download_info, &self.manifest);
        tracing::info!("{} Chunks to download", download_index.chunks.len());
        let chunk_states: Mutex<HashMap<&String, ChunkState>> = Mutex::new(HashMap::from_iter(download_index.chunks.keys().map(|id| (*id, ChunkState::Downloading(DEFAULT_CHUNK_RETRIES)))));

        let total_bytes: u64 = download_index.chunks.values().map(|ci| ci.chunk_manifest.ChunkSize).sum();
        let downloading_progress = DownloadProgress {
            total_bytes,
            total_files: 0,
            downloaded_files: AtomicU64::new(0),
            downloaded_bytes: AtomicU64::new(0),
            // Unused in here, pending removal, chunks are already deduped in the index
            downloaded_chunks: HashSet::new()
        };
        (updater)(downloading_progress.msg_files());
        (updater)(downloading_progress.msg_bytes());

        let download_queue: Injector<&ChunkInfo> = Injector::new();
        for chunk_info in download_index.chunks.values() {
            download_queue.push(chunk_info);
        }

        let chunk_check_queue: Injector<&ChunkInfo> = Injector::new();

        tracing::debug!("Spawning worker threads");
        std::thread::scope(|scope| {
            for _ in 0..thread_count {
                let updater_clone = updater.clone();
                scope.spawn(|| {
                    let local_updater = move |msg| {
                        (updater_clone)(msg);
                    };
                    // Try the last-stage queues first, moving on to the earlier stages if no jobs
                    // are available.
                    // Ideally there are not that much file assembly and chunk checking task to
                    // leave no threads for chunk downloading, so 1 or 2 will be running (assuming
                    // minimum pool size of 4), maximizing network utilization.
                    // The chunk check task is created after downloading a chunk (checking is
                    // skipped to free the thread for another job), and if the check succeeds, it
                    // is marked as successfully downloaded. Otherwise, decrement the retry count
                    // and push the downloading task back onto the queue.
                    'worker: loop {
                        if let Steal::Success(chunk_check_task) = chunk_check_queue.steal() {
                            let res = self.check_downloaded_chunk(chunk_check_task);
                            match res {
                                Ok(true) => {
                                    tracing::trace!("Successfully downloaded chunk `{}`", chunk_check_task.chunk_manifest.ChunkName);
                                    {
                                        let mut states_lock = chunk_states.lock().unwrap();
                                        let chunk_state = states_lock.get_mut(&chunk_check_task.chunk_manifest.ChunkName).unwrap();
                                        *chunk_state = ChunkState::Downloaded;
                                    }
                                    downloading_progress.add_bytes(chunk_check_task.chunk_manifest.ChunkSize);
                                    (local_updater)(downloading_progress.msg_bytes());
                                },
                                Ok(false) => {
                                    tracing::trace!("Chunk `{}` failed size+hash check", chunk_check_task.chunk_manifest.ChunkName);
                                    self.fail_chunk(chunk_check_task, &chunk_states, &download_queue, &local_updater);
                                },
                                Err(err) => {
                                    tracing::error!("I/O error checking chunk `{}`: {err}", chunk_check_task.chunk_manifest.ChunkName);
                                    (local_updater)(Update::DownloadingError(err.into()));
                                    self.fail_chunk(chunk_check_task, &chunk_states, &download_queue, &local_updater);
                                }
                            }
                            continue;
                        }
                        if let Steal::Success(chunk_download_task) = download_queue.steal() {
                            let res = self.download_chunk(chunk_download_task);
                            match res {
                                Ok(()) => {
                                    chunk_check_queue.push(chunk_download_task);
                                },
                                Err(err) => {
                                    tracing::error!("Error downloading chunk `{}`: {err}", chunk_download_task.chunk_manifest.ChunkName);
                                    (local_updater)(Update::DownloadingError(err));
                                }
                            }
                            continue;
                        }
                        // All queues are empty, end the thread
                        if chunk_check_queue.is_empty() && download_queue.is_empty() {
                            tracing::debug!("queues empty, thread exiting");
                            break 'worker;
                        }
                    }
                });
            }
        });
    }

    fn install_multithreaded(&self, thread_count: usize, output_folder: impl AsRef<Path>, updater: impl Fn(Update) + Clone + Send + 'static) {
        tracing::debug!("Starting mutlithreaded download and install");

        let download_index = DownloadingIndex::new(&self.download_info, &self.manifest);
        tracing::info!("{} Chunks to download, {} Files to install", download_index.chunks.len(), download_index.files.len());
        let chunk_states: Mutex<HashMap<&String, ChunkState>> = Mutex::new(HashMap::from_iter(download_index.chunks.keys().map(|id| (*id, ChunkState::Downloading(DEFAULT_CHUNK_RETRIES)))));

        let total_bytes: u64 = download_index.chunks.values().map(|ci| ci.chunk_manifest.ChunkSize).sum();
        let total_files: u64 = download_index.files.len() as u64;
        let downloading_progress = DownloadProgress {
            total_bytes,
            total_files,
            downloaded_files: AtomicU64::new(0),
            downloaded_bytes: AtomicU64::new(0),
            // Unused in here, pending removal, chunks are already deduped in the index
            downloaded_chunks: HashSet::new()
        };
        (updater)(downloading_progress.msg_files());
        (updater)(downloading_progress.msg_bytes());

        let download_queue: Injector<&ChunkInfo> = Injector::new();
        for chunk_info in download_index.chunks.values() {
            download_queue.push(chunk_info);
        }

        let chunk_check_queue: Injector<&ChunkInfo> = Injector::new();

        let file_queue: Injector<&FileInfo> = Injector::new();

        let out_folder = output_folder.as_ref();

        tracing::debug!("Spawning worker threads");
        std::thread::scope(|scope| {
            for _ in 0..thread_count {
                let updater_clone = updater.clone();
                scope.spawn(|| {
                    let local_updater = move |msg| {
                        (updater_clone)(msg);
                    };
                    // Try the last-stage queues first, moving on to the earlier stages if no jobs
                    // are available.
                    // Ideally there are not that much file assembly and chunk checking task to
                    // leave no threads for chunk downloading, so 1 or 2 will be running (assuming
                    // minimum pool size of 4), maximizing network utilization.
                    // The chunk check task is created after downloading a chunk (checking is
                    // skipped to free the thread for another job), and if the check succeeds, it
                    // is marked as successfully downloaded. Otherwise, decrement the retry count
                    // and push the downloading task back onto the queue.
                    'worker: loop {
                        // Assemble and check file
                        if let Steal::Success(file_task) = file_queue.steal() {
                            tracing::trace!("Assembling final file `{}`", file_task.file_manifest.AssetName);
                            if let Err(err) = self.file_assemble(out_folder, file_task, &download_index) {
                                tracing::error!("Error assembling file `{}`: {err}", file_task.file_manifest.AssetName);
                                (local_updater)(Update::DownloadingError(err));
                            } else {
                                tracing::trace!("Finished `{}`", file_task.file_manifest.AssetName);
                                downloading_progress.add_files(1);
                                (local_updater)(downloading_progress.msg_files());
                            };
                            continue;
                        }
                        // Check downloaded chunk
                        if let Steal::Success(chunk_check_task) = chunk_check_queue.steal() {
                            let res = self.check_downloaded_chunk(chunk_check_task);
                            match res {
                                Ok(true) => {
                                    tracing::trace!("Successfully downloaded chunk `{}`", chunk_check_task.chunk_manifest.ChunkName);
                                    {
                                        let mut states_lock = chunk_states.lock().unwrap();
                                        let chunk_state = states_lock.get_mut(&chunk_check_task.chunk_manifest.ChunkName).unwrap();
                                        *chunk_state = ChunkState::Downloaded;
                                    }
                                    for file_name in &chunk_check_task.used_in_files {
                                        let file_info = download_index.files.get(*file_name).unwrap();
                                        if file_info.is_file_ready(&chunk_states) {
                                            tracing::trace!("File `{}` is ready for assembly, pushing on queue", file_name);
                                            file_queue.push(file_info);
                                        }
                                    }
                                    downloading_progress.add_bytes(chunk_check_task.chunk_manifest.ChunkSize);
                                    (local_updater)(downloading_progress.msg_bytes());
                                },
                                Ok(false) => {
                                    tracing::trace!("Chunk `{}` failed size+hash check", chunk_check_task.chunk_manifest.ChunkName);
                                    self.fail_chunk(chunk_check_task, &chunk_states, &download_queue, &local_updater);
                                },
                                Err(err) => {
                                    tracing::error!("I/O error checking chunk `{}`: {err}", chunk_check_task.chunk_manifest.ChunkName);
                                    (local_updater)(Update::DownloadingError(err.into()));
                                    self.fail_chunk(chunk_check_task, &chunk_states, &download_queue, &local_updater);
                                }
                            }
                            continue;
                        }
                        // Download next chunk
                        if let Steal::Success(chunk_download_task) = download_queue.steal() {
                            let res = self.download_chunk(chunk_download_task);
                            match res {
                                Ok(()) => {
                                    chunk_check_queue.push(chunk_download_task);
                                },
                                Err(err) => {
                                    tracing::error!("Error downloading chunk `{}`: {err}", chunk_download_task.chunk_manifest.ChunkName);
                                    (local_updater)(Update::DownloadingError(err));
                                }
                            }
                            continue;
                        }
                        // All queues are empty, end the thread
                        if file_queue.is_empty() && chunk_check_queue.is_empty() && download_queue.is_empty() {
                            tracing::debug!("queues empty, thread exiting");
                            break 'worker;
                        }
                    }
                });
            }
        });
    }

    fn file_assemble(&self, out_folder: &Path, task: &FileInfo, index: &DownloadingIndex) -> Result<(), SophonError> {
        let temp_file_path = self.downloading_temp().join(&task.file_manifest.AssetHashMd5);
        self.assemble_temp_file_from_chunks(&temp_file_path, task, index)?;
        
        if check_file(&temp_file_path, task.file_manifest.AssetSize, &task.file_manifest.AssetHashMd5)? {
            let out_path = out_folder.join(&task.file_manifest.AssetName);
            ensure_parent(&out_path)?;
            std::fs::copy(&temp_file_path, out_path)?;
            let _ = std::fs::remove_file(&temp_file_path);
            Ok(())
        } else {
            Err(SophonError::FileHashMismatch { expected: task.file_manifest.AssetHashMd5.clone(), got: file_md5_hash_str(&temp_file_path)?, path: temp_file_path })
        }
    }

    fn assemble_temp_file_from_chunks(&self, temp_file_path: &Path, task: &FileInfo, index: &DownloadingIndex) -> std::io::Result<()> {
        let mut temp_file = File::create(temp_file_path)?;
        temp_file.set_len(task.file_manifest.AssetSize)?;

        for chunk_id in &task.chunks {
            let chunk_info = index.chunks.get(chunk_id).unwrap();
            if chunk_info.download_info.compression == 1 {
                self.write_compressed_chunk_to_file_segment(&mut temp_file, chunk_info)?;
            } else {
                self.write_chunk_to_file_segment(&mut temp_file, chunk_info)?;
            }
        }

        Ok(())
    }

    fn write_chunk_to_file_segment(&self, file: &mut File, chunk_info: &ChunkInfo) -> std::io::Result<()> {
        let chunk_path = self.chunk_temp_folder().join(format!("{}.chunk", chunk_info.chunk_manifest.ChunkName));

        let mut chunk_file = File::open(&chunk_path)?;

        file.seek(SeekFrom::Start(chunk_info.chunk_manifest.ChunkOnFileOffset))?;
        std::io::copy(&mut chunk_file, file)?;

        Ok(())
    }

    fn write_compressed_chunk_to_file_segment(&self, file: &mut File, chunk_info: &ChunkInfo) -> std::io::Result<()> {
        let chunk_path = self.chunk_temp_folder().join(format!("{}.chunk.zstd", chunk_info.chunk_manifest.ChunkName));

        let compressed_chunk_file = File::open(&chunk_path)?;
        let mut zstd_decoder = zstd::Decoder::new(compressed_chunk_file)?;

        file.seek(SeekFrom::Start(chunk_info.chunk_manifest.ChunkOnFileOffset))?;
        std::io::copy(&mut zstd_decoder, file)?;

        Ok(())
    }

    fn check_downloaded_chunk(&self, chunk_info: &ChunkInfo) -> std::io::Result<bool> {
        let (exp_size, exp_hash, _) = chunk_info.chunk_file_info();
        let chunk_path = chunk_info.chunk_path(&self.chunk_temp_folder());
        check_file(&chunk_path, exp_size, exp_hash)
    }

    fn download_chunk(&self, chunk_info: &ChunkInfo) -> Result<(), SophonError> {
        let (chunk_size, chunk_hash, _) = chunk_info.chunk_file_info();

        let chunk_path = chunk_info.chunk_path(&self.chunk_temp_folder());

        if check_file(&chunk_path, chunk_size, chunk_hash)? {
            Ok(())
        } else {
            let chunk_url = chunk_info.download_url();

            let response = self.client.get(&chunk_url)
                .send()?
                .error_for_status()?;

            let chunk_bytes = response.bytes()?;

            std::fs::write(&chunk_path, &chunk_bytes)?;

            Ok(())
        }
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

    pub fn pre_download(
        &self,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        let mut progress = DownloadProgress::new_from_manifest(&self.manifest);

        // Collect deduplicated map of chunks. If some files share chunks, they
        // will not be downloaded more than once.
        let chunks: HashMap<&String, &SophonManifestAssetChunk> = self.manifest.Assets.iter()
            .flat_map(|asset| &asset.AssetChunks)
            .map(|chunk_info| (&chunk_info.ChunkName, chunk_info))
            .collect();

        if self.check_free_space {
            (updater)(Update::CheckingFreeSpace(self.temp_folder.clone()));

            let download_size = self.download_info.stats.compressed_size.parse().unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_size)?;
        }

        /*
        (updater)(Update::DownloadingStarted(self.temp_folder.clone()));
        (updater)(progress.msg_bytes());
        */

        self.create_temp_dirs()?;

        /*
        for (_chunk_id, chunk_info) in chunks {
            if let Err(err) = self.download_chunk_raw(chunk_info, &mut progress) {
                (updater)(Update::DownloadingError(err))
            } else {
                (updater)(progress.msg_bytes())
            }
        }
        */

        self.predownload_multithreaded(14, updater);

        Ok(())
    }

    pub fn install(
        &self,
        output_folder: &Path,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        let mut progress = DownloadProgress::new_from_manifest(&self.manifest);

        let download_size = self.download_info.stats.compressed_size.parse().unwrap();
        let installed_size = self.download_info.stats.uncompressed_size.parse().unwrap();

        tracing::trace!("Checking free space availability");

        if self.check_free_space {
            (updater)(Update::CheckingFreeSpace(self.temp_folder.clone()));

            Self::free_space_check(updater.clone(), &self.temp_folder, download_size)?;

            (updater)(Update::CheckingFreeSpace(output_folder.to_owned()));

            let output_size_to_check = if free_space::is_same_disk(&self.temp_folder, output_folder) {
                download_size + installed_size
            } else {
                installed_size
            };

            Self::free_space_check(updater.clone(), output_folder, output_size_to_check)?;
        }

        tracing::trace!("Downloading files");

        (updater)(Update::DownloadingStarted(self.temp_folder.clone()));

        /*
        (updater)(progress.msg_files());
        (updater)(progress.msg_bytes());
        */

        self.create_temp_dirs()?;

        self.install_multithreaded(14, output_folder, updater.clone());
        //self.download_files(output_folder, updater.clone(), &mut progress);

        (updater)(Update::DownloadingFinished);

        Ok(())
    }

    fn download_files(
        &self,
        output_folder: &Path,
        updater: impl Fn(Update) + Clone + Send + 'static,
        progress: &mut DownloadProgress
    ) {
        for asset_file in &self.manifest.Assets {
            if asset_file.AssetName.ends_with("globalgamemanagers") {
                continue;
            }

            self.download_file_updater_handler(
                output_folder,
                asset_file,
                updater.clone(),
                progress
            );
        }

        if let Some(asset_file) = self.manifest.Assets.iter().find(|asset| asset.AssetName.ends_with("globalgamemanagers")) {
            self.download_file_updater_handler(
                output_folder,
                asset_file,
                updater.clone(),
                progress
            );
        }
    }

    fn download_file_updater_handler(
        &self,
        output_folder: &Path,
        asset_file: &SophonManifestAssetProperty,
        updater: impl Fn(Update) + Clone + Send + 'static,
        progress: &mut DownloadProgress
    ) {
        match self.download_chunked_file(output_folder, asset_file, updater.clone(), progress) {
            Ok(()) => {
                progress.downloaded_files.fetch_add(1, std::sync::atomic::Ordering::AcqRel);

                (updater)(progress.msg_files());
            }

            Err(e) => (updater)(Update::DownloadingError(e))
        }
    }

    // TODO: partial file skip if portion matches chunk hash?

    fn download_chunked_file(
        &self,
        output_folder: &Path,
        file_info: &SophonManifestAssetProperty,
        updater: impl Fn(Update) + Send + 'static,
        progress: &mut DownloadProgress
    ) -> Result<(), SophonError> {
        let out_file_path = output_folder.join(&file_info.AssetName);

        // check if file exists and hash matches to skip download
        if check_file(&out_file_path, file_info.AssetSize, &file_info.AssetHashMd5)? {
            let chunk_sizes = file_info
                .AssetChunks
                .iter()
                .map(|chunk| chunk.ChunkSize)
                .sum::<u64>();
            progress.downloaded_bytes.fetch_add(chunk_sizes, std::sync::atomic::Ordering::AcqRel);

            (updater)(progress.msg_bytes());

            return Ok(());
        }

        let temp_file_path = self.downloading_temp().join(format!("{}.temp", file_info.AssetHashMd5));

        let file = File::create(&temp_file_path).unwrap();

        file.set_len(file_info.AssetSize).unwrap();

        for chunk_info in &file_info.AssetChunks {
            let mut chunk_file = self.download_chunk_uncompressed(chunk_info, progress)?;

            (updater)(progress.msg_bytes());

            let mut buf = Vec::with_capacity(chunk_info.ChunkSizeDecompressed as usize);

            chunk_file.read_to_end(&mut buf)?;

            // Drop chunk file handle early, not needed anymore
            // Also just in case it would prevent deletion (if needed)
            drop(chunk_file);

            file.write_all_at(&buf, chunk_info.ChunkOnFileOffset)?;

            // Chunks downloaded with compression, and the compressed version si likely cached on
            // disk. An uncompressed version just been used, remove it to not duplicate.
            // If the chunk was downlaoded uncompressed - don't remove it
            if self.download_info.chunk_download.compression == 1 {
                let uncompressed_chunk_path = self.chunk_temp_folder().join(format!("{}.chunk", chunk_info.ChunkName));

                std::fs::remove_file(&uncompressed_chunk_path)?;
            }
        }

        drop(file);

        if check_file(&temp_file_path, file_info.AssetSize, &file_info.AssetHashMd5)? {
            ensure_parent(&out_file_path).map_err(|e| SophonError::TempFileError {
                path: temp_file_path.clone(),
                message: e.to_string()
            })?;

            std::fs::copy(&temp_file_path, &out_file_path).map_err(|e| {
                SophonError::OutputFileError {
                    path: temp_file_path.clone(),
                    message: e.to_string()
                }
            })?;

            std::fs::remove_file(&temp_file_path).map_err(|e| SophonError::OutputFileError {
                path: temp_file_path.clone(),
                message: e.to_string()
            })?;

            Ok(())
        }

        else {
            Err(SophonError::FileHashMismatch {
                got: file_md5_hash_str(&temp_file_path)?,
                path: temp_file_path,
                expected: file_info.AssetHashMd5.clone(),
            })
        }
    }

    #[inline]
    fn chunk_download_url(&self, chunk_id: &str) -> String {
        format!(
            "{}{}/{chunk_id}",
            self.download_info.chunk_download.url_prefix,
            self.download_info.chunk_download.url_suffix
        )
    }

    /// Download the chunk is the raw-est state and save to the temp folder, returning the
    /// path is is saved at. If the chunk is compressed, it is saved as `ChunkName.chunk.zstd`,
    /// otehrwise it's saved without `.zstd` file extension.
    /// If the chunk file already exists, checks it and returns the path to it if length and hash
    /// match.
    fn download_chunk_raw(
        &self,
        chunk_info: &SophonManifestAssetChunk,
        progress: &mut DownloadProgress,
    ) -> Result<PathBuf, SophonError> {
        let (chunk_file_name, chunk_size, chunk_hash) = if self.download_info.chunk_download.compression == 1 {
            (
                format!("{}.chunk.zstd", chunk_info.ChunkName),
                chunk_info.ChunkSize,
                &chunk_info.ChunkCompressedHashMd5
            )
        } else {
            (
                format!("{}.chunk", chunk_info.ChunkName),
                chunk_info.ChunkSizeDecompressed,
                &chunk_info.ChunkDecompressedHashMd5
            )
        };

        let chunk_path = self.chunk_temp_folder().join(&chunk_file_name);

        if check_file(&chunk_path, chunk_size, chunk_hash)? {
            progress.count_chunk(chunk_info);

            Ok(chunk_path)
        }

        else {
            let chunk_url = self.chunk_download_url(&chunk_info.ChunkName);

            let response = self.client.get(&chunk_url)
                .send()?
                .error_for_status()?;

            let chunk_bytes = response.bytes()?;

            if chunk_bytes.len() as u64 == chunk_size && bytes_check_md5(&chunk_bytes, chunk_hash) {
                std::fs::write(&chunk_path, &chunk_bytes)?;

                progress.count_chunk(chunk_info);

                Ok(chunk_path)
            }

            else {
                Err(SophonError::ChunkHashMismatch {
                    expected: chunk_hash.to_string(),
                    got: md5_hash_str(&chunk_bytes)
                })
            }
        }
    }

    /// Download the chunk and if it is compressed, decompress it. If a compressed chunk is
    /// downloaded already, it checks that file and uses it to produce a decompressed chunk.
    /// If a decompressed chunk already exists, checks it and returns its File without
    /// redownloading on successfull check.
    fn download_chunk_uncompressed(
        &self,
        chunk_info: &SophonManifestAssetChunk,
        progress: &mut DownloadProgress
    ) -> Result<File, SophonError> {
        let uncompressed_chunk_path = self.chunk_temp_folder().join(format!("{}.chunk", chunk_info.ChunkName));

        let uncompressed_size = chunk_info.ChunkSizeDecompressed;
        let uncompressed_hash = &chunk_info.ChunkDecompressedHashMd5;

        if std::fs::exists(&uncompressed_chunk_path)? && check_file(&uncompressed_chunk_path, uncompressed_size, uncompressed_hash)? {
            File::open(&uncompressed_chunk_path).map_err(Into::into)
        }

        else {
            let raw_chunk_path = self.download_chunk_raw(chunk_info, progress)?;

            if self.download_info.chunk_download.compression == 1 {
                // File is compressed, decompress it
                let file_contents = std::fs::read(&raw_chunk_path)?;
                let decompressed_bytes = zstd::decode_all(&*file_contents)?;

                if decompressed_bytes.len() as u64 == uncompressed_size && bytes_check_md5(&decompressed_bytes, uncompressed_hash) {
                    // Use OpenOptions so that the file doesn't need to be reopened because of
                    // missing `read` option when using `File::create`
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .read(true)
                        .open(&uncompressed_chunk_path)?;

                    file.write_all(&decompressed_bytes)?;

                    // Rewind the cursor
                    file.seek(SeekFrom::Start(0))?;

                    Ok(file)
                }

                else {
                    Err(SophonError::ChunkHashMismatch {
                        expected: uncompressed_hash.to_string(),
                        got: md5_hash_str(&decompressed_bytes)
                    })
                }
            }

            else {
                // File already downloaded uncompressed
                File::open(&raw_chunk_path).map_err(Into::into)
            }
        }
    }
}
