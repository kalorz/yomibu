//! Terminal presentation of shared analysis report data.
use super::{state_label, write_checks};
use std::io::Write;
use yomibu::app::local::AnalysisRunReport;

pub(crate) fn write_text(out: &mut impl Write, report: &AnalysisRunReport) -> std::io::Result<()> {
    writeln!(out, "{}", report.evaluation.notice)?;
    writeln!(out, "Overall: {} (completed)", state_label(report.outcome))?;
    let original = report.analysis.sentence.text();
    writeln!(out, "Sentence: \"{}\"", original.escape_debug())?;
    super::dictionary::write_loading(out, &report.analysis.provenance)?;
    for (index, description) in report.input.grammar.iter().enumerate() {
        writeln!(out, "Grammar {}: {}", index + 1, description.escape_debug())?;
    }
    writeln!(
        out,
        "Spans are half-open UTF-8 byte ranges in the original sentence."
    )?;
    write_checks(out, original, &report.evaluation)
}
