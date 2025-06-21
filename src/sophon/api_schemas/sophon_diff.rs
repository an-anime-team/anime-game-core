use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::sophon_manifests::{DownloadInfo, Manifest, ManifestStats};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SophonDiffs {
    pub build_id: String,
    pub patch_id: String,
    pub tag: String,
    pub manifests: Vec<SophonDiff>
}

impl SophonDiffs {
    /// `matching_field` is usually either `game` or one of the voiceover language options
    pub fn get_manifests_for(&self, matching_field: &str) -> Option<&SophonDiff> {
        self.manifests
            .iter()
            .find(|manifest| manifest.matching_field == matching_field)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SophonDiff {
    pub category_id: String,
    pub category_name: String,
    pub matching_field: String,
    pub manifest: Manifest,
    pub diff_download: DownloadInfo,
    pub manifest_download: DownloadInfo,
    pub stats: BTreeMap<String, ManifestStats>
}
