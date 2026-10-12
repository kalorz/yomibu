use serde::Deserialize;
use yomibu_core::domain::inventory::{
    InventoryGrammar, InventoryGrammarBinding, InventoryWord, LearnerInventory,
};

pub fn inventory() -> LearnerInventory {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fixture {
        version: u32,
        vocabulary: Vec<InventoryWord>,
        grammar_declarations: Vec<InventoryGrammar>,
        grammar_bindings: Vec<InventoryGrammarBinding>,
    }
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/story/inventory.json")).unwrap();
    assert_eq!(fixture.version, 1);
    let inventory = LearnerInventory {
        vocabulary: fixture.vocabulary,
        grammar_declarations: fixture.grammar_declarations,
        grammar_bindings: fixture.grammar_bindings,
        excluded: vec![],
    };
    inventory.validate().unwrap();
    inventory
}
