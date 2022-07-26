use std::process::Command;
use std::io::{Error, ErrorKind};

/// Try to apply hdiff patch
pub fn patch<T: ToString>(file: T, patch: T, output: T) -> std::io::Result<()> {
    let output = Command::new(super::STORAGE.map("hpatchz")?)
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
