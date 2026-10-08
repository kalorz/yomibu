//! Preserve sentence evidence and passage spans while composing explicit capabilities.
use crate::{
    capabilities::{SentenceAnalyzer, StoryAssessor},
    domain::{
        analysis::{Sentence, SentenceAnalysis},
        candidate::{CandidateAssessment, CandidateError, GeneratedCandidates, GeneratedPassage},
        story::{
            PlanDeparture, StoryAssessmentInputs, StoryCandidateAssessment, StoryFindings,
            StoryPassageAssessment, StorySentenceAssessment, TargetKind, TargetObservation,
            TargetState, TargetUncertainty, TargetUncertaintyReason, TargetUncertaintyScope,
        },
    },
};

pub fn assess_passages<A: SentenceAnalyzer, S: StoryAssessor>(
    passages: &[GeneratedPassage],
    inputs: &StoryAssessmentInputs<'_>,
    capabilities: Option<(&A, &S)>,
) -> Vec<StoryPassageAssessment<CandidateError<A::Error, S::Error>>> {
    passages
        .iter()
        .map(|passage| {
            let sentences: Vec<_> = passage
                .sentence_spans
                .iter()
                .map(|span| {
                    let assessment = match capabilities {
                        Some((analyzer, assessor)) => match passage.text.get(span.clone()) {
                            Some(text) => assess_candidate(text, inputs, analyzer, assessor),
                            None => {
                                failed_candidate(None, CandidateError::InvalidSentenceSpan, inputs)
                            }
                        },
                        None => StoryCandidateAssessment {
                            assessment: CandidateAssessment::NotRun,
                            targets: unrun_targets(inputs),
                            plan_departures: Vec::new(),
                        },
                    };
                    StorySentenceAssessment {
                        span: span.clone(),
                        assessment: assessment.into_owned(),
                    }
                })
                .collect();
            let mut targets = unrun_targets(inputs);
            for target in &mut targets {
                let mut spans = Vec::new();
                let mut uncertainties = Vec::new();
                let mut ran = false;
                for sentence in &sentences {
                    if let Some(observed) =
                        sentence.assessment.targets.iter().find(|observed| {
                            observed.id == target.id && observed.kind == target.kind
                        })
                    {
                        ran |= observed.state != TargetState::NotRun;
                        spans.extend(observed.spans.iter().map(|span| {
                            span.start + sentence.span.start..span.end + sentence.span.start
                        }));
                        uncertainties.extend(observed.uncertainties.iter().map(|uncertainty| {
                            TargetUncertainty {
                                span: uncertainty.span.start + sentence.span.start
                                    ..uncertainty.span.end + sentence.span.start,
                                scope: uncertainty.scope,
                                reason: uncertainty.reason,
                                inventory_entries: uncertainty.inventory_entries.clone(),
                            }
                        }));
                        if observed.state == TargetState::NotRun && capabilities.is_some() {
                            uncertainties.push(TargetUncertainty {
                                span: sentence.span.clone(),
                                scope: TargetUncertaintyScope::SentenceCoverage,
                                reason: TargetUncertaintyReason::AssessmentUnavailable,
                                inventory_entries: Vec::new(),
                            });
                        }
                    }
                }
                if ran {
                    *target = TargetObservation::from_evidence(
                        target.kind,
                        &target.id,
                        spans,
                        uncertainties,
                    );
                }
            }
            let plan_departures = sentences
                .iter()
                .flat_map(|sentence| {
                    sentence
                        .assessment
                        .plan_departures
                        .iter()
                        .map(|departure| PlanDeparture {
                            span: departure.span.start + sentence.span.start
                                ..departure.span.end + sentence.span.start,
                            inventory_entries: departure.inventory_entries.clone(),
                        })
                })
                .collect();
            StoryPassageAssessment {
                sentences,
                targets,
                plan_departures,
            }
        })
        .collect()
}

pub fn assess_candidates<'a, A: SentenceAnalyzer, S: StoryAssessor>(
    generated: &'a GeneratedCandidates,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: &A,
    assessor: &S,
) -> Vec<StoryCandidateAssessment<'a, CandidateError<A::Error, S::Error>>> {
    generated
        .passages()
        .iter()
        .flat_map(|passage| {
            passage.sentence_spans.iter().map(move |span| {
                assess_candidate(&passage.text[span.clone()], inputs, analyzer, assessor)
            })
        })
        .collect()
}

pub fn assess_candidate<'a, A: SentenceAnalyzer, S: StoryAssessor>(
    text: &'a str,
    inputs: &StoryAssessmentInputs<'_>,
    analyzer: &A,
    assessor: &S,
) -> StoryCandidateAssessment<'a, CandidateError<A::Error, S::Error>> {
    let sentence = match Sentence::new(text) {
        Ok(sentence) => sentence,
        Err(error) => {
            return failed_candidate(None, CandidateError::Sentence(error), inputs);
        }
    };
    let analysis = match analyzer.analyze(sentence) {
        Ok(analysis) => analysis,
        Err(error) => {
            return failed_candidate(None, CandidateError::Analysis(error), inputs);
        }
    };
    if analysis.sentence.text() != text {
        return failed_candidate(None, CandidateError::MismatchedAnalysis, inputs);
    }
    let findings = match assessor.assess(&analysis, inputs) {
        Ok(assessed) => assessed,
        Err(error) => {
            return failed_candidate(Some(analysis), CandidateError::Assessment(error), inputs);
        }
    };
    if findings.text != text {
        return failed_candidate(Some(analysis), CandidateError::MismatchedAssessment, inputs);
    }
    let StoryFindings {
        evaluation,
        targets,
        plan_departures,
        ..
    } = findings;
    StoryCandidateAssessment {
        assessment: CandidateAssessment::Completed {
            analysis,
            evaluation: Box::new(evaluation),
        },
        targets,
        plan_departures,
    }
}

fn failed_candidate<'a, A, E>(
    analysis: Option<SentenceAnalysis<'a>>,
    error: CandidateError<A, E>,
    inputs: &StoryAssessmentInputs<'_>,
) -> StoryCandidateAssessment<'a, CandidateError<A, E>> {
    let targets = unrun_targets(inputs);
    StoryCandidateAssessment {
        assessment: CandidateAssessment::ExecutionError { analysis, error },
        targets,
        plan_departures: Vec::new(),
    }
}

fn unrun_targets(inputs: &StoryAssessmentInputs<'_>) -> Vec<TargetObservation> {
    [
        (TargetKind::Vocabulary, &inputs.request().targets.vocabulary),
        (TargetKind::Grammar, &inputs.request().targets.grammar),
    ]
    .into_iter()
    .flat_map(|(kind, ids)| {
        ids.iter().map(move |id| TargetObservation {
            kind,
            id: id.clone(),
            state: TargetState::NotRun,
            spans: Vec::new(),
            uncertainties: Vec::new(),
        })
    })
    .collect()
}
