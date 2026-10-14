//! Source-independent material available for practice, with explicit uncertainty.
use crate::domain::{
    knowledge::grammar::GrammarRule,
    knowledge::{ExclusionReason, KnowledgeDecision, LearnerProgress, vocabulary_eligibility},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceOrigin {
    #[default]
    Manual,
    /// Provenance whose namespace and record identifier are opaque to core.
    External {
        /// Namespace identifying the external provider.
        provider: String,
        /// Record identifier within that namespace; the provider defines its stability.
        external_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryWord {
    pub id: String,
    pub spelling: String,
    pub readings: Vec<String>,
    pub meanings: Vec<String>,
    pub direct_object: Option<bool>,
    #[serde(skip_deserializing)]
    pub origin: SourceOrigin,
}

/// Mapped content retains provenance even when it cannot supply vocabulary.
#[derive(Debug)]
pub enum MappedContent {
    Vocabulary(InventoryWord),
    NonVocabulary(SourceOrigin),
    Unavailable(SourceOrigin),
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
#[derive(Debug, Serialize)]
pub struct ExcludedMaterial {
    pub origin: SourceOrigin,
    pub reason: ExclusionReason,
}
/// Material considered usable for a learner, derived from recorded progress and explicit declarations.
/// This represents eligibility, not verified mastery.
/// Also retains provenance and reasons for excluded material.
#[derive(Debug, Serialize)]
pub struct LearnerKnowledge {
    pub vocabulary: Vec<InventoryWord>,
    pub grammar_declarations: Vec<InventoryGrammar>,
    pub grammar_bindings: Vec<InventoryGrammarBinding>,
    pub excluded: Vec<ExcludedMaterial>,
}
#[derive(Debug, thiserror::Error)]
pub enum InventoryError {
    #[error("Recorded content access has expired.")]
    AccessExpired,
    #[error("Invalid learner inventory: {0}.")]
    Invalid(&'static str),
}
impl LearnerKnowledge {
    /// Prepare one learner's mapped values in input order and validate the result.
    /// Unavailable content takes precedence over progress. Eligible non-vocabulary is omitted.
    pub fn from_mapped(
        records: impl IntoIterator<Item = (MappedContent, Option<LearnerProgress>)>,
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
                vocabulary_eligibility(progress.as_ref())
            };
            match decision {
                KnowledgeDecision::Eligible => {
                    if let MappedContent::Vocabulary(word) = content {
                        inventory.vocabulary.push(word);
                    }
                }
                KnowledgeDecision::Excluded(reason) => {
                    let origin = match content {
                        MappedContent::Vocabulary(word) => word.origin,
                        MappedContent::NonVocabulary(origin)
                        | MappedContent::Unavailable(origin) => origin,
                    };
                    inventory.excluded.push(ExcludedMaterial { origin, reason });
                }
            }
        }
        inventory.validate()?;
        Ok(inventory)
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
                || !text(&word.spelling, 256)
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
