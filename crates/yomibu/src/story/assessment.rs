//! Full-inventory assessment and target observations.

use std::ops::Range;

use serde::Serialize;

use super::{
    StoryError, StoryRequest, StoryVocabularySelection, TargetKind, TargetObservation, TargetState,
    TargetUncertainty, TargetUncertaintyReason, TargetUncertaintyScope,
};
use crate::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::{Sentence, SentenceAnalysis},
    candidate::{CandidateAssessment, CandidateError, GeneratedCandidates, GeneratedPassage},
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

#[derive(Debug, Serialize)]
pub struct StoryCandidateAssessment<'a> {
    pub assessment: CandidateAssessment<'a>,
    pub targets: Vec<TargetObservation>,
    pub plan_departures: Vec<PlanDeparture>,
}

impl StoryCandidateAssessment<'_> {
    pub(super) fn into_owned(self) -> StoryCandidateAssessment<'static> {
        let assessment = match self.assessment {
            CandidateAssessment::NotRun => CandidateAssessment::NotRun,
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

#[derive(Debug, Serialize)]
pub struct StorySentenceAssessment {
    pub span: Range<usize>,
    pub assessment: StoryCandidateAssessment<'static>,
}
#[derive(Debug, Serialize)]
pub struct StoryPassageAssessment {
    pub sentences: Vec<StorySentenceAssessment>,
    pub targets: Vec<TargetObservation>,
    pub plan_departures: Vec<PlanDeparture>,
}

pub fn assess_passages(
    passages: &[GeneratedPassage],
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: Option<&SudachiAnalyzer>,
) -> Vec<StoryPassageAssessment> {
    passages
        .iter()
        .map(|passage| {
            let sentences: Vec<_> = passage
                .sentence_spans
                .iter()
                .map(|span| {
                    let assessment = match analyzer {
                        Some(analyzer) => match passage.text.get(span.clone()) {
                            Some(text) => assess_candidate(text, inputs, analyzer),
                            None => failed_candidate(
                                None,
                                CandidateError::Analysis(
                                    crate::adapters::sudachi::AnalysisError::InvalidSpan,
                                ),
                                inputs,
                            ),
                        },
                        None => StoryCandidateAssessment {
                            assessment: CandidateAssessment::NotRun,
                            targets: unrun_targets(inputs),
                            plan_departures: Vec::new(),
                        },
                    };
                    StorySentenceAssessment {
                        span: span.clone(),
                        assessment: assessment.into_owned(),
                    }
                })
                .collect();
            let mut targets = unrun_targets(inputs);
            for target in &mut targets {
                let mut spans = Vec::new();
                let mut uncertainties = Vec::new();
                let mut ran = false;
                for sentence in &sentences {
                    if let Some(observed) =
                        sentence.assessment.targets.iter().find(|observed| {
                            observed.id == target.id && observed.kind == target.kind
                        })
                    {
                        ran |= observed.state != TargetState::NotRun;
                        spans.extend(observed.spans.iter().map(|span| {
                            span.start + sentence.span.start..span.end + sentence.span.start
                        }));
                        uncertainties.extend(observed.uncertainties.iter().map(|uncertainty| {
                            TargetUncertainty {
                                span: uncertainty.span.start + sentence.span.start
                                    ..uncertainty.span.end + sentence.span.start,
                                scope: uncertainty.scope,
                                reason: uncertainty.reason,
                                inventory_entries: uncertainty.inventory_entries.clone(),
                            }
                        }));
                        if observed.state == TargetState::NotRun && analyzer.is_some() {
                            uncertainties.push(TargetUncertainty {
                                span: sentence.span.clone(),
                                scope: TargetUncertaintyScope::SentenceCoverage,
                                reason: TargetUncertaintyReason::AssessmentUnavailable,
                                inventory_entries: Vec::new(),
                            });
                        }
                    }
                }
                if ran {
                    *target = TargetObservation::from_evidence(
                        target.kind,
                        &target.id,
                        spans,
                        uncertainties,
                    );
                }
            }
            let plan_departures = sentences
                .iter()
                .flat_map(|sentence| {
                    sentence
                        .assessment
                        .plan_departures
                        .iter()
                        .map(|departure| PlanDeparture {
                            span: departure.span.start + sentence.span.start
                                ..departure.span.end + sentence.span.start,
                            inventory_entries: departure.inventory_entries.clone(),
                        })
                })
                .collect();
            StoryPassageAssessment {
                sentences,
                targets,
                plan_departures,
            }
        })
        .collect()
}

pub fn assess_candidates<'a>(
    generated: &'a GeneratedCandidates,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: &SudachiAnalyzer,
) -> Vec<StoryCandidateAssessment<'a>> {
    generated
        .passages()
        .iter()
        .flat_map(|passage| {
            passage
                .sentence_spans
                .iter()
                .map(move |span| assess_candidate(&passage.text[span.clone()], inputs, analyzer))
        })
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
    let targets = unrun_targets(inputs);
    StoryCandidateAssessment {
        assessment: CandidateAssessment::ExecutionError { analysis, error },
        targets,
        plan_departures: Vec::new(),
    }
}

fn unrun_targets(inputs: &StoryAssessmentInputs<'_>) -> Vec<TargetObservation> {
    [
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
    .collect()
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
                    && assessed.lexical[object.verb_unit].direct_object_evidence()
                        != DirectObjectEvidence::Confirmed
                {
                    let evidence = &assessed.lexical[object.verb_unit];
                    uncertainties.push(TargetUncertainty {
                        span: analysis.units[object.verb_unit].token.span.clone(),
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
