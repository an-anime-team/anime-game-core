#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceLocales {
    Chinese,
    English,
    Japanese,
    Korean
}

impl VoiceLocales {
    pub fn to_code(&self) -> &str {
        match self {
            VoiceLocales::Chinese => "zh-cn",
            VoiceLocales::English => "en-us",
            VoiceLocales::Japanese => "ja-jp",
            VoiceLocales::Korean => "ko-kr"
        }
    }

    pub fn to_name(&self) -> &str {
        match self {
            VoiceLocales::Chinese => "Chinese",
            VoiceLocales::English => "English",
            VoiceLocales::Japanese => "Japanese",
            VoiceLocales::Korean => "Korean"
        }
    }

    pub fn from_str(locale: &str) -> Option<VoiceLocales> {
        match locale {
            "zh-cn" => Some(VoiceLocales::Chinese),
            "en-us" => Some(VoiceLocales::English),
            "ja-jp" => Some(VoiceLocales::Japanese),
            "ko-kr" => Some(VoiceLocales::Korean),

            "Chinese" => Some(VoiceLocales::Chinese),
            "English" => Some(VoiceLocales::English),
            "Japanese" => Some(VoiceLocales::Japanese),
            "Korean" => Some(VoiceLocales::Korean),

            _ => None
        }
    }
}
