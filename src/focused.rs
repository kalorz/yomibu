//! Executable-only explicit file input and composition; no retained session.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};
use yomibu::{
    adapters::{
        openai::{Client, FocusedRequest, ProviderError, prepare_focused_request},
        sudachi::SudachiAnalyzer,
    },
    evaluation::{EvaluationBindings, GrammarBinding, VocabularyEntry},
    generation::{
        CandidateAssessment, ContextUsageReport, FocusOccurrenceReport, GenerationProvenance,
        assess_context_usage, assess_focus_occurrence,
    },
    generation_context::{
        ContextError, ExcludedEntries, ExclusionReason, GenerationContext, InclusionReason,
        LIMITATIONS, PERMISSION_FILE_LIMIT, SELECTOR_REVISION, SelectedEntry, Situation,
        SituationDecision, SlotRole, VocabularyEntryId, select_context,
    },
    grammar::GrammarDeclarations,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    version: u32,
    grammar: Vec<String>,
    bindings: EvaluationBindings,
}

pub(super) fn run(
    path: &Path,
    focus: usize,
    dictionary: Option<&Path>,
    json: bool,
    make_client: impl FnOnce(&str) -> Result<Client, ProviderError>,
    out: &mut impl Write,
) -> Result<()> {
    let bytes = super::cli_support::read_input_bounded(
        path,
        "Focused permission",
        PERMISSION_FILE_LIMIT,
        "4 MiB (4194304 bytes)",
    )?;
    let input: Input = serde_json::from_slice(&bytes).context("Invalid focused permission JSON")?;
    if input.version != 1 {
        bail!(
            "Unsupported focused permission version {}; expected 1.",
            input.version
        );
    }
    let grammar = GrammarDeclarations::from_descriptions(input.grammar).context("Grammar input")?;
    let focus = VocabularyEntryId::new(focus)?;
    let context = match select_context(&grammar, &input.bindings, focus) {
        Ok(context) => context,
        Err(ContextError::Unavailable { situations, .. }) => {
            let report = ContextReport::unavailable(&grammar, &input.bindings, focus, &situations);
            write_preview(out, &report, json)?;
            bail!("Focused context is unavailable; see the declared prerequisite decisions.");
        }
        Err(error) => return Err(error.into()),
    };
    let prepared = prepare_focused_request(context)?;
    let context_report = ContextReport::ready(&prepared);
    let Some(dictionary) = dictionary else {
        return write_preview(out, &context_report, json);
    };
    let analyzer = SudachiAnalyzer::load(dictionary).context("Dictionary initialization")?;
    let key = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Set OPENAI_API_KEY in the environment before explicitly requesting candidates."
            )
        })?;
    let client = make_client(&key)?;
    drop(key);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Generation runtime")?;
    let generated = runtime.block_on(client.generate_focused_candidates(&prepared))?;
    let assessments = generated.assess(&analyzer);
    let candidates = std::array::from_fn(|i| {
        FocusedCandidate::new(
            i + 1,
            &generated.texts()[i],
            &assessments[i],
            prepared.context(),
        )
    });
    let report = Report {
        version: 1,
        kind: "experimental_focused_sentence_candidates",
        notice: super::generate::NOTICE,
        context: context_report,
        generation: generated.provenance(),
        candidates,
    };
    if json {
        super::cli_support::write_json(out, &report)?;
    } else {
        writeln!(out, "{}", report.notice)?;
        write_context(out, &report.context, false)?;
        super::generate::write_provenance(out, report.generation)?;
        writeln!(
            out,
            "Spans are half-open UTF-8 byte ranges in the original candidate."
        )?;
        for candidate in &report.candidates {
            super::generate::write_candidate(out, &candidate.candidate)?;
            writeln!(
                out,
                "Focus occurrence: {:?}; compatible occurrences: {}",
                candidate.focus_occurrence.status, candidate.focus_occurrence.count
            )?;
            for occurrence in &candidate.focus_occurrence.occurrences {
                writeln!(
                    out,
                    "  bytes {}..{}: {:?}; reading {}",
                    occurrence.span.start,
                    occurrence.span.end,
                    occurrence.evidence,
                    occurrence.reading.escape_debug()
                )?;
            }
            for uncertainty in &candidate.focus_occurrence.uncertainties {
                writeln!(
                    out,
                    "  bytes {}..{}: {:?}",
                    uncertainty.span.start, uncertainty.span.end, uncertainty.reason
                )?;
            }
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
    if assessments
        .iter()
        .any(|a| matches!(a, CandidateAssessment::ExecutionError { .. }))
    {
        bail!("One or both candidate executions failed; see the experimental report.");
    }
    Ok(())
}

#[derive(Serialize)]
struct Report<'a> {
    version: u32,
    kind: &'static str,
    notice: &'static str,
    context: ContextReport<'a>,
    generation: &'a GenerationProvenance,
    candidates: [FocusedCandidate<'a>; 2],
}
#[derive(Serialize)]
struct FocusedCandidate<'a> {
    #[serde(flatten)]
    candidate: super::generate::CandidateReport<'a>,
    focus_occurrence: FocusOccurrenceReport,
    context_usage: ContextUsageReport,
}
impl<'a> FocusedCandidate<'a> {
    fn new(
        index: usize,
        text: &'a str,
        assessment: &'a CandidateAssessment<'a>,
        context: &GenerationContext<'_>,
    ) -> Self {
        let analysis = match assessment {
            CandidateAssessment::Completed { analysis, .. } => Some(analysis),
            CandidateAssessment::ExecutionError { analysis, .. } => analysis.as_ref(),
        };
        Self {
            candidate: super::generate::CandidateReport::new(index, text, assessment),
            focus_occurrence: assess_focus_occurrence(analysis, context),
            context_usage: assess_context_usage(analysis, context),
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
struct ContextReport<'a> {
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
    fn ready(prepared: &'a FocusedRequest<'a>) -> Self {
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
fn write_preview(out: &mut impl Write, report: &ContextReport<'_>, json: bool) -> Result<()> {
    if json {
        super::cli_support::write_json(out, report)?;
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
        super::cli_support::write_json(out, request)?;
    } else {
        writeln!(out, "Request: unavailable")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generation_text_does_not_expand_the_unselected_inventory() {
        let mut input: Input =
            serde_json::from_str(include_str!("../tests/fixtures/focused/pet-rest.json")).unwrap();
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
    #[test]
    fn preflight_and_preview_never_construct_a_client_and_output_errors_propagate() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("permissions.json");
        let input = include_str!("../tests/fixtures/focused/pet-rest.json");
        std::fs::write(&path, input).unwrap();
        let mut out = Vec::new();
        run(
            &path,
            1,
            None,
            true,
            |_| panic!("offline preview must not construct a client"),
            &mut out,
        )
        .unwrap();
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(run(&path, 1, None, true, |_| panic!("no client"), &mut Broken).is_err());
        for contents in [
            input.to_owned(),
            "{".into(),
            input.replace("\"TopicWa\"", "\"ObjectWo\""),
        ] {
            std::fs::write(&path, contents).unwrap();
            assert!(
                run(
                    &path,
                    1,
                    Some(Path::new("missing.dic")),
                    true,
                    |_| panic!("preflight must not construct a client"),
                    &mut Vec::new()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn typed_error_boundary_retains_valid_lexical_evidence_after_evaluation_error() {
        use yomibu::{
            analysis::{AnalysisProvenance, LexicalUnit, Sentence, SentenceAnalysis, Token},
            evaluation::EvaluationError,
            generation::CandidateError,
        };
        let input: Input =
            serde_json::from_str(include_str!("../tests/fixtures/focused/pet-rest.json")).unwrap();
        let grammar = GrammarDeclarations::from_descriptions(input.grammar).unwrap();
        let context = select_context(
            &grammar,
            &input.bindings,
            VocabularyEntryId::new(2).unwrap(),
        )
        .unwrap();
        let token = Token {
            span: 0..3,
            dictionary_form: "猫".into(),
            reading: "ネコ".into(),
            part_of_speech: vec![
                "名詞".into(),
                "*".into(),
                "*".into(),
                "*".into(),
                "*".into(),
                "*".into(),
            ],
            out_of_vocabulary: false,
        };
        let analysis = SentenceAnalysis {
            sentence: Sentence::new("猫").unwrap(),
            units: vec![LexicalUnit {
                components: vec![token.clone()],
                token,
            }],
            provenance: AnalysisProvenance {
                analyzer_revision: "synthetic-boundary",
                dictionary_version: "synthetic-boundary",
                dictionary_sha256: "synthetic-boundary",
                configuration_sha256: "synthetic-boundary".into(),
            },
        };
        let assessment = CandidateAssessment::ExecutionError {
            analysis: Some(analysis),
            error: CandidateError::Evaluation(EvaluationError::MissingDeclaration),
        };
        let report =
            serde_json::to_value(FocusedCandidate::new(1, "猫", &assessment, &context)).unwrap();
        assert_eq!(report["assessment"]["status"], "execution_error");
        assert_eq!(report["focus_occurrence"]["status"], "observed");
        assert_eq!(report["context_usage"]["status"], "completed");
        assert!(report["assessment"].get("evaluation").is_none());
    }
}
