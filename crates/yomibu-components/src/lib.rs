//! Concrete providers, storage and bounded Japanese components.
pub mod embedding_vocabulary_selection;
pub mod file_embedding_cache;
pub mod file_learning_store;
pub mod http_embeddings;
pub mod in_memory_learning_store;
pub mod japanese_constraint_checks;
pub mod learner_vocabulary_selection;
pub mod lexical_embeddings;
pub mod openai_story_generation;
pub mod story_prompt_preparation;
pub mod sudachi_dictionary;
pub mod wanikani_source;

pub const COMPONENTS: &[yomibu_core::capabilities::options::Component] = &[
    embedding_vocabulary_selection::COMPONENT,
    file_embedding_cache::COMPONENT,
    file_learning_store::COMPONENT,
    http_embeddings::COMPONENT,
    in_memory_learning_store::COMPONENT,
    japanese_constraint_checks::COMPONENT,
    learner_vocabulary_selection::COMPONENT,
    lexical_embeddings::COMPONENT,
    openai_story_generation::COMPONENT,
    story_prompt_preparation::COMPONENT,
    sudachi_dictionary::COMPONENT,
    wanikani_source::COMPONENT,
];

pub fn settings() -> impl Iterator<Item = yomibu_core::capabilities::options::Setting> {
    COMPONENTS
        .iter()
        .flat_map(|component| component.settings)
        .copied()
}

#[cfg(test)]
extern crate self as yomibu_components;
#[cfg(test)]
#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;
