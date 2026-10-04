//! Bounded offline analysis of supplied text, never exercise acceptance.

use std::ops::Range;

use serde::Serialize;

/// An unchanged, nonblank input of at most 100 Unicode scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Sentence<'a>(&'a str);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SentenceError {
    #[error("Sentence must not be blank.")]
    Blank,
    #[error("Sentence has {characters} Unicode characters; A1 permits at most 100.")]
    TooLong { characters: usize },
}

/// A dictionary hypothesis. Positions refer to the original UTF-8 input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Token {
    pub span: Range<usize>,
    pub dictionary_form: String,
    pub reading: String,
    pub part_of_speech: Vec<String>,
    pub out_of_vocabulary: bool,
}

/// C-mode whole unit and its A-mode components, without substituting permissions.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct LexicalUnit {
    pub token: Token,
    pub components: Vec<Token>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AnalysisProvenance {
    pub analyzer_revision: &'static str,
    pub dictionary_version: &'static str,
    pub dictionary_sha256: &'static str,
    pub configuration_sha256: String,
}

/// Morphological evidence only; tokens and readings are not linguistic truth.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SentenceAnalysis<'a> {
    pub sentence: Sentence<'a>,
    pub units: Vec<LexicalUnit>,
    pub provenance: AnalysisProvenance,
}

impl<'a> Sentence<'a> {
    pub fn new(text: &'a str) -> Result<Self, SentenceError> {
        if text.trim().is_empty() {
            return Err(SentenceError::Blank);
        }
        let characters = text.chars().count();
        if characters > 100 {
            return Err(SentenceError::TooLong { characters });
        }
        Ok(Self(text))
    }

    pub fn text(self) -> &'a str {
        self.0
    }
}
