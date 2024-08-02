use serde::{Serialize, Deserialize};

use std::fmt::{Debug, Display, Formatter};
use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version {
    pub version: [u8; 3]
}

impl Version {
    #[inline]
    pub fn new(a: u8, b: u8, c: u8) -> Self {
        Self {
            version: [a, b, c]
        }
    }

    #[allow(clippy::should_implement_trait)]
    /// Get `Version` from the string
    /// 
    /// ```
    /// use anime_game_core::prelude::Version;
    /// 
    /// let version = Version::from_str("1.10.2").expect("Failed to parse version string");
    /// ```
    pub fn from_str<T: AsRef<str>>(str: T) -> Option<Self> {
        let parts = str.as_ref().split('.').collect::<Vec<&str>>();

        if parts.len() != 3 {
            return None;
        }

        if let (Ok(a), Ok(b), Ok(c)) = (parts[0].parse(), parts[1].parse(), parts[2].parse()) {
            return Some(Version::new(a, b, c));
        }

        None
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

impl Debug for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.version[0], self.version[1], self.version[2])
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.version[0], self.version[1], self.version[2])
    }
}

// Equality with strings

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

// Comparison with strings

impl PartialOrd<String> for Version {
    fn partial_cmp(&self, other: &String) -> Option<Ordering> {
        self.to_string().partial_cmp(other)
    }
}

impl PartialOrd<Version> for String {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        self.partial_cmp(&other.to_string())
    }
}

impl PartialOrd<&str> for Version {
    fn partial_cmp(&self, other: &&str) -> Option<Ordering> {
        self.to_string()
            .as_str()
            .partial_cmp(other)
    }
}

impl PartialOrd<Version> for &str {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        self.partial_cmp(&other.to_string().as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_new() {
        let version = Version::new(0, 0, 0);

        assert_eq!(version, "0.0.0");
        assert_eq!(version, "0.0.0".to_string());
        assert_eq!(Some(version), Version::from_str("0.0.0"));
        assert_eq!(version.to_plain_string(), "000".to_string());
    }

    #[test]
    fn test_version_from_str() {
        let version = Version::from_str("0.0.0");

        assert!(version.is_some());

        let version = version.unwrap();

        assert_eq!(version, "0.0.0");
        assert_eq!(version, "0.0.0".to_string());
        assert_eq!(version, Version::new(0, 0, 0));
        assert_eq!(version.to_plain_string(), "000".to_string());
    }

    #[test]
    fn test_version_long() {
        let version = Version::from_str("100.0.255");

        assert!(version.is_some());

        let version = version.unwrap();

        assert_eq!(version, "100.0.255");
        assert_eq!(version, "100.0.255".to_string());
        assert_eq!(version, Version::new(100, 0, 255));
        assert_eq!(version.to_plain_string(), "1000255".to_string());
    }

    #[test]
    fn test_incorrect_versions() {
        assert_eq!(Version::from_str(""), None);
        assert_eq!(Version::from_str("..0"), None);
        assert_eq!(Version::from_str("0.0."), None);
    }

    #[test]
    #[allow(clippy::cmp_owned)]
    fn test_version_comparison() {
        assert!(Version::new(1, 0, 1) > "1.0.0");
        assert!(Version::new(1, 0, 0) < "1.0.1");

        assert!("1.0.0" < Version::new(1, 0, 1));
        assert!("1.0.1" > Version::new(1, 0, 0));

        assert!(Version::new(1, 0, 1) > String::from("1.0.0"));
        assert!(Version::new(1, 0, 0) < String::from("1.0.1"));

        assert!(String::from("1.0.0") < Version::new(1, 0, 1));
        assert!(String::from("1.0.1") > Version::new(1, 0, 0));

        assert!(Version::new(1, 0, 0) == "1.0.0");
        assert!("1.0.0" == Version::new(1, 0, 0));

        assert!(Version::new(1, 0, 0) == String::from("1.0.0"));
        assert!(String::from("1.0.0") == Version::new(1, 0, 0));
    }
}
