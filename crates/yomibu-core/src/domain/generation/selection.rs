use crate::domain::knowledge::inventory::{InventoryWord, LearnerInventory};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct VocabularyCandidate<'a> {
    pub word: &'a InventoryWord,
    pub reason: &'static str,
    pub score: Option<f64>,
}
#[derive(Debug, Serialize)]
pub struct StoryVocabularySelection<'a> {
    pub selected: Vec<VocabularyCandidate<'a>>,
    pub vocabulary_targets: &'a [String],
    pub grammar_targets: &'a [String],
}

#[derive(Debug, Clone, Copy)]
pub struct SelectionInput<'a> {
    pub inventory: &'a LearnerInventory,
    pub request: &'a super::StoryRequest,
    pub limit: usize,
}

/// Intermediate evidence and order; these entries are not a finalized selection.
#[derive(Debug)]
pub struct SelectionCandidates<'a> {
    pub entries: Vec<VocabularyCandidate<'a>>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    #[error("Selection step returned no candidates.")]
    EmptyCandidates,
    #[error("Selection candidate {id} does not belong to the inventory.")]
    ForeignCandidate { id: String },
    #[error("Selection step repeated candidate {id}.")]
    DuplicateCandidate { id: String },
    #[error("Selection step removed target {id}.")]
    MissingTarget { id: String },
    #[error("Selection score for {id} must be finite.")]
    NonfiniteScore { id: String },
}
