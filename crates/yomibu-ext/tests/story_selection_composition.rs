#[path = "../../../tests/support/story_inventory.rs"]
mod story_inventory;

use serde_json::json;
use yomibu_core::{
    domain::generation::finalization::select_target_first,
    domain::generation::{SelectionInput, StoryFormat, StoryGenerationOptions, StoryRequest},
};
use yomibu_ext::{
    learner_vocabulary_selection::SeededOrdering,
    openai::preparation::fit_selection_and_build_request,
};

#[test]
fn custom_composition_keeps_targets_and_full_inventory_after_support_trimming() {
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
    let selection = select_target_first(input, &[&SeededOrdering { seed: 7 }]).unwrap();
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["sleep", "dog", "cat", "walk"]
    );
    let (selection, prepared) = fit_selection_and_build_request(
        &inventory,
        &request,
        selection,
        StoryGenerationOptions {
            candidate_count: 1,
            format: StoryFormat::Passage,
            model: "test-model".into(),
        },
    )
    .unwrap();
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["sleep", "dog"]
    );
    assert_eq!(selection.vocabulary_targets, ["sleep"]);
    assert_eq!(selection.grammar_targets, ["polite"]);
    assert_eq!(selection.selected[0].reason, "practice_target");
    assert_eq!(selection.selected[1].reason, "local_sample");
    assert!(selection.selected.iter().all(|entry| entry.score.is_none()));
    assert!(prepared.body_utf8().len() <= 16384);
    assert_eq!(inventory.vocabulary.len(), 4);
}
