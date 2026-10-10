use super::{
    StoryInputs, assessment, inputs,
    progress::RunProgress,
    source,
    story_workflow::{self, GenerationClient, StoryEmbeddings},
};
use crate::application::input_file;
use crate::configuration::Configuration;
use crate::configuration::modules::ModuleId;
use chrono::{DateTime, Utc};
use yomibu_components::{
    file_embedding_cache::EmbeddingCacheFileError,
    file_learning_store::cache,
    openai_story_generation as openai,
    sudachi_dictionary::{DictionaryError, installation::InstallationError},
    wanikani_source as wanikani,
};
use yomibu_core::capabilities::LearningStore;
use yomibu_core::domain::{
    embedding::EmbeddingError, inventory::InventoryError, story::StoryError,
};
mod explicit;

use super::progress::{ProgressEvent, Step};
use crate::reports::run::StoryRunReport;

use super::Credentials;

pub struct ServiceEndpoints {
    pub wanikani: String,
    pub openai: String,
}
impl Default for ServiceEndpoints {
    fn default() -> Self {
        Self {
            wanikani: "https://api.wanikani.com/v2/".into(),
            openai: "https://api.openai.com/v1/".into(),
        }
    }
}

pub struct LocalApp {
    config: Configuration,
    credentials: std::sync::Arc<Credentials>,
    endpoints: std::sync::Arc<ServiceEndpoints>,
}

#[derive(Debug)]
pub struct SetupIssue {
    pub module: ModuleId,
}
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Options(#[from] crate::configuration::components::OptionError),
    #[error(transparent)]
    Credential(#[from] super::CredentialError),
    #[error("{}", crate::configuration::components::credential_guidance(*.0))]
    MissingCredential(crate::configuration::components::OptionKey<super::Secret>),
    #[error("Missing required setup.")]
    Setup { issues: Vec<SetupIssue> },
    #[error(transparent)]
    Cache(#[from] cache::CacheError),
    #[error(transparent)]
    Persistence(#[from] cache::WriteError),
    #[error(transparent)]
    InMemoryStore(#[from] yomibu_components::in_memory_learning_store::InMemoryStoreError),
    #[error(transparent)]
    Source(#[from] wanikani::Error),
    #[error(transparent)]
    Provider(#[from] openai::ProviderError),
    #[error(transparent)]
    Generation(#[from] yomibu_core::pipeline::story::GenerationError<openai::ProviderError>),
    #[error(transparent)]
    Input(#[from] input_file::InputFileError),
    #[error("Invalid {kind} JSON: {source}")]
    InvalidJson {
        kind: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("Unsupported analysis input version {version}; expected 1.")]
    InputVersion { version: u32 },
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Story(#[from] StoryError),
    #[error(
        "Recorded WaniKani content access has expired; run yomibu sync with a key to refresh access."
    )]
    AccessExpired,
    #[error(transparent)]
    Dictionary(#[from] DictionaryError),
    #[error(transparent)]
    Installation(#[from] InstallationError),
    #[error(transparent)]
    EmbeddingCache(#[from] EmbeddingCacheFileError),
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    #[error("{0}")]
    Resource(&'static str),
    #[error("{0}")]
    ResourceConfiguration(&'static str),
    #[error(transparent)]
    Validation(#[from] yomibu_core::domain::source::ValidationError),
    #[error(transparent)]
    Sentence(#[from] yomibu_core::domain::analysis::SentenceError),
    #[error(transparent)]
    Analysis(#[from] yomibu_components::sudachi_dictionary::AnalysisError),
    #[error(transparent)]
    Grammar(#[from] yomibu_core::domain::grammar::GrammarError),
    #[error(transparent)]
    Evaluation(#[from] yomibu_core::domain::evaluation::EvaluationError),
}

impl From<yomibu_components::http_embeddings::BuildError> for ApplicationError {
    fn from(error: yomibu_components::http_embeddings::BuildError) -> Self {
        use yomibu_components::http_embeddings::BuildError;
        match error {
            BuildError::Options(error) => error.into(),
            BuildError::Embedding(error) => error.into(),
        }
    }
}

impl LocalApp {
    /// Store supplied settings and credentials without doing I/O.
    pub fn new(config: Configuration, credentials: Credentials) -> Self {
        Self {
            config,
            credentials: std::sync::Arc::new(credentials),
            endpoints: std::sync::Arc::new(ServiceEndpoints::default()),
        }
    }
    pub fn with_endpoints(mut self, endpoints: ServiceEndpoints) -> Self {
        self.endpoints = std::sync::Arc::new(endpoints);
        self
    }

    /// Resolve one invocation without I/O or mutations to shared resources/defaults.
    pub fn for_invocation(
        &self,
        input: crate::configuration::Invocation,
        operation: &super::Operation,
    ) -> Result<Self, crate::configuration::ConfigError> {
        Ok(Self {
            config: self.config.for_invocation(input, operation)?,
            credentials: std::sync::Arc::clone(&self.credentials),
            endpoints: std::sync::Arc::clone(&self.endpoints),
        })
    }

    /// Load local story inputs and generate using the selected source store.
    ///
    /// # Safety
    /// Selected managed dictionaries must satisfy
    /// [`SudachiAnalyzer::load`](yomibu_components::sudachi_dictionary::SudachiAnalyzer::load)
    /// for this future's duration.
    pub async unsafe fn story<Store: LearningStore>(
        &self,
        store: &Store,
        now: DateTime<Utc>,
        seed: u64,
        emit: impl FnMut(ProgressEvent),
    ) -> Result<StoryRunReport, ApplicationError>
    where
        ApplicationError: From<Store::ReadError> + From<Store::WriteError>,
    {
        let mut progress = RunProgress::new(&self.config, emit);
        let started = progress.start(Step::Inputs);
        let request = inputs::read_request(&self.config)?;
        story_workflow::validate_story_request(&self.config, &request)?;
        let manual = inputs::read_manual(self.config.application.inventory.as_deref())?;
        let store =
            (manual.is_none() || self.config.application.wanikani_cache.is_some()).then_some(store);
        story_workflow::execute_story(
            &self.config,
            StoryInputs { request, manual },
            store,
            source::SourceClient::Local {
                credentials: &self.credentials,
                endpoint: &self.endpoints.wanikani,
            },
            GenerationClient::Local {
                credentials: &self.credentials,
                endpoint: &self.endpoints.openai,
            },
            StoryEmbeddings::Local(&self.credentials),
            now,
            seed,
            progress,
            started,
            |generated, inputs, progress| {
                // SAFETY: story's caller guarantees verified, unchanged dictionary
                // bytes for this future. The local analyzer is dropped inside this
                // callback, so its entire lifetime is covered by that guarantee.
                unsafe {
                    assessment::assess_local_optional(&self.config, generated, inputs, progress)
                }
            },
        )
        .await
    }
}
