use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub version: [u8; 3]
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

impl Version {
    pub fn new(a: u8, b: u8, c: u8) -> Version {
        Version {
            version: [a, b, c]
        }
    }

    // TODO: long versions support (1111.222222.3333333) and format checking
    pub fn from_str<T: ToString>(str: T) -> Version {
        // I had to write it like that
        let str = str.to_string();
        let str = str.as_bytes();

        Version::new(str[0] - 48, str[2] - 48, str[4] - 48)
    }

    /// Converts `Version` struct to readable format (e.g. "1.2.3")
    /// 
    /// ```
    /// assert_eq!(Version::new(1, 2, 3).to_string(), "1.2.3");
    /// ```
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.version[0], self.version[1], self.version[2])
    }

    /// Converts `Version` struct to plain format (e.g. "123")
    /// 
    /// ```
    /// assert_eq!(Version::new(1, 2, 3).to_plain_string(), "123");
    /// ```
    pub fn to_plain_string(&self) -> String {
        format!("{}{}{}", self.version[0], self.version[1], self.version[2])
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
