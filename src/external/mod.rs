pub mod hpatchz;

use std::collections::HashMap;

use kinda_virtual_fs::*;

lazy_static::lazy_static! {
    static ref STORAGE: Storage = Storage::new(HashMap::from([
        ("hpatchz".to_string(), Entry::new(include_bytes!("../../external/hpatchz/hpatchz").to_vec()))
    ]));
}
