use super::{
    ApplicationError, Credentials, SetupIssue, inputs,
    progress::{ProgressEvent, RunProgress, Step},
    selection, source,
};
use crate::configuration::Configuration;
use crate::configuration::modules::{ModuleId, ModuleReport, ModuleState};
use crate::reports::run::{SelectionReport, StoryRunReport};
use chrono::{DateTime, Utc};
use yomibu_components::{openai_story_generation as openai, wanikani_source as wanikani};
use yomibu_core::capabilities::LearningStore;
use yomibu_core::domain::{inventory::LearnerInventory, story::StoryRequest};
use yomibu_core::pipeline::story::prepare_story;

pub struct StoryInputs {
    pub request: StoryRequest,
    pub manual: Option<LearnerInventory>,
}

/// Borrowed resources for a supplied story call.
pub struct StoryResources<'a, Store> {
    pub store: Option<&'a Store>,
    pub source_client: Option<&'a mut wanikani::Client>,
    pub generation_client: &'a openai::Client,
}

pub(super) enum GenerationClient<'a> {
    Local {
        credentials: &'a Credentials,
        endpoint: &'a str,
    },
    Supplied(&'a openai::Client),
}

/// Generate from supplied inputs and resolved settings without reading input paths.
/// The caller owns the store and clients; absent source resources prohibit fetching.
pub async fn run_story<Store: LearningStore>(
    config: &Configuration,
    inputs: StoryInputs,
    resources: StoryResources<'_, Store>,
    now: DateTime<Utc>,
    seed: u64,
    emit: impl FnMut(ProgressEvent),
) -> Result<StoryRunReport, ApplicationError>
where
    ApplicationError: From<Store::ReadError> + From<Store::WriteError>,
{
    let StoryResources {
        store,
        source_client,
        generation_client,
    } = resources;
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
        GenerationClient::Supplied(generation_client),
        now,
        seed,
        progress,
        started,
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
    reason = "Keep invocation inputs and resources explicit."
)]
pub(super) async fn execute_story<Store: LearningStore, F: FnMut(ProgressEvent)>(
    config: &Configuration,
    inputs: StoryInputs,
    store: Option<&Store>,
    source_client: source::SourceClient<'_>,
    generation_client: GenerationClient<'_>,
    now: DateTime<Utc>,
    seed: u64,
    mut progress: RunProgress<F>,
    started: std::time::Instant,
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
    let started = progress.start(Step::Selection);
    let selection = selection::select_configured(
        &config.pipeline.selection,
        &inventory,
        &request,
        config.story.select,
        seed,
    )?;
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
    Ok(progress.into_story_report(request, selection, generated))
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
