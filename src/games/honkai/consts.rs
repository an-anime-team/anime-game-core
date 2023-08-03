use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameEdition {
    Global,
    Sea,
    China,
    Taiwan,
    Korea,
    Japan
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
        &[
            Self::Global,
            Self::Sea,
            Self::China,
            Self::Taiwan,
            Self::Korea,
            Self::Japan
        ]
    }

    #[inline]
    pub fn api_uri(&self) -> &str {
        match self {
            GameEdition::Global => concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/bh3_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10"),
            GameEdition::Sea    => concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/bh3_global/mdk/launcher/api/resource?launcher_id=9"),
            GameEdition::China  => concat!("https://sdk-static.", "mi", "ho", "yo" ,".com/bh3_cn/mdk/launcher/api/resource?launcher_id=4"),
            GameEdition::Taiwan => concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/bh3_global/mdk/launcher/api/resource?launcher_id=8"),
            GameEdition::Korea  => concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/bh3_global/mdk/launcher/api/resource?launcher_id=11"),
            GameEdition::Japan  => concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/bh3_global/mdk/launcher/api/resource?key=ojevZ0EyIyZNCy4n&launcher_id=19")
        }
    }

    #[inline]
    pub fn data_folder(&self) -> &str {
        "BH3_Data"
    }

    #[inline]
    pub fn telemetry_servers(&self) -> &[&str] {
        match self {
            Self::China => &[
                concat!("log-upload.m", "iho", "yo.com"),
                concat!("public-data-api.m", "iho", "yo.com"),
                concat!("dump.gam", "esafe.q", "q.com")
            ],

            _ => &[
                concat!("log-upload-os.ho", "yov", "erse.com"),
                concat!("sg-public-data-api.ho", "yov", "erse.com"),
                concat!("dump.gam", "esafe.q", "q.com")
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
        }

        else if locale.starts_with("zh_tw") {
            Self::Taiwan
        }

        else if locale.starts_with("ja") {
            Self::Japan
        }

        else if locale.starts_with("ko") {
            Self::Korea
        }

        else {
            Self::Global
        }
    }
}

