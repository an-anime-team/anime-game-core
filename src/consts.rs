use super::voice_data::locale::VoiceLocale;

// TODO: encode these strings to something

// This enum is used in `Game::get_edition` method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub static mut API_URI: &str = "https://sdk-os-static.hoyoverse.com/hk4e_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10";

/// Name of the game's data folder
pub static mut DATA_FOLDER_NAME: &str = "GenshinImpact_Data";

#[cfg(feature = "telemetry")]
pub static mut TELEMETRY_SERVERS: &[&str] = &[
    "log-upload-os.hoyoverse.com",
    "overseauspider.yuanshen.com"
];

pub fn get_api_uri(edition: GameEdition) -> &'static str {
    match edition {
        GameEdition::Global => "https://sdk-os-static.hoyoverse.com/hk4e_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10",
        GameEdition::China  => "https://sdk-static.mihoyo.com/hk4e_cn/mdk/launcher/api/resource?key=eYd89JmJ&launcher_id=18"
    }
}

pub fn get_data_folder_name(edition: GameEdition) -> &'static str {
    match edition {
        GameEdition::Global => "GenshinImpact_Data",
        GameEdition::China  => "YanShen_Data"
    }
}

#[cfg(feature = "telemetry")]
pub fn get_telemetry_servers(edition: GameEdition) -> &'static [&'static str] {
    match edition {
        GameEdition::Global => &[
            "log-upload-os.hoyoverse.com",
            "overseauspider.yuanshen.com"
        ],
        GameEdition::China => &[
            "log-upload.mihoyo.com",
            "uspider.yuanshen.com"
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

pub fn get_voice_packages_path<T: ToString>(game_path: T) -> String {
    let data_folder = unsafe { DATA_FOLDER_NAME };

    format!("{}/{data_folder}/StreamingAssets/Audio/GeneratedSoundBanks/Windows", game_path.to_string())
}

pub fn get_voice_package_path<T: ToString, F: ToFolder>(game_path: T, locale: F) -> String {
    let data_folder = unsafe { DATA_FOLDER_NAME };

    format!("{}/{data_folder}/StreamingAssets/Audio/GeneratedSoundBanks/Windows/{}", game_path.to_string(), locale.to_folder())
}
