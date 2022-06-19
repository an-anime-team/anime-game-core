pub mod consts;
pub mod api;
pub mod json_schemas;
pub mod version;
pub mod game;
pub mod voice_data;

#[cfg(feature = "install")]
pub mod installer;

pub mod prelude {
    pub use super::consts::*;
    pub use super::version::Version;
    pub use super::api::API;
    pub use super::game::{
        Game,
        VersionDiff as GameVersionDiff
    };
    pub use super::voice_data::prelude::*;

    #[cfg(feature = "install")]
    pub use super::installer::prelude::*;
}
