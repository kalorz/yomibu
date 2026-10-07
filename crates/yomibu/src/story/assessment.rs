//! Full-inventory assessment, target observations and original candidates.

use std::{collections::BTreeMap, ops::Range};

use serde::Serialize;

use super::{StoryError, StoryRequest, StoryVocabularySelection};
use crate::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::{Sentence, SentenceAnalysis, Token},
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
    selected_vocabulary_ids: Vec<&'a str>,
    structural_grammar: GrammarDeclarations,
    structural_bindings: EvaluationBindings,
}
impl<'a> StoryAssessmentInputs<'a> {
    pub fn new(
        inventory: &'a LearnerInventory,
        request: &'a StoryRequest,
        selection: &StoryVocabularySelection<'a>,
    ) -> Result<Self, StoryError> {
        request.validate(inventory)?;
        let (structural_grammar, structural_bindings) = project_structural_inputs(inventory)?;
        Ok(Self {
            inventory,
            request,
            selected_vocabulary_ids: selection
                .selected
                .iter()
                .map(|entry| entry.word.id.as_str())
                .collect(),
            structural_grammar,
            structural_bindings,
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
            .map(|declaration| declaration.description.as_str()),
    )
    .map_err(|_| StoryError::Invalid("grammar declarations"))?;
    let mut written_form_counts = BTreeMap::new();
    for word in &inventory.vocabulary {
        *written_form_counts.entry(&word.written_form).or_insert(0) += 1;
    }
    let vocabulary = inventory
        .vocabulary
        .iter()
        .filter(|word| written_form_counts.get(&word.written_form) == Some(&1))
        .filter_map(evaluation::single_use)
        .collect();
    let bindings = inventory
        .grammar_bindings
        .iter()
        .map(|binding| {
            inventory
                .grammar_declarations
                .iter()
                .position(|declaration| declaration.id == binding.declaration_id)
                .map(|index| GrammarBinding {
                    declaration_id: index + 1,
                    rule: binding.rule,
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
            let assessment = evaluate_candidate(text, inputs, analyzer);
            let analysis = match &assessment {
                CandidateAssessment::Completed { analysis, .. } => Some(analysis),
                CandidateAssessment::ExecutionError { analysis, .. } => analysis.as_ref(),
            };
            let mut targets = observe_vocabulary_targets(analysis, inputs);
            targets.extend(observe_grammar_targets(analysis, inputs));
            let plan_departures = observe_plan_departures(analysis, inputs);
            StoryCandidateAssessment {
                assessment,
                targets,
                plan_departures,
            }
        })
        .collect()
}

fn evaluate_candidate<'a>(
    text: &'a str,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: &SudachiAnalyzer,
) -> CandidateAssessment<'a> {
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
    match evaluation::evaluate_inventory(
        &analysis,
        &inputs.structural_grammar,
        &inputs.structural_bindings,
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
    }
}

fn observe_plan_departures(
    analysis: Option<&SentenceAnalysis<'_>>,
    inputs: &StoryAssessmentInputs<'_>,
) -> Vec<PlanDeparture> {
    let Some(analysis) = analysis else {
        return Vec::new();
    };
    analysis
        .units
        .iter()
        .filter_map(|unit| {
            let token = &unit.token;
            if !token.out_of_vocabulary && is_function_word_or_punctuation(token) {
                return None;
            }
            let inventory_entries: Vec<_> = inputs
                .inventory
                .vocabulary
                .iter()
                .filter(|word| word.written_form == token.dictionary_form)
                .map(|word| word.id.clone())
                .collect();
            if inventory_entries.is_empty()
                || inventory_entries
                    .iter()
                    .any(|id| inputs.selected_vocabulary_ids.contains(&id.as_str()))
            {
                None
            } else {
                Some(PlanDeparture {
                    span: token.span.clone(),
                    inventory_entries,
                })
            }
        })
        .collect()
}

fn observe_vocabulary_targets(
    analysis: Option<&SentenceAnalysis<'_>>,
    inputs: &StoryAssessmentInputs<'_>,
) -> Vec<TargetObservation> {
    let mut observations = Vec::new();
    for id in &inputs.request.targets.vocabulary {
        let word = inputs
            .inventory
            .vocabulary
            .iter()
            .find(|word| &word.id == id);
        let mut spans = Vec::new();
        let mut has_uncertain_evidence = false;
        let mut has_uncertain_target_occurrence = false;
        if let (Some(analysis), Some(word)) = (analysis, word) {
            let single_use = evaluation::single_use(word);
            let has_competing_identity = inputs
                .inventory
                .vocabulary
                .iter()
                .filter(|entry| entry.written_form == word.written_form)
                .count()
                != 1;
            for unit in &analysis.units {
                let supports_morphology =
                    supports_target_morphology(&unit.token, analysis.sentence.text());
                if unit.token.dictionary_form == word.written_form {
                    let matches_single_use = !has_competing_identity
                        && single_use.as_ref().is_some_and(|word_use| {
                            evaluation::reading_matches(
                                word_use,
                                &unit.token,
                                analysis.sentence.text(),
                            )
                        });
                    if matches_single_use && supports_morphology && !unit.token.out_of_vocabulary {
                        spans.push(unit.token.span.clone());
                    } else {
                        has_uncertain_evidence = true;
                        has_uncertain_target_occurrence = true;
                    }
                } else if unit
                    .components
                    .iter()
                    .any(|component| component.dictionary_form == word.written_form)
                {
                    has_uncertain_evidence = true;
                    has_uncertain_target_occurrence = true;
                }
                if unit.token.out_of_vocabulary
                    || (!supports_morphology && !is_function_word_or_punctuation(&unit.token))
                {
                    has_uncertain_evidence = true;
                }
            }
        }
        observations.push(TargetObservation {
            kind: "vocabulary",
            id: id.clone(),
            status: if analysis.is_none() {
                "not_run"
            } else if has_uncertain_target_occurrence
                || (has_uncertain_evidence && spans.is_empty())
            {
                "unassessable"
            } else if spans.is_empty() {
                "absent"
            } else {
                "observed"
            },
            completeness: if analysis.is_none() {
                "not_run"
            } else if has_uncertain_evidence {
                "partial"
            } else {
                "complete"
            },
            spans,
        });
    }
    observations
}

fn observe_grammar_targets(
    analysis: Option<&SentenceAnalysis<'_>>,
    inputs: &StoryAssessmentInputs<'_>,
) -> Vec<TargetObservation> {
    let mut observations = Vec::new();
    let observed_rules = analysis
        .and_then(|analysis| evaluation::observed_grammar(analysis, &inputs.structural_bindings));
    for id in &inputs.request.targets.grammar {
        let rules: Vec<GrammarRule> = inputs
            .inventory
            .grammar_bindings
            .iter()
            .filter(|binding| &binding.declaration_id == id)
            .map(|binding| binding.rule)
            .collect();
        let spans = observed_rules
            .as_ref()
            .map(|observed_rules| {
                observed_rules
                    .iter()
                    .filter(|(rule, _)| rules.contains(rule))
                    .map(|(_, span)| span.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let status = if analysis.is_none() {
            "not_run"
        } else if rules.is_empty() || observed_rules.is_none() {
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
            } else if rules.is_empty() || observed_rules.is_none() {
                "partial"
            } else {
                "complete"
            },
            spans,
        });
    }
    observations
}

fn is_function_word_or_punctuation(token: &Token) -> bool {
    ["助詞", "助動詞", "補助記号"].contains(&token.part_of_speech[0].as_str())
}

fn supports_target_morphology(token: &Token, text: &str) -> bool {
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
