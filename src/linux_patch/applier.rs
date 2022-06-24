use std::path::Path;
use std::process::{Command, Stdio};
use std::io::Error;

// use git2::{Repository, ResetType, Error};

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

    pub fn sync<T: ToString>(&self, remote: T) -> Result<(), Error> {
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
            },
            false => {
                Command::new("git")
                    .arg("clone")
                    .arg(remote.to_string())
                    .arg(&self.folder)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .output()?;
            }
        }

        Ok(())
    }
}
