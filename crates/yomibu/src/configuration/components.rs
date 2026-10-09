use serde::Deserialize;
pub use yomibu_core::capabilities::options::CredentialRequirement;

pub use yomibu_components::{
    http_embeddings::{
        API_KEY as EMBEDDING_KEY, DIMENSIONS as EMBEDDING_DIMENSIONS,
        ENDPOINT as EMBEDDING_ENDPOINT, MODEL as EMBEDDING_MODEL, REVISION as EMBEDDING_REVISION,
    },
    openai_story_generation::{API_KEY as GENERATION_KEY, MODEL as GENERATION_MODEL},
    wanikani_source::API_KEY as SOURCE_KEY,
};

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
choice!(Source, Wanikani, "wanikani-source");
choice!(LearningStore, File, "file-learning-store");
choice!(EmbeddingCache, File, "file-embedding-cache");
choice!(Preparation, StoryPrompt, "story-prompt-preparation");
choice!(Generation, Openai, "openai-story-generation");
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
