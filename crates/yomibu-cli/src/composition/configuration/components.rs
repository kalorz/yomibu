use crate::composition::options::{cli_name, environment_name};
use serde::Deserialize;
pub use yomibu_core::extension::options::{OptionError, OptionKey, Options, Setting, Value};
pub use yomibu_core::ports::infrastructure::credentials::Secret;
pub use yomibu_ext::{COMPONENTS, settings};

pub use yomibu_ext::{openai::API_KEY as GENERATION_KEY, wanikani::API_KEY as SOURCE_KEY};

pub fn credentials() -> impl Iterator<Item = OptionKey<Secret>> {
    settings().filter_map(Setting::secret)
}

pub fn options() -> impl Iterator<Item = Setting> {
    settings().filter(|setting| setting.secret().is_none())
}

pub fn credential_guidance(key: OptionKey<Secret>) -> String {
    format!(
        "{} Supply --{} or {}. On macOS, run yomibu auth {}.",
        key.description,
        cli_name(key.name),
        environment_name(key.name),
        key.name.component
    )
}

macro_rules! choice {
    ($name:ident, $variant:ident, $id:literal) => {
        #[derive(Debug, Clone, Copy, Default, Deserialize)]
        pub enum $name {
            #[default]
            #[serde(rename=$id)]
            $variant,
        }
    };
}
choice!(Source, Wanikani, "wanikani");
choice!(LearningStore, File, "file-learning-store");
choice!(Preparation, StoryPrompt, "story-prompt-preparation");
choice!(Generation, Openai, "openai");
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Components {
    pub source: Source,
    pub learning_store: LearningStore,
    pub preparation: Preparation,
    pub generation: Generation,
}

#[cfg(test)]
pub use yomibu_core::extension::options::OptionType;
#[cfg(test)]
pub use yomibu_ext::openai::MODEL as GENERATION_MODEL;
