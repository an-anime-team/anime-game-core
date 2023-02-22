use std::process::Command;
use std::io::{Error, ErrorKind};
use std::os::unix::prelude::PermissionsExt;
use std::path::PathBuf;

/// Try to apply hdiff patch
#[tracing::instrument(level = "debug")]
pub fn patch<T: Into<PathBuf> + std::fmt::Debug>(file: T, patch: T, output: T) -> std::io::Result<()> {
    let hpatchz = super::STORAGE.map("hpatchz")?;

    // Allow to execute this binary
    std::fs::set_permissions(&hpatchz, std::fs::Permissions::from_mode(0o777))?;

    let output = Command::new(hpatchz)
        .arg("-f")
        .arg(file.into().as_os_str())
        .arg(patch.into().as_os_str())
        .arg(output.into().as_os_str())
        .output()?;

    if String::from_utf8_lossy(output.stdout.as_slice()).contains("patch ok!") {
        Ok(())
    }

    else {
        Err(Error::new(ErrorKind::Other, format!("Failed to apply hdiff patch: {}", String::from_utf8_lossy(&output.stderr))))
    }
}
