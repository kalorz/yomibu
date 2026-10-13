use super::ApplicationError;
use super::configuration::{Configuration, SelectionStep};
use yomibu_app::application::generate_story::{
    self, StoryInputs, StoryPipeline, StoryRunError,
    progress::{ProgressEvent, RunProgress},
};
use yomibu_app::reports::run::StoryRunReport;
use yomibu_ext::{
    learner_vocabulary_selection::{SeededOrdering, selection_steps},
    openai,
};

pub(crate) async fn run_story(
    config: &Configuration,
    inputs: StoryInputs,
    client: &openai::Client,
    seed: u64,
    progress: RunProgress<impl FnMut(ProgressEvent)>,
) -> Result<StoryRunReport, ApplicationError> {
    let seeded = SeededOrdering {
        seed: config.story.seed.unwrap_or(seed),
    };
    let lexical = config
        .pipeline
        .selection
        .steps
        .contains(&SelectionStep::LexicalTopic);
    let steps = selection_steps(&seeded, lexical);
    generate_story::run_story(
        inputs,
        StoryPipeline {
            selection: &steps,
            preparation: &config.pipeline.components.preparation.construct(),
            generation: client,
        },
        config.generation()?,
        config.story.select,
        progress,
    )
    .await
    .map_err(|error| match error {
        StoryRunError::Story(error) | StoryRunError::Preparation(error) => error.into(),
        StoryRunError::Generation(error) => error.into(),
    })
}
