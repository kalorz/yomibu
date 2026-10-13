pub mod progress;
use crate::reports::run::{SelectionReport, StoryRunReport};
use progress::{ProgressEvent, RunProgress, Step};
use yomibu_core::{
    domain::{
        generation::{
            SelectionInput, StoryError, StoryGenerationOptions, StoryRequest,
            finalization::select_target_first,
            preparation::{GenerationError, prepare_story},
        },
        knowledge::inventory::LearnerInventory,
    },
    ports::capabilities::{CandidateGenerator, SelectionStep, StoryPreparer},
};

pub struct StoryInputs {
    pub request: StoryRequest,
    pub inventory: LearnerInventory,
}

pub struct StoryPipeline<'a, P, G> {
    pub selection: &'a [&'a dyn SelectionStep],
    pub selection_revision: &'static str,
    pub preparation: &'a P,
    pub generation: &'a G,
}

#[derive(Debug, thiserror::Error)]
pub enum StoryRunError<P, G> {
    #[error(transparent)]
    Story(#[from] StoryError),
    #[error(transparent)]
    Preparation(P),
    #[error(transparent)]
    Generation(GenerationError<G>),
}

pub async fn run_story<P, G>(
    inputs: StoryInputs,
    pipeline: StoryPipeline<'_, P, G>,
    options: StoryGenerationOptions,
    selection_limit: usize,
    seed: u64,
    mut progress: RunProgress<impl FnMut(ProgressEvent)>,
) -> Result<StoryRunReport, StoryRunError<P::Error, G::Error>>
where
    P: StoryPreparer,
    G: CandidateGenerator<PreparedRequest = P::PreparedRequest>,
{
    let StoryInputs { request, inventory } = inputs;
    request.validate_shape()?;
    request.validate_selection_limit(selection_limit)?;
    request.validate(&inventory)?;
    progress.finish_inputs();
    let started = progress.start(Step::Selection);
    let selection = select_target_first(
        SelectionInput {
            inventory: &inventory,
            request: &request,
            limit: selection_limit,
        },
        pipeline.selection,
        pipeline.selection_revision,
    )?;
    let plan = prepare_story(
        pipeline.preparation,
        &inventory,
        &request,
        selection,
        options,
    )
    .map_err(StoryRunError::Preparation)?;
    let selection = SelectionReport::from_selection(plan.selection(), seed);
    progress.finish(Step::Selection, started);
    let started = progress.start(Step::Generation);
    let generated = plan
        .generate(pipeline.generation)
        .await
        .map_err(StoryRunError::Generation)?;
    progress.finish(Step::Generation, started);
    Ok(progress.into_story_report(request, selection, generated))
}
