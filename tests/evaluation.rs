use std::{path::Path, sync::OnceLock};

use yomibu::{
    adapters::sudachi::SudachiAnalyzer,
    analysis::Sentence,
    evaluation::{
        CheckKind, CheckOutcome, CheckState, EvaluationBindings, GrammarBinding, GrammarRule,
        UnassessedAspect, VocabularyEntry, evaluate,
    },
    grammar::GrammarDeclarations,
};

fn analyzer() -> &'static SudachiAnalyzer {
    static ANALYZER: OnceLock<SudachiAnalyzer> = OnceLock::new();
    ANALYZER.get_or_init(|| {
        SudachiAnalyzer::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/current/system_core.dic"),
        )
        .expect("install the pinned Core dictionary using the documented A1 setup")
    })
}

fn word(written_form: &str, reading: &str, sense: &str) -> VocabularyEntry {
    VocabularyEntry {
        written_form: written_form.into(),
        reading: reading.into(),
        sense: sense.into(),
        direct_object: false,
    }
}

#[test]
fn bindings_can_be_validated_before_analysis_without_changing_error_precedence() {
    use yomibu::evaluation::EvaluationError;
    let grammar = GrammarDeclarations::from_descriptions(["description"]).unwrap();
    let mut bindings = EvaluationBindings::default();
    assert!(bindings.validate(&grammar).is_ok());
    bindings.grammar.push(GrammarBinding {
        declaration_id: 2,
        rule: GrammarRule::NominalDesu,
    });
    assert!(matches!(
        bindings.validate(&grammar),
        Err(EvaluationError::MissingDeclaration)
    ));
    bindings.vocabulary.push(word("猫", " ", "cat"));
    assert!(matches!(
        bindings.validate(&grammar),
        Err(EvaluationError::BlankVocabulary)
    ));
    let mut analysis = analyzer()
        .analyze(Sentence::new("猫です。").unwrap())
        .unwrap();
    analysis.units.clear();
    assert!(matches!(
        evaluate(&analysis, &grammar, &bindings),
        Err(EvaluationError::InvalidAnalysis)
    ));
}

#[derive(serde::Deserialize)]
struct VisibleCase {
    id: String,
    sentence: String,
    grammar: Vec<String>,
    bindings: EvaluationBindings,
}

fn visible_case(id: &str) -> VisibleCase {
    #[derive(serde::Deserialize)]
    struct Packet {
        cases: Vec<VisibleCase>,
    }
    serde_json::from_str::<Packet>(include_str!("fixtures/a1/review-draft-v2.json"))
        .unwrap()
        .cases
        .into_iter()
        .find(|case| case.id == id)
        .expect("case must exist in the frozen visible packet")
}

#[test]
fn vocabulary_checks_whole_identity_without_promoting_components_or_dictionary_guesses() {
    let grammar = GrammarDeclarations::from_descriptions(["an arbitrary declaration"]).unwrap();
    let bindings = EvaluationBindings {
        vocabulary: vec![
            word("東京", "トウキョウ", "Tokyo"),
            word("都", "ト", "metropolis"),
        ],
        ..Default::default()
    };
    let analysis = analyzer()
        .analyze(Sentence::new("東京都").unwrap())
        .unwrap();
    let report = evaluate(&analysis, &grammar, &bindings).unwrap();
    assert_eq!(
        report.check(CheckKind::Vocabulary).state,
        CheckState::Completed(CheckOutcome::Fail)
    );
    assert_eq!(report.check(CheckKind::Vocabulary).findings[0].span, 0..9);
    assert_eq!(grammar.entries()[0].description, "an arbitrary declaration");

    let bindings = EvaluationBindings {
        vocabulary: vec![word("東京都", "トウキョウト", "Tokyo Metropolis")],
        ..Default::default()
    };
    let report = evaluate(&analysis, &grammar, &bindings).unwrap();
    assert_eq!(
        report.check(CheckKind::Vocabulary).state,
        CheckState::Completed(CheckOutcome::Pass)
    );
    // A bare word has no supported sentence predicate; membership alone is insufficient.
    assert_eq!(
        report.outcome(),
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
}

#[test]
fn vocabulary_keeps_unknown_tokens_and_competing_uses_inconclusive() {
    let grammar = GrammarDeclarations::from_descriptions(Vec::<String>::new()).unwrap();
    let analysis = analyzer().analyze(Sentence::new("猫").unwrap()).unwrap();
    for vocabulary in [
        vec![
            word("猫", "ネコ", "cat"),
            word("猫", "ネコ", "another intended sense"),
        ],
        vec![
            word("猫", "ネコ", "cat"),
            word("猫", "ビョウ", "another intended reading"),
        ],
        vec![word("猫", "ビョウ", "another intended reading")],
    ] {
        let report = evaluate(
            &analysis,
            &grammar,
            &EvaluationBindings {
                vocabulary,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            report.check(CheckKind::Vocabulary).state,
            CheckState::Completed(CheckOutcome::Inconclusive)
        );
    }
    let report = evaluate(
        &analysis,
        &grammar,
        &EvaluationBindings {
            vocabulary: vec![word("猫", "ネコ", "cat"), word("猫", "ネコ", "cat")],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        report.check(CheckKind::Vocabulary).state,
        CheckState::Completed(CheckOutcome::Pass)
    );
    let unknown = analyzer().analyze(Sentence::new("🦀").unwrap()).unwrap();
    let report = evaluate(&unknown, &grammar, &EvaluationBindings::default()).unwrap();
    assert_eq!(
        report.check(CheckKind::Vocabulary).state,
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
}

#[test]
fn malformed_analysis_and_blank_permissions_are_execution_errors() {
    let grammar = GrammarDeclarations::from_descriptions(Vec::<String>::new()).unwrap();
    let mut analysis = analyzer()
        .analyze(Sentence::new("東京都").unwrap())
        .unwrap();
    assert!(
        evaluate(
            &analysis,
            &grammar,
            &EvaluationBindings {
                vocabulary: vec![word("猫", " ", "cat")],
                ..Default::default()
            }
        )
        .is_err()
    );
    analysis.units[0].components[0].span = 1..3;
    assert!(evaluate(&analysis, &grammar, &EvaluationBindings::default()).is_err());
    analysis.units.clear();
    assert!(evaluate(&analysis, &grammar, &EvaluationBindings::default()).is_err());
}

#[test]
fn nominal_predicate_needs_an_explicit_binding_and_checks_every_required_area() {
    let grammar = GrammarDeclarations::from_descriptions(["free-form: nominal です"]).unwrap();
    let analysis = analyzer()
        .analyze(Sentence::new("猫です。").unwrap())
        .unwrap();
    let mut bindings = EvaluationBindings {
        vocabulary: vec![word("猫", "ネコ", "cat")],
        ..Default::default()
    };
    let report = evaluate(&analysis, &grammar, &bindings).unwrap();
    assert_eq!(
        report.check(CheckKind::Nominal).state,
        CheckState::Completed(CheckOutcome::Fail)
    );
    assert_eq!(report.check(CheckKind::Nominal).findings[0].span, 3..9);
    bindings.grammar.push(GrammarBinding {
        declaration_id: 1,
        rule: GrammarRule::NominalDesu,
    });
    let report = evaluate(&analysis, &grammar, &bindings).unwrap();
    assert_eq!(
        report.outcome(),
        CheckState::Completed(CheckOutcome::Pass),
        "{report:?} {analysis:?}"
    );
    for kind in [
        CheckKind::Vocabulary,
        CheckKind::Inflection,
        CheckKind::Particles,
        CheckKind::Nominal,
        CheckKind::Scope,
    ] {
        assert_eq!(
            report.check(kind).state,
            CheckState::Completed(CheckOutcome::Pass)
        );
    }
    bindings.grammar[0].declaration_id = 2;
    assert!(evaluate(&analysis, &grammar, &bindings).is_err());
}

#[test]
fn regular_godan_polite_forms_keep_the_base_identity_and_separate_permissions() {
    let grammar = GrammarDeclarations::from_descriptions(["this wording is not parsed"]).unwrap();
    for (text, rule, ending_span) in [
        ("読みます。", GrammarRule::PoliteNonPast, 6..12),
        ("読みました。", GrammarRule::PolitePast, 6..15),
        ("読みません。", GrammarRule::PoliteNegativeNonPast, 6..15),
        ("読みませんでした。", GrammarRule::PoliteNegativePast, 6..24),
    ] {
        let analysis = analyzer().analyze(Sentence::new(text).unwrap()).unwrap();
        let mut bindings = EvaluationBindings {
            vocabulary: vec![word("読む", "ヨム", "read")],
            ..Default::default()
        };
        let report = evaluate(&analysis, &grammar, &bindings).unwrap();
        assert_eq!(
            report.check(CheckKind::Vocabulary).state,
            CheckState::Completed(CheckOutcome::Pass),
            "{analysis:?}"
        );
        assert_eq!(
            report.check(CheckKind::Inflection).state,
            CheckState::Completed(CheckOutcome::Fail),
            "{analysis:?}"
        );
        let findings = &report.check(CheckKind::Inflection).findings;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].span, ending_span);
        bindings.grammar.push(GrammarBinding {
            declaration_id: 1,
            rule,
        });
        let report = evaluate(&analysis, &grammar, &bindings).unwrap();
        assert_eq!(
            report.outcome(),
            CheckState::Completed(CheckOutcome::Pass),
            "{report:?} {analysis:?}"
        );
        assert_eq!(analysis.units[0].token.dictionary_form, "読む");
    }
}

#[test]
fn ichidan_and_godan_endings_use_dictionary_class_not_just_ru_spelling() {
    let grammar = GrammarDeclarations::from_descriptions(["synthetic polite forms"]).unwrap();
    for (base, reading, stem) in [
        ("食べる", "タベル", "食べ"),
        ("見る", "ミル", "見"),
        ("帰る", "カエル", "帰り"),
        ("買う", "カウ", "買い"),
        ("書く", "カク", "書き"),
        ("泳ぐ", "オヨグ", "泳ぎ"),
        ("話す", "ハナス", "話し"),
        ("待つ", "マツ", "待ち"),
        ("死ぬ", "シヌ", "死に"),
        ("遊ぶ", "アソブ", "遊び"),
    ] {
        for (ending, rule) in [
            ("ます", GrammarRule::PoliteNonPast),
            ("ました", GrammarRule::PolitePast),
            ("ません", GrammarRule::PoliteNegativeNonPast),
            ("ませんでした", GrammarRule::PoliteNegativePast),
        ] {
            let text = format!("{stem}{ending}。");
            let analysis = analyzer().analyze(Sentence::new(&text).unwrap()).unwrap();
            let bindings = EvaluationBindings {
                vocabulary: vec![word(base, reading, "synthetic intended use")],
                grammar: vec![GrammarBinding {
                    declaration_id: 1,
                    rule,
                }],
            };
            let report = evaluate(&analysis, &grammar, &bindings).unwrap();
            assert_eq!(
                report.outcome(),
                CheckState::Completed(CheckOutcome::Pass),
                "{text}: {report:?} {analysis:?}"
            );
        }
    }
}

#[test]
fn a_single_nominal_topic_is_checked_separately_from_its_predicate() {
    let grammar =
        GrammarDeclarations::from_descriptions(["arbitrary topic", "arbitrary predicate"]).unwrap();
    for (text, predicate_word, predicate_rule) in [
        (
            "猫は動物です。",
            word("動物", "ドウブツ", "animal"),
            GrammarRule::NominalDesu,
        ),
        (
            "猫は歩きます。",
            word("歩く", "アルク", "walk"),
            GrammarRule::PoliteNonPast,
        ),
    ] {
        let analysis = analyzer().analyze(Sentence::new(text).unwrap()).unwrap();
        let mut bindings = EvaluationBindings {
            vocabulary: vec![word("猫", "ネコ", "cat"), predicate_word],
            grammar: vec![GrammarBinding {
                declaration_id: 2,
                rule: predicate_rule,
            }],
        };
        let report = evaluate(&analysis, &grammar, &bindings).unwrap();
        assert_eq!(
            report.check(CheckKind::Particles).state,
            CheckState::Completed(CheckOutcome::Fail)
        );
        assert_eq!(report.check(CheckKind::Particles).findings[0].span, 3..6);
        bindings.grammar.push(GrammarBinding {
            declaration_id: 1,
            rule: GrammarRule::TopicWa,
        });
        let report = evaluate(&analysis, &grammar, &bindings).unwrap();
        assert_eq!(
            report.outcome(),
            CheckState::Completed(CheckOutcome::Pass),
            "{report:?} {analysis:?}"
        );
        assert_eq!(grammar.entries()[0].description, "arbitrary topic");
    }
}

#[test]
fn object_wo_checks_permissions_without_claiming_compositional_support() {
    let grammar =
        GrammarDeclarations::from_descriptions(["topic", "object", "polite verb"]).unwrap();
    let mut read = word("読む", "ヨム", "read written material");
    read.direct_object = true;
    for text in ["本を読みます。", "猫は本を読みます。"] {
        let analysis = analyzer().analyze(Sentence::new(text).unwrap()).unwrap();
        let mut bindings = EvaluationBindings {
            vocabulary: vec![
                word("猫", "ネコ", "cat"),
                word("本", "ホン", "book"),
                read.clone(),
            ],
            grammar: vec![
                GrammarBinding {
                    declaration_id: 1,
                    rule: GrammarRule::TopicWa,
                },
                GrammarBinding {
                    declaration_id: 3,
                    rule: GrammarRule::PoliteNonPast,
                },
            ],
        };
        let report = evaluate(&analysis, &grammar, &bindings).unwrap();
        assert_eq!(
            report.check(CheckKind::Particles).state,
            CheckState::Completed(CheckOutcome::Fail)
        );
        let object_span = text.find('を').unwrap();
        assert_eq!(
            report.check(CheckKind::Particles).findings[0].span,
            object_span..object_span + 3
        );
        bindings.grammar.push(GrammarBinding {
            declaration_id: 2,
            rule: GrammarRule::ObjectWo,
        });
        let report = evaluate(&analysis, &grammar, &bindings).unwrap();
        assert_eq!(
            report.outcome(),
            CheckState::Completed(CheckOutcome::Inconclusive),
            "{report:?} {analysis:?}"
        );
        for kind in [CheckKind::Particles, CheckKind::Scope] {
            assert_eq!(
                report.check(kind).state,
                CheckState::Completed(CheckOutcome::Inconclusive)
            );
        }
        bindings.vocabulary[2].direct_object = false;
        let report = evaluate(&analysis, &grammar, &bindings).unwrap();
        assert_eq!(
            report.check(CheckKind::Particles).state,
            CheckState::Completed(CheckOutcome::Inconclusive)
        );
    }
    let analysis = analyzer()
        .analyze(Sentence::new("道を歩きます。").unwrap())
        .unwrap();
    let bindings = EvaluationBindings {
        vocabulary: vec![word("道", "ミチ", "path"), word("歩く", "アルク", "walk")],
        grammar: vec![
            GrammarBinding {
                declaration_id: 2,
                rule: GrammarRule::ObjectWo,
            },
            GrammarBinding {
                declaration_id: 3,
                rule: GrammarRule::PoliteNonPast,
            },
        ],
    };
    let report = evaluate(&analysis, &grammar, &bindings).unwrap();
    assert_eq!(
        report.outcome(),
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
}

#[test]
fn object_transitivity_does_not_resolve_visible_multiword_uses() {
    for (id, text) in [
        ("9a9f3acbee", "手を貸します。"),
        ("4edc1c61c7", "油を売ります。"),
        ("0d61be162d", "目を通します。"),
    ] {
        let case = visible_case(id);
        assert_eq!(case.sentence, text);
        let grammar = GrammarDeclarations::from_descriptions(case.grammar).unwrap();
        let analysis = analyzer()
            .analyze(Sentence::new(&case.sentence).unwrap())
            .unwrap();
        let report = evaluate(&analysis, &grammar, &case.bindings).unwrap();
        assert_eq!(
            report.outcome(),
            CheckState::Completed(CheckOutcome::Inconclusive),
            "{id}: {report:?}"
        );
        for kind in [CheckKind::Particles, CheckKind::Scope] {
            let check = report.check(kind);
            assert_eq!(
                check.state,
                CheckState::Completed(CheckOutcome::Inconclusive)
            );
            assert_eq!(check.findings.len(), 1);
            assert_eq!(check.findings[0].span, 0..18);
            assert_eq!(
                check.findings[0].reason,
                "object/predicate combination has no multiword-expression assessment"
            );
        }
        for kind in [
            CheckKind::Vocabulary,
            CheckKind::Inflection,
            CheckKind::Nominal,
        ] {
            assert_eq!(
                report.check(kind).state,
                CheckState::Completed(CheckOutcome::Pass)
            );
        }
    }
}

#[test]
fn object_uncertainty_preserves_permission_failures_and_original_spans() {
    for (id, kind, failure_span, combination_span) in [
        ("81d78d6bc2", CheckKind::Particles, 6..9, 0..21),
        ("c81dc42489", CheckKind::Particles, 3..6, 6..27),
    ] {
        let case = visible_case(id);
        let grammar = GrammarDeclarations::from_descriptions(case.grammar).unwrap();
        let analysis = analyzer()
            .analyze(Sentence::new(&case.sentence).unwrap())
            .unwrap();
        let report = evaluate(&analysis, &grammar, &case.bindings).unwrap();
        assert_eq!(report.outcome(), CheckState::Completed(CheckOutcome::Fail));
        let check = report.check(kind);
        assert_eq!(check.state, CheckState::Completed(CheckOutcome::Fail));
        assert_eq!(check.findings[0].span, failure_span);
        assert_eq!(
            check.findings[0].reason,
            "recognized form has no permission binding"
        );
        let scope = report.check(CheckKind::Scope);
        assert_eq!(
            scope.state,
            CheckState::Completed(CheckOutcome::Inconclusive)
        );
        assert_eq!(scope.findings[0].span, combination_span);
        assert_eq!(check.findings[1], scope.findings[0]);
    }

    let mut case = visible_case("9a9f3acbee");
    let grammar = GrammarDeclarations::from_descriptions(case.grammar).unwrap();
    let analysis = analyzer()
        .analyze(Sentence::new(&case.sentence).unwrap())
        .unwrap();
    case.bindings
        .grammar
        .retain(|binding| binding.rule != GrammarRule::PoliteNonPast);
    let report = evaluate(&analysis, &grammar, &case.bindings).unwrap();
    assert_eq!(report.outcome(), CheckState::Completed(CheckOutcome::Fail));
    assert_eq!(
        report.check(CheckKind::Inflection).state,
        CheckState::Completed(CheckOutcome::Fail)
    );
    assert_eq!(report.check(CheckKind::Inflection).findings[0].span, 12..18);
    assert_eq!(
        report.check(CheckKind::Scope).state,
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
}

#[test]
fn unsupported_text_leaves_grammar_applicability_unresolved_instead_of_unchecked_passes() {
    let grammar = GrammarDeclarations::from_descriptions(["all A1 forms"]).unwrap();
    let bindings = EvaluationBindings {
        vocabulary: vec![
            word("猫", "ネコ", "cat"),
            word("犬", "イヌ", "dog"),
            word("歩く", "アルク", "walk"),
            word("来る", "クル", "come"),
            word("する", "スル", "do"),
        ],
        grammar: [
            GrammarRule::NominalDesu,
            GrammarRule::TopicWa,
            GrammarRule::ObjectWo,
            GrammarRule::PoliteNonPast,
            GrammarRule::PolitePast,
            GrammarRule::PoliteNegativeNonPast,
            GrammarRule::PoliteNegativePast,
        ]
        .into_iter()
        .map(|rule| GrammarBinding {
            declaration_id: 1,
            rule,
        })
        .collect(),
    };
    for text in [
        "猫でした。",
        "猫ではありません。",
        "猫が歩きます。",
        "猫です。犬です。",
        "猫ですか。",
        "来ます。",
        "します。",
        "歩く。",
        " 猫です。",
        "猫を犬です。",
    ] {
        let analysis = analyzer().analyze(Sentence::new(text).unwrap()).unwrap();
        let report = evaluate(&analysis, &grammar, &bindings).unwrap();
        for kind in [
            CheckKind::Inflection,
            CheckKind::Particles,
            CheckKind::Nominal,
            CheckKind::Scope,
        ] {
            assert_eq!(
                report.check(kind).state,
                CheckState::Completed(CheckOutcome::Inconclusive),
                "{text}: {report:?} {analysis:?}"
            );
        }
        assert_ne!(report.outcome(), CheckState::Completed(CheckOutcome::Pass));
    }
}

#[test]
fn reports_expose_semantic_limits_even_when_the_bounded_checks_pass() {
    let grammar = GrammarDeclarations::from_descriptions(["nominal"]).unwrap();
    let bindings = EvaluationBindings {
        vocabulary: vec![word("猫", "ネコ", "cat")],
        grammar: vec![GrammarBinding {
            declaration_id: 1,
            rule: GrammarRule::NominalDesu,
        }],
    };
    let analysis = analyzer()
        .analyze(Sentence::new("猫です。").unwrap())
        .unwrap();
    let report = evaluate(&analysis, &grammar, &bindings).unwrap();
    assert_eq!(report.outcome(), CheckState::Completed(CheckOutcome::Pass));
    assert!(report.notice.contains("not accepted exercises"));
    assert_eq!(
        report.unassessed,
        [
            UnassessedAspect::Naturalness,
            UnassessedAspect::MultiwordExpressions,
            UnassessedAspect::ContextualReadingAndSense
        ]
    );
    let spaced = analyzer()
        .analyze(Sentence::new(" 猫です。").unwrap())
        .unwrap();
    let report = evaluate(&spaced, &grammar, &bindings).unwrap();
    assert_eq!(
        report.check(CheckKind::Vocabulary).state,
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
}
