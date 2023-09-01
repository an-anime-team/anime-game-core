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

use crate::updater::*;

use crate::network::api::ApiExt;
use crate::network::downloader::DownloaderExt;
use crate::network::downloader::basic::{
    Downloader,
    Error as DownloaderError
};

use super::Game;
use super::Api;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to fetch data: {0}")]
    Minreq(#[from] minreq::Error),

    #[error("Failed to fetch data: {0}")]
    MinreqRef(#[from] &'static minreq::Error),

    #[error("Failed to send verifier message through the flume channel: {0}")]
    FlumeVerifierSendError(#[from] flume::SendError<((), u64, u64)>),

    #[error("Failed to send repairer message through the flume channel: {0}")]
    FlumeRepairerSendError(#[from] flume::SendError<(RepairerStatus, u64, u64)>),

    #[error("Failed to parse version: {0}")]
    VersionParseError(#[from] VersionError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to start downloader: {0}")]
    DownloaderError(#[from] DownloaderError)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairerStatus {
    PreparingTransition,
    RepairingFiles,
    FinishingTransition
}

pub type VerifyUpdater = BasicUpdater<(), Vec<PathBuf>, Error>;
pub type RepairUpdater = BasicUpdater<RepairerStatus, (), Error>;

impl VerifyIntegrityExt for Game {
    type Error = Error;
    type Updater = VerifyUpdater;

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
    type Updater = RepairUpdater;

    fn repair_files(&self, files: impl AsRef<[PathBuf]>) -> Result<Self::Updater, Self::Error> {
        let api = match Api::fetch(self.edition) {
            Ok(api) => api.data.game.latest.clone(),

            Err(err) => return Err(Error::MinreqRef(err))
        };

        // Ok(BasicRepairerUpdater::new(self.get_driver(), files.as_ref().to_vec(), api.decompressed_path))

        let driver = self.get_driver();
        let files = files.as_ref().to_vec();
        let base_download_uri = api.decompressed_path;

        Ok(BasicUpdater::spawn(|updater| {
            Box::new(move || -> Result<(), Error> {
                // I don't need to send this message but do it just for consistency
                updater.send((RepairerStatus::PreparingTransition, 0, 1))?;

                // TODO: list original files hashes or something to make repair transitions unique
                let transition_folder = driver.create_transition("action:repair")?;

                let total = files.len() as u64;

                updater.send((RepairerStatus::RepairingFiles, 0, total))?;

                for file in files {
                    // TODO: use updater to show downloading progress better
                    Downloader::new(format!("{base_download_uri}/{}", file.to_string_lossy()))
                        .download(transition_folder.join(&file))?
                        .wait()?;

                    updater.send((RepairerStatus::RepairingFiles, 0, total))?;
                }

                updater.send((RepairerStatus::FinishingTransition, 0, 1))?;

                driver.finish_transition("action:repair")?;

                Ok(())
            })
        }))
    }
}
