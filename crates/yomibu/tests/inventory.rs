use serde_json::{Value, json};
use yomibu::{
    inventory::{LearnerInventory, ManualInventory, SourceOrigin},
    knowledge::LearnerKnowledgePolicy,
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
fn wanikani_projection_keeps_eligibility_alternatives_and_missing_readings() {
    let value: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/preparation.json")).unwrap();
    let source = serde_json::from_value(value["snapshot"].clone()).unwrap();
    let inventory =
        LearnerInventory::from_wanikani(&source, &LearnerKnowledgePolicy::default()).unwrap();
    assert!(
        inventory
            .vocabulary
            .iter()
            .all(|v| v.id.starts_with("wanikani:"))
    );
    assert!(
        inventory
            .vocabulary
            .iter()
            .all(|v| v.direct_object.is_none())
    );
    assert!(inventory.excluded.iter().any(|v| v.subject_id == 1));
    let kana = inventory
        .vocabulary
        .iter()
        .find(|v| v.written_form == "これ")
        .unwrap();
    assert!(kana.readings.is_empty());
    let mut supplement: ManualInventory = serde_json::from_value(manual()).unwrap();
    supplement.vocabulary.clear();
    let combined = inventory.with_manual(supplement).unwrap();
    assert_eq!(combined.grammar_declarations[0].id, "topic");
}

#[test]
fn typed_exclusion_reasons_keep_the_existing_inventory_json_codes() {
    use yomibu::{inventory::ExcludedMaterial, knowledge::ExclusionReason};
    for (reason, code) in [
        (ExclusionReason::ContentUnavailable, "ContentUnavailable"),
        (ExclusionReason::Hidden, "Hidden"),
        (ExclusionReason::NoAssignment, "NoAssignment"),
        (
            ExclusionReason::NoRecordedLessonStart,
            "NoRecordedLessonStart",
        ),
        (ExclusionReason::NoRecordedPass, "NoRecordedPass"),
    ] {
        let excluded = ExcludedMaterial {
            subject_id: 42,
            reason,
        };
        assert_eq!(
            serde_json::to_value(excluded).unwrap(),
            json!({"subject_id":42,"reason":code})
        );
    }
}
