use super::{
    StoryInputs, inputs,
    progress::RunProgress,
    source,
    story_workflow::{self, GenerationClient},
};
use crate::application::input_file;
use crate::configuration::Configuration;
use crate::configuration::modules::ModuleId;
use chrono::{DateTime, Utc};
use yomibu_components::{
    file_learning_store::cache, openai_story_generation as openai, wanikani_source as wanikani,
};
use yomibu_core::capabilities::LearningStore;
use yomibu_core::domain::{inventory::InventoryError, story::StoryError};
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
    #[error("Missing required setup.")]
    Setup { issues: Vec<SetupIssue> },
    #[error(transparent)]
    Cache(#[from] cache::CacheError),
    #[error(transparent)]
    Persistence(#[from] cache::WriteError),
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
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Story(#[from] StoryError),
    #[error(
        "Recorded WaniKani content access has expired; run yomibu sync with a key to refresh access."
    )]
    AccessExpired,
    #[error(transparent)]
    Validation(#[from] yomibu_core::domain::source::ValidationError),
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
    pub async fn story<Store: LearningStore>(
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
            now,
            seed,
            progress,
            started,
        )
        .await
    }
}
