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

/// A learner's name, with surrounding whitespace removed and at least one
/// non-whitespace character. Names do not establish learner identity or uniqueness.
#[derive(Debug, PartialEq, Eq)]
pub struct LearnerName(String);

impl LearnerName {
    /// Normalizes a supplied name, returning `None` if it is blank.
    pub fn new(name: &str) -> Option<Self> {
        let name = name.trim();
        (!name.is_empty()).then(|| Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// UUID identifying a learner progress source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnerProgressSourceId(pub uuid::Uuid);

/// Material kind; a subject may have no kind before/outside entry resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialKind {
    Vocabulary,
    Grammar,
}

#[cfg(test)]
mod tests {
    use super::LearnerName;

    #[test]
    fn learner_name_trims_only_surrounding_whitespace() {
        for input in ["Alice  Smith", " \tAlice  Smith\n\u{3000}"] {
            assert_eq!(LearnerName::new(input).unwrap().as_str(), "Alice  Smith");
        }
        assert_eq!(LearnerName::new("  アリス  ").unwrap().as_str(), "アリス");
    }

    #[test]
    fn learner_name_rejects_blank_input() {
        for input in ["", " ", "\t\r\n\u{3000}"] {
            assert!(LearnerName::new(input).is_none());
        }
    }
}
