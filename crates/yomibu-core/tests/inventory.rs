use serde_json::{Value, json};
use yomibu_core::domain::{
    inventory::{InventoryWord, LearnerInventory, ManualInventory, MappedContent, SourceOrigin},
    knowledge::{ExclusionReason, LearnerKnowledgePolicy, LearnerProgress},
};

fn manual() -> Value {
    json!({"version":1,"vocabulary":[{"id":"cat","written_form":"猫","readings":["ねこ"],"meanings":["cat"],"direct_object":null}],
        "grammar_declarations":[{"id":"topic","description":"は topic"}],
        "grammar_bindings":[{"declaration_id":"topic","rule":"TopicWa"}]})
}

#[test]
fn manual_inventory_keeps_original_readings_and_unknown_evidence() {
    let input: ManualInventory = serde_json::from_value(manual()).unwrap();
    let inventory = LearnerInventory::from_manual(input).unwrap();
    assert_eq!(inventory.vocabulary[0].readings, ["ねこ"]);
    assert_eq!(inventory.vocabulary[0].analyzer_readings(), ["ネコ"]);
    assert_eq!(inventory.vocabulary[0].direct_object, None);
    assert_eq!(inventory.vocabulary[0].origin, SourceOrigin::Manual);
    assert_eq!(inventory.grammar_bindings[0].declaration_id, "topic");
}

#[test]
fn common_preparation_uses_each_learners_progress_for_shared_content() {
    let content: InventoryWord = serde_json::from_value(manual()["vocabulary"][0].clone()).unwrap();
    for (first_progress, second_progress, reason) in [
        (
            LearnerProgress {
                is_marked_known: Some(true),
                ..Default::default()
            },
            LearnerProgress {
                is_marked_known: Some(false),
                ..Default::default()
            },
            ExclusionReason::MarkedUnknown,
        ),
        (
            LearnerProgress {
                learning_started_at: chrono::DateTime::from_timestamp(1, 0),
                ..Default::default()
            },
            LearnerProgress::default(),
            ExclusionReason::NoRecordedLessonStart,
        ),
    ] {
        let policy = LearnerKnowledgePolicy::LessonStarted;
        let first = LearnerInventory::from_mapped(
            [(
                MappedContent::Vocabulary(content.clone()),
                Some(first_progress),
            )],
            &policy,
        )
        .unwrap();
        let second = LearnerInventory::from_mapped(
            [(
                MappedContent::Vocabulary(content.clone()),
                Some(second_progress),
            )],
            &policy,
        )
        .unwrap();
        assert_eq!(first.vocabulary.len(), 1);
        assert_eq!(first.vocabulary[0].id, "cat");
        assert!(first.excluded.is_empty());
        assert!(second.vocabulary.is_empty());
        assert_eq!(second.excluded.len(), 1);
        assert_eq!(second.excluded[0].origin, SourceOrigin::Manual);
        assert_eq!(second.excluded[0].reason, reason);
    }
}

#[test]
fn common_preparation_preserves_content_availability_and_input_order() {
    let content: InventoryWord = serde_json::from_value(manual()["vocabulary"][0].clone()).unwrap();
    let progress = LearnerProgress {
        is_marked_known: Some(true),
        ..Default::default()
    };
    for is_hidden in [false, true] {
        let inventory = LearnerInventory::from_mapped(
            [
                (MappedContent::Vocabulary(content.clone()), None),
                (
                    MappedContent::NonVocabulary(SourceOrigin::Manual),
                    Some(progress),
                ),
                (
                    MappedContent::Unavailable(SourceOrigin::Wanikani { subject_id: 42 }),
                    Some(LearnerProgress {
                        is_hidden,
                        learning_started_at: chrono::DateTime::from_timestamp(1, 0),
                        ..Default::default()
                    }),
                ),
            ],
            &LearnerKnowledgePolicy::LessonStarted,
        )
        .unwrap();
        assert!(inventory.vocabulary.is_empty());
        assert_eq!(
            inventory
                .excluded
                .iter()
                .map(|entry| (&entry.origin, entry.reason))
                .collect::<Vec<_>>(),
            [
                (&SourceOrigin::Manual, ExclusionReason::NoProgress),
                (
                    &SourceOrigin::Wanikani { subject_id: 42 },
                    ExclusionReason::ContentUnavailable
                ),
            ],
        );
    }
}

#[test]
fn inventory_rejects_duplicate_ids_dangling_bindings_and_blank_values() {
    for bad in 0..4 {
        let mut value = manual();
        match bad {
            0 => {
                let entry = value["vocabulary"][0].clone();
                value["vocabulary"].as_array_mut().unwrap().push(entry);
            }
            1 => value["grammar_bindings"][0]["declaration_id"] = json!("missing"),
            2 => value["vocabulary"][0]["readings"] = json!([" "]),
            _ => value["version"] = json!(2),
        }
        let input: ManualInventory = serde_json::from_value(value).unwrap();
        assert!(LearnerInventory::from_manual(input).is_err());
    }
}

#[test]
fn exclusions_serialize_source_identity_and_source_independent_reasons() {
    use yomibu_core::domain::inventory::ExcludedMaterial;
    for (reason, code) in [
        (ExclusionReason::ContentUnavailable, "ContentUnavailable"),
        (ExclusionReason::Hidden, "Hidden"),
        (ExclusionReason::NoProgress, "NoProgress"),
        (ExclusionReason::MarkedUnknown, "MarkedUnknown"),
        (
            ExclusionReason::NoRecordedLessonStart,
            "NoRecordedLessonStart",
        ),
        (ExclusionReason::NoRecordedPass, "NoRecordedPass"),
    ] {
        let excluded = ExcludedMaterial {
            origin: SourceOrigin::Wanikani { subject_id: 42 },
            reason,
        };
        assert_eq!(
            serde_json::to_value(excluded).unwrap(),
            json!({"origin":{"kind":"wanikani","subject_id":42},"reason":code})
        );
    }
}
