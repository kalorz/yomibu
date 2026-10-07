//! Full-inventory assessment, target observations and original candidates.

use std::ops::Range;

use serde::Serialize;

use super::{
    StoryError, StoryRequest, StoryVocabularySelection, TargetKind, TargetObservation, TargetState,
    TargetUncertainty, TargetUncertaintyReason, TargetUncertaintyScope,
};
use crate::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::{Sentence, SentenceAnalysis},
    candidate::{CandidateAssessment, CandidateError, GenerationProvenance},
    evaluation::{
        self, DirectObjectEvidence, LexicalStatus, LexicalUncertainty, SentenceAssessment,
    },
    inventory::LearnerInventory,
};

/// Assessment always uses the full inventory, including unselected material.
#[derive(Debug)]
pub struct StoryAssessmentInputs<'a> {
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    selected_vocabulary_ids: Vec<&'a str>,
}
impl<'a> StoryAssessmentInputs<'a> {
    pub fn new(
        inventory: &'a LearnerInventory,
        request: &'a StoryRequest,
        selection: &StoryVocabularySelection<'a>,
    ) -> Result<Self, StoryError> {
        request.validate(inventory)?;
        Ok(Self {
            inventory,
            request,
            selected_vocabulary_ids: selection
                .selected
                .iter()
                .map(|entry| entry.word.id.as_str())
                .collect(),
        })
    }
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
        .map(|text| assess_candidate(text, inputs, analyzer))
        .collect()
}

fn assess_candidate<'a>(
    text: &'a str,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: &SudachiAnalyzer,
) -> StoryCandidateAssessment<'a> {
    let sentence = match Sentence::new(text) {
        Ok(sentence) => sentence,
        Err(error) => {
            return failed_candidate(None, CandidateError::Sentence(error), inputs);
        }
    };
    let analysis = match analyzer.analyze(sentence) {
        Ok(analysis) => analysis,
        Err(error) => {
            return failed_candidate(None, CandidateError::Analysis(error), inputs);
        }
    };
    let assessed = match evaluation::assess_inventory(&analysis, inputs.inventory) {
        Ok(assessed) => assessed,
        Err(error) => {
            return failed_candidate(Some(analysis), CandidateError::Evaluation(error), inputs);
        }
    };
    let mut targets = observe_vocabulary_targets(&analysis, &assessed, inputs);
    targets.extend(observe_grammar_targets(&analysis, &assessed, inputs));
    let plan_departures = observe_plan_departures(&analysis, &assessed, inputs);
    StoryCandidateAssessment {
        assessment: CandidateAssessment::Completed {
            analysis,
            evaluation: Box::new(assessed.evaluation),
        },
        targets,
        plan_departures,
    }
}

fn failed_candidate<'a>(
    analysis: Option<SentenceAnalysis<'a>>,
    error: CandidateError,
    inputs: &StoryAssessmentInputs<'_>,
) -> StoryCandidateAssessment<'a> {
    let targets = [
        (TargetKind::Vocabulary, &inputs.request.targets.vocabulary),
        (TargetKind::Grammar, &inputs.request.targets.grammar),
    ]
    .into_iter()
    .flat_map(|(kind, ids)| {
        ids.iter().map(move |id| TargetObservation {
            kind,
            id: id.clone(),
            state: TargetState::NotRun,
            spans: Vec::new(),
            uncertainties: Vec::new(),
        })
    })
    .collect();
    StoryCandidateAssessment {
        assessment: CandidateAssessment::ExecutionError { analysis, error },
        targets,
        plan_departures: Vec::new(),
    }
}

fn observe_plan_departures(
    analysis: &SentenceAnalysis<'_>,
    assessed: &SentenceAssessment,
    inputs: &StoryAssessmentInputs<'_>,
) -> Vec<PlanDeparture> {
    analysis
        .units
        .iter()
        .zip(&assessed.lexical)
        .filter_map(|(unit, evidence)| {
            if evidence.status == LexicalStatus::Exempt {
                return None;
            }
            let inventory_entries = &evidence.inventory_entries;
            if inventory_entries.is_empty()
                || inventory_entries
                    .iter()
                    .any(|id| inputs.selected_vocabulary_ids.contains(&id.as_str()))
            {
                None
            } else {
                Some(PlanDeparture {
                    span: unit.token.span.clone(),
                    inventory_entries: inventory_entries.clone(),
                })
            }
        })
        .collect()
}

fn observe_vocabulary_targets(
    analysis: &SentenceAnalysis<'_>,
    assessed: &SentenceAssessment,
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
        let mut uncertainties = Vec::new();
        if let Some(word) = word {
            for (unit, evidence) in analysis.units.iter().zip(&assessed.lexical) {
                let supports_morphology =
                    evaluation::supports_target_morphology(&unit.token, analysis.sentence.text());
                let uncertainty = if unit.token.dictionary_form == word.written_form {
                    if evidence.status == LexicalStatus::Available && supports_morphology {
                        spans.push(unit.token.span.clone());
                        None
                    } else {
                        Some((
                            TargetUncertaintyScope::TargetOccurrence,
                            match evidence.status {
                                LexicalStatus::Unresolved(reason) => {
                                    TargetUncertaintyReason::Lexical(reason)
                                }
                                _ => TargetUncertaintyReason::UnsupportedMorphology,
                            },
                        ))
                    }
                } else if unit
                    .components
                    .iter()
                    .any(|component| component.dictionary_form == word.written_form)
                {
                    Some((
                        TargetUncertaintyScope::TargetOccurrence,
                        TargetUncertaintyReason::ComponentOnly,
                    ))
                } else if unit.token.out_of_vocabulary {
                    Some((
                        TargetUncertaintyScope::SentenceCoverage,
                        TargetUncertaintyReason::Lexical(LexicalUncertainty::OutOfDictionary),
                    ))
                } else if !supports_morphology && !unit.token.is_function_word_or_punctuation() {
                    Some((
                        TargetUncertaintyScope::SentenceCoverage,
                        TargetUncertaintyReason::UnsupportedMorphology,
                    ))
                } else {
                    None
                };
                if let Some((scope, reason)) = uncertainty {
                    uncertainties.push(TargetUncertainty {
                        span: unit.token.span.clone(),
                        scope,
                        reason,
                        inventory_entries: evidence.inventory_entries.clone(),
                    });
                }
            }
        }
        observations.push(TargetObservation::from_evidence(
            TargetKind::Vocabulary,
            id,
            spans,
            uncertainties,
        ));
    }
    observations
}

fn observe_grammar_targets(
    analysis: &SentenceAnalysis<'_>,
    assessed: &SentenceAssessment,
    inputs: &StoryAssessmentInputs<'_>,
) -> Vec<TargetObservation> {
    let mut observations = Vec::new();
    for id in &inputs.request.targets.grammar {
        let rules: Vec<_> = inputs
            .inventory
            .grammar_bindings
            .iter()
            .filter(|binding| &binding.declaration_id == id)
            .map(|binding| binding.rule)
            .collect();
        let mut spans = Vec::new();
        let mut uncertainties = Vec::new();
        if rules.is_empty() || assessed.construction.is_none() {
            uncertainties.push(TargetUncertainty {
                span: 0..analysis.sentence.text().len(),
                scope: TargetUncertaintyScope::SentenceCoverage,
                reason: if rules.is_empty() {
                    TargetUncertaintyReason::NoGrammarBinding
                } else {
                    TargetUncertaintyReason::UnsupportedConstruction
                },
                inventory_entries: Vec::new(),
            });
        } else if let Some(construction) = &assessed.construction {
            for (rule, span) in construction
                .rules
                .iter()
                .filter(|(rule, _)| rules.contains(rule))
            {
                if *rule == crate::grammar::GrammarRule::ObjectWo
                    && let Some(object) = &construction.object
                    && assessed.lexical[object.lexical_unit].direct_object_evidence()
                        != DirectObjectEvidence::Confirmed
                {
                    let evidence = &assessed.lexical[object.lexical_unit];
                    uncertainties.push(TargetUncertainty {
                        span: analysis.units[object.lexical_unit].token.span.clone(),
                        scope: TargetUncertaintyScope::TargetOccurrence,
                        reason: TargetUncertaintyReason::DirectObject(
                            evidence.direct_object_evidence(),
                        ),
                        inventory_entries: evidence.inventory_entries.clone(),
                    });
                } else {
                    spans.push(span.clone());
                }
            }
        }
        observations.push(TargetObservation::from_evidence(
            TargetKind::Grammar,
            id,
            spans,
            uncertainties,
        ));
    }
    observations
}

#[cfg(test)]
#[path = "assessment_tests.rs"]
mod tests;
