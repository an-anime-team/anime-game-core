use crate::downloader::unpack::*;

pub const ZIP_FILES: &[(&str, EntrySize)] = &[
    ("Cargo.lock", EntrySize::Both { compressed: 2464, uncompressed: 8905 }),
    ("Cargo.toml", EntrySize::Both { compressed: 208, uncompressed: 296 }),
    ("LICENSE", EntrySize::Both { compressed: 12118, uncompressed: 35147 }),
    ("README.md", EntrySize::Both { compressed: 311, uncompressed: 581 })
];

pub const ZIP_SIZE: EntrySize = EntrySize::Both { compressed: 15101, uncompressed: 44929 };

pub const TAR_FILES: &[(&str, EntrySize)] = &[
    ("Cargo.lock", EntrySize::Uncompressed(8905)),
    ("Cargo.toml", EntrySize::Uncompressed(296)),
    ("LICENSE", EntrySize::Uncompressed(35147)),
    ("README.md", EntrySize::Uncompressed(581))
];

pub const TAR_SIZE: EntrySize = EntrySize::Uncompressed(44929);

#[test]
fn test_zip_info() {
    let archive = Archive::open("src/tests/test.zip").expect("test.zip not found");

    assert_eq!(archive.get_type(), ArchiveType::Zip);

    let info = archive.get_info().expect("Failed loading archive info");

    assert_eq!(info.size, ZIP_SIZE);
    assert_eq!(info.files.len(), ZIP_FILES.len());

    let mut found = 0;

    for archive_file in info.files {
        for test_file in ZIP_FILES {
            if archive_file.path == test_file.0 {
                assert_eq!(archive_file.size, test_file.1);

                found += 1;

                break;
            }
        }
    }

    assert_eq!(ZIP_FILES.len(), found);
}

#[test]
fn test_tar_info() {
    let archive = Archive::open("src/tests/test.tar.xz").expect("test.tar.xz not found");

    assert_eq!(archive.get_type(), ArchiveType::Tar);

    let info = archive.get_info().expect("Failed loading archive info");

    assert_eq!(info.size, TAR_SIZE);
    assert_eq!(info.files.len(), TAR_FILES.len());

    let mut found = 0;

    for archive_file in info.files {
        for test_file in TAR_FILES {
            if archive_file.path == test_file.0 {
                assert_eq!(archive_file.size, test_file.1);

                found += 1;

                break;
            }
        }
    }

    assert_eq!(TAR_FILES.len(), found);
}
