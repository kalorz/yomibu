use serde_json::json;
use yomibu_components::{
    embedding_vocabulary_selection::{EmbeddingRanking, prepare_embedding_inputs},
    learner_vocabulary_selection::{LexicalTopicScoring, SeededOrdering},
};
use yomibu_core::{
    capabilities::SelectionStep,
    domain::{
        embedding::{EmbeddingCache, EmbeddingError, EmbeddingModelIdentity},
        inventory::LearnerInventory,
        story::{
            SelectionCandidates, SelectionError, SelectionInput, StoryError, StoryRequest,
            StoryVocabularySelection,
        },
    },
    pipeline::selection::select_target_first,
};

fn inventory() -> LearnerInventory {
    LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap()
}
fn request(topic: Option<&str>, targets: &[&str]) -> StoryRequest {
    serde_json::from_value(
        json!({"version": 1, "topic": topic, "targets": {"vocabulary": targets, "grammar": []}}),
    )
    .unwrap()
}
fn ids<'a>(selection: &StoryVocabularySelection<'a>) -> Vec<&'a str> {
    selection
        .selected
        .iter()
        .map(|entry| entry.word.id.as_str())
        .collect()
}
fn model() -> EmbeddingModelIdentity {
    EmbeddingModelIdentity {
        provider: "test".into(),
        model: "ties".into(),
        revision: "1".into(),
        dimensions: 2,
        encoding_revision: "1".into(),
    }
}
fn cache(inventory: &LearnerInventory, request: &StoryRequest) -> EmbeddingCache {
    let inputs = prepare_embedding_inputs(inventory, request).unwrap();
    EmbeddingCache::from_vectors(model(), &inputs, vec![vec![1., 0.]; inputs.len()]).unwrap()
}

#[test]
fn callers_can_omit_scoring_or_change_step_order_without_an_implicit_reorder() {
    let inventory = inventory();
    let request = request(Some("walk"), &[]);
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 2,
    };
    let scored = select_target_first(
        input,
        &[&LexicalTopicScoring, &SeededOrdering { seed: 7 }],
        "test-scored-v1",
    )
    .unwrap();
    assert_eq!(ids(&scored), ["walk", "dog"]);
    let sampled =
        select_target_first(input, &[&SeededOrdering { seed: 7 }], "test-seeded-only-v1").unwrap();
    assert_eq!(ids(&sampled), ["dog", "cat"]);
    assert!(sampled.selected.iter().all(|entry| entry.score.is_none()));
    assert!(
        sampled
            .selected
            .iter()
            .all(|entry| entry.reason == "local_sample")
    );
    assert_eq!(sampled.selector_revision, "test-seeded-only-v1");
    let ordered_before_scoring = select_target_first(
        input,
        &[&SeededOrdering { seed: 7 }, &LexicalTopicScoring],
        "test-order-then-score-v1",
    )
    .unwrap();
    assert_eq!(ids(&ordered_before_scoring), ["dog", "cat"]);
    assert!(
        ordered_before_scoring
            .selected
            .iter()
            .all(|entry| entry.score == Some(0.))
    );
}

#[test]
fn embedding_ranking_can_be_followed_by_existing_steps_with_honest_provenance() {
    let inventory = inventory();
    let request = request(Some("walk"), &[]);
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 4,
    };
    let cache = cache(&inventory, &request);
    let ranker = EmbeddingRanking::prepare(input, &cache, &cache.model).unwrap();
    let similarity = select_target_first(input, &[&ranker], "test-similarity").unwrap();
    assert_eq!(ids(&similarity), ["cat", "dog", "sleep", "walk"]);
    let seeded_ties = select_target_first(
        input,
        &[&ranker, &SeededOrdering { seed: 7 }],
        "test-similarity-seeded-ties-v1",
    )
    .unwrap();
    assert_eq!(ids(&seeded_ties), ["dog", "cat", "walk", "sleep"]);
    assert_eq!(seeded_ties.embedding_model.as_ref(), Some(&cache.model));
    assert!(
        seeded_ties
            .selected
            .iter()
            .all(|entry| entry.reason == "topic_similarity" && entry.score == Some(1.))
    );
    let overwritten = select_target_first(
        input,
        &[&ranker, &LexicalTopicScoring, &SeededOrdering { seed: 7 }],
        "test-rescored-v1",
    )
    .unwrap();
    assert_eq!(ids(&overwritten), ["walk", "dog", "cat", "sleep"]);
    assert!(overwritten.embedding_model.is_none());
    assert_eq!(overwritten.selected[0].score, Some(1.));
    assert_eq!(overwritten.selected[0].reason, "topic_overlap");
}

#[test]
fn embedding_evidence_must_be_complete_compatible_and_bound_to_the_selection_inputs() {
    let inventory = inventory();
    let request = request(Some("walk"), &[]);
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 2,
    };
    let mut cache = cache(&inventory, &request);
    let mut model = cache.model.clone();
    model.revision = "different".into();
    assert!(matches!(
        EmbeddingRanking::prepare(input, &cache, &model),
        Err(StoryError::Embedding(EmbeddingError::Missing))
    ));
    let ranker = EmbeddingRanking::prepare(input, &cache, &cache.model).unwrap();
    let other = self::request(Some("cat"), &[]);
    assert!(matches!(
        select_target_first(
            SelectionInput {
                request: &other,
                ..input
            },
            &[&ranker],
            "test"
        ),
        Err(StoryError::Selection(SelectionError::EvidenceMismatch))
    ));
    let other_inventory = self::inventory();
    assert!(matches!(
        select_target_first(
            SelectionInput {
                inventory: &other_inventory,
                ..input
            },
            &[&ranker],
            "test"
        ),
        Err(StoryError::Selection(SelectionError::EvidenceMismatch))
    ));
    cache.entries.pop();
    assert!(matches!(
        EmbeddingRanking::prepare(input, &cache, &cache.model),
        Err(StoryError::Embedding(EmbeddingError::Missing))
    ));
    let no_topic = self::request(None, &[]);
    assert!(matches!(
        EmbeddingRanking::prepare(
            SelectionInput {
                request: &no_topic,
                ..input
            },
            &cache,
            &cache.model
        ),
        Err(StoryError::Invalid("semantic selection requires a topic"))
    ));
}

struct KeepCatAndWalk;
impl SelectionStep for KeepCatAndWalk {
    fn apply<'a>(
        &self,
        _: SelectionInput<'a>,
        mut candidates: SelectionCandidates<'a>,
    ) -> Result<SelectionCandidates<'a>, StoryError> {
        candidates
            .entries
            .retain(|entry| ["cat", "walk"].contains(&entry.word.id.as_str()));
        candidates.entries.reverse();
        Ok(candidates)
    }
}

#[test]
fn existing_rankers_consume_an_external_steps_subset_without_restoring_removed_supports() {
    let inventory = inventory();
    let request = request(Some("walk"), &["cat"]);
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 4,
    };
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let cache = EmbeddingCache::from_vectors(
        model(),
        &inputs,
        vec![
            vec![0., 1.],  // sleep
            vec![1., 0.],  // cat
            vec![-1., 0.], // dog
            vec![3., 4.],  // walk
            vec![1., 0.],  // query
        ],
    )
    .unwrap();
    let ranker = EmbeddingRanking::prepare(input, &cache, &cache.model).unwrap();
    let selection = select_target_first(
        input,
        &[&KeepCatAndWalk, &ranker],
        "test-filtered-similarity-v1",
    )
    .unwrap();
    assert_eq!(ids(&selection), ["cat", "walk"]);
    assert_eq!(selection.selected[0].score, Some(1.));
    assert_eq!(selection.selected[1].score, Some(0.6)); // 3 / sqrt(3² + 4²)
    let selection = select_target_first(
        input,
        &[
            &KeepCatAndWalk,
            &LexicalTopicScoring,
            &SeededOrdering { seed: 7 },
        ],
        "test-filtered-lexical-v1",
    )
    .unwrap();
    assert_eq!(ids(&selection), ["cat", "walk"]);
    assert_eq!(selection.selected[0].score, Some(0.));
    assert_eq!(selection.selected[1].score, Some(1.));
    assert_eq!(inventory.vocabulary.len(), 4);
}
