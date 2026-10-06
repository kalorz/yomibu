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
                yomibu::grammar::GrammarError::BlankDescription { entry: index + 1 }
            );
        }
    }
}

#[test]
fn explicit_grammar_file_loading_is_read_only_and_matches_direct_input() {
    use yomibu::adapters::grammar_file;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("grammar.json");
    let bytes = r#"{"version":1,"declarations":["  です  ","は","は"]}"#;
    std::fs::write(&path, bytes).unwrap();
    let expected = GrammarDeclarations::from_descriptions(["  です  ", "は", "は"]).unwrap();
    assert_eq!(grammar_file::load(&path).unwrap(), expected);
    assert_eq!(grammar_file::parse(bytes).unwrap(), expected);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn grammar_file_errors_preserve_absence_version_and_validation_failures() {
    use yomibu::adapters::grammar_file::{self, Error};

    assert!(matches!(
        grammar_file::parse(r#"{"version":2}"#),
        Err(Error::UnsupportedVersion { version: 2 })
    ));
    for invalid in [
        "{",
        r#"{"declarations":[]}"#,
        r#"{"version":1}"#,
        r#"{"version":1,"declarations":null}"#,
        r#"{"version":1,"declarations":[1]}"#,
        r#"{"version":1,"declarations":[],"typo":true}"#,
    ] {
        assert!(matches!(
            grammar_file::parse(invalid),
            Err(Error::InvalidJson { .. })
        ));
    }
    assert!(matches!(
        grammar_file::parse(r#"{"version":1,"declarations":["です"," "]}"#),
        Err(Error::InvalidDeclaration(
            yomibu::grammar::GrammarError::BlankDescription { entry: 2 }
        ))
    ));
    assert!(
        grammar_file::parse(r#"{"version":1,"declarations":[]}"#)
            .unwrap()
            .entries()
            .is_empty()
    );
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        grammar_file::load(&dir.path().join("missing.json")),
        Err(Error::Read { .. })
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
