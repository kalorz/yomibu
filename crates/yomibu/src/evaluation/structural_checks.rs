//! Supported constructions, morphology and object/predicate uncertainty.

use std::ops::Range;

use super::{
    Check, CheckOutcome, Evaluation, EvaluationBindings, EvaluationError, GrammarRule,
    REPORT_NOTICE, UnassessedAspect, combine,
    lexical::{DirectObjectEvidence, LexicalEvidence, LexicalPermissions, check_vocabulary},
    morphology::regular_stem,
    passed, problem,
};
use crate::{
    analysis::{SentenceAnalysis, Token},
    grammar::GrammarDeclarations,
    inventory::LearnerInventory,
};

pub(crate) struct SentenceAssessment {
    pub evaluation: Evaluation,
    pub lexical: Vec<LexicalEvidence>,
    pub construction: Option<RecognizedConstruction>,
}

pub(crate) struct RecognizedConstruction {
    pub rules: Vec<(GrammarRule, Range<usize>)>,
    pub object: Option<ObjectConstruction>,
}

pub(crate) struct ObjectConstruction {
    pub lexical_unit: usize,
    pub particle: Range<usize>,
    pub combination: Range<usize>,
}

/// Check explicit permissions after validating original token spans and bindings.
/// Object/predicate combinations remain unresolved even with transitive-use evidence.
pub fn evaluate(
    analysis: &SentenceAnalysis<'_>,
    grammar: &GrammarDeclarations,
    bindings: &EvaluationBindings,
) -> Result<Evaluation, EvaluationError> {
    validate_analysis(analysis)?;
    bindings.validate(grammar)?;
    let rules: Vec<_> = bindings
        .grammar
        .iter()
        .map(|binding| binding.rule)
        .collect();
    Ok(assess_permissions(
        analysis,
        &rules,
        LexicalPermissions::Explicit(&bindings.vocabulary),
    )
    .evaluation)
}

pub(crate) fn assess_inventory(
    analysis: &SentenceAnalysis<'_>,
    inventory: &LearnerInventory,
) -> Result<SentenceAssessment, EvaluationError> {
    validate_analysis(analysis)?;
    let rules: Vec<_> = inventory
        .grammar_bindings
        .iter()
        .map(|binding| binding.rule)
        .collect();
    Ok(assess_permissions(
        analysis,
        &rules,
        LexicalPermissions::Inventory(inventory),
    ))
}

fn assess_permissions(
    analysis: &SentenceAnalysis<'_>,
    rules: &[GrammarRule],
    permissions: LexicalPermissions<'_>,
) -> SentenceAssessment {
    let basis = permissions.basis();
    let lexical: Vec<_> = analysis
        .units
        .iter()
        .map(|unit| permissions.resolve(&unit.token, analysis.sentence.text()))
        .collect();
    let mut evaluation = Evaluation {
        notice: REPORT_NOTICE,
        unassessed: [
            UnassessedAspect::Naturalness,
            UnassessedAspect::MultiwordExpressions,
            UnassessedAspect::ContextualReadingAndSense,
        ],
        basis,
        vocabulary: check_vocabulary(analysis, &lexical, basis),
        particles: passed("no particles present in supported construction"),
        inflection: passed("no verb predicate present"),
        nominal: passed("no nominal predicate in recognized construction"),
        scope: passed("one supported construction accounts for the complete text"),
    };
    let construction = recognize_construction(analysis);
    if let Some(construction) = &construction {
        for (rule, span) in &construction.rules {
            match rule {
                GrammarRule::NominalDesu => {
                    evaluation.nominal = permission(rules, *rule, span.clone())
                }
                GrammarRule::TopicWa => {
                    evaluation.particles = permission(rules, *rule, span.clone())
                }
                GrammarRule::ObjectWo => {}
                GrammarRule::PoliteNonPast
                | GrammarRule::PolitePast
                | GrammarRule::PoliteNegativeNonPast
                | GrammarRule::PoliteNegativePast => {
                    evaluation.inflection = permission(rules, *rule, span.clone())
                }
            }
        }
        if let Some(object) = &construction.object {
            if lexical[object.lexical_unit].direct_object_evidence()
                == DirectObjectEvidence::Confirmed
            {
                evaluation.particles = combine(
                    evaluation.particles,
                    permission(rules, GrammarRule::ObjectWo, object.particle.clone()),
                );
                // A word-use assertion cannot resolve a multiword expression.
                let unresolved = || {
                    problem(
                        CheckOutcome::Inconclusive,
                        object.combination.clone(),
                        "object/predicate combination has no multiword-expression assessment",
                    )
                };
                evaluation.particles = combine(evaluation.particles, unresolved());
                evaluation.scope = unresolved();
            } else {
                evaluation.particles = combine(
                    evaluation.particles,
                    problem(
                        CheckOutcome::Inconclusive,
                        object.particle.clone(),
                        "direct-object use lacks unambiguous lexical evidence",
                    ),
                );
                evaluation.scope = problem(
                    CheckOutcome::Inconclusive,
                    object.particle.clone(),
                    "route, departure, and other を uses are outside scope",
                );
            }
        }
    } else {
        let unresolved = || {
            problem(
                CheckOutcome::Inconclusive,
                0..analysis.sentence.text().len(),
                "construction/applicability is outside the supported patterns",
            )
        };
        evaluation.inflection = unresolved();
        evaluation.nominal = unresolved();
        evaluation.particles = unresolved();
        evaluation.scope = unresolved();
    }
    SentenceAssessment {
        evaluation,
        lexical,
        construction,
    }
}

fn recognize_construction(analysis: &SentenceAnalysis<'_>) -> Option<RecognizedConstruction> {
    let text = analysis.sentence.text();
    let mut tokens: Vec<_> = analysis.units.iter().map(|unit| &unit.token).collect();
    if tokens
        .last()
        .is_some_and(|token| surface(token, text) == "。" && !token.out_of_vocabulary)
    {
        tokens.pop();
    }
    let mut predicate = tokens.as_slice();
    let topic = if let [noun, wa, rest @ ..] = predicate
        && is_noun(noun)
        && is_particle(wa, "は", text)
    {
        predicate = rest;
        Some(*wa)
    } else {
        None
    };
    let object = if let [noun, wo, rest @ ..] = predicate
        && is_noun(noun)
        && is_particle(wo, "を", text)
    {
        predicate = rest;
        Some((*noun, *wo))
    } else {
        None
    };
    let (rule, span) = if let [noun, copula] = predicate
        && object.is_none()
        && is_noun(noun)
        && surface(copula, text) == "です"
        && copula.part_of_speech[0] == "助動詞"
    {
        (GrammarRule::NominalDesu, copula.span.clone())
    } else {
        (
            polite_form(predicate, text)?,
            predicate.first()?.span.end..predicate.last()?.span.end,
        )
    };
    let mut rules = Vec::new();
    if let Some(wa) = topic {
        rules.push((GrammarRule::TopicWa, wa.span.clone()));
    }
    let object = object.map(|(noun, wo)| {
        rules.push((GrammarRule::ObjectWo, wo.span.clone()));
        ObjectConstruction {
            lexical_unit: tokens.len() - predicate.len(),
            particle: wo.span.clone(),
            combination: noun.span.start..span.end,
        }
    });
    rules.push((rule, span));
    Some(RecognizedConstruction { rules, object })
}

fn polite_form(tokens: &[&Token], text: &str) -> Option<GrammarRule> {
    let (verb, auxiliaries) = tokens.split_first()?;
    if verb.out_of_vocabulary
        || verb.part_of_speech[0] != "動詞"
        || regular_stem(&verb.dictionary_form, verb).as_deref() != Some(surface(verb, text))
        || auxiliaries.is_empty()
        || auxiliaries
            .iter()
            .any(|token| token.out_of_vocabulary || token.part_of_speech[0] != "助動詞")
    {
        return None;
    }
    match &text[verb.span.end..tokens.last()?.span.end] {
        "ます" => Some(GrammarRule::PoliteNonPast),
        "ました" => Some(GrammarRule::PolitePast),
        "ません" => Some(GrammarRule::PoliteNegativeNonPast),
        "ませんでした" => Some(GrammarRule::PoliteNegativePast),
        _ => None,
    }
}

fn permission(rules: &[GrammarRule], rule: GrammarRule, span: Range<usize>) -> Check {
    if rules.contains(&rule) {
        passed("recognized form has an explicit grammar binding")
    } else {
        problem(
            CheckOutcome::Fail,
            span,
            "recognized form has no permission binding",
        )
    }
}

fn surface<'a>(token: &Token, text: &'a str) -> &'a str {
    &text[token.span.clone()]
}

fn is_particle(token: &Token, spelling: &str, text: &str) -> bool {
    !token.out_of_vocabulary
        && token.part_of_speech[0] == "助詞"
        && surface(token, text) == spelling
}

fn is_noun(token: &Token) -> bool {
    !token.out_of_vocabulary
        && (token.part_of_speech[0] == "名詞" || token.part_of_speech[0] == "代名詞")
}

fn validate_analysis(analysis: &SentenceAnalysis<'_>) -> Result<(), EvaluationError> {
    let text = analysis.sentence.text();
    let mut end = 0;
    for unit in &analysis.units {
        if unit.token.span.start != end || !valid_token(&unit.token, text) {
            return Err(EvaluationError::InvalidAnalysis);
        }
        let mut component_end = unit.token.span.start;
        for component in &unit.components {
            if component.span.start != component_end || !valid_token(component, text) {
                return Err(EvaluationError::InvalidAnalysis);
            }
            component_end = component.span.end;
        }
        if component_end != unit.token.span.end {
            return Err(EvaluationError::InvalidAnalysis);
        }
        end = unit.token.span.end;
    }
    if end != text.len() {
        return Err(EvaluationError::InvalidAnalysis);
    }
    Ok(())
}

fn valid_token(token: &Token, text: &str) -> bool {
    !token.span.is_empty()
        && text.get(token.span.clone()).is_some()
        && token.part_of_speech.len() == 6
        && !token.dictionary_form.is_empty()
}
