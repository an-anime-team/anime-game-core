#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Too few version numbers in string {0}")]
    TooFewNumbers(String),

    #[error("Too many version numbers in string {0}")]
    TooManyNumbers(String),

    #[error("Failed to parse u8 number from string {0}")]
    NumberParseError(String)
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub edition: u8
}

impl Version {
    #[inline]
    pub fn new(major: u8, minor: u8, patch: u8, edition: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            edition
        }
    }
}

impl std::str::FromStr for Version {
    type Err = Error;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        let numbers = version.split('.').collect::<Vec<_>>();

        if numbers.len() > 4 {
            Err(Error::TooManyNumbers(version.to_string()))
        }

        else if numbers.is_empty() {
            Err(Error::TooFewNumbers(version.to_string()))
        }

        else {
            let Ok(major) = numbers[0].parse::<u8>() else {
                return Err(Error::NumberParseError(numbers[0].to_string()));
            };

            if numbers.len() == 1 {
                Ok(Self {
                    major,
                    minor: 0,
                    patch: 0,
                    edition: 0
                })
            }

            else {
                let Ok(minor) = numbers[1].parse::<u8>() else {
                    return Err(Error::NumberParseError(numbers[1].to_string()));
                };

                if numbers.len() == 2 {
                    Ok(Self {
                        major,
                        minor,
                        patch: 0,
                        edition: 0
                    })
                }

                else {
                    let Ok(patch) = numbers[2].parse::<u8>() else {
                        return Err(Error::NumberParseError(numbers[2].to_string()));
                    };

                    if numbers.len() == 3 {
                        Ok(Self {
                            major,
                            minor,
                            patch,
                            edition: 0
                        })
                    }

                    else {
                        let Ok(edition) = numbers[3].parse::<u8>() else {
                            return Err(Error::NumberParseError(numbers[3].to_string()));
                        };
    
                        Ok(Self {
                            major,
                            minor,
                            patch,
                            edition
                        })
                    }
                }
            }
        }
    }
}

impl std::fmt::Display for Version {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.major, self.minor, self.patch, self.edition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() -> Result<(), Error> {
        assert_eq!("0.0.0.0".parse(), Ok(Version::new(0, 0, 0, 0)));
        assert_eq!("0.0.0.1".parse(), Ok(Version::new(0, 0, 0, 1)));
        assert_eq!("0.0.1.0".parse(), Ok(Version::new(0, 0, 1, 0)));
        assert_eq!("0.0.1.1".parse(), Ok(Version::new(0, 0, 1, 1)));
        assert_eq!("0.1.0.0".parse(), Ok(Version::new(0, 1, 0, 0)));
        assert_eq!("0.1.0.1".parse(), Ok(Version::new(0, 1, 0, 1)));
        assert_eq!("0.1.1.0".parse(), Ok(Version::new(0, 1, 1, 0)));
        assert_eq!("0.1.1.1".parse(), Ok(Version::new(0, 1, 1, 1)));
        assert_eq!("1.0.0.0".parse(), Ok(Version::new(1, 0, 0, 0)));
        assert_eq!("1.0.0.1".parse(), Ok(Version::new(1, 0, 0, 1)));
        assert_eq!("1.0.1.0".parse(), Ok(Version::new(1, 0, 1, 0)));
        assert_eq!("1.0.1.1".parse(), Ok(Version::new(1, 0, 1, 1)));
        assert_eq!("1.1.0.0".parse(), Ok(Version::new(1, 1, 0, 0)));
        assert_eq!("1.1.0.1".parse(), Ok(Version::new(1, 1, 0, 1)));
        assert_eq!("1.1.1.0".parse(), Ok(Version::new(1, 1, 1, 0)));
        assert_eq!("1.1.1.1".parse(), Ok(Version::new(1, 1, 1, 1)));

        assert_eq!("255.255.255.255".parse(), Ok(Version::new(255, 255, 255, 255)));

        assert_eq!("1.2.3.4".parse(), Ok(Version::new(1, 2, 3, 4)));
        assert_eq!("1.2.3".parse(),   Ok(Version::new(1, 2, 3, 0)));
        assert_eq!("1.2".parse(),     Ok(Version::new(1, 2, 0, 0)));
        assert_eq!("1".parse(),       Ok(Version::new(1, 0, 0, 0)));

        assert!("example string".parse::<Version>().is_err());
        assert!("1.2.3.4.5".parse::<Version>().is_err());
        assert!("256.256.256.256".parse::<Version>().is_err());

        Ok(())
    }
}
