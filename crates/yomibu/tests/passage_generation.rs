use serde_json::{Value, json};
use yomibu::application::selection::select_builtin_vocabulary;
#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu_components::openai_story_generation::{Client, PreparedRequest};
use yomibu_core::domain::story::{StoryFormat, StoryGenerationOptions};

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
        let validation = yomibu_components::openai_story_generation::validate_options(&options)
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
    assert!(yomibu_components::openai_story_generation::validate_options(&options).is_ok());
    assert!(PreparedRequest::new("prompt", "{}", "test", &options, 16384).is_ok());
}

#[test]
fn story_prompt_uses_complete_instructions_for_each_format_and_candidate_count() {
    use yomibu_components::story_prompt_preparation::fit_selection_and_build_request;
    use yomibu_core::domain::inventory::LearnerInventory;
    use yomibu_core::domain::story::StoryRequest;
    let inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
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

#[tokio::test]
async fn shared_generation_assesses_every_sentence_in_a_passage() {
    use yomibu::application::story::{generate_story, plan_generation};
    use yomibu_components::embedding_vocabulary_selection::prepare_embedding_inputs;
    use yomibu_components::lexical_embeddings::LexicalEmbedder;
    use yomibu_core::{
        capabilities::Embedder,
        domain::{inventory::LearnerInventory, story::StoryRequest},
        pipeline::embeddings::prepare_cache,
    };
    let inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    let request: StoryRequest =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    let encoder = LexicalEmbedder::new();
    let cache = prepare_cache(
        &encoder,
        &prepare_embedding_inputs(&inventory, &request).unwrap(),
        None,
    )
    .await
    .unwrap();
    let plan = plan_generation(
        &inventory,
        &request,
        &cache,
        encoder.model_identity(),
        2,
        Default::default(),
    )
    .unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"synthetic", "model":"returned", "status":"completed", "output":[{"type":"message", "role":"assistant", "status":"completed", "content":[{"type":"output_text", "text":json!({"candidates":[{"sentences":["猫は寝ます。","猫です。","猫は寝ます。"]}]}).to_string()}]}]}))).expect(1).mount(&server).await;
    let client = Client::with_base_url("synthetic", &format!("{}/", server.uri())).unwrap();
    let analyzer = test_dictionary::load_analyzer();
    let result = generate_story(&plan, &client, &analyzer).await.unwrap();
    assert_eq!(result.assessments().len(), 3);
    assert!(!result.has_execution_errors());
    let report = serde_json::to_value(yomibu::reports::story::StoryReport::new(
        &request, &plan, &result,
    ))
    .unwrap();
    assert_eq!(report["candidates"].as_array().unwrap().len(), 3);
}
