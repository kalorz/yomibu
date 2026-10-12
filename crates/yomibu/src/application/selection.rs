use yomibu_components::learner_vocabulary_selection::{LexicalTopicScoring, SeededOrdering};
use yomibu_core::{
    domain::{
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

pub(crate) fn select_configured<'a>(
    settings: &crate::configuration::SelectionSettings,
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
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
    if settings.steps.contains(&SelectionStep::LexicalTopic) {
        select_builtin_vocabulary(inventory, request, limit, seed)
    } else {
        select_target_first(input, &[&seeded], "seeded-only-v1")
    }
}
