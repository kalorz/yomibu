use super::ApplicationError;
use super::configuration::{Configuration, SelectionStep};
use yomibu_app::application::generate_story::{
    self, StoryInputs, StoryPipeline, StoryRunError,
    progress::{ProgressEvent, RunProgress},
};
use yomibu_app::reports::run::StoryRunReport;
use yomibu_core::ports::capabilities::SelectionStep as SelectionCapability;
use yomibu_ext::{
    learner_vocabulary_selection::{LexicalTopicScoring, SeededOrdering},
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
    let mut steps: Vec<&dyn SelectionCapability> = Vec::new();
    if lexical {
        steps.push(&LexicalTopicScoring);
    }
    steps.push(&seeded);
    generate_story::run_story(
        inputs,
        StoryPipeline {
            selection: &steps,
            selection_revision: if lexical {
                "builtin-v2"
            } else {
                "seeded-only-v1"
            },
            preparation: &config.pipeline.components.preparation.construct(),
            generation: client,
        },
        config.generation()?,
        config.story.select,
        seeded.seed,
        progress,
    )
    .await
    .map_err(|error| match error {
        StoryRunError::Story(error) | StoryRunError::Preparation(error) => error.into(),
        StoryRunError::Generation(error) => error.into(),
    })
}
