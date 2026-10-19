use super::*;
use crate::sophon::protos::{SophonPatchAssetChunk, SophonPatchAssetProperty, SophonPatchProto};
use std::collections::HashMap;

const VOICE_INDEX: &str = "Audio_English(US)_pkg_version";

fn manifest() -> SophonPatchProto {
    SophonPatchProto {
        patch_assets: vec![SophonPatchAssetProperty {
            asset_name: VOICE_INDEX.into(),
            asset_patch_chunks: HashMap::from([
                ("6.7.0".into(), SophonPatchAssetChunk {
                    original_file_md5: "25880e9015e5a58a14a9c047a2a28743".into(),
                    ..Default::default()
                }),
                ("7.0.0".into(), SophonPatchAssetChunk {
                    original_file_md5: "5286f45e1a6769283844811f1bebf1c9".into(),
                    ..Default::default()
                }),
            ]),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn identifies_imported_voice_index_from_previous_versions() {
    assert_eq!(
        version_from_patch_manifest(&manifest(), VOICE_INDEX, "25880E9015E5A58A14A9C047A2A28743"),
        Some(Version::new(6, 7, 0))
    );
    assert_eq!(
        version_from_patch_manifest(&manifest(), VOICE_INDEX, "5286f45e1a6769283844811f1bebf1c9"),
        Some(Version::new(7, 0, 0))
    );
}

#[test]
fn unknown_hash_or_another_voice_locale_has_no_guessed_version() {
    assert_eq!(version_from_patch_manifest(&manifest(), VOICE_INDEX, "unknown"), None);
    assert_eq!(
        version_from_patch_manifest(&manifest(), "Audio_Japanese_pkg_version", "25880e9015e5a58a14a9c047a2a28743"),
        None
    );
}

#[test]
fn identical_hashes_choose_the_newest_compatible_version() {
    let mut manifest = manifest();
    manifest.patch_assets[0].asset_patch_chunks.get_mut("7.0.0").unwrap().original_file_md5 =
        "25880e9015e5a58a14a9c047a2a28743".into();
    assert_eq!(
        version_from_patch_manifest(&manifest, VOICE_INDEX, "25880e9015e5a58a14a9c047a2a28743"),
        Some(Version::new(7, 0, 0))
    );
}

#[test]
fn game_executables_use_the_same_hash_lookup() {
    let mut manifest = manifest();
    manifest.patch_assets[0].asset_name = "GenshinImpact.exe".into();
    assert_eq!(
        version_from_patch_manifest(&manifest, "GenshinImpact.exe", "25880e9015e5a58a14a9c047a2a28743"),
        Some(Version::new(6, 7, 0))
    );
}
