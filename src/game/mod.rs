use crate::consts;
use crate::json_schemas;

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
}
