use std::process::{Command, Stdio};
use std::path::Path;

// TODO: rewrite to use git2 library

pub trait RemoteGitSyncExt {
    /// Path to folder with local git repository
    fn folder(&self) -> &Path;

    /// Verify that the folder is synced
    /// 
    /// Returns given remote with which current folder is synced,
    /// and `Ok(None)` if it's not synced
    /// 
    /// To check only specific remote use `is_sync_with`
    fn is_sync<T, F>(&self, remotes: T) -> anyhow::Result<Option<String>>
    where
        T: IntoIterator<Item = F>,
        F: AsRef<str>
    {
        tracing::trace!("Checking local repository sync state: {:?}", self.folder());

        if !self.folder().exists() {
            tracing::warn!("Given local repository folder doesn't exist");

            return Ok(None);
        }

        for remote in remotes {
            if self.is_sync_with(remote.as_ref())? {
                return Ok(Some(remote.as_ref().to_string()));
            }
        }

        Ok(None)
    }

    /// Verify that the folder is synced
    fn is_sync_with(&self, remote: impl AsRef<str>) -> anyhow::Result<bool> {
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
    fn sync(&self, remote: impl AsRef<str>) -> anyhow::Result<Vec<String>> {
        tracing::debug!("Syncing local patch repository with remote");

        if self.folder().exists() {
            // git rev-parse HEAD

            let head_commit = String::from_utf8(Command::new("git")
                .arg("rev-parse")
                .arg("HEAD")
                .current_dir(self.folder())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()?
                .stdout)?.trim_end().to_string();

            // git remote set-url origin <remote>

            Command::new("git")
                .arg("remote")
                .arg("set-url")
                .arg("origin")
                .arg(remote.as_ref())
                .current_dir(self.folder())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            // git fetch origin

            Command::new("git")
                .arg("fetch")
                .arg("origin")
                .current_dir(self.folder())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            // git reset --hard origin/HEAD

            Command::new("git")
                .arg("reset")
                .arg("--hard")
                .arg("origin/HEAD")
                .current_dir(self.folder())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            // git --no-pager log --oneline <head_commit (old)>..HEAD

            let changes = String::from_utf8(Command::new("git")
                .arg("--no-pager")
                .arg("log")
                .arg("--oneline")
                .arg(format!("{head_commit}..HEAD"))
                .current_dir(self.folder())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()?
                .stdout)?;

            Ok(changes.trim_end().lines().map(|line| line[8..].to_string()).collect())
        }

        else {
            // git clone <remote> <folder>

            Command::new("git")
                .arg("clone")
                .arg(remote.as_ref())
                .arg(self.folder())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()?;

            // TODO: maybe it's too long?
            // git --no-pager log --oneline

            let changes = String::from_utf8(Command::new("git")
                .arg("--no-pager")
                .arg("log")
                .arg("--oneline")
                .current_dir(self.folder())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()?
                .stdout)?;

            Ok(changes.trim_end().lines().map(|line| line[8..].to_string()).collect())
        }
    }
}
