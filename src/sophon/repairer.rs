use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use super::api_schemas::sophon_manifests::{DownloadInfo, SophonDownloadInfo};
use super::installer::get_download_manifest;
use super::protos::SophonManifest::{
    SophonManifestAssetChunk, SophonManifestAssetProperty, SophonManifestProto
};
use super::{
    bytes_check_md5, check_file, ensure_parent, file_md5_hash_str, file_region_hash_md5,
    md5_hash_str, SophonError
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    VerifyingStarted,

    VerifyingProgress { total: u64, checked: u64 },

    VerifyingFinished { broken: u64 },

    RepairingStarted,

    RepairingProgress { total: u64, repaired: u64 },

    RepairingFinished,

    DownloadingError(SophonError),

    FileHashCheckFailed(PathBuf)
}

pub struct SophonRepairer {
    pub client: Client,
    pub manifests: Vec<(SophonDownloadInfo, SophonManifestProto)>,
    pub temp_folder: PathBuf
}

impl SophonRepairer {
    pub fn new(
        client: Client,
        temp_dir: impl Into<PathBuf>,
        download_infos: impl IntoIterator<Item = SophonDownloadInfo>
    ) -> Result<Self, SophonError> {
        let manifests = download_infos
            .into_iter()
            .map(|download_info| {
                get_download_manifest(&client, &download_info)
                    .map(|manifest| (download_info, manifest))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            client,
            manifests,
            temp_folder: temp_dir.into()
        })
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

    pub fn check_and_repair(
        &self,
        game_dir: impl AsRef<Path>,
        file_check_threads: usize,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        (updater)(Update::VerifyingStarted);

        let broken = self.get_broken_files(&game_dir, file_check_threads, updater.clone())?;

        let total_broken = broken.len() as u64;

        (updater)(Update::VerifyingFinished {
            broken: total_broken
        });
        (updater)(Update::RepairingStarted);

        self.create_temp_dirs()?;

        let mut repaired = 0;

        (updater)(Update::RepairingProgress {
            total: total_broken,
            repaired
        });

        for (download_info, asset_info) in broken {
            let res = self.repair_file(&game_dir, download_info, asset_info);

            match res {
                Err(err) => {
                    tracing::error!(
                        ?err,
                        file_name = asset_info.AssetName,
                        "Failed to repair file"
                    );

                    (updater)(Update::DownloadingError(err))
                }

                Ok(()) => {
                    tracing::trace!(file_name = asset_info.AssetName, "Repaired file");

                    repaired += 1;

                    (updater)(Update::RepairingProgress {
                        total: total_broken,
                        repaired
                    })
                }
            }
        }

        (updater)(Update::RepairingFinished);

        Ok(())
    }

    pub fn get_broken_files(
        &self,
        game_dir: impl AsRef<Path>,
        threads: usize,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> std::io::Result<Vec<(&DownloadInfo, &SophonManifestAssetProperty)>> {
        let all_files = self
            .manifests
            .iter()
            .flat_map(|(download_info, manifest)| {
                manifest
                    .Assets
                    .iter()
                    .map(move |asset| (&download_info.chunk_download, asset))
            })
            .collect::<Vec<_>>();

        let total = all_files.len() as u64;

        (updater)(Update::VerifyingProgress { total, checked: 0 });

        let files_to_check_pool = Mutex::new(all_files);
        let verified_atomic = AtomicU64::new(0);

        let pool = &files_to_check_pool;

        let (sender, receiver) = std::sync::mpsc::channel();

        // scoped threads to allow borrowing some stuff from outer stuff.
        Ok(std::thread::scope(|scope| {
            let verified = &verified_atomic;

            for _ in 0..threads {
                let game_dir = game_dir.as_ref();
                let sender_clone = sender.clone();
                let updater_clone = updater.clone();

                scope.spawn(move || 'check: loop {
                    let (download_info, next_file) = {
                        let mut pool_lock = pool
                            .lock()
                            .expect("failed to lock files repairing pool mutex");

                        let Some(next) = pool_lock.pop()
                        else {
                            break 'check;
                        };

                        next
                    };

                    tracing::trace!(file_name = next_file.AssetName, "Checking file");

                    let result = check_file(
                        game_dir.join(&next_file.AssetName),
                        next_file.AssetSize,
                        &next_file.AssetHashMd5
                    );

                    match result {
                        Err(err) => tracing::error!(
                            ?err,
                            file_name = next_file.AssetName,
                            "Failed to check file"
                        ),

                        Ok(false) => {
                            let _ = sender_clone.send((download_info, next_file));
                        }

                        Ok(true) => ()
                    };

                    tracing::trace!(file_name = next_file.AssetName, "Check completed");

                    (updater_clone)(Update::VerifyingProgress {
                        total,
                        checked: verified.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
                    });
                });
            }

            drop(sender);

            receiver.into_iter().collect()
        }))
    }

    pub fn repair_file(
        &self,
        game_dir: impl AsRef<Path>,
        download_info: &DownloadInfo,
        asset_info: &SophonManifestAssetProperty
    ) -> Result<(), SophonError> {
        let target_file = game_dir.as_ref().join(&asset_info.AssetName);

        if !target_file.exists() {
            return self.download_chunked_file(&game_dir, download_info, asset_info);
        }

        let mut file = OpenOptions::new()
            .write(true)
            .read(true)
            .open(&target_file)?;

        file.set_len(asset_info.AssetSize)?;

        for chunk in &asset_info.AssetChunks {
            let region_hash = file_region_hash_md5(
                &mut file,
                chunk.ChunkOnFileOffset,
                chunk.ChunkSizeDecompressed
            )?;

            if chunk.ChunkDecompressedHashMd5 != region_hash {
                let mut chunk_file = self.download_chunk_uncompressed(download_info, chunk)?;

                file.seek(SeekFrom::Start(chunk.ChunkOnFileOffset))?;

                std::io::copy(&mut chunk_file, &mut file)?;
            }
        }

        drop(file);

        if !check_file(&target_file, asset_info.AssetSize, &asset_info.AssetHashMd5)? {
            return Err(SophonError::FileHashMismatch {
                got: file_md5_hash_str(&target_file)?,
                path: target_file,
                expected: asset_info.AssetHashMd5.clone()
            });
        }

        Ok(())
    }

    fn download_chunked_file(
        &self,
        output_folder: impl AsRef<Path>,
        download_info: &DownloadInfo,
        file_info: &SophonManifestAssetProperty
    ) -> Result<(), SophonError> {
        let out_file_path = output_folder.as_ref().join(&file_info.AssetName);

        // check if file exists and hash matches to skip download
        if check_file(&out_file_path, file_info.AssetSize, &file_info.AssetHashMd5)? {
            return Ok(());
        }

        let temp_file_path = self
            .downloading_temp()
            .join(format!("{}.temp", file_info.AssetHashMd5));

        let file = File::create(&temp_file_path)?;

        file.set_len(file_info.AssetSize)?;

        for chunk_info in &file_info.AssetChunks {
            let mut chunk_file = self.download_chunk_uncompressed(download_info, chunk_info)?;

            let mut buf = Vec::with_capacity(chunk_info.ChunkSizeDecompressed as usize);

            chunk_file.read_to_end(&mut buf)?;

            // Drop chunk file handle early, not needed anymore
            // Also just in case it would prevent deletion (if needed)
            drop(chunk_file);

            file.write_all_at(&buf, chunk_info.ChunkOnFileOffset)?;

            // Chunks downloaded with compression, and the compressed version si likely cached on
            // disk. An uncompressed version just been used, remove it to not duplicate.
            // If the chunk was downlaoded uncompressed - don't remove it
            if download_info.compression == 1 {
                let uncompressed_chunk_path = self
                    .chunk_temp_folder()
                    .join(format!("{}.chunk", chunk_info.ChunkName));

                std::fs::remove_file(&uncompressed_chunk_path)?;
            }
        }

        drop(file);

        if check_file(
            &temp_file_path,
            file_info.AssetSize,
            &file_info.AssetHashMd5
        )? {
            ensure_parent(&out_file_path).map_err(|e| SophonError::TempFileError {
                path: temp_file_path.clone(),
                message: e.to_string()
            })?;

            std::fs::copy(&temp_file_path, &out_file_path).map_err(|err| {
                SophonError::OutputFileError {
                    path: temp_file_path.clone(),
                    message: err.to_string()
                }
            })?;

            std::fs::remove_file(&temp_file_path).map_err(|err| SophonError::OutputFileError {
                path: temp_file_path.clone(),
                message: err.to_string()
            })?;

            Ok(())
        }
        else {
            Err(SophonError::FileHashMismatch {
                got: file_md5_hash_str(&temp_file_path)?,
                path: temp_file_path,
                expected: file_info.AssetHashMd5.clone()
            })
        }
    }

    /// Download the chunk is the raw-est state and save to the temp folder, returning the
    /// path is is saved at. If the chunk is compressed, it is saved as `ChunkName.chunk.zstd`,
    /// otehrwise it's saved without `.zstd` file extension.
    /// If the chunk file already exists, checks it and returns the path to it if length and hash
    /// match.
    fn download_chunk_raw(
        &self,
        download_info: &DownloadInfo,
        chunk_info: &SophonManifestAssetChunk
    ) -> Result<PathBuf, SophonError> {
        let (chunk_file_name, chunk_size, chunk_hash) = if download_info.compression == 1 {
            (
                format!("{}.chunk.zstd", chunk_info.ChunkName),
                chunk_info.ChunkSize,
                &chunk_info.ChunkCompressedHashMd5
            )
        }
        else {
            (
                format!("{}.chunk", chunk_info.ChunkName),
                chunk_info.ChunkSizeDecompressed,
                &chunk_info.ChunkDecompressedHashMd5
            )
        };

        let chunk_path = self.chunk_temp_folder().join(&chunk_file_name);

        if check_file(&chunk_path, chunk_size, chunk_hash)? {
            Ok(chunk_path)
        }
        else {
            let chunk_url = download_info.download_url(&chunk_info.ChunkName);

            let response = self.client.get(&chunk_url).send()?.error_for_status()?;

            let chunk_bytes = response.bytes()?;

            if chunk_bytes.len() as u64 == chunk_size && bytes_check_md5(&chunk_bytes, chunk_hash) {
                std::fs::write(&chunk_path, &chunk_bytes)?;

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
        download_info: &DownloadInfo,
        chunk_info: &SophonManifestAssetChunk
    ) -> Result<File, SophonError> {
        let uncompressed_chunk_path = self
            .chunk_temp_folder()
            .join(format!("{}.chunk", chunk_info.ChunkName));

        let uncompressed_size = chunk_info.ChunkSizeDecompressed;
        let uncompressed_hash = &chunk_info.ChunkDecompressedHashMd5;

        let result = uncompressed_chunk_path.is_file()
            && check_file(
                &uncompressed_chunk_path,
                uncompressed_size,
                uncompressed_hash
            )?;

        if result {
            return Ok(File::open(&uncompressed_chunk_path)?);
        }

        let raw_chunk_path = self.download_chunk_raw(download_info, chunk_info)?;

        if download_info.compression == 1 {
            // File is compressed, decompress it
            let file_contents = std::fs::read(&raw_chunk_path)?;
            let decompressed_bytes = zstd::decode_all(&*file_contents)?;

            if decompressed_bytes.len() as u64 == uncompressed_size
                && bytes_check_md5(&decompressed_bytes, uncompressed_hash)
            {
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
            Ok(File::open(&raw_chunk_path)?)
        }
    }
}
