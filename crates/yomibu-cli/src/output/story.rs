use super::{candidate, write_json};
use std::io::Write;
use yomibu::{
    app::{
        local::{ProgressEvent, StoryPreviewRunReport, StoryRunReport},
        modules::ModuleState,
    },
    retrieval::EmbeddingCache,
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
    json: bool,
    verbose: bool,
) -> anyhow::Result<()> {
    for warning in &report.warnings {
        writeln!(
            err,
            "warning: {}: {}",
            warning.module.metadata().name,
            warning.message.escape_debug()
        )?;
    }
    if verbose {
        let states: &mut dyn Write = if json { err } else { out };
        for module in &report.modules {
            let state = match &module.state {
                ModuleState::Disabled => "Disabled".into(),
                ModuleState::NotConfigured => "Missing config".into(),
                ModuleState::Available => "Available".into(),
                ModuleState::Skipped { reason } => format!("Skipped: {}", reason.escape_debug()),
                ModuleState::Unavailable { error } => {
                    format!("Unavailable: {}", error.escape_debug())
                }
            };
            writeln!(
                states,
                "{}: {state}{}",
                module.metadata.name,
                if module.required { " (required)" } else { "" }
            )?;
        }
        writeln!(states, "{}", report.notice)?;
    }
    if json {
        write_json(out, report)?;
    } else {
        if verbose {
            candidate::write_provenance(out, report.generated.provenance())?;
        }
        for passage in report.generated.passages() {
            writeln!(out, "{}", passage.text.escape_debug())?;
        }
    }
    Ok(())
}
pub(crate) fn write_preview(
    out: &mut impl Write,
    report: &StoryPreviewRunReport,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        write_json(out, report)?;
    } else {
        writeln!(
            out,
            "Experimental story generation plan — no generation performed"
        )?;
        if let Some(topic) = &report.request.topic {
            writeln!(out, "Topic: {}", topic.text().escape_debug())?;
        }
        writeln!(
            out,
            "Selector revision: {}",
            report.selection.selector_revision
        )?;
        for id in &report.selection.vocabulary_ids {
            writeln!(out, "Selected: {}", id.escape_debug())?;
        }
        writeln!(out, "Exact outbound request:")?;
        write_json(out, &report.provider_request)?;
        writeln!(out, "Generation requests made: 0")?;
    }
    Ok(())
}
pub(crate) fn write_retrieval(out: &mut impl Write, cache: &EmbeddingCache) -> std::io::Result<()> {
    writeln!(
        out,
        "Retrieval prepared: {} vectors; model {} / {}. No generation performed.",
        cache.entries.len(),
        cache.model.provider.escape_debug(),
        cache.model.model.escape_debug()
    )
}
