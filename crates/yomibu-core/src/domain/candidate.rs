//! Generated texts, provider metadata, assessments and execution errors.
use crate::domain::{analysis::SentenceAnalysis, evaluation::Evaluation};
use serde::{Deserialize, Serialize};

/// Counts reported by the provider, not an independently verified bill.
#[derive(Debug, Deserialize, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct GenerationProvenance {
    pub provider: &'static str,
    pub requested_model: String,
    pub returned_model: String,
    pub requested_tier: &'static str,
    pub returned_tier: Option<String>,
    pub prompt_revision: &'static str,
    pub request_sha256: String,
    pub request_bytes: usize,
    pub response_id: String,
    pub request_id: Option<String>,
    pub request_count: u8,
    pub usage: Option<TokenUsage>,
}

/// Original generated texts and metadata, independent of request/input lifetimes.
#[derive(Debug, Serialize)]
pub struct GeneratedPassage {
    pub text: String,
    pub sentence_spans: Vec<std::ops::Range<usize>>,
}

#[derive(Debug, Serialize)]
pub struct GeneratedCandidates {
    passages: Vec<GeneratedPassage>,
    provenance: GenerationProvenance,
}
#[derive(Debug, thiserror::Error)]
pub enum CandidateConstructionError {
    #[error("Generated candidates must contain at least one passage.")]
    Empty,
    #[error("Sentence spans must cover each original passage with valid UTF-8 boundaries.")]
    InvalidSpans,
    #[error(transparent)]
    Sentence(#[from] crate::domain::analysis::SentenceError),
}

impl GeneratedCandidates {
    pub fn new(
        passages: Vec<GeneratedPassage>,
        provenance: GenerationProvenance,
    ) -> Result<Self, CandidateConstructionError> {
        if passages.is_empty() {
            return Err(CandidateConstructionError::Empty);
        }
        for passage in &passages {
            if passage.sentence_spans.is_empty() {
                return Err(CandidateConstructionError::InvalidSpans);
            }
            let mut end = 0;
            for span in &passage.sentence_spans {
                if span.start != end {
                    return Err(CandidateConstructionError::InvalidSpans);
                }
                let text = passage
                    .text
                    .get(span.clone())
                    .ok_or(CandidateConstructionError::InvalidSpans)?;
                crate::domain::analysis::Sentence::new(text)?;
                end = span.end;
            }
            if end != passage.text.len() {
                return Err(CandidateConstructionError::InvalidSpans);
            }
        }
        Ok(Self {
            passages,
            provenance,
        })
    }

    pub fn passages(&self) -> &[GeneratedPassage] {
        &self.passages
    }
    pub fn provenance(&self) -> &GenerationProvenance {
        &self.provenance
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[serde(bound(serialize = "E: std::fmt::Display"))]
pub enum CandidateAssessment<'a, E> {
    NotRun,
    Completed {
        analysis: SentenceAnalysis<'a>,
        evaluation: Box<Evaluation>,
    },
    ExecutionError {
        analysis: Option<SentenceAnalysis<'a>>,
        #[serde(serialize_with = "serialize_candidate_error")]
        error: E,
    },
}

fn serialize_candidate_error<E: std::fmt::Display, S: serde::Serializer>(
    error: &E,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&error.to_string())
}
