use std::path::{Path, PathBuf};

use wincompatlib::wine::ext::*;

use crate::installer::downloader::Downloader;

// Source: https://github.com/Winetricks/winetricks/blob/8ffdb53f5aebfe51502ecceb0d5e7994ad814424/src/winetricks#L13702
// TODO: consider moving it to the wincompatlib

const URL: &str = "https://download.microsoft.com/download/6/D/F/6DF3FF94-F7F9-4F0B-838C-A328D1A7D0EE/vc_redist.x64.exe";

const LIBRARIES: &[&str] = &[
    "api-ms-win-crt-private-l1-1-0",
    "api-ms-win-crt-conio-l1-1-0",
    "api-ms-win-crt-convert-l1-1-0",
    "api-ms-win-crt-environment-l1-1-0",
    "api-ms-win-crt-filesystem-l1-1-0",
    "api-ms-win-crt-heap-l1-1-0",
    "api-ms-win-crt-locale-l1-1-0",
    "api-ms-win-crt-math-l1-1-0",
    "api-ms-win-crt-multibyte-l1-1-0",
    "api-ms-win-crt-process-l1-1-0",
    "api-ms-win-crt-runtime-l1-1-0",
    "api-ms-win-crt-stdio-l1-1-0",
    "api-ms-win-crt-string-l1-1-0",
    "api-ms-win-crt-utility-l1-1-0",
    "api-ms-win-crt-time-l1-1-0",
    "atl140",
    "concrt140",
    "msvcp140",
    "msvcp140_1",
    "msvcp140_atomic_wait",
    "ucrtbase",
    "vcomp140",
    "vccorlib140",
    "vcruntime140",
    "vcruntime140_1"
];

pub fn is_installed(wine_prefix: impl AsRef<Path>) -> bool {
    // Not listed above but it must be installed too
    wine_prefix.as_ref().join("drive_c/windows/system32/mfc140.dll").exists()
}

pub fn install(wine: impl WineWithExt + WineRunExt, wine_prefix: impl AsRef<Path>, temp: Option<impl Into<PathBuf>>) -> anyhow::Result<()> {
    let temp = temp
        .map(|path| path.into())
        .unwrap_or_else(std::env::temp_dir)
        .join("vcrun2015");

    if temp.exists() {
        std::fs::remove_dir_all(&temp)?;
    }

    std::fs::create_dir_all(&temp)?;

    let vcredist = temp.join("vc_redist.x64.exe");

    Downloader::new(URL)?
        .with_continue_downloading(false)
        .download(&vcredist, |_, _| {})?;

    let output = wine
        .with_prefix(wine_prefix.as_ref())
        .run_args([
            format!("{}", vcredist.to_string_lossy()),
            String::from("/install")
        ])?
        .wait_with_output()?;

    if !output.status.success() {
        anyhow::bail!("Failed to install vcrun2015: {}", String::from_utf8_lossy(&output.stderr));
    }

    let reg_file = wine_prefix.as_ref().join("user.reg");

    let reg = std::fs::read_to_string(&reg_file)?;
    let mut new_reg = String::new();

    for record in reg.split("\n\n") {
        if record.starts_with("[Software\\\\Wine\\\\DllOverrides]") {
            let mut new_record = record.to_string();

            for lib in LIBRARIES {
                if !new_record.contains(lib) {
                    new_record = format!("{new_record}\n\"{lib}\"=\"native,builtin\"");
                }
            }

            new_reg = format!("{new_reg}{new_record}\n\n");
        }

        else {
            new_reg = format!("{new_reg}{record}\n\n");
        }
    }

    std::fs::write(reg_file, new_reg)?;

    Ok(())
}
