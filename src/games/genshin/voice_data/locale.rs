use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoiceLocale {
    English,
    Japanese,
    Korean,
    Chinese
}

impl VoiceLocale {
    #[inline]
    pub fn list() -> &'static [VoiceLocale] {
        &[Self::English, Self::Japanese, Self::Korean, Self::Chinese]
    }

    /// Convert enum value to its name
    ///
    /// `VoiceLocale::English` -> `English`
    #[inline]
    pub fn to_name(&self) -> &str {
        match self {
            Self::English  => "English",
            Self::Japanese => "Japanese",
            Self::Korean   => "Korean",
            Self::Chinese  => "Chinese"
        }
    }

    /// Convert enum value to its code
    ///
    /// `VoiceLocale::English` -> `en-us`
    #[inline]
    pub fn to_code(&self) -> &str {
        match self {
            Self::English  => "en-us",
            Self::Japanese => "ja-jp",
            Self::Korean   => "ko-kr",
            Self::Chinese  => "zh-cn"
        }
    }

    #[inline]
    /// Convert enum value to its folder name
    ///
    /// `VoiceLocale::English` -> `English(US)`
    pub fn to_folder(&self) -> &str {
        match self {
            Self::English  => "English(US)",
            Self::Japanese => "Japanese",
            Self::Korean   => "Korean",
            Self::Chinese  => "Chinese"
        }
    }

    #[inline]
    /// Try to convert string to enum
    ///
    /// - `English` -> `VoiceLocale::English`
    /// - `English(US)` -> `VoiceLocale::English`
    /// - `en-us` -> `VoiceLocale::English`
    pub fn from_str<T: AsRef<str>>(str: T) -> Option<Self> {
        match str.as_ref() {
            // Locales names
            "English"  => Some(Self::English),
            "Japanese" => Some(Self::Japanese),
            "Korean"   => Some(Self::Korean),
            "Chinese"  => Some(Self::Chinese),

            // Lowercased variants
            "english"  => Some(Self::English),
            "japanese" => Some(Self::Japanese),
            "korean"   => Some(Self::Korean),
            "chinese"  => Some(Self::Chinese),

            // Folders
            "English(US)" => Some(Self::English),

            // Codes
            "en-us" => Some(Self::English),
            "ja-jp" => Some(Self::Japanese),
            "ko-kr" => Some(Self::Korean),
            "zh-cn" => Some(Self::Chinese),

            _ => None
        }
    }
}
