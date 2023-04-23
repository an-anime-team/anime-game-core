use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};

use crate::version::*;
use crate::traits::git_sync::RemoteGitSyncExt;

use crate::star_rail::consts::GameEdition;

use super::MainPatch;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchStatus {
    /// Patch is not available for selected region
    NotAvailable,

    /// The patch is outdated and nothing was made to update it
    Outdated {
        current: Version,
        latest: Version
    },

    /// Patch is available for the latest version of the game, but only in testing mode
    Testing {
        version: Version,
        srbase_hash: String,
        player_hash: String
    },

    /// Patch is fully available and tested for the latest version of the game
    Available {
        version: Version,
        srbase_hash: String,
        player_hash: String
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    folder: PathBuf
}

impl RemoteGitSyncExt for Patch {
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
    pub fn main_patch(&self, region: GameEdition) -> anyhow::Result<MainPatch> {
        MainPatch::from_folder(&self.folder, region)
    }
}
