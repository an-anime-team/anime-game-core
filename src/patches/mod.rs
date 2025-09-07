#[cfg(feature = "patch-jadeite")]
pub mod jadeite;

pub mod prelude {
    #[cfg(feature = "patch-jadeite")]
    pub use super::jadeite::{
        self,
        JadeiteLatest,
        metadata::*
    };
}
