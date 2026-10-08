use std::sync::Mutex;
use yomibu_components::lexical_embeddings::LexicalEmbedder;
use yomibu_core::{
    capabilities::Embedder,
    domain::embedding::{
        EmbeddingCache, EmbeddingError, EmbeddingInput, EmbeddingModelIdentity, EmbeddingPurpose,
    },
    pipeline::embeddings::prepare_cache,
};
struct RecordingEmbedder {
    inner: LexicalEmbedder,
    calls: Mutex<Vec<usize>>,
    fail_on: Option<usize>,
}
impl RecordingEmbedder {
    fn new(fail_on: Option<usize>) -> Self {
        Self {
            inner: LexicalEmbedder::new(),
            calls: Mutex::new(Vec::new()),
            fail_on,
        }
    }
}
impl Embedder for RecordingEmbedder {
    fn model_identity(&self) -> &EmbeddingModelIdentity {
        self.inner.model_identity()
    }
    async fn embed(&self, inputs: &[EmbeddingInput]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let call = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(inputs.len());
            calls.len()
        };
        if self.fail_on == Some(call) {
            return Err(EmbeddingError::Transport);
        }
        self.inner.embed(inputs).await
    }
}
fn inputs() -> Vec<EmbeddingInput> {
    (0..70)
        .map(|i| EmbeddingInput {
            purpose: EmbeddingPurpose::Document,
            text: format!("lexical word {i}"),
        })
        .collect()
}
#[tokio::test]
async fn prepares_only_missing_vectors_in_bounded_batches_and_reuses_complete_cache() {
    let inputs = inputs();
    let encoder = RecordingEmbedder::new(None);
    let first = EmbeddingCache::from_vectors(
        encoder.model_identity().clone(),
        &inputs[..32],
        encoder.inner.embed(&inputs[..32]).await.unwrap(),
    )
    .unwrap();
    let result = prepare_cache(&encoder, &inputs, Some(&first))
        .await
        .unwrap();
    assert_eq!(*encoder.calls.lock().unwrap(), [32, 6]);
    assert_eq!(
        result.vectors(encoder.model_identity(), &inputs).unwrap(),
        encoder
            .inner
            .embed(&inputs)
            .await
            .unwrap()
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>()
    );
    let reused = prepare_cache(&encoder, &inputs, Some(&result))
        .await
        .unwrap();
    assert_eq!(*encoder.calls.lock().unwrap(), [32, 6]);
    assert_eq!(
        serde_json::to_value(reused).unwrap(),
        serde_json::to_value(&result).unwrap()
    );
    let mut other = result;
    other.model.revision = "different".into();
    prepare_cache(&encoder, &inputs, Some(&other))
        .await
        .unwrap();
    assert_eq!(*encoder.calls.lock().unwrap(), [32, 6, 32, 32, 6]);
}
#[tokio::test]
async fn failed_later_batch_returns_no_partial_cache_and_preserves_previous_data() {
    let inputs = inputs();
    let encoder = RecordingEmbedder::new(Some(2));
    let previous = EmbeddingCache::from_vectors(
        encoder.model_identity().clone(),
        &inputs[..1],
        encoder.inner.embed(&inputs[..1]).await.unwrap(),
    )
    .unwrap();
    let before = serde_json::to_value(&previous).unwrap();
    assert!(matches!(
        prepare_cache(&encoder, &inputs, Some(&previous)).await,
        Err(EmbeddingError::Transport)
    ));
    assert_eq!(*encoder.calls.lock().unwrap(), [32, 32]);
    assert_eq!(serde_json::to_value(&previous).unwrap(), before);
}

#[tokio::test]
async fn oversized_later_input_fails_before_any_http_call_and_exact_limit_is_accepted() {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};
    use yomibu_components::http_embeddings::HttpEmbedder;
    use yomibu_core::domain::embedding::MAX_EMBEDDING_INPUT_BYTES;
    let server = MockServer::start().await;
    Mock::given(path("/v1/embeddings"))
        .respond_with(|request: &wiremock::Request| {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            let rows: Vec<_> = body["input"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(index, _)| serde_json::json!({"index":index, "embedding":[1.0, 0.0]}))
                .collect();
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"model":"boundary-model", "data":rows}))
        })
        .mount(&server)
        .await;
    let encoder = HttpEmbedder::local(
        &format!("{}/v1/", server.uri()),
        EmbeddingModelIdentity {
            provider: "local".into(),
            model: "boundary-model".into(),
            revision: "v1".into(),
            dimensions: 2,
            encoding_revision: "plain-v1".into(),
        },
    )
    .unwrap();
    let mut inputs: Vec<_> = inputs().into_iter().take(33).collect();
    inputs[32].text = "猫".repeat(MAX_EMBEDDING_INPUT_BYTES / 3 + 1);
    assert!(matches!(
        prepare_cache(&encoder, &inputs, None).await,
        Err(EmbeddingError::Invalid(_))
    ));
    let calls = server.received_requests().await.unwrap();
    assert!(
        calls.is_empty(),
        "made {} provider calls before rejecting a later input",
        calls.len()
    );
    inputs[32].text = "a".repeat(MAX_EMBEDDING_INPUT_BYTES);
    let cache = prepare_cache(&encoder, &inputs, None).await.unwrap();
    assert_eq!(
        cache
            .vectors(encoder.model_identity(), &inputs)
            .unwrap()
            .len(),
        33
    );
    let batches: Vec<_> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| {
            serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["input"]
                .as_array()
                .unwrap()
                .len()
        })
        .collect();
    assert_eq!(batches, [32, 1]);
}
