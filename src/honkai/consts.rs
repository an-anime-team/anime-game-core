pub static mut API_URI: &str = concat!("https://sdk-os-static.", "ho", "yo", "verse", ".com/bh3_global/mdk/launcher/api/resource?key=gcStgarh&launcher_id=10");

/// Name of the game's data folder
pub static mut DATA_FOLDER_NAME: &str = concat!("BH3_Data");

// FIXME: copied from gen-shi-n, not sure which ones for this game
#[cfg(feature = "telemetry")]
pub static mut TELEMETRY_SERVERS: &[&str] = &[
    concat!("log-upload-os.", "ho", "yo", "verse", ".com"),
    concat!("overseauspider.", "yu", "ans", "hen", ".com")
];
