use super::{StoryError, StoryRequest, StoryVocabularySelection, TargetObservation};
use crate::domain::{
    analysis::SentenceAnalysis, candidate::CandidateAssessment, inventory::LearnerInventory,
};
use serde::Serialize;
use std::ops::Range;

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

impl<E> StoryCandidateAssessment<'_, E> {
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
