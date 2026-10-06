//! Shared executable presentation for experimental candidate assessments.
use serde::Serialize;
use std::io::Write;
use yomibu::{
    adapters::sudachi::AnalysisError,
    analysis::{SentenceAnalysis, SentenceError},
    evaluation::{CheckKind, CheckState, Evaluation, EvaluationError},
    generation::{CandidateAssessment, CandidateError, GenerationProvenance},
};

pub(super) const NOTICE: &str = "Experimental sentence candidates — not accepted exercises";

#[derive(Serialize)]
pub(super) struct CandidateReport<'a> {
    index: usize,
    text: &'a str,
    analysis: Option<&'a SentenceAnalysis<'a>>,
    assessment: AssessmentReport<'a>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum AssessmentReport<'a> {
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
struct UnrunCheck {
    kind: CheckKind,
    state: CheckState,
}

impl<'a> CandidateReport<'a> {
    pub(super) fn new(
        index: usize,
        text: &'a str,
        assessment: &'a CandidateAssessment<'a>,
    ) -> Self {
        let (analysis, assessment) = match assessment {
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

pub(super) fn write_provenance(
    out: &mut impl Write,
    provenance: &GenerationProvenance,
) -> std::io::Result<()> {
    writeln!(out, "Provider: {}", provenance.provider)?;
    writeln!(out, "Requested model: {}", provenance.requested_model)?;
    writeln!(
        out,
        "Provider-reported model: {}",
        provenance.returned_model.escape_debug()
    )?;
    writeln!(
        out,
        "Requested processing tier: {}",
        provenance.requested_tier
    )?;
    writeln!(
        out,
        "Provider-reported processing tier: {}",
        provenance
            .returned_tier
            .as_deref()
            .unwrap_or("not reported")
            .escape_debug()
    )?;
    writeln!(out, "Prompt revision: {}", provenance.prompt_revision)?;
    writeln!(
        out,
        "Request SHA-256: {}; bytes: {}; requests: {}",
        provenance.request_sha256, provenance.request_bytes, provenance.request_count
    )?;
    writeln!(
        out,
        "Response ID: {}",
        provenance.response_id.escape_debug()
    )?;
    writeln!(
        out,
        "HTTP request ID: {}",
        provenance
            .request_id
            .as_deref()
            .unwrap_or("not reported")
            .escape_debug()
    )?;
    match &provenance.usage {
        Some(usage) => writeln!(
            out,
            "Provider-reported tokens: input {}; output {}; total {}",
            usage.input_tokens, usage.output_tokens, usage.total_tokens
        )?,
        None => writeln!(out, "Provider-reported tokens: not reported")?,
    }
    Ok(())
}

pub(super) fn write_candidate(
    out: &mut impl Write,
    candidate: &CandidateReport<'_>,
) -> std::io::Result<()> {
    writeln!(
        out,
        "Candidate {}: \"{}\"",
        candidate.index,
        candidate.text.escape_debug()
    )?;
    if let Some(analysis) = candidate.analysis {
        let provenance = &analysis.provenance;
        writeln!(out, "Analyzer revision: {}", provenance.analyzer_revision)?;
        writeln!(out, "Dictionary: {}", provenance.dictionary_version)?;
        if provenance.dictionary_loading.is_none() {
            writeln!(out, "Dictionary SHA-256: {}", provenance.dictionary_sha256)?;
        }
        super::dictionary::write_loading(out, provenance)?;
        writeln!(
            out,
            "Configuration SHA-256: {}",
            provenance.configuration_sha256
        )?;
    }
    match &candidate.assessment {
        AssessmentReport::Completed {
            outcome,
            evaluation,
        } => {
            writeln!(
                out,
                "Overall: {} (completed)",
                super::cli_support::state_label(*outcome)
            )?;
            super::cli_support::write_checks(out, candidate.text, evaluation)?;
        }
        AssessmentReport::ExecutionError {
            stage,
            code,
            message,
            checks,
        } => {
            writeln!(
                out,
                "Execution error ({stage}/{code}): {}",
                message.escape_debug()
            )?;
            for check in checks {
                writeln!(out, "{:?}: NotRun", check.kind)?;
            }
            writeln!(
                out,
                "Naturalness: not assessed\nMultiword expressions: not assessed\nContextual reading and sense: not assessed"
            )?;
        }
    }
    Ok(())
}
