pub use minreq;

/// Default requests timeout in seconds
pub const DEFAULT_REQUESTS_TIMEOUT: u64 = 4;

/// Core library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod version;
pub mod traits;
pub mod prettify_bytes;
pub mod check_domain;

// Games-specific functionality

mod games;

#[cfg(feature = "genshin")]
pub use games::genshin;

#[cfg(feature = "honkai")]
pub use games::honkai;

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

    #[cfg(feature = "genshin")]
    pub use super::genshin::prelude as genshin;

    #[cfg(feature = "honkai")]
    pub use super::honkai::prelude as honkai;

    #[cfg(feature = "install")]
    pub use super::installer::prelude::*;

    #[cfg(feature = "install")]
    pub use super::repairer::*;
}
