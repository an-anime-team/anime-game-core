use std::process::Command;
use std::path::Path;
use std::io::{Error, ErrorKind};

pub fn get_binary_path() -> Option<String> {
    if Path::new("hpatchz").exists() {
        Some("hpatchz".to_string())
    }

    else if Path::new("external/hpatchz").exists() {
        Some("external/hpatchz".to_string())
    }

    else {
        None
    }
}

// TODO: rewrite to FFI
pub fn patch<T: ToString>(file: T, patch: T, output: T) -> Result<bool, Error> {
    match get_binary_path() {
        Some(hpatchz) => {
            // ./public/hdiffpatch/hpatchz -f "${path.addSlashes(file)}" "${path.addSlashes(patch)}" "${path.addSlashes(output)}"
            let output = Command::new(hpatchz)
                .arg(file.to_string())
                .arg(patch.to_string())
                .arg(output.to_string())
                .output()?;

            Ok(String::from_utf8_lossy(output.stdout.as_slice()).contains("patch ok!"))
        },
        None => Err(Error::new(ErrorKind::Other, "Missing hpatchz binary"))
    }
}
