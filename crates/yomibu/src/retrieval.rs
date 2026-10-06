//! Explicit embedding inputs and a model-specific local vector cache.
use crate::{
    inventory::LearnerInventory,
    ports::Embedder,
    story::{StoryError, StoryRequest},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const MAX_EMBEDDING_INPUT_BYTES: usize = 32768;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingModelIdentity {
    pub provider: String,
    pub model: String,
    pub revision: String,
    pub dimensions: usize,
    pub encoding_revision: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingPurpose {
    Document,
    Query,
}
#[derive(Debug, Clone)]
pub struct EmbeddingInput {
    pub purpose: EmbeddingPurpose,
    pub text: String,
}
impl EmbeddingInput {
    pub fn key(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(match self.purpose {
            EmbeddingPurpose::Document => b"document\0".as_slice(),
            EmbeddingPurpose::Query => b"query\0".as_slice(),
        });
        hash.update(self.text.as_bytes());
        format!("{:x}", hash.finalize())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Invalid embedding data: {0}.")]
    Invalid(&'static str),
    #[error("Embedding cache is missing or stale; run prepare-retrieval explicitly.")]
    Missing,
    #[error("Embedding request failed; no vectors were cached.")]
    Transport,
    #[error("Embedding provider returned HTTP {0}; no vectors were cached.")]
    Http(u16),
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedVector {
    pub key: String,
    pub vector: Vec<f32>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddingCache {
    pub version: u32,
    pub model: EmbeddingModelIdentity,
    pub entries: Vec<CachedVector>,
}
impl EmbeddingCache {
    pub fn from_vectors(
        model: EmbeddingModelIdentity,
        inputs: &[EmbeddingInput],
        vectors: Vec<Vec<f32>>,
    ) -> Result<Self, EmbeddingError> {
        if inputs.len() != vectors.len() {
            return Err(EmbeddingError::Invalid("wrong vector count"));
        }
        for vector in &vectors {
            validate_vector(vector, model.dimensions)?;
        }
        // Identical lexical documents intentionally share one cached vector.
        let mut keys = BTreeSet::new();
        let entries = inputs
            .iter()
            .zip(vectors)
            .filter_map(|(input, vector)| {
                let key = input.key();
                keys.insert(key.clone())
                    .then_some(CachedVector { key, vector })
            })
            .collect();
        let cache = Self {
            version: 1,
            model,
            entries,
        };
        cache.validate()?;
        Ok(cache)
    }
    pub fn validate(&self) -> Result<(), EmbeddingError> {
        if self.version != 1
            || self.model.dimensions == 0
            || self.model.dimensions > 4096
            || self.entries.len() > 20_000
            || [
                &self.model.provider,
                &self.model.model,
                &self.model.revision,
                &self.model.encoding_revision,
            ]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 256)
        {
            return Err(EmbeddingError::Invalid("model identity or cache bounds"));
        }
        let mut keys = BTreeSet::new();
        for entry in &self.entries {
            if entry.key.len() != 64
                || !entry.key.bytes().all(|c| c.is_ascii_hexdigit())
                || !keys.insert(&entry.key)
            {
                return Err(EmbeddingError::Invalid("duplicate or malformed cache key"));
            }
            validate_vector(&entry.vector, self.model.dimensions)?;
        }
        Ok(())
    }
    pub fn vectors(
        &self,
        model: &EmbeddingModelIdentity,
        inputs: &[EmbeddingInput],
    ) -> Result<Vec<&[f32]>, EmbeddingError> {
        self.validate()?;
        if &self.model != model {
            return Err(EmbeddingError::Missing);
        }
        let by_key: std::collections::BTreeMap<_, _> = self
            .entries
            .iter()
            .map(|e| (e.key.as_str(), e.vector.as_slice()))
            .collect();
        inputs
            .iter()
            .map(|input| {
                by_key
                    .get(input.key().as_str())
                    .copied()
                    .ok_or(EmbeddingError::Missing)
            })
            .collect()
    }
}
pub fn validate_vector(vector: &[f32], dimensions: usize) -> Result<(), EmbeddingError> {
    if vector.len() != dimensions
        || vector.iter().any(|v| !v.is_finite())
        || !vector.iter().any(|v| *v != 0.)
    {
        return Err(EmbeddingError::Invalid(
            "wrong dimensions, nonfinite or zero vector",
        ));
    }
    Ok(())
}
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
    dot / (norm(a) * norm(b))
}
pub fn prepare_embedding_inputs(
    inventory: &LearnerInventory,
    request: &StoryRequest,
) -> Result<Vec<EmbeddingInput>, StoryError> {
    request.validate(inventory)?;
    let mut inputs: Vec<_> = inventory
        .vocabulary
        .iter()
        .map(|word| EmbeddingInput {
            purpose: EmbeddingPurpose::Document,
            text: format!(
                "Word: {}\nReadings: {}\nMeanings: {}",
                word.written_form,
                word.readings.join(" / "),
                word.meanings.join(" / ")
            ),
        })
        .collect();
    inputs.push(EmbeddingInput {
        purpose: EmbeddingPurpose::Query,
        text: request.brief.clone(),
    });
    validate_embedding_input_sizes(&inputs)?;
    Ok(inputs)
}

fn validate_embedding_input_sizes(inputs: &[EmbeddingInput]) -> Result<(), EmbeddingError> {
    if inputs
        .iter()
        .any(|input| input.text.len() > MAX_EMBEDDING_INPUT_BYTES)
    {
        return Err(EmbeddingError::Invalid(
            "combined embedding document exceeds the 32768-byte input limit",
        ));
    }
    Ok(())
}

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
