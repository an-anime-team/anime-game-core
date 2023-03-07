pub mod version;
pub mod curl;
pub mod api;
pub mod traits;
pub mod prettify_bytes;

pub use ::curl as curl_sys;

// Games-specific functionality

#[cfg(feature = "genshin")]
pub mod genshin;

#[cfg(feature = "honkai")]
pub mod honkai;

// Core functionality

#[cfg(feature = "external")]
pub mod external;

#[cfg(feature = "install")]
pub mod installer;

#[cfg(feature = "install")]
pub mod repairer;

pub mod prelude {
    pub use super::version::*;
    pub use super::curl::fetch;
    pub use super::api;
    pub use super::prettify_bytes::prettify_bytes;

    pub use super::traits::prelude::*;

    #[cfg(feature = "genshin")]
    pub use super::genshin::prelude as genshin;

    #[cfg(feature = "honkai")]
    pub use super::honkai::prelude as honkai;

    #[cfg(feature = "install")]
    pub use super::installer::prelude::*;

    #[cfg(feature = "install")]
    pub use super::repairer::*;
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
