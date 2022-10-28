#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceLocale {
    English,
    Japanese,
    Korean,
    Chinese
}

impl VoiceLocale {
    pub fn list() -> &'static [VoiceLocale] {
        &[
            Self::English,
            Self::Japanese,
            Self::Korean,
            Self::Chinese
        ]
    }

    /// Convert enum value to its name
    /// 
    /// `VoiceLocale::English` -> `English`
    pub fn to_name(&self) -> &str {
        match self {
            VoiceLocale::English  => "English",
            VoiceLocale::Japanese => "Japanese",
            VoiceLocale::Korean   => "Korean",
            VoiceLocale::Chinese  => "Chinese"
        }
    }

    /// Convert enum value to its code
    /// 
    /// `VoiceLocale::English` -> `en-us`
    pub fn to_code(&self) -> &str {
        match self {
            VoiceLocale::English  => "en-us",
            VoiceLocale::Japanese => "ja-jp",
            VoiceLocale::Korean   => "ko-kr",
            VoiceLocale::Chinese  => "zh-cn"
        }
    }

    /// Convert enum value to its folder name
    /// 
    /// `VoiceLocale::English` -> `English(US)`
    pub fn to_folder(&self) -> &str {
        match self {
            VoiceLocale::English  => "English(US)",
            VoiceLocale::Japanese => "Japanese",
            VoiceLocale::Korean   => "Korean",
            VoiceLocale::Chinese  => "Chinese"
        }
    }

    /// Try to convert string to enum
    /// 
    /// - `English` -> `VoiceLocale::English`
    /// - `English(US)` -> `VoiceLocale::English`
    /// - `en-us` -> `VoiceLocale::English`
    pub fn from_str<T: ToString>(str: T) -> Option<Self> {
        match str.to_string().as_str() {
            // Locales names
            "English"  => Some(VoiceLocale::English),
            "Japanese" => Some(VoiceLocale::Japanese),
            "Korean"   => Some(VoiceLocale::Korean),
            "Chinese"  => Some(VoiceLocale::Chinese),

            // Lowercased variants
            "english"  => Some(VoiceLocale::English),
            "japanese" => Some(VoiceLocale::Japanese),
            "korean"   => Some(VoiceLocale::Korean),
            "chinese"  => Some(VoiceLocale::Chinese),

            // Folders
            "English(US)" => Some(VoiceLocale::English),

            // Codes
            "en-us" => Some(VoiceLocale::English),
            "ja-jp" => Some(VoiceLocale::Japanese),
            "ko-kr" => Some(VoiceLocale::Korean),
            "zh-cn" => Some(VoiceLocale::Chinese),

            _ => None
        }
    }
}
