use yomibu_core::domain::analysis::{Sentence, SentenceError};

#[test]
fn sentence_preserves_original_text_and_limits_unicode_scalars_not_bytes() {
    let text = format!(" {}", "猫".repeat(99));
    let sentence = Sentence::new(&text).unwrap();
    assert_eq!(sentence.text(), text);
    assert_eq!(
        Sentence::new(&"猫".repeat(101)),
        Err(SentenceError::TooLong { characters: 101 })
    );
    for blank in ["", " \t", "　"] {
        assert_eq!(Sentence::new(blank), Err(SentenceError::Blank));
    }
}
