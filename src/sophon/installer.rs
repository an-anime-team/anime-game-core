use std::collections::{HashMap, HashSet};
use std::io::{Read, Write, Seek, SeekFrom};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::os::unix::fs::FileExt;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

// I ain't refactoring all this.
use super::{
    api_get_request,
    api_schemas::{
        game_branches::PackageInfo,
        sophon_manifests::{SophonDownloadInfo, SophonDownloads},
    },
    bytes_check_md5, check_file, ensure_parent, get_protobuf_from_url, md5_hash_str,
    protos::SophonManifest::{
        SophonManifestAssetChunk, SophonManifestAssetProperty, SophonManifestProto,
    },
    GameEdition, SophonError,
};

use crate::prelude::free_space;

fn sophon_download_info_url(
    password: &str,
    package_id: &str,
    pre_download: bool,
    edition: GameEdition,
) -> String {
    format!(
        "{}/downloader/sophon_chunk/api/getBuild?branch={}&password={password}&package_id={package_id}",
        edition.api_host(),
        if pre_download { "pre_download" } else { "main" }
    )
}

#[inline(always)]
pub fn get_game_download_sophon_info(
    client: Client,
    package_info: &PackageInfo,
    pre_download: bool,
    edition: GameEdition,
) -> Result<SophonDownloads, SophonError> {
    let url = sophon_download_info_url(
        &package_info.password,
        &package_info.package_id,
        pre_download,
        edition
    );

    api_get_request(client, &url)
}

pub fn get_download_manifest(
    client: Client,
    download_info: &SophonDownloadInfo
) -> Result<SophonManifestProto, SophonError> {
    let url_prefix = &download_info.manifest_download.url_prefix;
    let url_suffix = &download_info.manifest_download.url_suffix;
    let manifest_id = &download_info.manifest.id;

    let download_url = format!("{}{}/{}", url_prefix, url_suffix, manifest_id);

    get_protobuf_from_url(
        &download_url,
        client,
        download_info.manifest_download.compression == 1
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub total_bytes: u64,
    pub total_files: u64,
    pub downloaded_bytes: u64,
    pub downloaded_files: u64,
    pub downloaded_chunks: HashSet<String>,
}

impl DownloadProgress {
    fn new_from_manifest(manifest: &SophonManifestProto) -> Self {
        Self {
            total_bytes: manifest.total_bytes_compressed(),
            total_files: manifest.total_files(),
            downloaded_bytes: 0,
            downloaded_files: 0,
            downloaded_chunks: HashSet::with_capacity(manifest.total_chunks() as usize)
        }
    }

    fn msg_files(&self) -> Update {
        Update::DownloadingProgressFiles {
            downloaded_files: self.downloaded_files,
            total_files: self.total_files,
        }
    }

    fn msg_bytes(&self) -> Update {
        Update::DownloadingProgressBytes {
            downloaded_bytes: self.downloaded_bytes,
            total_bytes: self.total_bytes
        }
    }

    fn count_chunk(&mut self, chunk_info: &SophonManifestAssetChunk) {
        if !self.downloaded_chunks.contains(&chunk_info.ChunkName) {
            self.downloaded_bytes += chunk_info.ChunkSize;

            self.downloaded_chunks.insert(chunk_info.ChunkName.clone());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Update {
    CheckingFreeSpace(PathBuf),

    /// `(temp path)`
    DownloadingStarted(PathBuf),

    DownloadingProgressBytes {
        downloaded_bytes: u64,
        total_bytes: u64,
    },

    DownloadingProgressFiles {
        downloaded_files: u64,
        total_files: u64,
    },

    DownloadingFinished,
    DownloadingError(SophonError),

    FileHashCheckFailed(PathBuf)
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
        download_info: &SophonDownloadInfo,
        client: Client,
        temp_dir: impl AsRef<Path>
    ) -> Result<Self, SophonError> {
        let manifest = get_download_manifest(client.clone(), download_info)?;
        Ok(Self {
            client,
            manifest,
            download_info: download_info.clone(),
            check_free_space: true,
            temp_folder: temp_dir.as_ref().to_owned()
        })
    }

    #[inline(always)]
    pub fn with_free_space_check(mut self, check: bool) -> Self {
        self.check_free_space = check;

        self
    }

    #[inline(always)]
    pub fn with_temp_folder(mut self, temp_folder: PathBuf) -> Self {
        self.temp_folder = temp_folder;

        self
    }

    /// Folder to temporarily store files being downloaded
    #[inline(always)]
    pub fn downloading_temp(&self) -> PathBuf {
        self.temp_folder.join("downloading")
    }

    /// Folder to temporarily store chunks
    #[inline(always)]
    fn chunk_temp_folder(&self) -> PathBuf {
        self.downloading_temp().join("chunks")
    }

    /// Create all needed sub-directories in the temp folder
    fn create_temp_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.downloading_temp())?;
        std::fs::create_dir_all(self.chunk_temp_folder())?;

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

        (updater)(Update::DownloadingStarted(self.temp_folder.clone()));
        (updater)(progress.msg_bytes());

        self.create_temp_dirs()?;

        for (_chunk_id, chunk_info) in chunks {
            if let Err(err) = self.download_chunk_raw(chunk_info, &mut progress) {
                (updater)(Update::DownloadingError(err))
            } else {
                (updater)(progress.msg_bytes())
            }
        }

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

        (updater)(progress.msg_files());
        (updater)(progress.msg_bytes());

        self.create_temp_dirs()?;

        self.download_files(output_folder, updater.clone(), &mut progress);

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
                progress.downloaded_files += 1;

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
            progress.downloaded_bytes += file_info.AssetChunks.iter()
                .map(|chunk| chunk.ChunkSize)
                .sum::<u64>();

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

        let file_contents = std::fs::read(&temp_file_path)
            .map_err(|e| SophonError::TempFileError {
                path: temp_file_path.clone(),
                message: e.to_string(),
            })?;

        if bytes_check_md5(&file_contents, &file_info.AssetHashMd5) {
            ensure_parent(&out_file_path).map_err(|e| SophonError::TempFileError {
                path: temp_file_path.clone(),
                message: e.to_string(),
            })?;

            std::fs::copy(&temp_file_path, &out_file_path).map_err(|e| {
                SophonError::OutputFileError {
                    path: temp_file_path.clone(),
                    message: e.to_string(),
                }
            })?;

            std::fs::remove_file(&temp_file_path).map_err(|e| SophonError::OutputFileError {
                path: temp_file_path.clone(),
                message: e.to_string(),
            })?;

            Ok(())
        }

        else {
            Err(SophonError::FileHashMismatch {
                path: temp_file_path,
                expected: file_info.AssetHashMd5.clone(),
                got: md5_hash_str(&file_contents),
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
