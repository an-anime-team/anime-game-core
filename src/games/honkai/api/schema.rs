use serde::{Serialize, Deserialize};

use super::schema_old;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Response {
    pub retcode: u16,
    pub message: String,
    pub data: Data
}

impl From<schema_old::Response> for Response {
    fn from(value: schema_old::Response) -> Self {
        Response {
            retcode: value.retcode,
            message: value.message,
            data: Data::from(value.data)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Data {
    pub game_packages: Vec<GamePackage>
}

impl From<schema_old::Data> for Data {
    fn from(value: schema_old::Data) -> Self {
        Data {
            game_packages: vec![
                GamePackage {
                    game: GameId {
                        id: String::from("osvnlOc0S8"),
                        biz: String::from("bh3_cn")
                    },
                    main: GameInfo::from(value.game)
                }
            ]
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GamePackage {
    pub game: GameId,
    pub main: GameInfo
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameId {
    pub id: String,
    pub biz: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameInfo {
    pub major: GameLatestInfo
}

impl From<schema_old::Game> for GameInfo {
    fn from(value: schema_old::Game) -> Self {
        GameInfo {
            major: GameLatestInfo::from(value.latest)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameLatestInfo {
    pub version: String,
    pub game_pkgs: Vec<Segment>,
    pub res_list_url: String
}

impl From<schema_old::Latest> for GameLatestInfo {
    fn from(value: schema_old::Latest) -> Self {
        GameLatestInfo {
            version: value.version,
            game_pkgs: vec![
                Segment {
                    url: value.path,
                    md5: value.md5,
                    size: value.package_size,
                    decompressed_size: value.size
                }
            ],
            res_list_url: value.decompressed_path
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Segment {
    pub url: String,
    pub md5: String,
    pub size: String,
    pub decompressed_size: String
}
