use serde::{Deserialize, Serialize};

use crate::version::Version;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameBranches {
    pub game_branches: Vec<GameBranchInfo>
}

impl GameBranches {
    /// Get the latest version for the requested id.
    pub fn latest_version_by_id(&self, id: impl AsRef<str>) -> Option<Version> {
        let id = id.as_ref();

        self.game_branches
            .iter()
            .filter(|branch_info| branch_info.game.id == id)
            .max_by_key(|branch_info| &branch_info.main.tag)
            .and_then(|branch_info| Version::from_str(&branch_info.main.tag))
    }

    /// Get `GameBranchInfo` of a specified id and game version.
    pub fn get_game_by_id(&self, id: impl AsRef<str>, version: Version) -> Option<&GameBranchInfo> {
        let id = id.as_ref();
        let version = version.to_string();

        self.game_branches
            .iter()
            .find(|branch_info| branch_info.game.id == id && branch_info.main.tag == version)
    }

    /// Get latest version of specified game by id.
    pub fn get_game_latest_by_id(&self, id: impl AsRef<str>) -> Option<&GameBranchInfo> {
        let id = id.as_ref();

        self.game_branches
            .iter()
            .filter(|branch_info| branch_info.game.id == id)
            .max_by_key(|branch_info| &branch_info.main.tag)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameBranchInfo {
    pub game: Game,
    pub main: PackageInfo,
    pub pre_download: Option<PackageInfo>
}

impl GameBranchInfo {
    pub fn version(&self) -> Option<Version> {
        Version::from_str(&self.main.tag)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub biz: String
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageInfo {
    pub package_id: String,
    pub branch: String,
    pub password: String,
    pub tag: String,
    pub diff_tags: Vec<String>,
    pub categories: Vec<PackageCategory>
}

impl PackageInfo {
    pub fn version(&self) -> Option<Version> {
        Version::from_str(&self.tag)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageCategory {
    pub category_id: String,
    pub matching_field: String
}
