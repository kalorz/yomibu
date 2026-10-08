use super::{
    config::Configuration,
    modules::{MODULES, ModuleId, ModuleReport, ModuleState},
};
use crate::{
    adapters::openai,
    analysis::SentenceAnalysis,
    candidate::GeneratedCandidates,
    evaluation::{CheckState, Evaluation},
    reports::analysis::AnalysisInput,
    retrieval::EmbeddingModelIdentity,
    story::{StoryPassageAssessment, StoryRequest},
};
use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Inputs,
    Knowledge,
    Sync,
    Selection,
    Embeddings,
    Generation,
    Assessment,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ProgressEvent {
    Started { step: Step },
    Completed { step: Step, elapsed_ms: f64 },
    Skipped { step: Step, reason: String },
}
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
    pub(super) fn embedding_fallback(error: &impl std::fmt::Display) -> Self {
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
    pub(super) fn from_selection(
        selection: crate::story::StoryVocabularySelection<'_>,
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
            embedding_model: selection.embedding_model,
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
    pub assessments: Vec<StoryPassageAssessment>,
    pub modules: Vec<ModuleReport>,
    pub timings: Vec<StepTiming>,
    pub warnings: Vec<Warning>,
}

pub(super) struct RunProgress<F> {
    emit: F,
    pub(super) modules: Vec<ModuleReport>,
    timings: Vec<StepTiming>,
    pub(super) warnings: Vec<Warning>,
}
impl<F: FnMut(ProgressEvent)> RunProgress<F> {
    pub(super) fn new(config: &Configuration, emit: F) -> Self {
        let modules = MODULES
            .iter()
            .map(|metadata| ModuleReport {
                metadata,
                required: matches!(metadata.id, ModuleId::Knowledge | ModuleId::Generation),
                state: if config.enabled(metadata.id) {
                    ModuleState::NotConfigured
                } else {
                    ModuleState::Disabled
                },
            })
            .collect();
        Self {
            emit,
            modules,
            timings: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub(super) fn into_story_report(
        self,
        request: StoryRequest,
        selection: SelectionReport,
        generated: GeneratedCandidates,
        assessments: Vec<StoryPassageAssessment>,
    ) -> StoryRunReport {
        StoryRunReport {
            kind: "experimental_story",
            notice: "Experimental reading; verification and acceptance have not run.",
            request,
            selection,
            generated,
            assessments,
            modules: self.modules,
            timings: self.timings,
            warnings: self.warnings,
        }
    }

    pub(super) fn start(&mut self, step: Step) -> Instant {
        (self.emit)(ProgressEvent::Started { step });
        Instant::now()
    }
    pub(super) fn finish(&mut self, step: Step, start: Instant) {
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
        self.timings.push(StepTiming { step, elapsed_ms });
        (self.emit)(ProgressEvent::Completed { step, elapsed_ms });
    }
    pub(super) fn skip(&mut self, step: Step, reason: &str) {
        (self.emit)(ProgressEvent::Skipped {
            step,
            reason: reason.into(),
        });
    }
    pub(super) fn state(&mut self, id: ModuleId, state: ModuleState) {
        if let Some(module) = self
            .modules
            .iter_mut()
            .find(|module| module.metadata.id == id)
        {
            module.state = state;
        }
    }
    pub(super) fn warn(&mut self, module: ModuleId, message: String) {
        self.warnings.push(Warning { module, message });
    }
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
