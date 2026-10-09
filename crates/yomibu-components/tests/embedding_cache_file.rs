use yomibu_components::{
    file_embedding_cache::{EmbeddingCacheFile, EmbeddingCacheFileError},
    lexical_embeddings::LexicalEmbedder,
};
use yomibu_core::{
    capabilities::Embedder,
    domain::embedding::{EmbeddingCache, EmbeddingInput, EmbeddingPurpose},
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

#[tokio::test]
async fn invalid_cache_is_rejected_before_io_and_preserves_the_usable_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.json");
    let file = EmbeddingCacheFile::new(&path);
    let encoder = LexicalEmbedder::new();
    let inputs = [EmbeddingInput {
        purpose: EmbeddingPurpose::Query,
        text: "A cat sleeping".into(),
    }];
    let valid = EmbeddingCache::from_vectors(
        encoder.model_identity().clone(),
        &inputs,
        encoder.embed(&inputs).await.unwrap(),
    )
    .unwrap();
    file.save(&valid).unwrap();
    let original = std::fs::read(&path).unwrap();
    for case in ["version", "dimensions"] {
        let mut value = serde_json::to_value(&valid).unwrap();
        match case {
            "version" => value["version"] = serde_json::json!(2),
            _ => value["entries"][0]["vector"] = serde_json::json!([1.0]),
        }
        let invalid = serde_json::from_value(value).unwrap();
        assert!(
            matches!(
                file.save(&invalid),
                Err(EmbeddingCacheFileError::InvalidCache(_))
            ),
            "{case}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(
            file.load()
                .unwrap()
                .unwrap()
                .vectors(encoder.model_identity(), &inputs)
                .is_ok()
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        let absent_parent = EmbeddingCacheFile::new(dir.path().join("missing/cache.json"));
        assert!(matches!(
            absent_parent.save(&invalid),
            Err(EmbeddingCacheFileError::InvalidCache(_))
        ));
        assert!(!dir.path().join("missing").exists());
    }
}
