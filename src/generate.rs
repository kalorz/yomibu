//! Executable-only explicit input, credentials, runtime and experimental reports.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};
use yomibu::{
    adapters::{
        openai::{Client, ProviderError},
        sudachi::AnalysisError,
    },
    analysis::{SentenceAnalysis, SentenceError},
    evaluation::{CheckKind, CheckState, Evaluation, EvaluationBindings, EvaluationError},
    generation::{CandidateAssessment, CandidateError, GenerationProvenance},
    grammar::GrammarDeclarations,
};

pub(super) const NOTICE: &str = "Experimental sentence candidates — not accepted exercises";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: u32,
    grammar: Vec<String>,
    bindings: EvaluationBindings,
}

pub(super) fn run(
    dictionary: &super::dictionary::DictionaryArgs,
    input_path: &Path,
    json: bool,
    make_client: impl FnOnce(&str) -> Result<Client, ProviderError>,
    out: &mut impl Write,
) -> Result<()> {
    let input = load_input(input_path)?;
    let grammar = GrammarDeclarations::from_descriptions(input.grammar.iter().map(String::as_str))
        .context("Grammar input")?;
    input
        .bindings
        .validate(&grammar)
        .context("Evaluation bindings")?;
    let analyzer = dictionary.load().context("Dictionary initialization")?;
    let key = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Set OPENAI_API_KEY in the environment before explicitly requesting candidates."
            )
        })?;
    let client = make_client(&key)?;
    drop(key);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Generation runtime")?;
    let generated = runtime.block_on(client.generate_candidates(&grammar, &input.bindings))?;
    let assessments = generated.assess(&analyzer);
    let candidates = std::array::from_fn(|index| {
        CandidateReport::new(index + 1, &generated.texts()[index], &assessments[index])
    });
    let report = Report {
        version: 1,
        kind: "experimental_sentence_candidates",
        notice: NOTICE,
        input: &input,
        generation: generated.provenance(),
        candidates,
    };
    if json {
        super::cli_support::write_json(out, &report)?;
    } else {
        write_text(out, &report)?;
    }
    if assessments
        .iter()
        .any(|assessment| matches!(assessment, CandidateAssessment::ExecutionError { .. }))
    {
        bail!("One or both candidate executions failed; see the experimental report.");
    }
    Ok(())
}

fn load_input(path: &Path) -> Result<Input> {
    let bytes = super::cli_support::read_input(path, "Generation")?;
    let input: Input = serde_json::from_slice(&bytes).context("Invalid generation input JSON")?;
    if input.version != 1 {
        bail!(
            "Unsupported generation input version {}; expected 1.",
            input.version
        );
    }
    Ok(input)
}

#[derive(Serialize)]
struct Report<'a> {
    version: u32,
    kind: &'static str,
    notice: &'static str,
    input: &'a Input,
    generation: &'a GenerationProvenance,
    candidates: [CandidateReport<'a>; 2],
}

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

fn write_text(out: &mut impl Write, report: &Report<'_>) -> std::io::Result<()> {
    writeln!(out, "{}", report.notice)?;
    write_provenance(out, report.generation)?;
    for (index, description) in report.input.grammar.iter().enumerate() {
        writeln!(out, "Grammar {}: {}", index + 1, description.escape_debug())?;
    }
    writeln!(
        out,
        "Spans are half-open UTF-8 byte ranges in the original candidate."
    )?;
    for candidate in &report.candidates {
        write_candidate(out, candidate)?;
    }
    Ok(())
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
