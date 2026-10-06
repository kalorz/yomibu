use yomibu::{
    adapters::{
        embedding_cache_file::{EmbeddingCacheFile, EmbeddingCacheFileError},
        embeddings::LexicalEmbedder,
    },
    ports::Embedder,
    retrieval::{EmbeddingCache, EmbeddingInput, EmbeddingPurpose},
};
#[tokio::test]
async fn explicit_file_cache_roundtrips_without_implicit_directory_creation() {
    let dir = tempfile::tempdir().unwrap();
    let file = EmbeddingCacheFile::new(dir.path().join("cache.json"));
    assert!(file.load().unwrap().is_none());
    let encoder = LexicalEmbedder::new();
    let inputs = [EmbeddingInput {
        purpose: EmbeddingPurpose::Query,
        text: "A cat sleeping".into(),
    }];
    let cache = EmbeddingCache::from_vectors(
        encoder.model_identity().clone(),
        &inputs,
        encoder.embed(&inputs).await.unwrap(),
    )
    .unwrap();
    file.save(&cache).unwrap();
    assert_eq!(
        serde_json::to_value(file.load().unwrap().unwrap()).unwrap(),
        serde_json::to_value(&cache).unwrap()
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let missing_parent = dir.path().join("missing");
    assert!(
        EmbeddingCacheFile::new(missing_parent.join("cache.json"))
            .save(&cache)
            .is_err()
    );
    assert!(!missing_parent.exists());
}
#[test]
fn malformed_cache_is_an_error_and_original_file_survives() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.json");
    std::fs::write(&path, "{").unwrap();
    assert!(matches!(
        EmbeddingCacheFile::new(&path).load(),
        Err(EmbeddingCacheFileError::InvalidJson(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{");
}
