use crate::sudachi_dictionary::AnalysisError;
use yomibu_core::domain::{analysis::SentenceError, evaluation::EvaluationError};

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
