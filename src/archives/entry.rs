use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicEntry {
    /// Relative file path
    pub path: PathBuf,

    /// Uncompressed file size
    pub size: u64
}
