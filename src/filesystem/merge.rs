use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    File {
        name: OsString,
        path: PathBuf
    },
    Folder {
        name: OsString,
        path: PathBuf,
        children: MergeTree
    }
}

impl Entry {
    #[inline]
    pub fn is_file(&self) -> bool {
        matches!(self, Self::File { .. })
    }

    #[inline]
    pub fn is_dir(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }

    #[inline]
    pub fn name(&self) -> &OsStr {
        match self {
            Self::File { name, .. } |
            Self::Folder { name, .. } => name
        }
    }

    #[inline]
    pub fn path(&self) -> &Path {
        match self {
            Self::File { path, .. } |
            Self::Folder { path, .. } => path
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct MergeTree {
    entries: HashMap<OsString, Entry>
}

impl MergeTree {
    pub fn create(base_folder: impl AsRef<Path>) -> std::io::Result<Self> {
        let mut tree = HashMap::new();

        let base_folder = base_folder.as_ref();

        assert!(base_folder.is_dir());

        for entry in base_folder.read_dir()?.flatten() {
            if entry.path().is_file() {
                tree.insert(entry.file_name(), Entry::File {
                    name: entry.file_name(),
                    path: entry.path()
                });
            }

            else {
                tree.insert(entry.file_name(), Entry::Folder {
                    name: entry.file_name(),
                    path: entry.path(),
                    children: Self::default()
                });
            }
        }

        Ok(Self {
            entries: tree
        })
    }

    pub fn add_layer(&mut self, layer_folder: impl AsRef<Path>) -> std::io::Result<()> {
        let layer_folder = layer_folder.as_ref();

        assert!(layer_folder.is_dir());

        for entry in layer_folder.read_dir()?.flatten() {
            if let Some(base_entry) = self.entries.get_mut(&entry.file_name()) {
                if base_entry.is_file() && entry.path().is_file() {
                    *base_entry = Entry::File {
                        name: entry.file_name(),
                        path: entry.path()
                    };
                }

                else if base_entry.is_dir() && entry.path().is_dir() {
                    // Always happens
                    if let Entry::Folder { path, children, .. } = base_entry {
                        if children.is_empty() {
                            *children = Self::create(path)?;
                        }

                        children.add_layer(entry.path())?;
                    }
                }

                else {
                    todo!()
                }
            }

            else if entry.path().is_file() {
                self.entries.insert(entry.file_name(), Entry::File {
                    name: entry.file_name(),
                    path: entry.path()
                });
            }

            else {
                self.entries.insert(entry.file_name(), Entry::Folder {
                    name: entry.file_name(),
                    path: entry.path(),
                    children: Self::default()
                });
            }
        }

        Ok(())
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn mount(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();

        for entry in self.entries.values() {
            let entry_path = entry.path().to_path_buf();
            let link_path = path.join(entry.name());

            if entry.is_file() {
                std::os::unix::fs::symlink(entry_path, link_path)?;
            }

            else if let Entry::Folder { children, .. } = entry {
                if children.is_empty() {
                    std::os::unix::fs::symlink(entry_path, link_path)?;
                }

                else {
                    std::fs::create_dir_all(&link_path)?;

                    children.mount(link_path)?;
                }
            }
        }

        Ok(())
    }
}
