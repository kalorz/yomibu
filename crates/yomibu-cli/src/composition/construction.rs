use crate::composition::configuration::components;
use std::path::PathBuf;
use yomibu_ext::{
    file_store::FileLearningStore, openai, openai::preparation::StoryPromptPreparation, wanikani,
};

impl components::Source {
    pub(super) fn client(
        self,
        key: &str,
        endpoint: &str,
    ) -> Result<wanikani::Client, wanikani::Error> {
        match self {
            Self::Wanikani => wanikani::Client::with_base_url(key, endpoint),
        }
    }
}
impl components::LearningStore {
    pub fn open(self, data_dir: impl Into<PathBuf>) -> FileLearningStore {
        match self {
            Self::File => FileLearningStore::new(data_dir),
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
    ) -> Result<openai::Client, openai::ProviderError> {
        match self {
            Self::Openai => openai::Client::with_base_url(key, endpoint),
        }
    }
}
