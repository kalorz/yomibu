use std::{fmt, ops::Range};

use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::evaluation::{DirectObjectEvidence, LexicalUncertainty};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Vocabulary,
    Grammar,
}

impl fmt::Display for TargetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Vocabulary => "vocabulary",
            Self::Grammar => "grammar",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetCoverage {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
    NotRun,
    Absent,
    Unassessable,
    Observed(TargetCoverage),
}

impl TargetState {
    pub fn status(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::Absent => "absent",
            Self::Unassessable => "unassessable",
            Self::Observed(_) => "observed",
        }
    }

    pub fn completeness(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::Absent | Self::Observed(TargetCoverage::Complete) => "complete",
            Self::Unassessable | Self::Observed(TargetCoverage::Partial) => "partial",
        }
    }
}

impl Serialize for TargetState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut fields = serializer.serialize_struct("TargetState", 2)?;
        fields.serialize_field("status", self.status())?;
        fields.serialize_field("completeness", self.completeness())?;
        fields.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetUncertaintyScope {
    TargetOccurrence,
    SentenceCoverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum TargetUncertaintyReason {
    AssessmentUnavailable,
    Lexical(LexicalUncertainty),
    UnsupportedMorphology,
    ComponentOnly,
    UnsupportedConstruction,
    NoGrammarBinding,
    DirectObject(DirectObjectEvidence),
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct TargetUncertainty {
    pub span: Range<usize>,
    pub scope: TargetUncertaintyScope,
    pub reason: TargetUncertaintyReason,
    pub inventory_entries: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TargetObservation {
    pub kind: TargetKind,
    pub id: String,
    #[serde(flatten)]
    pub state: TargetState,
    pub spans: Vec<Range<usize>>,
    pub uncertainties: Vec<TargetUncertainty>,
}

impl TargetObservation {
    pub(crate) fn from_evidence(
        kind: TargetKind,
        id: &str,
        spans: Vec<Range<usize>>,
        uncertainties: Vec<TargetUncertainty>,
    ) -> Self {
        let state = if kind == TargetKind::Vocabulary
            && uncertainties
                .iter()
                .any(|uncertainty| uncertainty.scope == TargetUncertaintyScope::TargetOccurrence)
        {
            TargetState::Unassessable
        } else {
            match (spans.is_empty(), uncertainties.is_empty()) {
                (true, true) => TargetState::Absent,
                (true, false) => TargetState::Unassessable,
                (false, true) => TargetState::Observed(TargetCoverage::Complete),
                (false, false) => TargetState::Observed(TargetCoverage::Partial),
            }
        };
        Self {
            kind,
            id: id.to_owned(),
            state,
            spans,
            uncertainties,
        }
    }
}
