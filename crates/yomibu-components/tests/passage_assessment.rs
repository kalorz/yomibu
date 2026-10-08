#[path = "../../../tests/support/dictionary.rs"]
mod test_dictionary;
use serde_json::json;
use yomibu_components::{
    japanese_constraint_checks::assess_passages,
    learner_vocabulary_selection::select_builtin_vocabulary,
};
use yomibu_core::domain::{
    candidate::GeneratedPassage,
    evaluation::{CheckKind, CheckOutcome, CheckState},
    inventory::LearnerInventory,
    story::{StoryAssessmentInputs, StoryRequest},
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
    let analyzer = test_dictionary::load_analyzer();
    let results = assess_passages(&[passage], &inputs, Some(&analyzer));
    let result = &results[0];
    assert_eq!(result.sentences.len(), 3);
    assert_eq!(result.targets[0].spans, [0..3, 18..21, 30..33]);
    assert_eq!(result.plan_departures[0].span, 6..9);
    let yomibu_core::domain::candidate::CandidateAssessment::Completed { evaluation, .. } =
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
    ] {
        assert_eq!(evaluation.check(kind).state, CheckState::NotRun);
    }
    assert_eq!(
        evaluation.check(CheckKind::Scope).state,
        CheckState::Completed(CheckOutcome::Pass)
    );
}

#[test]
fn missing_grammar_preserves_independent_scope_findings_and_vocabulary_failures() {
    let inventory = LearnerInventory::from_manual(
        serde_json::from_value(json!({"version":1,"vocabulary":[
            {"id":"book","written_form":"本","readings":["ほん"],"meanings":["book"],"direct_object":null},
            {"id":"read","written_form":"読む","readings":["よむ"],"meanings":["read"],"direct_object":true},
            {"id":"walk","written_form":"歩く","readings":["あるく"],"meanings":["walk"],"direct_object":false}
        ],"grammar_declarations":[],"grammar_bindings":[]})).unwrap(),
    ).unwrap();
    let request: StoryRequest =
        serde_json::from_value(json!({"version":1,"targets":{"vocabulary":[],"grammar":[]}}))
            .unwrap();
    let selection = select_builtin_vocabulary(&inventory, &request, 3, 7).unwrap();
    let inputs = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    let analyzer = test_dictionary::load_analyzer();
    for (text, span, reason, vocabulary) in [
        (
            "本を読みます。",
            0..18,
            "object/predicate combination has no multiword-expression assessment",
            CheckOutcome::Pass,
        ),
        (
            "本を歩きます。",
            3..6,
            "route, departure, and other を uses are outside scope",
            CheckOutcome::Pass,
        ),
        (
            "本が読む。",
            0..15,
            "construction/applicability is outside the supported patterns",
            CheckOutcome::Pass,
        ),
        (
            "鳥を読みます。",
            0..18,
            "object/predicate combination has no multiword-expression assessment",
            CheckOutcome::Fail,
        ),
    ] {
        let passage = GeneratedPassage {
            text: text.into(),
            sentence_spans: std::iter::once(0..text.len()).collect(),
        };
        let results = assess_passages(&[passage], &inputs, Some(&analyzer));
        let yomibu_core::domain::candidate::CandidateAssessment::Completed { evaluation, .. } =
            &results[0].sentences[0].assessment.assessment
        else {
            panic!("{text}")
        };
        let scope = evaluation.check(CheckKind::Scope);
        assert_eq!(
            scope.state,
            CheckState::Completed(CheckOutcome::Inconclusive),
            "{text}"
        );
        assert_eq!(scope.findings[0].span, span, "{text}");
        assert_eq!(scope.findings[0].reason, reason, "{text}");
        assert_eq!(
            evaluation.check(CheckKind::Vocabulary).state,
            CheckState::Completed(vocabulary),
            "{text}"
        );
        assert_eq!(
            evaluation.outcome(),
            if vocabulary == CheckOutcome::Fail {
                CheckState::Completed(CheckOutcome::Fail)
            } else {
                CheckState::NotRun
            },
            "{text}"
        );
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
        yomibu_core::domain::candidate::CandidateAssessment::NotRun
    ));
    assert_eq!(results[0].targets[0].state.status(), "not_run");
}
