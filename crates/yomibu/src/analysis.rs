//! Bounded offline analysis of supplied text, never exercise acceptance.

use std::{borrow::Cow, ops::Range};

use serde::Serialize;

pub(crate) const MAX_SENTENCE_UNICODE_SCALARS: usize = 100;

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

/// A dictionary hypothesis. Positions refer to the original UTF-8 input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Token {
    pub span: Range<usize>,
    pub dictionary_form: String,
    pub reading: String,
    pub part_of_speech: Vec<String>,
    pub out_of_vocabulary: bool,
}

impl Token {
    pub(crate) fn is_function_word_or_punctuation(&self) -> bool {
        self.part_of_speech
            .first()
            .is_some_and(|pos| ["助詞", "助動詞", "補助記号"].contains(&pos.as_str()))
    }
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
    /// Absent for the legacy, fully verified owned snapshot policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dictionary_loading: Option<ManagedDictionaryProvenance>,
}

/// The checksum above identifies the expected pin. Managed loading relies on
/// installation-time verification and unchanged files, not a startup full hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedDictionaryProvenance {
    pub generation: String,
    pub storage: &'static str,
    pub verification: &'static str,
    pub startup_checks: &'static str,
    pub file_stability: &'static str,
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
        if characters > MAX_SENTENCE_UNICODE_SCALARS {
            return Err(SentenceError::TooLong { characters });
        }
        Ok(Self(Cow::Borrowed(text)))
    }

    pub fn text(&self) -> &str {
        &self.0
    }
}

impl SentenceAnalysis<'_> {
    pub(crate) fn into_owned(self) -> SentenceAnalysis<'static> {
        SentenceAnalysis {
            sentence: Sentence(Cow::Owned(self.sentence.text().to_owned())),
            units: self.units,
            provenance: self.provenance,
        }
    }
}
