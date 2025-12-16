/// Core library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

lazy_static::lazy_static! {
    /// Default requests timeout in seconds
    pub static ref REQUESTS_TIMEOUT: u64 = match std::env::var("LAUNCHER_REQUESTS_TIMEOUT") {
        Ok(timeout) => timeout.parse().unwrap_or(8),
        Err(_) => 8
    };
}

pub mod version;
pub mod traits;
pub mod prettify_bytes;
pub mod check_domain;
pub mod file_strings;

#[cfg(feature = "patches")]
pub mod patches;

// Games-specific functionality

mod games;

pub use minreq;

#[cfg(feature = "sophon")]
pub use reqwest;

#[cfg(feature = "genshin")]
pub use games::genshin;

#[cfg(feature = "star-rail")]
pub use games::star_rail;

#[cfg(feature = "zzz")]
pub use games::zzz;

#[cfg(feature = "honkai")]
pub use games::honkai;

// Core functionality

#[cfg(feature = "external")]
pub mod external;

#[cfg(feature = "install")]
pub mod installer;

#[cfg(feature = "install")]
pub mod repairer;

#[cfg(feature = "sophon")]
pub mod sophon;

pub mod prelude {
    pub use super::version::*;
    pub use super::prettify_bytes::prettify_bytes;
    
    pub use super::traits::prelude::*;

    #[cfg(feature = "patches")]
    #[allow(unused_imports)]
    pub use super::patches::prelude::*;

    #[cfg(feature = "genshin")]
    pub use super::genshin::prelude as genshin;

    #[cfg(feature = "star-rail")]
    pub use super::star_rail::prelude as star_rail;

    #[cfg(feature = "zzz")]
    pub use super::zzz::prelude as zzz;

    #[cfg(feature = "honkai")]
    pub use super::honkai::prelude as honkai;

    #[cfg(feature = "install")]
    pub use super::installer::prelude::*;

    #[cfg(feature = "install")]
    pub use super::repairer::*;
}
