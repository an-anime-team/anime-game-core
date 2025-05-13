use std::{collections::{HashMap, HashSet}, io::{Read, Seek, Take}};
use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;

// I ain't refactoring it.
use super::{
    api_post_request, api_schemas::{
        game_branches::PackageInfo,
        sophon_diff::{SophonDiff, SophonDiffs},
    }, file_md5_hash_str, get_protobuf_from_url, md5_hash_str, protos::SophonPatch::{
        SophonPatchAssetChunk, SophonPatchAssetProperty, SophonPatchProto, SophonUnusedAssetFile
    }, GameEdition, SophonError
};

use crate::{
    external::hpatchz,
    prelude::free_space,
    sophon::{bytes_check_md5, check_file},
    version::Version,
};

fn sophon_patch_info_url(
    password: &str,
    package_id: &str,
    pre_download: bool,
    edition: GameEdition
) -> String {
    format!(
        "{}/downloader/sophon_chunk/api/getPatchBuild?branch={}&password={password}&package_id={package_id}",
        edition.api_host(),
        if pre_download { "pre_download" } else { "main" }
    )
}

#[inline(always)]
pub fn get_game_diffs_sophon_info(
    client: Client,
    package_info: &PackageInfo,
    pre_download: bool,
    edition: GameEdition
) -> Result<SophonDiffs, SophonError> {
    let url = sophon_patch_info_url(
        &package_info.password,
        &package_info.package_id,
        pre_download,
        edition,
    );

    api_post_request(client, &url)
}

pub fn get_patch_manifest(
    client: Client,
    diff_info: &SophonDiff,
) -> Result<SophonPatchProto, SophonError> {
    let url_prefix = &diff_info.manifest_download.url_prefix;
    let url_suffix = &diff_info.manifest_download.url_suffix;
    let manifest_id = &diff_info.manifest.id;

    let download_url = format!("{}{}/{}", url_prefix, url_suffix, manifest_id);

    get_protobuf_from_url(
        &download_url,
        client,
        diff_info.manifest_download.compression == 1,
    )
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
        patched_files: u64,
        total_files: u64,
    },

    DownloadingProgressDeletedFiles {
        deleted_files: u64,
        total_unused: u64,
    },

    DownloadingFinished,
    DownloadingError(SophonError),
    PatchingError(String),

    FileHashCheckFailed(PathBuf)
}

#[derive(Debug)]
pub struct SophonPatcher {
    pub client: Client,
    pub patch_manifest: SophonPatchProto,
    pub diff_info: SophonDiff,
    pub check_free_space: bool,
    pub temp_folder: PathBuf,
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
    fn msg_bytes(&self) -> Update {
        Update::DownloadingProgressBytes {
            downloaded_bytes: self.downloaded_bytes,
            total_bytes: self.total_bytes,
        }
    }

    #[inline]
    fn msg_files(&self) -> Update {
        Update::DownloadingProgressFiles {
            patched_files: self.patched_files,
            total_files: self.total_files,
        }
    }

    #[inline]
    fn msg_deleted(&self) -> Update {
        Update::DownloadingProgressDeletedFiles {
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

impl SophonPatcher {
    pub fn new(
        diff: &SophonDiff,
        client: Client,
        temp_dir: impl AsRef<Path>,
    ) -> Result<Self, SophonError> {
        Ok(Self {
            patch_manifest: get_patch_manifest(client.clone(), diff)?,
            client,
            diff_info: diff.clone(),
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
    pub fn with_temp_folder(mut self, temp_folder: impl Into<PathBuf>) -> Self {
        self.temp_folder = temp_folder.into();

        self
    }

    fn free_space_check(
        updater: impl Fn(Update) + Clone + Send + 'static,
        path: impl AsRef<Path>,
        required: u64
    ) -> Result<(), SophonError> {
        (updater)(Update::CheckingFreeSpace(path.as_ref().to_owned()));

        if let Some(space) = free_space::available(&path) {
            if space >= required {
                return Ok(());
            }

            let err = SophonError::NoSpaceAvailable {
                path: path.as_ref().to_owned(),
                required,
                available: space
            };

            (updater)(Update::DownloadingError(err.clone()));

            Err(err)
        }

        else {
            let err = SophonError::PathNotMounted(path.as_ref().to_owned());

            (updater)(Update::DownloadingError(err.clone()));

            Err(err)
        }
    }

    pub fn pre_download(
        &self,
        from: Version,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        // deduplicated list of all patch chunks. Important because a lot of files share patch
        // chunks, using only a portion of the file.
        let patch_chunks: HashMap<&String, &SophonPatchAssetChunk> = self.patch_manifest.PatchAssets.iter()
            .flat_map(|patch_asset| {
                patch_asset.AssetPatchChunks.iter()
                    .filter_map(|(tag, patch_chunk)| {
                        (*tag == from).then_some((&patch_chunk.PatchName, patch_chunk))
                    })
            })
            .collect();

        let total_bytes = patch_chunks.values()
            .map(|patch_chunk| patch_chunk.PatchSize)
            .sum();

        let mut progress = PatchingStats::new(total_bytes, 0, 0, patch_chunks.values().count());

        if self.check_free_space {
            let download_bytes = self.diff_info.stats.get(&from.to_string()).unwrap()
                .compressed_size.parse().unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_bytes)?;
        }

        (updater)(Update::DownloadingStarted(self.temp_folder.clone()));

        (updater)(progress.msg_bytes());

        for (_name, patch_chunk) in patch_chunks {
            if let Err(err) = self.download_patch_chunk(patch_chunk, &mut progress, updater.clone()) {
                (updater)(Update::DownloadingError(err));
            }
        }

        (updater)(Update::DownloadingFinished);

        Ok(())
    }

    pub fn sophon_apply_patches(
        &self,
        target_dir: impl AsRef<Path>,
        from: Version,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        let unused_assets_for_ver = self.patch_manifest.UnusedAssets.iter()
            .find(|(version, _unused_asset)| **version == from);

        let patch_chunks: HashMap<&String, &SophonPatchAssetChunk> = self.patch_manifest.PatchAssets.iter()
            .flat_map(|patch_asset| {
                patch_asset.AssetPatchChunks.iter()
                    .filter_map(|(tag, patch_chunk)| {
                        (*tag == from).then_some((&patch_chunk.PatchName, patch_chunk))
                    })
            })
            .collect();

        let total_bytes = patch_chunks.values()
            .map(|patch_chunk| patch_chunk.PatchSize)
            .sum();

        let total_files = self.patch_manifest.PatchAssets.len() as u64;

        let total_unused = unused_assets_for_ver.map(|(_, assets_info)| assets_info.Assets.len() as u64);

        let mut progress = PatchingStats::new(total_bytes, total_files, total_unused.unwrap_or(0), patch_chunks.values().count());

        if self.check_free_space {
            let download_bytes = self.diff_info.stats.get(&from.to_string()).unwrap()
                .compressed_size.parse().unwrap();

            Self::free_space_check(updater.clone(), &self.temp_folder, download_bytes)?;
        }

        (updater)(Update::DownloadingStarted(target_dir.as_ref().to_owned()));
        (updater)(progress.msg_bytes());
        (updater)(progress.msg_files());
        (updater)(progress.msg_deleted());

        if let Some((_unused_ver, unused_assets)) = unused_assets_for_ver {
            let result = self.remove_unused_files(
                &unused_assets.Assets,
                &target_dir,
                &mut progress,
                updater.clone()
            );

            if let Err(err) = result {
                (updater)(Update::DownloadingError(err));
            }
        }

        for file_patch_info in &self.patch_manifest.PatchAssets {
            if file_patch_info.AssetName.ends_with("globalgamemanagers") {
                continue;
            }
            self.patch_file_updater_handler(file_patch_info, &target_dir, from, &mut progress, updater.clone());
        }

        if let Some(file_patch_info) = self.patch_manifest.PatchAssets.iter().find(|passet| passet.AssetName.ends_with("globalgamemanagers")) {
            self.patch_file_updater_handler(file_patch_info, target_dir, from, &mut progress, updater);
        }

        Ok(())
    }

    fn patch_file_updater_handler(&self, file_patch_info: &SophonPatchAssetProperty, target_dir: impl AsRef<Path>, installed_ver: Version, progress: &mut PatchingStats, updater: impl Fn(Update) + Clone + Send + 'static) {
        let result = self.sophon_patch_file(
            file_patch_info,
            &target_dir,
            installed_ver,
            progress,
            updater.clone(),
        );

        if let Err(err) = result {
            (updater)(Update::DownloadingError(err));
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
                    updater.clone(),
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
                    updater.clone(),
                )?;
            }
        }

        else {
            let valid_file = check_file(
                &target_file_path,
                patch_info.AssetSize,
                &patch_info.AssetHashMd5,
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

        let patch_path_tmp = self.temp_folder.join(
            format!("{}-{}.hdiff",patch_chunk.OriginalFileMd5, asset_info.AssetHashMd5)
        );

        extract_patch_chunk_region_to_file(patch_chunk_file, &patch_path_tmp, patch_chunk)?;

        let tmp_out_file_path = self
            .temp_folder
            .join(format!("{}.tmp", &asset_info.AssetHashMd5));

        if let Err(err) = hpatchz::patch(from.as_ref(), &patch_path_tmp, &tmp_out_file_path) {
            (updater)(Update::PatchingError(err.to_string()));
            return Ok(());
        }

        tracing::trace!("Checking post-patch");

        let valid_file = check_file(
            &tmp_out_file_path,
            asset_info.AssetSize,
            &asset_info.AssetHashMd5,
        )?;

        if !valid_file {
            (updater)(Update::FileHashCheckFailed(tmp_out_file_path.clone()));

            let file_hash = file_md5_hash_str(&tmp_out_file_path)?;

            return Err(SophonError::FileHashMismatch {
                path: tmp_out_file_path,
                expected: asset_info.AssetHashMd5.clone(),
                got: file_hash,
            });
        }

        // Delete original if patching is also a move
        if asset_info.AssetName != patch_chunk.OriginalFileName {
            std::fs::remove_file(from)?;
        }

        std::fs::copy(&tmp_out_file_path, &to)?;

        // tmp file was checked, doesn't need to be checked after copy
        std::fs::remove_file(&tmp_out_file_path)?;
        std::fs::remove_file(&patch_path_tmp)?;

        Ok(())
    }

    fn copy_over_file(
        &self,
        file_path: impl AsRef<Path>,
        patch_chunk: &SophonPatchAssetChunk,
        expected_size: u64,
        expected_md5: &str,
        progress: &mut PatchingStats,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> Result<(), SophonError> {
        let patch_chunk_path = self.download_patch_chunk(patch_chunk, progress, updater.clone())?;

        let tmp_file_path = self.temp_folder.join(format!("{}.tmp", expected_md5));

        extract_patch_chunk_region_to_file(&patch_chunk_path, &tmp_file_path, patch_chunk)?;

        if !check_file(&tmp_file_path, expected_size, expected_md5)? {
            (updater)(Update::FileHashCheckFailed(tmp_file_path.clone()));

            let file_hash = file_md5_hash_str(&tmp_file_path)?;

            Err(SophonError::FileHashMismatch {
                path: tmp_file_path,
                expected: expected_md5.to_owned(),
                got: file_hash,
            })
        }

        else {
            std::fs::copy(&tmp_file_path, &file_path)?;
            std::fs::remove_file(tmp_file_path)?;

            Ok(())
        }
    }

    fn remove_unused_files(
        &self,
        unused_assets: &[SophonUnusedAssetFile],
        target_dir: impl AsRef<Path>,
        progress: &mut PatchingStats,
        updater: impl Fn(Update) + Clone + Send + 'static
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
        updater: impl Fn(Update) + Clone + Send + 'static,
    ) -> Result<PathBuf, SophonError> {
        let patch_path = self.temp_folder.join(format!("{}.patch_chunk", patch_chunk_info.PatchName));

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

fn extract_patch_chunk_region_to_file(patch_chunk_file: impl AsRef<Path>, out_path: impl AsRef<Path>, patch_chunk: &SophonPatchAssetChunk) -> std::io::Result<()> {
    let mut patch_data = extract_patch_chunk_region(
        patch_chunk_file,
        patch_chunk.PatchOffset,
        patch_chunk.PatchLength,
    )?;

    let mut patch_file = File::create(out_path)?;

    std::io::copy(&mut patch_data, &mut patch_file)?;

    Ok(())
}
