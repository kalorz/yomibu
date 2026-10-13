use crate::reports::run::{SelectionReport, StepTiming, StoryRunReport};
use serde::Serialize;
use std::time::Instant;
use yomibu_core::domain::{generation::StoryRequest, generation::candidate::GeneratedCandidates};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Inputs,
    Selection,
    Generation,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ProgressEvent {
    Started { step: Step },
    Completed { step: Step, elapsed_ms: f64 },
}
pub struct RunProgress<F> {
    emit: F,
    timings: Vec<StepTiming>,
    inputs_started: Instant,
}
impl<F: FnMut(ProgressEvent)> RunProgress<F> {
    /// Start input preparation; pass this progress record to `run_story`.
    pub fn new(mut emit: F) -> Self {
        emit(ProgressEvent::Started { step: Step::Inputs });
        Self {
            emit,
            timings: Vec::new(),
            inputs_started: Instant::now(),
        }
    }

    pub(super) fn finish_inputs(&mut self) {
        self.finish(Step::Inputs, self.inputs_started);
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
}
