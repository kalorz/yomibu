use yomibu::preview::{CheckOutcome, PreviewError, WordEntry, preview};

fn word(text: &str, reading: &str, meaning: &str) -> WordEntry {
    WordEntry {
        text: text.into(),
        reading: reading.into(),
        meaning: meaning.into(),
    }
}

#[test]
fn selects_the_requested_prefix_of_structured_entries() {
    let words = [
        word("猫", "ねこ", "cat"),
        word("犬", "いぬ", "dog"),
        word("学校", "がっこう", "school"),
    ];
    let result = preview(&words, &[], 2).unwrap();
    assert_eq!(result.selected, &words[..2]);
    assert_eq!(result.checks.membership, CheckOutcome::Pass);
    assert_eq!(result.checks.count, CheckOutcome::Pass);
    assert_eq!(result.checks.grammar, CheckOutcome::NotAssessed);
    assert_eq!(
        result.checks.linguistic_correctness,
        CheckOutcome::NotAssessed
    );
}

#[test]
fn retains_duplicates_and_distinct_readings_and_meanings_in_order() {
    let words = [
        word("生", "なま", "raw"),
        word("生", "なま", "raw"),
        word("生", "せい", "life"),
        word("生", "なま", "unprocessed"),
    ];
    assert_eq!(preview(&words, &[], 4).unwrap().selected, &words);
}

#[test]
fn rejects_empty_input_zero_and_excessive_counts() {
    let words = [word("猫", "ねこ", "cat")];
    for (supplied, take) in [(&[][..], 1), (&[][..], 0), (&words[..], 0), (&words[..], 2)] {
        assert_eq!(
            preview(supplied, &[], take).unwrap_err(),
            PreviewError::InvalidCount {
                requested: take,
                available: supplied.len(),
            }
        );
    }
}

#[test]
fn rejects_blank_fields_even_in_unselected_entries() {
    for field in ["text", "reading", "meaning"] {
        for blank in ["", " \t\n\u{3000}"] {
            for index in 0..2 {
                let mut words = [word("猫", "ねこ", "cat"), word("犬", "いぬ", "dog")];
                match field {
                    "text" => words[index].text = blank.into(),
                    "reading" => words[index].reading = blank.into(),
                    "meaning" => words[index].meaning = blank.into(),
                    _ => unreachable!(),
                }
                let error = preview(&words, &[], 1).expect_err("blank word field was accepted");
                assert_eq!(
                    error.to_string(),
                    format!("Word entry {} has blank {field}.", index + 1)
                );
            }
        }
    }
}

#[test]
fn retains_manual_grammar_descriptions_including_order_and_duplicates() {
    let words = [word("猫", "ねこ", "cat")];
    let grammar = [
        "です".into(),
        "は".into(),
        "です".into(),
        "  A は B  ".into(),
    ];
    assert_eq!(preview(&words, &grammar, 1).unwrap().grammar, &grammar);
    assert!(preview(&words, &[], 1).unwrap().grammar.is_empty());
}

#[test]
fn rejects_blank_grammar_descriptions() {
    let words = [word("猫", "ねこ", "cat")];
    for blank in ["", " \n\t\u{3000}"] {
        let grammar = ["です".into(), blank.into()];
        let error = preview(&words, &grammar, 1).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Grammar entry 2 has a blank description."
        );
    }
}
