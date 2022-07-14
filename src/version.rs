use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub version: [u8; 3]
}

impl Version {
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

    /// Converts `Version` struct to readable format (e.g. "1.2.3")
    /// 
    /// ```
    /// use anime_game_core::prelude::Version;
    /// 
    /// assert_eq!(Version::new(1, 2, 3).to_string(), "1.2.3");
    /// ```
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.version[0], self.version[1], self.version[2])
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
    fn eq(&self, other: &String) -> bool {
        &self.to_string() == other
    }
}

impl PartialEq<Version> for String {
    fn eq(&self, other: &Version) -> bool {
        self == &other.to_string()
    }
}

impl PartialEq<&str> for Version {
    fn eq(&self, other: &&str) -> bool {
        &self.to_string() == other
    }
}

impl PartialEq<Version> for &str {
    fn eq(&self, other: &Version) -> bool {
        self == &other.to_string()
    }
}

pub trait ToVersion {
    fn to_version(&self) -> Option<Version>;
}

impl<T> ToVersion for T where T: ToString {
    fn to_version(&self) -> Option<Version> {
        Version::from_str(self.to_string())
    }
}
