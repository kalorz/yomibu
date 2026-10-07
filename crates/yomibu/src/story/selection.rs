//! Deterministic vocabulary ranking and selection from cached embeddings.

use serde::Serialize;

use super::{StoryError, StoryRequest};
use crate::{
    inventory::{InventoryWord, LearnerInventory},
    retrieval::{
        EmbeddingCache, EmbeddingError, EmbeddingModelIdentity, cosine, prepare_embedding_inputs,
    },
};

#[derive(Debug, Serialize)]
pub struct SelectedVocabulary<'a> {
    pub word: &'a InventoryWord,
    pub reason: &'static str,
    pub similarity: f64,
}
#[derive(Debug, Serialize)]
pub struct StoryVocabularySelection<'a> {
    pub selector_revision: &'static str,
    pub selected: Vec<SelectedVocabulary<'a>>,
    pub vocabulary_targets: &'a [String],
    pub grammar_targets: &'a [String],
    pub embedding_model: EmbeddingModelIdentity,
}
pub fn select_vocabulary<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: &EmbeddingCache,
    model: &EmbeddingModelIdentity,
    limit: usize,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    let inputs = prepare_embedding_inputs(inventory, request)?;
    request.validate_selection_limit(limit)?;
    let vectors = cache.vectors(model, &inputs)?;
    let query = vectors.last().ok_or(EmbeddingError::Missing)?;
    let mut ranked: Vec<_> = inventory
        .vocabulary
        .iter()
        .zip(&vectors)
        .map(|(w, v)| (w, cosine(v, query)))
        .collect();
    ranked.sort_by(|(a, sa), (b, sb)| sb.total_cmp(sa).then_with(|| a.id.cmp(&b.id)));
    let mut selected = Vec::new();
    for id in &request.targets.vocabulary {
        let (word, score) = ranked
            .iter()
            .find(|(word, _)| &word.id == id)
            .ok_or(StoryError::Invalid("missing target"))?;
        selected.push(SelectedVocabulary {
            word,
            reason: "practice_target",
            similarity: *score,
        });
    }
    for (word, score) in ranked {
        if selected.len() == limit {
            break;
        }
        if !selected.iter().any(|s| s.word.id == word.id) {
            selected.push(SelectedVocabulary {
                word,
                reason: "brief_similarity",
                similarity: score,
            });
        }
    }
    Ok(StoryVocabularySelection {
        selector_revision: "inventory-similarity-v1",
        selected,
        vocabulary_targets: &request.targets.vocabulary,
        grammar_targets: &request.targets.grammar,
        embedding_model: model.clone(),
    })
}
