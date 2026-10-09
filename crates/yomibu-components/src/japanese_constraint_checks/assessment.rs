//! Full-inventory assessment and target observations from supplied evidence.

use super as evaluation;
use super::{LexicalStatus, SentenceAssessment};
use yomibu_core::{
    capabilities::StoryAssessor,
    domain::{
        analysis::SentenceAnalysis,
        evaluation::{DirectObjectEvidence, EvaluationError, LexicalUncertainty},
        story::{
            PlanDeparture, StoryAssessmentInputs, StoryFindings, TargetKind, TargetObservation,
            TargetUncertainty, TargetUncertaintyReason, TargetUncertaintyScope,
        },
    },
};

pub struct JapaneseConstraintChecks;

impl StoryAssessor for JapaneseConstraintChecks {
    type Error = EvaluationError;

    fn assess<'a>(
        &self,
        analysis: &'a SentenceAnalysis<'_>,
        inputs: &StoryAssessmentInputs<'_>,
    ) -> Result<StoryFindings<'a>, EvaluationError> {
        let assessed = evaluation::assess_inventory(analysis, inputs.inventory())?;
        let mut targets = observe_vocabulary_targets(analysis, &assessed, inputs);
        targets.extend(observe_grammar_targets(analysis, &assessed, inputs));
        let plan_departures = observe_plan_departures(analysis, &assessed, inputs);
        StoryFindings::new(
            analysis.sentence.text(),
            assessed.evaluation,
            targets,
            plan_departures,
        )
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
                    .any(|id| inputs.selected_vocabulary_ids().contains(&id.as_str()))
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
    for id in &inputs.request().targets.vocabulary {
        let word = inputs
            .inventory()
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
                } else if !supports_morphology
                    && !super::lexical::is_function_word_or_punctuation(&unit.token)
                {
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
    for id in &inputs.request().targets.grammar {
        let rules: Vec<_> = inputs
            .inventory()
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
                if *rule == yomibu_core::domain::grammar::GrammarRule::ObjectWo
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
