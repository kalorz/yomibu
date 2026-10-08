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

pub(crate) fn select_configured<'a>(
    settings: &crate::configuration::SelectionSettings,
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: Option<&EmbeddingCache>,
    limit: usize,
    seed: u64,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    use crate::configuration::SelectionStep;
    let input = SelectionInput {
        inventory,
        request,
        limit,
    };
    let seeded = SeededOrdering { seed };
    if let Some(cache) = cache {
        let ranking = EmbeddingRanking::prepare(input, cache, &cache.model)?;
        return if settings
            .embedding_steps
            .contains(&SelectionStep::SeededOrder)
        {
            select_target_first(
                input,
                &[&ranking, &seeded],
                "inventory-similarity-seeded-v1",
            )
        } else {
            select_target_first(input, &[&ranking], "inventory-similarity-v1")
        };
    }
    if settings.steps.contains(&SelectionStep::LexicalTopic) {
        select_builtin_vocabulary(inventory, request, limit, seed)
    } else {
        select_target_first(input, &[&seeded], "seeded-only-v1")
    }
}
