pub mod progress;
use crate::reports::run::{SelectionReport, StoryRunReport};
use chrono::{DateTime, Utc};
use progress::{ProgressEvent, RunProgress, Step};
use yomibu_core::{
    domain::{
        generation::{
            SelectionInput, StoryError, StoryGenerationOptions, StoryRequest,
            finalization::select_target_first,
            preparation::{GenerationError, prepare_story},
        },
        knowledge::{inventory::LearnerInventory, learner::LearnerData},
    },
    ports::capabilities::{CandidateGenerator, SelectionStep, StoryPreparer},
};

pub struct StoryInputs {
    pub request: StoryRequest,
    pub inventory: LearnerInventory,
}

impl StoryInputs {
    /// Prepare and check learner data before resolving external dependencies.
    pub fn from_learner_data(
        request: StoryRequest,
        data: LearnerData,
        now: DateTime<Utc>,
    ) -> Result<Self, StoryError> {
        let inventory = data.into_inventory(now)?;
        request.validate(&inventory)?;
        Ok(Self { request, inventory })
    }
}

pub struct StoryPipeline<'a, P, G> {
    pub selection: &'a [&'a dyn SelectionStep],
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
    mut progress: RunProgress<impl FnMut(ProgressEvent)>,
) -> Result<StoryRunReport, StoryRunError<P::Error, G::Error>>
where
    P: StoryPreparer,
    G: CandidateGenerator<PreparedRequest = P::PreparedRequest>,
{
    let StoryInputs { request, inventory } = inputs;
    progress.finish_inputs();
    let started = progress.start(Step::Selection);
    let selection = select_target_first(
        SelectionInput {
            inventory: &inventory,
            request: &request,
            limit: selection_limit,
        },
        pipeline.selection,
    )?;
    let plan = prepare_story(
        pipeline.preparation,
        &inventory,
        &request,
        selection,
        options,
    )
    .map_err(StoryRunError::Preparation)?;
    let selection = SelectionReport::from_selection(plan.selection(), pipeline.selection);
    progress.finish(Step::Selection, started);
    let started = progress.start(Step::Generation);
    let generated = plan
        .generate(pipeline.generation)
        .await
        .map_err(StoryRunError::Generation)?;
    progress.finish(Step::Generation, started);
    Ok(progress.into_story_report(request, selection, generated))
}
