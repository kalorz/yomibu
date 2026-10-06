//! Terminal rendering of shared candidate report data.
use std::io::Write;
use yomibu::{
    generation::GenerationProvenance,
    reports::candidate::{AssessmentReport, CandidateReport},
};
pub(crate) fn write_provenance(
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

pub(crate) fn write_candidate(
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
            writeln!(out, "Overall: {} (completed)", super::state_label(*outcome))?;
            super::write_checks(out, candidate.text, evaluation)?;
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
