use super::{
    assessment,
    config::Configuration,
    embeddings, inputs,
    modules::{ModuleId, ModuleReport, ModuleState},
    reporting::RunProgress,
    source,
};
use crate::{
    adapters::{
        dictionary::InstallationError, embedding_cache_file::EmbeddingCacheFileError, input_file,
        openai, sources::wanikani, stores::file::cache, sudachi::DictionaryError,
    },
    inventory::InventoryError,
    retrieval::EmbeddingError,
    story::{StoryAssessmentInputs, StoryError, fit_selection_and_build_request},
};
use chrono::{DateTime, Utc};
mod explicit;

pub use super::reporting::{
    AnalysisRunReport, ProgressEvent, SelectionReport, Step, StepTiming, StoryPreviewRunReport,
    StoryRunReport, Warning,
};

#[derive(Default)]
pub struct Credentials {
    wanikani: Option<String>,
    openai: Option<String>,
}
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credentials { redacted }")
    }
}
impl Credentials {
    pub fn new(wanikani: Option<String>, openai: Option<String>) -> Self {
        Self {
            wanikani: wanikani.filter(|key| !key.trim().is_empty()),
            openai: openai.filter(|key| !key.trim().is_empty()),
        }
    }
}

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
pub struct SetupIssue {
    pub module: ModuleId,
}
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
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
    Validation(#[from] crate::domain::ValidationError),
    #[error(transparent)]
    Sentence(#[from] crate::analysis::SentenceError),
    #[error(transparent)]
    Analysis(#[from] crate::adapters::sudachi::AnalysisError),
    #[error(transparent)]
    Grammar(#[from] crate::grammar::GrammarError),
    #[error(transparent)]
    Evaluation(#[from] crate::evaluation::EvaluationError),
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

    /// # Safety
    /// Selected managed dictionaries must satisfy
    /// [`SudachiAnalyzer::load`](crate::adapters::sudachi::SudachiAnalyzer::load)
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
        request.validate_selection_limit(self.config.select)?;
        let manual = inputs::read_manual(self.config.inventory.as_deref())?;
        let cached = source::read_cache(&self.config, manual.is_none())?;
        let usable = cached
            .as_ref()
            .is_some_and(|data| source::usable_cache(data, &self.config.knowledge_policy, now));
        let needs_source = manual.is_none() && !usable;
        self.validate_story_setup(needs_source, &mut progress.modules)?;
        let client = openai::Client::with_base_url(
            self.credentials
                .openai
                .as_deref()
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
            self.credentials.wanikani.as_deref(),
            &self.endpoints.wanikani,
            manual.is_some(),
            cached,
            now,
            &mut progress,
        )
        .await?;
        let started = progress.start(Step::Knowledge);
        let inventory =
            inputs::prepare_inventory(&self.config.knowledge_policy, source.as_ref(), manual, now)?;
        progress.state(ModuleId::Knowledge, ModuleState::Available);
        progress.finish(Step::Knowledge, started);
        let seed = self.config.seed.unwrap_or(seed);
        request.validate(&inventory)?;
        let cache = embeddings::prepare_optional(
            &self.config,
            self.credentials.openai.as_deref(),
            &inventory,
            &request,
            &mut progress,
        )
        .await;
        let started = progress.start(Step::Selection);
        let (selection, retrieval_error) = embeddings::select_for_request(
            &inventory,
            &request,
            cache.as_ref(),
            self.config.select,
            seed,
        )?;
        if let Some(error) = retrieval_error {
            progress.warnings.push(Warning::embedding_fallback(&error));
            progress.state(
                ModuleId::Embeddings,
                ModuleState::Unavailable {
                    error: error.to_string(),
                },
            );
        }
        let (selection, prepared) = fit_selection_and_build_request(
            &inventory,
            &request,
            selection,
            self.config.generation.clone(),
        )?;
        let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection)?;
        let selection = SelectionReport::from_selection(selection, seed);
        progress.finish(Step::Selection, started);
        let started = progress.start(Step::Generation);
        let generated = client.generate_candidates(&prepared).await?;
        progress.finish(Step::Generation, started);
        let assessments = unsafe {
            assessment::assess_optional(&self.config, &generated, &inputs, &mut progress)
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
                        module.required && self.credentials.wanikani.is_none()
                    }
                    ModuleId::Generation => self.credentials.openai.is_none(),
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
