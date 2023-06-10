use std::cmp::Ordering;

use serde_json::Value as JsonValue;

use crate::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JadeiteMetadata {
    pub hi3rd: JadeiteHi3rdMetadata,
    pub hsr: JadeiteHsrMetadata
}

impl Default for JadeiteMetadata {
    #[inline]
    fn default() -> Self {
        Self {
            hi3rd: JadeiteHi3rdMetadata::default(),
            hsr: JadeiteHsrMetadata::default()
        }
    }
}

impl From<&JsonValue> for JadeiteMetadata {
    fn from(value: &JsonValue) -> Self {
        Self {
            hi3rd: match value.get("hi3rd") {
                Some(status) => JadeiteHi3rdMetadata::from(status),
                None => JadeiteHi3rdMetadata::default()
            },

            hsr: match value.get("hsr") {
                Some(status) => JadeiteHsrMetadata::from(status),
                None => JadeiteHsrMetadata::default()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JadeiteHi3rdMetadata {
    pub global: JadeitePatchStatus
}

impl Default for JadeiteHi3rdMetadata {
    #[inline]
    fn default() -> Self {
        Self {
            global: JadeitePatchStatus::default()
        }
    }
}

impl From<&JsonValue> for JadeiteHi3rdMetadata {
    fn from(value: &JsonValue) -> Self {
        Self {
            global: match value.get("global") {
                Some(status) => JadeitePatchStatus::from(status),
                None => JadeitePatchStatus::default()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JadeiteHsrMetadata {
    pub global: JadeitePatchStatus
}

impl Default for JadeiteHsrMetadata {
    #[inline]
    fn default() -> Self {
        Self {
            global: JadeitePatchStatus::default()
        }
    }
}

impl From<&JsonValue> for JadeiteHsrMetadata {
    fn from(value: &JsonValue) -> Self {
        Self {
            global: match value.get("global") {
                Some(status) => JadeitePatchStatus::from(status),
                None => JadeitePatchStatus::default()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JadeitePatchStatus {
    pub status: JadeitePatchStatusVariant,
    pub version: Version
}

impl Default for JadeitePatchStatus {
    #[inline]
    fn default() -> Self {
        Self {
            status: JadeitePatchStatusVariant::default(),
            version: Version::new(0, 0, 0)
        }
    }
}

impl From<&JsonValue> for JadeitePatchStatus {
    fn from(value: &JsonValue) -> Self {
        let default = Self::default();

        Self {
            status: match value.get("status").and_then(|status| status.as_str()) {
                Some(status) => JadeitePatchStatusVariant::from(status),
                None => default.status
            },

            version: match value.get("version").and_then(|version| version.as_str()) {
                Some(version) => Version::from_str(version).unwrap_or(default.version),
                None => default.version
            }
        }
    }
}

impl JadeitePatchStatus {
    /// Get the patch status for the provided game version
    pub fn get_status(&self, game_version: Version) -> JadeitePatchStatusVariant {
        match self.version.cmp(&game_version) {
            // Metadata game version is lower than the one we gave here,
            // so some predictions are needed
            Ordering::Less => match self.status {
                // Even if the patch was verified - return that it's not verified, at least because the game was updated
                JadeitePatchStatusVariant::Verified => JadeitePatchStatusVariant::Unverified,

                // If the patch wasn't verified - keep it unverified
                JadeitePatchStatusVariant::Unverified => JadeitePatchStatusVariant::Unverified,

                // If the patch was marked as broken - keep it broken
                JadeitePatchStatusVariant::Broken => JadeitePatchStatusVariant::Broken,

                // If the patch was marked as unsafe - keep it unsafe
                JadeitePatchStatusVariant::Unsafe => JadeitePatchStatusVariant::Unsafe
            },

            // Both metadata and given game versions are equal
            // so we just return current patch status
            Ordering::Equal => self.status,

            // Given game version is outdated, so we're not sure about its status
            // Here I suppose that it's just unverified and let user to decide what to do
            Ordering::Greater => JadeitePatchStatusVariant::Unverified
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JadeitePatchStatusVariant {
    /// Patch is verified and works fine
    /// 
    /// Value: `verified`
    Verified,

    /// Patch is not verified to be working
    /// 
    /// Value: `unverified`
    Unverified,

    /// Patch doesn't work
    /// 
    /// Value: `broken`
    Broken,

    /// Patch is working but unsafe for use
    /// 
    /// Value: `unsafe`
    Unsafe
}

impl Default for JadeitePatchStatusVariant {
    #[inline]
    fn default() -> Self {
        Self::Unverified
    }
}

impl From<&str> for JadeitePatchStatusVariant {
    fn from(value: &str) -> Self {
        match value {
            "verified"   => Self::Verified,
            "unverified" => Self::Unverified,
            "broken"     => Self::Broken,
            "unsafe"     => Self::Unsafe,

            // Not really a good practice but it's unlikely to happen anyway
            _ => Self::default()
        }
    }
}
