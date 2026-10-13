#[path = "../../../tests/support/story_inventory.rs"]
mod story_inventory;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use yomibu_core::domain::{
    generation::{StoryError, StoryFormat, StoryGenerationOptions, StoryRequest},
    knowledge::inventory::{
        InventoryGrammar, InventoryGrammarBinding, InventoryWord, LearnerInventory, SourceOrigin,
    },
};
use yomibu_core::extension::options::Options;
use yomibu_ext::learner_vocabulary_selection::select_builtin_vocabulary;
use yomibu_ext::{
    openai::{self, preparation::fit_selection_and_build_request},
    wanikani,
};

#[tokio::test]
async fn current_story_request_matches_the_reviewed_prompt_and_payload_snapshot() {
    let inventory = story_inventory::inventory();
    let request =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    let selection = select_builtin_vocabulary(&inventory, &request, 2, 7).unwrap();
    let options = openai::generation_options(
        Options::default().for_component(&openai::COMPONENT),
        1,
        StoryFormat::Passage,
    )
    .unwrap();
    let (_, prepared) =
        fit_selection_and_build_request(&inventory, &request, selection, options).unwrap();
    let body = prepared.body_utf8();
    let snapshot = include_str!("../../../tests/fixtures/story/provider-request.json");
    assert!(
        body == snapshot,
        "Current request differs from tests/fixtures/story/provider-request.json at byte {}.\nExpected:\n{}\nActual:\n{}",
        body.bytes()
            .zip(snapshot.bytes())
            .position(|(actual, expected)| actual != expected)
            .unwrap_or(body.len().min(snapshot.len())),
        request_for_review(snapshot),
        request_for_review(body),
    );
    assert_eq!(
        prepared.sha256(),
        format!("{:x}", Sha256::digest(body.as_bytes()))
    );
}

fn request_for_review(body: &str) -> String {
    let mut payload: Value = serde_json::from_str(body).unwrap();
    for input in payload["input"].as_array_mut().unwrap() {
        if input["role"] == "user" {
            input["content"] = serde_json::from_str(input["content"].as_str().unwrap()).unwrap();
        }
    }
    serde_json::to_string_pretty(&payload).unwrap()
}
fn inventory() -> LearnerInventory {
    LearnerInventory {
        vocabulary: [
            ("cat", "猫", "ねこ"),
            ("station", "駅", "えき"),
            ("meet", "会う", "あう"),
        ]
        .into_iter()
        .map(|(id, written, reading)| InventoryWord {
            id: id.into(),
            written_form: written.into(),
            readings: vec![reading.into()],
            meanings: vec![id.into()],
            direct_object: None,
            origin: SourceOrigin::Manual,
        })
        .collect(),
        grammar_declarations: vec![InventoryGrammar {
            id: "past".into(),
            description: "polite past".into(),
        }],
        grammar_bindings: vec![InventoryGrammarBinding {
            declaration_id: "past".into(),
            rule: yomibu_core::domain::knowledge::grammar::GrammarRule::PolitePast,
        }],
        excluded: vec![],
    }
}
fn request() -> StoryRequest {
    serde_json::from_value(json!({"version":1,"topic":"Meeting at a train station","targets":{"vocabulary":["cat"],"grammar":["past"]}})).unwrap()
}
#[test]
fn targets_cannot_grant_inventory_entries_or_grammar_recognizers() {
    let inventory = inventory();
    let mut r = request();
    r.targets.vocabulary.push("missing".into());
    assert!(r.validate(&inventory).is_err());
    let mut r = request();
    r.targets.grammar.push("question-ka".into());
    assert!(r.validate(&inventory).is_err());
    let mut r = request();
    r.targets.vocabulary.push("cat".into());
    assert!(r.validate(&inventory).is_err());
    let mut r = request();
    r.targets.vocabulary.clear();
    r.targets.grammar.clear();
    assert!(r.validate(&inventory).is_ok());
}

#[tokio::test]
async fn provenance_does_not_change_the_prepared_request() {
    use yomibu_ext::openai::preparation::fit_selection_and_build_request;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/preparation.json")).unwrap();
    let source = serde_json::from_value(fixture["snapshot"].clone()).unwrap();
    let wk = LearnerInventory::from_mapped(wanikani::map_content_and_progress(&source).unwrap())
        .unwrap();
    let manual = LearnerInventory {
        vocabulary: wk
            .vocabulary
            .iter()
            .cloned()
            .map(|mut word| {
                word.origin = SourceOrigin::Manual;
                word
            })
            .collect(),
        grammar_declarations: vec![],
        grammar_bindings: vec![],
        excluded: vec![],
    };
    let request: StoryRequest = serde_json::from_value(
        json!({"version":1,"topic":"one thing","targets":{"vocabulary":[],"grammar":[]}}),
    )
    .unwrap();
    let mut requests = Vec::new();
    for inventory in [&wk, &manual] {
        let plan = select_builtin_vocabulary(inventory, &request, 4, 7).unwrap();
        let (_, prepared_request) = fit_selection_and_build_request(
            inventory,
            &request,
            plan,
            StoryGenerationOptions {
                candidate_count: 1,
                format: StoryFormat::Passage,
                model: "test-model".into(),
            },
        )
        .unwrap();
        requests.push(prepared_request);
    }
    assert_eq!(requests[0].body_utf8(), requests[1].body_utf8());
    assert_eq!(requests[0].sha256(), requests[1].sha256());
    assert!(!requests[0].body_utf8().contains("synthetic-learner"));
    assert!(!requests[0].body_utf8().contains("sync_completed_at"));
}

#[test]
fn generation_options_keep_count_separate_and_validate_real_arithmetic_bounds() {
    assert!(
        serde_json::to_value(request())
            .unwrap()
            .get("candidate_count")
            .is_none()
    );
    for count in [1, 2, 8, 9, 100, usize::MAX / 2560] {
        assert!(
            yomibu_ext::openai::validate_options(&StoryGenerationOptions {
                candidate_count: count,
                format: StoryFormat::Passage,
                model: "test-model".into(),
            })
            .is_ok()
        );
    }
    for count in [0, usize::MAX / 2560 + 1, usize::MAX] {
        assert!(
            yomibu_ext::openai::validate_options(&StoryGenerationOptions {
                candidate_count: count,
                format: StoryFormat::Passage,
                model: "test-model".into(),
            })
            .is_err()
        );
    }
}

#[test]
fn request_bounds_count_topic_bytes_and_each_target_kind_independently() {
    let mut inventory = inventory();
    let word = inventory.vocabulary[0].clone();
    for index in 0..17 {
        let mut word = word.clone();
        word.id = format!("word-{index}");
        inventory.vocabulary.push(word);
        inventory.grammar_declarations.push(
            yomibu_core::domain::knowledge::inventory::InventoryGrammar {
                id: format!("grammar-{index}"),
                description: "familiar grammar".into(),
            },
        );
    }
    let mut request = request();
    request.targets.vocabulary = (0..16).map(|index| format!("word-{index}")).collect();
    request.targets.grammar = (0..16).map(|index| format!("grammar-{index}")).collect();
    request.topic = Some(
        yomibu_core::domain::generation::StoryTopic::new(format!("{}ab", "猫".repeat(682)))
            .unwrap(),
    );
    assert_eq!(request.topic.as_ref().unwrap().text().len(), 2048);
    assert!(request.validate(&inventory).is_ok());
    assert!(request.validate_selection_limit(16).is_ok());
    for limit in [0, 15, 17] {
        assert!(request.validate_selection_limit(limit).is_err());
    }

    request.topic = Some(
        serde_json::from_value(json!(format!(
            "{}c",
            request.topic.as_ref().unwrap().text()
        )))
        .unwrap(),
    );
    assert!(matches!(
        request.validate(&inventory),
        Err(StoryError::Invalid("version, topic or target limits"))
    ));
    request.topic = Some(
        yomibu_core::domain::generation::StoryTopic::new(format!("{}ab", "猫".repeat(682)))
            .unwrap(),
    );
    request.targets.vocabulary.push("word-16".into());
    assert!(request.validate(&inventory).is_err());
    request.targets.vocabulary.pop();
    request.targets.grammar.push("grammar-16".into());
    assert!(request.validate(&inventory).is_err());
}

#[test]
fn request_budget_drops_lowest_ranked_supports_and_keeps_targets_and_grammar() {
    let mut inventory = inventory();
    for word in &mut inventory.vocabulary {
        word.meanings = vec!["x".repeat(1024); 5];
    }
    inventory.grammar_declarations[0].description = "g".repeat(1024);
    let request = request();
    let selection = select_builtin_vocabulary(&inventory, &request, 3, 7).unwrap();
    assert_eq!(selection.selected.len(), 3);

    let (selection, prepared_request) = fit_selection_and_build_request(
        &inventory,
        &request,
        selection,
        StoryGenerationOptions {
            candidate_count: 1,
            format: StoryFormat::Passage,
            model: "test-model".into(),
        },
    )
    .unwrap();

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["cat", "meet"]
    );
    assert_eq!(selection.selected[0].reason, "practice_target");
    assert_eq!(selection.selected[1].reason, "local_sample");
    assert!(prepared_request.body_utf8().len() <= 16384);
    let body: serde_json::Value = serde_json::from_str(prepared_request.body_utf8()).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(body["input"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        data["targets"],
        serde_json::to_value(&request.targets).unwrap()
    );
    assert_eq!(
        data["grammar_declarations"],
        serde_json::to_value(&inventory.grammar_declarations).unwrap()
    );
    assert_eq!(
        data["grammar_bindings"],
        serde_json::to_value(&inventory.grammar_bindings).unwrap()
    );
    assert_eq!(inventory.vocabulary.len(), 3);
}

#[test]
fn request_budget_rejects_targets_that_cannot_fit() {
    let mut inventory = inventory();
    for word in &mut inventory.vocabulary {
        word.meanings = vec!["x".repeat(1024); 12];
    }
    let mut request = request();
    request.targets.vocabulary = vec!["cat".into(), "station".into()];
    let selection = select_builtin_vocabulary(&inventory, &request, 2, 7).unwrap();

    assert!(matches!(
        fit_selection_and_build_request(&inventory, &request, selection, StoryGenerationOptions {
            candidate_count: 1,
            format: StoryFormat::Passage,
            model: "test-model".into(),
        }),
        Err(StoryError::RequiredMaterialTooLarge { bytes, limit: 16384 }) if bytes > 16384
    ));
}
