use crate::application::story::DefaultCandidateError;
use crate::configuration::modules::{ModuleId, ModuleReport};
use crate::reports::analysis::AnalysisInput;
use serde::Serialize;
use yomibu_components::openai_story_generation as openai;
use yomibu_core::domain::{
    analysis::SentenceAnalysis,
    candidate::GeneratedCandidates,
    embedding::EmbeddingModelIdentity,
    evaluation::{CheckState, Evaluation},
    story::{StoryPassageAssessment, StoryRequest},
};

use crate::application::progress::Step;
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
impl Warning {
    pub(crate) fn embedding_fallback(error: &impl std::fmt::Display) -> Self {
        Self {
            module: ModuleId::Embeddings,
            message: format!("{error} Using built-in selection."),
        }
    }
}
#[derive(Debug, Serialize)]
pub struct SelectionReport {
    pub selector_revision: &'static str,
    pub vocabulary_ids: Vec<String>,
    pub seed: u64,
    pub embedding_model: Option<EmbeddingModelIdentity>,
}
impl SelectionReport {
    pub(crate) fn from_selection(
        selection: &yomibu_core::domain::story::StoryVocabularySelection<'_>,
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
            embedding_model: selection.embedding_model.clone(),
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
    pub assessments: Vec<StoryPassageAssessment<DefaultCandidateError>>,
    pub modules: Vec<ModuleReport>,
    pub timings: Vec<StepTiming>,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Serialize)]
pub struct StoryPreviewRunReport {
    pub kind: &'static str,
    pub request: StoryRequest,
    pub selection: SelectionReport,
    pub provider_request: openai::PreparedRequest,
    pub warnings: Vec<Warning>,
}
#[derive(Serialize)]
pub struct AnalysisRunReport {
    pub version: u32,
    pub input: AnalysisInput,
    pub analysis: SentenceAnalysis<'static>,
    pub outcome: CheckState,
    pub evaluation: Evaluation,
}
