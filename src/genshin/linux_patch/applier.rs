use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::io::{Error, ErrorKind, Write};
use std::fs;
use std::env::temp_dir;

use crate::version::ToVersion;

#[derive(Debug, Clone)]
pub struct PatchApplier {
    folder: PathBuf
}

// TODO: rewrite to use git2 library

impl PatchApplier {
    #[inline]
    pub fn new<T: Into<PathBuf>>(folder: T) -> Self {
        Self {
            folder: folder.into()
        }
    }

    /// Verify that the folder contains latest patch
    /// 
    /// To check only specific remote use `is_sync_with`
    #[tracing::instrument(level = "trace", ret)]
    pub fn is_sync<T: IntoIterator<Item = F> + std::fmt::Debug, F: ToString + std::fmt::Debug>(&self, remotes: T) -> Result<bool, Error> {
        tracing::trace!("Checking local patch repository sync state");

        if !self.folder.exists() {
            return Ok(false)
        }

        // FIXME: git ref-parse doesn't check removed files

        let head = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(&self.folder)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()?;

        for remote in remotes {
            Command::new("git")
                .arg("remote")
                .arg("set-url")
                .arg("origin")
                .arg(remote.to_string())
                .current_dir(&self.folder)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            let remote = Command::new("git")
                .arg("rev-parse")
                .arg("origin/HEAD")
                .current_dir(&self.folder)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()?;

            if head.stdout == remote.stdout {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Verify that the folder contains latest patch
    #[tracing::instrument(level = "trace", ret)]
    pub fn is_sync_with<T: ToString + std::fmt::Debug>(&self, remote: T) -> Result<bool, Error> {
        tracing::trace!("Checking local patch repository sync state");

        if !self.folder.exists() {
            return Ok(false)
        }

        // FIXME: git ref-parse doesn't check removed files

        let head = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(&self.folder)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()?;

        Command::new("git")
            .arg("remote")
            .arg("set-url")
            .arg("origin")
            .arg(remote.to_string())
            .current_dir(&self.folder)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        Command::new("git")
            .arg("fetch")
            .arg("origin")
            .current_dir(&self.folder)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        let remote = Command::new("git")
            .arg("rev-parse")
            .arg("origin/HEAD")
            .current_dir(&self.folder)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()?;

        Ok(head.stdout == remote.stdout)
    }

    /// Fetch patch updates from the git repository
    #[tracing::instrument(level = "debug", ret)]
    pub fn sync<T: ToString + std::fmt::Debug>(&self, remote: T) -> Result<bool, Error> {
        tracing::debug!("Syncing local patch repository with remote");

        if self.folder.exists() {
            Command::new("git")
                .arg("remote")
                .arg("set-url")
                .arg("origin")
                .arg(remote.to_string())
                .current_dir(&self.folder)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;
            
            Command::new("git")
                .arg("fetch")
                .arg("origin")
                .current_dir(&self.folder)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            Command::new("git")
                .arg("reset")
                .arg("--hard")
                .arg("origin/master")
                .current_dir(&self.folder)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            Ok(true)
        }

        else {
            let output = Command::new("git")
                .arg("clone")
                .arg(remote.to_string())
                .arg(&self.folder)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            Ok(output.status.success())
        }
    }

    #[inline]
    fn get_temp_path(&self) -> PathBuf {
        temp_dir().join(".patch-applying")
    }

    /// Apply the linux patch to the game
    /// 
    /// This method doesn't verify the state of the locally installed patch.
    /// You should do it manually using `is_sync` method
    /// 
    /// It's recommended to run this method with `use_root = true` to append telemetry entries to the hosts file.
    /// The patch script will be run with `pkexec` and this ask for root password
    #[tracing::instrument(level = "debug", ret)]
    pub fn apply<T, F>(&self, game_path: T, patch_version: F, use_root: bool) -> anyhow::Result<()>
    where
        T: Into<PathBuf> + std::fmt::Debug,
        F: ToVersion + std::fmt::Debug
    {
        tracing::debug!("Applying game patch");

        match patch_version.to_version() {
            Some(version) => {
                let temp_dir = self.get_temp_path();
                let patch_folder = self.folder.join(version.to_plain_string());

                // Verify that the patch folder exists (it can not be synced)
                if !patch_folder.exists() {
                    tracing::error!("Corresponding patch folder doesn't exist: {:?}", patch_folder);

                    return Err(anyhow::anyhow!("Corresponding patch folder doesn't exist: {:?}", patch_folder));
                }

                // Remove temp folder if it is for some reason already exists
                if temp_dir.exists() {
                    fs::remove_dir_all(&temp_dir)?;
                }

                // Create temp folder
                fs::create_dir_all(&temp_dir)?;

                // Copy patch files there
                let mut options = fs_extra::dir::CopyOptions::default();

                options.content_only = true; // Don't copy e.g. "270" folder, just its content

                if let Err(err) = fs_extra::dir::copy(patch_folder, &temp_dir, &options) {
                    tracing::error!("Failed to copy patch to the temp folder: {err}");

                    return Err(anyhow::anyhow!("Failed to copy patch to the temp folder: {err}"));
                }

                // Remove exit and read commands from the beginning of the patch.sh file
                // These lines are used for test patch restrictions so we don't need them
                let patch_file = temp_dir.join("patch.sh");

                let mut patch_script = fs::read_to_string(&patch_file)?;

                patch_script = format!("{}{}", {
                    patch_script[..650]
                        .replace("exit", "#exit")
                        .replace("read", "#read")
                }, &patch_script[650..]);

                // Update patch.sh file
                fs::write(&patch_file, patch_script)?;

                // Execute patch.sh from the game folder
                let output = if use_root {
                    // pkexec bash -c "cd '<game path>' ; bash '<patch path>/patch.sh'"
                    // We have to use this command as pkexec ignores current working directory
                    Command::new("pkexec")
                        .arg("bash")
                        .arg("-c")
                        .arg(format!("cd '{}' ; bash '{}'", game_path.into().to_string_lossy(), patch_file.to_string_lossy()))
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()?
                } else {
                    Command::new("bash")
                        .arg(patch_file)
                        .current_dir(game_path.into())
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()?
                };

                // Input "y" as it's asked in the patch script
                // I could remove it, but who actually cares?
                output.stdin.as_ref().unwrap().write(b"y")?;

                let output = output.wait_with_output()?;

                // Remove temp patch folder
                fs::remove_dir_all(temp_dir)?;

                // Return patching status
                let output = String::from_utf8_lossy(&output.stdout);

                if output.contains("Patch applied!") {
                    Ok(())
                }

                else {
                    tracing::error!("Failed to apply patch: {output}");

                    Err(Error::new(ErrorKind::Other, output).into())
                }
            },
            None => {
                tracing::error!("Failed to get patch version");

                Err(anyhow::anyhow!("Failed to get patch version"))
            }
        }
    }

    /// Revert patch
    /// 
    /// This method doesn't verify the state of the locally installed patch.
    /// You should do it manually using `is_sync` method
    #[tracing::instrument(level = "debug", ret)]
    pub fn revert<T, F>(&self, game_path: T, patch_version: F, force: bool) -> anyhow::Result<bool>
    where
        T: Into<PathBuf> + std::fmt::Debug,
        F: ToVersion + std::fmt::Debug
    {
        tracing::debug!("Reverting game patch");

        match patch_version.to_version() {
            Some(version) => {
                let temp_dir = self.get_temp_path();
                let patch_folder = self.folder.join(version.to_plain_string());

                // Verify that the patch folder exists (it can not be synced)
                if !patch_folder.exists() {
                    tracing::error!("Corresponding patch folder doesn't exist: {:?}", patch_folder);

                    return Err(anyhow::anyhow!("Corresponding patch folder doesn't exist: {:?}", patch_folder));
                }

                // Create temp folder
                fs::create_dir(&temp_dir)?;

                // Copy patch files there
                let mut options = fs_extra::dir::CopyOptions::default();

                options.content_only = true; // Don't copy e.g. "270" folder, just its content

                if let Err(err) = fs_extra::dir::copy(patch_folder, &temp_dir, &options) {
                    tracing::error!("Failed to copy patch to the temp folder: {err}");

                    return Err(anyhow::anyhow!("Failed to copy patch to the temp folder: {err}"));
                }

                let revert_file = temp_dir.join("patch_revert.sh");

                // Remove files timestamps checks if it's needed
                if force {
                    // Update patch_revert.sh file
                    fs::write(
                        &revert_file,
                        fs::read_to_string(&revert_file)?
                            .replace("difftime=$", "difftime=0 #difftime=$")
                    )?;
                }

                // Execute patch_revert.sh from the game folder
                let output = Command::new("bash")
                    .arg(revert_file)
                    .current_dir(game_path.into())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output()?;

                // Remove temp patch folder
                fs::remove_dir_all(temp_dir)?;

                // Return patching status
                Ok(!String::from_utf8_lossy(&output.stdout).contains("ERROR: "))
            },
            None => {
                tracing::error!("Failed to get patch version");

                Err(anyhow::anyhow!("Failed to get patch version"))
            }
        }
    }
}
