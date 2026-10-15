/// A learner's UUID identity, matching `learners.id` in the retained schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearnerId(pub uuid::Uuid);

/// A collection's UUID identity, matching `collections.id` in the retained schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionId(pub uuid::Uuid);
