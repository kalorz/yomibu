use super::LearnerName;

#[test]
fn learner_name_trims_only_surrounding_whitespace() {
    for input in ["Alice  Smith", " \tAlice  Smith\n\u{3000}"] {
        assert_eq!(LearnerName::new(input).unwrap().as_str(), "Alice  Smith");
    }
    assert_eq!(LearnerName::new("  アリス  ").unwrap().as_str(), "アリス");
}

#[test]
fn learner_name_rejects_blank_input() {
    for input in ["", " ", "\t\r\n\u{3000}"] {
        assert!(LearnerName::new(input).is_none());
    }
}
