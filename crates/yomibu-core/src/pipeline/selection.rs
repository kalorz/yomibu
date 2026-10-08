use crate::domain::{
    embedding::EmbeddingModelIdentity,
    inventory::InventoryWord,
    story::{SelectedVocabulary, StoryError, StoryRequest, StoryVocabularySelection},
};
use std::collections::BTreeSet;

pub fn select_ranked<'a>(
    request: &'a StoryRequest,
    ranked: Vec<(&'a InventoryWord, Option<f64>)>,
    limit: usize,
    support_reason: fn(Option<f64>) -> &'static str,
    selector_revision: &'static str,
    embedding_model: Option<EmbeddingModelIdentity>,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    let mut selected = Vec::new();
    for id in &request.targets.vocabulary {
        let (word, score) = ranked
            .iter()
            .find(|(word, _)| &word.id == id)
            .ok_or(StoryError::Invalid("missing target"))?;
        selected.push(SelectedVocabulary {
            word,
            reason: "practice_target",
            score: *score,
        });
    }
    for (word, score) in ranked {
        if selected.len() == limit {
            break;
        }
        if !selected.iter().any(|entry| entry.word.id == word.id) {
            selected.push(SelectedVocabulary {
                word,
                reason: support_reason(score),
                score,
            });
        }
    }
    Ok(StoryVocabularySelection {
        selector_revision,
        selected,
        vocabulary_targets: &request.targets.vocabulary,
        grammar_targets: &request.targets.grammar,
        embedding_model,
    })
}

pub fn validate_selection(
    inventory: &crate::domain::inventory::LearnerInventory,
    request: &StoryRequest,
    selection: &StoryVocabularySelection<'_>,
) -> Result<(), StoryError> {
    let mut ids = BTreeSet::new();
    if selection.selected.is_empty()
        || selection.selected.len() > crate::domain::story::MAX_SELECTED_VOCABULARY_ENTRIES
        || selection.selected.iter().any(|s| {
            !inventory.vocabulary.iter().any(|w| std::ptr::eq(w, s.word)) || !ids.insert(&s.word.id)
        })
        || request
            .targets
            .vocabulary
            .iter()
            .any(|id| !ids.contains(id))
        || selection.vocabulary_targets != request.targets.vocabulary
        || selection.grammar_targets != request.targets.grammar
    {
        return Err(StoryError::Invalid(
            "plan does not match inventory and targets",
        ));
    }
    Ok(())
}
