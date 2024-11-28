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
            GameEdition::China => concat!("https://hyp-api.", "mih", "oyo", ".com/hyp/hyp-connect/api/getGamePackages?launcher_id=jGHBHlcOq1"),
            _ => concat!("https://sg-hyp-api.", "ho", "yo", "verse", ".com/hyp/hyp-connect/api/getGamePackages?launcher_id=VYTpXlbWo8")
        }
    }

    pub fn api_game_id(&self) -> &str {
        // 5TIVvvcwtM  glb_official
        // g0mMIvshDb  jp_official
        // uxB4MC7nzC  kr_official
        // bxPTXSET5t  overseas_official
        // wkE5P5WsIf  asia_official
        match self {
            Self::Global => "5TIVvvcwtM",
            Self::Sea    => "bxPTXSET5t", // Nut sure
            Self::China  => "osvnlOc0S8",
            Self::Taiwan => "wkE5P5WsIf", // Nut sure
            Self::Korea  => "uxB4MC7nzC",
            Self::Japan  => "g0mMIvshDb"
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
