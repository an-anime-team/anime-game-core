use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Response {
    pub default: Data
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Data {
    /// CDN servers list
    pub cdnList: Vec<CdnInfo>,

    /// Relative path to the game files list
    pub resources: String,

    /// Relative path to the unpacked game files
    pub resourcesBasePath: String,

    pub version: String
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CdnInfo {
    pub K1: u32,
    pub K2: u32,
    pub P: u32,
    pub url: String
}
