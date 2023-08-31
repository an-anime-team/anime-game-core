use std::sync::Arc;
use std::ffi::OsStr;

use crate::filesystem::DriverExt;
use crate::game::component::ComponentExt;
use crate::game::version::Version;

use super::api::schema::Voiceover as VoiceoverSchema;
use super::Edition;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Variant {
    AudioEnglish,
    AudioJapanese,
    AudioKorean,
    AudioChinese
}

impl From<VoiceoverSchema> for Variant {
    #[inline]
    fn from(value: VoiceoverSchema) -> Self {
        match value.language.as_str() {
            "en-us" => Self::AudioEnglish,
            "ja-jp" => Self::AudioJapanese,
            "ko-kr" => Self::AudioKorean,
            "zh-cn" => Self::AudioChinese,

            _ => unimplemented!()
        }
    }
}

pub struct Component {
    pub variant: Variant,
    pub download_uri: String,
    pub latest_version: Version,
    pub edition: Edition,
    pub driver: Arc<dyn DriverExt>
}

impl ComponentExt for Component {
    type Variant = Variant;

    #[inline]
    fn variant(&self) -> &Self::Variant {
        &self.variant
    }

    fn is_installed(&self) -> bool {
        match &self.variant {
            Variant::AudioEnglish  => self.driver.exists(OsStr::new(&format!("{}/StreamingAssets/AudioAssets/English(US)", self.edition.data_folder()))),
            Variant::AudioJapanese => self.driver.exists(OsStr::new(&format!("{}/StreamingAssets/AudioAssets/Japanese", self.edition.data_folder()))),
            Variant::AudioKorean   => self.driver.exists(OsStr::new(&format!("{}/StreamingAssets/AudioAssets/Korean", self.edition.data_folder()))),
            Variant::AudioChinese  => self.driver.exists(OsStr::new(&format!("{}/StreamingAssets/AudioAssets/Chinese", self.edition.data_folder())))
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
