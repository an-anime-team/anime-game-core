use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::ffi::OsStr;

use crate::filesystem::DriverExt;
use crate::game::component::ComponentExt;
use crate::game::version::Version;

use super::api::schema::Voiceover as VoiceoverSchema;
use super::{Game, Edition};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Variant {
    Voiceover(String)
}

impl From<VoiceoverSchema> for Variant {
    #[inline]
    fn from(value: VoiceoverSchema) -> Self {
        Self::Voiceover(value.language)
    }
}

pub struct Component {
    pub variant: Variant,
    pub download_uri: String,
    pub latest_version: Version,
    pub edition: Edition,
    pub driver: Rc<Box<dyn DriverExt>>
}

impl ComponentExt for Component {
    type Variant = Variant;

    #[inline]
    fn variant(&self) -> &Self::Variant {
        &self.variant
    }

    fn is_installed(&self) -> bool {
        match &self.variant {
            Variant::Voiceover(language) => match language.as_str() {
                "en-us" => self.driver.exists(OsStr::new(&format!("{}/StreamingAssets/AudioAssets/English(US)", self.edition.data_folder()))),
                "ja-jp" => self.driver.exists(OsStr::new(&format!("{}/StreamingAssets/AudioAssets/Japanese", self.edition.data_folder()))),
                "zh-cn" => self.driver.exists(OsStr::new(&format!("{}/StreamingAssets/AudioAssets/Chinese", self.edition.data_folder()))),
                "ko-kr" => self.driver.exists(OsStr::new(&format!("{}/StreamingAssets/AudioAssets/Korean", self.edition.data_folder()))),

                _ => false
            }
        }
    }

    fn installed_version(&self) -> Option<Version> {
        if !self.is_installed() {
            return None;
        }

        todo!()
    }

    #[inline]
    fn latest_version(&self) -> Version {
        self.latest_version
    }

    #[inline]
    fn download_uri(&self) -> &str {
        &self.download_uri
    }
}
