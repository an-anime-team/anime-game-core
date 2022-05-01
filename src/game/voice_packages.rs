use std::fs::read_dir;
use std::io::{Error, ErrorKind};
use std::time::Duration;

use crate::json_schemas;
use crate::locales::VoiceLocales;
use crate::Version;
use crate::installer::prelude::*;

pub const LOCALES_FOLDERS: &[(VoiceLocales, &str)] = &[
    (VoiceLocales::Chinese, "Chinese"),
    (VoiceLocales::English, "English(US)"),
    (VoiceLocales::Japanese, "Japanese"),
    (VoiceLocales::Korean, "Korean")
];

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DiffError {
    AlreadyLatest,
    RemoteNotAvailable,
    CanNotCalculate,
    CanNotGetLocaleInfo
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diff {
    from: Version,
    to: Version,
    uri: String,
    size: u64,
    path_to_game: String
}

impl Diff {
    pub fn get_from_version(&self) -> Version {
        self.from
    }
    
    pub fn get_to_version(&self) -> Version {
        self.to
    }

    pub fn get_uri(&self) -> String {
        self.uri.clone()
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    #[cfg(feature = "install")]
    pub fn get_installer(&self) -> Result<Installer, minreq::Error> {
        Installer::new(self.uri.clone())
    }

    #[cfg(feature = "install")]
    pub fn download(&self, params: InstallerParams) -> Result<Duration, Error> {
        self.download_to(self.path_to_game.clone(), params)
    }

    #[cfg(feature = "install")]
    pub fn download_to<T: ToString>(&self, path: T, params: InstallerParams) -> Result<Duration, Error> {
        let path = path.to_string();
        let uri = self.uri.clone();

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
    }
}

pub struct VoicePackage {
    pub locale: VoiceLocales,
    pub version: Version,
    pub path: String
}

impl VoicePackage {
    pub fn installed(&self) -> bool {
        read_dir(&self.path).is_ok()
    }
}

pub struct VoicePackages {
    path: String,
    remote: Option<json_schemas::versions::Response>
}

impl VoicePackages {
    pub fn new(path: String, remote: Option<json_schemas::versions::Response>) -> VoicePackages {
        VoicePackages {
            path,
            remote
        }
    }

    pub fn locales_folder(&self) -> String {
        format!("{}/GenshinImpact_Data/StreamingAssets/Audio/GeneratedSoundBanks/Windows", &self.path)
    }

    pub fn folder_to_locale<T: ToString>(folder: T) -> Option<VoiceLocales> {
        for (locale, locale_folder) in LOCALES_FOLDERS {
            if locale_folder == &folder.to_string() {
                return Some(locale.clone())
            }
        }

        None
    }

    pub fn locale_to_folder(locale: VoiceLocales) -> String {
        for (voice_locale, folder) in LOCALES_FOLDERS {
            if voice_locale == &locale {
                return folder.clone().to_string()
            }
        }

        unreachable!()
    }

    /// Try to get info about installed voice package
    pub fn get_info(&self, locale: VoiceLocales) -> Option<VoicePackage> {
        let locale_folder = format!("{}/{}", self.locales_folder(), Self::locale_to_folder(locale));

        match read_dir(locale_folder.clone()) {
            Ok(dir) => {
                let mut latest = "1.0".to_string();
    
                for entry in dir {
                    if let Ok(entry) = entry {
                        let folder = entry.file_name().to_str().unwrap().to_string();

                        if &folder[..3] == "VO_" && &folder[3..6] > &latest {
                            latest = folder[3..6].to_string();
                        }
                    }
                }

                latest += ".0";

                Some(VoicePackage {
                    locale: locale.clone(),
                    version: Version::from_str(latest.as_str()),
                    path: locale_folder
                })
            },
            Err(_) => None
        }
    }

    /// Get list of installed voice packages
    pub fn installed(&self) -> Result<Vec<VoicePackage>, Error> {
        let mut packages = Vec::new();

        let locales_folder = self.locales_folder();

        match read_dir(locales_folder.clone()) {
            Ok(dir) => {
                for entry in dir {
                    if let Ok(entry) = entry {
                        if let Ok(info) = entry.file_type() {
                            if info.is_dir() {
                                let folder = entry.file_name().to_str().unwrap().to_string();
    
                                if let Some(package) = self.get_info(Self::folder_to_locale(folder).unwrap()) {
                                    packages.push(package);
                                }
                            }
                        }
                    }
                }
            },
            Err(err) => return Err(err)
        }

        Ok(packages)
    }

    /// Try to get list of available voice packages
    /// 
    /// Returns `None` if remote server is not available
    pub fn available(&self) -> Option<Vec<VoicePackage>> {
        match &self.remote {
            Some(remote) => {
                let mut packages = Vec::new();

                let version = Version::from_str(&remote.data.game.latest.version);
                let locales_folder = self.locales_folder();

                for pack in &remote.data.game.latest.voice_packs {
                    if let Some(locale) = VoiceLocales::from_str(&pack.language) {
                        packages.push(VoicePackage {
                            locale,
                            version,
                            path: format!("{}/{}", locales_folder, Self::locale_to_folder(locale))
                        });
                    }
                }

                Some(packages)
            },
            None => None
        }
    }

    /// Try to get a difference between installed voice package and the latest availale version
    pub fn diff(&self, locale: VoiceLocales) -> Result<Diff, DiffError> {
        match self.get_info(locale) {
            Some(info) => {
                match &self.remote {
                    Some(remote) => {
                        if info.version == remote.data.game.latest.version {
                            return Err(DiffError::AlreadyLatest);
                        }

                        for diff in &remote.data.game.diffs {
                            if diff.version == info.version {
                                return Ok(Diff {
                                    from: info.version.clone(),
                                    to: Version::from_str(remote.data.game.latest.version.clone()),
                                    uri: diff.path.clone(),
                                    size: diff.size.parse().unwrap(),
                                    path_to_game: self.path.clone()
                                });
                            }
                        }

                        Err(DiffError::CanNotCalculate)
                    },
                    None => Err(DiffError::RemoteNotAvailable)
                }
            },
            None => Err(DiffError::CanNotGetLocaleInfo)
        }
    }

    #[cfg(feature = "install")]
    pub fn download(&self, locale: VoiceLocales, params: InstallerParams) -> Result<Duration, Error> {
        self.download_to(self.path.clone(), locale, params)
    }

    #[cfg(feature = "install")]
    // TODO: find a way to somehow unite download functions with game_version mod
    pub fn download_to<T: ToString>(&self, path: T, locale: VoiceLocales, params: InstallerParams) -> Result<Duration, Error> {
        match &self.remote {
            Some(remote) => {
                let path = path.to_string();
                let mut uri = String::new();

                for pack in &remote.data.game.latest.voice_packs {
                    if pack.language == locale.to_code() {
                        uri = pack.path.clone();

                        break;
                    }
                }

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
            None => Err(Error::new(ErrorKind::InvalidData, "Remote server is not available"))
        }
    }
}
