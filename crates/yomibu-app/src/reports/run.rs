use serde::Serialize;
use yomibu_core::domain::{generation::StoryRequest, generation::candidate::GeneratedCandidates};

use crate::application::generate_story::progress::Step;
#[derive(Debug, Serialize)]
pub struct StepTiming {
    pub step: Step,
    pub elapsed_ms: f64,
}
#[derive(Debug, Serialize)]
pub struct SelectionReport {
    pub vocabulary_ids: Vec<String>,
    pub steps: Vec<SelectionStepReport>,
}
#[derive(Debug, Serialize)]
pub struct SelectionStepReport {
    pub revision: &'static str,
    pub seed: Option<u64>,
}
impl SelectionReport {
    pub(crate) fn from_selection(
        selection: &yomibu_core::domain::generation::StoryVocabularySelection<'_>,
        steps: &[&dyn yomibu_core::ports::capabilities::SelectionStep],
    ) -> Self {
        Self {
            vocabulary_ids: selection
                .selected
                .iter()
                .map(|entry| entry.word.id.clone())
                .collect(),
            steps: steps
                .iter()
                .map(|step| SelectionStepReport {
                    revision: step.revision(),
                    seed: step.seed(),
                })
                .collect(),
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
    pub timings: Vec<StepTiming>,
}
