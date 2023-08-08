use std::ffi::OsStr;
use std::rc::Rc;

use crate::game::version::{
    Version,
    Error as VersionError
};

use crate::filesystem::DriverExt;
use crate::game::GameExt;
use crate::network::api::ApiExt;

pub mod component;
pub mod api;
pub mod diff;

use component::{
    Component,
    Variant as ComponentVariant
};

use api::Api;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to parse installed game version")]
    GameVersionParseError,

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Minreq(#[from] minreq::Error),

    #[error("{0}")]
    MinreqRef(#[from] &'static minreq::Error),

    #[error("Failed to parse version: {0}")]
    VersionParseError(#[from] VersionError)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edition {
    Global,
    China
}

impl Edition {
    pub fn data_folder(&self) -> &'static str {
        match self {
            Self::Global => concat!("Gen", "shin", "Imp", "act_Data"),
            Self::China  => concat!("Yua", "nShe", "n_Data")
        }
    }
}

pub struct Game {
    driver: Rc<Box<dyn DriverExt>>,
    edition: Edition
}

impl GameExt for Game {
    type Edition = Edition;
    type Component = Component;
    type Error = Error;

    #[inline]
    fn new(driver: impl DriverExt + 'static, edition: Self::Edition) -> Self {
        Self {
            driver: Rc::new(Box::new(driver)),
            edition
        }
    }

    #[inline]
    fn driver(&self) -> &dyn DriverExt {
        self.driver.as_ref().as_ref()
    }

    #[inline]
    fn edition(&self) -> Self::Edition {
        self.edition
    }

    #[inline]
    fn is_installed(&self) -> bool {
        self.driver.exists(OsStr::new(&format!("{}/globalgamemanagers", self.edition.data_folder())))
    }

    fn get_version(&self) -> Result<Version, Self::Error> {
        fn bytes_to_num(bytes: &[u8]) -> u8 {
            bytes.iter().fold(0, |acc, &x| acc * 10 + (x - b'0'))
        }

        // [0..9]
        let allowed = [48, 49, 50, 51, 52, 53, 54, 55, 56, 57];

        let mut version: [Vec<u8>; 3] = [vec![], vec![], vec![]];
        let mut version_ptr: usize = 0;
        let mut correct = true;

        let bytes = self.driver.read(OsStr::new(&format!("{}/globalgamemanagers", self.edition.data_folder())))?;

        for byte in bytes.into_iter().skip(4000).take(10000) {
            match byte {
                0 => {
                    version = [vec![], vec![], vec![]];
                    version_ptr = 0;
                    correct = true;
                }

                46 => {
                    version_ptr += 1;

                    if version_ptr > 2 {
                        correct = false;
                    }
                }

                95 => {
                    if correct && !version[0].is_empty() && !version[1].is_empty() && !version[2].is_empty() {
                        return Ok(Version::new(
                            bytes_to_num(&version[0]),
                            bytes_to_num(&version[1]),
                            bytes_to_num(&version[2]),
                            0
                        ))
                    }

                    correct = false;
                }

                _ => {
                    if correct && allowed.contains(&byte) {
                        version[version_ptr].push(byte);
                    }

                    else {
                        correct = false;
                    }
                }
            }
        }

        Err(Error::GameVersionParseError)
    }

    fn get_latest_version(&self) -> Result<Version, Self::Error> {
        Api::fetch(self.edition).as_ref()
            .map_err(Error::from)
            .and_then(|response| Ok(response.data.game.latest.version.parse()?))
    }

    fn get_components(&self) -> Result<Vec<Self::Component>, Self::Error> {
        Api::fetch(self.edition).as_ref()
            .map_err(Error::from)
            .map(|response| response.data.game.latest.voice_packs.iter().cloned()
                .map(|voiceover| Component {
                    download_uri: voiceover.path.clone(),
                    latest_version: response.data.game.latest.version.parse().unwrap(),
                    edition: self.edition,
                    driver: self.driver.clone(),
                    variant: ComponentVariant::from(voiceover)
                })
                .collect())
    }
}
