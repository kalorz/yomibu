use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use yomibu_core::{
    domain::{
        inventory::LearnerInventory,
        story::{StoryError, StoryRequest, StoryVocabularySelection},
    },
    pipeline::selection::select_ranked,
};

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
