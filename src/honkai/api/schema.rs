use serde::{Serialize, Deserialize};

// In theory this can not contain data field
// and has some actual error, but I never had it in practice

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Response {
    pub retcode: u16,
    pub message: String,
    pub data: Data
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Data {
    pub web_url: String,
    pub game: Game,

    // We're not talking about it here

    // pub pre_download_game,

    // pub deprecated_packages,
    // pub plugin: Plugin,
    // pub force_update,
    // pub sdk
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Game {
    pub latest: Latest,

    // Isn't used by the game?
    // pub diffs: Vec<Diff>
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Latest {
    pub name: String,
    pub version: String,
    pub path: String,
    pub size: String,
    pub md5: String,
    pub entry: String,
    pub package_size: String,
    pub decompressed_path: String,

    // pub segments
}
