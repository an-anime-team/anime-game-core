use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};

use super::patches::*;

use crate::version::*;
use crate::traits::git_sync::RemoteGitSync;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Regions {
    /// `UnityPlayer.dll` / `Plugins/xlua.dll` md5 hash
    Global(String),
    
    /// `UnityPlayer.dll` / `Plugins/xlua.dll` md5 hash
    China(String),

    /// `UnityPlayer.dll` / `Plugins/xlua.dll` md5 hashes for different regions
    Both {
        global: String,
        china: String
    }
}

impl Regions {
    /// Compares `player_hash` with inner values
    /// 
    /// If `player_hash` not equal to the inner value, then the patch was applied
    #[inline]
    pub fn is_applied<T: AsRef<str>>(&self, player_hash: T) -> bool {
        let player_hash = player_hash.as_ref();

        match self {
            Self::Global(hash) |
            Self::China(hash) => hash != player_hash,
            Self::Both { global, china } => global != player_hash && china != player_hash
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchStatus {
    /// Patch is not available
    NotAvailable,

    /// The patch is outdated and nothing was made to update it
    Outdated {
        current: Version,
        latest: Version
    },

    /// Some preparations for the new version of the game were made, but the patch is not available
    /// 
    /// Technically the same as `Outdated`
    Preparation {
        version: Version
    },

    /// Patch is available for the latest version of the game, but only in testing mode
    Testing {
        version: Version,
        player_hash: Regions
    },

    /// Patch is fully available and tested for the latest version of the game
    Available {
        version: Version,
        player_hash: Regions
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    folder: PathBuf
}

impl RemoteGitSync for Patch {
    #[inline]
    fn folder(&self) -> &Path {
        self.folder.as_path()
    }
}

impl Patch {
    #[inline]
    pub fn new<T: Into<PathBuf>>(folder: T) -> Self {
        Self {
            folder: folder.into()
        }
    }

    #[inline]
    pub fn unity_player_patch(&self) -> anyhow::Result<UnityPlayerPatch> {
        UnityPlayerPatch::from_folder(&self.folder)
    }

    #[inline]
    pub fn xlua_patch(&self) -> anyhow::Result<XluaPatch> {
        XluaPatch::from_folder(&self.folder)
    }
}
