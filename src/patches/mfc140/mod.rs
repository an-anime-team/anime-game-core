use std::path::{Path, PathBuf};
use std::process::Command;

use crate::installer::downloader::Downloader;

// Source: https://github.com/Winetricks/winetricks/blob/08304e81f9ac9a83c552a6bd78689040d174bf95/src/winetricks#L13770
// TODO: consider moving it to the wincompatlib

const URL_X64: &str = "https://aka.ms/vs/17/release/vc_redist.x64.exe";

const LIBRARIES: &[&str] = &["mfc140.dll", "mfc140u.dll", "mfcm140.dll", "mfcm140u.dll"];

pub fn is_installed(wine_prefix: impl AsRef<Path>) -> bool {
    wine_prefix
        .as_ref()
        .join("drive_c/windows/system32/mfc140.dll")
        .exists()
}

pub fn install(
    wine_prefix: impl AsRef<Path>,
    temp: Option<impl Into<PathBuf>>,
) -> anyhow::Result<()> {
    let temp = temp
        .map(|path| path.into())
        .unwrap_or_else(std::env::temp_dir)
        .join("vcrun2022");

    if temp.exists() {
        std::fs::remove_dir_all(&temp)?;
    }

    std::fs::create_dir_all(&temp)?;

    let vcredist = temp.join("vc_redist.x64.exe");
    let cabs_dir = temp.join("cabs");
    let dll_dir = temp.join("dll");

    Downloader::new(URL_X64)?
        .with_continue_downloading(false)
        .download(&vcredist, |_, _| {})?;

    std::fs::create_dir_all(&cabs_dir)?;
    std::fs::create_dir_all(&dll_dir)?;

    // extract all embedded cabs from the installer; the cab number that holds
    // the mfc dlls shifts between versions so we just dump them all and search
    let output = Command::new("cabextract")
        .arg("-d")
        .arg(&cabs_dir)
        .arg(&vcredist)
        .spawn()?
        .wait_with_output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to extract vcredist cabs: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for entry in std::fs::read_dir(&cabs_dir)?.flatten() {
        let _ = Command::new("cabextract")
            .arg("-d")
            .arg(&dll_dir)
            .arg(entry.path())
            .spawn()
            .and_then(|mut c| c.wait());

        if LIBRARIES
            .iter()
            .all(|lib| dll_dir.join(format!("{lib}_amd64")).exists())
        {
            break;
        }
    }

    let system32 = wine_prefix.as_ref().join("drive_c/windows/system32");

    // dlls inside the cab are named with an _amd64 suffix, strip it on copy
    for lib in LIBRARIES {
        let src = dll_dir.join(format!("{lib}_amd64"));
        if !src.exists() {
            anyhow::bail!("Could not find {} in vcrun2022 installer", lib);
        }
        std::fs::copy(src, system32.join(lib))?;
    }

    std::fs::remove_dir_all(temp)?;

    Ok(())
}
