use serde_json::{Value, json};
use yomibu_components::wanikani_source::map_content_and_progress;
use yomibu_core::domain::{
    inventory::{LearnerInventory, ManualInventory, MappedContent, SourceOrigin},
    knowledge::{ExclusionReason, LearnerKnowledgePolicy},
    source::WaniKaniSyncData,
};

fn source() -> WaniKaniSyncData {
    let value: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/mixed.json")).unwrap();
    serde_json::from_value(value["snapshot"].clone()).unwrap()
}

fn prepare_inventory(data: &WaniKaniSyncData, policy: &LearnerKnowledgePolicy) -> LearnerInventory {
    LearnerInventory::from_mapped(map_content_and_progress(data).unwrap(), policy).unwrap()
}

fn exclusion(inventory: &LearnerInventory, id: u64) -> Option<ExclusionReason> {
    inventory
        .excluded
        .iter()
        .find(|entry| entry.origin == SourceOrigin::Wanikani { subject_id: id })
        .map(|entry| entry.reason)
}

#[test]
fn preparation_retains_exclusion_reasons_in_source_subject_order() {
    use ExclusionReason::{ContentUnavailable, Hidden, NoProgress, NoRecordedPass};
    let data = source();
    let inventory = prepare_inventory(&data, &LearnerKnowledgePolicy::default());
    assert_eq!(inventory.vocabulary.len(), 1);
    assert_eq!(inventory.vocabulary[0].written_form, "これ");
    assert_eq!(
        inventory
            .excluded
            .iter()
            .map(|entry| (&entry.origin, entry.reason))
            .collect::<Vec<_>>(),
        [
            (&SourceOrigin::Wanikani { subject_id: 1 }, Hidden),
            (&SourceOrigin::Wanikani { subject_id: 2 }, Hidden),
            (&SourceOrigin::Wanikani { subject_id: 4 }, NoProgress),
            (
                &SourceOrigin::Wanikani { subject_id: 5 },
                ContentUnavailable
            )
        ]
    );
    let passed = prepare_inventory(&data, &LearnerKnowledgePolicy::RecordedPass);
    assert_eq!(exclusion(&passed, 3), Some(NoRecordedPass));
}

#[test]
fn eligible_kanji_are_neither_vocabulary_nor_excluded() {
    let mut data = source();
    data.subjects[0].hidden_at = None;
    data.assignments[0].started_at = Some(data.sync_completed_at);
    let inventory = prepare_inventory(&data, &LearnerKnowledgePolicy::default());
    assert_eq!(exclusion(&inventory, 1), None);
    assert_eq!(
        inventory
            .vocabulary
            .iter()
            .map(|word| word.id.as_str())
            .collect::<Vec<_>>(),
        ["wanikani:3"]
    );
}

#[test]
fn mapping_preserves_progress_when_content_is_unavailable() {
    let mut data = source();
    let assignment = data
        .assignments
        .iter_mut()
        .find(|entry| entry.subject_id == 5)
        .unwrap();
    assignment.started_at = Some(data.sync_completed_at);
    assignment.passed_at = assignment.started_at;
    let (_, progress) = map_content_and_progress(&data)
        .unwrap()
        .find(|(content, _)| {
            matches!(
                content,
                MappedContent::Unavailable(SourceOrigin::Wanikani { subject_id: 5 })
            )
        })
        .unwrap();
    let progress = progress.unwrap();
    assert!(progress.is_hidden);
    assert_eq!(progress.learning_started_at, Some(data.sync_completed_at));
    assert_eq!(progress.first_passed_at, Some(data.sync_completed_at));
}

#[test]
fn lifecycle_evidence_not_stage_accuracy_or_burned_state_controls_eligibility() {
    let mut data = source();
    data.subjects[0].hidden_at = None;
    data.assignments[1].hidden = false;
    data.assignments[2].srs_stage = 999;
    let policy = LearnerKnowledgePolicy::default();
    let inventory = prepare_inventory(&data, &policy);
    assert_eq!(
        exclusion(&inventory, 1),
        Some(ExclusionReason::NoRecordedLessonStart)
    );
    assert_eq!(
        inventory
            .vocabulary
            .iter()
            .map(|word| word.id.as_str())
            .collect::<Vec<_>>(),
        ["wanikani:2", "wanikani:3"]
    );
    assert!(data.assignments[1].burned_at.is_some());
    let passed = prepare_inventory(&data, &LearnerKnowledgePolicy::RecordedPass);
    assert_eq!(passed.vocabulary[0].id, "wanikani:2");
    data.review_statistics[1].hidden = true;
    assert_eq!(
        exclusion(&prepare_inventory(&data, &policy), 2),
        Some(ExclusionReason::Hidden)
    );
    data.assignments[2].started_at = None;
    assert_eq!(
        exclusion(&prepare_inventory(&data, &policy), 3),
        Some(ExclusionReason::NoRecordedLessonStart)
    );
    data.review_statistics[2].hidden = true;
    assert_eq!(
        exclusion(&prepare_inventory(&data, &policy), 4),
        Some(ExclusionReason::Hidden)
    );
}

#[test]
fn preparation_order_is_stable_and_invalid_direct_input_is_rejected() {
    let mut data = source();
    let policy = LearnerKnowledgePolicy::default();
    let expected = serde_json::to_value(prepare_inventory(&data, &policy)).unwrap();
    data.subjects.reverse();
    data.assignments.reverse();
    data.review_statistics.reverse();
    assert_eq!(
        serde_json::to_value(prepare_inventory(&data, &policy)).unwrap(),
        expected
    );
    data.subjects.clear();
    assert!(map_content_and_progress(&data).is_err());
}

#[test]
fn preparation_preserves_unpaired_reading_and_meaning_alternatives() {
    use yomibu_core::domain::source::{LexicalContent, Meaning, Reading};
    let mut data = source();
    data.assignments[1].hidden = false;
    let subject = &mut data.subjects[1];
    subject.characters = "一日".into();
    if let LexicalContent::Vocabulary { readings, .. } = &mut subject.lexical {
        *readings = ["いちにち", "ついたち"]
            .into_iter()
            .enumerate()
            .map(|(i, reading)| Reading {
                reading: reading.into(),
                primary: i == 0,
                accepted_answer: true,
            })
            .collect();
    }
    subject.meanings = ["one day", "first day of the month"]
        .into_iter()
        .enumerate()
        .map(|(i, meaning)| Meaning {
            meaning: meaning.into(),
            primary: i == 0,
            accepted_answer: true,
        })
        .collect();
    let inventory = prepare_inventory(&data, &LearnerKnowledgePolicy::default());
    let words: Vec<_> = inventory
        .vocabulary
        .iter()
        .filter(|word| word.written_form == "一日")
        .collect();
    assert_eq!(words.len(), 1);
    assert_eq!(words[0].readings, ["いちにち", "ついたち"]);
    assert_eq!(words[0].meanings, ["one day", "first day of the month"]);
}

#[test]
fn preparation_keeps_missing_readings_and_manual_grammar() {
    let value: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/preparation.json")).unwrap();
    let source = serde_json::from_value(value["snapshot"].clone()).unwrap();
    let inventory = prepare_inventory(&source, &LearnerKnowledgePolicy::default());
    assert!(
        inventory
            .vocabulary
            .iter()
            .all(|word| word.id.starts_with("wanikani:"))
    );
    assert!(
        inventory
            .vocabulary
            .iter()
            .all(|word| word.direct_object.is_none())
    );
    assert!(exclusion(&inventory, 1).is_some());
    assert!(
        inventory
            .vocabulary
            .iter()
            .find(|word| word.written_form == "これ")
            .unwrap()
            .readings
            .is_empty()
    );
    let supplement: ManualInventory = serde_json::from_value(json!({
        "version": 1, "vocabulary": [],
        "grammar_declarations": [{"id": "topic", "description": "は topic"}],
        "grammar_bindings": [{"declaration_id": "topic", "rule": "TopicWa"}]
    }))
    .unwrap();
    let combined = inventory.with_manual(supplement).unwrap();
    assert_eq!(combined.grammar_declarations[0].id, "topic");
    assert_eq!(combined.grammar_bindings[0].declaration_id, "topic");
}
