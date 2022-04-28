use crate::Version;

mod archives;

#[test]
fn test_version_from_str() {
    assert_eq!(Version::from_str("1.2.3").to_string(), "1.2.3");
}
