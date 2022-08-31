pub mod consts;
pub mod json_schemas;
pub mod api;
pub mod game;

#[cfg(feature = "install")]
pub mod repairer;

#[cfg(feature = "telemetry")]
pub mod telemetry;

pub mod prelude {
    pub use super::consts::*;
    pub use super::game::Game;

    #[cfg(feature = "install")]
    pub use super::repairer;

    #[cfg(feature = "telemetry")]
    pub use super::telemetry;
}
