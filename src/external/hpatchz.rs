use std::process::Command;
use std::io::{Error, ErrorKind};
use std::os::unix::prelude::PermissionsExt;

/// Try to apply hdiff patch
pub fn patch<T: ToString>(file: T, patch: T, output: T) -> std::io::Result<()> {
    let hpatchz = super::STORAGE.map("hpatchz")?;

    // Allow to execute this binary
    std::fs::set_permissions(&hpatchz, std::fs::Permissions::from_mode(0o777))?;

    let output = Command::new(hpatchz)
        .arg("-f")
        .arg(file.to_string())
        .arg(patch.to_string())
        .arg(output.to_string())
        .output()?;

    if String::from_utf8_lossy(output.stdout.as_slice()).contains("patch ok!") {
        Ok(())
    }

    else {
        Err(Error::new(ErrorKind::Other, format!("Failed to apply hdiff patch: {}", String::from_utf8_lossy(&output.stderr))))
    }
}
