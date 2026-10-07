#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;

use serde_json::{Value, json};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::{
    adapters::{
        embeddings::LexicalEmbedder,
        openai::{Client, ProviderError},
        sudachi::SudachiAnalyzer,
    },
    candidate::CandidateAssessment,
    evaluation::{CheckKind, CheckOutcome, CheckState},
    inventory::LearnerInventory,
    ports::Embedder,
    retrieval::{EmbeddingCache, prepare_embedding_inputs},
    story::{StoryRequest, generate_story, plan_generation},
};

async fn inputs() -> (LearnerInventory, StoryRequest, EmbeddingCache) {
    let inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    let request: StoryRequest =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    let encoder = LexicalEmbedder::new();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let cache = EmbeddingCache::from_vectors(
        encoder.model_identity().clone(),
        &inputs,
        encoder.embed(&inputs).await.unwrap(),
    )
    .unwrap();
    (inventory, request, cache)
}

#[tokio::test]
async fn shared_workflow_preserves_owned_texts_full_inventory_and_findings_after_inputs_drop() {
    let server = MockServer::start().await;
    let texts = [
        "犬は寝ます。",
        "猫は寝ます。",
        "鳥は寝ます。",
        "qzxvは寝ます。",
    ];
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"synthetic", "model":"reported", "status":"completed", "output":[{"type":"message","role":"assistant","status":"completed",
        "content":[{"type":"output_text","text":json!({"candidates":texts.iter().map(|text| json!({"sentences":[text]})).collect::<Vec<_>>()}).to_string()}]}]
    }))).expect(1).mount(&server).await;
    let result = {
        let (inventory, request, cache) = inputs().await;
        let plan = plan_generation(
            &inventory,
            &request,
            &cache,
            &cache.model,
            2,
            sentence_options(4),
        )
        .unwrap();
        let bytes = plan.prepared_request().body_utf8().as_bytes().to_vec();
        let client = Client::with_base_url("synthetic", &format!("{}/v1/", server.uri())).unwrap();
        let analyzer =
            SudachiAnalyzer::load(test_dictionary::bundle().join("system_core.dic")).unwrap();
        let result = generate_story(&plan, &client, &analyzer).await.unwrap();
        assert_eq!(server.received_requests().await.unwrap()[0].body, bytes);
        result
    };
    assert_eq!(
        result
            .candidates()
            .passages()
            .iter()
            .map(|passage| passage.text.as_str())
            .collect::<Vec<_>>(),
        texts
    );
    assert_eq!(result.candidates().provenance().request_count, 1);
    assert!(!result.has_execution_errors());
    assert_eq!(result.assessments().len(), 4);
    assert_eq!(result.assessments()[0].plan_departures[0].span, 0..3);
    let CandidateAssessment::Completed {
        analysis,
        evaluation,
    } = &result.assessments()[2].assessment
    else {
        panic!()
    };
    assert_eq!(analysis.sentence.text(), texts[2]);
    assert_eq!(
        evaluation.check(CheckKind::Vocabulary).state,
        CheckState::Completed(CheckOutcome::Fail)
    );
    assert_eq!(
        evaluation.check(CheckKind::Vocabulary).findings[0].span,
        0..3
    );
}

#[tokio::test]
async fn offline_planning_validates_options_targets_cache_and_request_bounds() {
    let (inventory, request, cache) = inputs().await;
    assert!(
        plan_generation(
            &inventory,
            &request,
            &cache,
            &cache.model,
            1,
            sentence_options(2)
        )
        .is_err()
    );
    assert!(
        plan_generation(
            &inventory,
            &request,
            &cache,
            &cache.model,
            2,
            yomibu::story::StoryGenerationOptions {
                candidate_count: 0,
                format: yomibu::story::StoryFormat::Sentence,
                ..Default::default()
            }
        )
        .is_err()
    );
    let mut stale = cache.model.clone();
    stale.revision = "stale".into();
    assert!(plan_generation(&inventory, &request, &cache, &stale, 2, sentence_options(2)).is_err());
    let mut invalid: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/story/request.json")).unwrap();
    invalid["targets"]["vocabulary"] = json!(["missing"]);
    let invalid = serde_json::from_value(invalid).unwrap();
    assert!(
        plan_generation(
            &inventory,
            &invalid,
            &cache,
            &cache.model,
            2,
            sentence_options(2)
        )
        .is_err()
    );
    let mut oversized = inventory;
    oversized.grammar_declarations[0].description = "x".repeat(1024);
    for i in 0..30 {
        oversized
            .grammar_declarations
            .push(yomibu::inventory::InventoryGrammar {
                id: format!("extra-{i}"),
                description: "x".repeat(1024),
            });
    }
    let error = plan_generation(
        &oversized,
        &request,
        &cache,
        &cache.model,
        2,
        sentence_options(2),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("16384-byte request limit"),
        "{error}"
    );
}

#[tokio::test]
async fn provider_failure_returns_no_result_and_never_retries() {
    let (inventory, request, cache) = inputs().await;
    let plan = plan_generation(
        &inventory,
        &request,
        &cache,
        &cache.model,
        2,
        sentence_options(2),
    )
    .unwrap();
    let analyzer =
        SudachiAnalyzer::load(test_dictionary::bundle().join("system_core.dic")).unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).set_body_string("secret"))
        .expect(1)
        .mount(&server)
        .await;
    let client = Client::with_base_url("synthetic", &format!("{}/v1/", server.uri())).unwrap();
    assert!(matches!(
        generate_story(&plan, &client, &analyzer).await,
        Err(ProviderError::Http { status: 429 })
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

fn sentence_options(candidate_count: usize) -> yomibu::story::StoryGenerationOptions {
    yomibu::story::StoryGenerationOptions {
        candidate_count,
        format: yomibu::story::StoryFormat::Sentence,
        ..Default::default()
    }
}
