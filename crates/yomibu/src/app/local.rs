use crate::{
    adapters::{
        dictionary::InstallationError, embedding_cache_file::EmbeddingCacheFileError,
        sudachi::DictionaryError,
    },
    retrieval::{EmbeddingCache, EmbeddingError, EmbeddingModelIdentity},
    story::{StoryAssessmentInputs, StoryPassageAssessment, assess_passages, select_vocabulary},
};
use chrono::{DateTime, Utc};
use serde::{Serialize, de::DeserializeOwned};
use std::{path::Path, time::Instant};
mod explicit;
use super::{
    config::Configuration,
    modules::{MODULES, ModuleId, ModuleReport, ModuleState},
};
use crate::{
    adapters::{input_file, openai, sources::wanikani, stores::file::cache},
    candidate::GeneratedCandidates,
    domain::WaniKaniSyncData,
    inventory::{InventoryError, LearnerInventory, ManualInventory},
    story::{
        PracticeTargets, StoryError, StoryRequest, StoryTopic, fit_selection_and_build_request,
        select_builtin_vocabulary,
    },
};
pub use explicit::{AnalysisRunReport, StoryPreviewRunReport};

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

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Inputs,
    Knowledge,
    Sync,
    Selection,
    Embeddings,
    Generation,
    Assessment,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ProgressEvent {
    Started { step: Step },
    Completed { step: Step, elapsed_ms: f64 },
    Skipped { step: Step, reason: String },
}
#[derive(Debug, Serialize)]
pub struct StepTiming {
    pub step: Step,
    pub elapsed_ms: f64,
}
#[derive(Debug, Serialize)]
pub struct Warning {
    pub module: ModuleId,
    pub message: String,
}
#[derive(Debug, Serialize)]
pub struct SelectionReport {
    pub selector_revision: &'static str,
    pub vocabulary_ids: Vec<String>,
    pub seed: u64,
    pub embedding_model: Option<EmbeddingModelIdentity>,
}
impl SelectionReport {
    fn from_selection(selection: crate::story::StoryVocabularySelection<'_>, seed: u64) -> Self {
        Self {
            selector_revision: selection.selector_revision,
            vocabulary_ids: selection
                .selected
                .iter()
                .map(|entry| entry.word.id.clone())
                .collect(),
            seed,
            embedding_model: selection.embedding_model,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct StoryRunReport {
    pub kind: &'static str,
    pub notice: &'static str,
    pub request: StoryRequest,
    pub selection: SelectionReport,
    pub generated: GeneratedCandidates,
    pub assessments: Vec<StoryPassageAssessment>,
    pub modules: Vec<ModuleReport>,
    pub timings: Vec<StepTiming>,
    pub warnings: Vec<Warning>,
}

struct RunProgress<F> {
    emit: F,
    modules: Vec<ModuleReport>,
    timings: Vec<StepTiming>,
    warnings: Vec<Warning>,
}
impl<F: FnMut(ProgressEvent)> RunProgress<F> {
    fn start(&mut self, step: Step) -> Instant {
        (self.emit)(ProgressEvent::Started { step });
        Instant::now()
    }
    fn finish(&mut self, step: Step, start: Instant) {
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        self.timings.push(StepTiming { step, elapsed_ms });
        (self.emit)(ProgressEvent::Completed { step, elapsed_ms });
    }
    fn skip(&mut self, step: Step, reason: &str) {
        (self.emit)(ProgressEvent::Skipped {
            step,
            reason: reason.into(),
        });
    }
    fn state(&mut self, id: ModuleId, state: ModuleState) {
        if let Some(module) = self
            .modules
            .iter_mut()
            .find(|module| module.metadata.id == id)
        {
            module.state = state;
        }
    }
    fn warn(&mut self, module: ModuleId, message: String) {
        self.warnings.push(Warning { module, message });
    }
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
    /// [`SudachiAnalyzer::load_managed`](crate::adapters::sudachi::SudachiAnalyzer::load_managed)
    /// for this future's duration.
    pub async unsafe fn story(
        &self,
        now: DateTime<Utc>,
        seed: u64,
        emit: impl FnMut(ProgressEvent),
    ) -> Result<StoryRunReport, ApplicationError> {
        let mut progress = RunProgress {
            emit,
            modules: self.catalog(),
            timings: Vec::new(),
            warnings: Vec::new(),
        };
        let started = progress.start(Step::Inputs);
        let request = self.read_request()?;
        request.validate_shape()?;
        request.validate_selection_limit(self.config.select)?;
        let manual = self.read_manual()?;
        let cached = self.read_cache(manual.is_none())?;
        let usable = cached
            .as_ref()
            .is_some_and(|data| self.usable_cache(data, now));
        let needs_source = manual.is_none() && !usable;
        let issues: Vec<_> = progress
            .modules
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
        let source = if manual.is_none() || self.config.wanikani_cache.is_some() {
            self.prepare_source(cached, now, &mut progress).await?
        } else {
            if self.config.enabled(ModuleId::Sync) {
                progress.state(
                    ModuleId::Sync,
                    ModuleState::Skipped {
                        reason: "Manual inventory selected".into(),
                    },
                );
            }
            progress.skip(Step::Sync, "Manual inventory selected");
            None
        };
        let started = progress.start(Step::Knowledge);
        let inventory = self.derive_inventory(source.as_ref(), manual, now)?;
        progress.state(ModuleId::Knowledge, ModuleState::Available);
        progress.finish(Step::Knowledge, started);
        let seed = self.config.seed.unwrap_or(seed);
        request.validate(&inventory)?;
        let cache = self
            .optional_embeddings(&inventory, &request, &mut progress)
            .await;
        let started = progress.start(Step::Selection);
        let selection = match cache.as_ref().map(|cache| {
            select_vocabulary(
                &inventory,
                &request,
                cache,
                &cache.model,
                self.config.select,
            )
        }) {
            Some(Ok(selection)) => selection,
            Some(Err(error)) => {
                progress.warn(
                    ModuleId::Embeddings,
                    format!("{error} Using built-in selection."),
                );
                progress.state(
                    ModuleId::Embeddings,
                    ModuleState::Unavailable {
                        error: error.to_string(),
                    },
                );
                select_builtin_vocabulary(&inventory, &request, self.config.select, seed)?
            }
            None => select_builtin_vocabulary(&inventory, &request, self.config.select, seed)?,
        };
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
        let assessments = unsafe { self.optional_assessment(&generated, &inputs, &mut progress) };
        Ok(StoryRunReport {
            kind: "experimental_story",
            notice: "Experimental reading; verification and acceptance have not run.",
            request,
            selection,
            generated,
            assessments,
            modules: progress.modules,
            timings: progress.timings,
            warnings: progress.warnings,
        })
    }

    fn catalog(&self) -> Vec<ModuleReport> {
        MODULES
            .iter()
            .map(|metadata| ModuleReport {
                metadata,
                required: matches!(metadata.id, ModuleId::Knowledge | ModuleId::Generation),
                state: if self.config.enabled(metadata.id) {
                    ModuleState::NotConfigured
                } else {
                    ModuleState::Disabled
                },
            })
            .collect()
    }
    fn cache_path(&self) -> std::path::PathBuf {
        self.config
            .wanikani_cache
            .clone()
            .unwrap_or_else(|| self.config.data_dir.join("wanikani.json"))
    }
    fn read_manual(&self) -> Result<Option<LearnerInventory>, ApplicationError> {
        self.config
            .inventory
            .as_deref()
            .map(|path| {
                Ok(LearnerInventory::from_manual(
                    read_json::<ManualInventory>(path, "manual inventory", 4194304)?,
                )?)
            })
            .transpose()
    }
    fn read_cache(&self, use_default: bool) -> Result<Option<WaniKaniSyncData>, ApplicationError> {
        if !use_default && self.config.wanikani_cache.is_none() {
            return Ok(None);
        }
        match cache::load_path(&self.cache_path()) {
            Ok(data) => Ok(Some(data)),
            Err(cache::CacheError::Missing { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    fn usable_cache(&self, data: &WaniKaniSyncData, now: DateTime<Utc>) -> bool {
        !access_expired(data, now)
            && LearnerInventory::from_wanikani(data, &self.config.knowledge_policy)
                .is_ok_and(|inventory| !inventory.vocabulary.is_empty())
    }
    fn fresh(&self, data: &WaniKaniSyncData, now: DateTime<Utc>) -> bool {
        !access_expired(data, now)
            && now
                .signed_duration_since(data.sync_completed_at)
                .to_std()
                .unwrap_or_default()
                < self.config.cache_max_age
    }

    async fn prepare_source<F: FnMut(ProgressEvent)>(
        &self,
        previous: Option<WaniKaniSyncData>,
        now: DateTime<Utc>,
        progress: &mut RunProgress<F>,
    ) -> Result<Option<WaniKaniSyncData>, ApplicationError> {
        if previous.as_ref().is_some_and(|data| self.fresh(data, now)) {
            progress.state(
                ModuleId::Sync,
                if self.config.enabled(ModuleId::Sync) {
                    ModuleState::Skipped {
                        reason: "Cache is fresh".into(),
                    }
                } else {
                    ModuleState::Disabled
                },
            );
            progress.skip(Step::Sync, "Cache is fresh");
            return Ok(previous);
        }
        if !self.config.enabled(ModuleId::Sync) || self.credentials.wanikani.is_none() {
            if previous
                .as_ref()
                .is_some_and(|data| access_expired(data, now))
            {
                return Err(ApplicationError::AccessExpired);
            }
            if previous.is_some() {
                progress.warn(
                    ModuleId::Sync,
                    "Using an old WaniKani cache; sync is disabled or no key was supplied.".into(),
                );
            }
            progress.skip(Step::Sync, "Sync disabled or no WaniKani key");
            return Ok(previous);
        }
        let started = progress.start(Step::Sync);
        let mut client = wanikani::Client::with_base_url(
            self.credentials
                .wanikani
                .as_deref()
                .ok_or_else(|| ApplicationError::Setup {
                    issues: vec![SetupIssue {
                        module: ModuleId::Sync,
                    }],
                })?,
            &self.endpoints.wanikani,
        )?;
        let guard = match cache::SyncGuard::acquire_path(&self.cache_path()) {
            Ok(guard) => guard,
            Err(cache::WriteError::Locked)
                if previous
                    .as_ref()
                    .is_some_and(|data| self.usable_cache(data, now)) =>
            {
                progress.warn(
                    ModuleId::Sync,
                    "Another writer is refreshing WaniKani; using the previous usable cache."
                        .into(),
                );
                progress.state(
                    ModuleId::Sync,
                    ModuleState::Skipped {
                        reason: "Writer contention".into(),
                    },
                );
                progress.skip(Step::Sync, "Writer contention");
                return Ok(previous);
            }
            Err(error) => return Err(error.into()),
        };
        let rechecked = self.read_cache(true)?;
        if rechecked.as_ref().is_some_and(|data| self.fresh(data, now)) {
            progress.state(
                ModuleId::Sync,
                ModuleState::Skipped {
                    reason: "Another writer refreshed the cache".into(),
                },
            );
            progress.finish(Step::Sync, started);
            return Ok(rechecked);
        }
        let previous = rechecked.or(previous);
        match client.fetch().await {
            Ok(data) => {
                if access_expired(&data, now) {
                    return Err(ApplicationError::AccessExpired);
                }
                guard.replace(&data)?;
                progress.state(ModuleId::Sync, ModuleState::Available);
                progress.finish(Step::Sync, started);
                Ok(Some(data))
            }
            Err(error)
                if temporary_source_error(&error)
                    && previous
                        .as_ref()
                        .is_some_and(|data| self.usable_cache(data, now)) =>
            {
                progress.warn(
                    ModuleId::Sync,
                    format!("{error} Using the previous usable cache."),
                );
                progress.state(
                    ModuleId::Sync,
                    ModuleState::Unavailable {
                        error: error.to_string(),
                    },
                );
                progress.finish(Step::Sync, started);
                Ok(previous)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn derive_inventory(
        &self,
        source: Option<&WaniKaniSyncData>,
        manual: Option<LearnerInventory>,
        now: DateTime<Utc>,
    ) -> Result<LearnerInventory, ApplicationError> {
        if source.is_some_and(|source| access_expired(source, now)) {
            return Err(ApplicationError::AccessExpired);
        }
        match (source, manual) {
            (Some(source), Some(manual)) => Ok(LearnerInventory::from_wanikani(
                source,
                &self.config.knowledge_policy,
            )?
            .merge(manual)?),
            (Some(source), None) => Ok(LearnerInventory::from_wanikani(
                source,
                &self.config.knowledge_policy,
            )?),
            (None, Some(manual)) => Ok(manual),
            (None, None) => Err(ApplicationError::Setup {
                issues: vec![SetupIssue {
                    module: ModuleId::Knowledge,
                }],
            }),
        }
    }
    fn read_request(&self) -> Result<StoryRequest, ApplicationError> {
        if let Some(path) = &self.config.request {
            return read_json(path, "story request", 65536);
        }
        Ok(StoryRequest {
            version: 1,
            topic: self
                .config
                .topic
                .as_ref()
                .map(|text| StoryTopic::new(text.clone()))
                .transpose()?,
            targets: PracticeTargets {
                vocabulary: Vec::new(),
                grammar: Vec::new(),
            },
        })
    }

    pub async fn prepare_retrieval(
        &self,
        now: DateTime<Utc>,
    ) -> Result<EmbeddingCache, ApplicationError> {
        let manual = self.read_manual()?;
        let source = self.read_cache(manual.is_none())?;
        let inventory = self.derive_inventory(source.as_ref(), manual, now)?;
        let request = self.read_request()?;
        request.validate_selection_limit(self.config.select)?;
        super::resources::prepare_embeddings(
            &self.config,
            self.credentials.openai.as_deref(),
            &inventory,
            &request,
        )
        .await
    }

    async fn optional_embeddings<F: FnMut(ProgressEvent)>(
        &self,
        inventory: &LearnerInventory,
        request: &StoryRequest,
        progress: &mut RunProgress<F>,
    ) -> Option<EmbeddingCache> {
        if !self.config.enabled(ModuleId::Embeddings) {
            progress.skip(Step::Embeddings, "Embeddings disabled");
            return None;
        }
        if request.topic.is_none() {
            progress.state(
                ModuleId::Embeddings,
                ModuleState::Skipped {
                    reason: "No topic; query retrieval is unnecessary".into(),
                },
            );
            progress.skip(Step::Embeddings, "No topic");
            return None;
        }
        let started = progress.start(Step::Embeddings);
        let result = super::resources::prepare_embeddings(
            &self.config,
            self.credentials.openai.as_deref(),
            inventory,
            request,
        )
        .await;
        progress.finish(Step::Embeddings, started);
        match result {
            Ok(cache) => {
                progress.state(ModuleId::Embeddings, ModuleState::Available);
                Some(cache)
            }
            Err(error) => {
                progress.warn(
                    ModuleId::Embeddings,
                    format!("{error} Using built-in selection."),
                );
                progress.state(
                    ModuleId::Embeddings,
                    match error {
                        ApplicationError::ResourceConfiguration(_) => ModuleState::NotConfigured,
                        _ => ModuleState::Unavailable {
                            error: error.to_string(),
                        },
                    },
                );
                None
            }
        }
    }

    unsafe fn optional_assessment<F: FnMut(ProgressEvent)>(
        &self,
        generated: &GeneratedCandidates,
        inputs: &StoryAssessmentInputs<'_>,
        progress: &mut RunProgress<F>,
    ) -> Vec<StoryPassageAssessment> {
        if !self.config.enabled(ModuleId::Assessment) {
            progress.skip(Step::Assessment, "Assessment disabled");
            return assess_passages(generated.passages(), inputs, None);
        }
        if self.config.dictionary.is_none() && !self.config.dictionary_dir.exists() {
            progress.skip(Step::Assessment, "No dictionary configured");
            return assess_passages(generated.passages(), inputs, None);
        }
        let started = progress.start(Step::Assessment);
        let analyzer = match unsafe { super::resources::load_analyzer(&self.config) } {
            Ok(analyzer) => {
                progress.state(ModuleId::Assessment, ModuleState::Available);
                Some(analyzer)
            }
            Err(error) => {
                progress.warn(ModuleId::Assessment, error.to_string());
                progress.state(
                    ModuleId::Assessment,
                    ModuleState::Unavailable {
                        error: error.to_string(),
                    },
                );
                None
            }
        };
        let assessments = assess_passages(generated.passages(), inputs, analyzer.as_ref());
        for sentence in assessments.iter().flat_map(|passage| &passage.sentences) {
            if let crate::candidate::CandidateAssessment::ExecutionError { error, .. } =
                &sentence.assessment.assessment
            {
                progress.warn(ModuleId::Assessment, error.to_string());
                progress.state(
                    ModuleId::Assessment,
                    ModuleState::Unavailable {
                        error: error.to_string(),
                    },
                );
            }
        }
        progress.finish(Step::Assessment, started);
        assessments
    }
}

fn access_expired(data: &WaniKaniSyncData, now: DateTime<Utc>) -> bool {
    data.learner.subscription.active
        && data
            .learner
            .subscription
            .period_ends_at
            .is_some_and(|expiry| now >= expiry)
}
fn temporary_source_error(error: &wanikani::Error) -> bool {
    matches!(
        error,
        wanikani::Error::Transport { .. }
            | wanikani::Error::RateLimitWait
            | wanikani::Error::Http {
                status: 429 | 500..=599,
                ..
            }
    )
}
pub(super) fn read_json<T: DeserializeOwned>(
    path: &Path,
    kind: &'static str,
    limit: usize,
) -> Result<T, ApplicationError> {
    let bytes = input_file::read_bounded(path, kind, limit, &format!("{limit} bytes"))?;
    serde_json::from_slice(&bytes).map_err(|source| ApplicationError::InvalidJson { kind, source })
}
