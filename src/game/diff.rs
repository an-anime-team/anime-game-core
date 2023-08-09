use crate::updater::UpdaterExt;

pub trait GetDiffExt {
    type Diff;
    type Error;

    /// Get component version diff
    fn get_diff(&self) -> Result<Self::Diff, Self::Error>;
}

pub trait DiffExt {
    type Updater: UpdaterExt;

    /// Check if current diff is installable
    fn is_installable(&self) -> bool;

    /// Install diff
    /// 
    /// Return `None` if current diff
    /// is not supposed to be installed (e.g. `Latest`)
    fn install(self) -> Option<Self::Updater>;
}
