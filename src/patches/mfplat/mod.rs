use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::process::{Command, Stdio};

use md5::{Md5, Digest};

#[cfg(feature = "install")]
use crate::installer::{
    downloader::Downloader,
    archives::Archive
};

// TODO: consider moving it to the wincompatlib

const PATCH_URI: &str = "https://github.com/z0z0z/mf-install/archive/refs/tags/1.0.zip";
const PATCH_HASH: &str = "51340459ae099fe3aaa5f7f1bb98ae1c";
const MFPLAT_DLL_HASH: &str = "54b5dcd55b223bc5df50b82e1e9e86b1";

/// Check if the patch is applied to the wine prefix
pub fn is_applied(prefix_path: impl AsRef<Path>) -> anyhow::Result<bool> {
    let mfplat_path = prefix_path.as_ref().join("drive_c/windows/system32/mfplat.dll");

    if !mfplat_path.exists() {
        return Ok(false);
    }

    Ok(format!("{:x}", Md5::digest(std::fs::read(mfplat_path)?)) == MFPLAT_DLL_HASH)
}

#[cfg(feature = "install")]
/// Apply available patch
pub fn apply(prefix_path: impl AsRef<OsStr>, temp_folder: impl AsRef<Path>) -> anyhow::Result<bool> {
    tracing::debug!("Applying wine prefix patch");

    let temp_folder = temp_folder.as_ref();
    let mfplat = temp_folder.join("mfplat.zip");

    // Download patch files
    Downloader::new(PATCH_URI)?.download(&mfplat, |_, _| {})?;

    // Verify archive hash
    if format!("{:x}", Md5::digest(std::fs::read(&mfplat)?)) != PATCH_HASH {
        anyhow::bail!("Incorrect mfplat patch hash");
    }

    // Extract patch files
    Archive::open(&mfplat)?.extract(&temp_folder)?;

    // Run patch installer
    let output = Command::new("bash")
        .arg("mf-install-1.0/mf-install.sh")
        .env("WINEPREFIX", prefix_path.as_ref())
        .current_dir(&temp_folder)
        .stdout(Stdio::piped())
        .output()?;

    // Remove temp patch files
    std::fs::remove_dir_all(temp_folder.join("mf-install-1.0"))?;
    std::fs::remove_file(mfplat)?;

    // Return patching status
    if let Ok(applied) = is_applied(PathBuf::from(prefix_path.as_ref())) {
        Ok(applied)
    }

    else {
        let stdout = String::from_utf8_lossy(&output.stdout);

        tracing::error!("Failed to apply patch: {stdout}");

        anyhow::bail!("Failed to apply patch: {stdout}");
    }
}
