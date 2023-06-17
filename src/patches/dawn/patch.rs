use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};

use crate::version::*;
use crate::genshin::consts::GameEdition;
use crate::traits::git_sync::RemoteGitSyncExt;

use super::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Regions {
    /// `UnityPlayer.dll` md5 hash
    Global(String),

    /// `UnityPlayer.dll` md5 hash
    China(String),

    /// `UnityPlayer.dll` md5 hashes for different regions
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

// Not really needed because is used only once, but I'll keep
// it here for perhaps some future use, like returning of
// xlua patch or something

pub trait PatchExt {
    /// Try to parse patch status
    /// 
    /// `patch_folder` should point to standard patch repository folder
    fn from_folder(patch_folder: impl AsRef<Path>, game_edition: GameEdition) -> anyhow::Result<Self> where Self: Sized;

    /// Get current patch repository folder
    fn folder(&self) -> &Path;

    /// Get latest available patch status
    fn status(&self) -> &PatchStatus;

    /// Check if the patch is applied to the game
    fn is_applied(&self, game_folder: impl AsRef<Path>) -> anyhow::Result<bool>;

    /// Apply available patch
    fn apply(&self, game_folder: impl AsRef<Path>, use_root: bool) -> anyhow::Result<()>;

    /// Revert available patch
    fn revert(&self, game_folder: impl AsRef<Path>, forced: bool) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    folder: PathBuf,
    edition: GameEdition
}

impl RemoteGitSyncExt for Patch {
    #[inline]
    fn folder(&self) -> &Path {
        self.folder.as_path()
    }
}

impl Patch {
    #[inline]
    pub fn new<T: Into<PathBuf>>(folder: T, game_edition: GameEdition) -> Self {
        Self {
            folder: folder.into(),
            edition: game_edition
        }
    }

    #[inline]
    pub fn player_patch(&self) -> anyhow::Result<PlayerPatch> {
        PlayerPatch::from_folder(&self.folder, self.edition)
    }
}
