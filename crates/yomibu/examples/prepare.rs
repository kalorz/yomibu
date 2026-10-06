use yomibu::{
    domain::WaniKaniSyncData,
    grammar::GrammarDeclarations,
    knowledge::{KnowledgeDecision, LearnerKnowledgePolicy},
    preparation::{PracticeTarget, UnassessedAspect, prepare_context},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        snapshot: WaniKaniSyncData,
    }
    // Explicit synthetic input for this demonstration, never a production fallback.
    let fixture = include_str!("../../../tests/fixtures/preparation.json");
    let source = serde_json::from_str::<Envelope>(fixture)?.snapshot;
    let grammar = GrammarDeclarations::from_descriptions(["です", "は as a topic marker"])?;
    let targets = [PracticeTarget {
        word: "一つ".into(),
        intended_reading: "ひとつ".into(),
        intended_sense: "one thing".into(),
    }];
    let policy = LearnerKnowledgePolicy::default();
    let result = prepare_context(&source, &grammar, &policy, &targets)?;

    assert_eq!(result.targets[0].target, &targets[0]);
    assert_eq!(result.targets[0].subject.id, 2);
    assert_eq!(result.targets[0].assignment.id, 102);
    assert_eq!(result.targets[0].examples[0].japanese, "一つあります。");
    assert_eq!(result.knowledge.grammar, grammar.entries());
    assert_eq!(
        result.knowledge.materials[1].decision,
        KnowledgeDecision::Eligible
    );
    assert!(
        result
            .unassessed
            .contains(&UnassessedAspect::ReadingSenseAssociation)
    );
    println!("{result:#?}");
    Ok(())
}
