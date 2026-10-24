use std::sync::Arc;

use yomibu_core::domain::candidate::Candidate;
use yomibu_core::domain::inventory::Inventory;
use yomibu_core::domain::plan::Plan;
use yomibu_core::ports::capabilities::generator::{Generator, GeneratorError};
use yomibu_core::ports::infrastructure::inventory_source::{InventorySource, InventorySourceError};

use super::{WriteStory, WriteStoryError};

struct FixedInventory(Vec<&'static str>);

impl InventorySource for FixedInventory {
    fn read_inventory(&self) -> Result<Inventory, InventorySourceError> {
        let vocabulary = self.0.iter().map(|word| word.to_string()).collect();
        Ok(Inventory { vocabulary })
    }
}

struct UnavailableInventory;

impl InventorySource for UnavailableInventory {
    fn read_inventory(&self) -> Result<Inventory, InventorySourceError> {
        Err(InventorySourceError::Unavailable {
            source: "test inventory".to_string(),
            reason: "unavailable on purpose".to_string(),
        })
    }
}

/// Joins the plan's vocabulary so the test can see the inventory reached the generator.
struct JoinWithCommas;

impl Generator for JoinWithCommas {
    fn generate(&self, plan: &Plan) -> Result<Candidate, GeneratorError> {
        Ok(Candidate {
            text: plan.inventory.vocabulary.join(","),
        })
    }
}

struct FailingGenerator;

impl Generator for FailingGenerator {
    fn generate(&self, _plan: &Plan) -> Result<Candidate, GeneratorError> {
        Err(GeneratorError {
            reason: "failing on purpose".to_string(),
        })
    }
}

#[test]
fn generates_one_candidate_from_the_inventory() {
    let write_story = WriteStory::new(
        Arc::new(FixedInventory(vec!["猫", "犬"])),
        Box::new(JoinWithCommas),
    );

    let candidate = write_story.run().expect("story written");

    assert_eq!(candidate.text, "猫,犬");
}

#[test]
fn reports_an_unavailable_inventory_source() {
    let write_story = WriteStory::new(Arc::new(UnavailableInventory), Box::new(JoinWithCommas));

    let error = write_story.run().expect_err("unavailable source fails");

    assert!(matches!(error, WriteStoryError::InventorySource(_)));
}

#[test]
fn reports_a_failing_generator() {
    let write_story = WriteStory::new(
        Arc::new(FixedInventory(vec!["猫"])),
        Box::new(FailingGenerator),
    );

    let error = write_story.run().expect_err("failing generator fails");

    assert!(matches!(error, WriteStoryError::Generator(_)));
}
