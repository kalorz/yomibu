use std::fmt;
use std::sync::Arc;

use yomibu_core::domain::candidate::Candidate;
use yomibu_core::domain::plan::Plan;
use yomibu_core::ports::capabilities::generator::{Generator, GeneratorError};
use yomibu_core::ports::infrastructure::inventory_source::{InventorySource, InventorySourceError};

/// Writes one story from the whole inventory.
pub struct WriteStory {
    inventory_source: Arc<dyn InventorySource>,
    generator: Box<dyn Generator>,
}

impl WriteStory {
    pub fn new(
        inventory_source: Arc<dyn InventorySource>,
        generator: Box<dyn Generator>,
    ) -> WriteStory {
        WriteStory {
            inventory_source,
            generator,
        }
    }

    /// Steps in fixed order: read inventory, build plan, generate.
    pub fn run(&self) -> Result<Candidate, WriteStoryError> {
        let inventory = self.inventory_source.read_inventory()?;
        let plan = Plan { inventory };
        let candidate = self.generator.generate(&plan)?;
        Ok(candidate)
    }
}

#[derive(Debug)]
pub enum WriteStoryError {
    InventorySource(InventorySourceError),
    Generator(GeneratorError),
}

impl From<InventorySourceError> for WriteStoryError {
    fn from(error: InventorySourceError) -> WriteStoryError {
        WriteStoryError::InventorySource(error)
    }
}

impl From<GeneratorError> for WriteStoryError {
    fn from(error: GeneratorError) -> WriteStoryError {
        WriteStoryError::Generator(error)
    }
}

impl fmt::Display for WriteStoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteStoryError::InventorySource(error) => error.fmt(f),
            WriteStoryError::Generator(error) => error.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests;
