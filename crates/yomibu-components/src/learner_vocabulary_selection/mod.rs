use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use yomibu_core::{
    capabilities::SelectionStep,
    domain::story::{SelectionCandidates, SelectionInput, StoryError},
};

/// Replace lexical scores and reasons; clear embedding provenance.
pub struct LexicalTopicScoring;

impl SelectionStep for LexicalTopicScoring {
    fn apply<'a>(
        &self,
        input: SelectionInput<'a>,
        mut candidates: SelectionCandidates<'a>,
    ) -> Result<SelectionCandidates<'a>, StoryError> {
        let topic = input
            .request
            .topic
            .as_ref()
            .map(|topic| (topic.text(), lexical_terms(topic.text())));
        for entry in &mut candidates.entries {
            let word = entry.word;
            entry.score = topic.as_ref().map(|(text, topic_terms)| {
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
            entry.reason = if entry.score.is_some_and(|score| score > 0.) {
                "topic_overlap"
            } else {
                "local_sample"
            };
        }
        candidates.embedding_model = None;
        Ok(candidates)
    }
}

/// Order current scores descending, then SHA-256(seed bytes, ID), then ID.
/// Mark neutral `inventory_entry` reasons as `local_sample`; preserve scoring reasons.
pub struct SeededOrdering {
    pub seed: u64,
}

impl SelectionStep for SeededOrdering {
    fn apply<'a>(
        &self,
        _: SelectionInput<'a>,
        mut candidates: SelectionCandidates<'a>,
    ) -> Result<SelectionCandidates<'a>, StoryError> {
        let mut ranked: Vec<_> = candidates
            .entries
            .into_iter()
            .map(|mut entry| {
                if entry.reason == "inventory_entry" {
                    entry.reason = "local_sample";
                }
                let mut hash = Sha256::new();
                hash.update(self.seed.to_be_bytes());
                hash.update(entry.word.id.as_bytes());
                (entry, hash.finalize())
            })
            .collect();
        ranked.sort_by(|(a, ha), (b, hb)| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| ha.cmp(hb))
                .then_with(|| a.word.id.cmp(&b.word.id))
        });
        candidates.entries = ranked.into_iter().map(|(entry, _)| entry).collect();
        Ok(candidates)
    }
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
