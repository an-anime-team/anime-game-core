use serde::{Serialize, Deserialize};

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
            GameEdition::Global => concat!("https://hk", "rpg-launcher-static.ho", "yov", "erse.com/hk", "rpg_global/mdk/launcher/api/resource?channel_id=1&key=vplOVX8Vn7cwG8yb&launcher_id=35"),
            GameEdition::China  => concat!("https://api-launcher.m", "ih", "oy", "o.com/hk", "rpg_cn/mdk/launcher/api/resource?channel_id=1&key=6KcVuOkbcqjJomjZ&launcher_id=33")
        }
    }

    #[inline]
    pub fn data_folder(&self) -> &str {
        match self {
            GameEdition::Global => concat!("Sta", "rRai", "l_Data"),

            // FIXME update CN version's data folder name
            GameEdition::China  => concat!("Sta", "rRai", "l_Data")
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
