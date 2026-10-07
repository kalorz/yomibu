//! Generated texts, provider metadata, assessments and execution errors.
use crate::{
    adapters::sudachi::AnalysisError,
    analysis::{SentenceAnalysis, SentenceError},
    evaluation::{Evaluation, EvaluationError},
};
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
    pub(crate) passages: Vec<GeneratedPassage>,
    pub(crate) provenance: GenerationProvenance,
}
impl GeneratedCandidates {
    pub fn passages(&self) -> &[GeneratedPassage] {
        &self.passages
    }
    pub fn provenance(&self) -> &GenerationProvenance {
        &self.provenance
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CandidateAssessment<'a> {
    NotRun,
    Completed {
        analysis: SentenceAnalysis<'a>,
        evaluation: Box<Evaluation>,
    },
    ExecutionError {
        analysis: Option<SentenceAnalysis<'a>>,
        #[serde(serialize_with = "serialize_candidate_error")]
        error: CandidateError,
    },
}

fn serialize_candidate_error<S: serde::Serializer>(
    error: &CandidateError,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&error.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum CandidateError {
    #[error(transparent)]
    Sentence(SentenceError),
    // Retain the typed cause for callers without reflecting dependency text in reports.
    #[error("Pinned Sudachi analysis failed.")]
    Analysis(#[source] AnalysisError),
    #[error(transparent)]
    Evaluation(EvaluationError),
}
