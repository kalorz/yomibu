use crate::configuration::components;
use std::path::PathBuf;
use yomibu_components::{
    file_embedding_cache::EmbeddingCacheFile, file_learning_store::FileLearningStore,
    openai_story_generation, story_prompt_preparation::StoryPromptPreparation, wanikani_source,
};

impl components::Source {
    pub(super) fn client(
        self,
        key: &str,
        endpoint: &str,
    ) -> Result<wanikani_source::Client, wanikani_source::Error> {
        match self {
            Self::Wanikani => wanikani_source::Client::with_base_url(key, endpoint),
        }
    }
}
impl components::LearningStore {
    pub fn open(self, path: impl Into<PathBuf>) -> FileLearningStore {
        match self {
            Self::File => FileLearningStore::at_path(path),
        }
    }
}
impl components::EmbeddingCache {
    pub(super) fn open(self, path: impl Into<PathBuf>) -> EmbeddingCacheFile {
        match self {
            Self::File => EmbeddingCacheFile::new(path),
        }
    }
}
impl components::Preparation {
    pub(super) fn construct(self) -> StoryPromptPreparation {
        match self {
            Self::StoryPrompt => StoryPromptPreparation,
        }
    }
}
impl components::Generation {
    pub(super) fn client(
        self,
        key: &str,
        endpoint: &str,
    ) -> Result<openai_story_generation::Client, openai_story_generation::ProviderError> {
        match self {
            Self::Openai => openai_story_generation::Client::with_base_url(key, endpoint),
        }
    }
}
impl components::Assessment {
    pub(super) fn construct(
        self,
    ) -> yomibu_components::japanese_constraint_checks::JapaneseConstraintChecks {
        match self {
            Self::JapaneseConstraints => {
                yomibu_components::japanese_constraint_checks::JapaneseConstraintChecks
            }
        }
    }
}
