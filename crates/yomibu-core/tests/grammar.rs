use yomibu_core::domain::grammar::GrammarDeclarations;

#[test]
fn manual_descriptions_keep_content_and_duplicate_entries() {
    let grammar = GrammarDeclarations::from_descriptions(["  です  ", "は", "は"]).unwrap();
    let entries: Vec<_> = grammar
        .entries()
        .iter()
        .map(|entry| (entry.id, entry.description.as_str()))
        .collect();
    assert_eq!(entries, vec![(1, "  です  "), (2, "は"), (3, "は")]);
}

#[test]
fn rejects_blank_assertions_even_after_valid_entries_and_accepts_empty_input() {
    assert!(
        GrammarDeclarations::from_descriptions(Vec::<String>::new())
            .unwrap()
            .entries()
            .is_empty()
    );
    for blank in ["", " \t\n\u{3000}"] {
        for index in 0..2 {
            let mut descriptions = ["です", "は"];
            descriptions[index] = blank;
            let error = GrammarDeclarations::from_descriptions(descriptions).unwrap_err();
            assert_eq!(
                error,
                yomibu_core::domain::grammar::GrammarError::BlankDescription { entry: index + 1 }
            );
        }
    }
}
