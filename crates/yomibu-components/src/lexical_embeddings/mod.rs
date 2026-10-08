use sha2::{Digest, Sha256};
use yomibu_core::{
    capabilities::Embedder,
    domain::embedding::{EmbeddingError, EmbeddingInput, EmbeddingModelIdentity, validate_vector},
};

/// A comparison baseline, not a semantic model and never an automatic fallback.
pub struct LexicalEmbedder {
    model: EmbeddingModelIdentity,
}
impl Default for LexicalEmbedder {
    fn default() -> Self {
        Self::new()
    }
}
impl LexicalEmbedder {
    pub fn new() -> Self {
        Self {
            model: EmbeddingModelIdentity {
                provider: "local-baseline".into(),
                model: "lexical-hash".into(),
                revision: "1".into(),
                dimensions: 512,
                encoding_revision: "tokens-v1".into(),
            },
        }
    }
}
impl Embedder for LexicalEmbedder {
    fn model_identity(&self) -> &EmbeddingModelIdentity {
        &self.model
    }
    async fn embed(&self, inputs: &[EmbeddingInput]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        inputs
            .iter()
            .map(|input| {
                let mut v = vec![0.; self.model.dimensions];
                for word in input
                    .text
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|s| !s.is_empty())
                {
                    let lower = word.to_lowercase();
                    let hash = Sha256::digest(lower.as_bytes());
                    let slot = usize::from(u16::from_be_bytes([hash[0], hash[1]])) % v.len();
                    v[slot] += 1.;
                }
                validate_vector(&v, self.model.dimensions)?;
                Ok(v)
            })
            .collect()
    }
}
