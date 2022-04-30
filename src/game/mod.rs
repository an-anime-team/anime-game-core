use crate::consts;
use crate::json_schemas;
use crate::installer::prelude::*;

use std::io::ErrorKind;
use std::time::Duration;
use std::io::Error;

mod game_version;
mod voice_packages;

pub use game_version::*;
pub use voice_packages::*;

#[derive(Debug, Clone)]
pub struct Game {
    /// Path to the game's folder
    path: String,
    remote: Option<json_schemas::versions::Response>
}

impl Game {
    pub fn new(path: String) -> Game {
        Game {
            path,
            remote: None
        }
    }

    /// Get information from the game's API
    fn get_remote(&mut self) -> Result<&json_schemas::versions::Response, minreq::Error> {
        if self.remote == None {
            match minreq::get(consts::VERSIONS_URL).send() {
                Ok(response) => {
                    match response.json::<json_schemas::versions::Response>() {
                        Ok(json_response) => {
                            self.remote = Some(json_response);
                        },
                        Err(err) => return Err(err)
                    }
                },
                Err(err) => return Err(err)
            }
        }

        Ok(self.remote.as_ref().unwrap())
    }

    pub fn version(&mut self) -> GameVersion {
        let remote = if let Ok(remote) = self.get_remote() { Some(remote.clone()) } else { None };

        GameVersion::new(self.path.clone(), remote)
    }

    pub fn voice_packages(&mut self) -> VoicePackages {
        let remote = if let Ok(remote) = self.get_remote() { Some(remote.clone()) } else { None };

        VoicePackages::new(self.path.clone(), remote)
    }

    pub fn download(&mut self, path: &str, params: InstallerParams) -> Result<Duration, Error> {
        match self.get_remote() {
            Ok(remote) => {
                let path = path.to_string();
                let uri = &remote.data.game.latest.path;

                match Installer::new(uri) {
                    Ok(mut installer) => {
                        installer.on_update(params.on_update);

                        installer.set_downloader(params.downloader);
                        installer.set_downloader_interval(params.downloader_updates_interval);
                        installer.set_unpacker_interval(params.unpacker_updates_interval);

                        installer.install(path)
                    },
                    Err(err) => Err(Error::new(ErrorKind::AddrNotAvailable, format!("Installer init error: {:?}", err)))
                }
            },
            Err(err) => Err(Error::new(ErrorKind::AddrNotAvailable, format!("Installer init error: {:?}", err)))
        }
    }
}
