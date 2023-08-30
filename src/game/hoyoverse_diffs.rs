use std::sync::Arc;
use std::ffi::OsStr;
use std::path::Path;

use crate::filesystem::DriverExt;
use crate::builtin::hpatchz;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    ApplyingHdiffStarted,
    ApplyingHdiffProgress(u64, u64),
    ApplyingHdiffFinished,
    DeletingObsoleteStarted,
    DeletingObsoleteProgress(u64, u64),
    DeletingObsoleteFinished
}

#[allow(clippy::expect_fun_call)]
#[tracing::instrument(skip(driver, updater))]
pub fn apply_update(driver: Arc<dyn DriverExt>, transition_path: &Path, updater: impl Fn(Status)) -> std::io::Result<()> {
    // Apply hdiff patches
    // We're ignoring Err because in practice it means that hdifffiles.txt is missing
    if let Ok(files) = std::fs::read_to_string(transition_path.join("hdifffiles.txt")) {
        tracing::debug!("Applying hdiff patches");

        (updater)(Status::ApplyingHdiffStarted);

        let files = files.lines().collect::<Vec<&str>>();
        let hdiffs = files.len() as u64;

        // {"remoteName": "AnimeGame_Data/StreamingAssets/Audio/GeneratedSoundBanks/Windows/Japanese/1001.pck"}
        for (i, file) in files.into_iter().enumerate() {
            // FIXME: Not really good...
            let relative_file = &file[16..file.len() - 2];

            let file = transition_path.join(relative_file);
            let patch = transition_path.join(format!("{relative_file}.hdiff"));
            let output = transition_path.join(format!("{relative_file}.hdiff_patched"));

            // Copy file needed to be patched from the driver to the temporary transition folder
            driver.copy(OsStr::new(relative_file), &file)?;

            // If failed to apply the patch
            if let Err(err) = hpatchz::patch(&file, &patch, &output) {
                tracing::warn!("Failed to apply hdiff patch for {:?}: {err}", file);
                // tracing::debug!("Trying to repair corrupted file");

                // If we were able to get API response - it shouldn't be impossible
                // to also get integrity files list from the same API
                // match super::repairer::try_get_integrity_file(self.edition(), relative_file, Some(*crate::REQUESTS_TIMEOUT)) {
                //     Ok(Some(integrity)) => {
                //         if !integrity.fast_verify(&path) {
                //             if let Err(err) = integrity.repair(&path) {
                //                 tracing::error!("Failed to repair corrupted file: {err}");

                //                 return Err(err.into());
                //             }
                //         }
                //     }

                //     Ok(None) => {
                //         tracing::error!("Failed to repair corrupted file: not found");

                //         return Err(Self::Error::HdiffPatch(err.to_string()))
                //     }

                //     Err(repair_fail) => {
                //         tracing::error!("Failed to repair corrupted file: {repair_fail}");

                //         return Err(Self::Error::HdiffPatch(err.to_string()))
                //     }
                // }

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

            (updater)(Status::ApplyingHdiffProgress(i as u64 + 1, hdiffs));
        }

        std::fs::remove_file(transition_path.join("hdifffiles.txt"))
            .expect("Failed to remove hdifffiles.txt");

        (updater)(Status::ApplyingHdiffFinished);
    }

    Ok(())
}

#[allow(clippy::expect_fun_call)]
#[tracing::instrument(skip(driver, updater))]
pub fn post_transition(driver: Arc<dyn DriverExt>, updater: impl Fn(Status)) -> std::io::Result<()> {
    tracing::debug!("Deleting outdated files");

    // Remove outdated files
    // We're ignoring Err because in practice it means that deletefiles.txt is missing
    if let Ok(files) = driver.read_to_string(OsStr::new("deletefiles.txt")) {
        let files = files.lines().collect::<Vec<&str>>();
        let files_len = files.len() as u64;

        (updater)(Status::DeletingObsoleteStarted);

        // AnimeGame_Data/Plugins/metakeeper.dll
        for (i, file) in files.into_iter().enumerate() {
            driver.remove_file(OsStr::new(file))
                .expect(&format!("Failed to remove outdated file: {:?}", file));

            (updater)(Status::DeletingObsoleteProgress(i as u64 + 1, files_len));
        }

        driver.remove_file(OsStr::new("deletefiles.txt"))
            .expect("Failed to remove deletefiles.txt");

        (updater)(Status::DeletingObsoleteFinished);
    }

    Ok(())
}
