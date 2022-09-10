use serde::Deserialize;

// In theory this can not contain data field
// and has some actual error, but I never had it in practice

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Response {
    pub retcode: u16,
    pub message: String,
    pub data: Data
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    pub web_url: String,
    pub game: Game,
    pub pre_download_game: Option<Latest>,

    // We're not talking about it here

    // pub deprecated_packages,
    // pub plugin: Plugin,
    // pub force_update,
    // pub sdk
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Game {
    pub latest: Latest,
    pub diffs: Vec<Diff>
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
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

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VoicePack {
    pub language: String,
    pub name: String,
    pub path: String,
    pub size: String,
    pub md5: String,
    pub package_size: String
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
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
