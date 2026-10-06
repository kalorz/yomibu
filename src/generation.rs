//! Experimental text from one explicit provider request; never accepted exercises.
//!
//! The caller explicitly authorizes each request, supplies credentials and owns
//! the Tokio runtime. Assessment then stays synchronous and offline:
//!
//! ```no_run
//! use std::path::Path;
//! use yomibu::{adapters::{openai::Client, sudachi::SudachiAnalyzer},
//!     evaluation::{EvaluationBindings, VocabularyEntry, GrammarBinding, GrammarRule},
//!     grammar::GrammarDeclarations};
//!
//! # async fn example(api_key: &str, dictionary: &Path) -> Result<(), Box<dyn std::error::Error>> {
//! let grammar = GrammarDeclarations::from_descriptions(["です — nominal copula"])?;
//! let bindings = EvaluationBindings {
//!     vocabulary: vec![VocabularyEntry {
//!         written_form: "犬".into(), reading: "イヌ".into(), sense: "dog".into(),
//!         direct_object: false,
//!     }],
//!     grammar: vec![GrammarBinding { declaration_id: 1, rule: GrammarRule::NominalDesu }],
//! };
//! bindings.validate(&grammar)?;
//! let analyzer = SudachiAnalyzer::load(dictionary)?;
//! let client = Client::new(api_key)?;
//! let generated = client.generate_candidates(&grammar, &bindings).await?;
//! let assessments = generated.assess(&analyzer);
//! assert_eq!(generated.texts().len(), assessments.len());
//! // Inspect both results. A completed Pass is still not an accepted exercise.
//! # Ok(())
//! # }
//! ```

//!
//! Focused composition retains the prepared request through generation and assessment.
//! Preparation needs no analyzer, credential, client or runtime. A host can perform
//! it first, initialize execution resources, then await generation on its own runtime:
//!
//! ```no_run
//! use yomibu::{
//!     adapters::{openai::{Client, prepare_focused_request}, sudachi::SudachiAnalyzer},
//!     evaluation::EvaluationBindings,
//!     generation_context::{select_context, VocabularyEntryId},
//!     grammar::GrammarDeclarations,
//! };
//! # async fn focused(grammar: &GrammarDeclarations, permissions: &EvaluationBindings,
//! #     focus: VocabularyEntryId, api_key: &str, dictionary: &std::path::Path)
//! #     -> Result<(), Box<dyn std::error::Error>> {
//! let context = select_context(grammar, permissions, focus)?;
//! let prepared = prepare_focused_request(context)?;
//! let analyzer = SudachiAnalyzer::load(dictionary)?;
//! let client = Client::new(api_key)?;
//! let generated = client.generate_focused_candidates(&prepared).await?;
//! let assessments = generated.assess_focused(&analyzer, prepared.context());
//! // Both original texts and all available assessment evidence remain accessible.
//! assert_eq!(generated.texts().len(), assessments.len());
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};

use crate::{
    adapters::{
        openai::ProviderError,
        sudachi::{AnalysisError, SudachiAnalyzer},
    },
    analysis::{Sentence, SentenceAnalysis, SentenceError},
    evaluation::{Evaluation, EvaluationBindings, EvaluationError, evaluate},
    grammar::GrammarDeclarations,
};

#[derive(Debug, thiserror::Error)]
pub enum GenerationError {
    #[error(transparent)]
    Context(#[from] crate::generation_context::ContextError),
    #[error(transparent)]
    Bindings(#[from] EvaluationError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// Counts reported by the provider, not an independently verified bill.
#[derive(Debug, Deserialize, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct GenerationProvenance {
    pub provider: &'static str,
    pub requested_model: &'static str,
    pub returned_model: String,
    pub requested_tier: &'static str,
    pub returned_tier: Option<String>,
    pub prompt_revision: &'static str,
    pub request_sha256: String,
    pub request_bytes: usize,
    pub response_id: String,
    pub request_id: Option<String>,
    pub request_count: u8,
    pub usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_context: Option<FocusedGenerationProvenance>,
}

#[derive(Debug, Serialize)]
pub struct FocusedGenerationProvenance {
    pub selector_revision: &'static str,
    pub situation: &'static str,
    pub selected_entries: Vec<usize>,
}

#[derive(Debug)]
pub struct GeneratedCandidates<'input> {
    pub(crate) texts: [String; 2],
    pub(crate) provenance: GenerationProvenance,
    pub(crate) grammar: &'input GrammarDeclarations,
    pub(crate) bindings: &'input EvaluationBindings,
}

impl GeneratedCandidates<'_> {
    pub fn texts(&self) -> &[String; 2] {
        &self.texts
    }
    pub fn provenance(&self) -> &GenerationProvenance {
        &self.provenance
    }
    /// Each candidate is assessed independently against the unchanged permissions.
    pub fn assess(&self, analyzer: &SudachiAnalyzer) -> [CandidateAssessment<'_>; 2] {
        self.texts.each_ref().map(|text| {
            let sentence = match Sentence::new(text) {
                Ok(sentence) => sentence,
                Err(error) => {
                    return CandidateAssessment::ExecutionError {
                        analysis: None,
                        error: CandidateError::Sentence(error),
                    };
                }
            };
            let analysis = match analyzer.analyze(sentence) {
                Ok(analysis) => analysis,
                Err(error) => {
                    return CandidateAssessment::ExecutionError {
                        analysis: None,
                        error: CandidateError::Analysis(error),
                    };
                }
            };
            match evaluate(&analysis, self.grammar, self.bindings) {
                Ok(evaluation) => CandidateAssessment::Completed {
                    analysis,
                    evaluation: Box::new(evaluation),
                },
                Err(error) => CandidateAssessment::ExecutionError {
                    analysis: Some(analysis),
                    error: CandidateError::Evaluation(error),
                },
            }
        })
    }

    /// Assess both texts against the original full permissions, then observe focus
    /// and context use. Supply the context from the request that produced these texts.
    /// Candidate errors retain available analysis and do not stop the other candidate.
    pub fn assess_focused(
        &self,
        analyzer: &SudachiAnalyzer,
        context: &GenerationContext<'_>,
    ) -> [FocusedCandidateAssessment<'_>; 2] {
        self.assess(analyzer)
            .map(|assessment| FocusedCandidateAssessment::new(assessment, context))
    }
}

#[derive(Debug)]
pub enum CandidateAssessment<'a> {
    Completed {
        analysis: SentenceAnalysis<'a>,
        evaluation: Box<Evaluation>,
    },
    ExecutionError {
        analysis: Option<SentenceAnalysis<'a>>,
        error: CandidateError,
    },
}

/// A candidate's execution result and separate morphological observations.
/// These observations neither change evaluator judgments nor establish acceptance.
#[derive(Debug)]
pub struct FocusedCandidateAssessment<'a> {
    pub assessment: CandidateAssessment<'a>,
    pub focus_occurrence: FocusOccurrenceReport,
    pub context_usage: ContextUsageReport,
}

impl<'a> FocusedCandidateAssessment<'a> {
    fn new(assessment: CandidateAssessment<'a>, context: &GenerationContext<'_>) -> Self {
        let analysis = match &assessment {
            CandidateAssessment::Completed { analysis, .. } => Some(analysis),
            CandidateAssessment::ExecutionError { analysis, .. } => analysis.as_ref(),
        };
        let focus_occurrence = assess_focus_occurrence(analysis, context);
        let context_usage = assess_context_usage(analysis, context);
        Self {
            assessment,
            focus_occurrence,
            context_usage,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CandidateError {
    #[error(transparent)]
    Sentence(SentenceError),
    // Retain the typed cause for callers without reflecting dependency text in reports.
    #[error("Pinned Sudachi analysis failed.")]
    Analysis(#[source] AnalysisError),
    #[error(transparent)]
    Evaluation(EvaluationError),
}

use crate::{
    analysis::Token,
    evaluation::{VocabularyEntry, reading_matches, regular_stem},
    generation_context::{GenerationContext, SlotRole},
};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusStatus {
    Observed,
    Absent,
    Ambiguous,
    Unassessable,
    NotRun,
}
/// Completeness of the lexical observations, not a reading/sense judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusCompleteness {
    Complete,
    Partial,
    NotRun,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OccurrenceEvidence {
    Noninflected,
    RegularStem,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LexicalUncertaintyReason {
    ComponentOnly,
    OutOfVocabulary,
    UnsupportedMorphology,
    ReadingDisagreement,
    CompetingIdentity,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct FocusOccurrence {
    pub span: Range<usize>,
    pub reading: String,
    pub evidence: OccurrenceEvidence,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct LexicalUncertainty {
    pub span: Range<usize>,
    pub reason: LexicalUncertaintyReason,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct FocusOccurrenceReport {
    pub status: FocusStatus,
    pub completeness: FocusCompleteness,
    pub count: usize,
    pub occurrences: Vec<FocusOccurrence>,
    pub uncertainties: Vec<LexicalUncertainty>,
    pub contextual_reading_and_sense: &'static str,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextUnitStatus {
    SelectedEvidence,
    UnselectedPermissionEvidence,
    NoPermissionEntry,
    Unresolved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextUsageStatus {
    Completed,
    NotRun,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ContextUnitEvidence {
    #[serde(flatten)]
    pub token: Token,
    /// Full-inventory dictionary-form matches; still hypotheses, not judgments.
    pub entries: Vec<usize>,
    pub status: ContextUnitStatus,
    pub reason: Option<LexicalUncertaintyReason>,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ContextUsageReport {
    pub status: ContextUsageStatus,
    pub units: Vec<ContextUnitEvidence>,
}

/// Observe morphological occurrences independently of evaluator outcomes.
/// Unrelated uncertainty limits completeness without overriding observed focus.
/// Compatible spans survive focus uncertainty; neither reading nor sense is validated.
pub fn assess_focus_occurrence(
    analysis: Option<&SentenceAnalysis<'_>>,
    context: &GenerationContext<'_>,
) -> FocusOccurrenceReport {
    let mut report = FocusOccurrenceReport {
        status: FocusStatus::NotRun,
        completeness: FocusCompleteness::NotRun,
        count: 0,
        occurrences: Vec::new(),
        uncertainties: Vec::new(),
        contextual_reading_and_sense: "not_assessed",
    };
    let Some(analysis) = analysis.filter(|a| structurally_valid(a)) else {
        return report;
    };
    // Selection's independent validation guarantees exactly one first focus.
    let Some(focus) = context.selected().first() else {
        return report;
    };
    let mut ambiguous = false;
    let mut focus_unassessable = false;
    for unit in &analysis.units {
        let token = &unit.token;
        if grammar_token(token) {
            continue;
        }
        let potential = token.dictionary_form == focus.vocabulary.written_form;
        let evidence = morphology(token, analysis.sentence.text());
        let issue = if token.out_of_vocabulary {
            Some(LexicalUncertaintyReason::OutOfVocabulary)
        } else if potential && !role_matches(focus.role, token) {
            Some(LexicalUncertaintyReason::CompetingIdentity)
        } else if evidence.is_none() {
            Some(LexicalUncertaintyReason::UnsupportedMorphology)
        } else if potential && competing_identity(token, context) {
            Some(LexicalUncertaintyReason::CompetingIdentity)
        } else if potential && !reading_matches(focus.vocabulary, token, analysis.sentence.text()) {
            Some(LexicalUncertaintyReason::ReadingDisagreement)
        } else {
            None
        };
        if let Some(reason) = issue {
            if matches!(
                reason,
                LexicalUncertaintyReason::CompetingIdentity
                    | LexicalUncertaintyReason::ReadingDisagreement
            ) {
                ambiguous = true;
            } else if potential {
                focus_unassessable = true;
            }
            report.uncertainties.push(LexicalUncertainty {
                span: token.span.clone(),
                reason,
            });
        } else if potential && let Some(evidence) = evidence {
            report.occurrences.push(FocusOccurrence {
                span: token.span.clone(),
                reading: token.reading.clone(),
                evidence,
            });
        }
        if !potential {
            for component in &unit.components {
                if component.dictionary_form == focus.vocabulary.written_form {
                    ambiguous = true;
                    report.uncertainties.push(LexicalUncertainty {
                        span: component.span.clone(),
                        reason: LexicalUncertaintyReason::ComponentOnly,
                    });
                }
            }
        }
    }
    report
        .uncertainties
        .sort_by_key(|u| (u.span.start, u.span.end));
    report.count = report.occurrences.len();
    report.completeness = if report.uncertainties.is_empty() {
        FocusCompleteness::Complete
    } else {
        FocusCompleteness::Partial
    };
    report.status = if ambiguous {
        FocusStatus::Ambiguous
    } else if focus_unassessable
        || (report.count == 0 && report.completeness == FocusCompleteness::Partial)
    {
        // Unidentified lexical evidence may conceal a focus occurrence, so it
        // cannot establish absence even when its dictionary form differs.
        FocusStatus::Unassessable
    } else if report.count > 0 {
        FocusStatus::Observed
    } else {
        FocusStatus::Absent
    };
    report
}

/// Whole-unit membership observations using the full local inventory. Grammar and
/// punctuation are excluded, and observations never change permission judgments.
pub fn assess_context_usage(
    analysis: Option<&SentenceAnalysis<'_>>,
    context: &GenerationContext<'_>,
) -> ContextUsageReport {
    let Some(analysis) = analysis.filter(|a| structurally_valid(a)) else {
        return ContextUsageReport {
            status: ContextUsageStatus::NotRun,
            units: Vec::new(),
        };
    };
    let units = analysis
        .units
        .iter()
        .filter(|u| !grammar_token(&u.token))
        .map(|unit| {
            let token = &unit.token;
            let matches: Vec<(usize, &VocabularyEntry)> = context
                .permissions()
                .vocabulary
                .iter()
                .enumerate()
                .filter(|(_, w)| w.written_form == token.dictionary_form)
                .collect();
            let reason = if token.out_of_vocabulary {
                Some(LexicalUncertaintyReason::OutOfVocabulary)
            } else if morphology(token, analysis.sentence.text()).is_none() {
                Some(LexicalUncertaintyReason::UnsupportedMorphology)
            } else if competing_identity(token, context)
                || context.selected().iter().any(|s| {
                    s.vocabulary.written_form == token.dictionary_form
                        && !role_matches(s.role, token)
                })
            {
                Some(LexicalUncertaintyReason::CompetingIdentity)
            } else if matches
                .first()
                .is_some_and(|(_, word)| !reading_matches(word, token, analysis.sentence.text()))
            {
                Some(LexicalUncertaintyReason::ReadingDisagreement)
            } else {
                None
            };
            let status = if reason.is_some() {
                ContextUnitStatus::Unresolved
            } else if matches.is_empty() {
                ContextUnitStatus::NoPermissionEntry
            } else if context
                .selected()
                .iter()
                .any(|s| s.vocabulary == matches[0].1)
            {
                ContextUnitStatus::SelectedEvidence
            } else {
                ContextUnitStatus::UnselectedPermissionEvidence
            };
            ContextUnitEvidence {
                token: token.clone(),
                entries: matches.iter().map(|(i, _)| i + 1).collect(),
                status,
                reason,
            }
        })
        .collect();
    ContextUsageReport {
        status: ContextUsageStatus::Completed,
        units,
    }
}

fn competing_identity(token: &Token, context: &GenerationContext<'_>) -> bool {
    let mut entries = context
        .permissions()
        .vocabulary
        .iter()
        .filter(|w| w.written_form == token.dictionary_form);
    entries
        .next()
        .is_some_and(|first| entries.any(|other| other != first))
}
fn role_matches(role: SlotRole, token: &Token) -> bool {
    match role {
        SlotRole::Predicate => token.part_of_speech[0] == "動詞",
        SlotRole::Participant | SlotRole::Object => {
            ["名詞", "代名詞"].contains(&token.part_of_speech[0].as_str())
        }
    }
}
fn grammar_token(token: &Token) -> bool {
    !token.out_of_vocabulary
        && ["助詞", "助動詞", "補助記号"].contains(&token.part_of_speech[0].as_str())
}
pub(crate) fn morphology(token: &Token, text: &str) -> Option<OccurrenceEvidence> {
    match token.part_of_speech[0].as_str() {
        "名詞" | "代名詞" => Some(OccurrenceEvidence::Noninflected),
        "動詞" => {
            let stem = regular_stem(&token.dictionary_form, token)?;
            let surface = text.get(token.span.clone())?;
            if surface == token.dictionary_form {
                Some(OccurrenceEvidence::Noninflected)
            } else if surface == stem {
                Some(OccurrenceEvidence::RegularStem)
            } else {
                None
            }
        }
        _ => None,
    }
}

// Reporting must also be safe when supplied analysis survives an evaluation
// error. Check complete whole/component coverage before any POS indexing/slicing;
// the evaluator remains unchanged and owns its independent validation boundary.
fn structurally_valid(analysis: &SentenceAnalysis<'_>) -> bool {
    let text = analysis.sentence.text();
    let valid = |t: &Token| {
        !t.span.is_empty()
            && text.get(t.span.clone()).is_some()
            && t.part_of_speech.len() == 6
            && !t.dictionary_form.is_empty()
    };
    let mut end = 0;
    for unit in &analysis.units {
        if unit.token.span.start != end || !valid(&unit.token) {
            return false;
        }
        let mut component_end = end;
        for component in &unit.components {
            if component.span.start != component_end || !valid(component) {
                return false;
            }
            component_end = component.span.end;
        }
        if component_end != unit.token.span.end {
            return false;
        }
        end = unit.token.span.end;
    }
    end == text.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_error_boundary_retains_valid_lexical_evidence_after_evaluation_error() {
        use crate::{
            analysis::{AnalysisProvenance, LexicalUnit, Sentence, SentenceAnalysis, Token},
            evaluation::EvaluationError,
            generation_context::{VocabularyEntryId, select_context},
        };
        let input: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/focused/pet-rest.json")).unwrap();
        let grammar = GrammarDeclarations::from_descriptions(
            input["grammar"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| g.as_str().unwrap()),
        )
        .unwrap();
        let permissions = serde_json::from_value(input["bindings"].clone()).unwrap();
        let context =
            select_context(&grammar, &permissions, VocabularyEntryId::new(2).unwrap()).unwrap();
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
                dictionary_loading: None,
            },
        };
        let assessment = CandidateAssessment::ExecutionError {
            analysis: Some(analysis),
            error: CandidateError::Evaluation(EvaluationError::MissingDeclaration),
        };
        let focused = FocusedCandidateAssessment::new(assessment, &context);
        assert!(matches!(
            focused.assessment,
            CandidateAssessment::ExecutionError {
                analysis: Some(_),
                error: CandidateError::Evaluation(EvaluationError::MissingDeclaration)
            }
        ));
        assert_eq!(focused.focus_occurrence.status, FocusStatus::Observed);
        assert_eq!(focused.context_usage.status, ContextUsageStatus::Completed);
    }
}
