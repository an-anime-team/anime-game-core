use std::process::{Command, Stdio};
use std::path::Path;

// TODO: rewrite to use git2 library

pub trait RemoteGitSync: std::fmt::Debug {
    /// Path to folder with local git repository
    fn folder(&self) -> &Path;

    /// Verify that the folder is synced
    /// 
    /// Returns given remote with which current folder is synced,
    /// and `Ok(None)` if it's not synced
    /// 
    /// To check only specific remote use `is_sync_with`
    #[tracing::instrument(level = "debug", ret)]
    fn is_sync<T, F>(&self, remotes: T) -> anyhow::Result<Option<String>>
    where
        T: IntoIterator<Item = F> + std::fmt::Debug,
        F: AsRef<str> + std::fmt::Debug
    {
        tracing::trace!("Checking local repository sync state: {:?}", self.folder());

        if !self.folder().exists() {
            tracing::warn!("Given local repository folder doesn't exist");

            anyhow::bail!("Given local repository folder doesn't exist");
        }

        for remote in remotes {
            if self.is_sync_with(remote.as_ref())? {
                return Ok(Some(remote.as_ref().to_string()));
            }
        }

        Ok(None)
    }

    /// Verify that the folder is synced
    #[tracing::instrument(level = "debug", ret)]
    fn is_sync_with<T: AsRef<str> + std::fmt::Debug>(&self, remote: T) -> std::io::Result<bool> {
        tracing::trace!("Checking local repository sync state. Folder: {:?}. Remote: {}", self.folder(), remote.as_ref());

        if !self.folder().exists() {
            tracing::warn!("Given local repository folder doesn't exist");

            return Ok(false);
        }

        // FIXME: git ref-parse doesn't check removed files

        let head = Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(self.folder())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()?;

        Command::new("git")
            .arg("remote")
            .arg("set-url")
            .arg("origin")
            .arg(remote.as_ref())
            .current_dir(self.folder())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        Command::new("git")
            .arg("fetch")
            .arg("origin")
            .current_dir(self.folder())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        let remote = Command::new("git")
            .arg("rev-parse")
            .arg("origin/HEAD")
            .current_dir(self.folder())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()?;

        Ok(head.stdout == remote.stdout)
    }

    /// Fetch patch updates from the git repository
    #[tracing::instrument(level = "debug", ret)]
    fn sync<T: AsRef<str> + std::fmt::Debug>(&self, remote: T) -> std::io::Result<bool> {
        tracing::debug!("Syncing local patch repository with remote");

        if self.folder().exists() {
            Command::new("git")
                .arg("remote")
                .arg("set-url")
                .arg("origin")
                .arg(remote.as_ref())
                .current_dir(self.folder())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            Command::new("git")
                .arg("fetch")
                .arg("origin")
                .current_dir(self.folder())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            Command::new("git")
                .arg("reset")
                .arg("--hard")
                .arg("origin/HEAD")
                .current_dir(self.folder())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            Ok(true)
        }

        else {
            let output = Command::new("git")
                .arg("clone")
                .arg(remote.as_ref())
                .arg(self.folder())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            Ok(output.status.success())
        }
    }
}
