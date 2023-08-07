use super::version::Version;

pub trait ComponentExt {
    type Variant;

    /// Get component variant
    fn variant(&self) -> &Self::Variant;

    /// Check if component is installed
    fn is_installed(&self) -> bool;

    /// Check if installed component version is latest available
    fn is_latest(&self) -> bool {
        let Some(installed) = self.installed_version() else {
            return false;
        };

        installed == self.latest_version()
    }

    /// Get installed component version, or `None` if not installed
    fn installed_version(&self) -> Option<Version>;

    /// Get latest available component version, or `Err` if failed to fetch game API
    fn latest_version(&self) -> Version;

    /// Get component downloading URI
    fn download_uri(&self) -> &str;
}
