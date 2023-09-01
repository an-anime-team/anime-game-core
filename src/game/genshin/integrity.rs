use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Serialize, Deserialize};

use md5::{Md5, Digest};

use crate::game::GameExt;
use crate::game::integrity::*;

use crate::game::version::{
    // Version,
    Error as VersionError
};

use crate::network::api::ApiExt;
use crate::updater::*;

use super::Game;
use super::Api;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to fetch data: {0}")]
    Minreq(#[from] minreq::Error),

    #[error("Failed to fetch data: {0}")]
    MinreqRef(#[from] &'static minreq::Error),

    #[error("Failed to send message through the flume channel: {0}")]
    FlumeSendError(#[from] flume::SendError<((), u64, u64)>),

    #[error("Failed to parse version: {0}")]
    VersionParseError(#[from] VersionError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error)
}

impl VerifyIntegrityExt for Game {
    type Error = Error;
    type Updater = BasicUpdater<(), Vec<PathBuf>, Error>;

    fn verify_files(&self) -> Result<Self::Updater, Self::Error> {
        let api = match Api::fetch(self.edition) {
            Ok(api) => api.data.game.latest.clone(),

            Err(err) => return Err(Error::MinreqRef(err))
        };

        let decompressed_path = api.decompressed_path;
        // let version = api.version.parse::<Version>()?;

        // Should I use transitions for files verification?
        // let game_folder = self.driver.create_transition(&format!("action:verify_files-component:game_{}-version:v{version}", self.edition.to_str()))?;

        #[derive(Serialize, Deserialize)]
        #[allow(non_snake_case)]
        struct PkgVersionFile {
            pub remoteName: String,
            pub md5: String,
            pub fileSize: u64
        }

        let files = minreq::get(format!("{decompressed_path}/pkg_version"))
            .send()?
            .as_str()?
            .lines()
            .flat_map(serde_json::from_str::<PkgVersionFile>)
            .map(|file| (
                PathBuf::from(file.remoteName),
                (file.fileSize, file.md5.to_ascii_lowercase())
            ))
            .collect::<HashMap<_, _>>();

        let driver = self.get_driver();

        Ok(BasicUpdater::spawn(|updater| {
            Box::new(move || {
                let mut broken = Vec::new();
                let total = files.len() as u64;

                for (i, (file, (file_size, file_hash))) in files.into_iter().enumerate() {
                    let verified = driver.exists(file.as_os_str()) &&
                        driver.metadata(file.as_os_str())?.len() == file_size &&
                        format!("{:x}", Md5::digest(driver.read(file.as_os_str())?)).to_ascii_lowercase() == file_hash;

                    if !verified {
                        broken.push(file);
                    }

                    updater.send(((), i as u64 + 1, total))?;
                }

                Ok(broken)
            })
        }))
    }
}

impl RepairFilesExt for Game {
    type Error = Error;
    type Updater = BasicRepairerUpdater;

    fn repair_files(&self, files: impl AsRef<[PathBuf]>) -> Result<Self::Updater, Self::Error> {
        let api = match Api::fetch(self.edition) {
            Ok(api) => api.data.game.latest.clone(),

            Err(err) => return Err(Error::MinreqRef(err))
        };

        Ok(BasicRepairerUpdater::new(self.get_driver(), files.as_ref().to_vec(), api.decompressed_path))
    }
}
