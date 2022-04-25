use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub retcode: u16,
    pub message: String,
    pub data: Data
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Data {
    pub web_url: String,
    pub game: Game,

    // pub deprecated_packages,
    // pub plugin: Plugin,
    // pub force_update,
    // pub pre_download_game,
    // pub sdk
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub latest: Latest,
    pub diffs: Vec<Diff>
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Latest {
    pub name: String,
    pub version: String,
    pub path: String,
    pub size: String,
    pub md5: String,
    pub entry: String,
    pub package_size: String,
    pub decompressed_path: String,
    pub voice_packs: Vec<VoicePack>,

    // pub segments
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct VoicePack {
    pub language: String,
    pub name: String,
    pub path: String,
    pub size: String,
    pub md5: String,
    pub package_size: String
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    pub name: String,
    pub version: String,
    pub path: String,
    pub size: String,
    pub md5: String,
    pub is_recommended_update: bool,
    pub package_size: String,
    pub voice_packs: Vec<VoicePack>
}
