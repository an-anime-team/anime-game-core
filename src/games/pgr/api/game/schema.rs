use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Response {
    pub default: Data
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Data {
    /// Relative path to the game files list
    pub resources: String,

    /// Relative path to the unpacked game files
    pub resourcesBasePath: String,

    pub version: String
}
