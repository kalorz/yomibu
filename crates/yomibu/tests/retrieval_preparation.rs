use std::sync::Mutex;
use yomibu::{
    adapters::embeddings::LexicalEmbedder,
    ports::Embedder,
    retrieval::{
        EmbeddingCache, EmbeddingError, EmbeddingInput, EmbeddingModelIdentity, EmbeddingPurpose,
        prepare_cache,
    },
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
