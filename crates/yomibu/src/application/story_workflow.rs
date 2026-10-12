use super::{
    ApplicationError,
    progress::{ProgressEvent, RunProgress, Step},
    selection,
};
use crate::{
    configuration::Configuration,
    reports::run::{SelectionReport, StoryRunReport},
};
use yomibu_components::openai_story_generation as openai;
use yomibu_core::{
    domain::{inventory::LearnerInventory, story::StoryRequest},
    pipeline::story::prepare_story,
};

pub struct StoryInputs {
    pub request: StoryRequest,
    pub inventory: LearnerInventory,
}

pub async fn run_story(
    config: &Configuration,
    inputs: StoryInputs,
    client: &openai::Client,
    seed: u64,
    emit: impl FnMut(ProgressEvent),
) -> Result<StoryRunReport, ApplicationError> {
    let mut progress = RunProgress::new(emit);
    let started = progress.start(Step::Inputs);
    validate_story_request(config, &inputs.request)?;
    inputs.request.validate(&inputs.inventory)?;
    progress.finish(Step::Inputs, started);
    execute_story(config, inputs, client, seed, progress).await
}

pub(super) fn validate_story_request(
    config: &Configuration,
    request: &StoryRequest,
) -> Result<(), ApplicationError> {
    request.validate_shape()?;
    request.validate_selection_limit(config.story.select)?;
    Ok(())
}

pub(super) async fn execute_story<F: FnMut(ProgressEvent)>(
    config: &Configuration,
    inputs: StoryInputs,
    client: &openai::Client,
    seed: u64,
    mut progress: RunProgress<F>,
) -> Result<StoryRunReport, ApplicationError> {
    let StoryInputs { request, inventory } = inputs;
    let seed = config.story.seed.unwrap_or(seed);
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
