use std::collections::HashMap;
use std::time::Duration;

use crate::version::{Version, ToVersion};
use crate::curl::fetch;
use crate::api::API;

/// If this line is commented in the `patch.sh` file, then it's stable version.
/// Otherwise it's in testing phase
const STABILITY_MARK: &str = "#echo \"If you would like to test this patch, modify this script and remove the line below this one.\"";

static mut PATCH_RESPONSES: Option<HashMap<(String, Version), Patch>> = None;

#[derive(Debug, Clone)]
pub enum Regions {
    /// `UnityPlayer.dll` md5 hash
    Global(String),
    
    /// `UnityPlayer.dll` md5 hash
    China(String),

    /// `UnityPlayer.dll` md5 hashes for different regions
    Both {
        global: String,
        china: String
    }
}

impl Regions {
    /// Compares `player_hash` with inner values
    /// 
    /// If `player_hash` not equal to the inner value, then the patch was applied
    pub fn is_applied<T: ToString>(&self, player_hash: T) -> bool {
        let player_hash = &player_hash.to_string();

        match self {
            Self::Global(hash) => hash != player_hash,
            Self::China(hash) => hash != player_hash,
            Self::Both { global, china } => global != player_hash && china != player_hash
        }
    }
}

#[derive(Debug, Clone)]
pub enum Patch {
    /// Patch is not available
    NotAvailable,

    /// The patch is outdated and nothing was made to update it
    Outdated {
        current: Version,
        latest: Version,
        host: String
    },

    /// Some preparations for the new version of the game were made, but the patch is not available
    /// 
    /// Technically the same as `Outdated`
    Preparation {
        version: Version,
        host: String
    },

    /// Patch is available for the latest version of the game, but only in testing mode
    Testing {
        version: Version,
        host: String,
        player_hash: Regions
    },

    /// Patch is fully available and tested for the latest version of the game
    Available {
        version: Version,
        host: String,
        player_hash: Regions
    }
}

impl Patch {
    /// Try to fetch remote patch state
    /// 
    /// This method will look at hosts in their order. If the first host is not available - then
    /// it'll check the second host. Once the host is available this method will gather path status
    /// and return it. This means that if the first host contains outdated version, and the second - updated,
    /// this method will return outdated version
    /// 
    /// Timeout is applied to all the requests separately, so the whole operation can take more than `Some(timeout)`
    pub fn try_fetch<T: ToString>(hosts: Vec<T>, timeout: Option<Duration>) -> Result<Self, curl::Error> {
        let response = API::try_fetch()?;
        
        match response.try_json::<crate::json_schemas::versions::Response>() {
            Ok(response) => {
                let mut versions = vec![Version::from_str(&response.data.game.latest.version).unwrap()];

                for diff in response.data.game.diffs {
                    versions.push(Version::from_str(diff.version).unwrap());
                }

                for version in versions {
                    for host in &hosts {
                        match Self::try_fetch_version(host.to_string(), version, timeout) {
                            Ok(Patch::NotAvailable) => continue,
                            Err(_) => continue,

                            Ok(status) => {
                                return if Some(version) == Version::from_str(&response.data.game.latest.version) {
                                    Ok(status)
                                } else {
                                    Ok(Self::Outdated {
                                        current: version,
                                        latest: Version::from_str(&response.data.game.latest.version).unwrap(),
                                        host: host.to_string()
                                    })
                                }
                            }
                        }
                    }
                }
                
                // No useful outputs from all the hosts
                Ok(Patch::NotAvailable)
            },
            Err(_) => panic!("Failed to decode json server response") // FIXME
        }
    }

    /// Try to fetch the patch with specified game version
    /// 
    /// Never returns `Some(Patch::Outdated)` because doesn't check the latest game version
    pub fn try_fetch_version<T: ToString>(host: T, version: Version, timeout: Option<Duration>) -> Result<Self, curl::Error> {
        // Return cached value if it exists
        unsafe {
            if let Some(responses) = &PATCH_RESPONSES {
                if let Some(response) = responses.get(&(host.to_string(), version)) {
                    return Ok(response.clone());
                }
            }

            else {
                PATCH_RESPONSES = Some(HashMap::new());
            }
        }

        // Otherwise gather patch info
        let response = fetch(format!("{}/raw/master/{}/README.txt", host.to_string(), version.to_plain_string()), timeout)?;

        // Preparation / Testing / Available
        if response.is_ok() {
            let response = fetch(format!("{}/raw/master/{}/patch_files/unityplayer_patch_os.vcdiff", host.to_string(), version.to_plain_string()), timeout)?;

            // Testing / Available
            if response.is_ok() {
                let mut response = fetch(format!("{}/raw/master/{}/patch.sh", host.to_string(), version.to_plain_string()), timeout)?;

                let body = response.get_body()?;
                let body = String::from_utf8_lossy(&body);

                let mut hashes = Vec::new();

                for line in body.lines() {
                    // if [ "${sum}" == "8c8c3d845b957e4cb84c662bed44d072" ]; then
                    // if [ "${sum}" == "<TODO>" ]; then
                    if line.len() > 20 && &line[..18] == "if [ \"${sum}\" == \"" {
                        let hash = &line[18..line.len() - 9];

                        hashes.push(if hash.len() == 32 { Some(hash) } else { None });
                    }
                }

                let player_hash = match hashes.len() {
                    0 => None,
                    1 => {
                        if hashes[0] == None {
                            None
                        } else {
                            Some(Regions::Global(hashes[0].unwrap().to_string()))
                        }
                    },
                    2 => {
                        if hashes[0] == None {
                            Some(Regions::China(hashes[1].unwrap().to_string()))
                        }

                        else if hashes[1] == None {
                            Some(Regions::Global(hashes[0].unwrap().to_string()))
                        }

                        else {
                            Some(Regions::Both {
                                global: hashes[0].unwrap().to_string(),
                                china: hashes[1].unwrap().to_string()
                            })
                        }
                    },
                    _ => unreachable!()
                };

                let patch = match player_hash {
                    Some(player_hash) => {
                        // If patch.sh contains STABILITY_MARK - then it's stable version
                        if body.contains(STABILITY_MARK) {
                            Self::Available {
                                version,
                                host: host.to_string(),
                                player_hash
                            }
                        }

                        // Otherwise it's in testing
                        else {
                            Self::Testing {
                                version,
                                host: host.to_string(),
                                player_hash
                            }
                        }
                    },

                    // Failed to parse UnityPlayer.dll hashes -> likely in preparation state
                    // but also could be changed file structure, or something else
                    None => Self::Preparation {
                        version,
                        host: host.to_string()
                    }
                };

                // Cache result
                unsafe {
                    let responses = PATCH_RESPONSES.as_mut().unwrap();

                    responses.insert((host.to_string(), version), patch.clone());
                }

                // Return patch
                Ok(patch)
            }

            // This file is not found so it should be preparation state
            else if response.status == Some(404) {
                let patch = Self::Preparation {
                    version,
                    host: host.to_string()
                };

                unsafe {
                    let responses = PATCH_RESPONSES.as_mut().unwrap();

                    responses.insert((host.to_string(), version), patch.clone());
                }

                Ok(patch)
            }

            // Server is not available
            else {
                let patch = Self::NotAvailable;

                unsafe {
                    let responses = PATCH_RESPONSES.as_mut().unwrap();

                    responses.insert((host.to_string(), version), patch.clone());
                }

                Ok(Self::NotAvailable)
            }
        }

        // Not found / server is not available / ...
        else {
            let patch = Self::NotAvailable;

            unsafe {
                let responses = PATCH_RESPONSES.as_mut().unwrap();

                responses.insert((host.to_string(), version), patch.clone());
            }

            Ok(Self::NotAvailable)
        }
    }

    /// Check whether this patch is applied to the game
    /// 
    /// This method will return `Ok(false)` if the patch is not available, outdated or in preparation state
    pub fn is_applied<T: ToString>(&self, game_path: T) -> Result<bool, std::io::Error> {
        let dll = std::fs::read(format!("{}/UnityPlayer.dll", game_path.to_string()))?;
        let hash = format!("{:x}", md5::compute(dll));

        match self {
            Patch::NotAvailable => Ok(false),
            Patch::Outdated { .. } => Ok(false),
            Patch::Preparation { .. } => Ok(false),

            Patch::Testing { player_hash, .. } => Ok(player_hash.is_applied(hash)),
            Patch::Available { player_hash, .. } => Ok(player_hash.is_applied(hash))
        }
    }
}

impl ToVersion for Patch {
    fn to_version(&self) -> Option<Version> {
        match self {
            Patch::Preparation { version, .. } |
            Patch::Testing { version, .. } |
            Patch::Available { version, .. } => Some(*version),
            _ => None
        }
    }
}
