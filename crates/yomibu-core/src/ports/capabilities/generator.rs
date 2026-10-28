use std::fmt;

use crate::domain::candidate::Candidate;
use crate::domain::plan::Plan;

/// Implementations are built per request and may be moved to a worker thread.
pub trait Generator: Send + Sync {
    fn generate(&self, plan: &Plan) -> Result<Candidate, GeneratorError>;
}

#[derive(Debug)]
pub struct GeneratorError {
    pub reason: String,
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the generator failed: {}", self.reason)
    }
}
