use crate::composition::configuration::Configuration;
use crate::composition::configuration::components::OptionKey;
use chrono::{DateTime, Utc};
use yomibu_app::application::generate_story::StoryInputs;
use yomibu_core::domain::{
    generation::StoryError, knowledge::inventory::InventoryError, knowledge::learner::LearnerData,
};
use yomibu_ext::{file_store::cache, openai, wanikani};
mod explicit;

use yomibu_app::application::generate_story::progress::{ProgressEvent, RunProgress};
use yomibu_app::reports::run::StoryRunReport;

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
    credentials: Credentials,
    endpoints: ServiceEndpoints,
}

#[derive(Debug)]
pub enum SetupIssue {
    Inventory,
    Credential(OptionKey<super::Secret>),
}
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Options(#[from] crate::composition::configuration::components::OptionError),
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
    Generation(
        #[from]
        yomibu_core::domain::generation::preparation::GenerationError<openai::ProviderError>,
    ),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Story(#[from] StoryError),
    #[error(transparent)]
    Validation(#[from] yomibu_core::domain::knowledge::source::ValidationError),
}

impl LocalApp {
    /// Store supplied settings and credentials without doing I/O.
    pub fn new(config: Configuration, credentials: Credentials) -> Self {
        Self {
            config,
            credentials,
            endpoints: ServiceEndpoints::default(),
        }
    }
    pub fn with_endpoints(mut self, endpoints: ServiceEndpoints) -> Self {
        self.endpoints = endpoints;
        self
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
        let progress = RunProgress::new(emit);
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
        super::selection::run_story(
            &self.config,
            StoryInputs { request, inventory },
            &client,
            seed,
            progress,
        )
        .await
    }
}
