use sysinfo::{System, SystemExt, DiskExt};

/// Get available free disk space by specified path
/// 
/// Can return `None` if path is not prefixed by any available disk
pub fn available(path: &str) -> Option<u64> {
    let mut system = System::new_all();

    system.sort_disks_by(|a, b| {
        let a = a.mount_point().as_os_str().len();
        let b = b.mount_point().as_os_str().len();

        a.cmp(&b).reverse()
    });

    for disk in system.disks() {
        let disk_path = disk.mount_point().as_os_str();

        if path.len() >= disk_path.len() && &path[..disk_path.len()] == disk_path {
            return Some(disk.available_space());
        }
    }

    None
}

/// Check if two paths exist on the same disk
pub fn is_same_disk(path1: &str, path2: &str) -> bool {
    let mut system = System::new_all();

    system.sort_disks_by(|a, b| {
        let a = a.mount_point().as_os_str().len();
        let b = b.mount_point().as_os_str().len();

        a.cmp(&b).reverse()
    });

    for disk in system.disks() {
        let disk_path = disk.mount_point().as_os_str();

        if path1.len() > disk_path.len() &&
           path2.len() > disk_path.len() &&
           &path1[..disk_path.len()] == disk_path &&
           &path2[..disk_path.len()] == disk_path
        {
            return true;
        }
    }

    false
}
