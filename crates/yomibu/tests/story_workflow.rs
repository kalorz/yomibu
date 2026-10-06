use serde_json::{Value, json};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::{
    adapters::{
        embeddings::LexicalEmbedder,
        openai::{Client, ProviderError},
        sudachi::SudachiAnalyzer,
    },
    analysis::SentenceError,
    evaluation::{CheckKind, CheckOutcome, CheckState},
    generation::{CandidateAssessment, CandidateError},
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
async fn shared_workflow_preserves_exact_request_full_inventory_and_partial_results_after_inputs_drop()
 {
    let server = MockServer::start().await;
    let oversized = "猫".repeat(101);
    let texts = ["犬は寝ます。", " \n", "鳥は寝ます。", oversized.as_str()];
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id":"synthetic", "model":"reported", "status":"completed",
            "output":[{"type":"message","role":"assistant","status":"completed",
                "content":[{"type":"output_text","text":json!({"candidates":texts}).to_string()}]}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let result = {
        let (inventory, request, cache) = inputs().await;
        let options = yomibu::story::StoryGenerationOptions { candidate_count: 4 };
        let plan = plan_generation(&inventory, &request, &cache, &cache.model, 2, options).unwrap();
        let bytes = plan.ai_model_request().body_utf8().as_bytes().to_vec();
        let client = Client::with_base_url("synthetic", &format!("{}/v1/", server.uri())).unwrap();
        let analyzer = SudachiAnalyzer::load(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/a1/current/system_core.dic"),
        )
        .unwrap();
        let result = generate_story(&plan, &client, &analyzer).await.unwrap();
        assert_eq!(server.received_requests().await.unwrap()[0].body, bytes);
        result
    };
    assert_eq!(result.candidates().texts(), &texts);
    assert_eq!(result.candidates().provenance().request_count, 1);
    assert!(result.has_execution_errors());
    let assessments = result.assessments();
    assert_eq!(assessments.len(), 4);
    let CandidateAssessment::Completed {
        analysis,
        evaluation,
    } = &assessments[0].assessment
    else {
        panic!()
    };
    assert_eq!(analysis.sentence.text(), texts[0]);
    assert_eq!(
        evaluation.check(CheckKind::Vocabulary).state,
        CheckState::Completed(CheckOutcome::Pass)
    );
    assert_eq!(assessments[0].plan_departures[0].span, 0..3);
    assert!(matches!(
        assessments[1].assessment,
        CandidateAssessment::ExecutionError {
            analysis: None,
            error: CandidateError::Sentence(SentenceError::Blank)
        }
    ));
    assert_eq!(assessments[1].targets[0].status, "not_run");
    assert!(matches!(
        assessments[3].assessment,
        CandidateAssessment::ExecutionError {
            analysis: None,
            error: CandidateError::Sentence(SentenceError::TooLong { .. })
        }
    ));
    assert_eq!(assessments[3].targets[0].status, "not_run");
    let CandidateAssessment::Completed { evaluation, .. } = &assessments[2].assessment else {
        panic!()
    };
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
    let plan = plan_generation(
        &inventory,
        &request,
        &cache,
        &cache.model,
        2,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        plan.ai_model_request().body_utf8(),
        include_str!("../../../tests/fixtures/story/provider-request.json")
    );
    assert_eq!(
        plan.ai_model_request().sha256(),
        include_str!("../../../tests/fixtures/story/provider-request.sha256").trim()
    );
    assert!(
        plan_generation(
            &inventory,
            &request,
            &cache,
            &cache.model,
            1,
            Default::default()
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
            yomibu::story::StoryGenerationOptions { candidate_count: 0 }
        )
        .is_err()
    );
    let mut stale = cache.model.clone();
    stale.revision = "stale".into();
    assert!(plan_generation(&inventory, &request, &cache, &stale, 2, Default::default()).is_err());
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
            Default::default()
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
        Default::default(),
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
        Default::default(),
    )
    .unwrap();
    let analyzer = SudachiAnalyzer::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/a1/current/system_core.dic"),
    )
    .unwrap();
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
