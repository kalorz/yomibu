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
    pub selector_revision: &'static str,
    pub vocabulary_ids: Vec<String>,
    pub seed: u64,
}
impl SelectionReport {
    pub(crate) fn from_selection(
        selection: &yomibu_core::domain::generation::StoryVocabularySelection<'_>,
        seed: u64,
    ) -> Self {
        Self {
            selector_revision: selection.selector_revision,
            vocabulary_ids: selection
                .selected
                .iter()
                .map(|entry| entry.word.id.clone())
                .collect(),
            seed,
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
