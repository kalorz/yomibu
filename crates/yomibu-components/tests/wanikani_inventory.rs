use serde_json::{Value, json};
use yomibu_components::wanikani_source::project_inventory;
use yomibu_core::domain::{
    inventory::{LearnerInventory, ManualInventory, SourceOrigin},
    knowledge::{ExclusionReason, LearnerKnowledgePolicy},
    source::WaniKaniSyncData,
};

fn source() -> WaniKaniSyncData {
    let value: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/mixed.json")).unwrap();
    serde_json::from_value(value["snapshot"].clone()).unwrap()
}

fn exclusion(inventory: &LearnerInventory, id: u64) -> Option<ExclusionReason> {
    inventory
        .excluded
        .iter()
        .find(|entry| entry.origin == SourceOrigin::Wanikani { subject_id: id })
        .map(|entry| entry.reason)
}

#[test]
fn projection_retains_exclusion_reasons_in_source_subject_order() {
    use ExclusionReason::{ContentUnavailable, Hidden, NoProgress, NoRecordedPass};
    let data = source();
    let inventory = project_inventory(&data, &LearnerKnowledgePolicy::default()).unwrap();
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
    let passed = project_inventory(&data, &LearnerKnowledgePolicy::RecordedPass).unwrap();
    assert_eq!(exclusion(&passed, 3), Some(NoRecordedPass));
}

#[test]
fn lifecycle_evidence_not_stage_accuracy_or_burned_state_controls_eligibility() {
    let mut data = source();
    data.subjects[0].hidden_at = None;
    data.assignments[1].hidden = false;
    data.assignments[2].srs_stage = 999;
    let policy = LearnerKnowledgePolicy::default();
    let inventory = project_inventory(&data, &policy).unwrap();
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
    let passed = project_inventory(&data, &LearnerKnowledgePolicy::RecordedPass).unwrap();
    assert_eq!(passed.vocabulary[0].id, "wanikani:2");
    data.review_statistics[1].hidden = true;
    assert_eq!(
        exclusion(&project_inventory(&data, &policy).unwrap(), 2),
        Some(ExclusionReason::Hidden)
    );
    data.assignments[2].started_at = None;
    assert_eq!(
        exclusion(&project_inventory(&data, &policy).unwrap(), 3),
        Some(ExclusionReason::NoRecordedLessonStart)
    );
    data.review_statistics[2].hidden = true;
    assert_eq!(
        exclusion(&project_inventory(&data, &policy).unwrap(), 4),
        Some(ExclusionReason::Hidden)
    );
}

#[test]
fn projection_order_is_stable_and_invalid_direct_input_is_rejected() {
    let mut data = source();
    let policy = LearnerKnowledgePolicy::default();
    let expected = serde_json::to_value(project_inventory(&data, &policy).unwrap()).unwrap();
    data.subjects.reverse();
    data.assignments.reverse();
    data.review_statistics.reverse();
    assert_eq!(
        serde_json::to_value(project_inventory(&data, &policy).unwrap()).unwrap(),
        expected
    );
    data.subjects.clear();
    assert!(project_inventory(&data, &policy).is_err());
}

#[test]
fn account_progress_independently_controls_eligibility_for_shared_content() {
    let first = source();
    let mut second = source();
    second.learner.id = "another-learner".into();
    second.assignments[2].started_at = None;
    let policy = LearnerKnowledgePolicy::default();
    let first_inventory = project_inventory(&first, &policy).unwrap();
    let second_inventory = project_inventory(&second, &policy).unwrap();
    assert_eq!(first_inventory.vocabulary.len(), 1);
    assert_eq!(
        first_inventory.vocabulary[0].origin,
        SourceOrigin::Wanikani { subject_id: 3 }
    );
    assert_eq!(first_inventory.vocabulary[0].written_form, "これ");
    assert!(second_inventory.vocabulary.is_empty());
    assert_eq!(
        exclusion(&second_inventory, 3),
        Some(ExclusionReason::NoRecordedLessonStart)
    );
}

#[test]
fn projection_preserves_unpaired_reading_and_meaning_alternatives() {
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
    let inventory = project_inventory(&data, &LearnerKnowledgePolicy::default()).unwrap();
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
fn projection_keeps_missing_readings_and_manual_grammar() {
    let value: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/preparation.json")).unwrap();
    let source = serde_json::from_value(value["snapshot"].clone()).unwrap();
    let inventory = project_inventory(&source, &LearnerKnowledgePolicy::default()).unwrap();
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
