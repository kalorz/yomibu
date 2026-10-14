#[path = "../../../tests/support/story_inventory.rs"]
mod story_inventory;

use serde_json::json;
use yomibu_core::{
    domain::generation::finalization::select_target_first,
    domain::{
        generation::{
            SelectionCandidates, SelectionInput, StoryError, StoryRequest, StoryVocabularySelection,
        },
        knowledge::inventory::LearnerKnowledge,
    },
    ports::capabilities::SelectionStep,
};
use yomibu_ext::learner_vocabulary_selection::{
    LexicalTopicScoring, SeededOrdering, selection_steps,
};

fn inventory() -> LearnerKnowledge {
    story_inventory::inventory()
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
#[test]
fn callers_can_omit_scoring_or_change_step_order_without_an_implicit_reorder() {
    let inventory = inventory();
    let request = request(Some("walk"), &[]);
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 2,
    };
    let ordering = SeededOrdering { seed: 7 };
    let scored = select_target_first(input, &selection_steps(&ordering, true)).unwrap();
    assert_eq!(ids(&scored), ["walk", "dog"]);
    let sampled = select_target_first(input, &selection_steps(&ordering, false)).unwrap();
    assert_eq!(ids(&sampled), ["dog", "cat"]);
    assert!(sampled.selected.iter().all(|entry| entry.score.is_none()));
    assert!(
        sampled
            .selected
            .iter()
            .all(|entry| entry.reason == "local_sample")
    );
    let ordered_before_scoring =
        select_target_first(input, &[&SeededOrdering { seed: 7 }, &LexicalTopicScoring]).unwrap();
    assert_eq!(ids(&ordered_before_scoring), ["dog", "cat"]);
    assert!(
        ordered_before_scoring
            .selected
            .iter()
            .all(|entry| entry.score == Some(0.))
    );
}

struct KeepCatAndWalk;
impl SelectionStep for KeepCatAndWalk {
    fn revision(&self) -> &'static str {
        "test-KeepCatAndWalk"
    }

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
    let selection = select_target_first(
        input,
        &[
            &KeepCatAndWalk,
            &LexicalTopicScoring,
            &SeededOrdering { seed: 7 },
        ],
    )
    .unwrap();
    assert_eq!(ids(&selection), ["cat", "walk"]);
    assert_eq!(selection.selected[0].score, Some(0.));
    assert_eq!(selection.selected[1].score, Some(1.));
    assert_eq!(inventory.vocabulary.len(), 4);
}
