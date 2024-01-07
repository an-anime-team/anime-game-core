use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Merge {
    pub base_path: PathBuf,
    pub layers: Vec<PathBuf>
}

impl Merge {
    pub fn new(base_path: impl Into<PathBuf>, layers: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self {
            base_path: base_path.into(),
            layers: layers.into_iter()
                .map(|path| path.into())
                .collect()
        }
    }

    // pub fn create(&self, output_path: impl AsRef<Path>) {
    //     // mount -t overlay overlay -o lowerdir=/lower1:/lower2:/lower3,upperdir=/upper,workdir=/work /merged
    //     Command::new("mount")
    // }
}
