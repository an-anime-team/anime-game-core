use crate::prelude::*;

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
