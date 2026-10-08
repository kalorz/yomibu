use yomibu_components::{
    embedding_vocabulary_selection::EmbeddingRanking,
    learner_vocabulary_selection::{LexicalTopicScoring, SeededOrdering},
};
use yomibu_core::{
    domain::{
        embedding::{EmbeddingCache, EmbeddingModelIdentity},
        inventory::LearnerInventory,
        story::{SelectionInput, StoryError, StoryRequest, StoryVocabularySelection},
    },
    pipeline::selection::select_target_first,
};

pub fn select_builtin_vocabulary<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    limit: usize,
    seed: u64,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    select_target_first(
        SelectionInput {
            inventory,
            request,
            limit,
        },
        &[&LexicalTopicScoring, &SeededOrdering { seed }],
        "builtin-v2",
    )
}

pub fn select_vocabulary<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: &EmbeddingCache,
    model: &EmbeddingModelIdentity,
    limit: usize,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    let input = SelectionInput {
        inventory,
        request,
        limit,
    };
    let ranking = EmbeddingRanking::prepare(input, cache, model)?;
    select_target_first(input, &[&ranking], "inventory-similarity-v1")
}
