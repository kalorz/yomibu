//! Deterministic manual word selection, without storage or linguistic analysis.

use std::collections::HashSet;
use thiserror::Error;

/// One user-declared use; text, reading, and meaning stay associated.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct WordEntry {
    pub text: String,
    pub reading: String,
    pub meaning: String,
}

/// A transient selection borrowing its inputs, not a validated exercise.
#[derive(Debug, PartialEq, Eq)]
pub struct Preview<'a> {
    pub selected: &'a [WordEntry],
    pub grammar: &'a [String],
    pub checks: PreviewChecks,
}

/// G0 checks only exact entry membership and count, never linguistic truth.
#[derive(Debug, PartialEq, Eq)]
pub struct PreviewChecks {
    pub membership: CheckOutcome,
    pub count: CheckOutcome,
    pub grammar: CheckOutcome,
    /// Includes the correctness of readings, meanings, and naturalness.
    pub linguistic_correctness: CheckOutcome,
}

/// An explicit outcome for the limited checks performed by manual preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    Pass,
    Fail,
    NotAssessed,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PreviewError {
    #[error(
        "Requested count must be positive and at most {available} supplied entries; got {requested}."
    )]
    InvalidCount { requested: usize, available: usize },
    /// Entry numbers are one-based; `field` identifies the required word field.
    #[error("Word entry {entry} has blank {field}.")]
    BlankWordField { entry: usize, field: &'static str },
    /// Grammar entry numbers are one-based.
    #[error("Grammar entry {entry} has a blank description.")]
    BlankGrammar { entry: usize },
}

/// Validate all inputs, select the first `take` entries, then check the result.
///
/// Order, duplicates, and structured input content are preserved verbatim. Blank
/// fields are rejected, including entries outside the selected prefix. The
/// result borrows its inputs; no store, account, runtime, or I/O is involved.
pub fn preview<'a>(
    words: &'a [WordEntry],
    grammar: &'a [String],
    take: usize,
) -> Result<Preview<'a>, PreviewError> {
    validate_inputs(words, grammar, take)?;
    let selected = &words[..take];
    let checks = check_selection(words, selected, take);
    Ok(Preview {
        selected,
        grammar,
        checks,
    })
}

fn validate_inputs(
    words: &[WordEntry],
    grammar: &[String],
    take: usize,
) -> Result<(), PreviewError> {
    if take == 0 || take > words.len() {
        return Err(PreviewError::InvalidCount {
            requested: take,
            available: words.len(),
        });
    }
    for (index, word) in words.iter().enumerate() {
        for (field, value) in [
            ("text", &word.text),
            ("reading", &word.reading),
            ("meaning", &word.meaning),
        ] {
            if value.trim().is_empty() {
                return Err(PreviewError::BlankWordField {
                    entry: index + 1,
                    field,
                });
            }
        }
    }
    for (index, description) in grammar.iter().enumerate() {
        if description.trim().is_empty() {
            return Err(PreviewError::BlankGrammar { entry: index + 1 });
        }
    }
    Ok(())
}

fn check_selection(supplied: &[WordEntry], selected: &[WordEntry], take: usize) -> PreviewChecks {
    let supplied: HashSet<_> = supplied.iter().collect();
    PreviewChecks {
        membership: if selected.iter().all(|word| supplied.contains(word)) {
            CheckOutcome::Pass
        } else {
            CheckOutcome::Fail
        },
        count: if selected.len() == take {
            CheckOutcome::Pass
        } else {
            CheckOutcome::Fail
        },
        grammar: CheckOutcome::NotAssessed,
        linguistic_correctness: CheckOutcome::NotAssessed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, reading: &str, meaning: &str) -> WordEntry {
        WordEntry {
            text: text.into(),
            reading: reading.into(),
            meaning: meaning.into(),
        }
    }

    #[test]
    fn checks_supplied_entries_independently_of_prefix_selection() {
        let supplied = [word("生", "なま", "raw"), word("生", "せい", "life")];
        let checks = check_selection(&supplied, &supplied[1..], 1);
        assert_eq!(checks.membership, CheckOutcome::Pass);
        assert_eq!(checks.count, CheckOutcome::Pass);
        assert_eq!(checks.grammar, CheckOutcome::NotAssessed);
        assert_eq!(checks.linguistic_correctness, CheckOutcome::NotAssessed);
    }

    #[test]
    fn checks_reject_unsupplied_entries_and_recombined_readings_and_meanings() {
        let supplied = [word("生", "なま", "raw"), word("生", "せい", "life")];
        for invalid in [
            word("生", "なま", "life"),
            word("生", "せい", "raw"),
            word("生", "しょう", "raw"),
            word("猫", "なま", "raw"),
            word("生", "なま", "fresh"),
        ] {
            let checks = check_selection(&supplied, &[invalid], 1);
            assert_eq!(checks.membership, CheckOutcome::Fail);
            assert_eq!(checks.count, CheckOutcome::Pass);
        }
    }

    #[test]
    fn checks_reject_wrong_counts_even_when_all_entries_are_supplied() {
        let supplied = [word("猫", "ねこ", "cat"), word("犬", "いぬ", "dog")];
        for selected in [&[][..], &supplied[..]] {
            let checks = check_selection(&supplied, selected, 1);
            assert_eq!(checks.membership, CheckOutcome::Pass);
            assert_eq!(checks.count, CheckOutcome::Fail);
        }
    }
}
