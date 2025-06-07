use std::{collections::HashMap, sync::{atomic::AtomicU64, Mutex}};
use std::io::{Seek, SeekFrom};
use std::fs::File;
use std::path::{Path, PathBuf};

use crossbeam_deque::{Injector, Steal};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

// I ain't refactoring all this.
use super::{
    api_get_request, api_schemas::{
        game_branches::PackageInfo,
        sophon_manifests::{DownloadInfo, SophonDownloadInfo, SophonDownloads},
    }, check_file, ensure_parent, file_md5_hash_str, get_protobuf_from_url, protos::SophonManifest::{
        SophonManifestAssetChunk, SophonManifestAssetProperty, SophonManifestProto,
    }, ChunkState, GameEdition, SophonError
};

use crate::{prelude::free_space, sophon::DEFAULT_CHUNK_RETRIES};

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
    files: HashMap<&'a String, FileInfo<'a>>,
    total_bytes: u64,
    downloaded_bytes: AtomicU64,
    downloaded_files: AtomicU64
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
            total_bytes: Self::total_bytes_calculate(&chunks),
            chunks,
            files,
            downloaded_bytes: AtomicU64::new(0),
            downloaded_files: AtomicU64::new(0),
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
            total_bytes: Self::total_bytes_calculate(&chunks),
            chunks,
            files: HashMap::new(),
            downloaded_bytes: AtomicU64::new(0),
            downloaded_files: AtomicU64::new(0),
        }
    }

    fn total_bytes_calculate(chunks: &HashMap<&'a String, ChunkInfo<'a>>) -> u64 {
        chunks.values().map(|chunk| chunk.chunk_manifest.ChunkSize).sum()
    }

    #[inline(always)]
    fn total_files(&self) -> u64 {
        self.files.len() as u64
    }

    fn msg_files(&self) -> Update {
        Update::DownloadingProgressFiles {
            downloaded_files: self.downloaded_files.load(std::sync::atomic::Ordering::Acquire),
            total_files: self.total_files()
        }
    }

    fn msg_bytes(&self) -> Update {
        Update::DownloadingProgressBytes {
            downloaded_bytes: self.downloaded_bytes.load(std::sync::atomic::Ordering::Acquire),
            total_bytes: self.total_bytes
        }
    }

    fn add_files(&self, amount: u64) {
        self.downloaded_files.fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }

    fn add_bytes(&self, amount: u64) {
        self.downloaded_bytes.fetch_add(amount, std::sync::atomic::Ordering::SeqCst);
    }
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

    fn fail_chunk<'a>(&self, chunk_info: &'a ChunkInfo<'a>, states: &Mutex<HashMap<&'a String, ChunkState>>, download_queue: &Injector<&'a ChunkInfo<'a>>, updater: impl Fn(Update)) {
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

    fn predownload_multithreaded(&self, thread_count: usize, updater: impl Fn(Update) + Clone + Send + 'static) {
        tracing::debug!("Starting multithreaded predownload");

        let downloading_index = DownloadingIndex::new_chunks_only(&self.download_info, &self.manifest);
        tracing::info!("{} Chunks to download", downloading_index.chunks.len());
        let chunk_states: Mutex<HashMap<&String, ChunkState>> = Mutex::new(HashMap::from_iter(downloading_index.chunks.keys().map(|id| (*id, ChunkState::Downloading(DEFAULT_CHUNK_RETRIES)))));

        (updater)(downloading_index.msg_files());
        (updater)(downloading_index.msg_bytes());

        let download_queue: Injector<&ChunkInfo> = Injector::new();
        for chunk_info in downloading_index.chunks.values() {
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
                            self.chunk_check_handler(chunk_check_task, &chunk_states, &downloading_index, &local_updater, None, &download_queue);
                            continue;
                        }
                        if let Steal::Success(chunk_download_task) = download_queue.steal() {
                            self.chunk_download_handler(chunk_download_task, &chunk_states, &download_queue, &chunk_check_queue, &local_updater);
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

        let downloading_index = DownloadingIndex::new(&self.download_info, &self.manifest);
        tracing::info!("{} Chunks to download, {} Files to install", downloading_index.chunks.len(), downloading_index.files.len());
        let chunk_states: Mutex<HashMap<&String, ChunkState>> = Mutex::new(HashMap::from_iter(downloading_index.chunks.keys().map(|id| (*id, ChunkState::default()))));

        (updater)(downloading_index.msg_files());
        (updater)(downloading_index.msg_bytes());

        let download_queue: Injector<&ChunkInfo> = Injector::new();
        for chunk_info in downloading_index.chunks.values() {
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
                            self.file_handler(file_task, &downloading_index, out_folder, &local_updater);
                            continue;
                        }
                        // Check downloaded chunk
                        if let Steal::Success(chunk_check_task) = chunk_check_queue.steal() {
                            self.chunk_check_handler(chunk_check_task, &chunk_states, &downloading_index, &local_updater, Some(&file_queue), &download_queue);
                            continue;
                        }
                        // Download next chunk
                        if let Steal::Success(chunk_download_task) = download_queue.steal() {
                            self.chunk_download_handler(chunk_download_task, &chunk_states, &download_queue, &chunk_check_queue, &local_updater);
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

    fn file_handler(&self, file_task: &FileInfo, downloading_index: &DownloadingIndex, out_folder: &Path, updater: impl Fn(Update)) {
        tracing::trace!("Assembling final file `{}`", file_task.file_manifest.AssetName);
        if let Err(err) = self.file_assemble(out_folder, file_task, downloading_index) {
            tracing::error!("Error assembling file `{}`: {err}", file_task.file_manifest.AssetName);
            (updater)(Update::DownloadingError(err));
        } else {
            tracing::trace!("Finished `{}`", file_task.file_manifest.AssetName);
            downloading_index.add_files(1);
            (updater)(downloading_index.msg_files());
        };
    }

    fn chunk_check_handler<'a>(
        &self,
        chunk_check_task: &'a ChunkInfo<'a>,
        chunk_states: &Mutex<HashMap<&'a String, ChunkState>>,
        downloading_index: &'a DownloadingIndex<'a>,
        local_updater: impl Fn(Update),
        file_queue: Option<&Injector<&'a FileInfo<'a>>>,
        download_queue: &Injector<&'a ChunkInfo<'a>>
    ) {
        let res = self.check_downloaded_chunk(chunk_check_task);
        match res {
            Ok(true) => {
                tracing::trace!("Successfully downloaded chunk `{}`", chunk_check_task.chunk_manifest.ChunkName);
                {
                    let mut states_lock = chunk_states.lock().unwrap();
                    let chunk_state = states_lock.get_mut(&chunk_check_task.chunk_manifest.ChunkName).unwrap();
                    *chunk_state = ChunkState::Downloaded;
                }
                if let Some(file_queue) = file_queue {
                    for file_name in &chunk_check_task.used_in_files {
                        let file_info = downloading_index.files.get(*file_name).unwrap();
                        if file_info.is_file_ready(chunk_states) {
                            tracing::trace!("File `{}` is ready for assembly, pushing on queue", file_name);
                            file_queue.push(file_info);
                        }
                    }
                }
                downloading_index.add_bytes(chunk_check_task.chunk_manifest.ChunkSize);
                (local_updater)(downloading_index.msg_bytes());
            },
            Ok(false) => {
                tracing::trace!("Chunk `{}` failed size+hash check", chunk_check_task.chunk_manifest.ChunkName);
                self.fail_chunk(chunk_check_task, chunk_states, download_queue, local_updater);
            },
            Err(err) => {
                tracing::error!("I/O error checking chunk `{}`: {err}", chunk_check_task.chunk_manifest.ChunkName);
                (local_updater)(Update::DownloadingError(err.into()));
                self.fail_chunk(chunk_check_task, chunk_states, download_queue, local_updater);
            }
        }
    }

    fn chunk_download_handler<'a>(
        &self,
        chunk_download_task: &'a ChunkInfo<'a>,
        chunk_states: &Mutex<HashMap<&'a String, ChunkState>>,
        download_queue: &Injector<&'a ChunkInfo<'a>>,
        chunk_check_queue: &Injector<&'a ChunkInfo<'a>>,
        local_updater: impl Fn(Update)
    ) {
        let res = self.download_chunk(chunk_download_task);
        match res {
            Ok(()) => {
                chunk_check_queue.push(chunk_download_task);
            },
            Err(err) => {
                tracing::error!("Error downloading chunk `{}`: {err}", chunk_download_task.chunk_manifest.ChunkName);
                (local_updater)(Update::DownloadingError(err));
                self.fail_chunk(chunk_download_task, chunk_states, download_queue, local_updater);
            }
        }
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
        if self.check_free_space {
            (updater)(Update::CheckingFreeSpace(self.temp_folder.clone()));

            let download_size = self.download_info.stats.compressed_size.parse().unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_size)?;
        }

        self.create_temp_dirs()?;

        self.predownload_multithreaded(14, updater);

        Ok(())
    }

    pub fn install(
        &self,
        output_folder: &Path,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
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

        self.create_temp_dirs()?;

        self.install_multithreaded(14, output_folder, updater.clone());

        (updater)(Update::DownloadingFinished);

        Ok(())
    }
}
