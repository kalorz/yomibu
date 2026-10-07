use super::*;
use crate::{
    evaluation::{CheckKind, CheckOutcome, CheckState, EvaluationBasis},
    grammar::GrammarRule::{ObjectWo, TopicWa},
    inventory::{InventoryGrammar, InventoryGrammarBinding},
    story::TargetCoverage,
};

#[test]
fn object_uncertainty_preserves_independent_grammar_targets() {
    use DirectObjectEvidence::*;
    use LexicalUncertainty::*;

    let analyzer = SudachiAnalyzer::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/a1/current/system_core.dic"),
    )
    .unwrap();
    let request: StoryRequest = serde_json::from_value(serde_json::json!({
        "version":1, "brief":"A bounded object sentence",
        "targets":{"vocabulary":[],"grammar":["topic","polite","object","topic-or-object"]}
    }))
    .unwrap();
    for (case, expected) in [
        ("unknown", Unknown),
        ("not_asserted", NotAsserted),
        ("competing", Unresolved(CompetingIdentities)),
        ("alternatives", Unresolved(MeaningAlternatives)),
        ("missing_reading", Unresolved(MissingReading)),
        ("reading_alternatives", Unresolved(ReadingAlternatives)),
        ("missing_meaning", Unresolved(MissingMeaning)),
        ("reading_mismatch", Unresolved(ReadingMismatch)),
        ("missing_verb", Unavailable),
        ("unrelated_duplicate", Confirmed),
    ] {
        let mut inventory = LearnerInventory::from_manual(
            serde_json::from_str(include_str!(
                "../../../../tests/fixtures/story/inventory.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let verb = &mut inventory.vocabulary[0];
        verb.written_form = "食べる".into();
        verb.readings = vec!["たべる".into()];
        verb.meanings = vec!["eat".into()];
        verb.direct_object = match expected {
            Unknown => None,
            NotAsserted => Some(false),
            _ => Some(true),
        };
        match case {
            "alternatives" => verb.meanings.push("another use".into()),
            "missing_reading" => verb.readings.clear(),
            "reading_alternatives" => verb.readings.push("another reading".into()),
            "missing_meaning" => verb.meanings.clear(),
            "reading_mismatch" => verb.readings = vec!["ねる".into()],
            "competing" => {
                let mut other = verb.clone();
                other.id = "another-eat".into();
                other.direct_object = Some(false);
                inventory.vocabulary.push(other);
            }
            "missing_verb" => {
                inventory.vocabulary.remove(0);
            }
            "unrelated_duplicate" => {
                let mut other = inventory.vocabulary[1].clone();
                other.id = "another-cat".into();
                inventory.vocabulary.push(other);
            }
            _ => {}
        }
        for (id, rules) in [
            ("object", vec![ObjectWo]),
            ("topic-or-object", vec![TopicWa, ObjectWo]),
        ] {
            inventory.grammar_declarations.push(InventoryGrammar {
                id: id.into(),
                description: id.into(),
            });
            inventory
                .grammar_bindings
                .extend(rules.into_iter().map(|rule| InventoryGrammarBinding {
                    declaration_id: id.into(),
                    rule,
                }));
        }
        inventory.grammar_declarations.reverse();
        request.validate(&inventory).unwrap();
        let inputs = StoryAssessmentInputs {
            inventory: &inventory,
            request: &request,
            selected_vocabulary_ids: inventory
                .vocabulary
                .iter()
                .map(|word| word.id.as_str())
                .collect(),
        };
        let assessed = assess_candidate("猫は犬を食べます。", &inputs, &analyzer);
        for (index, span) in [(0, 3..6), (1, 18..24)] {
            assert_eq!(
                assessed.targets[index].state,
                TargetState::Observed(TargetCoverage::Complete),
                "{case}"
            );
            assert_eq!(assessed.targets[index].spans, [span], "{case}");
        }
        let object = &assessed.targets[2];
        let combined = &assessed.targets[3];
        if expected == Confirmed {
            assert_eq!(
                object.state,
                TargetState::Observed(TargetCoverage::Complete),
                "{case}"
            );
            assert_eq!(object.spans.len(), 1);
            assert_eq!(object.spans[0], 9..12);
            assert!(object.uncertainties.is_empty());
            assert_eq!(
                combined.state,
                TargetState::Observed(TargetCoverage::Complete)
            );
            assert_eq!(combined.spans, [3..6, 9..12]);
        } else {
            assert_eq!(object.state, TargetState::Unassessable, "{case}");
            assert_eq!(object.uncertainties.len(), 1);
            assert_eq!(
                object.uncertainties[0].reason,
                TargetUncertaintyReason::DirectObject(expected),
                "{case}"
            );
            assert_eq!(object.uncertainties[0].span, 12..18);
            let entries = match case {
                "competing" => vec!["sleep", "another-eat"],
                "missing_verb" => vec![],
                _ => vec!["sleep"],
            };
            assert_eq!(object.uncertainties[0].inventory_entries, entries, "{case}");
            assert_eq!(
                combined.state,
                TargetState::Observed(TargetCoverage::Partial),
                "{case}"
            );
            assert_eq!(combined.spans.len(), 1);
            assert_eq!(combined.spans[0], 3..6);
            assert_eq!(combined.uncertainties, object.uncertainties);
        }
        let CandidateAssessment::Completed { evaluation, .. } = assessed.assessment else {
            panic!()
        };
        assert_eq!(evaluation.basis, EvaluationBasis::FullLearnerInventory);
        assert_eq!(
            serde_json::to_value(&evaluation).unwrap()["basis"],
            "full_learner_inventory"
        );
        if case == "missing_verb" {
            assert_eq!(
                evaluation.check(CheckKind::Vocabulary).state,
                CheckState::Completed(CheckOutcome::Fail)
            );
        }
        for kind in [CheckKind::Particles, CheckKind::Scope] {
            assert_eq!(
                evaluation.check(kind).state,
                CheckState::Completed(CheckOutcome::Inconclusive),
                "{case}"
            );
        }
        let absent = assess_candidate("犬です。", &inputs, &analyzer);
        assert!(
            absent
                .targets
                .iter()
                .all(|target| target.state == TargetState::Absent)
        );
    }
}
