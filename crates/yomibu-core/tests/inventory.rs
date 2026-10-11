use serde_json::{Value, json};
use yomibu_core::domain::inventory::{LearnerInventory, ManualInventory, SourceOrigin};

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
    use yomibu_core::domain::knowledge::ExclusionReason;
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
