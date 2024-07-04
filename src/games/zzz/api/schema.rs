use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Response {
    pub retcode: u16,
    pub message: String,
    pub data: Data
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Data {
    pub game_packages: Vec<GamePackage>
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GamePackage {
    pub game: GameId,
    pub main: GameInfo,
    pub pre_download: Option<GamePredownloadInfo>
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameId {
    pub id: String,
    pub biz: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameInfo {
    pub major: GameLatestInfo,
    pub patches: Vec<GamePatch>
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameLatestInfo {
    pub version: String,
    pub game_pkgs: Vec<Segment>,
    pub audio_pkgs: Vec<AudioPackage>,
    pub res_list_url: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Segment {
    pub url: String,
    pub md5: String,
    pub size: String,
    pub decompressed_size: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioPackage {
    pub language: String,
    pub url: String,
    pub md5: String,
    pub size: String,
    pub decompressed_size: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GamePatch {
    pub version: String,
    pub game_pkgs: Vec<Segment>,
    pub audio_pkgs: Vec<AudioPackage>
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GamePredownloadInfo {
    pub major: Option<GameLatestInfo>,
    pub patches: Vec<GamePatch>
}
