use std::collections::HashMap;

use kinda_virtual_fs::*;

pub mod hpatchz;

lazy_static::lazy_static! {
    static ref STORAGE: Storage = Storage::new(HashMap::from([
        (String::from("hpatchz"), include_bytes!("../../builtin/HDiffPatch/hpatchz").to_vec().into())
    ]));
}
