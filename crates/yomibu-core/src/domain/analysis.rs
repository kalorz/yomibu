//! Bounded sentence text.

use std::borrow::Cow;

use serde::Serialize;

pub const MAX_SENTENCE_UNICODE_SCALARS: usize = 100;

/// An unchanged, nonblank input of at most 100 Unicode scalar values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sentence<'a>(Cow<'a, str>);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SentenceError {
    #[error("Sentence must not be blank.")]
    Blank,
    #[error("Sentence has {characters} Unicode characters; limit is 100.")]
    TooLong { characters: usize },
}

impl<'a> Sentence<'a> {
    pub fn new(text: &'a str) -> Result<Self, SentenceError> {
        if text.trim().is_empty() {
            return Err(SentenceError::Blank);
        }
        let characters = text.chars().count();
        if characters > MAX_SENTENCE_UNICODE_SCALARS {
            return Err(SentenceError::TooLong { characters });
        }
        Ok(Self(Cow::Borrowed(text)))
    }

    pub fn text(&self) -> &str {
        &self.0
    }
}
