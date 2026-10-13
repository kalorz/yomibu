//! Concrete providers, storage and bounded Japanese components.
pub mod file_store;
pub mod learner_vocabulary_selection;
pub mod macos_keychain;
pub mod openai;
pub mod wanikani;

pub const COMPONENTS: &[yomibu_core::extension::Component] = &[
    file_store::COMPONENT,
    learner_vocabulary_selection::COMPONENT,
    openai::COMPONENT,
    openai::preparation::COMPONENT,
    wanikani::COMPONENT,
];

pub fn settings() -> impl Iterator<Item = yomibu_core::extension::options::Setting> {
    COMPONENTS
        .iter()
        .flat_map(|component| component.settings)
        .copied()
}

#[cfg(test)]
extern crate self as yomibu_ext;
