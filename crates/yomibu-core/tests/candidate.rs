use yomibu_core::domain::candidate::{GeneratedCandidates, GeneratedPassage, GenerationProvenance};

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
    let text = " 猫です。犬です。".to_owned();
    let allocation = text.as_ptr();
    let generated = GeneratedCandidates::new(
        vec![GeneratedPassage {
            text,
            sentence_spans: vec![0..13, 13..25],
        }],
        provenance(),
    )
    .unwrap();
    assert_eq!(generated.passages()[0].text, " 猫です。犬です。");
    assert_eq!(generated.passages()[0].text.as_ptr(), allocation);
    assert_eq!(generated.passages()[0].sentence_spans, [0..13, 13..25]);
    assert_eq!(generated.provenance().response_id, "test");
}

#[test]
fn candidate_construction_rejects_invalid_or_incomplete_sentence_spans() {
    assert!(GeneratedCandidates::new(vec![], provenance()).is_err());
    for spans in [
        vec![],
        vec![0..1, 1..6],
        std::iter::once(0..3).collect(),
        std::iter::once(3..6).collect(),
        vec![0..3, 0..6],
        vec![0..3, 3..7],
        vec![0..0, 0..6],
    ] {
        assert!(
            GeneratedCandidates::new(
                vec![GeneratedPassage {
                    text: "猫犬".into(),
                    sentence_spans: spans,
                }],
                provenance()
            )
            .is_err()
        );
    }
    for text in [" ".to_owned(), "猫".repeat(101)] {
        let span = 0..text.len();
        assert!(
            GeneratedCandidates::new(
                vec![GeneratedPassage {
                    text,
                    sentence_spans: std::iter::once(span).collect(),
                }],
                provenance()
            )
            .is_err()
        );
    }
}
