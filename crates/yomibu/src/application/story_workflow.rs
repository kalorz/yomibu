use super::{
    ApplicationError, Credentials, SetupIssue, assessment, embeddings, inputs,
    progress::{ProgressEvent, RunProgress, Step},
    source,
};
use crate::configuration::Configuration;
use crate::configuration::modules::{ModuleId, ModuleReport, ModuleState};
use crate::reports::run::{SelectionReport, StoryRunReport};
use chrono::{DateTime, Utc};
use yomibu_components::{
    openai_story_generation as openai, sudachi_dictionary::SudachiAnalyzer,
    wanikani_source as wanikani,
};
use yomibu_core::capabilities::LearningStore;
use yomibu_core::domain::{
    candidate::GeneratedCandidates,
    embedding::EmbeddingCache,
    inventory::LearnerInventory,
    story::{StoryAssessmentInputs, StoryPassageAssessment, StoryRequest},
};
use yomibu_core::pipeline::story::prepare_story;

pub struct StoryInputs {
    pub request: StoryRequest,
    pub manual: Option<LearnerInventory>,
}

pub(super) enum StoryEmbeddings<'a> {
    Local(&'a Credentials),
    Supplied(Option<&'a EmbeddingCache>),
}

pub(super) enum GenerationClient<'a> {
    Local {
        credentials: &'a Credentials,
        endpoint: &'a str,
    },
    Supplied(&'a openai::Client),
}

/// Generate from supplied inputs and resolved settings without reading configuration
/// files or configured request, inventory, or dictionary paths. Reuse an initialized
/// analyzer across calls; `None` skips analysis. Disabled assessment ignores it.
/// `Some(store)` participates in source preparation; `None` excludes source data.
/// `source_client` permits refresh through that mutable client; `None` forbids
/// fetching. Application WaniKani credentials and endpoint are never consulted.
/// `client` owns generation credentials and endpoint; this call
/// uses model and generation options from the resolved configuration.
/// `embedding_cache` supplies prepared vectors. Missing or incompatible evidence
/// warns and uses base selection when embeddings are enabled with a topic.
/// Embedding paths, credentials and providers are never used for acquisition.
#[expect(
    clippy::too_many_arguments,
    reason = "Keep supplied resources and per-call inputs explicit."
)]
pub async fn run_story<Store: LearningStore>(
    config: &Configuration,
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
    let mut progress = RunProgress::new(config, emit);
    let started = progress.start(Step::Inputs);
    validate_story_request(config, &inputs.request)?;
    if let Some(manual) = &inputs.manual {
        manual.validate()?;
    } else if store.is_none() {
        return Err(ApplicationError::Setup {
            issues: vec![SetupIssue {
                module: ModuleId::Knowledge,
            }],
        });
    }
    execute_story(
        config,
        inputs,
        store,
        source_client.map_or(
            source::SourceClient::Unavailable,
            source::SourceClient::Supplied,
        ),
        GenerationClient::Supplied(client),
        StoryEmbeddings::Supplied(embedding_cache),
        now,
        seed,
        progress,
        started,
        |generated, inputs, progress| {
            assessment::assess_optional(config, generated, inputs, analyzer, progress)
        },
    )
    .await
}

pub(super) fn validate_story_request(
    config: &Configuration,
    request: &StoryRequest,
) -> Result<(), ApplicationError> {
    request.validate_shape()?;
    request.validate_selection_limit(config.story.select)?;
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "Keep invocation inputs and deferred assessment explicit."
)]
pub(super) async fn execute_story<Store: LearningStore, F: FnMut(ProgressEvent)>(
    config: &Configuration,
    inputs: StoryInputs,
    store: Option<&Store>,
    source_client: source::SourceClient<'_>,
    generation_client: GenerationClient<'_>,
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
    let usable = cached
        .as_ref()
        .is_some_and(|data| source::usable_cache(data, &config.pipeline.knowledge_policy, now));
    let needs_source = manual.is_none() && !usable;
    validate_story_setup(
        config,
        needs_source,
        &source_client,
        &generation_client,
        &mut progress.modules,
    )?;
    let local_client;
    let client = match generation_client {
        GenerationClient::Supplied(client) => client,
        GenerationClient::Local {
            credentials,
            endpoint,
        } => {
            local_client = config.pipeline.components.generation.client(
                credentials
                    .generation()?
                    .ok_or_else(|| ApplicationError::Setup {
                        issues: vec![SetupIssue {
                            module: ModuleId::Generation,
                        }],
                    })?,
                endpoint,
            )?;
            &local_client
        }
    };
    progress.state(ModuleId::Generation, ModuleState::Available);
    progress.finish(Step::Inputs, started);
    let source =
        source::prepare_source(config, source_client, store, cached, now, &mut progress).await?;
    let started = progress.start(Step::Knowledge);
    let inventory = inputs::prepare_inventory(
        &config.pipeline.knowledge_policy,
        source.as_deref(),
        manual,
        now,
    )?;
    progress.state(ModuleId::Knowledge, ModuleState::Available);
    progress.finish(Step::Knowledge, started);
    let seed = config.story.seed.unwrap_or(seed);
    request.validate(&inventory)?;
    let local_cache;
    let cache = if let Some(started) = embeddings::start_optional(config, &request, &mut progress) {
        let result = match embedding_source {
            StoryEmbeddings::Local(credentials) => {
                match embeddings::prepare_embeddings(config, credentials, &inventory, &request)
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
                embeddings::validate_supplied(config, &inventory, &request, cache)
            }
        };
        progress.finish(Step::Embeddings, started);
        match result {
            Ok(cache) => {
                progress.state(ModuleId::Embeddings, ModuleState::Available);
                Some(cache)
            }
            Err(error) => {
                embeddings::report_fallback(config, &error, &mut progress);
                None
            }
        }
    } else {
        None
    };
    let started = progress.start(Step::Selection);
    let (selection, retrieval_error) = embeddings::select_for_request(
        &config.pipeline.selection,
        &inventory,
        &request,
        cache,
        config.story.select,
        seed,
    )?;
    if let Some(error) = retrieval_error {
        embeddings::report_fallback(config, &error.into(), &mut progress);
    }
    let plan = prepare_story(
        &config.pipeline.components.preparation.construct(),
        &inventory,
        &request,
        selection,
        config.generation()?,
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
    config: &Configuration,
    needs_source: bool,
    source_client: &source::SourceClient<'_>,
    generation_client: &GenerationClient<'_>,
    modules: &mut [ModuleReport],
) -> Result<(), ApplicationError> {
    let issues: Vec<_> = modules
        .iter_mut()
        .filter_map(|module| {
            let missing = match module.metadata.id {
                ModuleId::Knowledge => {
                    needs_source
                        && (!config.enabled(ModuleId::Sync)
                            || matches!(source_client, source::SourceClient::Unavailable))
                }
                ModuleId::Sync => {
                    module.required = needs_source
                        && config.enabled(ModuleId::Sync)
                        && !matches!(source_client, source::SourceClient::Unavailable);
                    module.required
                        && match source_client {
                            source::SourceClient::Local { credentials, .. } => {
                                credentials.is_missing(crate::configuration::components::SOURCE_KEY)
                            }
                            _ => false,
                        }
                }
                ModuleId::Generation => match generation_client {
                    GenerationClient::Local { credentials, .. } => {
                        credentials.is_missing(crate::configuration::components::GENERATION_KEY)
                    }
                    GenerationClient::Supplied(_) => false,
                },
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
