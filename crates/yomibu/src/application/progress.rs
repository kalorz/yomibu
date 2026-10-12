use crate::reports::run::{SelectionReport, StepTiming, StoryRunReport, Warning};
use serde::Serialize;
use std::time::Instant;
use yomibu_core::domain::{candidate::GeneratedCandidates, story::StoryRequest};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Inputs,
    Knowledge,
    Sync,
    Selection,
    Generation,
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
    timings: Vec<StepTiming>,
    pub(super) warnings: Vec<Warning>,
}
impl<F: FnMut(ProgressEvent)> RunProgress<F> {
    pub(super) fn new(emit: F) -> Self {
        Self {
            emit,
            timings: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub(super) fn into_story_report(
        self,
        request: StoryRequest,
        selection: SelectionReport,
        generated: GeneratedCandidates,
    ) -> StoryRunReport {
        StoryRunReport {
            kind: "experimental_story",
            notice: "Experimental reading; verification and acceptance have not run.",
            request,
            selection,
            generated,
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
    pub(super) fn warn(&mut self, step: Step, message: String) {
        self.warnings.push(Warning { step, message });
    }
}
