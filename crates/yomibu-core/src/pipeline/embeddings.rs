use crate::{
    capabilities::Embedder,
    domain::embedding::{
        CachedVector, EmbeddingCache, EmbeddingError, EmbeddingInput,
        validate_embedding_input_sizes,
    },
};

/// Reuse compatible cached vectors and encode missing inputs in bounded batches.
/// All input sizes are checked before encoder work. Failure returns no partial
/// cache; publication remains the caller's explicit step.
pub async fn prepare_cache(
    embedder: &impl Embedder,
    inputs: &[EmbeddingInput],
    previous: Option<&EmbeddingCache>,
) -> Result<EmbeddingCache, EmbeddingError> {
    validate_embedding_input_sizes(inputs)?;
    let model = embedder.model_identity();
    let previous = previous.filter(|c| &c.model == model);
    let mut entries: std::collections::BTreeMap<String, Vec<f32>> = previous
        .map(|c| {
            c.entries
                .iter()
                .map(|e| (e.key.clone(), e.vector.clone()))
                .collect()
        })
        .unwrap_or_default();
    let missing: Vec<_> = inputs
        .iter()
        .filter(|i| !entries.contains_key(&i.key()))
        .cloned()
        .collect();
    for chunk in missing.chunks(32) {
        let vectors = embedder.embed(chunk).await?;
        let batch = EmbeddingCache::from_vectors(model.clone(), chunk, vectors)?;
        for entry in batch.entries {
            entries.insert(entry.key, entry.vector);
        }
    }
    let result = EmbeddingCache {
        version: 1,
        model: model.clone(),
        entries: entries
            .into_iter()
            .map(|(key, vector)| CachedVector { key, vector })
            .collect(),
    };
    result.vectors(model, inputs)?;
    Ok(result)
}
