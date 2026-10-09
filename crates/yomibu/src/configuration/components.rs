use super::modules::ModuleId;
use serde::Deserialize;
pub use yomibu_components::{COMPONENTS, settings};
pub use yomibu_core::component::credentials::Secret;
pub use yomibu_core::component::options::{
    OptionError, OptionKey, OptionType, Options, Setting, Value,
};

pub use yomibu_components::{
    http_embeddings::{
        API_KEY as EMBEDDING_KEY, DIMENSIONS as EMBEDDING_DIMENSIONS,
        ENDPOINT as EMBEDDING_ENDPOINT, MODEL as EMBEDDING_MODEL, REVISION as EMBEDDING_REVISION,
    },
    openai_story_generation::{API_KEY as GENERATION_KEY, MODEL as GENERATION_MODEL},
    wanikani_source::API_KEY as SOURCE_KEY,
};

pub fn credentials() -> impl Iterator<Item = OptionKey<Secret>> {
    settings().filter_map(Setting::secret)
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
                self.enabled(ModuleId::Sync)
            } else if requirement.name == EMBEDDING_KEY.name {
                self.enabled(ModuleId::Embeddings)
                    && self.pipeline.embedding_provider == Some(super::EmbeddingProvider::Openai)
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
choice!(EmbeddingCache, File, "file-embedding-cache");
choice!(Preparation, StoryPrompt, "story-prompt-preparation");
choice!(Generation, Openai, "openai");
choice!(Analysis, Sudachi, "sudachi-dictionary");
choice!(
    Assessment,
    JapaneseConstraints,
    "japanese-constraint-checks"
);

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Components {
    pub source: Source,
    pub learning_store: LearningStore,
    pub embedding_cache: EmbeddingCache,
    pub preparation: Preparation,
    pub generation: Generation,
    pub analysis: Analysis,
    pub assessment: Assessment,
}
