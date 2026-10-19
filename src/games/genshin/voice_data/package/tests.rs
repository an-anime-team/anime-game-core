use super::*;

fn package(root: &Path) -> VoicePackage {
    let path = get_voice_package_path(root, GameEdition::Global, VoiceLocale::English);
    std::fs::create_dir_all(&path).unwrap();
    VoicePackage::new(path, GameEdition::Global).unwrap()
}

#[test]
fn unmarked_package_without_official_index_is_not_assumed_latest() {
    let root = tempfile::tempdir().unwrap();
    let package = package(root.path());
    let error = package.try_get_version().unwrap_err().to_string();
    assert!(error.contains("Could not determine the installed English voice version"));
    assert!(error.contains("Audio_English(US)_pkg_version"));
}

#[test]
fn explicit_voice_version_takes_priority() {
    let root = tempfile::tempdir().unwrap();
    let package = package(root.path());
    let VoicePackage::Installed { path, .. } = &package else { unreachable!() };
    for version in [&b"6.7.0\n"[..], &[6, 7, 0][..]] {
        std::fs::write(path.join(".version"), version).unwrap();
        assert_eq!(package.try_get_version().unwrap(), Version::new(6, 7, 0));
    }
}

#[test]
fn an_unreadable_version_marker_is_not_treated_as_missing() {
    let root = tempfile::tempdir().unwrap();
    let package = package(root.path());
    let VoicePackage::Installed { path, .. } = &package else { unreachable!() };
    std::fs::create_dir(path.join(".version")).unwrap();
    let error = package.try_get_version().unwrap_err();
    assert!(error.downcast_ref::<std::io::Error>().is_some());
}
