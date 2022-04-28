pub mod consts {
    pub const VERSIONS_URL: &str = "https://sdk-os-static.mihoyo.com/hk4e_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10";
}

pub mod game;
pub mod locales;
pub mod json_schemas;

#[cfg(feature = "install")]
pub mod downloader;

mod version;

#[cfg(test)]
mod tests;

pub use version::Version;
