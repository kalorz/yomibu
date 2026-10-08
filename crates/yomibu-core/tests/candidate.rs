use yomibu_core::domain::candidate::{
    CandidateConstructionError, GeneratedCandidates, GeneratedPassage, GenerationProvenance,
};
use yomibu_core::domain::story::StoryFormat;

fn provenance() -> GenerationProvenance {
    GenerationProvenance {
        provider: "test",
        requested_model: "test".into(),
        returned_model: "test".into(),
        requested_tier: "default",
        returned_tier: None,
        prompt_revision: "test",
        request_sha256: "test".into(),
        request_bytes: 1,
        response_id: "test".into(),
        request_id: None,
        request_count: 1,
        usage: None,
    }
}

#[test]
fn candidate_construction_preserves_original_text_spans_and_provenance() {
    let text = " 猫です。犬です。寝ます。".to_owned();
    let allocation = text.as_ptr();
    let generated = GeneratedCandidates::new(
        vec![GeneratedPassage {
            text,
            sentence_spans: vec![0..13, 13..25, 25..37],
        }],
        provenance(),
        StoryFormat::Passage,
        1,
    )
    .unwrap();
    assert_eq!(generated.passages()[0].text, " 猫です。犬です。寝ます。");
    assert_eq!(generated.passages()[0].text.as_ptr(), allocation);
    assert_eq!(
        generated.passages()[0].sentence_spans,
        [0..13, 13..25, 25..37]
    );
    assert_eq!(generated.provenance().response_id, "test");
}

#[test]
fn candidate_construction_rejects_invalid_or_incomplete_sentence_spans() {
    assert!(GeneratedCandidates::new(vec![], provenance(), StoryFormat::Sentence, 1).is_err());
    for (format, spans) in [
        (StoryFormat::Sentence, std::iter::once(0..1).collect()),
        (StoryFormat::Sentence, std::iter::once(0..3).collect()),
        (StoryFormat::Sentence, std::iter::once(3..9).collect()),
        (StoryFormat::Passage, vec![0..1, 1..6, 6..9]),
        (StoryFormat::Passage, vec![0..3, 0..6, 6..9]),
        (StoryFormat::Passage, vec![0..3, 3..6, 6..10]),
    ] {
        assert!(matches!(
            GeneratedCandidates::new(
                vec![GeneratedPassage {
                    text: "猫犬鳥".into(),
                    sentence_spans: spans,
                }],
                provenance(),
                format,
                1,
            ),
            Err(CandidateConstructionError::InvalidSpans)
        ));
    }
    assert!(matches!(
        GeneratedCandidates::new(
            vec![GeneratedPassage {
                text: "猫犬鳥".into(),
                sentence_spans: vec![0..0, 0..6, 6..9],
            }],
            provenance(),
            StoryFormat::Passage,
            1,
        ),
        Err(CandidateConstructionError::Sentence(_))
    ));
    for text in [" ".to_owned(), "猫".repeat(101)] {
        let span = 0..text.len();
        assert!(
            GeneratedCandidates::new(
                vec![GeneratedPassage {
                    text,
                    sentence_spans: std::iter::once(span).collect(),
                }],
                provenance(),
                StoryFormat::Sentence,
                1,
            )
            .is_err()
        );
    }
}

fn passage(sentences: usize) -> GeneratedPassage {
    let sentence = "猫です。";
    GeneratedPassage {
        text: sentence.repeat(sentences),
        sentence_spans: (0..sentences)
            .map(|i| i * sentence.len()..(i + 1) * sentence.len())
            .collect(),
    }
}

#[test]
fn candidate_construction_requires_the_requested_candidate_count() {
    for count in [1, 50] {
        let result = GeneratedCandidates::new(
            (0..count).map(|_| passage(1)).collect(),
            provenance(),
            StoryFormat::Sentence,
            3,
        );
        assert!(
            matches!(result, Err(CandidateConstructionError::CandidateCount { expected: 3, actual }) if actual == count)
        );
    }
}

#[test]
fn candidate_construction_requires_sentence_counts_to_match_the_format() {
    for (format, sentences) in [
        (StoryFormat::Sentence, 0),
        (StoryFormat::Sentence, 2),
        (StoryFormat::Passage, 1),
        (StoryFormat::Passage, 2),
        (StoryFormat::Passage, 40),
    ] {
        let result = GeneratedCandidates::new(vec![passage(sentences)], provenance(), format, 1);
        assert!(
            matches!(result, Err(CandidateConstructionError::SentenceCount { format: actual_format, actual }) if actual_format == format && actual == sentences)
        );
    }
}

#[test]
fn candidate_construction_accepts_requested_counts_and_format_boundaries() {
    for (format, sentences) in [
        (StoryFormat::Sentence, 1),
        (StoryFormat::Passage, 3),
        (StoryFormat::Passage, 5),
    ] {
        let generated = GeneratedCandidates::new(
            (0..3).map(|_| passage(sentences)).collect(),
            provenance(),
            format,
            3,
        )
        .unwrap();
        assert_eq!(generated.passages().len(), 3);
        for passage in generated.passages() {
            assert_eq!(passage.sentence_spans.len(), sentences);
            assert_eq!(passage.text, "猫です。".repeat(sentences));
        }
    }
}
