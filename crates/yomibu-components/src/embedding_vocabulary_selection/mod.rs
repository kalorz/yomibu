use yomibu_core::{
    domain::{
        embedding::{EmbeddingCache, EmbeddingError, EmbeddingModelIdentity, cosine},
        inventory::LearnerInventory,
        story::{StoryError, StoryRequest, StoryVocabularySelection},
    },
    pipeline::selection::select_ranked,
};

pub fn select_vocabulary<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: &EmbeddingCache,
    model: &EmbeddingModelIdentity,
    limit: usize,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    if request.topic.is_none() {
        return Err(StoryError::Invalid("semantic selection requires a topic"));
    }
    let inputs = prepare_embedding_inputs(inventory, request)?;
    request.validate_selection_limit(limit)?;
    let vectors = cache.vectors(model, &inputs)?;
    let query = vectors.last().ok_or(EmbeddingError::Missing)?;
    let mut ranked: Vec<_> = inventory
        .vocabulary
        .iter()
        .zip(&vectors)
        .map(|(word, vector)| (word, Some(cosine(vector, query))))
        .collect();
    ranked.sort_by(|(a, sa), (b, sb)| {
        sb.partial_cmp(sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    select_ranked(
        request,
        ranked,
        limit,
        |_| "topic_similarity",
        "inventory-similarity-v1",
        Some(model.clone()),
    )
}

mod inputs;
pub use inputs::prepare_embedding_inputs;
