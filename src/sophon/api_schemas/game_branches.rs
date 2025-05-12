use serde::{Deserialize, Serialize};

use crate::version::Version;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameBranches {
    pub game_branches: Vec<GameBranchInfo>
}

impl GameBranches {
    /// Get the latest version for the requested id
    pub fn latest_version_by_id(&self, id: &str) -> Option<Version> {
        self.game_branches.iter()
            .filter(|gbi| gbi.game.id == id)
            .max_by_key(|gbi| &gbi.main.tag)
            .map(|gbi| Version::from_str(&gbi.main.tag).unwrap())
    }

    /// Get `GameBranchInfo` of a specified id and game version
    pub fn get_game_by_id(&self, id: &str, ver: Version) -> Option<&GameBranchInfo> {
        let ver_str = ver.to_string();

        self.game_branches.iter()
            .find(|gbi| gbi.game.id == id && gbi.main.tag == ver_str)
    }

    /// Get latest version of specified game by id
    pub fn get_game_latest_by_id(&self, id: &str) -> Option<&GameBranchInfo> {
        self.game_branches.iter()
            .filter(|gbi| gbi.game.id == id)
            .max_by_key(|gbi| &gbi.main.tag)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameBranchInfo {
    pub game: Game,
    pub main: PackageInfo,
    pub pre_download: Option<PackageInfo>
}

impl GameBranchInfo {
    pub fn version(&self) -> Version {
        Version::from_str(&self.main.tag).unwrap()
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
    pub fn version(&self) -> Version {
        Version::from_str(&self.tag).unwrap()
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageCategory {
    pub category_id: String,
    pub matching_field: String
}
