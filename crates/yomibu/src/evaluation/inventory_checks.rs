//! Full-inventory lexical checks and conservative single-use projection.

use super::structural_checks::{evaluate, reading_matches};
use super::{
    CheckOutcome, Evaluation, EvaluationBindings, EvaluationError, VocabularyEntry, combine,
    passed, problem,
};
use crate::{analysis::SentenceAnalysis, grammar::GrammarDeclarations, inventory::InventoryWord};

/// Only an explicitly represented single use can feed the historical structural
/// checker. Alternatives/missing evidence remain in the full inventory check.
pub(crate) fn single_use(word: &InventoryWord) -> Option<VocabularyEntry> {
    if word.readings.len() != 1 || word.meanings.len() != 1 {
        return None;
    }
    Some(VocabularyEntry {
        written_form: word.written_form.clone(),
        reading: word.analyzer_readings().remove(0),
        sense: word.meanings[0].clone(),
        direct_object: word.direct_object == Some(true),
    })
}

/// Reuse the frozen structural checks while checking lexical availability against
/// the complete source-independent inventory. No reading/sense pairs are invented.
pub(crate) fn evaluate_inventory(
    analysis: &SentenceAnalysis<'_>,
    grammar: &GrammarDeclarations,
    structural_bindings: &EvaluationBindings,
    inventory: &crate::inventory::LearnerInventory,
) -> Result<Evaluation, EvaluationError> {
    let mut result = evaluate(analysis, grammar, structural_bindings)?;
    let mut vocabulary = passed("full learner inventory; contextual reading/sense unassessed");
    for unit in &analysis.units {
        let t = &unit.token;
        if !t.out_of_vocabulary
            && ["助詞", "助動詞", "補助記号"].contains(&t.part_of_speech[0].as_str())
        {
            continue;
        }
        let entries: Vec<_> = inventory
            .vocabulary
            .iter()
            .filter(|w| w.written_form == t.dictionary_form)
            .collect();
        let uncertainty = if t.out_of_vocabulary || t.part_of_speech[0] == "空白" {
            Some("lexical identity is unresolved")
        } else if entries.is_empty() {
            vocabulary = combine(
                vocabulary,
                problem(
                    CheckOutcome::Fail,
                    t.span.clone(),
                    "whole word is outside the learner inventory",
                ),
            );
            continue;
        } else if entries.len() != 1 {
            Some("competing inventory identities are unresolved")
        } else if single_use(entries[0])
            .is_none_or(|w| !reading_matches(&w, t, analysis.sentence.text()))
        {
            Some("reading or meaning alternatives are unresolved")
        } else {
            None
        };
        if let Some(reason) = uncertainty {
            vocabulary = combine(
                vocabulary,
                problem(CheckOutcome::Inconclusive, t.span.clone(), reason),
            );
        }
    }
    vocabulary.coverage = "full learner inventory; contextual reading/sense unassessed";
    result.vocabulary = vocabulary;
    Ok(result)
}
