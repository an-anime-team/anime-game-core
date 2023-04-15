use serde::{Serialize, Deserialize};

use crate::version::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchStatus {
    /// The patch is outdated and nothing was made to update it
    Outdated {
        current: Version,
        latest: Version
    },

    /// Patch is available for the latest version of the game, but only in testing mode
    Testing {
        version: Version,
        bh3base_hash: String,
        player_hash: String
    },

    /// Patch is fully available and tested for the latest version of the game
    Available {
        version: Version,
        bh3base_hash: String,
        player_hash: String
    }
}
