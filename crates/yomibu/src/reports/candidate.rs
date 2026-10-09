//! Structured candidate reports without terminal formatting or execution.
use crate::application::story::DefaultCandidateError;
use serde::Serialize;
use yomibu_components::sudachi_dictionary::AnalysisError;
use yomibu_core::domain::{
    analysis::{SentenceAnalysis, SentenceError},
    candidate::{CandidateAssessment, CandidateError},
    evaluation::{CheckKind, CheckState, Evaluation, EvaluationError},
};

pub const NOTICE: &str = "Experimental sentence candidates — not accepted exercises";

#[derive(Serialize)]
pub struct CandidateReport<'a> {
    pub index: usize,
    pub text: &'a str,
    pub analysis: Option<&'a SentenceAnalysis<'a>>,
    pub assessment: AssessmentReport<'a>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AssessmentReport<'a> {
    NotRun,
    Completed {
        outcome: CheckState,
        evaluation: &'a Evaluation,
    },
    ExecutionError {
        stage: &'static str,
        code: &'static str,
        message: String,
        checks: [UnrunCheck; 5],
    },
}

#[derive(Serialize)]
pub struct UnrunCheck {
    pub kind: CheckKind,
    pub state: CheckState,
}

impl<'a> CandidateReport<'a> {
    pub fn new(
        index: usize,
        text: &'a str,
        assessment: &'a CandidateAssessment<'a, DefaultCandidateError>,
    ) -> Self {
        let (analysis, assessment) = match assessment {
            CandidateAssessment::NotRun => (None, AssessmentReport::NotRun),
            CandidateAssessment::Completed {
                analysis,
                evaluation,
            } => (
                Some(analysis),
                AssessmentReport::Completed {
                    outcome: evaluation.outcome(),
                    evaluation,
                },
            ),
            CandidateAssessment::ExecutionError { analysis, error } => {
                let (stage, code) = match &error.0 {
                    CandidateError::InvalidSentenceSpan => ("analysis", "invalid_analysis_span"),
                    CandidateError::MismatchedAnalysis => ("analysis", "mismatched_analysis"),
                    CandidateError::MismatchedAssessment => ("evaluation", "mismatched_assessment"),
                    CandidateError::Sentence(SentenceError::Blank) => {
                        ("sentence", "blank_sentence")
                    }
                    CandidateError::Sentence(SentenceError::TooLong { .. }) => {
                        ("sentence", "sentence_too_long")
                    }
                    CandidateError::Analysis(AnalysisError::InvalidSpan) => {
                        ("analysis", "invalid_analysis_span")
                    }
                    CandidateError::Analysis(AnalysisError::Analyzer(_)) => {
                        ("analysis", "analysis_failed")
                    }
                    CandidateError::Assessment(EvaluationError::BlankVocabulary) => {
                        ("evaluation", "blank_vocabulary")
                    }
                    CandidateError::Assessment(EvaluationError::MissingDeclaration) => {
                        ("evaluation", "missing_declaration")
                    }
                    CandidateError::Assessment(EvaluationError::InvalidAnalysis) => {
                        ("evaluation", "invalid_analysis")
                    }
                    CandidateError::Assessment(EvaluationError::InvalidFindingSpan) => {
                        ("evaluation", "invalid_finding_span")
                    }
                    CandidateError::Assessment(EvaluationError::InvalidCheckState) => {
                        ("evaluation", "invalid_check_state")
                    }
                };
                (
                    analysis.as_ref(),
                    AssessmentReport::ExecutionError {
                        stage,
                        code,
                        message: error.to_string(),
                        checks: [
                            CheckKind::Vocabulary,
                            CheckKind::Inflection,
                            CheckKind::Particles,
                            CheckKind::Nominal,
                            CheckKind::Scope,
                        ]
                        .map(|kind| UnrunCheck {
                            kind,
                            state: CheckState::NotRun,
                        }),
                    },
                )
            }
        };
        Self {
            index,
            text,
            analysis,
            assessment,
        }
    }
}

pub use yomibu_core::domain::candidate::GenerationProvenance;
