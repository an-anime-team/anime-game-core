use std::os::unix::fs::MetadataExt;
use std::path::Path;

use sysinfo::Disks;

// TODO: support for relative paths

/// Get available free disk space by specified path
///
/// Can return `None` if path is not prefixed by any available disk
pub fn available(path: impl AsRef<Path>) -> Option<u64> {
    let disks = Disks::new_with_refreshed_list();

    let Some(meta) = path
        .as_ref()
        .ancestors()
        .find_map(|parent_path| parent_path.metadata().ok())
    else {
        tracing::error!(path = ?path.as_ref(), "Could not find metadata for any of the ancestors of the path");
        return None;
    };
    let devno = meta.dev();

    for disk in disks.iter() {
        let disk_meta = disk.mount_point().metadata();
        if disk_meta.is_ok_and(|m| m.dev() == devno) {
            return Some(disk.available_space());
        }
    }

    None
}

/// Check if two paths are contained on the same device
pub fn is_same_disk(path1: impl AsRef<Path>, path2: impl AsRef<Path>) -> bool {
    let Some(dev1) = path1.as_ref().metadata().ok()
    else {
        return false;
    };
    let Some(dev2) = path2.as_ref().metadata().ok()
    else {
        return false;
    };

    // The semantics here aren't exactly the same as the old code (is_same_mount):
    // This tests if the path are on the same device specifically,
    // not that the mount point is the same.
    dev1.dev() == dev2.dev()
}

/// Check if two paths share the same mount point
pub fn is_same_mount(path1: impl AsRef<Path>, path2: impl AsRef<Path>) -> bool {
    let mut disks = Disks::new_with_refreshed_list();

    disks.sort_by(|a, b| {
        let a = a.mount_point().as_os_str().len();
        let b = b.mount_point().as_os_str().len();

        a.cmp(&b).reverse()
    });

    let path1 = path1
        .as_ref()
        .read_link()
        .unwrap_or_else(|_| path1.as_ref().to_path_buf());

    let path2 = path2
        .as_ref()
        .read_link()
        .unwrap_or_else(|_| path2.as_ref().to_path_buf());

    for disk in disks.iter() {
        let disk_path = disk.mount_point();

        if path1.starts_with(disk_path) && path2.starts_with(disk_path) {
            return true;
        }
    }

    false
}
