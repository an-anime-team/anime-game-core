use std::path::Path;
use std::process::{Command, Stdio};
use std::io::{Error, ErrorKind, Write};
use std::fs;
use std::env::temp_dir;

use uuid::Uuid;
// use git2::{Repository, ResetType, Error};

use crate::version::ToVersion;

pub struct PatchApplier {
    folder: String
}

// TODO: rewrite to use git2 library

impl PatchApplier {
    pub fn new<T: ToString>(folder: T) -> Self {
        /*Ok(Self {
            repository: match Path::new(&folder.to_string()).exists() {
                true => Repository::open(folder.to_string())?,
                false => Repository::init(folder.to_string())?
            }
        })*/

        Self {
            folder: folder.to_string()
        }
    }

    /// Verify that the folder contains latest patch
    /// 
    /// To check only specific remote use `is_sync_with`
    pub fn is_sync<T: IntoIterator<Item = F>, F: ToString>(&self, remotes: T) -> Result<bool, Error> {
        if !Path::new(&self.folder).exists() {
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
    pub fn is_sync_with<T: ToString>(&self, remote: T) -> Result<bool, Error> {
        if !Path::new(&self.folder).exists() {
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
    pub fn sync<T: ToString>(&self, remote: T) -> Result<bool, Error> {
        /*self.repository.remote_set_url("origin", &remote.to_string())?;

        let mut remote = self.repository.find_remote("origin")?;

        remote.fetch(&["master"], None, None)?;

        self.repository.reset(&["master"], ResetType::Hard, None);

        Ok(())*/

        // FIXME: errors handling
        match Path::new(&self.folder).exists() {
            true => {
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
                    .arg("--all")
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
            },
            false => {
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
    }

    fn get_temp_path(&self) -> String {
        let temp_file = temp_dir().to_str().unwrap().to_string();

        format!("{}/.{}-patch-applying", temp_file, Uuid::new_v4().to_string())
    }

    /// Apply the linux patch to the game
    /// 
    /// This method doesn't verify the state of the locally installed patch.
    /// You should do it manually using `is_sync` method
    /// 
    /// It's recommended to run this method with `use_root = true` to append telemetry entries to the hosts file.
    /// The patch script will be run with `pkexec` and this ask for root password
    pub fn apply<T: ToString, F: ToVersion>(&self, game_path: T, patch_version: F, use_root: bool) -> Result<(), Error> {
        match patch_version.to_version() {
            Some(version) => {
                let temp_dir = self.get_temp_path();
                let patch_folder = format!("{}/{}", self.folder, version.to_plain_string());

                // Verify that the patch folder exists (it can not be synced)
                if !Path::new(&patch_folder).exists() {
                    return Err(Error::new(ErrorKind::Other, format!("Corresponding patch folder doesn't exist: {}", patch_folder)));
                }

                // Create temp folder
                fs::create_dir(&temp_dir)?;

                // Copy patch files there
                let mut options = fs_extra::dir::CopyOptions::default();

                options.content_only = true; // Don't copy e.g. "270" folder, just its content

                if let Err(err) = fs_extra::dir::copy(patch_folder, &temp_dir, &options) {
                    return Err(Error::new(ErrorKind::Other, format!("Failed to copy patch to the temp folder: {}", err)));
                }

                // Remove exit and read commands from the beginning of the patch.sh file
                // These lines are used for test patch restrictions so we don't need them
                let patch_file = format!("{}/patch.sh", temp_dir);

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
                        .arg(format!("cd '{}' ; bash '{}'", game_path.to_string(), patch_file))
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()?
                } else {
                    Command::new("bash")
                        .arg(patch_file)
                        .current_dir(game_path.to_string())
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
                if String::from_utf8_lossy(&output.stdout).contains("Patch applied!") {
                    Ok(())
                }

                else {
                    Err(Error::new(ErrorKind::Other, String::from_utf8_lossy(&output.stderr)))
                }
            },
            None => Err(Error::new(ErrorKind::Other, "Failed to get patch version"))
        }
    }

    /// Revert patch
    /// 
    /// This method doesn't verify the state of the locally installed patch.
    /// You should do it manually using `is_sync` method
    pub fn revert<T: ToString, F: ToVersion>(&self, game_path: T, patch_version: F, force: bool) -> Result<bool, Error> {
        match patch_version.to_version() {
            Some(version) => {
                let temp_dir = self.get_temp_path();
                let patch_folder = format!("{}/{}", self.folder, version.to_plain_string());

                // Verify that the patch folder exists (it can not be synced)
                if !Path::new(&patch_folder).exists() {
                    return Err(Error::new(ErrorKind::Other, format!("Corresponding patch folder doesn't exist: {}", patch_folder)));
                }

                // Create temp folder
                fs::create_dir(&temp_dir)?;

                // Copy patch files there
                let mut options = fs_extra::dir::CopyOptions::default();

                options.content_only = true; // Don't copy e.g. "270" folder, just its content

                if let Err(err) = fs_extra::dir::copy(patch_folder, &temp_dir, &options) {
                    return Err(Error::new(ErrorKind::Other, format!("Failed to copy patch to the temp folder: {}", err)));
                }

                let revert_file = format!("{}/patch_revert.sh", temp_dir);

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
                    .current_dir(game_path.to_string())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output()?;

                // Remove temp patch folder
                fs::remove_dir_all(temp_dir)?;

                // Return patching status
                Ok(!String::from_utf8_lossy(&output.stdout).contains("ERROR: "))
            },
            None => Err(Error::new(ErrorKind::Other, "Failed to get patch version"))
        }
    }
}
