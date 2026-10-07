//! Full-inventory assessment, target observations and original candidates.

use std::{collections::BTreeMap, ops::Range};

use serde::Serialize;

use super::{StoryError, StoryRequest, StoryVocabularySelection};
use crate::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::{Sentence, SentenceAnalysis},
    candidate::{CandidateAssessment, CandidateError, GenerationProvenance},
    evaluation::{self, EvaluationBindings, GrammarBinding, GrammarRule},
    grammar::GrammarDeclarations,
    inventory::LearnerInventory,
};

/// Full original inventory, request and selected IDs for candidate assessment.
/// The structural checker needs an owned projection; vocabulary checks still use
/// the complete inventory, including entries not selected for the prompt.
#[derive(Debug)]
pub struct StoryAssessmentInputs<'a> {
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    selected_vocabulary: Vec<&'a str>,
    grammar: GrammarDeclarations,
    bindings: EvaluationBindings,
}
impl<'a> StoryAssessmentInputs<'a> {
    pub fn new(
        inventory: &'a LearnerInventory,
        request: &'a StoryRequest,
        plan: &StoryVocabularySelection<'a>,
    ) -> Result<Self, StoryError> {
        request.validate(inventory)?;
        let (grammar, bindings) = project_structural_inputs(inventory)?;
        Ok(Self {
            inventory,
            request,
            selected_vocabulary: plan.selected.iter().map(|s| s.word.id.as_str()).collect(),
            grammar,
            bindings,
        })
    }
}

fn project_structural_inputs(
    inventory: &LearnerInventory,
) -> Result<(GrammarDeclarations, EvaluationBindings), StoryError> {
    let grammar = GrammarDeclarations::from_descriptions(
        inventory
            .grammar_declarations
            .iter()
            .map(|g| g.description.as_str()),
    )
    .map_err(|_| StoryError::Invalid("grammar declarations"))?;
    let mut form_counts = BTreeMap::new();
    for word in &inventory.vocabulary {
        *form_counts.entry(&word.written_form).or_insert(0) += 1;
    }
    let vocabulary = inventory
        .vocabulary
        .iter()
        .filter(|word| form_counts.get(&word.written_form) == Some(&1))
        .filter_map(evaluation::single_use)
        .collect();
    let bindings = inventory
        .grammar_bindings
        .iter()
        .map(|b| {
            inventory
                .grammar_declarations
                .iter()
                .position(|g| g.id == b.declaration_id)
                .map(|i| GrammarBinding {
                    declaration_id: i + 1,
                    rule: b.rule,
                })
                .ok_or(StoryError::Invalid("missing grammar declaration"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        grammar,
        EvaluationBindings {
            vocabulary,
            grammar: bindings,
        },
    ))
}
#[derive(Debug, Serialize)]
pub struct TargetObservation {
    pub kind: &'static str,
    pub id: String,
    pub status: &'static str,
    pub completeness: &'static str,
    pub spans: Vec<Range<usize>>,
}
#[derive(Debug, Serialize)]
pub struct PlanDeparture {
    pub span: Range<usize>,
    pub inventory_entries: Vec<String>,
}
#[derive(Debug)]
pub struct StoryCandidateAssessment<'a> {
    pub assessment: CandidateAssessment<'a>,
    pub targets: Vec<TargetObservation>,
    pub plan_departures: Vec<PlanDeparture>,
}

impl StoryCandidateAssessment<'_> {
    pub(super) fn into_owned(self) -> StoryCandidateAssessment<'static> {
        let assessment = match self.assessment {
            CandidateAssessment::Completed {
                analysis,
                evaluation,
            } => CandidateAssessment::Completed {
                analysis: analysis.into_owned(),
                evaluation,
            },
            CandidateAssessment::ExecutionError { analysis, error } => {
                CandidateAssessment::ExecutionError {
                    analysis: analysis.map(SentenceAnalysis::into_owned),
                    error,
                }
            }
        };
        StoryCandidateAssessment {
            assessment,
            targets: self.targets,
            plan_departures: self.plan_departures,
        }
    }
}

/// Original generated texts and metadata, independent of request/input lifetimes.
#[derive(Debug)]
pub struct StoryCandidates {
    pub(crate) texts: Vec<String>,
    pub(crate) provenance: GenerationProvenance,
}
impl StoryCandidates {
    pub fn texts(&self) -> &[String] {
        &self.texts
    }
    pub fn provenance(&self) -> &crate::candidate::GenerationProvenance {
        &self.provenance
    }
}

pub fn assess_candidates<'a>(
    generated: &'a StoryCandidates,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: &SudachiAnalyzer,
) -> Vec<StoryCandidateAssessment<'a>> {
    generated
        .texts()
        .iter()
        .map(|text| {
            let assessment = match Sentence::new(text) {
                Err(error) => CandidateAssessment::ExecutionError {
                    analysis: None,
                    error: CandidateError::Sentence(error),
                },
                Ok(sentence) => match analyzer.analyze(sentence) {
                    Err(error) => CandidateAssessment::ExecutionError {
                        analysis: None,
                        error: CandidateError::Analysis(error),
                    },
                    Ok(analysis) => match evaluation::evaluate_inventory(
                        &analysis,
                        &inputs.grammar,
                        &inputs.bindings,
                        inputs.inventory,
                    ) {
                        Ok(evaluation) => CandidateAssessment::Completed {
                            analysis,
                            evaluation: Box::new(evaluation),
                        },
                        Err(error) => CandidateAssessment::ExecutionError {
                            analysis: Some(analysis),
                            error: CandidateError::Evaluation(error),
                        },
                    },
                },
            };
            let analysis = match &assessment {
                CandidateAssessment::Completed { analysis, .. } => Some(analysis),
                CandidateAssessment::ExecutionError { analysis, .. } => analysis.as_ref(),
            };
            let targets = observe_targets(analysis, inputs);
            let plan_departures = analysis
                .map(|a| {
                    a.units
                        .iter()
                        .filter_map(|u| {
                            if !u.token.out_of_vocabulary
                                && ["助詞", "助動詞", "補助記号"]
                                    .contains(&u.token.part_of_speech[0].as_str())
                            {
                                return None;
                            }
                            let entries: Vec<_> = inputs
                                .inventory
                                .vocabulary
                                .iter()
                                .filter(|w| w.written_form == u.token.dictionary_form)
                                .map(|w| w.id.clone())
                                .collect();
                            if entries.is_empty()
                                || entries
                                    .iter()
                                    .any(|id| inputs.selected_vocabulary.contains(&id.as_str()))
                            {
                                None
                            } else {
                                Some(PlanDeparture {
                                    span: u.token.span.clone(),
                                    inventory_entries: entries,
                                })
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            StoryCandidateAssessment {
                assessment,
                targets,
                plan_departures,
            }
        })
        .collect()
}
fn observe_targets(
    analysis: Option<&SentenceAnalysis<'_>>,
    inputs: &StoryAssessmentInputs<'_>,
) -> Vec<TargetObservation> {
    let mut observations = Vec::new();
    for id in &inputs.request.targets.vocabulary {
        let word = inputs.inventory.vocabulary.iter().find(|w| &w.id == id);
        let mut spans = Vec::new();
        let mut uncertain = false;
        let mut target_uncertain = false;
        if let (Some(a), Some(word)) = (analysis, word) {
            let lexical = evaluation::single_use(word);
            let competing = inputs
                .inventory
                .vocabulary
                .iter()
                .filter(|w| w.written_form == word.written_form)
                .count()
                != 1;
            for unit in &a.units {
                let supported = supports_target_morphology(&unit.token, a.sentence.text());
                if unit.token.dictionary_form == word.written_form {
                    let matches = !competing
                        && lexical.as_ref().is_some_and(|w| {
                            evaluation::reading_matches(w, &unit.token, a.sentence.text())
                        });
                    if matches && supported && !unit.token.out_of_vocabulary {
                        spans.push(unit.token.span.clone());
                    } else {
                        uncertain = true;
                        target_uncertain = true;
                    }
                } else if unit
                    .components
                    .iter()
                    .any(|t| t.dictionary_form == word.written_form)
                {
                    uncertain = true;
                    target_uncertain = true;
                }
                if unit.token.out_of_vocabulary
                    || (!supported
                        && !["助詞", "助動詞", "補助記号"]
                            .contains(&unit.token.part_of_speech[0].as_str()))
                {
                    uncertain = true;
                }
            }
        }
        observations.push(TargetObservation {
            kind: "vocabulary",
            id: id.clone(),
            status: if analysis.is_none() {
                "not_run"
            } else if target_uncertain || (uncertain && spans.is_empty()) {
                "unassessable"
            } else if spans.is_empty() {
                "absent"
            } else {
                "observed"
            },
            completeness: if analysis.is_none() {
                "not_run"
            } else if uncertain {
                "partial"
            } else {
                "complete"
            },
            spans,
        });
    }
    let matched =
        analysis.and_then(|analysis| evaluation::observed_grammar(analysis, &inputs.bindings));
    for id in &inputs.request.targets.grammar {
        let rules: Vec<GrammarRule> = inputs
            .inventory
            .grammar_bindings
            .iter()
            .filter(|b| &b.declaration_id == id)
            .map(|b| b.rule)
            .collect();
        let spans = matched
            .as_ref()
            .map(|m| {
                m.iter()
                    .filter(|(rule, _)| rules.contains(rule))
                    .map(|(_, span)| span.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let status = if analysis.is_none() {
            "not_run"
        } else if rules.is_empty() || matched.is_none() {
            "unassessable"
        } else if spans.is_empty() {
            "absent"
        } else {
            "observed"
        };
        observations.push(TargetObservation {
            kind: "grammar",
            id: id.clone(),
            status,
            completeness: if analysis.is_none() {
                "not_run"
            } else if rules.is_empty() || matched.is_none() {
                "partial"
            } else {
                "complete"
            },
            spans,
        });
    }
    observations
}

fn supports_target_morphology(token: &crate::analysis::Token, text: &str) -> bool {
    match token.part_of_speech[0].as_str() {
        "名詞" | "代名詞" => true,
        "動詞" => {
            let Some(stem) = evaluation::regular_stem(&token.dictionary_form, token) else {
                return false;
            };
            let Some(surface) = text.get(token.span.clone()) else {
                return false;
            };
            surface == token.dictionary_form || surface == stem
        }
        _ => false,
    }
}
