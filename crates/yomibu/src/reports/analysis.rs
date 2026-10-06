//! Existing supplied-text input/report shape, independent of files and terminals.
use crate::{
    analysis::SentenceAnalysis,
    evaluation::{CheckState, Evaluation, EvaluationBindings},
};
use serde::{Deserialize, Serialize};
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
