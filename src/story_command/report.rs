//! Story presentation consumes completed plans and assessments, never runs them.
use crate::{candidate_report, cli_support};
use anyhow::Result;
use serde::Serialize;
use std::io::Write;
use yomibu::story::{
    AiModelRequest, PlanDeparture, StoryCandidateAssessment, StoryCandidates, StoryGenerationPlan,
    StoryRequest, TargetObservation,
};
#[derive(Serialize)]
struct RequestBytes<'a> {
    body_utf8: &'a str,
    bytes: usize,
    sha256: &'a str,
    prompt_revision: &'static str,
}
#[derive(Serialize)]
pub(super) struct Preview<'a> {
    version: u32,
    kind: &'static str,
    request: &'a StoryRequest,
    plan: &'a yomibu::story::StoryGenerationPlan<'a>,
    generation_options: yomibu::story::StoryGenerationOptions,
    provider_request: RequestBytes<'a>,
}
pub(super) fn preview<'a>(
    request: &'a StoryRequest,
    plan: &'a StoryGenerationPlan<'a>,
    ai_request: &'a AiModelRequest,
) -> Preview<'a> {
    Preview {
        version: 1,
        kind: "story_generation_plan_preview",
        request,
        plan,
        generation_options: ai_request.options(),
        provider_request: RequestBytes {
            body_utf8: ai_request.body_utf8(),
            bytes: ai_request.body_utf8().len(),
            sha256: ai_request.sha256(),
            prompt_revision: yomibu::story::STORY_PROMPT_REVISION,
        },
    }
}
pub(super) fn write_preview(
    out: &mut impl Write,
    request: &StoryRequest,
    plan: &StoryGenerationPlan<'_>,
    ai_request: &AiModelRequest,
    json: bool,
) -> Result<()> {
    let report = preview(request, plan, ai_request);
    if json {
        cli_support::write_json(out, &report)?;
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
fn write_plan(out: &mut impl Write, report: &Preview<'_>) -> Result<()> {
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
    cli_support::write_json(out, &report.provider_request)?;
    Ok(())
}
#[derive(Serialize)]
struct Candidate<'a> {
    #[serde(flatten)]
    candidate: candidate_report::CandidateReport<'a>,
    targets: &'a [TargetObservation],
    plan_departures: &'a [PlanDeparture],
}
#[derive(Serialize)]
struct Report<'a> {
    version: u32,
    kind: &'static str,
    notice: &'static str,
    plan: Preview<'a>,
    generation: &'a yomibu::generation::GenerationProvenance,
    candidates: Vec<Candidate<'a>>,
}
pub(super) fn write_generation(
    out: &mut impl Write,
    preview: Preview<'_>,
    generated: &StoryCandidates,
    assessments: &[StoryCandidateAssessment<'_>],
    json: bool,
) -> Result<()> {
    let report = Report {
        version: 1,
        kind: "experimental_story_candidates",
        notice: candidate_report::NOTICE,
        plan: preview,
        generation: generated.provenance(),
        candidates: assessments
            .iter()
            .enumerate()
            .map(|(i, assessment)| Candidate {
                candidate: candidate_report::CandidateReport::new(
                    i + 1,
                    &generated.texts()[i],
                    &assessment.assessment,
                ),
                targets: &assessment.targets,
                plan_departures: &assessment.plan_departures,
            })
            .collect(),
    };
    if json {
        cli_support::write_json(out, &report)?;
    } else {
        writeln!(out, "{}", report.notice)?;
        write_plan(out, &report.plan)?;
        candidate_report::write_provenance(out, report.generation)?;
        writeln!(
            out,
            "Spans are half-open UTF-8 byte ranges in the original candidate."
        )?;
        for c in &report.candidates {
            candidate_report::write_candidate(out, &c.candidate)?;
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
