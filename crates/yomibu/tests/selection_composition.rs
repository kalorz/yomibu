use serde_json::json;
use yomibu_components::{
    embedding_vocabulary_selection::{EmbeddingRanking, prepare_embedding_inputs},
    learner_vocabulary_selection::SeededOrdering,
    story_prompt_preparation::fit_selection_and_build_request,
};
use yomibu_core::{
    domain::{
        embedding::{EmbeddingCache, EmbeddingModelIdentity},
        inventory::LearnerInventory,
        story::{SelectionInput, StoryAssessmentInputs, StoryRequest},
    },
    pipeline::selection::select_target_first,
};

#[test]
fn custom_composition_keeps_provenance_and_full_inventory_after_support_trimming() {
    let mut inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    for word in &mut inventory.vocabulary {
        word.meanings = vec!["x".repeat(1024); 5];
    }
    let request: StoryRequest = serde_json::from_value(json!({
        "version": 1, "topic": "walk", "targets": {"vocabulary": ["sleep"], "grammar": ["polite"]}
    }))
    .unwrap();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let cache = EmbeddingCache::from_vectors(
        EmbeddingModelIdentity {
            provider: "test".into(),
            model: "ties".into(),
            revision: "1".into(),
            dimensions: 2,
            encoding_revision: "1".into(),
        },
        &inputs,
        vec![vec![1., 0.]; inputs.len()],
    )
    .unwrap();
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 4,
    };
    let ranker = EmbeddingRanking::prepare(input, &cache, &cache.model).unwrap();
    let selection = select_target_first(
        input,
        &[&ranker, &SeededOrdering { seed: 7 }],
        "test-similarity-seeded-v1",
    )
    .unwrap();
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
    assert_eq!(selection.selector_revision, "test-similarity-seeded-v1");
    assert_eq!(selection.embedding_model.as_ref(), Some(&cache.model));
    assert_eq!(selection.vocabulary_targets, ["sleep"]);
    assert_eq!(selection.grammar_targets, ["polite"]);
    assert_eq!(selection.selected[0].reason, "practice_target");
    assert_eq!(selection.selected[1].reason, "topic_similarity");
    assert!(
        selection
            .selected
            .iter()
            .all(|entry| entry.score == Some(1.))
    );
    assert!(prepared.body_utf8().len() <= 16384);
    let assessment = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    assert!(std::ptr::eq(assessment.inventory(), &inventory));
    assert_eq!(assessment.inventory().vocabulary.len(), 4);
    assert_eq!(assessment.selected_vocabulary_ids(), ["sleep", "dog"]);
}
