use std::io::{Error, ErrorKind};
use std::process::Command;
use std::os::unix::prelude::PermissionsExt;
use std::path::Path;

/// Try to apply hdiff patch
pub fn patch(file: impl AsRef<Path>, patch: impl AsRef<Path>, output: impl AsRef<Path>) -> std::io::Result<()> {
    let hpatchz = super::STORAGE.map("hpatchz")?;

    // Allow to execute this binary
    std::fs::set_permissions(&hpatchz, std::fs::Permissions::from_mode(0o777))?;

    let output = Command::new(hpatchz)
        .arg("-f")
        .arg(file.as_ref())
        .arg(patch.as_ref())
        .arg(output.as_ref())
        .output()?;

    if String::from_utf8_lossy(&output.stdout).contains("patch ok!") {
        Ok(())
    }

    else {
        let err = String::from_utf8_lossy(&output.stderr);

        Err(Error::new(ErrorKind::Other, format!("Failed to apply hdiff patch: {err}")))
    }
}
