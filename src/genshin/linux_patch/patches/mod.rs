use std::path::Path;

use super::PatchStatus;

mod unity_player_patch;
mod xlua_patch;

pub use unity_player_patch::UnityPlayerPatch;
// TODO: pub use xlua_patch::XluaPatch;

pub trait PatchExt {
    /// Try to parse patch status
    /// 
    /// `patch_folder` should point to standard patch repository folder
    fn from_folder<T: AsRef<Path>>(patch_folder: T) -> anyhow::Result<Self> where Self: Sized;

    /// Get current patch repository folder
    fn folder(&self) -> &Path;

    /// Get latest available patch status
    fn status(&self) -> &PatchStatus;

    /// Check if the patch is applied to the game
    fn is_applied<T: AsRef<Path>>(&self, game_folder: T) -> anyhow::Result<bool>;

    /// Apply available patch
    fn apply<T: AsRef<Path>>(&self, game_folder: T, use_root: bool) -> anyhow::Result<()>;

    /// Revert available patch
    fn revert<T: AsRef<Path>>(&self, game_folder: T, forced: bool) -> anyhow::Result<()>;
}
