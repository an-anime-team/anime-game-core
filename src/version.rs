use std::fmt;

#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub version: [u8; 3]
}

impl Version {
    #[inline]
    pub fn new(a: u8, b: u8, c: u8) -> Version {
        Version {
            version: [a, b, c]
        }
    }

    /// Get `Version` from the string
    /// 
    /// ```
    /// use anime_game_core::prelude::Version;
    /// 
    /// let version = Version::from_str("1.10.2").expect("Failed to parse version string");
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str<T: ToString>(str: T) -> Option<Version> {
        let str = str.to_string();
        let parts = str.split('.').collect::<Vec<&str>>();

        if parts.len() == 3 {
            if let Ok(a) = parts[0].parse() {
                if let Ok(b) = parts[1].parse() {
                    if let Ok(c) = parts[2].parse() {
                        return Some(Version::new(a, b, c));
                    }
                }
            }

            None
        }

        else {
            None
        }
    }

    /// Converts `Version` struct to plain format (e.g. "123")
    /// 
    /// ```
    /// use anime_game_core::prelude::Version;
    /// 
    /// assert_eq!(Version::new(1, 2, 3).to_plain_string(), "123");
    /// ```
    pub fn to_plain_string(&self) -> String {
        format!("{}{}{}", self.version[0], self.version[1], self.version[2])
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.version[0], self.version[1], self.version[2])
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.version[0], self.version[1], self.version[2])
    }
}

impl PartialEq<String> for Version {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        &self.to_string() == other
    }
}

impl PartialEq<Version> for String {
    #[inline]
    fn eq(&self, other: &Version) -> bool {
        self == &other.to_string()
    }
}

impl PartialEq<&str> for Version {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        &self.to_string() == other
    }
}

impl PartialEq<Version> for &str {
    #[inline]
    fn eq(&self, other: &Version) -> bool {
        self == &other.to_string()
    }
}

pub trait ToVersion {
    fn to_version(&self) -> Option<Version>;
}

impl<T> ToVersion for T where T: ToString {
    #[inline]
    fn to_version(&self) -> Option<Version> {
        Version::from_str(self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_version_new() {
        let version = Version::new(0, 0, 0);

        assert_eq!(version, "0.0.0");
        assert_eq!(version, "0.0.0".to_string());
        assert_eq!(Some(version), Version::from_str("0.0.0"));
        assert_eq!(version.to_plain_string(), "000".to_string());
    }

    #[test]
    pub fn test_version_from_str() {
        let version = Version::from_str("0.0.0");

        assert!(version.is_some());

        let version = version.unwrap();

        assert_eq!(version, "0.0.0");
        assert_eq!(version, "0.0.0".to_string());
        assert_eq!(version, Version::new(0, 0, 0));
        assert_eq!(version.to_plain_string(), "000".to_string());
    }

    #[test]
    pub fn test_version_long() {
        let version = Version::from_str("100.0.255");

        assert!(version.is_some());

        let version = version.unwrap();

        assert_eq!(version, "100.0.255");
        assert_eq!(version, "100.0.255".to_string());
        assert_eq!(version, Version::new(100, 0, 255));
        assert_eq!(version.to_plain_string(), "1000255".to_string());
    }

    #[test]
    pub fn test_incorrect_versions() {
        assert_eq!(Version::from_str(""), None);
        assert_eq!(Version::from_str("..0"), None);
        assert_eq!(Version::from_str("0.0."), None);
    }
}
