#[cfg(feature = "patch-dawn")]
pub mod dawn;

#[cfg(feature = "patch-jadeite")]
pub mod jadeite;

#[cfg(feature = "patch-mfplat")]
pub mod mfplat;

#[cfg(feature = "patch-mfc140")]
pub mod mfc140;

pub mod prelude {
    #[cfg(feature = "patch-dawn")]
    pub use super::dawn::prelude::*;

    #[cfg(feature = "patch-jadeite")]
    pub use super::jadeite::{
        self,
        JadeiteLatest,
        metadata::*
    };

    #[cfg(feature = "patch-mfplat")]
    pub use super::mfplat;

    #[cfg(feature = "patch-mfc140")]
    pub use super::mfc140;
}
