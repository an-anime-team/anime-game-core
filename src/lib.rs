pub mod consts;
pub mod api;
pub mod json_schemas;
pub mod version;
pub mod curl;
pub mod game;
pub mod voice_data;
pub mod external;

#[cfg(feature = "install")]
pub mod installer;

#[cfg(feature = "linux-patch")]
pub mod linux_patch;

pub mod prelude {
    pub use super::consts::*;
    pub use super::version::Version;
    pub use super::api::API;
    pub use super::curl::fetch;
    pub use super::game::Game;
    pub use super::voice_data::prelude::*;

    #[cfg(feature = "install")]
    pub use super::installer::prelude::*;

    #[cfg(feature = "install")]
    pub use super::external::hdiff;
}
