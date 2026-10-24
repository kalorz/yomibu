use serde::Deserialize;
use yomibu_core::domain::candidate::Candidate;
use yomibu_core::domain::plan::Plan;
use yomibu_core::ports::capabilities::generator::{Generator, GeneratorError};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EchoOptions {}

/// Repeats the plan's vocabulary as lines of text.
pub struct EchoGenerator;

impl EchoGenerator {
    pub const NAME: &str = "echo";
    pub const DESCRIPTION: &str = "Repeats the inventory as lines of text; no service call";

    pub fn new(_options: EchoOptions) -> EchoGenerator {
        EchoGenerator
    }
}

impl Generator for EchoGenerator {
    fn generate(&self, plan: &Plan) -> Result<Candidate, GeneratorError> {
        Ok(Candidate {
            text: plan.inventory.vocabulary.join("\n"),
        })
    }
}

#[cfg(test)]
mod tests;
