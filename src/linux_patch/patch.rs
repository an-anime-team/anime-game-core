use crate::version::Version;
use crate::curl::fetch;
use crate::api::API;

/// If this line is commented in the `patch.sh` file, then it's stable version.
/// Otherwise it's in testing phase
const STABILITY_MARK: &'static str = "#echo \"If you would like to test this patch, modify this script and remove the line below this one.\"";

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

#[derive(Debug, Clone)]
pub enum Patch {
    /// Curl error (failed to perform request)
    Curl(curl::Error),

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
    /// TODO: this should be changed in future
    pub fn fetch<T: ToString>(hosts: Vec<T>) -> Option<Self> {
        match API::try_fetch() {
            Ok(response) => {
                match response.try_json::<crate::json_schemas::versions::Response>() {
                    Ok(response) => {
                        let mut versions = vec![Version::from_str(response.data.game.latest.version)];

                        for diff in response.data.game.diffs {
                            versions.push(Version::from_str(diff.version));
                        }

                        for version in versions {
                            for host in &hosts {
                                match Self::fetch_version(host.to_string(), version) {
                                    Patch::Curl(_) => continue,
                                    Patch::NotAvailable => continue,

                                    status => return Some(status)
                                }
                            }
                        }
                        
                        // No useful outputs from all the hosts
                        Some(Patch::NotAvailable)
                    },
                    Err(_) => None
                }
            },
            Err(_) => None
        }
    }

    /// Try to fetch the patch with specified game version
    /// 
    /// Never returns `Some(Patch::Outdated)` because doesn't check the latest game version
    pub fn fetch_version<T: ToString>(host: T, version: Version) -> Self {
        match fetch(format!("{}/raw/master/{}/README.txt", host.to_string(), version.to_plain_string())) {
            Ok(response) => {
                // Preparation / Testing / Available
                if response.is_ok() {
                    match fetch(format!("{}/raw/master/{}/patch_files/unityplayer_patch_os.vcdiff", host.to_string(), version.to_plain_string())) {
                        Ok(response) => {
                            // Testing / Available
                            if response.is_ok() {
                                match fetch(format!("{}/raw/master/{}/patch.sh", host.to_string(), version.to_plain_string())) {
                                    Ok(mut response) => {
                                        match response.get_body() {
                                            Ok(body) => {
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

                                                match player_hash {
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
                                                }
                                            },
                                            Err(err) => Self::Curl(err)
                                        }
                                    },
                                    Err(err) => Self::Curl(err)
                                }
                            }

                            // This file is not found so it should be preparation state
                            else if response.status == Some(404) {
                                Self::Preparation {
                                    version,
                                    host: host.to_string()
                                }
                            }

                            // Server is not available
                            else {
                                Self::NotAvailable
                            }
                        },
                        Err(err) => Self::Curl(err)
                    }
                }
        
                // Not found / server is not available / ...
                else {
                    Self::NotAvailable
                }
            },
            Err(err) => Self::Curl(err)
        }
    }
}
