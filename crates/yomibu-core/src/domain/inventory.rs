//! Source-independent material available for practice, with explicit uncertainty.
use crate::domain::{
    grammar::GrammarRule,
    knowledge::{ExclusionReason, KnowledgeDecision, LearnerKnowledgePolicy, LearnerProgress},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceOrigin {
    #[default]
    Manual,
    Wanikani {
        subject_id: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryWord {
    pub id: String,
    pub written_form: String,
    pub readings: Vec<String>,
    pub meanings: Vec<String>,
    pub direct_object: Option<bool>,
    #[serde(skip_deserializing)]
    pub origin: SourceOrigin,
}

impl InventoryWord {
    pub fn validate(&self) -> Result<(), InventoryError> {
        validate_word(
            &self.id,
            &self.written_form,
            self.readings.iter().map(String::as_str),
            self.meanings.iter().map(String::as_str),
        )
    }

    /// Hiragana-to-katakana comparison form; original source readings stay intact.
    pub fn analyzer_readings(&self) -> Vec<String> {
        self.readings
            .iter()
            .map(|reading| analyzer_reading(reading))
            .collect()
    }
}

fn validate_word<'a>(
    id: &str,
    spelling: &str,
    mut readings: impl ExactSizeIterator<Item = &'a str>,
    mut meanings: impl ExactSizeIterator<Item = &'a str>,
) -> Result<(), InventoryError> {
    let text = |s: &str, limit: usize| !s.trim().is_empty() && s.len() <= limit;
    if !text(id, 128)
        || !text(spelling, 256)
        || readings.len() > 32
        || meanings.len() > 32
        || readings.any(|reading| !text(reading, 256))
        || meanings.any(|meaning| !text(meaning, 1024))
    {
        return Err(InventoryError::Invalid(
            "invalid or duplicate vocabulary entry",
        ));
    }
    Ok(())
}

/// Source alternatives without evidence linking each reading to its translations.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnresolvedVocabulary {
    pub word: InventoryWord,
    pub observation: serde_json::Value,
}

/// One evidence-backed spelling/reading and its explicitly associated translations.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VocabularyEntry {
    pub id: i64,
    pub spelling: String,
    pub reading: String,
    pub parts_of_speech: Vec<String>,
    pub is_primary_reading_in_source: Option<bool>,
    /// Identifies the supplied association evidence; no resolver is implied.
    pub mapping_method: String,
    pub has_direct_object_evidence: Option<bool>,
    pub translations: Vec<VocabularyTranslation>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VocabularyTranslation {
    pub id: i64,
    pub language_code: String,
    pub text: String,
    pub display_order: i64,
    pub is_primary_in_source: Option<bool>,
    pub origin: String,
}

impl VocabularyEntry {
    pub fn inventory_id(&self) -> String {
        format!("vocabulary_entry:{}", self.id)
    }

    pub fn into_inventory_word(mut self, origin: SourceOrigin) -> InventoryWord {
        self.translations.sort_by(|a, b| {
            (&a.language_code, a.display_order).cmp(&(&b.language_code, b.display_order))
        });
        InventoryWord {
            id: self.inventory_id(),
            written_form: self.spelling,
            readings: vec![self.reading],
            meanings: self
                .translations
                .into_iter()
                .map(|translation| translation.text)
                .collect(),
            direct_object: self.has_direct_object_evidence,
            origin,
        }
    }

    pub fn validate(&self) -> Result<(), InventoryError> {
        validate_word(
            &self.inventory_id(),
            &self.spelling,
            std::iter::once(self.reading.as_str()),
            self.translations
                .iter()
                .map(|translation| translation.text.as_str()),
        )?;
        if self.mapping_method.trim().is_empty()
            || self
                .parts_of_speech
                .iter()
                .any(|part| part.trim().is_empty())
            || self.translations.iter().any(|translation| {
                translation.language_code.trim().is_empty() || translation.origin.trim().is_empty()
            })
        {
            return Err(InventoryError::Invalid(
                "missing vocabulary association evidence",
            ));
        }
        Ok(())
    }
}

/// Unresolved source observations stay distinct from evidence-backed entries.
#[derive(Debug)]
pub enum MappedContent {
    Vocabulary {
        entries: Vec<VocabularyEntry>,
        origin: SourceOrigin,
    },
    UnresolvedVocabulary(UnresolvedVocabulary),
    NonVocabulary(SourceOrigin),
    Unavailable(SourceOrigin),
}

impl MappedContent {
    pub fn origin(&self) -> &SourceOrigin {
        match self {
            Self::Vocabulary { origin, .. }
            | Self::NonVocabulary(origin)
            | Self::Unavailable(origin) => origin,
            Self::UnresolvedVocabulary(unresolved) => &unresolved.word.origin,
        }
    }
}

pub fn analyzer_reading(reading: &str) -> String {
    reading
        .chars()
        .map(|c| {
            if ('\u{3041}'..='\u{3096}').contains(&c) {
                char::from_u32(c as u32 + 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryGrammar {
    pub id: String,
    pub description: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryGrammarBinding {
    pub declaration_id: String,
    pub rule: GrammarRule,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualInventory {
    pub version: u32,
    pub vocabulary: Vec<InventoryWord>,
    pub grammar_declarations: Vec<InventoryGrammar>,
    pub grammar_bindings: Vec<InventoryGrammarBinding>,
}
#[derive(Debug, Serialize)]
pub struct ExcludedMaterial {
    pub origin: SourceOrigin,
    pub reason: ExclusionReason,
}
#[derive(Debug, Serialize)]
pub struct LearnerInventory {
    pub vocabulary: Vec<InventoryWord>,
    pub grammar_declarations: Vec<InventoryGrammar>,
    pub grammar_bindings: Vec<InventoryGrammarBinding>,
    pub excluded: Vec<ExcludedMaterial>,
}
#[derive(Debug, thiserror::Error)]
pub enum InventoryError {
    #[error("Invalid learner inventory: {0}.")]
    Invalid(&'static str),
}
impl LearnerInventory {
    /// Prepare subjects in input order, with resolved entries ordered by ID.
    /// Translations use language/display order; unresolved alternatives stay independent.
    /// Unavailable content takes precedence over progress. Eligible non-vocabulary is omitted.
    pub fn from_mapped(
        records: impl IntoIterator<Item = (MappedContent, Option<LearnerProgress>)>,
        policy: &LearnerKnowledgePolicy,
    ) -> Result<Self, InventoryError> {
        let mut inventory = Self {
            vocabulary: Vec::new(),
            grammar_declarations: Vec::new(),
            grammar_bindings: Vec::new(),
            excluded: Vec::new(),
        };
        for (content, progress) in records {
            let decision = if matches!(content, MappedContent::Unavailable(_)) {
                KnowledgeDecision::Excluded(ExclusionReason::ContentUnavailable)
            } else {
                policy.decide(progress.as_ref())
            };
            match decision {
                KnowledgeDecision::Eligible => match content {
                    MappedContent::Vocabulary {
                        mut entries,
                        origin,
                    } => {
                        entries.sort_by_key(|entry| entry.id);
                        for entry in entries {
                            entry.validate()?;
                            inventory
                                .vocabulary
                                .push(entry.into_inventory_word(origin.clone()));
                        }
                    }
                    MappedContent::UnresolvedVocabulary(unresolved) => {
                        inventory.vocabulary.push(unresolved.word)
                    }
                    MappedContent::NonVocabulary(_) | MappedContent::Unavailable(_) => {}
                },
                KnowledgeDecision::Excluded(reason) => {
                    let origin = content.origin().clone();
                    inventory.excluded.push(ExcludedMaterial { origin, reason });
                }
            }
        }
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn from_manual(input: ManualInventory) -> Result<Self, InventoryError> {
        if input.version != 1 {
            return Err(InventoryError::Invalid("expected version 1"));
        }
        let result = Self {
            vocabulary: input.vocabulary,
            grammar_declarations: input.grammar_declarations,
            grammar_bindings: input.grammar_bindings,
            excluded: Vec::new(),
        };
        result.validate()?;
        Ok(result)
    }
    pub fn with_manual(self, input: ManualInventory) -> Result<Self, InventoryError> {
        self.merge(Self::from_manual(input)?)
    }
    pub fn merge(mut self, manual: Self) -> Result<Self, InventoryError> {
        self.vocabulary.extend(manual.vocabulary);
        self.grammar_declarations
            .extend(manual.grammar_declarations);
        self.grammar_bindings.extend(manual.grammar_bindings);
        self.excluded.extend(manual.excluded);
        self.validate()?;
        Ok(self)
    }
    pub fn validate(&self) -> Result<(), InventoryError> {
        let text = |s: &str, limit: usize| !s.trim().is_empty() && s.len() <= limit;
        if self.vocabulary.len() > 10_000
            || self.grammar_declarations.len() > 128
            || self.grammar_bindings.len() > 512
        {
            return Err(InventoryError::Invalid("collection limit exceeded"));
        }
        let mut ids = BTreeSet::new();
        for word in &self.vocabulary {
            word.validate()?;
            if !ids.insert(&word.id) {
                return Err(InventoryError::Invalid(
                    "invalid or duplicate vocabulary entry",
                ));
            }
        }
        let mut grammar_ids = BTreeSet::new();
        for g in &self.grammar_declarations {
            if !text(&g.id, 128) || !text(&g.description, 1024) || !grammar_ids.insert(&g.id) {
                return Err(InventoryError::Invalid(
                    "invalid or duplicate grammar declaration",
                ));
            }
        }
        if self
            .grammar_bindings
            .iter()
            .any(|b| !grammar_ids.contains(&b.declaration_id))
        {
            return Err(InventoryError::Invalid(
                "binding refers to a missing grammar declaration",
            ));
        }
        Ok(())
    }
}
