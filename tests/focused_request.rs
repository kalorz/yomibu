use std::{fs, path::Path};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use yomibu::{
    adapters::openai::{ProviderError, prepare_focused_request},
    evaluation::{EvaluationBindings, GrammarBinding, GrammarRule, VocabularyEntry},
    generation::GenerationError,
    generation_context::{VocabularyEntryId, select_context},
    grammar::GrammarDeclarations,
};

fn input() -> (GrammarDeclarations, EvaluationBindings) {
    input_from_json(include_bytes!("fixtures/focused/pet-rest.json"))
}

fn input_from_json(bytes: &[u8]) -> (GrammarDeclarations, EvaluationBindings) {
    let value: Value = serde_json::from_slice(bytes).unwrap();
    (
        GrammarDeclarations::from_descriptions(
            value["grammar"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap()),
        )
        .unwrap(),
        serde_json::from_value(value["bindings"].clone()).unwrap(),
    )
}

#[test]
fn preparation_is_pure_selected_only_and_preserves_all_grammar_verbatim() {
    let (_, mut permissions) = input();
    let descriptions = [
        "extra\n\u{1b}description",
        "は — topic marker",
        "duplicate",
        "duplicate",
        "ます — polite nonpast",
    ];
    let grammar = GrammarDeclarations::from_descriptions(descriptions).unwrap();
    permissions.grammar = vec![
        GrammarBinding {
            declaration_id: 5,
            rule: GrammarRule::PoliteNonPast,
        },
        GrammarBinding {
            declaration_id: 2,
            rule: GrammarRule::TopicWa,
        },
        GrammarBinding {
            declaration_id: 2,
            rule: GrammarRule::TopicWa,
        },
        GrammarBinding {
            declaration_id: 1,
            rule: GrammarRule::PolitePast,
        },
    ];
    let context =
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap();
    let prepared = prepare_focused_request(context).unwrap();
    assert_eq!(prepared.bytes(), prepared.body_utf8().len());
    assert_eq!(
        prepared.sha256(),
        format!("{:x}", Sha256::digest(prepared.body_utf8().as_bytes()))
    );
    assert_eq!(prepared.method(), "POST");
    assert_eq!(prepared.url(), "https://api.openai.com/v1/responses");
    let body: Value = serde_json::from_str(prepared.body_utf8()).unwrap();
    assert_eq!(body.as_object().unwrap().len(), 13);
    assert_eq!(body["model"], "gpt-6-luna");
    assert_eq!(body["service_tier"], "default");
    assert_eq!(body["reasoning"], json!({"effort":"none"}));
    assert_eq!(body["max_output_tokens"], 1024);
    for key in ["store", "background", "stream"] {
        assert_eq!(body[key], false);
    }
    assert_eq!(body["truncation"], "disabled");
    assert_eq!(body["tools"], json!([]));
    assert_eq!(body["tool_choice"], "none");
    assert_eq!(body["prompt_cache_options"], json!({"mode":"explicit"}));
    assert_eq!(body["text"]["format"]["name"], "sentence_candidates");
    assert_eq!(body["text"]["format"]["strict"], true);
    let user: Value = serde_json::from_str(body["input"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        user,
        json!({"version":1,"kind":"focused_sentence_context","vocabulary":permissions.vocabulary[..2],"focus":{"vocabulary_index":1},"situation":{"id":"pet-rest","description":"A pet rests. Describe the selected pet sleeping.","required_grammar":["TopicWa","PoliteNonPast"]},"grammar":descriptions,"grammar_bindings":permissions.grammar})
    );
    for excluded in [
        "歩く",
        "犬",
        "learner",
        "exclusions",
        "/tmp/",
        "dictionary",
        "entry_number",
    ] {
        assert!(!prepared.body_utf8().contains(excluded));
    }
    assert!(std::ptr::eq(prepared.context().permissions(), &permissions));
}

#[test]
fn six_thousand_entry_inventory_does_not_expand_the_request() {
    let (grammar, mut permissions) = input();
    let focus = VocabularyEntryId::new(1).unwrap();
    let baseline = prepare_focused_request(select_context(&grammar, &permissions, focus).unwrap())
        .unwrap()
        .body_utf8()
        .to_owned();
    for n in 4..6000 {
        permissions.vocabulary.push(VocabularyEntry {
            written_form: format!("無関係{n}"),
            reading: "ムカンケイ".into(),
            sense: format!("synthetic unrelated entry {n}"),
            direct_object: false,
        });
    }
    assert!(serde_json::to_vec(&permissions).unwrap().len() > 65536);
    let request =
        prepare_focused_request(select_context(&grammar, &permissions, focus).unwrap()).unwrap();
    assert_eq!(request.body_utf8(), baseline);
    assert_eq!(request.context().selected().len(), 2);
    assert!(request.bytes() <= 16384);
}

#[test]
fn exact_outbound_byte_limit_and_escaping_expansion_are_checked_without_truncation() {
    let (_, permissions) = input();
    let focus = VocabularyEntryId::new(1).unwrap();
    let mut descriptions = vec!["x".to_owned(); 32];
    let grammar = GrammarDeclarations::from_descriptions(&descriptions).unwrap();
    let length = prepare_focused_request(select_context(&grammar, &permissions, focus).unwrap())
        .unwrap()
        .bytes();
    let mut padding = 16384 - length;
    for description in &mut descriptions {
        let add = padding.min(1023);
        description.push_str(&"x".repeat(add));
        padding -= add;
    }
    assert_eq!(padding, 0);
    let grammar = GrammarDeclarations::from_descriptions(&descriptions).unwrap();
    assert_eq!(
        prepare_focused_request(select_context(&grammar, &permissions, focus).unwrap())
            .unwrap()
            .bytes(),
        16384
    );
    descriptions.last_mut().unwrap().push('x');
    let grammar = GrammarDeclarations::from_descriptions(&descriptions).unwrap();
    assert!(matches!(
        prepare_focused_request(select_context(&grammar, &permissions, focus).unwrap()),
        Err(GenerationError::Provider(ProviderError::RequestTooLarge))
    ));
    let grammar = GrammarDeclarations::from_descriptions(vec!["\u{1b}".repeat(1024); 3]).unwrap();
    assert!(matches!(
        prepare_focused_request(select_context(&grammar, &permissions, focus).unwrap()),
        Err(GenerationError::Provider(ProviderError::RequestTooLarge))
    ));
}

#[test]
fn canonical_v2_request_enforces_selected_lexical_boundary_and_matches_exact_bytes() {
    let (grammar, permissions) = input();
    let request = prepare_focused_request(
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(request.prompt_revision(), "g2-focused-sentence-v2");
    assert_eq!(
        request.body_utf8().as_bytes(),
        include_bytes!("fixtures/focused/pet-rest-request.json")
    );
    assert_eq!(request.bytes(), 2312);
    assert_eq!(
        request.sha256(),
        "a3cbfc09ec6cb2b1264737a0ba97e90367644e91b151a0689f6276d2c4b43422"
    );
}

#[test]
fn current_comparison_requests_match_exact_fixtures_and_manifest() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/focused/comparison");
    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/focused/comparison/manifest.json")).unwrap();
    let current = &manifest["versions"]["v2"];
    assert_eq!(manifest["inputs"].as_object().unwrap().len(), 3);
    assert_eq!(current["requests"].as_object().unwrap().len(), 3);

    let read_fixture = |record: &Value| {
        let path = record["path"].as_str().unwrap();
        let bytes = fs::read(fixtures.join(path)).unwrap();
        assert_eq!(record["bytes"], json!(bytes.len()), "{path}: byte length");
        assert_eq!(
            record["sha256"],
            json!(format!("{:x}", Sha256::digest(&bytes))),
            "{path}: SHA-256"
        );
        bytes
    };

    for situation in ["pet-rest", "pet-walk", "book-reading"] {
        let input_record = &manifest["inputs"][situation];
        assert_eq!(input_record["path"], format!("{situation}.json"));
        let (grammar, permissions) = input_from_json(&read_fixture(input_record));
        let focus = VocabularyEntryId::new(
            usize::try_from(input_record["focus_entry"].as_u64().unwrap()).unwrap(),
        )
        .unwrap();
        let request =
            prepare_focused_request(select_context(&grammar, &permissions, focus).unwrap())
                .unwrap();
        assert_eq!(request.context().situation().id, situation);
        assert_eq!(current["prompt_revision"], request.prompt_revision());

        let request_record = &current["requests"][situation];
        assert_eq!(
            request_record["path"],
            format!("{situation}-v2-request.json")
        );
        assert_eq!(
            request.body_utf8().as_bytes(),
            read_fixture(request_record),
            "{situation}: exact prepared request"
        );
        assert_eq!(request_record["bytes"], json!(request.bytes()));
        assert_eq!(request_record["sha256"], request.sha256());
    }
}
