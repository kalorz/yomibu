use super::candidate;
use std::io::Write;
use yomibu_app::{
    application::generate_story::progress::ProgressEvent, reports::run::StoryRunReport,
};

pub(crate) fn write_progress(out: &mut impl Write, event: &ProgressEvent) -> std::io::Result<()> {
    match event {
        ProgressEvent::Started { step } => writeln!(out, "[{step:?}] started"),
        ProgressEvent::Completed { step, elapsed_ms } => {
            writeln!(out, "[{step:?}] completed in {elapsed_ms:.1} ms")
        }
    }
}
pub(crate) fn write_run(
    out: &mut impl Write,
    report: &StoryRunReport,
    verbose: bool,
) -> anyhow::Result<()> {
    if verbose {
        writeln!(out, "{}", report.notice)?;
        candidate::write_provenance(out, report.generated.provenance())?;
    }
    for passage in report.generated.passages() {
        writeln!(out, "{}", passage.text.escape_debug())?;
    }
    Ok(())
}
