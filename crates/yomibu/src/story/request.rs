//! Story intent, execution options and input validation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    inventory::{InventoryError, LearnerInventory},
    retrieval::EmbeddingError,
};

pub(super) const MAX_SELECTED_VOCABULARY_ENTRIES: usize = 16;
const MAX_VOCABULARY_TARGET_IDS: usize = 16;
const MAX_GRAMMAR_TARGET_IDS: usize = 16;
const MAX_STORY_BRIEF_BYTES: usize = 2048;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PracticeTargets {
    pub vocabulary: Vec<String>,
    pub grammar: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoryRequest {
    pub version: u32,
    pub brief: String,
    pub targets: PracticeTargets,
}
/// Execution choices, separate from the requested story's brief and targets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoryGenerationOptions {
    pub candidate_count: usize,
}
impl Default for StoryGenerationOptions {
    fn default() -> Self {
        Self { candidate_count: 2 }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum StoryError {
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    #[error("Invalid story request: targets and grammar exceed the {limit}-byte request limit.")]
    RequiredMaterialTooLarge { bytes: usize, limit: usize },
    #[error("Invalid story request: {0}.")]
    Invalid(&'static str),
}
impl StoryRequest {
    pub fn validate_selection_limit(&self, limit: usize) -> Result<(), StoryError> {
        if limit == 0
            || limit > MAX_SELECTED_VOCABULARY_ENTRIES
            || self.targets.vocabulary.len() > limit
        {
            return Err(StoryError::Invalid(
                "selection limit must include all targets and be at most 16",
            ));
        }
        Ok(())
    }
    pub fn validate(&self, inventory: &LearnerInventory) -> Result<(), StoryError> {
        inventory.validate()?;
        if self.version != 1
            || self.brief.trim().is_empty()
            || self.brief.len() > MAX_STORY_BRIEF_BYTES
            || self.targets.vocabulary.len() > MAX_VOCABULARY_TARGET_IDS
            || self.targets.grammar.len() > MAX_GRAMMAR_TARGET_IDS
        {
            return Err(StoryError::Invalid("version, brief or target limits"));
        }
        let mut seen = BTreeSet::new();
        for id in &self.targets.vocabulary {
            if !seen.insert(id) || !inventory.vocabulary.iter().any(|w| &w.id == id) {
                return Err(StoryError::Invalid(
                    "missing or duplicate vocabulary target",
                ));
            }
        }
        seen.clear();
        for id in &self.targets.grammar {
            if !seen.insert(id) || !inventory.grammar_declarations.iter().any(|g| &g.id == id) {
                return Err(StoryError::Invalid("missing or duplicate grammar target"));
            }
        }
        if inventory.vocabulary.is_empty() {
            return Err(StoryError::Invalid("no available vocabulary"));
        }
        Ok(())
    }
}
