use std::path::PathBuf;

use fs_extra::dir::get_size;

use crate::version::Version;

use crate::genshin::{
    voice_data::locale::VoiceLocale,
    json_schemas::versions::VoicePack as RemoteVoicePack,
    consts::get_voice_package_path,
    api
};

#[cfg(feature = "install")]
use crate::installer::diff::{VersionDiff, TryGetDiff};

/// List of voiceover sizes
/// 
/// Format: `(version, english, japanese, korean, chinese)`
/// 
/// Korean and Chinese sizes are predicted and can be incorrect
pub const VOICE_PACKAGES_SIZES: &[(&str, u64, u64, u64, u64)] = &[
    ("3.1.0", 10160526140, 11223463952, 10160526140, 10160526140),
    ("3.0.0", 9359645164,  10314955860, 9359645164,  9359645164)
];

/// Get specific voice package sizes from `VOICE_PACKAGES_SIZES` constant
pub fn get_voice_pack_sizes(locale: VoiceLocale) -> Vec<(String, u64)> {
    VOICE_PACKAGES_SIZES.into_iter().map(|item| {
        match locale {
            VoiceLocale::English  => (item.0.to_string(), item.1),
            VoiceLocale::Japanese => (item.0.to_string(), item.2),
            VoiceLocale::Korean   => (item.0.to_string(), item.3),
            VoiceLocale::Chinese  => (item.0.to_string(), item.4)
        }
    }).collect::<Vec<(String, u64)>>()
}

/// Predict next value of slice using WMA
pub fn wma_predict(values: &[u64]) -> u64 {
    match values.len() {
        0 => 0,
        1 => values[1],
        2 => (values[1] as f64 * (values[1] as f64 / values[0] as f64)).round() as u64,
        n => {
            let mut weighted_sum = 0.0;
            let mut weighted_delim = 0.0;

            for i in 0..n - 1 {
                weighted_sum += values[i + 1] as f64 / values[i] as f64 * (n - i - 1) as f64;
                weighted_delim += (n - i - 1) as f64;
            }

            (values[n - 1] as f64 * weighted_sum / weighted_delim).round() as u64
        }
    }
}

/// Predict new voice package size using WMA based on `VOICE_PACKAGES_SIZES` constant
pub fn predict_new_voice_pack_size(locale: VoiceLocale) -> u64 {
    wma_predict(&get_voice_pack_sizes(locale).into_iter().map(|item| item.1).collect::<Vec<u64>>())
}

/// Find voice package with specified locale from list of packages
fn find_voice_pack(list: Vec<RemoteVoicePack>, locale: VoiceLocale) -> RemoteVoicePack {
    for pack in list {
        if pack.language == locale.to_code() {
            return pack;
        }
    }

    // We're sure that all possible voice packages are listed in VoiceLocale... right?
    unreachable!();
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VoicePackage {
    Installed {
        path: PathBuf,
        locale: VoiceLocale
    },
    NotInstalled {
        locale: VoiceLocale,
        version: Version,
        data: RemoteVoicePack,
        game_path: Option<PathBuf>
    }
}

impl VoicePackage {
    /// Voice packages can't be instaled wherever you want.
    /// Thus this method can return `None` in case the path
    /// doesn't point to a real voice package folder
    pub fn new<T: Into<PathBuf>>(path: T) -> Option<Self> {
        let path = path.into();

        if path.exists() && path.is_dir() {
            match path.file_name() {
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

    /// Get latest voice package with specified locale
    /// 
    /// Note that returned object will be `VoicePackage::NotInstalled`, but
    /// technically it can be installed. This method just don't know the game's path
    pub fn with_locale(locale: VoiceLocale) -> anyhow::Result<Self> {
        let response = api::try_fetch_json()?;
        let latest = response.data.game.latest;

        Ok(Self::NotInstalled {
            locale,
            version: Version::from_str(latest.version).unwrap(),
            data: find_voice_pack(latest.voice_packs, locale),
            game_path: None
        })
    }

    // TODO: find_in(game_path: String, locale: VoiceLocale)

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
    /// 
    /// (unpacked size, Option(archive size))
    pub fn size(&self) -> (u64, Option<u64>) {
        match self {
            VoicePackage::Installed { path, .. } => (get_size(path).unwrap(), None),
            VoicePackage::NotInstalled { data, .. } => (
                data.package_size.parse::<u64>().unwrap(),
                Some(data.size.parse::<u64>().unwrap())
            ),
        }
    }

    /// This method will return `true` if the package has `VoicePackage::Installed` enum value
    /// 
    /// If it's `VoicePackage::NotInstalled`, then this method will check `game_path`'s voices folder
    pub fn is_installed_in<T: Into<PathBuf>>(&self, game_path: T) -> bool {
        match self {
            Self::Installed { .. } => true,
            Self::NotInstalled { locale, .. } => {
                get_voice_package_path(game_path, locale.to_folder()).exists()
            }
        }
    }

    /// Get list of latest voice packages
    pub fn list_latest() -> anyhow::Result<Vec<VoicePackage>> {
        let response = api::try_fetch_json()?;

        let mut packages = Vec::new();
        let version = Version::from_str(response.data.game.latest.version).unwrap();

        for package in response.data.game.latest.voice_packs {
            packages.push(Self::NotInstalled {
                locale: VoiceLocale::from_str(&package.language).unwrap(),
                version: version.clone(),
                data: package,
                game_path: None
            });
        }

        Ok(packages)
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
    pub fn try_get_version(&self) -> anyhow::Result<Version> {
        match &self {
            Self::NotInstalled { locale: _, version, data: _, game_path: _} => Ok(*version),
            Self::Installed { path, locale } => {
                let package_size = get_size(&path)?;
                let response = api::try_fetch_json()?;

                let mut voice_packages_sizes = get_voice_pack_sizes(*locale);

                // If latest voice packages sizes aren't listed in `VOICE_PACKAGES_SIZES`
                // then we should predict their sizes
                if VOICE_PACKAGES_SIZES[0].0 != response.data.game.latest.version {
                    let mut t = voice_packages_sizes;

                    voice_packages_sizes = vec![(response.data.game.latest.version.clone(), predict_new_voice_pack_size(*locale))];
                    voice_packages_sizes.append(&mut t);
                }

                // To predict voice package version we're going through saved voice packages sizes in the `VOICE_PACKAGES_SIZES` constant
                // plus predicted voice packages sizes if needed. The version with closest folder size is version we have installed
                for (version, size) in voice_packages_sizes {
                    if package_size > size - 512 * 1024 * 1024 {
                        return Ok(Version::from_str(version).unwrap());
                    }
                }

                // This *should* be unreachable
                unreachable!()
            }
        }
    }

    /// Try to delete voice package
    /// 
    /// FIXME:
    /// ⚠️ May fail on Chinese version due to paths differences
    pub fn delete(&self) -> anyhow::Result<()> {
        match self {
            VoicePackage::Installed { path, .. } => {
                let mut game_path = path.clone();

                for _ in 0..6 {
                    game_path = match game_path.parent() {
                        Some(game_path) => game_path.into(),
                        None => return Err(anyhow::anyhow!("Failed to find game directory"))
                    };
                }

                self.delete_in(game_path)
            },
            VoicePackage::NotInstalled { game_path, .. } => {
                match game_path {
                    Some(game_path) => self.delete_in(game_path),
                    None => return Err(anyhow::anyhow!("Failed to find game directory"))
                }
            }
        }
    }

    /// Try to delete voice package from specific game directory
    /// 
    /// FIXME:
    /// ⚠️ May fail on Chinese version due to paths differences
    pub fn delete_in<T: Into<PathBuf>>(&self, game_path: T) -> anyhow::Result<()> {
        let locale = match self {
            VoicePackage::Installed { locale, .. } |
            VoicePackage::NotInstalled { locale, .. } => locale
        };

        let game_path = game_path.into();

        // Audio_<locale folder>_pkg_version
        std::fs::remove_dir_all(get_voice_package_path(&game_path, locale.clone()))?;
        std::fs::remove_file(game_path.join(format!("Audio_{}_pkg_version", locale.to_folder())))?;

        Ok(())
    }
}

#[cfg(feature = "install")]
impl TryGetDiff for VoicePackage {
    fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        let response = api::try_fetch_json()?;

        if self.is_installed() {
            let current = self.try_get_version()?;

            if response.data.game.latest.version == current {
                // If we're running latest game version the diff we need to download
                // must always be `predownload.diffs[0]`, but just to be safe I made
                // a loop through possible variants, and if none of them was correct
                // (which is not possible in reality) we should just say thath the game
                // is latest
                if let Some(predownload) = response.data.pre_download_game {
                    for diff in predownload.diffs {
                        if diff.version == current {
                            let diff = find_voice_pack(diff.voice_packs, self.locale());

                            return Ok(VersionDiff::Predownload {
                                current,
                                latest: Version::from_str(predownload.latest.version).unwrap(),
                                url: diff.path,
                                download_size: diff.size.parse::<u64>().unwrap(),
                                unpacked_size: diff.package_size.parse::<u64>().unwrap(),
                                unpacking_path: match self {
                                    VoicePackage::Installed { .. } => None,
                                    VoicePackage::NotInstalled { game_path, .. } => game_path.clone()
                                }
                            })
                        }
                    }
                }

                Ok(VersionDiff::Latest(current))
            }

            else {
                for diff in response.data.game.diffs {
                    if diff.version == current {
                        let diff = find_voice_pack(diff.voice_packs, self.locale());

                        return Ok(VersionDiff::Diff {
                            current,
                            latest: Version::from_str(response.data.game.latest.version).unwrap(),
                            url: diff.path,
                            download_size: diff.size.parse::<u64>().unwrap(),
                            unpacked_size: diff.package_size.parse::<u64>().unwrap(),
                            unpacking_path: match self {
                                VoicePackage::Installed { .. } => None,
                                VoicePackage::NotInstalled { game_path, .. } => game_path.clone()
                            }
                        })
                    }
                }

                Ok(VersionDiff::Outdated {
                    current,
                    latest: Version::from_str(response.data.game.latest.version).unwrap()
                })
            }
        }
        
        else {
            let latest = find_voice_pack(response.data.game.latest.voice_packs, self.locale());

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(response.data.game.latest.version).unwrap(),
                url: latest.path,
                download_size: latest.size.parse::<u64>().unwrap(),
                unpacked_size: latest.package_size.parse::<u64>().unwrap(),
                unpacking_path: match self {
                    VoicePackage::Installed { .. } => None,
                    VoicePackage::NotInstalled { game_path, .. } => game_path.clone()
                }
            })
        }
    }
}
