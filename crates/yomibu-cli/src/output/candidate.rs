//! Terminal rendering of shared candidate report data.
use std::io::Write;
use yomibu::reports::candidate::GenerationProvenance;
pub(crate) fn write_provenance(
    out: &mut impl Write,
    provenance: &GenerationProvenance,
) -> std::io::Result<()> {
    writeln!(out, "Provider: {}", provenance.provider)?;
    writeln!(
        out,
        "Requested model: {}",
        provenance.requested_model.escape_debug()
    )?;
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
