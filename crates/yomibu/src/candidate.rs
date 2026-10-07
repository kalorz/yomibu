//! Candidate assessments, execution errors and provider metadata.
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
    pub requested_model: &'static str,
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

#[derive(Debug)]
pub enum CandidateAssessment<'a> {
    Completed {
        analysis: SentenceAnalysis<'a>,
        evaluation: Box<Evaluation>,
    },
    ExecutionError {
        analysis: Option<SentenceAnalysis<'a>>,
        error: CandidateError,
    },
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
