use std::path::PathBuf;

use sysinfo::{System, SystemExt, DiskExt};

// TODO: support for relative paths

/// Get available free disk space by specified path
/// 
/// Can return `None` if path is not prefixed by any available disk
#[tracing::instrument(level = "trace")]
pub fn available<T: Into<PathBuf> + std::fmt::Debug>(path: T) -> Option<u64> {
    let mut system = System::new_all();

    system.sort_disks_by(|a, b| {
        let a = a.mount_point().as_os_str().len();
        let b = b.mount_point().as_os_str().len();

        a.cmp(&b).reverse()
    });

    let path: PathBuf = path.into();

    for disk in system.disks() {
        if path.starts_with(disk.mount_point()) {
            return Some(disk.available_space());
        }
    }

    None
}

/// Check if two paths exist on the same disk
#[tracing::instrument(level = "trace")]
pub fn is_same_disk<T: Into<PathBuf> + std::fmt::Debug>(path1: T, path2: T) -> bool {
    let mut system = System::new_all();

    system.sort_disks_by(|a, b| {
        let a = a.mount_point().as_os_str().len();
        let b = b.mount_point().as_os_str().len();

        a.cmp(&b).reverse()
    });

    let path1: PathBuf = path1.into();
    let path2: PathBuf = path2.into();

    for disk in system.disks() {
        let disk_path = disk.mount_point();

        if path1.starts_with(disk_path) && path2.starts_with(disk_path) {
            return true;
        }
    }

    false
}
