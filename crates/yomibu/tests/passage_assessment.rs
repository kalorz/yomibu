#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;
use serde_json::json;
use yomibu::{
    adapters::sudachi::SudachiAnalyzer,
    candidate::GeneratedPassage,
    evaluation::{CheckKind, CheckState},
    inventory::LearnerInventory,
    story::{StoryAssessmentInputs, StoryRequest, assess_passages, select_builtin_vocabulary},
};

#[test]
fn passage_checks_full_inventory_sentence_by_sentence_and_leaves_missing_grammar_unknown() {
    let mut inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    inventory.grammar_declarations.clear();
    inventory.grammar_bindings.clear();
    let request: StoryRequest =
        serde_json::from_value(json!({"version":1,"targets":{"vocabulary":["cat"],"grammar":[]}}))
            .unwrap();
    let selection = select_builtin_vocabulary(&inventory, &request, 1, 7).unwrap();
    let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    let passage = GeneratedPassage {
        text: "猫は寝ます。猫です。猫は寝ます。".into(),
        sentence_spans: vec![0..18, 18..30, 30..48],
    };
    let analyzer =
        SudachiAnalyzer::load(test_dictionary::bundle().join("system_core.dic")).unwrap();
    let results = assess_passages(&[passage], &inputs, Some(&analyzer));
    let result = &results[0];
    assert_eq!(result.sentences.len(), 3);
    assert_eq!(result.targets[0].spans, [0..3, 18..21, 30..33]);
    assert_eq!(result.plan_departures[0].span, 6..9);
    let yomibu::candidate::CandidateAssessment::Completed { evaluation, .. } =
        &result.sentences[0].assessment.assessment
    else {
        panic!()
    };
    assert!(matches!(
        evaluation.check(CheckKind::Vocabulary).state,
        CheckState::Completed(_)
    ));
    for kind in [
        CheckKind::Inflection,
        CheckKind::Particles,
        CheckKind::Nominal,
        CheckKind::Scope,
    ] {
        assert_eq!(evaluation.check(kind).state, CheckState::NotRun);
    }
}

#[test]
fn absent_dictionary_reports_not_run_without_losing_the_passage() {
    let inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    let request: StoryRequest =
        serde_json::from_value(json!({"version":1,"targets":{"vocabulary":["cat"],"grammar":[]}}))
            .unwrap();
    let selection = select_builtin_vocabulary(&inventory, &request, 1, 7).unwrap();
    let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    let results = assess_passages(
        &[GeneratedPassage {
            text: "猫です。".into(),
            sentence_spans: std::iter::once(0..12).collect(),
        }],
        &inputs,
        None,
    );
    assert!(matches!(
        results[0].sentences[0].assessment.assessment,
        yomibu::candidate::CandidateAssessment::NotRun
    ));
    assert_eq!(results[0].targets[0].state.status(), "not_run");
}
