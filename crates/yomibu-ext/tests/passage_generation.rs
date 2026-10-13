#[path = "../../../tests/support/story_inventory.rs"]
mod story_inventory;

use serde_json::{Value, json};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu_core::domain::generation::{StoryFormat, StoryGenerationOptions};
use yomibu_ext::learner_vocabulary_selection::select_builtin_vocabulary;
use yomibu_ext::openai::{Client, PreparedRequest};

#[tokio::test]
async fn passage_request_uses_selected_model_and_preserves_sentence_byte_ranges() {
    let options = StoryGenerationOptions {
        model: "chosen-model".into(),
        ..Default::default()
    };
    let request = PreparedRequest::new("prompt", "{}", "test", &options, 16384).unwrap();
    let body: Value = serde_json::from_str(request.body_utf8()).unwrap();
    assert_eq!(body["model"], "chosen-model");
    assert_eq!(body["max_output_tokens"], 2560);
    let schema = &body["text"]["format"]["schema"]["properties"]["candidates"]["items"]["properties"]
        ["sentences"];
    assert_eq!(schema["minItems"], 3);
    assert_eq!(schema["maxItems"], 5);
    assert_eq!(schema["items"]["maxLength"], 100);
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"synthetic", "model":"returned-model", "status":"completed",
        "output":[{"type":"message", "role":"assistant", "status":"completed", "content":[{"type":"output_text",
        "text":json!({"candidates":[{"sentences":["猫です。", "寝ます。", "朝です。"]}]}).to_string()}]}]
    }))).expect(1).mount(&server).await;
    let generated = Client::with_base_url("synthetic", &format!("{}/", server.uri()))
        .unwrap()
        .generate_candidates(&request)
        .await
        .unwrap();
    assert_eq!(generated.provenance().requested_model, "chosen-model");
    let passage = &generated.passages()[0];
    assert_eq!(passage.text, "猫です。寝ます。朝です。");
    assert_eq!(passage.sentence_spans, [0..12, 12..24, 24..36]);
}

#[test]
fn sentence_mode_has_one_sentence_and_its_own_budget() {
    let options = StoryGenerationOptions {
        format: StoryFormat::Sentence,
        ..Default::default()
    };
    let request = PreparedRequest::new("prompt", "{}", "test", &options, 16384).unwrap();
    let body: Value = serde_json::from_str(request.body_utf8()).unwrap();
    assert_eq!(body["max_output_tokens"], 512);
    assert_eq!(
        body["text"]["format"]["schema"]["properties"]["candidates"]["items"]["properties"]["sentences"]
            ["maxItems"],
        1
    );
}

#[test]
fn invalid_text_models_report_the_model_instead_of_the_candidate_count() {
    for model in [String::new(), " \n".into(), "x".repeat(257)] {
        let options = StoryGenerationOptions {
            model,
            ..Default::default()
        };
        let validation = yomibu_ext::openai::validate_options(&options)
            .unwrap_err()
            .to_string();
        let preparation = PreparedRequest::new("prompt", "{}", "test", &options, 16384)
            .unwrap_err()
            .to_string();
        for error in [validation, preparation] {
            assert!(error.contains("model"), "{error}");
            assert!(
                !error.to_ascii_lowercase().contains("candidate count"),
                "{error}"
            );
        }
    }
    let options = StoryGenerationOptions {
        model: "x".repeat(256),
        ..Default::default()
    };
    assert!(yomibu_ext::openai::validate_options(&options).is_ok());
    assert!(PreparedRequest::new("prompt", "{}", "test", &options, 16384).is_ok());
}

#[test]
fn story_prompt_uses_complete_instructions_for_each_format_and_candidate_count() {
    use yomibu_core::domain::generation::StoryRequest;
    use yomibu_ext::openai::preparation::fit_selection_and_build_request;
    let inventory = story_inventory::inventory();
    let request: StoryRequest =
        serde_json::from_value(json!({"version":1,"targets":{"vocabulary":[],"grammar":[]}}))
            .unwrap();
    for (format, sentences) in [
        (StoryFormat::Sentence, "one sentence"),
        (StoryFormat::Passage, "3–5 sentences"),
    ] {
        for candidate_count in [1, 2] {
            let selection = select_builtin_vocabulary(&inventory, &request, 2, 7).unwrap();
            let (_, prepared) = fit_selection_and_build_request(
                &inventory,
                &request,
                selection,
                StoryGenerationOptions {
                    format,
                    candidate_count,
                    ..Default::default()
                },
            )
            .unwrap();
            let body: Value = serde_json::from_str(prepared.body_utf8()).unwrap();
            let prompt = body["input"][0]["content"].as_str().unwrap();
            let opening = format!(
                "Candidate count: {candidate_count}. Each candidate must contain {sentences}. Generate short, natural stories in ordinary modern Japanese. Each sentence must be nonblank and at most 100 Unicode scalar values. "
            );
            assert!(prompt.starts_with(&opening), "{prompt}");
            assert_eq!(prepared.prompt_revision(), "story-inventory-v3");
        }
    }
}
