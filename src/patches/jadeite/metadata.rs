use std::cmp::Ordering;

use serde_json::Value as JsonValue;

use crate::version::Version;

#[cfg(feature = "star-rail")]
use crate::games::star_rail::consts::GameEdition as StarRailGameEdition;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct JadeiteMetadata {
    pub jadeite: JadeitePatchMetadata,
    pub games: JadeiteGamesMetadata
}

impl From<&JsonValue> for JadeiteMetadata {
    fn from(value: &JsonValue) -> Self {
        Self {
            jadeite: value.get("jadeite")
                .map(JadeitePatchMetadata::from)
                .unwrap_or_default(),

            games: value.get("games")
                .map(JadeiteGamesMetadata::from)
                .unwrap_or_default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JadeitePatchMetadata {
    pub version: Version
}

impl Default for JadeitePatchMetadata {
    #[inline]
    fn default() -> Self {
        Self {
            version: Version::new(0, 0, 0)
        }
    }
}

impl From<&JsonValue> for JadeitePatchMetadata {
    fn from(value: &JsonValue) -> Self {
        let default = Self::default();

        Self {
            version: value.get("version")
                .and_then(|version| version.as_str())
                .and_then(Version::from_str)
                .unwrap_or(default.version)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct JadeiteGamesMetadata {
    pub hi3rd: JadeiteHi3rdMetadata,
    pub hsr: JadeiteHsrMetadata
}

impl From<&JsonValue> for JadeiteGamesMetadata {
    fn from(value: &JsonValue) -> Self {
        Self {
            hi3rd: value.get("hi3rd")
                .map(JadeiteHi3rdMetadata::from)
                .unwrap_or_default(),

            hsr: value.get("hsr")
                .map(JadeiteHsrMetadata::from)
                .unwrap_or_default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct JadeiteHi3rdMetadata {
    pub global: JadeitePatchStatus,
    pub sea: JadeitePatchStatus,
    pub china: JadeitePatchStatus,
    pub taiwan: JadeitePatchStatus,
    pub korea: JadeitePatchStatus,
    pub japan: JadeitePatchStatus
}

impl From<&JsonValue> for JadeiteHi3rdMetadata {
    fn from(value: &JsonValue) -> Self {
        Self {
            global: value.get("global")
                .map(JadeitePatchStatus::from)
                .unwrap_or_default(),

            sea: value.get("sea")
                .map(JadeitePatchStatus::from)
                .unwrap_or_default(),

            china: value.get("china")
                .map(JadeitePatchStatus::from)
                .unwrap_or_default(),

            taiwan: value.get("taiwan")
                .map(JadeitePatchStatus::from)
                .unwrap_or_default(),

            korea: value.get("korea")
                .map(JadeitePatchStatus::from)
                .unwrap_or_default(),

            japan: value.get("japan")
                .map(JadeitePatchStatus::from)
                .unwrap_or_default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct JadeiteHsrMetadata {
    pub global: JadeitePatchStatus,
    pub china: JadeitePatchStatus
}

impl From<&JsonValue> for JadeiteHsrMetadata {
    fn from(value: &JsonValue) -> Self {
        Self {
            global: value.get("global")
                .map(JadeitePatchStatus::from)
                .unwrap_or_default(),

            china: value.get("china")
                .map(JadeitePatchStatus::from)
                .unwrap_or_default()
        }
    }
}

#[cfg(feature = "star-rail")]
impl JadeiteHsrMetadata {
    pub fn for_edition(&self, edition: StarRailGameEdition) -> JadeitePatchStatus {
        match edition {
            StarRailGameEdition::Global => self.global,
            StarRailGameEdition::China => self.china
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
            status: value.get("status")
                .and_then(|status| status.as_str())
                .map(JadeitePatchStatusVariant::from)
                .unwrap_or(default.status),

            version: value.get("version")
                .and_then(|version| version.as_str())
                .and_then(Version::from_str)
                .unwrap_or(default.version)
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
                JadeitePatchStatusVariant::Unsafe => JadeitePatchStatusVariant::Unsafe,

                // If the patch was concerning - then it's still concerning
                JadeitePatchStatusVariant::Concerning => JadeitePatchStatusVariant::Concerning
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
    /// You can't run the game with this status
    /// 
    /// Value: `unsafe`
    Unsafe,

    /// Patch is working but we have some concerns about it
    /// 
    /// You still can run the game with this status
    /// 
    /// Value: `concerning`
    Concerning
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
            "concerning" => Self::Concerning,

            // Not really a good practice but it's unlikely to happen anyway
            _ => Self::default()
        }
    }
}
