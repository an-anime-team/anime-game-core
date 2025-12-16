use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};

use super::voice_data::locale::VoiceLocale;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameEdition {
    Global,
    China
}

impl Default for GameEdition {
    #[inline]
    fn default() -> Self {
        Self::Global
    }
}

impl GameEdition {
    #[inline]
    pub fn list() -> &'static [GameEdition] {
        &[Self::Global, Self::China]
    }

    #[inline]
    pub fn api_uri(&self) -> &str {
        match self {
            GameEdition::Global => concat!("https://sg-hyp-api.", "ho", "yo", "verse", ".com/hyp/hyp-connect/api/getGamePackages?launcher_id=VYTpXlbWo8"),
            GameEdition::China  => concat!("https://hyp-api.", "mih", "oyo", ".com/hyp/hyp-connect/api/getGamePackages?launcher_id=jGHBHlcOq1")
        }
    }

    #[inline]
    pub fn data_folder(&self) -> &str {
        // Same data folder name for every region
        concat!("Sta", "rRai", "l_Data")
    }

    /// API IDs used by Sophon
    #[inline]
    pub fn api_game_id(&self) -> &str {
        match self {
            Self::Global => "4ziysqXOQ8",
            Self::China => "64kMb5iAWu"
        }
    }

    #[inline]
    pub fn telemetry_servers(&self) -> &[&str] {
        match self {
            GameEdition::Global => &[
                concat!("log-upload-os.ho", "yo", "ver", "se.com"),
                concat!("sg-public-data-api.ho", "yo", "ver", "se.com"),
                concat!("hkrpg-log-upload-os.ho", "yo", "ver", "se.com")
            ],

            GameEdition::China => &[
                concat!("log-upload.m", "iho", "yo.com"),
                concat!("public-data-api.m", "iho", "yo.com")
            ]
        }
    }

    pub fn from_system_lang() -> Self {
        let locale = std::env::var("LC_ALL")
            .unwrap_or_else(|_| std::env::var("LC_MESSAGES")
            .unwrap_or_else(|_| std::env::var("LANG")
            .unwrap_or(String::from("en_us"))))
            .to_ascii_lowercase();

        if locale.starts_with("zh_cn") {
            Self::China
        } else {
            Self::Global
        }
    }
}

#[inline]
pub fn get_voice_packages_path<T: AsRef<Path>>(game_path: T, game_edition: GameEdition) -> PathBuf {
    game_path.as_ref()
        .join(game_edition.data_folder())
        .join("Persistent/Audio/AudioPackage/Windows")
}

#[inline]
pub fn get_voice_package_path<T: AsRef<Path>>(game_path: T, game_edition: GameEdition, locale: VoiceLocale) -> PathBuf {
    get_voice_packages_path(game_path, game_edition).join(locale.to_folder())
}
