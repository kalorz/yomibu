use serde_json::json;
use yomibu_components::learner_vocabulary_selection::select_builtin_vocabulary;
use yomibu_core::domain::{inventory::LearnerInventory, story::StoryRequest};

fn inventory() -> LearnerInventory {
    LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap()
}

#[test]
fn local_selection_is_repeatable_varied_and_keeps_explicit_targets_first() {
    let inventory = inventory();
    let request: StoryRequest = serde_json::from_value(json!({
        "version": 1, "targets": {"vocabulary": ["cat"], "grammar": []}
    }))
    .unwrap();
    let ids = |seed| {
        select_builtin_vocabulary(&inventory, &request, 2, seed)
            .unwrap()
            .selected
            .into_iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(7), ids(7));
    let samples: std::collections::BTreeSet<_> = (0..16).map(ids).collect();
    assert!(samples.len() > 1);
    assert!(
        samples
            .iter()
            .all(|sample| sample.len() == 2 && sample[0] == "cat")
    );
    assert_eq!(inventory.vocabulary.len(), 4);
}

#[test]
fn a_topic_ranks_matching_words_without_vectors() {
    let inventory = inventory();
    let request: StoryRequest = serde_json::from_value(json!({
        "version": 1, "topic": "a DOG", "targets": {"vocabulary": ["cat"], "grammar": []}
    }))
    .unwrap();
    let selected = select_builtin_vocabulary(&inventory, &request, 2, 7).unwrap();
    assert_eq!(
        selected
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["cat", "dog"]
    );
}

#[test]
fn japanese_written_forms_match_inside_topics_without_dictionary_or_reading_substrings() {
    let inventory = inventory();
    for (topic, expected) in [("猫が寝る", vec!["cat", "sleep"]), ("ねこがねる", vec![])] {
        let request: StoryRequest = serde_json::from_value(json!({
            "version":1,"topic":topic,"targets":{"vocabulary":[],"grammar":[]}
        }))
        .unwrap();
        let selection = select_builtin_vocabulary(&inventory, &request, 4, 7).unwrap();
        let matches: std::collections::BTreeSet<_> = selection
            .selected
            .iter()
            .filter(|entry| entry.score.is_some_and(|score| score > 0.))
            .map(|entry| entry.word.id.as_str())
            .collect();
        assert_eq!(matches, expected.into_iter().collect(), "{topic}");
        assert!(
            selection
                .selected
                .iter()
                .take(matches.len())
                .all(|entry| entry.score.is_some_and(|score| score > 0.))
        );
        assert_eq!(selection.selector_revision, "builtin-v2");
    }
}

#[test]
fn zero_overlap_supports_are_local_samples_and_targets_keep_their_reason() {
    let inventory = inventory();
    let request: StoryRequest = serde_json::from_value(json!({
        "version":1,"topic":"cat","targets":{"vocabulary":["dog"],"grammar":[]}
    }))
    .unwrap();
    let selection = select_builtin_vocabulary(&inventory, &request, 4, 7).unwrap();
    assert_eq!(selection.selected[0].word.id, "dog");
    assert_eq!(selection.selected[0].reason, "practice_target");
    for entry in &selection.selected[1..] {
        assert_eq!(
            entry.reason,
            if entry.score == Some(0.) {
                "local_sample"
            } else {
                "topic_overlap"
            }
        );
    }
}

#[test]
fn a_story_request_needs_no_topic_and_does_not_invent_one() {
    let request: StoryRequest = serde_json::from_value(json!({
        "version": 1,
        "targets": {"vocabulary": [], "grammar": []}
    }))
    .unwrap();
    request.validate(&inventory()).unwrap();
    assert!(
        serde_json::to_value(request)
            .unwrap()
            .get("topic")
            .is_none()
    );
}
