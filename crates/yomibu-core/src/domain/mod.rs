#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::print_stdout,
    clippy::print_stderr
)]

/// UUID identifying a learner (`learners.id` in the retained schema).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnerId(pub uuid::Uuid);

/// UUID identifying a collection (`collections.id` in the retained schema).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectionId(pub uuid::Uuid);
