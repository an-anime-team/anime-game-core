use std::path::Path;

use crate::version::Version;

#[cfg(feature = "install")]
use crate::installer::installer::*;

pub mod metadata;

pub const REPO_URI: &str = "https://codeberg.org/mkrsym1/jadeite";
pub const REPO_API_URI: &str = "https://codeberg.org/api/v1/repos/mkrsym1/jadeite/releases/latest";
pub const METADATA_URI: &str = "https://codeberg.org/mkrsym1/jadeite/raw/branch/master/metadata.json";

#[inline]
pub fn is_installed(folder: impl AsRef<Path>) -> bool {
    folder.as_ref().join(".version").exists()
}

pub fn get_version(folder: impl AsRef<Path>) -> anyhow::Result<Version> {
    let bytes = std::fs::read(folder.as_ref().join(".version"))?;

    Ok(Version::new(bytes[0], bytes[1], bytes[2]))
}

#[cfg(feature = "install")]
#[cached::proc_macro::cached(result)]
pub fn get_latest() -> anyhow::Result<JadeiteLatest> {
    let response = minreq::get(REPO_API_URI).send()?.json::<serde_json::Value>()?;

    let version = response.get("tag_name")
        .and_then(|tag| tag.as_str())
        .map(|tag| tag.strip_prefix('v').unwrap_or(tag))
        .and_then(Version::from_str);

    let Some(version) = version else {
        anyhow::bail!("Failed to request latest patch version");
    };

    let download_uri = response.get("assets")
        .and_then(|assets| assets.as_array())
        .and_then(|assets| assets.first())
        .and_then(|asset| asset.get("browser_download_url"))
        .and_then(|url| url.as_str());

    let Some(download_uri) = download_uri else {
        anyhow::bail!("Failed to request patch downloading URI");
    };

    Ok(JadeiteLatest {
        version,
        download_uri: download_uri.to_string()
    })
}

#[cfg(feature = "install")]
#[cached::proc_macro::cached(result)]
pub fn get_metadata() -> anyhow::Result<metadata::JadeiteMetadata> {
    Ok(metadata::JadeiteMetadata::from(&minreq::get(METADATA_URI).send()?.json::<serde_json::Value>()?))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JadeiteLatest {
    pub version: Version,
    pub download_uri: String
}

impl JadeiteLatest {
    #[cfg(feature = "install")]
    pub fn install(&self, folder: impl AsRef<Path>, updater: impl Fn(Update) + Clone + Send + 'static) -> anyhow::Result<()> {
        Installer::new(&self.download_uri)?
            .with_filename("jadeite.zip")
            .with_free_space_check(false)
            .install(folder.as_ref(), updater);

        std::fs::write(folder.as_ref().join(".version"), self.version.version)?;

        Ok(())
    }
}
