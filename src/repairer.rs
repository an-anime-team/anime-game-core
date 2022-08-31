use super::installer::downloader::{Downloader, DownloadingError};

// {"remoteName": "UnityPlayer.dll", "md5": "8c8c3d845b957e4cb84c662bed44d072", "fileSize": 33466104}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityFile {
    pub path: String,
    pub md5: String,
    pub size: u64,
    pub base_url: String
}

impl IntegrityFile {
    /// Compare files' sizes and (if needed) hashes
    pub fn verify<T: ToString>(&self, game_path: T) -> bool {
        let file_path = format!("{}/{}", game_path.to_string(), self.path);

        // Compare files' sizes. If they're different - they 100% different
        match std::fs::metadata(&file_path) {
            Ok(metadata) => {
                if metadata.len() != self.size {
                    false
                }

                else {
                    // And if files' sizes are same we should compare their hashes
                    match std::fs::read(&file_path) {
                        Ok(hash) => format!("{:x}", md5::compute(hash)) == self.md5,
                        Err(_) => false
                    }
                }
            },
            Err(_) => false
        }
    }

    /// Compare files' sizes and do not compare files' hashes. Works lots faster than `verify`
    pub fn fast_verify<T: ToString>(&self, game_path: T) -> bool {
        match std::fs::metadata(format!("{}/{}", game_path.to_string(), self.path)) {
            Ok(metadata) => metadata.len() == self.size,
            Err(_) => false
        }
    }

    /// Replace remote file with the latest one
    /// 
    /// This method doesn't compare them, so you should do it manually
    pub fn repair<T: ToString>(&self, game_path: T) -> Result<(), DownloadingError> {
        let mut downloader = Downloader::new(format!("{}/{}", self.base_url, self.path))?;

        Ok(downloader.download_to(format!("{}/{}", game_path.to_string(), self.path), |_, _| {})?)
    }
}
