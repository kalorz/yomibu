use crate::{
    capabilities::SelectionStep,
    domain::{
        inventory::{InventoryWord, LearnerInventory},
        story::{
            SelectionCandidates, SelectionError, SelectionInput, StoryError, StoryRequest,
            StoryVocabularySelection, VocabularyCandidate,
        },
    },
};
use std::collections::{BTreeMap, BTreeSet};

pub fn validate_selection(
    inventory: &LearnerInventory,
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

/// Validate inputs, run steps in caller order, then put targets first and fill support slots.
/// Empty step lists keep inventory order. Each step must retain targets and return
/// unique inventory references with finite scores (or absent scores).
/// Candidates start with reason `inventory_entry`. Finalization reserves `practice_target`
/// for requested targets and resets that reason on supports to `inventory_entry`.
/// `selector_revision` identifies the whole composition; changed behavior needs a new revision.
pub fn select_target_first<'a>(
    input: SelectionInput<'a>,
    steps: &[&dyn SelectionStep],
    selector_revision: &'static str,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    input.request.validate(input.inventory)?;
    input.request.validate_selection_limit(input.limit)?;
    let mut candidates = SelectionCandidates {
        entries: input
            .inventory
            .vocabulary
            .iter()
            .map(|word| VocabularyCandidate {
                word,
                score: None,
                reason: "inventory_entry",
            })
            .collect(),
        embedding_model: None,
    };
    let inventory: BTreeMap<_, _> = input
        .inventory
        .vocabulary
        .iter()
        .map(|word| (word.id.as_str(), word))
        .collect();
    for step in steps {
        candidates = step.apply(input, candidates)?;
        validate_candidates(&inventory, input.request, &candidates)?;
    }
    let mut selected = Vec::new();
    for id in &input.request.targets.vocabulary {
        let index = candidates
            .entries
            .iter()
            .position(|entry| &entry.word.id == id)
            .ok_or_else(|| SelectionError::MissingTarget { id: id.clone() })?;
        let mut target = candidates.entries.remove(index);
        target.reason = "practice_target";
        selected.push(target);
    }
    selected.extend(
        candidates
            .entries
            .into_iter()
            .take(input.limit - selected.len())
            .map(|mut support| {
                if support.reason == "practice_target" {
                    support.reason = "inventory_entry";
                }
                support
            }),
    );
    Ok(StoryVocabularySelection {
        selected,
        selector_revision,
        vocabulary_targets: &input.request.targets.vocabulary,
        grammar_targets: &input.request.targets.grammar,
        embedding_model: candidates.embedding_model,
    })
}

fn validate_candidates(
    inventory: &BTreeMap<&str, &InventoryWord>,
    request: &StoryRequest,
    candidates: &SelectionCandidates<'_>,
) -> Result<(), SelectionError> {
    if candidates.entries.is_empty() {
        return Err(SelectionError::EmptyCandidates);
    }
    let mut ids = BTreeSet::new();
    for entry in &candidates.entries {
        let id = &entry.word.id;
        if !inventory
            .get(id.as_str())
            .is_some_and(|word| std::ptr::eq(*word, entry.word))
        {
            return Err(SelectionError::ForeignCandidate { id: id.clone() });
        }
        if !ids.insert(id) {
            return Err(SelectionError::DuplicateCandidate { id: id.clone() });
        }
        if entry.score.is_some_and(|score| !score.is_finite()) {
            return Err(SelectionError::NonfiniteScore { id: id.clone() });
        }
    }
    for id in &request.targets.vocabulary {
        if !ids.contains(id) {
            return Err(SelectionError::MissingTarget { id: id.clone() });
        }
    }
    Ok(())
}
