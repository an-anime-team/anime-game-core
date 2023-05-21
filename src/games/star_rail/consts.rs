use serde::{Serialize, Deserialize};

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
            GameEdition::Global => concat!("https://hk", "rpg-launcher-static.ho", "yov", "erse.com/hk", "rpg_global/mdk/launcher/api/resource?channel_id=1&key=vplOVX8Vn7cwG8yb&launcher_id=35"),
            GameEdition::China  => concat!("https://api-launcher.m", "ih", "oy", "o.com/hk", "rpg_cn/mdk/launcher/api/resource?channel_id=1&key=6KcVuOkbcqjJomjZ&launcher_id=33")
        }
    }

    #[inline]
    pub fn data_folder(&self) -> &str {
        // Same data folder name for every region
        concat!("Sta", "rRai", "l_Data")
    }

    #[inline]
    pub fn telemetry_servers(&self) -> &[&str] {
        match self {
            GameEdition::Global => &[
                concat!("log-upload-os.", "ho", "yo", "ver", "se.com"),
                concat!("sg-public-data-api.ho", "yo", "ver", "se.com")
            ],
            GameEdition::China => &[
                concat!("log-upload.m", "iho", "yo.com"),
                concat!("public-data-api.m", "iho", "yo.com")
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
