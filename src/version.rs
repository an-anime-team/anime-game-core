#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub version: [u8; 3]
}

impl Version {
    pub fn new(a: u8, b: u8, c: u8) -> Version {
        Version {
            version: [a, b, c]
        }
    }

    pub fn from_str(str: &str) -> Version {
        let str = str.as_bytes();

        Version::new(str[0] - 48, str[2] - 48, str[4] - 48)
    }

    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.version[0], self.version[1], self.version[2])
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
