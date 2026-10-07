//! Terminal display of dictionary loading provenance.
use std::io::Write;
use yomibu::adapters::dictionary::Verification;
use yomibu::analysis::AnalysisProvenance;

pub(crate) fn write_import(
    out: &mut impl Write,
    generation: &str,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        return super::write_json(out, &serde_json::json!({"generation": generation}));
    }
    writeln!(
        out,
        "Verified dictionary installed: {}",
        generation.escape_debug()
    )?;
    writeln!(
        out,
        "Managed files must remain unchanged; reimport publishes a new generation."
    )?;
    Ok(())
}

pub(crate) fn write_verified(
    out: &mut impl Write,
    verification: &Verification,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        return super::write_json(
            out,
            &serde_json::json!({
                "verified": true,
                "metadata_matches_installation": verification.metadata_matches_installation,
            }),
        );
    }
    writeln!(
        out,
        "Full pinned verification passed for the dictionary and both publisher notices."
    )?;
    if verification.metadata_matches_installation {
        writeln!(
            out,
            "Recorded dictionary metadata matches installation evidence."
        )?;
    } else {
        writeln!(
            out,
            "Dictionary metadata changed since installation; startup will reject this generation."
        )?;
        writeln!(
            out,
            "Reimport with yomibu dictionary import --bundle PATH --dictionary-dir PATH."
        )?;
    }
    Ok(())
}

pub(crate) fn write_loading(
    out: &mut impl Write,
    provenance: &AnalysisProvenance,
) -> std::io::Result<()> {
    let loading = &provenance.dictionary_loading;
    writeln!(
        out,
        "Dictionary loading: memory mapped; full SHA-256 verified at installation; startup checks records, file metadata and header."
    )?;
    writeln!(
        out,
        "Dictionary stability: requires unchanged managed files for analyzer lifetime."
    )?;
    writeln!(
        out,
        "Dictionary generation: {}",
        loading.generation.escape_debug()
    )?;
    writeln!(
        out,
        "Expected dictionary SHA-256: {}",
        provenance.dictionary_sha256
    )?;
    Ok(())
}
