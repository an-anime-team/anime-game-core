pub use minreq;

/// Core library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

lazy_static::lazy_static! {
    /// Default requests timeout in seconds
    pub static ref REQUESTS_TIMEOUT: u64 = match option_env!("LAUNCHER_REQUESTS_TIMEOUT") {
        Some(timeout) => timeout.parse().unwrap_or(8),
        None => 8
    };
}

pub mod version;
pub mod traits;
pub mod prettify_bytes;
pub mod check_domain;

#[cfg(feature = "patches")]
pub mod patches;

// Games-specific functionality

mod games;

#[cfg(feature = "genshin")]
pub use games::genshin;

#[cfg(feature = "star-rail")]
pub use games::star_rail;

#[cfg(feature = "honkai")]
pub use games::honkai;

#[cfg(feature = "pgr")]
pub use games::pgr;

#[cfg(feature = "wuwa")]
pub use games::wuwa;

// Core functionality

#[cfg(feature = "external")]
pub mod external;

#[cfg(feature = "install")]
pub mod installer;

#[cfg(feature = "install")]
pub mod repairer;

pub mod prelude {
    pub use super::version::*;
    pub use super::prettify_bytes::prettify_bytes;

    pub use super::traits::prelude::*;

    #[cfg(feature = "patches")]
    pub use super::patches::prelude::*;

    #[cfg(feature = "genshin")]
    pub use super::genshin::prelude as genshin;

    #[cfg(feature = "honkai")]
    pub use super::honkai::prelude as honkai;

    #[cfg(feature = "install")]
    pub use super::installer::prelude::*;

    #[cfg(feature = "install")]
    pub use super::repairer::*;
}
