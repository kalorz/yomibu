use serde::Deserialize;
pub use yomibu_components::{COMPONENTS, settings};
pub use yomibu_core::component::credentials::Secret;
pub use yomibu_core::component::options::{
    OptionError, OptionKey, OptionType, Options, Setting, Value,
};

pub use yomibu_components::{
    openai_story_generation::{API_KEY as GENERATION_KEY, MODEL as GENERATION_MODEL},
    wanikani_source::API_KEY as SOURCE_KEY,
};

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
        key.name.cli(),
        key.name.environment(),
        key.name.component
    )
}

impl super::Configuration {
    pub fn credential_requirements(&self) -> impl Iterator<Item = OptionKey<Secret>> + '_ {
        credentials().filter(|requirement| {
            if requirement.name == SOURCE_KEY.name {
                self.application.sync
            } else {
                true
            }
        })
    }
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
