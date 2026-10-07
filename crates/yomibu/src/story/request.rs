//! Story intent, execution options and input validation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    inventory::{InventoryError, LearnerInventory},
    retrieval::EmbeddingError,
};

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
impl StoryGenerationOptions {
    pub fn validate(&self) -> Result<(), StoryError> {
        if self.candidate_count == 0 || self.candidate_count.checked_mul(512).is_none() {
            return Err(StoryError::Invalid(
                "candidate count must be positive and fit the output token budget",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoryError {
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    #[error("Invalid story request: {0}.")]
    Invalid(&'static str),
}
impl StoryRequest {
    pub fn validate_selection_limit(&self, limit: usize) -> Result<(), StoryError> {
        if limit == 0 || limit > 16 || self.targets.vocabulary.len() > limit {
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
            || self.brief.len() > 2048
            || self.targets.vocabulary.len() > 16
            || self.targets.grammar.len() > 16
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
