use yomibu_core::domain::inventory::Inventory;
use yomibu_core::domain::plan::Plan;
use yomibu_core::ports::capabilities::generator::Generator;

use super::{EchoGenerator, EchoOptions};

fn plan(vocabulary: &[&str]) -> Plan {
    let vocabulary = vocabulary.iter().map(|word| word.to_string()).collect();
    Plan {
        inventory: Inventory { vocabulary },
    }
}

#[test]
fn repeats_the_vocabulary_as_lines() {
    let generator = EchoGenerator::new(EchoOptions {});

    let candidate = generator
        .generate(&plan(&["猫", "犬"]))
        .expect("echo never fails");

    assert_eq!(candidate.text, "猫\n犬");
}

#[test]
fn repeats_an_empty_inventory_as_empty_text() {
    let generator = EchoGenerator::new(EchoOptions {});

    let candidate = generator.generate(&plan(&[])).expect("echo never fails");

    assert_eq!(candidate.text, "");
}
