//! Story intent, execution options and input validation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::domain::{
    embedding::EmbeddingError,
    inventory::{InventoryError, LearnerInventory},
};

pub(crate) const MAX_SELECTED_VOCABULARY_ENTRIES: usize = 16;
const MAX_VOCABULARY_TARGET_IDS: usize = 16;
const MAX_GRAMMAR_TARGET_IDS: usize = 16;
const MAX_STORY_TOPIC_BYTES: usize = 2048;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StoryTopic(String);

impl StoryTopic {
    pub fn new(text: String) -> Result<Self, StoryError> {
        if text.trim().is_empty() || text.len() > MAX_STORY_TOPIC_BYTES {
            return Err(StoryError::Invalid(
                "topic must be nonblank and at most 2048 bytes",
            ));
        }
        Ok(Self(text))
    }

    pub fn text(&self) -> &str {
        &self.0
    }
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<StoryTopic>,
    pub targets: PracticeTargets,
}
/// Execution choices, separate from the requested story's topic and targets.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoryFormat {
    Sentence,
    #[default]
    Passage,
}
impl StoryFormat {
    pub fn sentence_bounds(self) -> (usize, usize) {
        match self {
            Self::Sentence => (1, 1),
            Self::Passage => (3, 5),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoryGenerationOptions {
    pub candidate_count: usize,
    pub format: StoryFormat,
    pub model: String,
}
impl Default for StoryGenerationOptions {
    fn default() -> Self {
        Self {
            candidate_count: 1,
            format: StoryFormat::Passage,
            model: "gpt-6-luna".into(),
        }
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
    pub fn validate_shape(&self) -> Result<(), StoryError> {
        if self.version != 1
            || self.topic.as_ref().is_some_and(|topic| {
                topic.text().trim().is_empty() || topic.text().len() > MAX_STORY_TOPIC_BYTES
            })
            || self.targets.vocabulary.len() > MAX_VOCABULARY_TARGET_IDS
            || self.targets.grammar.len() > MAX_GRAMMAR_TARGET_IDS
        {
            return Err(StoryError::Invalid("version, topic or target limits"));
        }
        Ok(())
    }
    pub fn validate(&self, inventory: &LearnerInventory) -> Result<(), StoryError> {
        inventory.validate()?;
        self.validate_shape()?;
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

impl std::str::FromStr for StoryFormat {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "passage" => Ok(Self::Passage),
            "sentence" => Ok(Self::Sentence),
            _ => Err("Choose passage or sentence."),
        }
    }
}
