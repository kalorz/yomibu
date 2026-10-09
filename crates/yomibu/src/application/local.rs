use super::{assessment, embeddings, inputs, progress::RunProgress, source};
use crate::application::input_file;
use crate::configuration::Configuration;
use crate::configuration::modules::{ModuleId, ModuleReport, ModuleState};
use chrono::{DateTime, Utc};
use yomibu_components::{
    file_embedding_cache::EmbeddingCacheFileError,
    file_learning_store::cache,
    openai_story_generation as openai,
    sudachi_dictionary::{DictionaryError, installation::InstallationError},
    wanikani_source as wanikani,
};
use yomibu_core::domain::{
    embedding::EmbeddingError, inventory::InventoryError, story::StoryError,
};
use yomibu_core::pipeline::story::prepare_story;
mod explicit;

use super::progress::{ProgressEvent, Step};
use crate::reports::run::{SelectionReport, StoryRunReport, Warning};

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

    /// # Safety
    /// Selected managed dictionaries must satisfy
    /// [`SudachiAnalyzer::load`](yomibu_components::sudachi_dictionary::SudachiAnalyzer::load)
    /// for this future's duration.
    pub async unsafe fn story(
        &self,
        now: DateTime<Utc>,
        seed: u64,
        emit: impl FnMut(ProgressEvent),
    ) -> Result<StoryRunReport, ApplicationError> {
        let mut progress = RunProgress::new(&self.config, emit);
        let started = progress.start(Step::Inputs);
        let request = inputs::read_request(&self.config)?;
        request.validate_shape()?;
        request.validate_selection_limit(self.config.story.select)?;
        let manual = inputs::read_manual(self.config.application.inventory.as_deref())?;
        let cached = source::read_cache(&self.config, manual.is_none())?;
        let usable = cached.as_ref().is_some_and(|data| {
            source::usable_cache(data, &self.config.pipeline.knowledge_policy, now)
        });
        let needs_source = manual.is_none() && !usable;
        self.validate_story_setup(needs_source, &mut progress.modules)?;
        let client = self.config.pipeline.components.generation.client(
            self.credentials
                .generation()?
                .ok_or_else(|| ApplicationError::Setup {
                    issues: vec![SetupIssue {
                        module: ModuleId::Generation,
                    }],
                })?,
            &self.endpoints.openai,
        )?;
        progress.state(ModuleId::Generation, ModuleState::Available);
        progress.finish(Step::Inputs, started);
        let source = source::prepare_source(
            &self.config,
            &self.credentials,
            &self.endpoints.wanikani,
            manual.is_some(),
            cached,
            now,
            &mut progress,
        )
        .await?;
        let started = progress.start(Step::Knowledge);
        let inventory = inputs::prepare_inventory(
            &self.config.pipeline.knowledge_policy,
            source.as_ref(),
            manual,
            now,
        )?;
        progress.state(ModuleId::Knowledge, ModuleState::Available);
        progress.finish(Step::Knowledge, started);
        let seed = self.config.story.seed.unwrap_or(seed);
        request.validate(&inventory)?;
        let cache = embeddings::prepare_optional(
            &self.config,
            &self.credentials,
            &inventory,
            &request,
            &mut progress,
        )
        .await;
        let started = progress.start(Step::Selection);
        let (selection, retrieval_error) = embeddings::select_for_request(
            &self.config.pipeline.selection,
            &inventory,
            &request,
            cache.as_ref(),
            self.config.story.select,
            seed,
        )?;
        if let Some(error) = retrieval_error {
            progress.warnings.push(Warning::embedding_fallback(
                &error,
                &self.config.pipeline.selection,
            ));
            progress.state(
                ModuleId::Embeddings,
                ModuleState::Unavailable {
                    error: error.to_string(),
                },
            );
        }
        let plan = prepare_story(
            &self.config.pipeline.components.preparation.construct(),
            &inventory,
            &request,
            selection,
            self.config.generation(),
        )?;
        let selection = SelectionReport::from_selection(plan.selection(), seed);
        progress.finish(Step::Selection, started);
        let started = progress.start(Step::Generation);
        let generated = plan.generate(&client).await?;
        progress.finish(Step::Generation, started);
        let assessments = unsafe {
            assessment::assess_optional(
                &self.config,
                &generated,
                plan.assessment_inputs(),
                &mut progress,
            )
        };
        Ok(progress.into_story_report(request, selection, generated, assessments))
    }

    fn validate_story_setup(
        &self,
        needs_source: bool,
        modules: &mut [ModuleReport],
    ) -> Result<(), ApplicationError> {
        let issues: Vec<_> = modules
            .iter_mut()
            .filter_map(|module| {
                let missing = match module.metadata.id {
                    ModuleId::Knowledge => needs_source && !self.config.enabled(ModuleId::Sync),
                    ModuleId::Sync => {
                        module.required = needs_source && self.config.enabled(ModuleId::Sync);
                        module.required
                            && self
                                .credentials
                                .is_missing(crate::configuration::components::SOURCE_KEY)
                    }
                    ModuleId::Generation => self
                        .credentials
                        .is_missing(crate::configuration::components::GENERATION_KEY),
                    _ => false,
                };
                missing.then_some(SetupIssue {
                    module: module.metadata.id,
                })
            })
            .collect();
        if !issues.is_empty() {
            return Err(ApplicationError::Setup { issues });
        }
        Ok(())
    }
}
