//! Focused report conversion and terminal presentation.
use anyhow::Result;
use serde::Serialize;
use std::io::Write;
use yomibu::{
    adapters::openai::FocusedRequest,
    evaluation::{EvaluationBindings, GrammarBinding, VocabularyEntry},
    generation::{
        ContextUsageReport, FocusOccurrenceReport, FocusedCandidateAssessment, GeneratedCandidates,
        GenerationProvenance,
    },
    generation_context::{
        ExcludedEntries, ExclusionReason, InclusionReason, LIMITATIONS, SELECTOR_REVISION,
        SelectedEntry, Situation, SituationDecision, SlotRole, VocabularyEntryId,
    },
    grammar::GrammarDeclarations,
};

pub(super) fn write_unavailable(
    out: &mut impl Write,
    input: &super::Permissions,
    focus: VocabularyEntryId,
    situations: &[SituationDecision],
    json: bool,
) -> Result<()> {
    let report = ContextReport::unavailable(&input.grammar, &input.bindings, focus, situations);
    write_preview(out, &report, json)
}

#[derive(Serialize)]
pub(super) struct GenerationReport<'a> {
    version: u32,
    kind: &'static str,
    notice: &'static str,
    context: ContextReport<'a>,
    generation: &'a GenerationProvenance,
    candidates: [FocusedCandidate<'a>; 2],
}
impl<'a> GenerationReport<'a> {
    pub(super) fn new(
        prepared: &'a FocusedRequest<'a>,
        generated: &'a GeneratedCandidates<'a>,
        assessments: &'a [FocusedCandidateAssessment<'a>; 2],
    ) -> Self {
        let candidates = std::array::from_fn(|i| {
            FocusedCandidate::new(i + 1, &generated.texts()[i], &assessments[i])
        });
        Self {
            version: 1,
            kind: "experimental_focused_sentence_candidates",
            notice: crate::candidate_report::NOTICE,
            context: ContextReport::ready(prepared),
            generation: generated.provenance(),
            candidates,
        }
    }
}

pub(super) fn write_generation(
    out: &mut impl Write,
    report: &GenerationReport<'_>,
    json: bool,
) -> Result<()> {
    if json {
        crate::cli_support::write_json(out, report)?;
    } else {
        writeln!(out, "{}", report.notice)?;
        write_context(out, &report.context, false)?;
        crate::candidate_report::write_provenance(out, report.generation)?;
        writeln!(
            out,
            "Spans are half-open UTF-8 byte ranges in the original candidate."
        )?;
        for candidate in &report.candidates {
            crate::candidate_report::write_candidate(out, &candidate.candidate)?;
            write_focus_occurrence(out, candidate.focus_occurrence)?;
            writeln!(out, "Context usage: {:?}", candidate.context_usage.status)?;
            for unit in &candidate.context_usage.units {
                writeln!(
                    out,
                    "  bytes {}..{}: {} — {:?}; entries {:?}; uncertainty {:?}",
                    unit.token.span.start,
                    unit.token.span.end,
                    unit.token.dictionary_form.escape_debug(),
                    unit.status,
                    unit.entries,
                    unit.reason
                )?;
            }
        }
    }
    Ok(())
}

fn write_focus_occurrence(out: &mut dyn Write, report: &FocusOccurrenceReport) -> Result<()> {
    writeln!(
        out,
        "Focus occurrence: {:?}; compatible occurrences: {}",
        report.status, report.count
    )?;
    writeln!(
        out,
        "Focus assessment completeness: {:?}",
        report.completeness
    )?;
    for occurrence in &report.occurrences {
        writeln!(
            out,
            "  bytes {}..{}: {:?}; reading {}",
            occurrence.span.start,
            occurrence.span.end,
            occurrence.evidence,
            occurrence.reading.escape_debug()
        )?;
    }
    for uncertainty in &report.uncertainties {
        writeln!(
            out,
            "  bytes {}..{}: {:?}",
            uncertainty.span.start, uncertainty.span.end, uncertainty.reason
        )?;
    }
    Ok(())
}

#[derive(Serialize)]
struct FocusedCandidate<'a> {
    #[serde(flatten)]
    candidate: crate::candidate_report::CandidateReport<'a>,
    focus_occurrence: &'a FocusOccurrenceReport,
    context_usage: &'a ContextUsageReport,
}
impl<'a> FocusedCandidate<'a> {
    fn new(index: usize, text: &'a str, focused: &'a FocusedCandidateAssessment<'a>) -> Self {
        Self {
            candidate: crate::candidate_report::CandidateReport::new(
                index,
                text,
                &focused.assessment,
            ),
            focus_occurrence: &focused.focus_occurrence,
            context_usage: &focused.context_usage,
        }
    }
}

#[derive(Serialize)]
struct RequestReport<'a> {
    method: &'static str,
    url: &'static str,
    body_utf8: &'a str,
    bytes: usize,
    sha256: &'a str,
}
#[derive(Serialize)]
pub(super) struct ContextReport<'a> {
    version: u32,
    kind: &'static str,
    status: &'static str,
    selector_revision: &'static str,
    focus_entry: usize,
    situation: Option<&'a Situation>,
    selected: &'a [SelectedEntry<'a>],
    grammar: Vec<&'a str>,
    grammar_bindings: &'a [GrammarBinding],
    excluded: &'a [ExcludedEntries],
    situations: &'a [SituationDecision],
    limitations: [&'static str; 5],
    request: Option<RequestReport<'a>>,
    #[serde(skip)]
    permissions: &'a EvaluationBindings,
}
impl<'a> ContextReport<'a> {
    fn unavailable(
        grammar: &'a GrammarDeclarations,
        permissions: &'a EvaluationBindings,
        focus: VocabularyEntryId,
        situations: &'a [SituationDecision],
    ) -> Self {
        Self {
            version: 1,
            kind: "focused_context_preview",
            status: "unavailable",
            selector_revision: SELECTOR_REVISION,
            focus_entry: focus.get(),
            situation: None,
            selected: &[],
            grammar: grammar
                .entries()
                .iter()
                .map(|g| g.description.as_str())
                .collect(),
            grammar_bindings: &permissions.grammar,
            excluded: &[],
            situations,
            limitations: LIMITATIONS,
            request: None,
            permissions,
        }
    }
    pub(super) fn ready(prepared: &'a FocusedRequest<'a>) -> Self {
        let c = prepared.context();
        Self {
            status: "ready",
            situation: Some(c.situation()),
            selected: c.selected(),
            excluded: c.excluded(),
            request: Some(RequestReport {
                method: prepared.method(),
                url: prepared.url(),
                body_utf8: prepared.body_utf8(),
                bytes: prepared.bytes(),
                sha256: prepared.sha256(),
            }),
            ..Self::unavailable(
                c.grammar(),
                c.permissions(),
                c.focus_entry(),
                c.situations(),
            )
        }
    }
}
pub(super) fn write_preview(
    out: &mut impl Write,
    report: &ContextReport<'_>,
    json: bool,
) -> Result<()> {
    if json {
        crate::cli_support::write_json(out, report)?;
    } else {
        writeln!(
            out,
            "Experimental focused context — no generation performed"
        )?;
        write_context(out, report, true)?;
        writeln!(out, "Requests made: 0")?;
    }
    Ok(())
}
fn role_label(role: SlotRole) -> &'static str {
    match role {
        SlotRole::Predicate => "predicate",
        SlotRole::Participant => "participant",
        SlotRole::Object => "object",
    }
}
fn write_context(
    out: &mut impl Write,
    report: &ContextReport<'_>,
    show_excluded_forms: bool,
) -> Result<()> {
    match report.permissions.vocabulary.get(report.focus_entry - 1) {
        Some(VocabularyEntry {
            written_form,
            reading,
            sense,
            ..
        }) => writeln!(
            out,
            "Focus #{}: {} / {} / {}",
            report.focus_entry,
            written_form.escape_debug(),
            reading.escape_debug(),
            sense.escape_debug()
        )?,
        None => writeln!(out, "Focus #{}: not found", report.focus_entry)?,
    }
    writeln!(out, "Selector revision: {}", report.selector_revision)?;
    if let Some(s) = report.situation {
        writeln!(out, "Situation: {} — {}", s.id, s.description)?;
    }
    for selected in report.selected {
        let reason = match selected.reason {
            InclusionReason::ExplicitFocus => {
                format!("explicit focus; {}", role_label(selected.role))
            }
            InclusionReason::FirstAvailableDeclaredAlternative => format!(
                "required {}; first available declared alternative",
                role_label(selected.role)
            ),
        };
        writeln!(
            out,
            "Selected #{}: {} — {}",
            selected.entry.get(),
            selected.vocabulary.written_form.escape_debug(),
            reason
        )?;
        writeln!(
            out,
            "  Reading: {}; sense: {}; direct_object: {}",
            selected.vocabulary.reading.escape_debug(),
            selected.vocabulary.sense.escape_debug(),
            selected.vocabulary.direct_object
        )?;
    }
    for group in report.excluded {
        let reason = match group.reason {
            ExclusionReason::DuplicateOfSelectedTuple => "duplicate of a selected tuple",
            ExclusionReason::AmbiguousAssociation => "ambiguous association",
            ExclusionReason::UnusedSlotAlternative => "participant alternative not needed",
            ExclusionReason::UnsupportedTupleVariant => {
                "unsupported tuple variant for an associated spelling"
            }
            ExclusionReason::OutsideChosenSituation => "outside the chosen situation",
        };
        if !show_excluded_forms {
            writeln!(out, "Excluded entries {:?}: {}", group.entries, reason)?;
            continue;
        }
        for entry in &group.entries {
            writeln!(
                out,
                "Excluded #{}: {} — {}",
                entry,
                report.permissions.vocabulary[*entry - 1]
                    .written_form
                    .escape_debug(),
                reason
            )?;
        }
    }
    if let Some(s) = report.situation {
        let required = s
            .required_grammar
            .iter()
            .map(|rule| {
                let ids = report
                    .grammar_bindings
                    .iter()
                    .filter(|b| b.rule == *rule)
                    .map(|b| format!("#{}", b.declaration_id))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{rule:?} ({ids})")
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(out, "Required grammar: {required}")?;
    }
    writeln!(
        out,
        "All retained grammar (including additional declarations and bindings):"
    )?;
    for (i, description) in report.grammar.iter().enumerate() {
        writeln!(out, "  Grammar #{}: {}", i + 1, description.escape_debug())?;
    }
    for b in report.grammar_bindings {
        writeln!(out, "  Binding #{}: {:?}", b.declaration_id, b.rule)?;
    }
    for decision in report.situations {
        writeln!(
            out,
            "Situation decision {}: {}",
            decision.id,
            if decision.feasible {
                "declared prerequisites available"
            } else {
                "unavailable"
            }
        )?;
        for reason in &decision.reasons {
            writeln!(out, "  {reason:?}")?;
        }
    }
    writeln!(
        out,
        "Feasible: {}",
        if report.status == "ready" {
            "declared selection prerequisites are available"
        } else {
            "no situation has all declared prerequisites"
        }
    )?;
    writeln!(
        out,
        "Contextual reading/sense and situation quality: not assessed"
    )?;
    writeln!(
        out,
        "Associations: original synthetic suggestions; comprehension and naturalness: not assessed; not accepted exercises"
    )?;
    if let Some(request) = &report.request {
        writeln!(
            out,
            "Exact outbound request (decode body_utf8 to recover the request bytes):"
        )?;
        crate::cli_support::write_json(out, request)?;
    } else {
        writeln!(out, "Request: unavailable")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::Input;
    use super::*;
    use yomibu::{adapters::openai::prepare_focused_request, generation_context::select_context};
    #[test]
    fn focus_text_boundary_separates_presence_and_completeness_and_escapes_readings() {
        use yomibu::generation::{
            FocusCompleteness, FocusOccurrence, FocusStatus, LexicalUncertainty,
            LexicalUncertaintyReason, OccurrenceEvidence,
        };
        let report = FocusOccurrenceReport {
            status: FocusStatus::Observed,
            completeness: FocusCompleteness::Partial,
            count: 1,
            occurrences: vec![FocusOccurrence {
                span: 7..10,
                reading: "ネ\n\u{1b}".into(),
                evidence: OccurrenceEvidence::RegularStem,
            }],
            uncertainties: vec![LexicalUncertainty {
                span: 0..4,
                reason: LexicalUncertaintyReason::OutOfVocabulary,
            }],
            contextual_reading_and_sense: "not_assessed",
        };
        let mut output = Vec::new();
        write_focus_occurrence(&mut output, &report).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            concat!(
                "Focus occurrence: Observed; compatible occurrences: 1\n",
                "Focus assessment completeness: Partial\n",
                "  bytes 7..10: RegularStem; reading ネ\\n\\u{1b}\n",
                "  bytes 0..4: OutOfVocabulary\n"
            )
        );
    }
    #[test]
    fn generation_text_does_not_expand_the_unselected_inventory() {
        let mut input: Input =
            serde_json::from_str(include_str!("../../tests/fixtures/focused/pet-rest.json"))
                .unwrap();
        input.bindings.vocabulary[3].written_form = "private-unselected-label".into();
        let grammar = GrammarDeclarations::from_descriptions(input.grammar).unwrap();
        let request = prepare_focused_request(
            select_context(
                &grammar,
                &input.bindings,
                VocabularyEntryId::new(1).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let report = ContextReport::ready(&request);
        let mut output = Vec::new();
        write_context(&mut output, &report, false).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(!text.contains("private-unselected-label"));
        assert!(text.contains("Excluded entries [4]: outside the chosen situation"));
    }
}
