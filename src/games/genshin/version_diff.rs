use std::path::{Path, PathBuf};
use std::os::unix::prelude::PermissionsExt;

use serde::{Serialize, Deserialize};
use thiserror::Error;

use super::consts::GameEdition;

use crate::version::Version;
use crate::traits::version_diff::VersionDiffExt;

#[cfg(feature = "install")]
use crate::{
    installer::{
        downloader::{Downloader, DownloadingError},
        installer::Update as InstallerUpdate,
        free_space,
        archives::Archive
    },
    external::hpatchz
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffUpdate {
    CheckingFreeSpace(PathBuf),

    InstallerUpdate(InstallerUpdate),

    ApplyingHdiffStarted,
    ApplyingHdiffProgress(u64, u64),
    ApplyingHdiffFinished,

    RemovingOutdatedStarted,
    RemovingOutdatedProgress(u64, u64),
    RemovingOutdatedFinished
}

impl From<InstallerUpdate> for DiffUpdate {
    #[inline]
    fn from(update: InstallerUpdate) -> Self {
        Self::InstallerUpdate(update)
    }
}

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffDownloadingError {
    /// Your installation is already up to date and not needed to be updated
    #[error("Component version is already latest")]
    AlreadyLatest,

    /// Current version is too outdated and can't be updated.
    /// It means that you have to download everything from zero
    #[error("Components version is too outdated and can't be updated")]
    Outdated,

    /// When there's multiple urls and you can't save them as a single file
    #[error("Component has multiple downloading urls and can't be saved as a single file")]
    MultipleSegments,

    /// Failed to fetch remove data. Redirected from `Downloader`
    #[error("{0}")]
    DownloadingError(#[from] DownloadingError),

    /// Failed to apply hdiff patch
    #[error("Failed to apply hdiff patch: {0}")]
    HdiffPatch(String),

    /// Installation path wasn't specified. This could happen when you
    /// try to call `install` method on `VersionDiff` that was generated
    /// in `VoicePackage::list_latest`. This method couldn't know
    /// your game installation path and thus indicates that it doesn't know
    /// where this package needs to be installed
    #[error("Path to the component's downloading folder is not specified")]
    PathNotSpecified
}

impl From<minreq::Error> for DiffDownloadingError {
    fn from(error: minreq::Error) -> Self {
        DownloadingError::Minreq(error.to_string()).into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionDiff {
    /// Latest version
    Latest {
        version: Version,
        edition: GameEdition
    },

    /// Component's update can be predownloaded, but you still can use it
    Predownload {
        current: Version,
        latest: Version,

        uri: String,
        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        /// 
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        installation_path: Option<PathBuf>,

        /// Optional path to the `.version` file
        version_file_path: Option<PathBuf>,

        /// Temp folder path
        temp_folder: Option<PathBuf>
    },

    /// Component should be updated before using it
    Diff {
        current: Version,
        latest: Version,

        uri: String,
        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        /// 
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        installation_path: Option<PathBuf>,

        /// Optional path to the `.version` file
        version_file_path: Option<PathBuf>,

        /// Temp folder path
        temp_folder: Option<PathBuf>
    },

    /// Difference can't be calculated because installed version is too old
    Outdated {
        current: Version,
        latest: Version,
        edition: GameEdition
    },

    /// Component is not yet installed
    NotInstalled {
        latest: Version,
        segments_uris: Vec<String>,
        edition: GameEdition,

        downloaded_size: u64,
        unpacked_size: u64,

        /// Path to the folder this difference should be installed by the `install` method
        /// 
        /// This value can be `None`, so `install` will return `Err(DiffDownloadError::PathNotSpecified)`
        installation_path: Option<PathBuf>,

        /// Optional path to the `.version` file
        version_file_path: Option<PathBuf>,

        /// Temp folder path
        temp_folder: Option<PathBuf>
    }
}

impl VersionDiff {
    /// Get `.version` file path
    pub fn version_file_path(&self) -> Option<PathBuf> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { version_file_path, .. } |
            Self::Diff { version_file_path, .. } |
            Self::NotInstalled { version_file_path, .. } => version_file_path.to_owned()
        }
    }

    /// Return currently selected temp folder path
    /// 
    /// Default is `std::env::temp_dir()` value
    pub fn temp_folder(&self) -> PathBuf {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => std::env::temp_dir(),

            // Can be installed
            Self::Predownload { temp_folder, .. } |
            Self::Diff { temp_folder, .. } |
            Self::NotInstalled { temp_folder, .. } => match temp_folder {
                Some(path) => path.to_owned(),
                None => std::env::temp_dir()
            }
        }
    }

    pub fn with_temp_folder(mut self, temp: PathBuf) -> Self {
        match &mut self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => self,

            // Can be installed
            Self::Predownload { temp_folder, .. } => {
                *temp_folder = Some(temp);

                self
            }

            Self::Diff { temp_folder, .. } => {
                *temp_folder = Some(temp);

                self
            }

            Self::NotInstalled { temp_folder, .. } => {
                *temp_folder = Some(temp);

                self
            }
        }
    }
}

impl VersionDiffExt for VersionDiff {
    type Error = DiffDownloadingError;
    type Update = DiffUpdate;
    type Edition = GameEdition;

    fn edition(&self) -> GameEdition {
        match self {
            Self::Latest { edition, .. } |
            Self::Predownload { edition, .. } |
            Self::Diff { edition, .. } |
            Self::Outdated { edition, .. } |
            Self::NotInstalled { edition, .. } => *edition
        }
    }

    fn current(&self) -> Option<Version> {
        match self {
            Self::Latest { version: current, .. } |
            Self::Predownload { current, .. } |
            Self::Diff { current, .. } |
            Self::Outdated { current, .. } => Some(*current),

            Self::NotInstalled { .. } => None
        }
    }

    fn latest(&self) -> Version {
        match self {
            Self::Latest { version: latest, .. } |
            Self::Predownload { latest, .. } |
            Self::Diff { latest, .. } |
            Self::Outdated { latest, .. } |
            Self::NotInstalled { latest, .. } => *latest
        }
    }

    fn downloaded_size(&self) -> Option<u64> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { downloaded_size, .. } |
            Self::Diff { downloaded_size, .. } |
            Self::NotInstalled { downloaded_size, .. } => Some(*downloaded_size)
        }
    }

    fn unpacked_size(&self) -> Option<u64> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { unpacked_size, .. } |
            Self::Diff { unpacked_size, .. } |
            Self::NotInstalled { unpacked_size, .. } => Some(*unpacked_size)
        }
    }

    fn installation_path(&self) -> Option<&Path> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { installation_path, .. } |
            Self::Diff { installation_path, .. } |
            Self::NotInstalled { installation_path, .. } => match installation_path {
                Some(path) => Some(path.as_path()),
                None => None
            }
        }
    }

    fn downloading_uri(&self) -> Option<String> {
        match self {
            // Can't be installed
            Self::Latest { .. } |
            Self::Outdated { .. } => None,

            // Can be installed
            Self::Predownload { uri, .. } |
            Self::Diff { uri, .. } => Some(uri.to_owned()),

            // Can be installed but amogus
            Self::NotInstalled { .. } => None
        }
    }

    fn download_as(&mut self, path: impl AsRef<Path>, progress: impl Fn(u64, u64) + Send + 'static) -> Result<(), Self::Error> {
        tracing::debug!("Downloading version difference");

        let mut downloader = Downloader::new(match self {
            // Can't be downloaded
            Self::Latest { .. } => return Err(Self::Error::AlreadyLatest),
            Self::Outdated { .. } => return Err(Self::Error::Outdated),

            // Can be downloaded
            Self::Predownload { uri, .. } |
            Self::Diff { uri, .. } => uri,

            // Can be installed but amogus
            Self::NotInstalled { .. } => return Err(Self::Error::MultipleSegments)
        })?;

        if let Err(err) = downloader.download(path.as_ref(), progress) {
            tracing::error!("Failed to download version difference: {err}");

            return Err(err.into());
        }

        Ok(())
    }

    fn install_to(&self, path: impl AsRef<Path>, updater: impl Fn(Self::Update) + Clone + Send + 'static) -> Result<(), Self::Error> {
        tracing::debug!("Installing version difference");

        let uris = match self {
            // Can't be installed
            Self::Latest { .. } => return Err(Self::Error::AlreadyLatest),
            Self::Outdated { .. } => return Err(Self::Error::Outdated),

            // Can be installed
            Self::Predownload { uri, .. } |
            Self::Diff { uri, .. } => vec![uri.to_owned()],

            Self::NotInstalled { segments_uris, .. } => segments_uris.to_owned()
        };

        let path = path.as_ref().to_path_buf();
        let temp_folder = self.temp_folder();

        let downloaded_size = self.downloaded_size().expect("Failed to retrieve downloaded size");
        let unpacked_size = self.unpacked_size().expect("Failed to retrieve unpacked size");

        (updater)(DiffUpdate::CheckingFreeSpace(temp_folder.clone()));

        // Check available free space for archive itself
        let Some(space) = free_space::available(&temp_folder) else {
            tracing::error!("Path is not mounted: {:?}", temp_folder);

            return Err(DownloadingError::PathNotMounted(temp_folder).into());
        };

        // We can possibly store downloaded archive + unpacked data on the same disk
        let required = if free_space::is_same_disk(&temp_folder, &path) {
            downloaded_size + unpacked_size
        } else {
            downloaded_size
        };

        if space < required {
            tracing::error!("No free space available in the temp folder. Required: {required}. Available: {space}");

            return Err(DownloadingError::NoSpaceAvailable(temp_folder, required, space).into());
        }

        (updater)(DiffUpdate::CheckingFreeSpace(path.clone()));

        // Check available free space for unpacked archive data
        let Some(space) = free_space::available(&path) else {
            tracing::error!("Path is not mounted: {:?}", &path);

            return Err(DownloadingError::PathNotMounted(path.to_path_buf()).into());
        };

        // We can possibly store downloaded archive + unpacked data on the same disk
        let required = if free_space::is_same_disk(&path, &temp_folder) {
            unpacked_size + downloaded_size
        } else {
            unpacked_size
        };

        if space < required {
            tracing::error!("No free space available in the installation folder. Required: {required}. Available: {space}");

            return Err(DownloadingError::NoSpaceAvailable(path.to_path_buf(), required, space).into());
        }

        let mut current_downloaded = 0;
        let mut segments_names = Vec::new();

        // Imitate Installer update message
        (updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::DownloadingStarted(temp_folder.to_path_buf())));

        // Download segments
        for uri in uris {
            let installer_updater = updater.clone();

            let mut downloader = Downloader::new(uri)?
                // Don't perform space checks because we've already done it
                .with_free_space_check(false);

            let local_total = downloader.length().unwrap();
            let segment_name = downloader.get_filename().to_string();

            // Download segment
            downloader.download(temp_folder.join(&segment_name), move |current, _| {
                (installer_updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::DownloadingProgress(
                    current_downloaded + current,
                    downloaded_size
                )));
            })?;

            segments_names.push(segment_name);

            current_downloaded += local_total;
        }

        // Report 100% download progress (just in case)
        (updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::DownloadingProgress(downloaded_size, downloaded_size)));

        let first_segment_name = segments_names[0].clone();

        // Imitate Installer update message
        (updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::DownloadingFinished));

        // Extract downloaded segments
        // Ctrl+C / Ctrl+V from the Installer. Not a good approach,
        // but current core library is somehow legacy as I already started work
        // on a full rewrite so this code won't stay here for always
        match Archive::open(temp_folder.join(&first_segment_name)) {
            Ok(mut archive) => {
                // Temporary workaround as we can't get archive extraction process
                // directly - we'll spawn it in another thread and check this archive entries appearance in the filesystem
                let mut total = 0;

                let entries = archive
                    .get_entries()
                    .expect("Failed to get archive entries");

                for entry in &entries {
                    total += entry.size.get_size();

                    let path = path.join(&entry.name);

                    // Failed to change permissions => likely patch-related file and was made by the sudo, so root
                    #[allow(unused_must_use)]
                    if std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).is_err() {
                        // For weird reason we can delete files made by root, but can't modify their permissions
                        // We're not checking its result because if it's error - then it's either couldn't be removed (which is not the case)
                        // or the file doesn't exist, which we obviously can just ignore
                        std::fs::remove_file(&path);
                    }
                }

                tracing::trace!("Extracting archive");

                let unpacking_path = path.clone();
                let unpacking_updater = updater.clone();

                let handle_2 = std::thread::spawn(move || {
                    let mut entries = entries.into_iter()
                        .map(|entry| (unpacking_path.join(&entry.name), entry.size.get_size(), true))
                        .collect::<Vec<_>>();

                    let mut unpacked = 0;

                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(250));

                        let mut empty = true;

                        for (path, size, remained) in &mut entries {
                            if *remained {
                                empty = false;

                                if std::path::Path::new(path).exists() {
                                    *remained = false;

                                    unpacked += *size;
                                }
                            }
                        }

                        (unpacking_updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::UnpackingProgress(unpacked, total)));

                        if empty {
                            break;
                        }
                    }
                });

                let unpacking_updater = updater.clone();
                let extract_to = path.clone();

                // Run archive extraction in another thread to not to freeze the current one
                let handle_1 = std::thread::spawn(move || {
                    (unpacking_updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::UnpackingStarted(extract_to.clone())));

                    // We have to create new instance of Archive here
                    // because otherwise it may not work after get_entries method call
                    match Archive::open(temp_folder.join(first_segment_name)) {
                        Ok(mut archive) => match archive.extract(&extract_to) {
                            Ok(_) => {
                                // TODO error handling
                                #[allow(unused_must_use)] {
                                    for name in segments_names {
                                        std::fs::remove_file(temp_folder.join(name));
                                    }
                                }

                                (unpacking_updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::UnpackingFinished));
                            }

                            Err(err) => (unpacking_updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::UnpackingError(err.to_string())))
                        }

                        Err(err) => (unpacking_updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::UnpackingError(err.to_string())))
                    }
                });

                handle_1.join().unwrap();
                handle_2.join().unwrap();
            }

            Err(err) => (updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::UnpackingError(err.to_string())))
        }

        // Imitate Installer update message
        (updater)(DiffUpdate::InstallerUpdate(InstallerUpdate::UnpackingFinished));

        // Create `.version` file here even if hdiff patching is failed because
        // it's easier to explain user why he should run files repairer than
        // why he should re-download entire game update because something is failed
        #[allow(unused_must_use)] {
            let version_path = self.version_file_path()
                .unwrap_or(path.join(".version"));

            std::fs::write(version_path, self.latest().version);
        }

        // Apply hdiff patches
        // We're ignoring Err because in practice it means that hdifffiles.txt is missing
        if let Ok(files) = std::fs::read_to_string(path.join("hdifffiles.txt")) {
            tracing::debug!("Applying hdiff patches");

            (updater)(Self::Update::ApplyingHdiffStarted);

            let files = files.lines().collect::<Vec<&str>>();
            let hdiffs = files.len() as u64;

            // {"remoteName": "AnimeGame_Data/StreamingAssets/Audio/GeneratedSoundBanks/Windows/Japanese/1001.pck"}
            for (i, file) in files.into_iter().enumerate() {
                let relative_file = &file[16..file.len() - 2];

                let file = path.join(relative_file);
                let patch = path.join(format!("{relative_file}.hdiff"));
                let output = path.join(format!("{relative_file}.hdiff_patched"));

                // If failed to apply the patch
                if let Err(err) = hpatchz::patch(&file, &patch, &output) {
                    tracing::warn!("Failed to apply hdiff patch for {:?}: {err}", file);
                    tracing::debug!("Trying to repair corrupted file");

                    // If we were able to get API response - it shouldn't be impossible
                    // to also get integrity files list from the same API
                    match super::repairer::try_get_integrity_file(self.edition(), relative_file, Some(*crate::REQUESTS_TIMEOUT)) {
                        Ok(Some(integrity)) => {
                            if !integrity.fast_verify(&path) {
                                if let Err(err) = integrity.repair(&path) {
                                    tracing::error!("Failed to repair corrupted file: {err}");

                                    return Err(err.into());
                                }
                            }
                        }

                        Ok(None) => {
                            tracing::error!("Failed to repair corrupted file: not found");

                            return Err(Self::Error::HdiffPatch(err.to_string()))
                        }

                        Err(repair_fail) => {
                            tracing::error!("Failed to repair corrupted file: {repair_fail}");

                            return Err(Self::Error::HdiffPatch(err.to_string()))
                        }
                    }

                    #[allow(unused_must_use)] {
                        std::fs::remove_file(&patch);
                    }
                }

                // If patch was successfully applied
                else {
                    // FIXME: handle errors properly
                    std::fs::remove_file(&file)
                        .expect(&format!("Failed to remove hdiff patch: {:?}", file));

                    std::fs::remove_file(&patch)
                        .expect(&format!("Failed to remove hdiff patch: {:?}", patch));

                    std::fs::rename(&output, &file)
                        .expect(&format!("Failed to rename hdiff patch: {:?}", file));
                }

                (updater)(Self::Update::ApplyingHdiffProgress(i as u64 + 1, hdiffs));
            }

            std::fs::remove_file(path.join("hdifffiles.txt"))
                .expect("Failed to remove hdifffiles.txt");

            (updater)(Self::Update::ApplyingHdiffFinished);
        }

        tracing::debug!("Deleting outdated files");

        // Remove outdated files
        // We're ignoring Err because in practice it means that deletefiles.txt is missing
        if let Ok(files) = std::fs::read_to_string(path.join("deletefiles.txt")) {
            let files = files.lines().collect::<Vec<&str>>();
            let files_len = files.len() as u64;

            (updater)(Self::Update::RemovingOutdatedStarted);

            // AnimeGame_Data/Plugins/metakeeper.dll
            for (i, file) in files.into_iter().enumerate() {
                let file = path.join(file);

                std::fs::remove_file(&file)
                    .expect(&format!("Failed to remove outdated file: {:?}", file));

                (updater)(Self::Update::RemovingOutdatedProgress(i as u64 + 1, files_len));
            }

            std::fs::remove_file(path.join("deletefiles.txt"))
                .expect("Failed to remove deletefiles.txt");

            (updater)(Self::Update::RemovingOutdatedFinished);
        }

        Ok(())
    }
}
