use std::path::Path;

use crate::version::Version;

pub trait VersionDiffExt {
    /// Type that will be used as downloading / unpacking / installation error
    type Error;

    /// Type that will be used in the `install`-like methods
    /// as the current installation progress update
    type Update;

    /// Type that will represent the game edition this `VersionDiff` belongs to
    type Edition;

    /// Get selected game edition
    fn edition(&self) -> Self::Edition;

    /// Return currently installed version
    /// 
    /// Return `None` if it's not installed
    fn current(&self) -> Option<Version>;

    /// Return latest available version
    fn latest(&self) -> Version;

    /// Return size of data in bytes needed to be downloaded
    /// 
    /// Return `None` if this information is not available for current diff type
    fn downloaded_size(&self) -> Option<u64>;

    /// Return size of unpacked data in bytes
    /// 
    /// Return `None` if this information is not available for current diff type
    fn unpacked_size(&self) -> Option<u64>;

    /// Return the path this difference should be installed to
    /// 
    /// Return `None` if the path is not available for current diff type
    fn installation_path(&self) -> Option<&Path>;

    /// Get the downloading URI if it's available
    /// 
    /// Return `None` if the URI is not provided
    fn downloading_uri(&self) -> Option<String>;

    /// Get the name of the file from downloading URI
    /// 
    /// - `https://example.com/example.zip` -> `example.zip`
    /// - `https://example.com/` -> `index.html`
    /// - `https://example.com` -> `index.html`
    /// 
    /// Return `None` if the URI is not provided
    fn file_name(&self) -> Option<String> {
        self.downloading_uri().map(|uri| {
            let Some(index) = uri.replace('\\', "/").rfind('/') else {
                return String::from("index.html");
            };

            let file = &uri[index + 1..];

            file.is_empty()
                .then(|| String::from("index.html"))
                .unwrap_or_else(|| String::from(file))
        })
    }

    // TODO: think about async

    #[cfg(feature = "install")]
    /// Try to download the diff into the specified folder,
    /// using `Self::file_name` result as a name of the file to be saved as
    fn download_to(&mut self, folder: impl AsRef<Path>, progress: impl Fn(u64, u64) + Send + 'static) -> Result<(), Self::Error> {
        let filename = self.file_name()
            .expect("Failed to resolve downloading file name");

        self.download_as(folder.as_ref().join(filename), progress)
    }

    #[cfg(feature = "install")]
    /// Try to download the diff into the specified path, assuming that it contains the file name
    /// this difference should be saved as
    fn download_as(&mut self, path: impl AsRef<Path>, progress: impl Fn(u64, u64) + Send + 'static) -> Result<(), Self::Error>;

    #[cfg(feature = "install")]
    /// Try to install the difference into the path returned by `Self::installation_path` method
    /// 
    /// This method can fail if installation path is not provided
    fn install(&self, thread_count: usize, updater: impl Fn(Self::Update) + Clone + Send + 'static) -> Result<(), Self::Error> {
        let path = self.installation_path()
            .expect("Difference installation path is not provided");

        self.install_to(path, thread_count, updater)
    }

    #[cfg(feature = "install")]
    /// Try to install the difference by given location
    fn install_to(&self, path: impl AsRef<Path>, thread_count: usize, updater: impl Fn(Self::Update) + Clone + Send + 'static) -> Result<(), Self::Error>;
}
