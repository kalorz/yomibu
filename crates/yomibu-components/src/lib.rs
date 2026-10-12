//! Concrete providers, storage and bounded Japanese components.
pub mod file_learning_store;
pub mod learner_vocabulary_selection;
pub mod openai_story_generation;
pub mod story_prompt_preparation;
pub mod wanikani_source;

pub const COMPONENTS: &[yomibu_core::component::Component] = &[
    file_learning_store::COMPONENT,
    learner_vocabulary_selection::COMPONENT,
    openai_story_generation::COMPONENT,
    story_prompt_preparation::COMPONENT,
    wanikani_source::COMPONENT,
];

pub fn settings() -> impl Iterator<Item = yomibu_core::component::options::Setting> {
    COMPONENTS
        .iter()
        .flat_map(|component| component.settings)
        .copied()
}

#[cfg(test)]
extern crate self as yomibu_components;
