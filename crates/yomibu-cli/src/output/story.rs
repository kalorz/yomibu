use super::candidate;
use std::io::Write;
use yomibu::{
    application::progress::ProgressEvent,
    reports::run::{StoryRunReport, Warning},
};

pub(crate) fn write_progress(out: &mut impl Write, event: &ProgressEvent) -> std::io::Result<()> {
    match event {
        ProgressEvent::Started { step } => writeln!(out, "[{step:?}] started"),
        ProgressEvent::Completed { step, elapsed_ms } => {
            writeln!(out, "[{step:?}] completed in {elapsed_ms:.1} ms")
        }
        ProgressEvent::Skipped { step, reason } => {
            writeln!(out, "[{step:?}] skipped: {}", reason.escape_debug())
        }
    }
}
pub(crate) fn write_run(
    out: &mut impl Write,
    err: &mut impl Write,
    report: &StoryRunReport,
    verbose: bool,
) -> anyhow::Result<()> {
    write_warnings(err, &report.warnings)?;
    if verbose {
        writeln!(out, "{}", report.notice)?;
        candidate::write_provenance(out, report.generated.provenance())?;
    }
    for passage in report.generated.passages() {
        writeln!(out, "{}", passage.text.escape_debug())?;
    }
    Ok(())
}
fn write_warnings(err: &mut impl Write, warnings: &[Warning]) -> std::io::Result<()> {
    for warning in warnings {
        writeln!(
            err,
            "warning: {:?}: {}",
            warning.step,
            warning.message.escape_debug()
        )?;
    }
    Ok(())
}
