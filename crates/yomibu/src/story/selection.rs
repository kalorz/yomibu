//! Vocabulary selection from local lexical evidence or explicit cached vectors.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use super::{StoryError, StoryRequest};
use crate::{
    inventory::{InventoryWord, LearnerInventory},
    retrieval::{
        EmbeddingCache, EmbeddingError, EmbeddingModelIdentity, cosine, prepare_embedding_inputs,
    },
};

#[derive(Debug, Serialize)]
pub struct SelectedVocabulary<'a> {
    pub word: &'a InventoryWord,
    pub reason: &'static str,
    pub score: Option<f64>,
}
#[derive(Debug, Serialize)]
pub struct StoryVocabularySelection<'a> {
    pub selector_revision: &'static str,
    pub selected: Vec<SelectedVocabulary<'a>>,
    pub vocabulary_targets: &'a [String],
    pub grammar_targets: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<EmbeddingModelIdentity>,
}

pub fn select_builtin_vocabulary<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    limit: usize,
    seed: u64,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    request.validate(inventory)?;
    request.validate_selection_limit(limit)?;
    let topic = request
        .topic
        .as_ref()
        .map(|topic| (topic.text(), lexical_terms(topic.text())));
    let mut ranked: Vec<_> = inventory
        .vocabulary
        .iter()
        .map(|word| {
            let score = topic.as_ref().map(|(text, topic_terms)| {
                let terms: BTreeSet<_> = std::iter::once(word.written_form.as_str())
                    .chain(
                        word.readings
                            .iter()
                            .chain(&word.meanings)
                            .map(String::as_str),
                    )
                    .flat_map(lexical_terms)
                    .collect();
                let overlap = terms.intersection(topic_terms).count() as f64;
                let written_match =
                    word.written_form.chars().any(is_japanese) && text.contains(&word.written_form);
                overlap.max(f64::from(written_match))
            });
            let mut hash = Sha256::new();
            hash.update(seed.to_be_bytes());
            hash.update(word.id.as_bytes());
            (word, score, hash.finalize())
        })
        .collect();
    ranked.sort_by(|(a, sa, ha), (b, sb, hb)| {
        sb.partial_cmp(sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| ha.cmp(hb))
            .then_with(|| a.id.cmp(&b.id))
    });
    select_ranked(
        request,
        ranked
            .into_iter()
            .map(|(word, score, _)| (word, score))
            .collect(),
        limit,
        |score| {
            if score.is_some_and(|score| score > 0.) {
                "topic_overlap"
            } else {
                "local_sample"
            }
        },
        "builtin-v2",
        None,
    )
}

fn is_japanese(character: char) -> bool {
    matches!(character,
        '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}'
        | '\u{f900}'..='\u{faff}' | '\u{ff66}'..='\u{ff9d}'
        | '\u{20000}'..='\u{323af}'
    )
}

fn lexical_terms(text: &str) -> BTreeSet<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect()
}

pub fn select_vocabulary<'a>(
    inventory: &'a LearnerInventory,
    request: &'a StoryRequest,
    cache: &EmbeddingCache,
    model: &EmbeddingModelIdentity,
    limit: usize,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    if request.topic.is_none() {
        return Err(StoryError::Invalid("semantic selection requires a topic"));
    }
    let inputs = prepare_embedding_inputs(inventory, request)?;
    request.validate_selection_limit(limit)?;
    let vectors = cache.vectors(model, &inputs)?;
    let query = vectors.last().ok_or(EmbeddingError::Missing)?;
    let mut ranked: Vec<_> = inventory
        .vocabulary
        .iter()
        .zip(&vectors)
        .map(|(word, vector)| (word, Some(cosine(vector, query))))
        .collect();
    ranked.sort_by(|(a, sa), (b, sb)| {
        sb.partial_cmp(sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    select_ranked(
        request,
        ranked,
        limit,
        |_| "topic_similarity",
        "inventory-similarity-v1",
        Some(model.clone()),
    )
}

fn select_ranked<'a>(
    request: &'a StoryRequest,
    ranked: Vec<(&'a InventoryWord, Option<f64>)>,
    limit: usize,
    support_reason: fn(Option<f64>) -> &'static str,
    selector_revision: &'static str,
    embedding_model: Option<EmbeddingModelIdentity>,
) -> Result<StoryVocabularySelection<'a>, StoryError> {
    let mut selected = Vec::new();
    for id in &request.targets.vocabulary {
        let (word, score) = ranked
            .iter()
            .find(|(word, _)| &word.id == id)
            .ok_or(StoryError::Invalid("missing target"))?;
        selected.push(SelectedVocabulary {
            word,
            reason: "practice_target",
            score: *score,
        });
    }
    for (word, score) in ranked {
        if selected.len() == limit {
            break;
        }
        if !selected.iter().any(|entry| entry.word.id == word.id) {
            selected.push(SelectedVocabulary {
                word,
                reason: support_reason(score),
                score,
            });
        }
    }
    Ok(StoryVocabularySelection {
        selector_revision,
        selected,
        vocabulary_targets: &request.targets.vocabulary,
        grammar_targets: &request.targets.grammar,
        embedding_model,
    })
}
