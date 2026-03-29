#[cfg(feature = "patch-jadeite")]
pub mod jadeite;

#[cfg(feature = "patch-mfc140")]
pub mod mfc140;

pub mod prelude {
    #[cfg(feature = "patch-jadeite")]
    pub use super::jadeite::{
        self,
        JadeiteLatest,
        metadata::*
    };

    #[cfg(feature = "patch-mfc140")]
    pub use super::mfc140;
}
