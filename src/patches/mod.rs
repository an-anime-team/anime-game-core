#[cfg(feature = "patch-jadeite")]
pub mod jadeite;

#[cfg(feature = "patch-mfc140")]
pub mod mfc140;

#[cfg(feature = "patch-vcrun2015")]
pub mod vcrun2015;

pub mod prelude {
    #[cfg(feature = "patch-dawn")]
    pub use super::dawn::prelude::*;

    #[cfg(feature = "patch-jadeite")]
    pub use super::jadeite::{
        self,
        JadeiteLatest,
        metadata::*
    };

    #[cfg(feature = "patch-mfc140")]
    pub use super::mfc140;

    #[cfg(feature = "patch-vcrun2015")]
    pub use super::vcrun2015;
}
