pub mod consts;
pub mod json_schemas;
pub mod api;
pub mod game;
pub mod voice_data;

#[cfg(feature = "install")]
pub mod repairer;

#[cfg(feature = "telemetry")]
pub mod telemetry;

#[cfg(feature = "linux-patch")]
pub mod linux_patch;

pub mod prelude {
    pub use super::consts::*;
    pub use super::game::Game;
    pub use super::voice_data::prelude::*;

    #[cfg(feature = "install")]
    pub use super::repairer;

    #[cfg(feature = "telemetry")]
    pub use super::telemetry;

    #[cfg(feature = "linux-patch")]
    pub use super::linux_patch::prelude::*;
}
