use serde_json::json;
use yomibu_core::domain::{
    inventory::{
        InventoryGrammar, InventoryGrammarBinding, InventoryWord, LearnerInventory, MappedContent,
        SourceOrigin,
    },
    knowledge::{ExclusionReason, LearnerProgress},
};

fn inventory() -> LearnerInventory {
    LearnerInventory {
        vocabulary: vec![InventoryWord {
            id: "cat".into(),
            written_form: "猫".into(),
            readings: vec!["ねこ".into()],
            meanings: vec!["cat".into()],
            direct_object: None,
            origin: SourceOrigin::Manual,
        }],
        grammar_declarations: vec![InventoryGrammar {
            id: "topic".into(),
            description: "は topic".into(),
        }],
        grammar_bindings: vec![InventoryGrammarBinding {
            declaration_id: "topic".into(),
            rule: yomibu_core::domain::grammar::GrammarRule::TopicWa,
        }],
        excluded: vec![],
    }
}

#[test]
fn inventory_keeps_original_readings_and_unknown_evidence() {
    let inventory = inventory();
    inventory.validate().unwrap();
    assert_eq!(inventory.vocabulary[0].readings, ["ねこ"]);
    assert_eq!(inventory.vocabulary[0].direct_object, None);
    assert_eq!(inventory.vocabulary[0].origin, SourceOrigin::Manual);
    assert_eq!(inventory.grammar_bindings[0].declaration_id, "topic");
}

#[test]
fn common_preparation_uses_each_learners_progress_for_shared_content() {
    let content = inventory().vocabulary.remove(0);
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
        let first = LearnerInventory::from_mapped([(
            MappedContent::Vocabulary(content.clone()),
            Some(first_progress),
        )])
        .unwrap();
        let second = LearnerInventory::from_mapped([(
            MappedContent::Vocabulary(content.clone()),
            Some(second_progress),
        )])
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
    let content = inventory().vocabulary.remove(0);
    let progress = LearnerProgress {
        is_marked_known: Some(true),
        ..Default::default()
    };
    for is_hidden in [false, true] {
        let inventory = LearnerInventory::from_mapped([
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
        ])
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
    for bad in 0..3 {
        let mut inventory = inventory();
        match bad {
            0 => inventory.vocabulary.push(inventory.vocabulary[0].clone()),
            1 => inventory.grammar_bindings[0].declaration_id = "missing".into(),
            _ => inventory.vocabulary[0].readings = vec![" ".into()],
        }
        assert!(inventory.validate().is_err());
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
