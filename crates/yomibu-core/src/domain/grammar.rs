//! Manual learner assertions, independent of grammar recognition.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrammarRule {
    NominalDesu,
    TopicWa,
    ObjectWo,
    PoliteNonPast,
    PolitePast,
    PoliteNegativeNonPast,
    PoliteNegativePast,
}
