use std::fs::read_dir;
use std::io::Error;

use crate::json_schemas;
use crate::locales::VoiceLocales;
use crate::Version;

pub const LOCALES_FOLDERS: &[(VoiceLocales, &str)] = &[
    (VoiceLocales::Chinese, "Chinese"),
    (VoiceLocales::English, "English(US)"),
    (VoiceLocales::Japanese, "Japanese"),
    (VoiceLocales::Korean, "Korean")
];

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

    pub fn folder_to_locale(folder: &str) -> Option<VoiceLocales> {
        for (locale, locale_folder) in LOCALES_FOLDERS {
            if locale_folder == &folder {
                return Some(locale.clone())
            }
        }

        None
    }

    pub fn locale_to_folder(locale: VoiceLocales) -> Option<String> {
        for (voice_locale, folder) in LOCALES_FOLDERS {
            if voice_locale == &locale {
                return Some(folder.clone().to_string())
            }
        }

        None
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
    
                                if let Some(locale) = Self::folder_to_locale(folder.as_str()) {
                                    let mut latest = "1.0".to_string();
    
                                    for entry in read_dir(format!("{}/{}", locales_folder, folder)).unwrap() {
                                        if let Ok(entry) = entry {
                                            let folder = entry.file_name().to_str().unwrap().to_string();
    
                                            if &folder[..3] == "VO_" && &folder[3..6] > &latest {
                                                latest = folder[3..6].to_string();
                                            }
                                        }
                                    }
    
                                    latest += ".0";
    
                                    packages.push(VoicePackage {
                                        locale: locale.clone(),
                                        version: Version::from_str(latest.as_str()),
                                        path: format!("{}/{}", locales_folder, folder)
                                    });
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

    pub fn available(&self) -> Option<Vec<VoicePackage>> {
        match &self.remote {
            Some(remote) => {
                let mut packages = Vec::new();

                let version = Version::from_str(&remote.data.game.latest.version);
                let locales_folder = self.locales_folder();

                for pack in &remote.data.game.latest.voice_packs {
                    if let Some(locale) = VoiceLocales::from_str(&pack.language) {
                        if let Some(folder) = Self::locale_to_folder(locale) {
                            packages.push(VoicePackage {
                                locale,
                                version,
                                path: format!("{}/{}", locales_folder, folder)
                            });
                        }
                    }
                }

                Some(packages)
            },
            None => None
        }
    }
}
