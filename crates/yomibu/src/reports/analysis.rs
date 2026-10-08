//! Existing supplied-text input/report shape, independent of files and terminals.
use serde::{Deserialize, Serialize};
use yomibu_core::domain::{analysis::SentenceAnalysis, evaluation::EvaluationBindings};
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisInput {
    pub version: u32,
    pub sentence: String,
    pub grammar: Vec<String>,
    pub bindings: EvaluationBindings,
}

#[derive(Serialize)]
pub struct AnalysisReport<'a> {
    pub version: u32,
    pub input: &'a AnalysisInput,
    pub analysis: SentenceAnalysis<'a>,
    pub outcome: CheckState,
    pub evaluation: Evaluation,
}

pub use yomibu_core::domain::analysis::AnalysisProvenance;
pub use yomibu_core::domain::evaluation::{
    CheckKind, CheckOutcome, CheckState, Evaluation, UnassessedAspect,
};
