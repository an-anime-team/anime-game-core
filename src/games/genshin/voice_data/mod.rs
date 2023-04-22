pub mod locale;
pub mod package;

pub mod prelude {
    pub use super::locale::VoiceLocale;
    pub use super::package::VoicePackage;
}
