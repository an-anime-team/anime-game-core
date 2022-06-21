use std::path::Path;
use std::cmp::{max, min};
use std::io::{Error, ErrorKind};

use fs_extra::dir::get_size;

use super::locale::VoiceLocale;
use crate::version::Version;
use crate::api::API;
use crate::json_schemas::versions::{
    Response as ApiResponse,
    VoicePack as RemoteVoicePack
};
use crate::consts::get_voice_package_path;
use crate::installer::diff::{VersionDiff, TryGetDiff};

pub enum VoicePackage {
    Installed {
        path: String,
        locale: VoiceLocale
    },
    NotInstalled {
        locale: VoiceLocale,
        version: Version,
        data: RemoteVoicePack,
        game_path: Option<String>
    }
}

impl VoicePackage {
    /// Voice packages can't be instaled wherever you want.
    /// Thus this method can return `None` in case the path
    /// doesn't point to a real voice package folder
    pub fn new<T: ToString>(path: T) -> Option<Self> {
        let path = path.to_string();
        let fs_path = Path::new(path.as_str());

        if fs_path.exists() && fs_path.is_dir() {
            match fs_path.file_name() {
                Some(name) => match VoiceLocale::from_str(name.to_string_lossy()) {
                    Some(locale) => Some(Self::Installed {
                        path,
                        locale
                    }),
                    None => None
                },
                None => None
            }
        }

        else {
            None
        }
    }

    /// Get installation status of this package
    /// 
    /// This method will return `false` if this package is `VoicePackage::NotInstalled` enum value
    /// 
    /// If you want to check it's actually installed - you'd need to use `is_installed_in`
    pub fn is_installed(&self) -> bool {
        match self {
            Self::Installed { .. } => true,
            Self::NotInstalled { .. } => false
        }
    }

    /// Calculate voice package size in bytes
    pub fn get_size(&self) -> u64 {
        match self {
            VoicePackage::Installed { path, .. } => get_size(path).unwrap(),
            VoicePackage::NotInstalled { data, .. } => data.package_size.parse::<u64>().unwrap(),
        }
    }

    /// This method will return `true` if the package has `VoicePackage::Installed` enum value
    /// 
    /// If it's `VoicePackage::NotInstalled`, then this method will check `game_path`'s voices folder
    pub fn is_installed_in<T: ToString>(&self, game_path: T) -> bool {
        match self {
            Self::Installed { .. } => true,
            Self::NotInstalled { locale, .. } => {
                Path::new(&get_voice_package_path(game_path, locale.to_folder())).exists()
            }
        }
    }

    /// Get list of 
    pub fn list_latest() -> Option<Vec<VoicePackage>> {
        match API::try_fetch() {
            Ok(response) => {
                match response.try_json::<ApiResponse>() {
                    Ok(response) => {
                        let mut packages = Vec::new();
                        let version = Version::from_str(response.data.game.latest.version);

                        for package in response.data.game.latest.voice_packs {
                            packages.push(Self::NotInstalled {
                                locale: VoiceLocale::from_str(&package.language).unwrap(),
                                version: version.clone(),
                                data: package,
                                game_path: None
                            });
                        }

                        Some(packages)
                    },
                    Err(_) => None
                }
            },
            Err(_) => None
        }
    }

    /// Get voice package locale
    pub fn locale(&self) -> VoiceLocale {
        match self {
            Self::Installed { path: _, locale } => *locale,
            Self::NotInstalled { locale, version: _, data: _, game_path: _ } => *locale
        }
    }

    /// This method can fail to parse this package version.
    /// It also can mean that the corresponding folder doesn't
    /// contain voice package files
    /// 
    /// TODO: maybe some errors output
    pub fn try_get_version(&self) -> Option<Version> {
        fn find_voice_pack(list: Vec<RemoteVoicePack>, locale: VoiceLocale) -> RemoteVoicePack {
            for pack in list {
                if pack.language == locale.to_code() {
                    return pack;
                }
            }
    
            // We're sure that all possible voice packages are listed in VoiceLocale... right?
            unreachable!();
        }

        match &self {
            Self::NotInstalled { locale: _, version, data: _, game_path: _} => Some(*version),
            Self::Installed { path, locale } => {
                // self.path is Some(...) if self.version is None
                // this means that this struct was made from some currently installed path
                match get_size(&path) {
                    Ok(package_size) => {
                        // Since anime company changed the way they store voice packages data
                        // now to identify its version I want to calculate the actual
                        // size of the voice package directory and compare it with all the
                        // remotely available voice packages sizes. The closest one is the actual version of the package
        
                        // (version, remote_size, |package_size - remote_size|)
                        let mut curr: (Version, u64, u64);
        
                        match API::try_fetch() {
                            Ok(response) => {
                                match response.try_json::<ApiResponse>() {
                                    Ok(response) => {
                                        let latest_voice_pack = find_voice_pack(response.data.game.latest.voice_packs, *locale);
        
                                        curr = (
                                            Version::from_str(response.data.game.latest.version),
                                            latest_voice_pack.package_size.parse().unwrap(),
                                            0
                                        );
        
                                        // We have to use it here because e.g. (2 - 3) can cause u64 overflow
                                        curr.2 = max(package_size, curr.1) - min(package_size, curr.1);
        
                                        // List through other versions of the game
                                        for diff in response.data.game.diffs {
                                            let voice_pack = find_voice_pack(diff.voice_packs, *locale);
                                            let voice_pack_size = voice_pack.package_size.parse().unwrap();
        
                                            let size_diff = max(package_size, voice_pack_size) - min(package_size, voice_pack_size);
        
                                            // If this version has lower size difference - then it's likely
                                            // an actual version we have
                                            if size_diff < curr.2 {
                                                curr = (
                                                    Version::from_str(diff.version),
                                                    voice_pack_size,
                                                    size_diff
                                                );
                                            }
                                        }
        
                                        // If the difference is too big - we expect this voice package
                                        // to be like really old, and we can't predict its version
                                        // for now this difference is 3 GB. Idk which value is better
                                        // This one should work fine for 2.5.0 - 2.7.0 versions window
                                        if curr.2 < 3072000000 {
                                            Some(curr.0)
                                        }
        
                                        else {
                                            None
                                        }
                                    },
                                    Err(_) => None
                                }
                            },
                            Err(_) => None
                        }
                    },
                    Err(_) => None
                }
            }
        }
    }
}

impl TryGetDiff for VoicePackage {
    fn try_get_diff(&self) -> Result<VersionDiff, Error> {
        match API::try_fetch() {
            Ok(response) => match response.try_json::<ApiResponse>() {
                Ok(response) => {
                    if self.is_installed() {
                        match self.try_get_version() {
                            Some(current) => {
                                if response.data.game.latest.version == current {
                                    Ok(VersionDiff::Latest(current))
                                }
            
                                else {
                                    for diff in response.data.game.diffs {
                                        if diff.version == current {
                                            return Ok(VersionDiff::Diff {
                                                current,
                                                latest: Version::from_str(response.data.game.latest.version),
                                                url: diff.path,
                                                download_size: diff.size.parse::<u64>().unwrap(),
                                                unpacked_size: diff.package_size.parse::<u64>().unwrap(),
                                                unpacking_path: match self {
                                                    VoicePackage::Installed { .. } => None,
                                                    VoicePackage::NotInstalled { game_path, .. } => game_path.clone(),
                                                }
                                            })
                                        }
                                    }
            
                                    Ok(VersionDiff::Outdated {
                                        current,
                                        latest: Version::from_str(response.data.game.latest.version)
                                    })
                                }
                            },
                            None => Err(Error::new(ErrorKind::Other, "Failed to get voice package version"))
                        }
                    }
                    
                    else {
                        let latest = response.data.game.latest;

                        Ok(VersionDiff::NotInstalled {
                            latest: Version::from_str(&latest.version),
                            url: latest.path,
                            download_size: latest.size.parse::<u64>().unwrap(),
                            unpacked_size: latest.package_size.parse::<u64>().unwrap(),
                            unpacking_path: match self {
                                VoicePackage::Installed { .. } => None,
                                VoicePackage::NotInstalled { game_path, .. } => game_path.clone(),
                            }
                        })
                    }
                },
                Err(err) => Err(Error::new(ErrorKind::InvalidData, format!("Failed to decode server response: {}", err.to_string())))
            },
            Err(err) => Err(err)
        }
    }
}
