use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};

use super::voice_data::locale::VoiceLocale;

static mut GAME_EDITION: GameEdition = GameEdition::Global;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn selected() -> Self {
        unsafe {
            GAME_EDITION
        }
    }

    #[inline]
    pub fn select(self) {
        unsafe {
            GAME_EDITION = self;
        }
    }

    #[inline]
    pub fn api_uri(&self) -> &str {
        match self {
            GameEdition::Global => concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/hk4e_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10"),
            GameEdition::China  => concat!("https://sdk-static.", "mih", "oyo", ".com/hk4e_cn/mdk/launcher/api/resource?key=eYd89JmJ&launcher_id=18")
        }
    }

    #[inline]
    pub fn data_folder(&self) -> &str {
        match self {
            GameEdition::Global => concat!("Ge", "nsh", "inIm", "pact_Data"),
            GameEdition::China  => concat!("Yu", "anS", "hen", "_Data")
        }
    }

    #[inline]
    pub fn telemetry_servers(&self) -> &[&str] {
        match self {
            GameEdition::Global => &[
                concat!("log-upload-os.", "ho", "yo", "verse", ".com"),
                concat!("overseauspider.", "yu", "ans", "hen", ".com")
            ],
            GameEdition::China => &[
                concat!("log-upload.", "mih", "oyo", ".com"),
                concat!("uspider.", "yu", "ans", "hen", ".com")
            ]
        }
    }

    pub fn from_system_lang() -> Self {
        #[allow(clippy::or_fun_call)]
        let locale = std::env::var("LC_ALL")
            .unwrap_or_else(|_| std::env::var("LC_MESSAGES")
            .unwrap_or_else(|_| std::env::var("LANG")
            .unwrap_or(String::from("en_us"))));

        if locale.len() > 4 && &locale[..5].to_ascii_lowercase() == "zh_cn" {
            Self::China
        } else {
            Self::Global
        }
    }
}

#[inline]
pub fn get_voice_packages_path<T: AsRef<Path>>(game_path: T) -> PathBuf {
    game_path.as_ref()
        .join(GameEdition::selected().data_folder())
        .join("StreamingAssets/AudioAssets")
}

#[inline]
pub fn get_voice_package_path<T: AsRef<Path>>(game_path: T, locale: VoiceLocale) -> PathBuf {
    get_voice_packages_path(game_path).join(locale.to_folder())
}
