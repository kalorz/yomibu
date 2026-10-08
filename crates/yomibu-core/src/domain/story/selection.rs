use crate::domain::{embedding::EmbeddingModelIdentity, inventory::InventoryWord};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SelectedVocabulary<'a> {
    pub word: &'a InventoryWord,
    pub reason: &'static str,
    pub score: Option<f64>,
}
#[derive(Debug, Serialize)]
pub struct StoryVocabularySelection<'a> {
    pub selector_revision: &'static str,
    pub selected: Vec<SelectedVocabulary<'a>>,
    pub vocabulary_targets: &'a [String],
    pub grammar_targets: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<EmbeddingModelIdentity>,
}
