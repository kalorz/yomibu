//! Structured candidate reports without terminal formatting or execution.
use serde::Serialize;
use yomibu_components::{
    japanese_constraint_checks::CandidateError, sudachi_dictionary::AnalysisError,
};
use yomibu_core::domain::{
    analysis::{SentenceAnalysis, SentenceError},
    candidate::CandidateAssessment,
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
        assessment: &'a CandidateAssessment<'a, CandidateError>,
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
                let (stage, code) = match error {
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
                    CandidateError::Evaluation(EvaluationError::BlankVocabulary) => {
                        ("evaluation", "blank_vocabulary")
                    }
                    CandidateError::Evaluation(EvaluationError::MissingDeclaration) => {
                        ("evaluation", "missing_declaration")
                    }
                    CandidateError::Evaluation(EvaluationError::InvalidAnalysis) => {
                        ("evaluation", "invalid_analysis")
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
