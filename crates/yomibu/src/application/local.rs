use super::{assessment, embeddings, inputs, progress::RunProgress, source};
use crate::application::input_file;
use crate::configuration::Configuration;
use crate::configuration::modules::{ModuleId, ModuleReport, ModuleState};
use chrono::{DateTime, Utc};
use yomibu_components::{
    file_embedding_cache::EmbeddingCacheFileError,
    file_learning_store::cache,
    openai_story_generation as openai,
    sudachi_dictionary::{DictionaryError, SudachiAnalyzer, installation::InstallationError},
    wanikani_source as wanikani,
};
use yomibu_core::capabilities::LearningStore;
use yomibu_core::domain::{
    candidate::GeneratedCandidates,
    embedding::{EmbeddingCache, EmbeddingError},
    inventory::{InventoryError, LearnerInventory},
    story::{StoryAssessmentInputs, StoryError, StoryPassageAssessment, StoryRequest},
};
use yomibu_core::pipeline::story::prepare_story;
mod explicit;

use super::progress::{ProgressEvent, Step};
use crate::reports::run::{SelectionReport, StoryRunReport};

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

pub struct StoryInputs {
    pub request: StoryRequest,
    pub manual: Option<LearnerInventory>,
}

enum StoryEmbeddings<'a> {
    Local,
    Supplied(Option<&'a EmbeddingCache>),
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
        self.validate_story_request(&request)?;
        let manual = inputs::read_manual(self.config.application.inventory.as_deref())?;
        let store =
            (manual.is_none() || self.config.application.wanikani_cache.is_some()).then_some(store);
        self.execute_story(
            StoryInputs { request, manual },
            store,
            source::SourceClient::Local {
                credentials: &self.credentials,
                endpoint: &self.endpoints.wanikani,
            },
            None,
            StoryEmbeddings::Local,
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

    /// Generate from supplied inputs without reading configured request, inventory,
    /// or dictionary paths. Reuse an initialized analyzer across calls; `None`
    /// skips analysis. Disabled assessment ignores the supplied analyzer.
    /// `Some(store)` participates in source preparation; `None` excludes source data.
    /// `source_client` permits refresh through that mutable client; `None` forbids
    /// fetching. Application WaniKani credentials and endpoint are never consulted.
    /// `client` owns generation credentials and endpoint; this call
    /// resolves model and generation options from its invocation configuration.
    /// `embedding_cache` supplies prepared vectors. Missing or incompatible evidence
    /// warns and uses base selection when embeddings are enabled with a topic.
    /// Embedding paths, credentials and providers are never used for acquisition.
    #[expect(
        clippy::too_many_arguments,
        reason = "Keep supplied resources and per-call inputs explicit."
    )]
    pub async fn story_with_inputs<Store: LearningStore>(
        &self,
        inputs: StoryInputs,
        store: Option<&Store>,
        source_client: Option<&mut wanikani::Client>,
        client: &openai::Client,
        analyzer: Option<&SudachiAnalyzer>,
        embedding_cache: Option<&EmbeddingCache>,
        now: DateTime<Utc>,
        seed: u64,
        emit: impl FnMut(ProgressEvent),
    ) -> Result<StoryRunReport, ApplicationError>
    where
        ApplicationError: From<Store::ReadError> + From<Store::WriteError>,
    {
        let mut progress = RunProgress::new(&self.config, emit);
        let started = progress.start(Step::Inputs);
        self.validate_story_request(&inputs.request)?;
        if let Some(manual) = &inputs.manual {
            manual.validate()?;
        } else if store.is_none() {
            return Err(ApplicationError::Setup {
                issues: vec![SetupIssue {
                    module: ModuleId::Knowledge,
                }],
            });
        }
        self.execute_story(
            inputs,
            store,
            source_client.map_or(
                source::SourceClient::Unavailable,
                source::SourceClient::Supplied,
            ),
            Some(client),
            StoryEmbeddings::Supplied(embedding_cache),
            now,
            seed,
            progress,
            started,
            |generated, inputs, progress| {
                assessment::assess_optional(&self.config, generated, inputs, analyzer, progress)
            },
        )
        .await
    }

    fn validate_story_request(&self, request: &StoryRequest) -> Result<(), ApplicationError> {
        request.validate_shape()?;
        request.validate_selection_limit(self.config.story.select)?;
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Keep invocation inputs and deferred assessment explicit."
    )]
    async fn execute_story<Store: LearningStore, F: FnMut(ProgressEvent)>(
        &self,
        inputs: StoryInputs,
        store: Option<&Store>,
        source_client: source::SourceClient<'_>,
        supplied_client: Option<&openai::Client>,
        embedding_source: StoryEmbeddings<'_>,
        now: DateTime<Utc>,
        seed: u64,
        mut progress: RunProgress<F>,
        started: std::time::Instant,
        assess: impl FnOnce(
            &GeneratedCandidates,
            &StoryAssessmentInputs<'_>,
            &mut RunProgress<F>,
        ) -> Vec<StoryPassageAssessment<super::story::DefaultCandidateError>>,
    ) -> Result<StoryRunReport, ApplicationError>
    where
        ApplicationError: From<Store::ReadError> + From<Store::WriteError>,
    {
        let StoryInputs { request, manual } = inputs;
        let cached = store.map(source::load_cache).transpose()?.flatten();
        let usable = cached.as_ref().is_some_and(|data| {
            source::usable_cache(data, &self.config.pipeline.knowledge_policy, now)
        });
        let needs_source = manual.is_none() && !usable;
        self.validate_story_setup(
            needs_source,
            &source_client,
            supplied_client.is_none(),
            &mut progress.modules,
        )?;
        let local_client;
        let client = match supplied_client {
            Some(client) => client,
            None => {
                local_client = self.config.pipeline.components.generation.client(
                    self.credentials
                        .generation()?
                        .ok_or_else(|| ApplicationError::Setup {
                            issues: vec![SetupIssue {
                                module: ModuleId::Generation,
                            }],
                        })?,
                    &self.endpoints.openai,
                )?;
                &local_client
            }
        };
        progress.state(ModuleId::Generation, ModuleState::Available);
        progress.finish(Step::Inputs, started);
        let source = source::prepare_source(
            &self.config,
            source_client,
            store,
            cached,
            now,
            &mut progress,
        )
        .await?;
        let started = progress.start(Step::Knowledge);
        let inventory = inputs::prepare_inventory(
            &self.config.pipeline.knowledge_policy,
            source.as_deref(),
            manual,
            now,
        )?;
        progress.state(ModuleId::Knowledge, ModuleState::Available);
        progress.finish(Step::Knowledge, started);
        let seed = self.config.story.seed.unwrap_or(seed);
        request.validate(&inventory)?;
        let local_cache;
        let cache = if let Some(started) =
            embeddings::start_optional(&self.config, &request, &mut progress)
        {
            let result = match embedding_source {
                StoryEmbeddings::Local => {
                    match embeddings::prepare_embeddings(
                        &self.config,
                        &self.credentials,
                        &inventory,
                        &request,
                    )
                    .await
                    {
                        Ok(cache) => {
                            local_cache = cache;
                            Ok(&local_cache)
                        }
                        Err(error) => Err(error),
                    }
                }
                StoryEmbeddings::Supplied(cache) => {
                    embeddings::validate_supplied(&self.config, &inventory, &request, cache)
                }
            };
            progress.finish(Step::Embeddings, started);
            match result {
                Ok(cache) => {
                    progress.state(ModuleId::Embeddings, ModuleState::Available);
                    Some(cache)
                }
                Err(error) => {
                    embeddings::report_fallback(&self.config, &error, &mut progress);
                    None
                }
            }
        } else {
            None
        };
        let started = progress.start(Step::Selection);
        let (selection, retrieval_error) = embeddings::select_for_request(
            &self.config.pipeline.selection,
            &inventory,
            &request,
            cache,
            self.config.story.select,
            seed,
        )?;
        if let Some(error) = retrieval_error {
            embeddings::report_fallback(&self.config, &error.into(), &mut progress);
        }
        let plan = prepare_story(
            &self.config.pipeline.components.preparation.construct(),
            &inventory,
            &request,
            selection,
            self.config.generation()?,
        )?;
        let selection = SelectionReport::from_selection(plan.selection(), seed);
        progress.finish(Step::Selection, started);
        let started = progress.start(Step::Generation);
        let generated = plan.generate(client).await?;
        progress.finish(Step::Generation, started);
        let assessments = assess(&generated, plan.assessment_inputs(), &mut progress);
        Ok(progress.into_story_report(request, selection, generated, assessments))
    }

    fn validate_story_setup(
        &self,
        needs_source: bool,
        source_client: &source::SourceClient<'_>,
        needs_local_generation: bool,
        modules: &mut [ModuleReport],
    ) -> Result<(), ApplicationError> {
        let issues: Vec<_> = modules
            .iter_mut()
            .filter_map(|module| {
                let missing = match module.metadata.id {
                    ModuleId::Knowledge => {
                        needs_source
                            && (!self.config.enabled(ModuleId::Sync)
                                || matches!(source_client, source::SourceClient::Unavailable))
                    }
                    ModuleId::Sync => {
                        module.required = needs_source
                            && self.config.enabled(ModuleId::Sync)
                            && !matches!(source_client, source::SourceClient::Unavailable);
                        module.required
                            && match source_client {
                                source::SourceClient::Local { credentials, .. } => credentials
                                    .is_missing(crate::configuration::components::SOURCE_KEY),
                                _ => false,
                            }
                    }
                    ModuleId::Generation => {
                        needs_local_generation
                            && self
                                .credentials
                                .is_missing(crate::configuration::components::GENERATION_KEY)
                    }
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
