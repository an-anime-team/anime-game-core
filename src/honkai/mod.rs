pub mod consts;
pub mod api;
pub mod game;
pub mod telemetry;

#[cfg(feature = "install")]
pub mod repairer;

pub mod prelude {
    pub use super::consts::*;
    pub use super::game::Game;
    pub use super::telemetry;

    #[cfg(feature = "install")]
    pub use super::repairer;
}
