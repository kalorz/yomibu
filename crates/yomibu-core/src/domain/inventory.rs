//! Source-independent material available for practice, with explicit uncertainty.
use crate::domain::{
    grammar::GrammarRule,
    knowledge::{ExclusionReason, KnowledgeDecision, LearnerKnowledgePolicy},
    source::{LexicalContent, WaniKaniSyncData},
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
    /// Hiragana-to-katakana comparison form; original source readings stay intact.
    pub fn analyzer_readings(&self) -> Vec<String> {
        self.readings
            .iter()
            .map(|reading| analyzer_reading(reading))
            .collect()
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
    pub subject_id: u64,
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
    #[error("Invalid WaniKani source: {0}")]
    Source(#[from] crate::domain::source::ValidationError),
}
impl LearnerInventory {
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
    pub fn from_wanikani(
        source: &WaniKaniSyncData,
        policy: &LearnerKnowledgePolicy,
    ) -> Result<Self, InventoryError> {
        let knowledge = policy.derive(source)?;
        let mut result = Self {
            vocabulary: Vec::new(),
            grammar_declarations: Vec::new(),
            grammar_bindings: Vec::new(),
            excluded: Vec::new(),
        };
        for item in knowledge.materials {
            if let KnowledgeDecision::Excluded(reason) = item.decision {
                result.excluded.push(ExcludedMaterial {
                    subject_id: item.subject_id,
                    reason,
                });
                continue;
            }
            let Some(subject) = item.material else {
                continue;
            };
            let readings = match &subject.lexical {
                LexicalContent::Kanji { .. } => continue,
                LexicalContent::Vocabulary { readings, .. } => readings
                    .iter()
                    .filter(|r| r.accepted_answer)
                    .map(|r| r.reading.clone())
                    .collect(),
                LexicalContent::KanaVocabulary { .. } => Vec::new(),
            };
            result.vocabulary.push(InventoryWord {
                id: format!("wanikani:{}", subject.id),
                written_form: subject.characters.clone(),
                readings,
                meanings: subject
                    .meanings
                    .iter()
                    .filter(|m| m.accepted_answer)
                    .map(|m| m.meaning.clone())
                    .collect(),
                direct_object: None,
                origin: SourceOrigin::Wanikani {
                    subject_id: subject.id,
                },
            });
        }
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
            if !text(&word.id, 128)
                || !ids.insert(&word.id)
                || !text(&word.written_form, 256)
                || word.readings.len() > 32
                || word.meanings.len() > 32
                || word.readings.iter().any(|r| !text(r, 256))
                || word.meanings.iter().any(|m| !text(m, 1024))
            {
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
