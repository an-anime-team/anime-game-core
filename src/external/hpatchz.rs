use std::process::Command;
use std::path::Path;
use std::io::{Error, ErrorKind};

/// Try to find path to the hpatchz binary
pub fn get_binary_path() -> Option<String> {
    for path in ["hpatchz", "external/hpatchz", "external/hpatchz/hpatchz", "hpatchz/hpatchz"] {
        if Path::new(path).exists() {
            return Some(String::from(path));
        }
    }

    None
}

/// Try to apply hdiff patch
pub fn patch<T: ToString>(file: T, patch: T, output: T) -> Result<(), Error> {
    match get_binary_path() {
        Some(hpatchz) => {
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
        },
        None => Err(Error::new(ErrorKind::Other, "hpatchz binary is missing"))
    }
}
