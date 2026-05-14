use std::path::Path;

use crate::version::Version;
#[cfg(feature = "install")]
use crate::installer::installer::*;

pub mod metadata;

pub const REPO_URI: &str = "https://codeberg.org/mkrsym1/jadeite";
pub const REPO_API_URI: &str = "https://codeberg.org/api/v1/repos/mkrsym1/jadeite/releases/latest";

pub const METADATA_URIS: &[&str] = &[
    // Primary
    "https://codeberg.org/mkrsym1/jadeite/raw/branch/master/metadata.json",
    // Mirrors
    "https://notabug.org/mkrsym1/jadeite-mirror/raw/master/metadata.json"
];

#[inline]
pub fn is_installed(folder: impl AsRef<Path>) -> bool {
    folder.as_ref().join(".version").exists()
}

pub fn get_version(folder: impl AsRef<Path>) -> anyhow::Result<Version> {
    let dotversion_path = folder.as_ref().join(".version");
    let version_bytes = std::fs::read(dotversion_path)?;

    if version_bytes.len() == 3 {
        tracing::info!("Found old format version file");
        Ok(Version::new(
            version_bytes[0],
            version_bytes[1],
            version_bytes[2]
        ))
    }
    else if version_bytes.len() > 3 {
        let version_str = String::from_utf8(version_bytes)?;
        Version::from_str(version_str.trim_end())
            .ok_or_else(|| anyhow::anyhow!("Invalid version string: {version_str}"))
    }
    else {
        Err(anyhow::anyhow!(
            "The `.version` file is too short, cannot parse"
        ))
    }
}

#[cfg(feature = "install")]
#[cached::proc_macro::cached(result)]
pub fn get_latest() -> anyhow::Result<JadeiteLatest> {
    let response = minreq::get(REPO_API_URI)
        .send()?
        .json::<serde_json::Value>()?;

    let version = response
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        .map(|tag| tag.strip_prefix('v').unwrap_or(tag))
        .and_then(Version::from_str);

    let Some(version) = version
    else {
        anyhow::bail!("Failed to request latest patch version");
    };

    let download_uri = response
        .get("assets")
        .and_then(|assets| assets.as_array())
        .and_then(|assets| assets.first())
        .and_then(|asset| asset.get("browser_download_url"))
        .and_then(|url| url.as_str());

    let Some(download_uri) = download_uri
    else {
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
    for uri in METADATA_URIS {
        let Ok(resp) = minreq::get(*uri).with_timeout(20).send()
        else {
            tracing::warn!("Could not reach '{uri}'. Attempting to use next fallback");
            continue;
        };

        let Ok(json) = resp.json::<serde_json::Value>()
        else {
            tracing::warn!("Got invalid response from '{uri}'. Attempting to use next fallback");
            continue;
        };

        return Ok(metadata::JadeiteMetadata::from(&json));
    }

    anyhow::bail!("Could not get metadata from any of the mirrors");
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JadeiteLatest {
    pub version: Version,
    pub download_uri: String
}

impl JadeiteLatest {
    #[cfg(feature = "install")]
    pub fn install(
        &self,
        folder: impl AsRef<Path>,
        updater: impl Fn(Update) + Clone + Send + 'static
    ) -> anyhow::Result<()> {
        Installer::new(&self.download_uri)?
            .with_filename("jadeite.zip")
            .with_free_space_check(false)
            .install(folder.as_ref(), updater);

        std::fs::write(folder.as_ref().join(".version"), self.version.version)?;

        Ok(())
    }
}
