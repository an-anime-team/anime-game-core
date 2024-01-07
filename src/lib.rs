pub mod network;
pub mod filesystem;
pub mod archive;
pub mod builtin;
pub mod updater;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
