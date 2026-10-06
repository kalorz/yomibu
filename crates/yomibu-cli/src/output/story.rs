//! CLI story output consumes shared reports, never runs assessment.
use super::{candidate, write_json};
use anyhow::Result;
use std::io::Write;
use yomibu::{
    reports::story::{StoryPreviewReport, StoryReport},
    story::{StoryGenerationPlan, StoryGenerationResult, StoryRequest},
};
pub(crate) fn write_preview(
    out: &mut impl Write,
    request: &StoryRequest,
    plan: &StoryGenerationPlan<'_>,
    json: bool,
) -> Result<()> {
    let report = StoryPreviewReport::new(request, plan);
    if json {
        write_json(out, &report)?;
    } else {
        writeln!(
            out,
            "Experimental story generation plan — no generation performed"
        )?;
        write_plan(out, &report)?;
        writeln!(out, "Generation requests made: 0")?;
    }
    Ok(())
}
fn write_plan(out: &mut impl Write, report: &StoryPreviewReport<'_>) -> Result<()> {
    writeln!(out, "Brief: {}", report.request.brief.escape_debug())?;
    writeln!(
        out,
        "Requested candidates: {}",
        report.generation_options.candidate_count
    )?;
    writeln!(out, "Selector revision: {}", report.plan.selector_revision)?;
    writeln!(
        out,
        "Embedding model: {} / {} / {}",
        report.plan.embedding_model.provider.escape_debug(),
        report.plan.embedding_model.model.escape_debug(),
        report.plan.embedding_model.revision.escape_debug()
    )?;
    for s in &report.plan.selected {
        writeln!(
            out,
            "Selected {}: {} — {}; similarity {:.4}",
            s.word.id.escape_debug(),
            s.word.written_form.escape_debug(),
            s.reason,
            s.similarity
        )?;
    }
    for id in &report.request.targets.grammar {
        writeln!(out, "Grammar target: {}", id.escape_debug())?;
    }
    writeln!(
        out,
        "Topic adherence, naturalness and contextual reading/sense: not assessed"
    )?;
    writeln!(
        out,
        "Exact outbound request (decode body_utf8 to recover the request bytes):"
    )?;
    write_json(out, &report.provider_request)?;
    Ok(())
}
pub(crate) fn write_generation(
    out: &mut impl Write,
    request: &StoryRequest,
    plan: &StoryGenerationPlan<'_>,
    result: &StoryGenerationResult,
    json: bool,
) -> Result<()> {
    let report = StoryReport::new(request, plan, result);
    if json {
        write_json(out, &report)?;
    } else {
        writeln!(out, "{}", report.notice)?;
        write_plan(out, &report.plan)?;
        candidate::write_provenance(out, report.generation)?;
        writeln!(
            out,
            "Spans are half-open UTF-8 byte ranges in the original candidate."
        )?;
        for c in &report.candidates {
            candidate::write_candidate(out, &c.candidate)?;
            for t in c.targets {
                writeln!(
                    out,
                    "Target {} {}: {}; completeness {}; spans {:?}",
                    t.kind,
                    t.id.escape_debug(),
                    t.status,
                    t.completeness,
                    t.spans
                )?;
            }
            for d in c.plan_departures {
                writeln!(
                    out,
                    "Plan departure at bytes {}..{}: {}",
                    d.span.start,
                    d.span.end,
                    d.inventory_entries
                        .iter()
                        .map(|s| s.escape_debug().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
            }
        }
    }
    Ok(())
}
