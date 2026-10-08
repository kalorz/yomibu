use super::{StoryError, StoryRequest, StoryVocabularySelection, TargetObservation};
use crate::domain::evaluation::{CheckKind, Evaluation, EvaluationError};
use crate::domain::{
    analysis::SentenceAnalysis, candidate::CandidateAssessment, inventory::LearnerInventory,
};
use serde::Serialize;
use std::ops::Range;

/// Completed findings for one original sentence, separate from its analysis.
#[derive(Debug)]
pub struct StoryFindings<'a> {
    pub(crate) text: &'a str,
    pub(crate) evaluation: Evaluation,
    pub(crate) targets: Vec<TargetObservation>,
    pub(crate) plan_departures: Vec<PlanDeparture>,
}

impl<'a> StoryFindings<'a> {
    /// Validate all finding spans and retain the original assessed text's binding.
    pub fn new(
        text: &'a str,
        evaluation: Evaluation,
        targets: Vec<TargetObservation>,
        plan_departures: Vec<PlanDeparture>,
    ) -> Result<Self, EvaluationError> {
        let check_spans = [
            CheckKind::Vocabulary,
            CheckKind::Inflection,
            CheckKind::Particles,
            CheckKind::Nominal,
            CheckKind::Scope,
        ]
        .into_iter()
        .flat_map(|kind| {
            evaluation
                .check(kind)
                .findings
                .iter()
                .map(|finding| &finding.span)
        });
        let target_spans = targets.iter().flat_map(|target| {
            target.spans.iter().chain(
                target
                    .uncertainties
                    .iter()
                    .map(|uncertainty| &uncertainty.span),
            )
        });
        if check_spans
            .chain(target_spans)
            .chain(plan_departures.iter().map(|departure| &departure.span))
            .any(|span| text.get(span.clone()).is_none())
        {
            return Err(EvaluationError::InvalidFindingSpan);
        }
        Ok(Self {
            text,
            evaluation,
            targets,
            plan_departures,
        })
    }

    pub fn text(&self) -> &str {
        self.text
    }

    pub fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }
    pub fn targets(&self) -> &[TargetObservation] {
        &self.targets
    }
    pub fn plan_departures(&self) -> &[PlanDeparture] {
        &self.plan_departures
    }
}

/// Assessment always uses the full inventory, including unselected material.
#[derive(Debug)]
pub struct StoryAssessmentInputs<'a> {
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    selected_vocabulary_ids: Vec<&'a str>,
}
impl<'a> StoryAssessmentInputs<'a> {
    pub fn new(
        inventory: &'a LearnerInventory,
        request: &'a StoryRequest,
        selection: &StoryVocabularySelection<'a>,
    ) -> Result<Self, StoryError> {
        request.validate(inventory)?;
        Ok(Self {
            inventory,
            request,
            selected_vocabulary_ids: selection
                .selected
                .iter()
                .map(|entry| entry.word.id.as_str())
                .collect(),
        })
    }
    pub fn inventory(&self) -> &'a LearnerInventory {
        self.inventory
    }
    pub fn request(&self) -> &'a StoryRequest {
        self.request
    }
    pub fn selected_vocabulary_ids(&self) -> &[&'a str] {
        &self.selected_vocabulary_ids
    }
}

#[derive(Debug, Serialize)]
pub struct PlanDeparture {
    pub span: Range<usize>,
    pub inventory_entries: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(bound(serialize = "E: std::fmt::Display"))]
pub struct StoryCandidateAssessment<'a, E> {
    pub assessment: CandidateAssessment<'a, E>,
    pub targets: Vec<TargetObservation>,
    pub plan_departures: Vec<PlanDeparture>,
}

impl<'a, E> StoryCandidateAssessment<'a, E> {
    pub fn map_error<F>(self, map: impl FnOnce(E) -> F) -> StoryCandidateAssessment<'a, F> {
        let assessment = match self.assessment {
            CandidateAssessment::NotRun => CandidateAssessment::NotRun,
            CandidateAssessment::Completed {
                analysis,
                evaluation,
            } => CandidateAssessment::Completed {
                analysis,
                evaluation,
            },
            CandidateAssessment::ExecutionError { analysis, error } => {
                CandidateAssessment::ExecutionError {
                    analysis,
                    error: map(error),
                }
            }
        };
        StoryCandidateAssessment {
            assessment,
            targets: self.targets,
            plan_departures: self.plan_departures,
        }
    }

    pub fn into_owned(self) -> StoryCandidateAssessment<'static, E> {
        let assessment = match self.assessment {
            CandidateAssessment::NotRun => CandidateAssessment::NotRun,
            CandidateAssessment::Completed {
                analysis,
                evaluation,
            } => CandidateAssessment::Completed {
                analysis: analysis.into_owned(),
                evaluation,
            },
            CandidateAssessment::ExecutionError { analysis, error } => {
                CandidateAssessment::ExecutionError {
                    analysis: analysis.map(SentenceAnalysis::into_owned),
                    error,
                }
            }
        };
        StoryCandidateAssessment {
            assessment,
            targets: self.targets,
            plan_departures: self.plan_departures,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(bound(serialize = "E: std::fmt::Display"))]
pub struct StorySentenceAssessment<E> {
    pub span: Range<usize>,
    pub assessment: StoryCandidateAssessment<'static, E>,
}
#[derive(Debug, Serialize)]
#[serde(bound(serialize = "E: std::fmt::Display"))]
pub struct StoryPassageAssessment<E> {
    pub sentences: Vec<StorySentenceAssessment<E>>,
    pub targets: Vec<TargetObservation>,
    pub plan_departures: Vec<PlanDeparture>,
}

impl<E> StoryPassageAssessment<E> {
    pub fn map_error<F>(self, mut map: impl FnMut(E) -> F) -> StoryPassageAssessment<F> {
        StoryPassageAssessment {
            sentences: self
                .sentences
                .into_iter()
                .map(|sentence| StorySentenceAssessment {
                    span: sentence.span,
                    assessment: sentence.assessment.map_error(&mut map),
                })
                .collect(),
            targets: self.targets,
            plan_departures: self.plan_departures,
        }
    }
}
