use std::sync::Arc;

use crate::filesystem::DriverExt;

pub mod version;
pub mod component;
pub mod diff;
pub mod integrity;

use version::Version;
use component::ComponentExt;

pub mod genshin;

pub trait GameExt {
    type Edition;
    type Component: ComponentExt;
    type Error;

    /// Create game manager instance
    fn new(driver: impl DriverExt + 'static, edition: Self::Edition) -> Self;

    /// Get currently selected game files driver
    fn get_driver(&self) -> Arc<dyn DriverExt>;

    /// Get currently selected game edition
    fn get_edition(&self) -> Self::Edition;

    /// Check if the game is installed
    fn is_installed(&self) -> bool;

    /// Get installed game version
    fn get_version(&self) -> Result<Version, Self::Error>;

    /// Get latest game version
    fn get_latest_version(&self) -> Result<Version, Self::Error>;

    /// Get list of game components
    fn get_components(&self) -> Result<Vec<Self::Component>, Self::Error>;
}
