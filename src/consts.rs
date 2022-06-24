use super::voice_data::locale::VoiceLocale;

// TODO: encode this string to something
pub const API_URI: &'static str = "https://sdk-os-static.hoyoverse.com/hk4e_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10";

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
    format!("{}/GenshinImpact_Data/StreamingAssets/Audio/GeneratedSoundBanks/Windows", game_path.to_string())
}

pub fn get_voice_package_path<T: ToString, F: ToFolder>(game_path: T, locale: F) -> String {
    format!("{}/GenshinImpact_Data/StreamingAssets/Audio/GeneratedSoundBanks/Windows/{}", game_path.to_string(), locale.to_folder())
}
