use serde::{Deserialize, Serialize};

/// Default amount of threads `VersionDiff` will use to download stuff
pub const DEFAULT_DOWNLOADER_THREADS: usize = 8;

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
            GameEdition::Global => concat!("https://prod-awscdn-gamestarter.ku", "rogame.net/pcstarter/prod/game/G143/4/index.json"),
            GameEdition::China  => concat!("https://prod-cn-alicdn-gamestarter.ku", "rogame.com/pcstarter/prod/game/G152/10003_Y8xXrXk65DqFHEDgApn3cpK5lfczpFx5/index.json")
        }
    }

    #[inline]
    pub fn cdn_uri(&self) -> &str {
        match self {
            GameEdition::Global => concat!("https://prod-awscdn-gamestarter.ku", "rogame.net"),
            GameEdition::China  => concat!("https://pcdownload-huoshan.aki-game.com")
        }
    }

    #[inline]
    pub fn telemetry_servers(&self) -> &[&str] {
        match self {
            GameEdition::Global => &[
                "pc.crashsight.wetest.net"
            ],
            GameEdition::China => &[
                "pc.crashsight.wetest.net"
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
        } else {
            Self::Global
        }
    }
}
