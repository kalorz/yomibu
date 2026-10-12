#[path = "../../../tests/support/story_inventory.rs"]
mod story_inventory;

use serde_json::json;
use yomibu_components::{
    learner_vocabulary_selection::SeededOrdering,
    story_prompt_preparation::fit_selection_and_build_request,
};
use yomibu_core::{
    domain::story::{SelectionInput, StoryRequest},
    pipeline::selection::select_target_first,
};

#[test]
fn custom_composition_keeps_provenance_and_full_inventory_after_support_trimming() {
    let mut inventory = story_inventory::inventory();
    for word in &mut inventory.vocabulary {
        word.meanings = vec!["x".repeat(1024); 5];
    }
    let request: StoryRequest = serde_json::from_value(json!({
        "version": 1, "topic": "walk", "targets": {"vocabulary": ["sleep"], "grammar": ["polite"]}
    }))
    .unwrap();
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 4,
    };
    let selection =
        select_target_first(input, &[&SeededOrdering { seed: 7 }], "test-seeded-v1").unwrap();
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["sleep", "dog", "cat", "walk"]
    );
    let (selection, prepared) =
        fit_selection_and_build_request(&inventory, &request, selection, Default::default())
            .unwrap();
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["sleep", "dog"]
    );
    assert_eq!(selection.selector_revision, "test-seeded-v1");
    assert_eq!(selection.vocabulary_targets, ["sleep"]);
    assert_eq!(selection.grammar_targets, ["polite"]);
    assert_eq!(selection.selected[0].reason, "practice_target");
    assert_eq!(selection.selected[1].reason, "local_sample");
    assert!(selection.selected.iter().all(|entry| entry.score.is_none()));
    assert!(prepared.body_utf8().len() <= 16384);
    assert_eq!(inventory.vocabulary.len(), 4);
}
