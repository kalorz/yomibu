pub const COMPONENT: yomibu_core::capabilities::options::Component =
    yomibu_core::capabilities::options::Component {
        id: "embedding-vocabulary-selection",
        settings: &[],
    };

use std::collections::BTreeMap;
use yomibu_core::{
    capabilities::SelectionStep,
    domain::{
        embedding::{EmbeddingCache, EmbeddingError, EmbeddingModelIdentity, cosine},
        inventory::LearnerInventory,
        story::{SelectionCandidates, SelectionError, SelectionInput, StoryError, StoryRequest},
    },
};

pub struct EmbeddingRanking<'e> {
    inventory: &'e LearnerInventory,
    request: &'e StoryRequest,
    vectors: BTreeMap<&'e str, &'e [f32]>,
    query: &'e [f32],
    model: &'e EmbeddingModelIdentity,
}

impl<'e> EmbeddingRanking<'e> {
    /// Borrow complete compatible vectors without I/O; requires a topic and valid input.
    /// Applying to different inventory or request references returns `EvidenceMismatch`.
    pub fn prepare(
        input: SelectionInput<'e>,
        cache: &'e EmbeddingCache,
        model: &'e EmbeddingModelIdentity,
    ) -> Result<Self, StoryError> {
        if input.request.topic.is_none() {
            return Err(StoryError::Invalid("semantic selection requires a topic"));
        }
        let inputs = prepare_embedding_inputs(input.inventory, input.request)?;
        input.request.validate_selection_limit(input.limit)?;
        let mut vectors = cache.vectors(model, &inputs)?;
        let query = vectors.pop().ok_or(EmbeddingError::Missing)?;
        Ok(Self {
            inventory: input.inventory,
            request: input.request,
            vectors: input
                .inventory
                .vocabulary
                .iter()
                .zip(vectors)
                .map(|(word, vector)| (word.id.as_str(), vector))
                .collect(),
            query,
            model,
        })
    }
}

impl SelectionStep for EmbeddingRanking<'_> {
    fn apply<'a>(
        &self,
        input: SelectionInput<'a>,
        mut candidates: SelectionCandidates<'a>,
    ) -> Result<SelectionCandidates<'a>, StoryError> {
        if !std::ptr::eq(input.inventory, self.inventory)
            || !std::ptr::eq(input.request, self.request)
        {
            return Err(SelectionError::EvidenceMismatch.into());
        }
        for entry in &mut candidates.entries {
            let vector = self
                .vectors
                .get(entry.word.id.as_str())
                .ok_or(EmbeddingError::Missing)?;
            entry.score = Some(cosine(vector, self.query));
            entry.reason = "topic_similarity";
        }
        candidates.entries.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.word.id.cmp(&b.word.id))
        });
        candidates.embedding_model = Some(self.model.clone());
        Ok(candidates)
    }
}

mod inputs;
pub use inputs::prepare_embedding_inputs;
