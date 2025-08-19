use std::path::{Path, PathBuf};

use fs_extra::dir::get_size;

use crate::version::Version;
use crate::sophon::api_schemas::sophon_diff::SophonDiff;
use crate::sophon::api_schemas::sophon_manifests::SophonDownloadInfo;
use crate::sophon;
use crate::genshin::consts::*;
use crate::genshin::voice_data::locale::VoiceLocale;
#[cfg(feature = "install")]
use crate::genshin::version_diff::*;

/// List of voiceover sizes
///
/// Format: `(version, english, japanese, korean, chinese)`
pub const VOICE_PACKAGES_SIZES: &[(&str, u64, u64, u64, u64)] = &[
    //         English(US)   Japanese      Korean        Chinese
    ("5.7.0",  19512988499,  22222747101,  16908205123,  17127113750),
    ("5.6.0",  18981328917,  21627518083,  16446002370,  16665257451),
    ("5.5.0",  18412510172,  20933946124,  15886079840,  16128960332),

    // Size changed back and forth so I decided to comment old records.

    // ("5.1.0",  16207071335,  18254192787,  13784144703,  14055732779),
    // ("5.0.0",  18503031452,  20808521048,  15590542644,  15865413012),
    // ("4.8.0",  17809995945,  20012816885,  14991865472,  15260781792), // Predicted
    // ("4.7.0",  17116960439,  19217112723,  14393188301,  14656150572), // For whatever reason, who would have known,
    //                                                                    // those values are from the `size` field instead of `decompressed_size`
    //                                                                    // from the game API because later one looked COMPLETELY WRONG
    // ("4.6.0",  16414267279,  18393435927,  13796034335,  14036168895),
    // ("4.5.0",  15959234252,  17912625028,  13434833996,  13671635640),
    // ("4.4.0",  15719789566,  17526530996,  13139301525,  13399060711), // Predicted
    // ("4.3.0",  15067231819,  16799654823,  12604647523,  12861374519),
    // ("4.2.0",  14569503723,  16263036031,  12221968655,  12476776215),
    // ("4.1.0",  13889855947,  15500986871,  11635183963,  11885602119),
    // ("4.0.0",  13109710863,  14592012075,  10979621411,  11224640167),
    // ("3.8.0",  12220820203,  13571842139,  10221829179,  10441921175),
    // ("3.7.0",  11778060451,  13044149443,  9857960459,   10075853323),
    // ("3.6.0",  11041879555,  12412351703,  9434697975,   9626464559),
    // ("3.5.0",  10352166715,  11641949699,  8861959147,   9062163032),
    // ("3.4.0",  9702104595,   10879201351,  8329592851,   8498622343),
    // ("3.3.0",  9183929971,   10250403911,  7896362859,   8047012675),
    // ("3.2.0",  8636001252,   9600770928,   7416414724,   7563358032)
];

/// Acceptable error to select a version for the voiceover folder
pub const VOICE_PACKAGE_THRESHOLD: u64 = 400 * 1024 * 1024; // 400 MB, ~4 files

/// Get specific voice package sizes from `VOICE_PACKAGES_SIZES` constant
pub fn get_voice_pack_sizes<'a>(locale: VoiceLocale) -> Vec<(&'a str, u64)> {
    VOICE_PACKAGES_SIZES
        .iter()
        .map(|item| match locale {
            VoiceLocale::English => (item.0, item.1),
            VoiceLocale::Japanese => (item.0, item.2),
            VoiceLocale::Korean => (item.0, item.3),
            VoiceLocale::Chinese => (item.0, item.4)
        })
        .collect()
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

/// Predict new voice package size using WMA based on `VOICE_PACKAGES_SIZES`
/// constant
pub fn predict_new_voice_pack_size(locale: VoiceLocale) -> u64 {
    wma_predict(
        &get_voice_pack_sizes(locale)
            .into_iter()
            .map(|item| item.1)
            .rev()
            .collect::<Vec<u64>>()
    )
}

/// Find voice package with specified locale from list of packages
fn find_voice_pack(list: &[SophonDownloadInfo], locale: VoiceLocale) -> SophonDownloadInfo {
    for pack in list {
        if pack.matching_field == locale.to_code() {
            return pack.clone();
        }
    }

    // We're sure that all possible voice packages are listed in VoiceLocale...
    // right?
    unreachable!();
}

/// Find voice package with specified locale from list of packages
fn find_voice_pack_diff(list: &[SophonDiff], locale: VoiceLocale) -> SophonDiff {
    for pack in list {
        if pack.matching_field == locale.to_code() {
            return pack.clone();
        }
    }

    // We're sure that all possible voice packages are listed in VoiceLocale...
    // right?
    unreachable!();
}

#[allow(clippy::large_enum_variant)]
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
        data: SophonDownloadInfo,
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

        if path.is_dir() {
            let name = path.file_name()?;

            return VoiceLocale::from_str(name.to_string_lossy()).map(|locale| Self::Installed {
                path,
                locale,
                game_edition
            });
        }

        None
    }

    /// Get latest voice package with specified locale
    ///
    /// Note that returned object will be `VoicePackage::NotInstalled`, but
    /// technically it can be installed. This method just don't know the game's
    /// path
    pub fn with_locale(locale: VoiceLocale, game_edition: GameEdition) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::new();

        let game_branches = sophon::get_game_branches_info(&client, game_edition.into())?;

        let latest_version = game_branches
            .latest_version_by_id(game_edition.game_id())
            .ok_or_else(|| {
                anyhow::anyhow!("failed to find the latest game version")
                    .context(format!("game id: {}", game_edition.game_id()))
            })?;

        let game_branch_info = game_branches
            .get_game_by_id(game_edition.game_id(), latest_version)
            .ok_or_else(|| {
                anyhow::anyhow!("failed to get the game version information")
                    .context(format!("game id: {}", game_edition.game_id()))
                    .context(format!("game version: {latest_version}"))
            })?;

        let downloads_info = sophon::installer::get_game_download_sophon_info(
            &client,
            &game_branch_info.main,
            game_edition.into()
        )?;

        Ok(Self::NotInstalled {
            locale,
            version: latest_version,
            data: find_voice_pack(&downloads_info.manifests, locale),
            game_path: None,
            game_edition
        })
    }

    #[inline]
    pub fn game_edition(&self) -> GameEdition {
        match self {
            Self::Installed {
                game_edition, ..
            }
            | Self::NotInstalled {
                game_edition, ..
            } => *game_edition
        }
    }

    // TODO: find_in(game_path: String, locale: VoiceLocale)

    /// Get installation status of this package
    ///
    /// This method will return `false` if this package is
    /// `VoicePackage::NotInstalled` enum value
    ///
    /// If you want to check it's actually installed - you'd need to use
    /// `is_installed_in`
    #[inline]
    pub fn is_installed(&self) -> bool {
        match self {
            Self::Installed {
                ..
            } => true,
            Self::NotInstalled {
                ..
            } => false
        }
    }

    /// Calculate voice package size in bytes
    ///
    /// (unpacked size, Option(archive size))
    pub fn size(&self) -> (u64, Option<u64>) {
        match self {
            VoicePackage::Installed {
                path, ..
            } => (get_size(path).unwrap(), None),
            VoicePackage::NotInstalled {
                data, ..
            } => (
                data.stats.compressed_size.parse::<u64>().unwrap(),
                Some(data.stats.uncompressed_size.parse::<u64>().unwrap())
            )
        }
    }

    /// This method will return `true` if the package has
    /// `VoicePackage::Installed` enum value
    ///
    /// If it's `VoicePackage::NotInstalled`, then this method will check
    /// `game_path`'s voices folder
    #[inline]
    pub fn is_installed_in<T: AsRef<Path>>(&self, game_path: T) -> bool {
        match self {
            Self::Installed {
                ..
            } => true,
            Self::NotInstalled {
                locale, ..
            } => get_voice_package_path(game_path, self.game_edition(), *locale).exists()
        }
    }

    /// Get list of latest voice packages
    pub fn list_latest(game_edition: GameEdition) -> anyhow::Result<Vec<VoicePackage>> {
        let client = reqwest::blocking::Client::new();

        let game_branches = sophon::get_game_branches_info(&client, game_edition.into())?;

        let latest_version = game_branches
            .latest_version_by_id(game_edition.game_id())
            .ok_or_else(|| {
                anyhow::anyhow!("failed to find the latest game version")
                    .context(format!("game id: {}", game_edition.game_id()))
            })?;

        let branch_info = game_branches
            .get_game_by_id(game_edition.game_id(), latest_version)
            .ok_or_else(|| {
                anyhow::anyhow!("failed to get the game version information")
                    .context(format!("game id: {}", game_edition.game_id()))
                    .context(format!("game version: {latest_version}"))
            })?;

        let downloads_info = sophon::installer::get_game_download_sophon_info(
            &client,
            &branch_info.main,
            game_edition.into()
        )?;

        let mut packages = Vec::new();

        for package in &downloads_info.manifests {
            if let Some(locale) = VoiceLocale::from_str(&package.matching_field) {
                packages.push(Self::NotInstalled {
                    locale,
                    version: latest_version,
                    data: package.clone(),
                    game_path: None,
                    game_edition
                });
            }
        }

        Ok(packages)
    }

    /// Get voice package locale
    #[inline]
    pub fn locale(&self) -> VoiceLocale {
        match self {
            Self::Installed {
                locale, ..
            }
            | Self::NotInstalled {
                locale, ..
            } => *locale
        }
    }

    /// This method can fail to parse this package version.
    /// It also can mean that the corresponding folder doesn't
    /// contain voice package files
    #[tracing::instrument(level = "debug", ret)]
    pub fn try_get_version(&self) -> anyhow::Result<Version> {
        tracing::debug!(locale = ?self.locale(), "Trying to get voice package version");

        let client = reqwest::blocking::Client::new();

        match &self {
            Self::NotInstalled {
                version, ..
            } => Ok(*version),

            Self::Installed {
                path,
                locale,
                game_edition
            } => {
                match std::fs::read(path.join(".version")) {
                    Ok(curr) => {
                        tracing::debug!("Found .version file: {}.{}.{}", curr[0], curr[1], curr[2]);

                        Ok(Version::new(curr[0], curr[1], curr[2]))
                    }

                    // We don't create .version file here because we don't
                    // actually know current version and just predict it
                    // This file will be properly created in the install method
                    Err(_) => {
                        let package_size = get_size(path)?;

                        let game_branches =
                            sophon::get_game_branches_info(&client, (*game_edition).into())?;

                        let game_branch_info = game_branches
                            .get_game_latest_by_id(game_edition.game_id())
                            .expect("Latest version should be available");

                        tracing::debug!(
                            package_size,
                            ".version file wasn't found, predicting voiceover version"
                        );

                        let mut voice_packages_sizes = get_voice_pack_sizes(*locale);

                        // Get the latest game version's voice pack sizes from the API
                        if !voice_packages_sizes
                            .iter()
                            .any(|(tag, _)| *tag == game_branch_info.main.tag)
                        {
                            let locale = locale.to_code();

                            let download_info = sophon::installer::get_game_download_sophon_info(
                                &client,
                                &game_branch_info.main,
                                (*game_edition).into()
                            )?;

                            let info = download_info
                                .manifests
                                .iter()
                                .find(|download_info| download_info.matching_field == locale);

                            if let Some(api_size) = info {
                                let size = api_size.stats.uncompressed_size.parse()?;

                                // Inserting at `0` so that it gets picked up at the next bit of
                                // code, doesn't have to predict.
                                voice_packages_sizes.insert(0, (&game_branch_info.main.tag, size));
                            }
                        }

                        // If latest voice packages sizes aren't listed in `VOICE_PACKAGES_SIZES`
                        // then we should predict their sizes
                        if VOICE_PACKAGES_SIZES[0].0 != game_branch_info.main.tag {
                            let mut t = voice_packages_sizes;

                            voice_packages_sizes = vec![(
                                &game_branch_info.main.tag,
                                predict_new_voice_pack_size(*locale)
                            )];
                            voice_packages_sizes.append(&mut t);
                        }

                        // To predict voice package version we're going through saved voice packages
                        // sizes in the `VOICE_PACKAGES_SIZES` constant plus
                        // predicted voice packages sizes if needed. The version with closest folder
                        // size is version we have installed
                        for (version, size) in voice_packages_sizes {
                            if package_size > size - VOICE_PACKAGE_THRESHOLD {
                                tracing::debug!("Predicted version: {version}");

                                return Ok(Version::from_str(version).unwrap());
                            }
                        }

                        anyhow::bail!("Failed to determine installed voice package version")
                    }
                }
            }
        }
    }

    /// Try to delete voice package
    ///
    /// FIXME:
    /// ⚠️ May fail on Chinese version due to paths differences
    #[tracing::instrument(level = "trace", ret)]
    pub fn delete(&self) -> anyhow::Result<()> {
        tracing::trace!(locale = ?self.locale(), "Deleting voice package");

        match self {
            VoicePackage::Installed {
                path, ..
            } => {
                let mut game_path = path.clone();

                for _ in 0..4 {
                    game_path = match game_path.parent() {
                        Some(game_path) => game_path.into(),

                        None => {
                            tracing::error!("Failed to find game directory");

                            anyhow::bail!("Failed to find game directory");
                        }
                    };
                }

                self.delete_in(game_path)?;
            }

            VoicePackage::NotInstalled {
                game_path, ..
            } => match game_path {
                Some(game_path) => self.delete_in(game_path)?,

                None => {
                    tracing::error!("Failed to find game directory");

                    anyhow::bail!("Failed to find game directory");
                }
            }
        }

        Ok(())
    }

    #[tracing::instrument(level = "debug", ret)]
    /// Try to delete voice package from specific game directory
    ///
    /// FIXME:
    /// ⚠️ May fail on Chinese version due to paths differences
    pub fn delete_in<T: Into<PathBuf> + std::fmt::Debug>(
        &self,
        game_path: T
    ) -> anyhow::Result<()> {
        let game_path = game_path.into();
        let locale = self.locale();

        tracing::debug!(?locale, "Deleting voice package");

        // Audio_<locale folder>_pkg_version
        std::fs::remove_dir_all(get_voice_package_path(
            &game_path,
            self.game_edition(),
            locale
        ))?;
        std::fs::remove_file(game_path.join(format!("Audio_{}_pkg_version", locale.to_folder())))?;

        Ok(())
    }

    #[cfg(feature = "install")]
    #[tracing::instrument(level = "debug", ret)]
    pub fn try_get_diff(&self) -> anyhow::Result<VersionDiff> {
        tracing::debug!(
            "Trying to find version diff for {} voice package",
            self.locale().to_code()
        );

        let game_edition = self.game_edition();

        let client = reqwest::blocking::Client::new();

        let game_branches = sophon::get_game_branches_info(&client, game_edition.into())?;

        let latest_version = game_branches
            .latest_version_by_id(game_edition.game_id())
            .ok_or_else(|| {
                anyhow::anyhow!("failed to find the latest game version")
                    .context(format!("game id: {}", game_edition.game_id()))
            })?;

        let branch_info = game_branches
            .get_game_by_id(game_edition.game_id(), latest_version)
            .ok_or_else(|| {
                anyhow::anyhow!("failed to get the game version information")
                    .context(format!("game id: {}", game_edition.game_id()))
                    .context(format!("game version: {latest_version}"))
            })?;

        let downloads_info = sophon::installer::get_game_download_sophon_info(
            &client,
            &branch_info.main,
            game_edition.into()
        )?;

        if self.is_installed() {
            let current = self.try_get_version()?;

            if latest_version == current {
                tracing::debug!("Package version is latest");

                // If we're running latest game version the diff we need to download
                // must always be `predownload.diffs[0]`, but just to be safe I made
                // a loop through possible variants, and if none of them was correct
                // (which is not possible in reality) we should just say thath the game
                // is latest
                if let Some(predownload_info) = &branch_info.pre_download {
                    if predownload_info
                        .diff_tags
                        .iter()
                        .any(|pre_ver| *pre_ver == current)
                    {
                        let game_patches = sophon::updater::get_game_diffs_sophon_info(
                            &client,
                            predownload_info,
                            game_edition.into()
                        )?;

                        let diff = find_voice_pack_diff(&game_patches.manifests, self.locale());

                        let stats = diff
                            .stats
                            .get(&current.to_string())
                            .ok_or_else(|| anyhow::anyhow!("failed to get voiceover diff stats"))?;

                        let predownload_version = predownload_info.version().ok_or_else(|| {
                            anyhow::anyhow!("failed to get predownload game version")
                        })?;

                        return Ok(VersionDiff::Predownload {
                            current,
                            latest: predownload_version,

                            downloaded_size: stats.compressed_size.parse()?,
                            unpacked_size: stats.uncompressed_size.parse()?,

                            download_info: sophon::api_schemas::DownloadOrDiff::Patch(diff),

                            installation_path: match self {
                                VoicePackage::Installed {
                                    ..
                                } => None,
                                VoicePackage::NotInstalled {
                                    game_path, ..
                                } => game_path.clone()
                            },

                            version_file_path: match self {
                                VoicePackage::Installed {
                                    path, ..
                                } => Some(path.join(".version")),

                                VoicePackage::NotInstalled {
                                    game_path, ..
                                } => game_path.as_ref().map(|game_path| {
                                    get_voice_package_path(game_path, game_edition, self.locale())
                                        .join(".version")
                                })
                            },

                            temp_folder: None,
                            edition: game_edition
                        });
                    }
                }

                Ok(VersionDiff::Latest {
                    version: current,
                    edition: game_edition
                })
            }
            else {
                tracing::debug!(
                    current_version = current.to_string(),
                    latest_version = latest_version.to_string(),
                    "Voice package is outdated"
                );

                if branch_info.main.diff_tags.iter().any(|tag| *tag == current) {
                    let game_patches = sophon::updater::get_game_diffs_sophon_info(
                        &client,
                        &branch_info.main,
                        game_edition.into()
                    )?;

                    let diff = find_voice_pack_diff(&game_patches.manifests, self.locale());

                    let current_ver_patch_stats = diff
                        .stats
                        .get(&current.to_string())
                        .ok_or_else(|| anyhow::anyhow!("failed to get voiceover diff stats"))?;

                    return Ok(VersionDiff::Diff {
                        current,
                        latest: latest_version,

                        downloaded_size: current_ver_patch_stats.compressed_size.parse()?,
                        unpacked_size: current_ver_patch_stats.uncompressed_size.parse()?,
                        diff,

                        installation_path: match self {
                            VoicePackage::Installed {
                                ..
                            } => None,
                            VoicePackage::NotInstalled {
                                game_path, ..
                            } => game_path.clone()
                        },

                        version_file_path: match self {
                            VoicePackage::Installed {
                                path, ..
                            } => Some(path.join(".version")),

                            VoicePackage::NotInstalled {
                                game_path, ..
                            } => game_path.as_ref().map(|game_path| {
                                get_voice_package_path(game_path, game_edition, self.locale())
                                    .join(".version")
                            })
                        },

                        temp_folder: None,
                        edition: game_edition
                    });
                }

                Ok(VersionDiff::Outdated {
                    current,
                    latest: latest_version,
                    edition: game_edition
                })
            }
        }
        else {
            tracing::debug!("Package is not installed");

            let latest = find_voice_pack(&downloads_info.manifests, self.locale());

            Ok(VersionDiff::NotInstalled {
                latest: latest_version,

                downloaded_size: latest.stats.compressed_size.parse()?,
                unpacked_size: latest.stats.uncompressed_size.parse()?,
                download_info: latest,

                installation_path: match self {
                    VoicePackage::Installed {
                        ..
                    } => None,
                    VoicePackage::NotInstalled {
                        game_path, ..
                    } => game_path.clone()
                },

                version_file_path: match self {
                    VoicePackage::Installed {
                        path, ..
                    } => Some(path.join(".version")),

                    VoicePackage::NotInstalled {
                        game_path, ..
                    } => game_path.as_ref().map(|game_path| {
                        get_voice_package_path(game_path, game_edition, self.locale())
                            .join(".version")
                    })
                },

                temp_folder: None,
                edition: game_edition
            })
        }
    }
}
