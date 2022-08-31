#[cfg(feature = "genshin")]
pub mod genshin;

#[cfg(feature = "honkai")]
pub mod honkai;

pub mod version;
pub mod curl;
pub mod api;

pub use ::curl as curl_sys;

#[cfg(feature = "external")]
pub mod external;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod prelude {
    pub use super::version::*;
    pub use super::curl::fetch;
    pub use super::api;

    #[cfg(feature = "genshin")]
    pub use super::genshin::prelude as genshin;

    #[cfg(feature = "honkai")]
    pub use super::honkai::prelude as honkai;
}
