use std::path::{Path, PathBuf};

use fs_extra::dir::get_size;

use crate::version::Version;

use crate::genshin::api::{
    self,
    schema::AudioPackage
};

use crate::genshin::consts::*;
use crate::genshin::voice_data::locale::VoiceLocale;

#[cfg(feature = "install")]
use crate::genshin::version_diff::*;

/// List of voiceover sizes
///
/// Format: `(version, english, japanese, korean, chinese)`
pub const VOICE_PACKAGES_SIZES: &[(&str, u64, u64, u64, u64)] = &[
    //         English(US)   Japanese      Korean        Chinese
    ("5.1.0",  16207071335,  18254192787,  13784144703,  14055732779),
    ("5.0.0",  18503031452,  20808521048,  15590542644,  15865413012),
    ("4.8.0",  17809995945,  20012816885,  14991865472,  15260781792), // Predicted
    ("4.7.0",  17116960439,  19217112723,  14393188301,  14656150572), // For whatever reason, who would have known,
                                                                       // those values are from the `size` field instead of `decompressed_size`
                                                                       // from the game API because later one looked COMPLETELY WRONG
    ("4.6.0",  16414267279,  18393435927,  13796034335,  14036168895),
    ("4.5.0",  15959234252,  17912625028,  13434833996,  13671635640),
    ("4.4.0",  15719789566,  17526530996,  13139301525,  13399060711), // Predicted
    ("4.3.0",  15067231819,  16799654823,  12604647523,  12861374519),
    ("4.2.0",  14569503723,  16263036031,  12221968655,  12476776215),
    ("4.1.0",  13889855947,  15500986871,  11635183963,  11885602119),
    ("4.0.0",  13109710863,  14592012075,  10979621411,  11224640167),
    ("3.8.0",  12220820203,  13571842139,  10221829179,  10441921175),
    ("3.7.0",  11778060451,  13044149443,  9857960459,   10075853323),
    ("3.6.0",  11041879555,  12412351703,  9434697975,   9626464559),
    ("3.5.0",  10352166715,  11641949699,  8861959147,   9062163032),
    ("3.4.0",  9702104595,   10879201351,  8329592851,   8498622343),
    ("3.3.0",  9183929971,   10250403911,  7896362859,   8047012675),
    ("3.2.0",  8636001252,   9600770928,   7416414724,   7563358032)
];

/// Acceptable error to select a version for the voiceover folder
pub const VOICE_PACKAGE_THRESHOLD: u64 = 400 * 1024 * 1024; // 400 MB, ~4 files

/// Get specific voice package sizes from `VOICE_PACKAGES_SIZES` constant
pub fn get_voice_pack_sizes<'a>(locale: VoiceLocale) -> Vec<(&'a str, u64)> {
    VOICE_PACKAGES_SIZES.iter().map(|item| {
        match locale {
            VoiceLocale::English  => (item.0, item.1),
            VoiceLocale::Japanese => (item.0, item.2),
            VoiceLocale::Korean   => (item.0, item.3),
            VoiceLocale::Chinese  => (item.0, item.4)
        }
    }).collect()
}

/// Predict next value of slice using WMA
pub fn wma_predict(values: &[u64]) -> u64 {
    match values.len() {
        0 => 0,
        1 => values[0],
        2 => (values[1] as f64 * (values[1] as f64 / values[0] as f64)).round() as u64,

        n => {
            let mut weighted_sum = 0.0;
            let mut weighted_delim = 0.0;

            for i in 0..n - 1 {
                weighted_sum += values[i + 1] as f64 / values[i] as f64 * (i + 1) as f64;
                weighted_delim += (i + 1) as f64;
            }

            (values[n - 1] as f64 * weighted_sum / weighted_delim).round() as u64
        }
    }
}

/// Predict new voice package size using WMA based on `VOICE_PACKAGES_SIZES` constant
pub fn predict_new_voice_pack_size(locale: VoiceLocale) -> u64 {
    wma_predict(&get_voice_pack_sizes(locale).into_iter().map(|item| item.1).rev().collect::<Vec<u64>>())
}

/// Find voice package with specified locale from list of packages
fn find_voice_pack(list: Vec<AudioPackage>, locale: VoiceLocale) -> AudioPackage {
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
        locale: VoiceLocale,
        game_edition: GameEdition
    },

    NotInstalled {
        locale: VoiceLocale,
        version: Version,
        data: AudioPackage,
        game_path: Option<PathBuf>,
        game_edition: GameEdition
    }
}

impl VoicePackage {
    /// Voice packages can't be instaled wherever you want.
    /// Thus this method can return `None` in case the path
    /// doesn't point to a real voice package folder
    pub fn new<T: Into<PathBuf>>(path: T, game_edition: GameEdition) -> Option<Self> {
        let path = path.into();

        if path.exists() && path.is_dir() {
            match path.file_name() {
                Some(name) => match VoiceLocale::from_str(name.to_string_lossy()) {
                    Some(locale) => Some(Self::Installed {
                        path,
                        locale,
                        game_edition
                    }),

                    None => None
                }

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
    pub fn with_locale(locale: VoiceLocale, game_edition: GameEdition) -> anyhow::Result<Self> {
        let latest = api::request(game_edition)?.main.major;

        Ok(Self::NotInstalled {
            locale,
            version: Version::from_str(latest.version).unwrap(),
            data: find_voice_pack(latest.audio_pkgs, locale),
            game_path: None,
            game_edition
        })
    }

    #[inline]
    pub fn game_edition(&self) -> GameEdition {
        match self {
            Self::Installed { game_edition, .. } |
            Self::NotInstalled { game_edition, .. } => *game_edition
        }
    }

    // TODO: find_in(game_path: String, locale: VoiceLocale)

    #[inline]
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
                data.decompressed_size.parse::<u64>().unwrap(),
                Some(data.size.parse::<u64>().unwrap())
            )
        }
    }

    #[inline]
    /// This method will return `true` if the package has `VoicePackage::Installed` enum value
    ///
    /// If it's `VoicePackage::NotInstalled`, then this method will check `game_path`'s voices folder
    pub fn is_installed_in<T: AsRef<Path>>(&self, game_path: T) -> bool {
        match self {
            Self::Installed { .. } => true,
            Self::NotInstalled { locale, .. } => get_voice_package_path(game_path, self.game_edition(), *locale).exists()
        }
    }

    /// Get list of latest voice packages
    pub fn list_latest(game_edition: GameEdition) -> anyhow::Result<Vec<VoicePackage>> {
        let response = api::request(game_edition)?;

        let mut packages = Vec::new();
        let version = Version::from_str(response.main.major.version).unwrap();

        for package in response.main.major.audio_pkgs {
            packages.push(Self::NotInstalled {
                locale: VoiceLocale::from_str(&package.language).unwrap(),
                version: version.clone(),
                data: package,
                game_path: None,
                game_edition
            });
        }

        Ok(packages)
    }

    #[inline]
    /// Get voice package locale
    pub fn locale(&self) -> VoiceLocale {
        match self {
            Self::Installed { locale, .. } |
            Self::NotInstalled { locale, .. } => *locale
        }
    }

    #[tracing::instrument(level = "debug", ret)]
    /// This method can fail to parse this package version.
    /// It also can mean that the corresponding folder doesn't
    /// contain voice package files
    pub fn try_get_version(&self) -> anyhow::Result<Version> {
        tracing::debug!("Trying to get {} voice package version", self.locale().to_code());

        match &self {
            Self::NotInstalled { version, .. } => Ok(*version),
            Self::Installed { path, locale, game_edition } => {
                let package_size = get_size(&path)?;
                let response = api::request(*game_edition)?;

                match std::fs::read(path.join(".version")) {
                    Ok(curr) => {
                        tracing::debug!("Found .version file: {}.{}.{}", curr[0], curr[1], curr[2]);

                        Ok(Version::new(curr[0], curr[1], curr[2]))
                    },

                    // We don't create .version file here because we don't
                    // actually know current version and just predict it
                    // This file will be properly created in the install method
                    Err(_) => {
                        tracing::debug!(".version file wasn't found. Predict version. Package size: {package_size}");

                        let mut voice_packages_sizes = get_voice_pack_sizes(*locale);

                        // If latest voice packages sizes aren't listed in `VOICE_PACKAGES_SIZES`
                        // then we should predict their sizes
                        if VOICE_PACKAGES_SIZES[0].0 != response.main.major.version {
                            let mut t = voice_packages_sizes;

                            voice_packages_sizes = vec![(&response.main.major.version, predict_new_voice_pack_size(*locale))];
                            voice_packages_sizes.append(&mut t);
                        }

                        // To predict voice package version we're going through saved voice packages sizes in the `VOICE_PACKAGES_SIZES` constant
                        // plus predicted voice packages sizes if needed. The version with closest folder size is version we have installed
                        for (version, size) in voice_packages_sizes {
                            if package_size > size - VOICE_PACKAGE_THRESHOLD {
                                tracing::debug!("Predicted version: {version}");

                                return Ok(Version::from_str(version).unwrap());
                            }
                        }

                        // This *should* be unreachable
                        unreachable!()
                    }
                }
            }
        }
    }

    #[tracing::instrument(level = "trace", ret)]
    /// Try to delete voice package
    ///
    /// FIXME:
    /// ⚠️ May fail on Chinese version due to paths differences
    pub fn delete(&self) -> anyhow::Result<()> {
        tracing::trace!("Deleting {} voice package", self.locale().to_code());

        match self {
            VoicePackage::Installed { path, .. } => {
                let mut game_path = path.clone();

                for _ in 0..4 {
                    game_path = match game_path.parent() {
                        Some(game_path) => game_path.into(),
                        None => {
                            tracing::error!("Failed to find game directory");

                            return Err(anyhow::anyhow!("Failed to find game directory"))
                        }
                    };
                }

                self.delete_in(game_path)
            }

            VoicePackage::NotInstalled { game_path, .. } => {
                match game_path {
                    Some(game_path) => self.delete_in(game_path),
                    None => {
                        tracing::error!("Failed to find game directory");

                        return Err(anyhow::anyhow!("Failed to find game directory"))
                    }
                }
            }
        }
    }

    #[tracing::instrument(level = "debug", ret)]
    /// Try to delete voice package from specific game directory
    ///
    /// FIXME:
    /// ⚠️ May fail on Chinese version due to paths differences
    pub fn delete_in<T: Into<PathBuf> + std::fmt::Debug>(&self, game_path: T) -> anyhow::Result<()> {
        let game_path = game_path.into();
        let locale = self.locale();

        tracing::debug!("Deleting {} voice package", locale.to_code());

        // Audio_<locale folder>_pkg_version
        std::fs::remove_dir_all(get_voice_package_path(&game_path, self.game_edition(), locale))?;
        std::fs::remove_file(game_path.join(format!("Audio_{}_pkg_version", locale.to_folder())))?;

        Ok(())
    }

    #[cfg(feature = "install")]
    #[tracing::instrument(level = "debug", ret)]
    pub fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        tracing::debug!("Trying to find version diff for {} voice package", self.locale().to_code());

        let game_edition = self.game_edition();
        let response = api::request(game_edition)?;

        if self.is_installed() {
            let current = self.try_get_version()?;

            if response.main.major.version == current {
                tracing::debug!("Package version is latest");

                // If we're running latest game version the diff we need to download
                // must always be `predownload.diffs[0]`, but just to be safe I made
                // a loop through possible variants, and if none of them was correct
                // (which is not possible in reality) we should just say thath the game
                // is latest
                if let Some(predownload_info) = response.pre_download {
                    if let Some(predownload_major) = predownload_info.major {
                        for diff in predownload_info.patches {
                            if diff.version == current {
                                let diff = find_voice_pack(diff.audio_pkgs, self.locale());

                                return Ok(VersionDiff::Predownload {
                                    current,
                                    latest: Version::from_str(predownload_major.version).unwrap(),
                                    uri: diff.url,

                                    downloaded_size: diff.size.parse::<u64>().unwrap(),
                                    unpacked_size: diff.decompressed_size.parse::<u64>().unwrap(),

                                    installation_path: match self {
                                        VoicePackage::Installed { .. } => None,
                                        VoicePackage::NotInstalled { game_path, .. } => game_path.clone()
                                    },

                                    version_file_path: match self {
                                        VoicePackage::Installed { path, .. } => Some(path.join(".version")),
                                        VoicePackage::NotInstalled { game_path, .. } => {
                                            match game_path {
                                                Some(game_path) => Some(get_voice_package_path(game_path, game_edition, self.locale()).join(".version")),
                                                None => None
                                            }
                                        }
                                    },

                                    temp_folder: None,
                                    edition: game_edition
                                })
                            }
                        }
                    }
                }

                Ok(VersionDiff::Latest {
                    version: current,
                    edition: game_edition
                })
            }

            else {
                tracing::debug!("Package is outdated: {} -> {}", current, response.main.major.version);

                for diff in response.main.patches {
                    if diff.version == current {
                        let diff = find_voice_pack(diff.audio_pkgs, self.locale());

                        return Ok(VersionDiff::Diff {
                            current,
                            latest: Version::from_str(response.main.major.version).unwrap(),
                            uri: diff.url,

                            downloaded_size: diff.size.parse::<u64>().unwrap(),
                            unpacked_size: diff.decompressed_size.parse::<u64>().unwrap(),

                            installation_path: match self {
                                VoicePackage::Installed { .. } => None,
                                VoicePackage::NotInstalled { game_path, .. } => game_path.clone()
                            },

                            version_file_path: match self {
                                VoicePackage::Installed { path, .. } => Some(path.join(".version")),
                                VoicePackage::NotInstalled { game_path, .. } => {
                                    match game_path {
                                        Some(game_path) => Some(get_voice_package_path(game_path, game_edition, self.locale()).join(".version")),
                                        None => None
                                    }
                                }
                            },

                            temp_folder: None,
                            edition: game_edition
                        })
                    }
                }

                Ok(VersionDiff::Outdated {
                    current,
                    latest: Version::from_str(response.main.major.version).unwrap(),
                    edition: game_edition
                })
            }
        }

        else {
            tracing::debug!("Package is not installed");

            let latest = find_voice_pack(response.main.major.audio_pkgs, self.locale());

            Ok(VersionDiff::NotInstalled {
                latest: Version::from_str(response.main.major.version).unwrap(),
                segments_uris: vec![latest.url],

                downloaded_size: latest.size.parse::<u64>().unwrap(),
                unpacked_size: latest.decompressed_size.parse::<u64>().unwrap(),

                installation_path: match self {
                    VoicePackage::Installed { .. } => None,
                    VoicePackage::NotInstalled { game_path, .. } => game_path.clone()
                },

                version_file_path: match self {
                    VoicePackage::Installed { path, .. } => Some(path.join(".version")),
                    VoicePackage::NotInstalled { game_path, .. } => {
                        match game_path {
                            Some(game_path) => Some(get_voice_package_path(game_path, game_edition, self.locale()).join(".version")),
                            None => None
                        }
                    }
                },

                temp_folder: None,
                edition: game_edition
            })
        }
    }
}
