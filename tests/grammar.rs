use yomibu::grammar::GrammarDeclarations;

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
