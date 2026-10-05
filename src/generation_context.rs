//! Synchronous focused context from explicit permissions, never source eligibility.

use crate::{
    evaluation::{EvaluationBindings, EvaluationError, GrammarRule, VocabularyEntry},
    grammar::GrammarDeclarations,
};
use serde::Serialize;
use std::{collections::BTreeMap, num::NonZeroUsize};

pub const PERMISSION_FILE_LIMIT: usize = 4_194_304;

/// A positive local entry number. It is not a source identity or a permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct VocabularyEntryId(NonZeroUsize);

impl VocabularyEntryId {
    pub fn new(entry: usize) -> Result<Self, ContextError> {
        NonZeroUsize::new(entry)
            .map(Self)
            .ok_or(ContextError::InvalidFocusEntry)
    }
    pub fn get(self) -> usize {
        self.0.get()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Focus entry must be a positive one-based number.")]
    InvalidFocusEntry,
    #[error("{field} exceeds its G2 limit of {limit} (actual {actual}).")]
    Limit {
        field: &'static str,
        limit: usize,
        actual: usize,
    },
    #[error(transparent)]
    Bindings(#[from] EvaluationError),
    #[error("Focused context is unavailable; see the declared prerequisite decisions.")]
    Unavailable {
        focus_entry: usize,
        situations: Vec<SituationDecision>,
    },
    #[error("Selected context violates membership, focus, slot, order, or size invariants.")]
    InvalidSelection,
}

fn bounded(field: &'static str, actual: usize, limit: usize) -> Result<(), ContextError> {
    if actual > limit {
        return Err(ContextError::Limit {
            field,
            actual,
            limit,
        });
    }
    Ok(())
}

/// Validate the entire inventory, including entries that will not be transmitted.
fn validate_inputs(
    grammar: &GrammarDeclarations,
    permissions: &EvaluationBindings,
) -> Result<(), ContextError> {
    bounded("Vocabulary entries", permissions.vocabulary.len(), 10_000)?;
    bounded("Grammar declarations", grammar.entries().len(), 128)?;
    bounded("Grammar bindings", permissions.grammar.len(), 512)?;
    for word in &permissions.vocabulary {
        bounded("Written form bytes", word.written_form.len(), 256)?;
        bounded("Reading bytes", word.reading.len(), 256)?;
        bounded("Sense bytes", word.sense.len(), 1024)?;
    }
    for entry in grammar.entries() {
        bounded("Grammar description bytes", entry.description.len(), 1024)?;
    }
    permissions.validate(grammar)?;
    Ok(())
}

pub const SELECTOR_REVISION: &str = "g2-situations-v1";
pub const LIMITATIONS: [&str; 5] = [
    "synthetic_situation_associations",
    "contextual_reading_and_sense_not_assessed",
    "situation_quality_not_assessed",
    "comprehension_not_assessed",
    "not_accepted_exercises",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotRole {
    Predicate,
    Participant,
    Object,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InclusionReason {
    ExplicitFocus,
    FirstAvailableDeclaredAlternative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelectedEntry<'a> {
    pub entry: VocabularyEntryId,
    pub role: SlotRole,
    pub reason: InclusionReason,
    pub vocabulary: &'a VocabularyEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    DuplicateOfSelectedTuple,
    AmbiguousAssociation,
    UnusedSlotAlternative,
    UnsupportedTupleVariant,
    OutsideChosenSituation,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ExcludedEntries {
    pub reason: ExclusionReason,
    pub entries: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum UnavailableReason {
    FocusNotFound,
    AmbiguousFocus { entries: Vec<usize> },
    UnsupportedFocus,
    MissingSupport { role: SlotRole },
    AmbiguousSupport { role: SlotRole, entries: Vec<usize> },
    MissingGrammar { rule: GrammarRule },
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SituationDecision {
    pub id: &'static str,
    pub feasible: bool,
    pub reasons: Vec<UnavailableReason>,
}

#[derive(Debug, Serialize)]
pub struct Situation {
    pub id: &'static str,
    pub description: &'static str,
    pub required_grammar: &'static [GrammarRule],
}

// Original authored suggestions. These exact associations grant no permissions.
struct Tuple(&'static str, &'static str, &'static str, bool);
impl Tuple {
    fn matches(&self, word: &VocabularyEntry) -> bool {
        word.written_form == self.0
            && word.reading == self.1
            && word.sense == self.2
            && word.direct_object == self.3
    }
}
struct Slot {
    role: SlotRole,
    alternatives: &'static [Tuple],
}
struct Definition {
    situation: Situation,
    slots: &'static [Slot],
}
static DEFINITIONS: [Definition; 3] = [
    Definition {
        situation: Situation {
            id: "pet-rest",
            description: "A pet rests. Describe the selected pet sleeping.",
            required_grammar: &[GrammarRule::TopicWa, GrammarRule::PoliteNonPast],
        },
        slots: &[
            Slot {
                role: SlotRole::Predicate,
                alternatives: &[Tuple("寝る", "ネル", "sleep", false)],
            },
            Slot {
                role: SlotRole::Participant,
                alternatives: &[
                    Tuple("猫", "ネコ", "cat", false),
                    Tuple("犬", "イヌ", "dog", false),
                ],
            },
        ],
    },
    Definition {
        situation: Situation {
            id: "pet-walk",
            description: "A pet moves about. Describe the selected pet walking.",
            required_grammar: &[GrammarRule::TopicWa, GrammarRule::PoliteNonPast],
        },
        slots: &[
            Slot {
                role: SlotRole::Predicate,
                alternatives: &[Tuple("歩く", "アルク", "walk", false)],
            },
            Slot {
                role: SlotRole::Participant,
                alternatives: &[
                    Tuple("猫", "ネコ", "cat", false),
                    Tuple("犬", "イヌ", "dog", false),
                ],
            },
        ],
    },
    Definition {
        situation: Situation {
            id: "book-reading",
            description: "A person reads a book. Describe the selected person reading the selected book.",
            required_grammar: &[
                GrammarRule::TopicWa,
                GrammarRule::ObjectWo,
                GrammarRule::PoliteNonPast,
            ],
        },
        slots: &[
            Slot {
                role: SlotRole::Predicate,
                alternatives: &[Tuple("読む", "ヨム", "read", true)],
            },
            Slot {
                role: SlotRole::Participant,
                alternatives: &[
                    Tuple("学生", "ガクセイ", "student", false),
                    Tuple("先生", "センセイ", "teacher", false),
                ],
            },
            Slot {
                role: SlotRole::Object,
                alternatives: &[Tuple("本", "ホン", "book", false)],
            },
        ],
    },
];

/// Borrows the immutable complete permissions; selected references are guidance only.
#[derive(Debug)]
pub struct GenerationContext<'a> {
    grammar: &'a GrammarDeclarations,
    permissions: &'a EvaluationBindings,
    focus: VocabularyEntryId,
    situation: &'static Situation,
    selected: Vec<SelectedEntry<'a>>,
    excluded: Vec<ExcludedEntries>,
    situations: Vec<SituationDecision>,
}
impl<'a> GenerationContext<'a> {
    pub fn grammar(&self) -> &'a GrammarDeclarations {
        self.grammar
    }
    pub fn permissions(&self) -> &'a EvaluationBindings {
        self.permissions
    }
    pub fn focus_entry(&self) -> VocabularyEntryId {
        self.focus
    }
    pub fn situation(&self) -> &'static Situation {
        self.situation
    }
    pub fn selected(&self) -> &[SelectedEntry<'a>] {
        &self.selected
    }
    pub fn excluded(&self) -> &[ExcludedEntries] {
        &self.excluded
    }
    pub fn situations(&self) -> &[SituationDecision] {
        &self.situations
    }

    /// Independently check the produced references, slot coverage, ordering and bounds.
    pub fn validate(&self) -> Result<(), ContextError> {
        let invalid = || ContextError::InvalidSelection;
        let definition = DEFINITIONS
            .iter()
            .find(|d| std::ptr::eq(&d.situation, self.situation))
            .ok_or_else(invalid)?;
        if self.selected.len() != definition.slots.len() || self.selected.len() > 3 {
            return Err(invalid());
        }
        let first = self.selected.first().ok_or_else(invalid)?;
        if first.entry != self.focus || first.reason != InclusionReason::ExplicitFocus {
            return Err(invalid());
        }
        let mut remaining_roles = definition
            .slots
            .iter()
            .map(|s| s.role)
            .filter(|role| *role != first.role);
        for (index, selected) in self.selected.iter().enumerate() {
            if self
                .permissions
                .vocabulary
                .get(selected.entry.get() - 1)
                .is_none_or(|word| !std::ptr::eq(word, selected.vocabulary))
                || self.selected[..index]
                    .iter()
                    .any(|s| s.vocabulary == selected.vocabulary || s.role == selected.role)
                || !definition.slots.iter().any(|slot| {
                    slot.role == selected.role
                        && slot
                            .alternatives
                            .iter()
                            .any(|t| t.matches(selected.vocabulary))
                })
                || (index > 0
                    && (remaining_roles.next() != Some(selected.role)
                        || selected.reason != InclusionReason::FirstAvailableDeclaredAlternative))
            {
                return Err(invalid());
            }
        }
        if self
            .situation
            .required_grammar
            .iter()
            .any(|rule| !self.permissions.grammar.iter().any(|b| b.rule == *rule))
        {
            return Err(invalid());
        }
        Ok(())
    }
}

pub fn select_context<'a>(
    grammar: &'a GrammarDeclarations,
    permissions: &'a EvaluationBindings,
    focus: VocabularyEntryId,
) -> Result<GenerationContext<'a>, ContextError> {
    validate_inputs(grammar, permissions)?;
    let focus_word = permissions.vocabulary.get(focus.get() - 1);
    let ambiguous = ambiguous_forms(permissions);
    let mut situations = Vec::new();
    let mut chosen = None;
    for definition in &DEFINITIONS {
        let mut reasons = Vec::new();
        let focus_slot = focus_word.and_then(|word| {
            definition
                .slots
                .iter()
                .find(|slot| slot.alternatives.iter().any(|t| t.matches(word)))
        });
        let mut selected = Vec::new();
        if let Some(entries) = focus_word.and_then(|w| ambiguous.get(w.written_form.as_str())) {
            reasons.push(UnavailableReason::AmbiguousFocus {
                entries: entries.clone(),
            });
        } else if let (Some(word), Some(slot)) = (focus_word, focus_slot) {
            selected.push(SelectedEntry {
                entry: focus,
                role: slot.role,
                reason: InclusionReason::ExplicitFocus,
                vocabulary: word,
            });
            for support in definition
                .slots
                .iter()
                .filter(|support| support.role != slot.role)
            {
                let found = support
                    .alternatives
                    .iter()
                    .filter(|t| !ambiguous.contains_key(t.0))
                    .find_map(|t| {
                        permissions
                            .vocabulary
                            .iter()
                            .enumerate()
                            .find(|(_, w)| t.matches(w))
                    });
                match found {
                    Some((index, vocabulary)) => selected.push(SelectedEntry {
                        entry: VocabularyEntryId::new(index + 1)?,
                        role: support.role,
                        reason: InclusionReason::FirstAvailableDeclaredAlternative,
                        vocabulary,
                    }),
                    None => {
                        let mut entries: Vec<_> = support
                            .alternatives
                            .iter()
                            .filter(|t| permissions.vocabulary.iter().any(|w| t.matches(w)))
                            .filter_map(|t| ambiguous.get(t.0))
                            .flatten()
                            .copied()
                            .collect();
                        entries.sort_unstable();
                        entries.dedup();
                        reasons.push(if entries.is_empty() {
                            UnavailableReason::MissingSupport { role: support.role }
                        } else {
                            UnavailableReason::AmbiguousSupport {
                                role: support.role,
                                entries,
                            }
                        });
                    }
                }
            }
            for rule in definition.situation.required_grammar {
                if !permissions
                    .grammar
                    .iter()
                    .any(|binding| binding.rule == *rule)
                {
                    reasons.push(UnavailableReason::MissingGrammar { rule: *rule });
                }
            }
        } else {
            reasons.push(if focus_word.is_none() {
                UnavailableReason::FocusNotFound
            } else {
                UnavailableReason::UnsupportedFocus
            });
        }
        let feasible = reasons.is_empty();
        situations.push(SituationDecision {
            id: definition.situation.id,
            feasible,
            reasons,
        });
        if feasible && chosen.is_none() {
            chosen = Some((definition, selected));
        }
    }
    let Some((definition, selected)) = chosen else {
        return Err(ContextError::Unavailable {
            focus_entry: focus.get(),
            situations,
        });
    };
    let excluded = exclusions(permissions, definition, &selected, &ambiguous);
    let context = GenerationContext {
        grammar,
        permissions,
        focus,
        situation: &definition.situation,
        selected,
        excluded,
        situations,
    };
    context.validate()?;
    Ok(context)
}

fn exclusions(
    permissions: &EvaluationBindings,
    definition: &Definition,
    selected: &[SelectedEntry<'_>],
    ambiguous: &BTreeMap<&str, Vec<usize>>,
) -> Vec<ExcludedEntries> {
    use ExclusionReason::*;
    let mut groups: Vec<_> = [
        DuplicateOfSelectedTuple,
        AmbiguousAssociation,
        UnusedSlotAlternative,
        UnsupportedTupleVariant,
        OutsideChosenSituation,
    ]
    .into_iter()
    .map(|reason| ExcludedEntries {
        reason,
        entries: Vec::new(),
    })
    .collect();
    for (index, word) in permissions.vocabulary.iter().enumerate() {
        if selected.iter().any(|s| s.entry.get() == index + 1) {
            continue;
        }
        let category = if selected.iter().any(|s| s.vocabulary == word) {
            0
        } else if ambiguous.contains_key(word.written_form.as_str())
            && definition
                .slots
                .iter()
                .flat_map(|s| s.alternatives)
                .any(|t| t.0 == word.written_form)
        {
            1
        } else if definition
            .slots
            .iter()
            .flat_map(|s| s.alternatives)
            .any(|t| t.matches(word))
        {
            2
        } else if definition
            .slots
            .iter()
            .flat_map(|s| s.alternatives)
            .any(|t| t.0 == word.written_form)
        {
            3
        } else {
            4
        };
        groups[category].entries.push(index + 1);
    }
    groups.retain(|g| !g.entries.is_empty());
    groups
}

// Group once so large duplicate inventories do not require quadratic comparisons.
fn ambiguous_forms(permissions: &EvaluationBindings) -> BTreeMap<&str, Vec<usize>> {
    let mut forms: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, word) in permissions.vocabulary.iter().enumerate() {
        forms.entry(&word.written_form).or_default().push(index + 1);
    }
    forms.retain(|_, entries| {
        entries
            .iter()
            .any(|id| permissions.vocabulary[*id - 1] != permissions.vocabulary[entries[0] - 1])
    });
    forms
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independently_rejects_invalid_produced_membership_focus_slots_order_and_size() {
        let grammar =
            GrammarDeclarations::from_descriptions(["topic", "polite", "object"]).unwrap();
        let permissions = EvaluationBindings {
            vocabulary: [
                ("読む", "ヨム", "read", true),
                ("学生", "ガクセイ", "student", false),
                ("本", "ホン", "book", false),
            ]
            .map(|(w, r, s, d)| VocabularyEntry {
                written_form: w.into(),
                reading: r.into(),
                sense: s.into(),
                direct_object: d,
            })
            .to_vec(),
            grammar: [
                GrammarRule::TopicWa,
                GrammarRule::PoliteNonPast,
                GrammarRule::ObjectWo,
            ]
            .into_iter()
            .enumerate()
            .map(|(i, rule)| crate::evaluation::GrammarBinding {
                declaration_id: i + 1,
                rule,
            })
            .collect(),
        };
        let foreign = permissions.vocabulary[0].clone();
        for corruption in 0..7 {
            let mut context =
                select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap();
            match corruption {
                0 => context.selected[0].vocabulary = &foreign,
                1 => context.selected[0].entry = VocabularyEntryId::new(2).unwrap(),
                2 => context.selected[1].role = SlotRole::Object,
                3 => context.selected.swap(1, 2),
                4 => {
                    context.selected.pop();
                }
                5 => context.selected.push(context.selected[0].clone()),
                _ => context.selected[1].reason = InclusionReason::ExplicitFocus,
            }
            assert!(matches!(
                context.validate(),
                Err(ContextError::InvalidSelection)
            ));
        }
    }
}
