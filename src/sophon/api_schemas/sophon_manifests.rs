use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SophonDownloads {
    pub build_id: String,
    pub tag: String,
    pub manifests: Vec<SophonDownloadInfo>
}

impl SophonDownloads {
    /// `matching_field` is usually either `game` or one of the voiceover language options
    pub fn get_manifests_for(&self, matching_field: &str) -> Option<&SophonDownloadInfo> {
        self.manifests.iter()
            .find(|man| man.matching_field == matching_field)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SophonDownloadInfo {
    pub category_id: String,
    pub category_name: String,
    pub matching_field: String,
    pub manifest: Manifest,
    pub chunk_download: DownloadInfo,
    pub manifest_download: DownloadInfo,
    pub stats: ManifestStats,
    pub deduplicated_stats: ManifestStats
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub checksum: String,
    pub compressed_size: String,
    pub uncompressed_size: String
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub encryption: u8,
    pub password: String,
    pub compression: u8,
    pub url_prefix: String,
    pub url_suffix: String
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestStats {
    pub compressed_size: String,
    pub uncompressed_size: String,
    pub file_count: String,
    pub chunk_count: String
}
