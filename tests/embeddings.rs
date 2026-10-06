use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::{
    adapters::embeddings::{HttpEmbedder, LexicalEmbedder},
    ports::Embedder,
    retrieval::{EmbeddingInput, EmbeddingModelIdentity, EmbeddingPurpose},
};
fn inputs() -> Vec<EmbeddingInput> {
    vec![
        EmbeddingInput {
            purpose: EmbeddingPurpose::Document,
            text: "猫 cat".into(),
        },
        EmbeddingInput {
            purpose: EmbeddingPurpose::Query,
            text: "cat".into(),
        },
    ]
}
fn identity() -> EmbeddingModelIdentity {
    EmbeddingModelIdentity {
        provider: "local".into(),
        model: "test-model".into(),
        revision: "fixed-test-revision".into(),
        dimensions: 2,
        encoding_revision: "plain-v1".into(),
    }
}
#[tokio::test]
async fn http_embeddings_reorders_indexed_vectors_and_rejects_partial_data_without_retry() {
    let server = MockServer::start().await;
    let client = HttpEmbedder::local(&format!("{}/v1/", server.uri()), identity()).unwrap();
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"model":"test-model","data":[{"index":1,"embedding":[0.,1.]},{"index":0,"embedding":[1.,0.]}]}))).expect(1).mount(&server).await;
    assert_eq!(
        client.embed(&inputs()).await.unwrap(),
        [vec![1., 0.], vec![0., 1.]]
    );
    let received = server.received_requests().await.unwrap();
    assert!(received[0].headers.get("authorization").is_none());
    let body: serde_json::Value = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(body["input"], json!(["猫 cat", "cat"]));
    for body in [
        json!({"model":"test-model","data":[{"index":0,"embedding":[1.,0.]}]}),
        json!({"model":"test-model","data":[{"index":0,"embedding":[1.,0.]},{"index":0,"embedding":[1.,0.]}]}),
        json!({"model":"wrong","data":[]}),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        assert!(client.embed(&inputs()).await.is_err());
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
    server.reset().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&server)
        .await;
    assert!(client.embed(&inputs()).await.is_err());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
#[tokio::test]
async fn lexical_baseline_is_explicit_deterministic_and_has_a_distinct_identity() {
    let encoder = LexicalEmbedder::new();
    assert_eq!(encoder.model_identity().provider, "local-baseline");
    assert_eq!(
        encoder.embed(&inputs()).await.unwrap(),
        encoder.embed(&inputs()).await.unwrap()
    );
    assert!(HttpEmbedder::local("https://example.com/v1/", identity()).is_err());
    assert!(HttpEmbedder::local("http://secret@127.0.0.1/v1/", identity()).is_err());
}
