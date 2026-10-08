use super::{
    Check, CheckOutcome, CheckState, Finding, VocabularyEntry, morphology::reading_matches,
};
use yomibu_core::domain::evaluation::{DirectObjectEvidence, EvaluationBasis, LexicalUncertainty};
use yomibu_core::domain::{
    analysis::{SentenceAnalysis, Token},
    inventory::{InventoryWord, LearnerInventory, analyzer_reading},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LexicalStatus {
    Exempt,
    Available,
    OutsidePermissions,
    Unresolved(LexicalUncertainty),
}

pub(crate) enum LexicalPermissions<'a> {
    Explicit(&'a [VocabularyEntry]),
    Inventory(&'a LearnerInventory),
}

pub(crate) struct LexicalEvidence {
    pub inventory_entries: Vec<String>,
    pub status: LexicalStatus,
    direct_object: Option<bool>,
}

impl LexicalEvidence {
    pub fn direct_object_evidence(&self) -> DirectObjectEvidence {
        match self.status {
            LexicalStatus::Available => match self.direct_object {
                Some(true) => DirectObjectEvidence::Confirmed,
                Some(false) => DirectObjectEvidence::NotAsserted,
                None => DirectObjectEvidence::Unknown,
            },
            LexicalStatus::Unresolved(reason) => DirectObjectEvidence::Unresolved(reason),
            _ => DirectObjectEvidence::Unavailable,
        }
    }
}

impl LexicalPermissions<'_> {
    pub fn basis(&self) -> EvaluationBasis {
        match self {
            Self::Explicit(_) => EvaluationBasis::ExplicitWordUses,
            Self::Inventory(_) => EvaluationBasis::FullLearnerInventory,
        }
    }

    pub fn resolve(&self, token: &Token, text: &str) -> LexicalEvidence {
        let (mut status, direct_object, inventory_entries) = match self {
            Self::Explicit(words) => {
                let mut matches = words
                    .iter()
                    .filter(|word| word.written_form == token.dictionary_form);
                let first = matches.next();
                let status = match first {
                    None => LexicalStatus::OutsidePermissions,
                    Some(first) if matches.any(|word| word != first) => {
                        LexicalStatus::Unresolved(LexicalUncertainty::CompetingUses)
                    }
                    Some(first) if !reading_matches(&first.reading, token, text) => {
                        LexicalStatus::Unresolved(LexicalUncertainty::ReadingMismatch)
                    }
                    Some(_) => LexicalStatus::Available,
                };
                (status, first.map(|word| word.direct_object), Vec::new())
            }
            Self::Inventory(inventory) => {
                let matches: Vec<_> = inventory
                    .vocabulary
                    .iter()
                    .filter(|word| word.written_form == token.dictionary_form)
                    .collect();
                let status = match matches.as_slice() {
                    [] => LexicalStatus::OutsidePermissions,
                    [word] => inventory_use_status(word, token, text),
                    _ => LexicalStatus::Unresolved(LexicalUncertainty::CompetingIdentities),
                };
                (
                    status,
                    matches.first().and_then(|word| word.direct_object),
                    matches.iter().map(|word| word.id.clone()).collect(),
                )
            }
        };
        if !token.out_of_vocabulary && is_function_word_or_punctuation(token) {
            status = LexicalStatus::Exempt;
        } else if token.part_of_speech[0] == "空白" {
            status = LexicalStatus::Unresolved(LexicalUncertainty::Whitespace);
        } else if token.out_of_vocabulary {
            status = LexicalStatus::Unresolved(LexicalUncertainty::OutOfDictionary);
        }
        LexicalEvidence {
            inventory_entries,
            status,
            direct_object,
        }
    }
}

fn inventory_use_status(word: &InventoryWord, token: &Token, text: &str) -> LexicalStatus {
    let uncertainty = match (word.readings.as_slice(), word.meanings.as_slice()) {
        ([], _) => Some(LexicalUncertainty::MissingReading),
        ([_, _, ..], _) => Some(LexicalUncertainty::ReadingAlternatives),
        (_, []) => Some(LexicalUncertainty::MissingMeaning),
        (_, [_, _, ..]) => Some(LexicalUncertainty::MeaningAlternatives),
        ([reading], [_]) => (!reading_matches(&analyzer_reading(reading), token, text))
            .then_some(LexicalUncertainty::ReadingMismatch),
    };
    uncertainty.map_or(LexicalStatus::Available, LexicalStatus::Unresolved)
}

pub(crate) fn check_vocabulary(
    analysis: &SentenceAnalysis<'_>,
    evidence: &[LexicalEvidence],
    basis: EvaluationBasis,
) -> Check {
    let mut findings = Vec::new();
    let mut outcome = CheckOutcome::Pass;
    for (unit, evidence) in analysis.units.iter().zip(evidence) {
        let reason = match (basis, evidence.status) {
            (_, LexicalStatus::Exempt | LexicalStatus::Available) => continue,
            (EvaluationBasis::ExplicitWordUses, LexicalStatus::OutsidePermissions) => {
                "whole word is not permitted"
            }
            (EvaluationBasis::FullLearnerInventory, LexicalStatus::OutsidePermissions) => {
                "whole word is outside the learner inventory"
            }
            (EvaluationBasis::ExplicitWordUses, LexicalStatus::Unresolved(reason)) => {
                match reason {
                    LexicalUncertainty::Whitespace => {
                        "whitespace is outside the bounded lexical construction"
                    }
                    LexicalUncertainty::OutOfDictionary => {
                        "out-of-dictionary identity is unresolved"
                    }
                    _ => "reading or sense alternatives are unresolved",
                }
            }
            (EvaluationBasis::FullLearnerInventory, LexicalStatus::Unresolved(reason)) => {
                match reason {
                    LexicalUncertainty::Whitespace | LexicalUncertainty::OutOfDictionary => {
                        "lexical identity is unresolved"
                    }
                    LexicalUncertainty::CompetingIdentities => {
                        "competing inventory identities are unresolved"
                    }
                    _ => "reading or meaning alternatives are unresolved",
                }
            }
        };
        findings.push(Finding {
            span: unit.token.span.clone(),
            reason,
        });
        if evidence.status == LexicalStatus::OutsidePermissions {
            outcome = CheckOutcome::Fail;
        } else if outcome != CheckOutcome::Fail {
            outcome = CheckOutcome::Inconclusive;
        }
    }
    Check {
        state: CheckState::Completed(outcome),
        findings,
        coverage: coverage(basis),
    }
}

pub(super) fn is_function_word_or_punctuation(token: &Token) -> bool {
    token
        .part_of_speech
        .first()
        .is_some_and(|pos| ["助詞", "助動詞", "補助記号"].contains(&pos.as_str()))
}

fn coverage(basis: EvaluationBasis) -> &'static str {
    match basis {
        EvaluationBasis::ExplicitWordUses => "whole-word permissions; contextual sense unassessed",
        EvaluationBasis::FullLearnerInventory => {
            "full learner inventory; contextual reading/sense unassessed"
        }
    }
}
