use serde::{Deserialize, Serialize};
use sophon_diff::SophonDiff;
use sophon_manifests::SophonDownloadInfo;

pub mod game_branches;
pub mod sophon_diff;
pub mod sophon_manifests;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub retcode: i16,
    pub message: String,
    pub data: T,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadOrDiff {
    Download(SophonDownloadInfo),
    Patch(SophonDiff),
}
