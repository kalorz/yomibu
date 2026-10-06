//! Terminal display of dictionary loading provenance.
use std::io::Write;
use yomibu::analysis::AnalysisProvenance;

pub(crate) fn write_loading(
    out: &mut impl Write,
    provenance: &AnalysisProvenance,
) -> std::io::Result<()> {
    if let Some(loading) = &provenance.dictionary_loading {
        writeln!(
            out,
            "Dictionary loading: memory mapped; full SHA-256 verified at installation; startup checks records, size and header."
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
    }
    Ok(())
}
