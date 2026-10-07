use serde_json::{Value, json};
#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::{
    adapters::openai::{Client, PreparedRequest},
    story::{StoryFormat, StoryGenerationOptions},
};

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

#[tokio::test]
async fn shared_generation_assesses_every_sentence_in_a_passage() {
    use yomibu::{
        adapters::{embeddings::LexicalEmbedder, sudachi::SudachiAnalyzer},
        inventory::LearnerInventory,
        ports::Embedder,
        retrieval::{prepare_cache, prepare_embedding_inputs},
        story::{StoryRequest, generate_story, plan_generation},
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
    let analyzer =
        SudachiAnalyzer::load(test_dictionary::bundle().join("system_core.dic")).unwrap();
    let result = generate_story(&plan, &client, &analyzer).await.unwrap();
    assert_eq!(result.assessments().len(), 3);
    assert!(!result.has_execution_errors());
    let report = serde_json::to_value(yomibu::reports::story::StoryReport::new(
        &request, &plan, &result,
    ))
    .unwrap();
    assert_eq!(report["candidates"].as_array().unwrap().len(), 3);
}
