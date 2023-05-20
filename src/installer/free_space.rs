use std::path::Path;

use sysinfo::{System, SystemExt, DiskExt};

// TODO: support for relative paths

/// Get available free disk space by specified path
/// 
/// Can return `None` if path is not prefixed by any available disk
pub fn available(path: impl AsRef<Path>) -> Option<u64> {
    let mut system = System::new();

    system.refresh_disks_list();
    system.refresh_disks();

    system.sort_disks_by(|a, b| {
        let a = a.mount_point().as_os_str().len();
        let b = b.mount_point().as_os_str().len();

        a.cmp(&b).reverse()
    });

    let path = path.as_ref();

    for disk in system.disks() {
        if path.starts_with(disk.mount_point()) {
            return Some(disk.available_space());
        }
    }

    None
}

/// Check if two paths exist on the same disk
pub fn is_same_disk(path1: impl AsRef<Path>, path2: impl AsRef<Path>) -> bool {
    let mut system = System::new();

    system.refresh_disks_list();
    system.refresh_disks();

    system.sort_disks_by(|a, b| {
        let a = a.mount_point().as_os_str().len();
        let b = b.mount_point().as_os_str().len();

        a.cmp(&b).reverse()
    });

    let path1 = path1.as_ref();
    let path2 = path2.as_ref();

    for disk in system.disks() {
        let disk_path = disk.mount_point();

        if path1.starts_with(disk_path) && path2.starts_with(disk_path) {
            return true;
        }
    }

    false
}
