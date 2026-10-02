use yomibu::preview::{CheckOutcome, PreviewError, WordEntry, preview};

fn main() -> Result<(), PreviewError> {
    let words = [
        ("猫", "ねこ", "cat"),
        ("犬", "いぬ", "dog"),
        ("学校", "がっこう", "school"),
    ]
    .map(|(text, reading, meaning)| WordEntry {
        text: text.into(),
        reading: reading.into(),
        meaning: meaning.into(),
    });
    let grammar = ["です".into(), "は".into()];
    let result = preview(&words, &grammar, 2)?;

    assert_eq!(result.selected, &words[..2]);
    assert_eq!(result.grammar, &grammar);
    assert_eq!(result.checks.membership, CheckOutcome::Pass);
    assert_eq!(result.checks.count, CheckOutcome::Pass);
    assert_eq!(result.checks.grammar, CheckOutcome::NotAssessed);
    assert_eq!(
        result.checks.linguistic_correctness,
        CheckOutcome::NotAssessed
    );
    println!("{result:#?}");
    Ok(())
}
