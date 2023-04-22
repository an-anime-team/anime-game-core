use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::env::temp_dir;

use serde::{Serialize, Deserialize};
use md5::{Md5, Digest};

use super::PatchStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainPatch {
    pub folder: PathBuf,
    pub status: PatchStatus
}

impl MainPatch {
    /// Try to parse patch status
    /// 
    /// `patch_folder` should point to standard patch repository folder
    pub fn from_folder<T: AsRef<Path>>(patch_folder: T) -> anyhow::Result<Self> where Self: Sized {
        Ok(Self {
            folder: patch_folder.as_ref().to_path_buf(),
            status: PatchStatus::NotAvailable
        })

        /*let patch_folder = patch_folder.as_ref().to_path_buf();

        // Immediately throw error if patch folder doesn't even exist
        // but it actually shouldn't be possible because we get this struct
        // from `Patch` struct which implements `GitRemoteSync` where it's verified
        // but anyway
        if !patch_folder.exists() {
            anyhow::bail!("Given patch folder doesn't exist: {:?}", patch_folder);
        }

        // Get patch metadata
        let metadata: PatchMetadata = serde_json::from_str(&std::fs::read_to_string(patch_folder.join("version.json"))?)?;
        let patch_version = Version::from_str(metadata.version).unwrap();

        // Get latest available game version
        let latest_version = Version::from_str(api::request()?.data.game.latest.version).unwrap();

        // Return PatchStatus::Outdated if the patch is, well, outdated
        if patch_version < latest_version {
            return Ok(Self {
                folder: patch_folder,
                status: PatchStatus::Outdated {
                    current: patch_version,
                    latest: latest_version
                }
            });
        }

        // TODO: region selection

        match metadata.hashes.global {
            Some(hashes) => {
                if metadata.testing {
                    Ok(Self {
                        folder: patch_folder,
                        status: PatchStatus::Testing {
                            version: patch_version,
                            bh3base_hash: hashes.bh3base,
                            player_hash: hashes.player
                        }
                    })
                }
        
                else {
                    Ok(Self {
                        folder: patch_folder,
                        status: PatchStatus::Available {
                            version: patch_version,
                            bh3base_hash: hashes.bh3base,
                            player_hash: hashes.player
                        }
                    })
                }
            }

            None => Ok(Self {
                folder: patch_folder,
                status: PatchStatus::NotAvailable
            })
        }*/
    }

    #[inline]
    /// Get current patch repository folder
    pub fn folder(&self) -> &Path {
        self.folder.as_path()
    }

    #[inline]
    /// Get latest available patch status
    pub fn status(&self) -> &PatchStatus {
        &self.status
    }

    /// Check if the patch is applied to the game
    pub fn is_applied<T: AsRef<Path>>(&self, game_folder: T) -> anyhow::Result<bool> {
        match &self.status {
            PatchStatus::NotAvailable |
            PatchStatus::Outdated { .. } => Ok(false),

            PatchStatus::Testing { srbase_hash, player_hash, .. } |
            PatchStatus::Available { srbase_hash, player_hash, .. } => {
                let status =
                    &format!("{:x}", Md5::digest(std::fs::read(game_folder.as_ref().join("StarRailBase.dll"))?)) != srbase_hash &&
                    &format!("{:x}", Md5::digest(std::fs::read(game_folder.as_ref().join("UnityPlayer.dll"))?)) != player_hash;

                Ok(status)
            }
        }
    }

    /// Apply available patch
    pub fn apply<T: AsRef<Path>>(&self, game_folder: T, use_root: bool) -> anyhow::Result<()> {
        tracing::debug!("Applying game patch");

        match &self.status {
            PatchStatus::NotAvailable => anyhow::bail!("Patch for selected region is not available"),
            PatchStatus::Outdated { .. } => anyhow::bail!("Patch is outdated and can't be applied"),

            PatchStatus::Testing { .. } |
            PatchStatus::Available { .. } => {
                let temp_dir = temp_dir().join(".patch-applying");

                // Verify that the patch folder exists
                // Kinda irrealistic situation but still
                if !self.folder.exists() {
                    tracing::error!("Patch folder doesn't exist: {:?}", self.folder);

                    anyhow::bail!("Patch folder doesn't exist: {:?}", self.folder);
                }

                // Remove temp folder if it is for some reason already exists
                if temp_dir.exists() {
                    std::fs::remove_dir_all(&temp_dir)?;
                }

                // Create temp folder
                std::fs::create_dir_all(&temp_dir)?;

                // Copy patch files there
                let mut options = fs_extra::dir::CopyOptions::default();

                options.content_only = true; // Don't copy e.g. "270" folder, just its content

                if let Err(err) = fs_extra::dir::copy(&self.folder, &temp_dir, &options) {
                    tracing::error!("Failed to copy patch to the temp folder: {err}");

                    anyhow::bail!("Failed to copy patch to the temp folder: {err}");
                }

                let mut command = Command::new("bash");

                command
                    .arg(temp_dir.join("install.sh"))
                    .arg("--yes-to-all")
                    .current_dir(game_folder)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                if !use_root {
                    command.arg("--no-root");
                }

                let output = command.output()?;

                // Remove temp patch folder
                std::fs::remove_dir_all(temp_dir)?;

                // Return patching status
                let stdout = String::from_utf8_lossy(&output.stdout);

                if stdout.contains("Done") {
                    Ok(())
                }

                else {
                    let stderr = String::from_utf8_lossy(&output.stderr);

                    tracing::error!("Failed to apply patch: {stderr}");

                    anyhow::bail!("Failed to apply patch: {stderr}");
                }
            }
        }
    }

    /// Revert available patch
    pub fn revert<T: AsRef<Path>>(&self, game_folder: T) -> anyhow::Result<()> {
        tracing::debug!("Reverting game patch");

        match &self.status {
            PatchStatus::NotAvailable => anyhow::bail!("Patch for selected region is not available"),
            PatchStatus::Outdated { .. } => anyhow::bail!("Patch can't be reverted because it's outdated"),

            PatchStatus::Testing { .. } |
            PatchStatus::Available { .. } => {
                let temp_dir = temp_dir().join(".patch-applying");

                // Verify that the patch folder exists
                // Kinda irrealistic situation but still
                if !self.folder.exists() {
                    tracing::error!("Patch folder doesn't exist: {:?}", self.folder);

                    anyhow::bail!("Patch folder doesn't exist: {:?}", self.folder);
                }

                // Remove temp folder if it is for some reason already exists
                if temp_dir.exists() {
                    std::fs::remove_dir_all(&temp_dir)?;
                }

                // Create temp folder
                std::fs::create_dir_all(&temp_dir)?;

                // Copy patch files there
                let mut options = fs_extra::dir::CopyOptions::default();

                options.content_only = true; // Don't copy e.g. "270" folder, just its content

                if let Err(err) = fs_extra::dir::copy(&self.folder, &temp_dir, &options) {
                    tracing::error!("Failed to copy patch to the temp folder: {err}");

                    anyhow::bail!("Failed to copy patch to the temp folder: {err}");
                }

                // Execute uninstall.sh from the game folder
                let output = Command::new("bash")
                    .arg(temp_dir.join("uninstall.sh"))
                    .current_dir(game_folder)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()?;

                // Remove temp patch folder
                std::fs::remove_dir_all(temp_dir)?;

                // Return reverting status
                let stdout = String::from_utf8_lossy(&output.stdout);

                if stdout.contains("Done") {
                    Ok(())
                }

                else {
                    let stderr = String::from_utf8_lossy(&output.stderr);

                    tracing::error!("Failed to revert patch: {stderr}");

                    anyhow::bail!("Failed to revert patch: {stderr}");
                }
            }
        }
    }
}
