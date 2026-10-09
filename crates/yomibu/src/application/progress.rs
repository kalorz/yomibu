use crate::application::story::DefaultCandidateError;
use crate::configuration::{
    Configuration,
    modules::{MODULES, ModuleId, ModuleReport, ModuleState},
};
use crate::reports::run::{SelectionReport, StepTiming, StoryRunReport, Warning};
use serde::Serialize;
use std::time::Instant;
use yomibu_core::domain::{
    candidate::GeneratedCandidates,
    story::{StoryPassageAssessment, StoryRequest},
};
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
        assessments: Vec<StoryPassageAssessment<DefaultCandidateError>>,
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
