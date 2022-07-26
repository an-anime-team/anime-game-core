pub mod consts;
pub mod api;
pub mod json_schemas;
pub mod version;
pub mod curl;
pub mod game;
pub mod voice_data;

pub use ::curl as curl_sys;

#[cfg(test)]
mod tests;

#[cfg(feature = "install")]
pub mod installer;

#[cfg(feature = "install")]
pub mod repairer;

#[cfg(feature = "linux-patch")]
pub mod linux_patch;

#[cfg(feature = "external")]
pub mod external;

pub mod prelude {
    pub use super::consts::*;
    pub use super::version::*;
    pub use super::api::API;
    pub use super::curl::fetch;
    pub use super::game::Game;
    pub use super::voice_data::prelude::*;

    #[cfg(feature = "install")]
    pub use super::installer::prelude::*;

    #[cfg(feature = "install")]
    pub use super::repairer;

    #[cfg(feature = "linux-patch")]
    pub use super::linux_patch::prelude::*;

    #[cfg(feature = "external")]
    pub use super::external;
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
