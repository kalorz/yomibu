use super::{StoryInputs, progress::RunProgress, story_workflow};
use crate::configuration::Configuration;
use crate::configuration::components::OptionKey;
use chrono::{DateTime, Utc};
use yomibu_components::{
    file_learning_store::cache, openai_story_generation as openai, wanikani_source as wanikani,
};
use yomibu_core::domain::{inventory::InventoryError, learner::LearnerData, story::StoryError};
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
pub enum SetupIssue {
    Inventory,
    Credential(OptionKey<super::Secret>),
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
    Persistence(#[from] cache::WriteError),
    #[error(transparent)]
    Source(#[from] wanikani::Error),
    #[error(transparent)]
    Provider(#[from] openai::ProviderError),
    #[error(transparent)]
    Generation(#[from] yomibu_core::pipeline::story::GenerationError<openai::ProviderError>),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Story(#[from] StoryError),
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

    pub fn read_learner_data(&self) -> Result<Option<LearnerData>, cache::CacheError> {
        self.config
            .pipeline
            .components
            .learning_store
            .open(&self.config.data_dir)
            .read_learner_data()
    }

    pub async fn story(
        &self,
        data: Option<LearnerData>,
        now: DateTime<Utc>,
        seed: u64,
        emit: impl FnMut(ProgressEvent),
    ) -> Result<StoryRunReport, ApplicationError> {
        let mut progress = RunProgress::new(emit);
        let started = progress.start(Step::Inputs);
        let request = self.config.story.request()?;
        let inventory = data.map(|data| data.into_inventory(now)).transpose()?;
        if let Some(inventory) = &inventory {
            request.validate(inventory)?;
        }
        let key = self.credentials.generation()?;
        let (inventory, key) = match (inventory, key) {
            (Some(inventory), Some(key)) => (inventory, key),
            (inventory, key) => {
                let mut issues = Vec::new();
                if inventory.is_none() {
                    issues.push(SetupIssue::Inventory);
                }
                if key.is_none() {
                    issues.push(SetupIssue::Credential(openai::API_KEY));
                }
                return Err(ApplicationError::Setup { issues });
            }
        };
        let client = self
            .config
            .pipeline
            .components
            .generation
            .client(key, &self.endpoints.openai)?;
        progress.finish(Step::Inputs, started);
        story_workflow::execute_story(
            &self.config,
            StoryInputs { request, inventory },
            &client,
            seed,
            progress,
        )
        .await
    }
}
