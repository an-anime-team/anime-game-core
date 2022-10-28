use std::path::PathBuf;

use super::voice_data::locale::VoiceLocale;

// This enum is used in `Game::get_edition` method
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameEdition {
    Global,
    China
}

impl Default for GameEdition {
    fn default() -> Self {
        Self::Global
    }
}

pub static mut GAME_EDITION: GameEdition = GameEdition::Global;

pub static mut API_URI: &str = concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/hk4e_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10");

/// Name of the game's data folder
pub static mut DATA_FOLDER_NAME: &str = concat!("Ge", "nsh", "inIm", "pact_Data");

#[cfg(feature = "telemetry")]
pub static mut TELEMETRY_SERVERS: &[&str] = &[
    concat!("log-upload-os.", "ho", "yo", "verse", ".com"),
    concat!("overseauspider.", "yu", "ans", "hen", ".com")
];

pub fn get_api_uri(edition: GameEdition) -> &'static str {
    match edition {
        GameEdition::Global => concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/hk4e_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10"),
        GameEdition::China  => concat!("https://sdk-static.", "mih", "oyo", ".com/hk4e_cn/mdk/launcher/api/resource?key=eYd89JmJ&launcher_id=18")
    }
}

pub fn get_data_folder_name(edition: GameEdition) -> &'static str {
    match edition {
        GameEdition::Global => concat!("Ge", "nsh", "inIm", "pact_Data"),
        GameEdition::China  => concat!("Yu", "anS", "hen", "_Data")
    }
}

#[cfg(feature = "telemetry")]
pub fn get_telemetry_servers(edition: GameEdition) -> &'static [&'static str] {
    match edition {
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

/// Set the game edition
/// 
/// Updates all related constants from this mod
pub fn set_game_edition(edition: GameEdition) {
    unsafe {
        GAME_EDITION = edition;
        API_URI = get_api_uri(edition);
        DATA_FOLDER_NAME = get_data_folder_name(edition);

        if cfg!(feature = "telemetry") {
            TELEMETRY_SERVERS = get_telemetry_servers(edition);
        }
    }
}

pub trait ToFolder {
    fn to_folder(&self) -> String;
}

impl<T: ToString> ToFolder for T {
    fn to_folder(&self) -> String {
        self.to_string()
    }
}

impl ToFolder for VoiceLocale {
    fn to_folder(&self) -> String {
        self.to_folder().to_string()
    }
}

pub fn get_voice_packages_path<T: Into<PathBuf>>(game_path: T) -> PathBuf {
    game_path
        .into()
        .join(unsafe { DATA_FOLDER_NAME })
        .join("StreamingAssets/Audio/GeneratedSoundBanks/Windows")
}

pub fn get_voice_package_path<T: Into<PathBuf>, F: ToFolder>(game_path: T, locale: F) -> PathBuf {
    get_voice_packages_path(game_path).join(locale.to_folder())
}
