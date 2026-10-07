//! Supported constructions, morphology and object/predicate uncertainty.

use std::ops::Range;

use super::{
    Check, CheckOutcome, CheckState, Evaluation, EvaluationBindings, EvaluationError, Finding,
    GrammarRule, REPORT_NOTICE, UnassessedAspect, VocabularyEntry, combine, passed, problem,
};
use crate::{
    analysis::{SentenceAnalysis, Token},
    grammar::GrammarDeclarations,
};

/// Check explicit synthetic permissions and the complete bounded construction.
///
/// Supplied token spans/POS shapes and bindings are validated before slicing.
/// Dictionary hypotheses and sense labels are not contextual linguistic proof.
/// An unsupported construction is Inconclusive; a supported permission violation
/// is Fail. Neither is an execution error. This operation performs no I/O.
/// Object/predicate combinations keep Particles and Scope unresolved even with
/// a transitive-use binding; independent permission failures are retained.
pub fn evaluate(
    analysis: &SentenceAnalysis<'_>,
    grammar: &GrammarDeclarations,
    bindings: &EvaluationBindings,
) -> Result<Evaluation, EvaluationError> {
    validate_analysis(analysis)?;
    bindings.validate(grammar)?;
    let vocabulary = check_vocabulary(analysis, bindings);
    let text = analysis.sentence.text();
    let mut tokens: Vec<_> = analysis.units.iter().map(|unit| &unit.token).collect();
    if tokens
        .last()
        .is_some_and(|token| &text[token.span.clone()] == "。" && !token.out_of_vocabulary)
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
    let mut particles = passed("no particles present in supported construction");
    let mut inflection = passed("no verb predicate present");
    let mut nominal = passed("no nominal predicate in recognized construction");
    let mut scope = passed("one supported construction accounts for the complete text");
    if let [noun, copula] = predicate
        && object.is_none()
        && is_noun(noun)
        && surface(copula, text) == "です"
        && copula.part_of_speech[0] == "助動詞"
    {
        nominal = permission(bindings, GrammarRule::NominalDesu, copula.span.clone());
    } else if let Some(rule) = polite_form(predicate, text) {
        inflection = permission(
            bindings,
            rule,
            predicate[0].span.end..predicate[predicate.len() - 1].span.end,
        );
    } else {
        let unresolved = || {
            problem(
                CheckOutcome::Inconclusive,
                0..text.len(),
                "construction/applicability is outside the supported patterns",
            )
        };
        inflection = unresolved();
        nominal = unresolved();
        particles = unresolved();
        scope = unresolved();
    }
    if scope.state == CheckState::Completed(CheckOutcome::Pass)
        && let Some(wa) = topic
    {
        particles = permission(bindings, GrammarRule::TopicWa, wa.span.clone());
    }
    if scope.state == CheckState::Completed(CheckOutcome::Pass)
        && let Some((noun, wo)) = object
    {
        let verb = predicate[0];
        let transitive_use = has_direct_object_evidence(bindings, verb, text);
        if transitive_use {
            particles = combine(
                particles,
                permission(bindings, GrammarRule::ObjectWo, wo.span.clone()),
            );
            // Transitivity of one word cannot resolve its use in a multiword expression.
            let unresolved = || {
                problem(
                    CheckOutcome::Inconclusive,
                    noun.span.start..predicate[predicate.len() - 1].span.end,
                    "object/predicate combination has no multiword-expression assessment",
                )
            };
            particles = combine(particles, unresolved());
            scope = unresolved();
        } else {
            particles = combine(
                particles,
                problem(
                    CheckOutcome::Inconclusive,
                    wo.span.clone(),
                    "direct-object use lacks unambiguous lexical evidence",
                ),
            );
            scope = problem(
                CheckOutcome::Inconclusive,
                wo.span.clone(),
                "route, departure, and other を uses are outside scope",
            );
        }
    }
    Ok(Evaluation {
        notice: REPORT_NOTICE,
        unassessed: [
            UnassessedAspect::Naturalness,
            UnassessedAspect::MultiwordExpressions,
            UnassessedAspect::ContextualReadingAndSense,
        ],
        vocabulary,
        inflection,
        particles,
        nominal,
        scope,
    })
}

fn has_direct_object_evidence(bindings: &EvaluationBindings, verb: &Token, text: &str) -> bool {
    let mut uses = bindings
        .vocabulary
        .iter()
        .filter(|word| word.written_form == verb.dictionary_form);
    uses.next().is_some_and(|first| {
        first.direct_object && reading_matches(first, verb, text) && uses.all(|word| word == first)
    })
}

fn check_vocabulary(analysis: &SentenceAnalysis<'_>, bindings: &EvaluationBindings) -> Check {
    let mut findings = Vec::new();
    let mut outcome = CheckOutcome::Pass;
    for unit in &analysis.units {
        let token = &unit.token;
        if !token.out_of_vocabulary
            && ["助詞", "助動詞", "補助記号"].contains(&token.part_of_speech[0].as_str())
        {
            continue;
        }
        let mut matches = bindings
            .vocabulary
            .iter()
            .filter(|word| word.written_form == token.dictionary_form);
        let first = matches.next();
        let (result, reason) = if token.part_of_speech[0] == "空白" {
            (
                CheckOutcome::Inconclusive,
                "whitespace is outside the bounded lexical construction",
            )
        } else if token.out_of_vocabulary {
            (
                CheckOutcome::Inconclusive,
                "out-of-dictionary identity is unresolved",
            )
        } else if let Some(word) = first {
            if matches.any(|other| other != word)
                || !reading_matches(word, token, analysis.sentence.text())
            {
                (
                    CheckOutcome::Inconclusive,
                    "reading or sense alternatives are unresolved",
                )
            } else {
                (CheckOutcome::Pass, "")
            }
        } else {
            (CheckOutcome::Fail, "whole word is not permitted")
        };
        if result != CheckOutcome::Pass {
            findings.push(Finding {
                span: token.span.clone(),
                reason,
            });
            if outcome != CheckOutcome::Fail {
                outcome = result;
            }
        }
    }
    Check {
        state: CheckState::Completed(outcome),
        findings,
        coverage: "whole-word permissions; contextual sense unassessed",
    }
}

pub(crate) fn reading_matches(word: &VocabularyEntry, token: &Token, text: &str) -> bool {
    if token.part_of_speech[0] == "動詞" && surface(token, text) != token.dictionary_form {
        regular_stem(&word.reading, token).is_some_and(|reading| reading == token.reading)
    } else {
        word.reading == token.reading
    }
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

pub(crate) fn regular_stem(base: &str, token: &Token) -> Option<String> {
    let class = &token.part_of_speech[4];
    if class.starts_with("五段-") {
        godan_stem(base)
    } else if class.starts_with("上一段-") || class.starts_with("下一段-") {
        base.strip_suffix(['る', 'ル']).map(str::to_owned)
    } else {
        None
    }
}

fn godan_stem(base: &str) -> Option<String> {
    let (index, last) = base.char_indices().next_back()?;
    let replacement = match last {
        'う' => 'い',
        'く' => 'き',
        'ぐ' => 'ぎ',
        'す' => 'し',
        'つ' => 'ち',
        'ぬ' => 'に',
        'ぶ' => 'び',
        'む' => 'み',
        'る' => 'り',
        'ウ' => 'イ',
        'ク' => 'キ',
        'グ' => 'ギ',
        'ス' => 'シ',
        'ツ' => 'チ',
        'ヌ' => 'ニ',
        'ブ' => 'ビ',
        'ム' => 'ミ',
        'ル' => 'リ',
        _ => return None,
    };
    Some(format!("{}{replacement}", &base[..index]))
}

fn permission(bindings: &EvaluationBindings, rule: GrammarRule, span: Range<usize>) -> Check {
    if bindings.grammar.iter().any(|binding| binding.rule == rule) {
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

/// Positive rule occurrences only within the same bounded recognized shapes.
/// None means the structure is unsupported, not that a target is absent.
pub(crate) fn observed_grammar(
    analysis: &SentenceAnalysis<'_>,
    bindings: &EvaluationBindings,
) -> Option<Vec<(GrammarRule, Range<usize>)>> {
    let text = analysis.sentence.text();
    let mut tokens: Vec<_> = analysis.units.iter().map(|u| &u.token).collect();
    if tokens
        .last()
        .is_some_and(|t| surface(t, text) == "。" && !t.out_of_vocabulary)
    {
        tokens.pop();
    }
    let mut predicate = tokens.as_slice();
    let mut found = Vec::new();
    if let [noun, wa, rest @ ..] = predicate
        && is_noun(noun)
        && is_particle(wa, "は", text)
    {
        found.push((GrammarRule::TopicWa, wa.span.clone()));
        predicate = rest;
    }
    if let [noun, wo, rest @ ..] = predicate
        && is_noun(noun)
        && is_particle(wo, "を", text)
    {
        if !has_direct_object_evidence(bindings, rest.first()?, text) {
            return None;
        }
        found.push((GrammarRule::ObjectWo, wo.span.clone()));
        predicate = rest;
    }
    if let [noun, copula] = predicate
        && !found.iter().any(|(rule, _)| *rule == GrammarRule::ObjectWo)
        && is_noun(noun)
        && surface(copula, text) == "です"
        && copula.part_of_speech[0] == "助動詞"
    {
        found.push((GrammarRule::NominalDesu, copula.span.clone()));
    } else {
        let rule = polite_form(predicate, text)?;
        let end = predicate.last()?.span.end;
        found.push((rule, predicate.first()?.span.end..end));
    }
    Some(found)
}
