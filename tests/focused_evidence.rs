//! Structural/report boundary cases use explicit synthetic analysis. Real Sudachi
//! integration is tested separately; these are not substitute analyzer results.
use serde_json::Value;
use std::ops::Range;
use yomibu::{
    analysis::{AnalysisProvenance, LexicalUnit, Sentence, SentenceAnalysis, Token},
    evaluation::EvaluationBindings,
    generation::{assess_context_usage, assess_focus_occurrence},
    generation_context::{VocabularyEntryId, select_context},
    grammar::GrammarDeclarations,
};

fn input() -> (GrammarDeclarations, EvaluationBindings) {
    let v: Value = serde_json::from_str(include_str!("fixtures/focused/pet-rest.json")).unwrap();
    (
        GrammarDeclarations::from_descriptions(["topic", "polite"]).unwrap(),
        serde_json::from_value(v["bindings"].clone()).unwrap(),
    )
}
fn token(span: Range<usize>, form: &str, reading: &str, pos: &str, class: &str) -> Token {
    Token {
        span,
        dictionary_form: form.into(),
        reading: reading.into(),
        part_of_speech: vec![
            pos.into(),
            "*".into(),
            "*".into(),
            "*".into(),
            class.into(),
            "*".into(),
        ],
        out_of_vocabulary: false,
    }
}
fn analysis<'a>(text: &'a str, tokens: Vec<Token>) -> SentenceAnalysis<'a> {
    SentenceAnalysis {
        sentence: Sentence::new(text).unwrap(),
        units: tokens
            .into_iter()
            .map(|token| LexicalUnit {
                components: vec![token.clone()],
                token,
            })
            .collect(),
        provenance: AnalysisProvenance {
            analyzer_revision: "synthetic-boundary",
            dictionary_version: "synthetic-boundary",
            dictionary_sha256: "synthetic-boundary",
            configuration_sha256: "synthetic-boundary".into(),
        },
    }
}
#[test]
fn boundary_focus_uses_whole_dictionary_forms_stems_and_original_spans() {
    let (grammar, permissions) = input();
    let context =
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap();
    let a = analysis(
        "寝る寝",
        vec![
            token(0..6, "寝る", "ネル", "動詞", "下一段-ナ行"),
            token(6..9, "寝る", "ネ", "動詞", "下一段-ナ行"),
        ],
    );
    let report = serde_json::to_value(assess_focus_occurrence(Some(&a), &context)).unwrap();
    assert_eq!(report["status"], "observed");
    assert_eq!(report["count"], 2);
    assert_eq!(report["occurrences"][0]["evidence"], "noninflected");
    assert_eq!(report["occurrences"][1]["evidence"], "regular_stem");
    assert_eq!(
        report["occurrences"][1]["span"],
        serde_json::json!({"start":6,"end":9})
    );
    assert_eq!(report["contextual_reading_and_sense"], "not_assessed");
    let a = analysis("寝る", vec![token(0..6, "別", "ベツ", "名詞", "*")]);
    assert_eq!(
        serde_json::to_value(assess_focus_occurrence(Some(&a), &context)).unwrap()["status"],
        "absent"
    );
}
#[test]
fn boundary_ambiguity_precedes_unassessability_but_retains_compatible_occurrences() {
    let (grammar, permissions) = input();
    let context =
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap();
    let mut oov = token(6..9, "謎", "ナゾ", "名詞", "*");
    oov.out_of_vocabulary = true;
    let mut a = analysis(
        "寝る謎寝る",
        vec![
            token(0..6, "寝る", "ネル", "動詞", "下一段-ナ行"),
            oov,
            token(9..15, "寝る", "チガウ", "動詞", "下一段-ナ行"),
        ],
    );
    let r = serde_json::to_value(assess_focus_occurrence(Some(&a), &context)).unwrap();
    assert_eq!(r["status"], "ambiguous");
    assert_eq!(r["count"], 1);
    assert_eq!(r["uncertainties"][0]["reason"], "out_of_vocabulary");
    assert_eq!(r["uncertainties"][1]["reason"], "reading_disagreement");
    a.units[2].token.reading = "ネル".into();
    assert_eq!(
        serde_json::to_value(assess_focus_occurrence(Some(&a), &context)).unwrap()["status"],
        "unassessable"
    );
    a.units[2].token.part_of_speech[0] = "名詞".into();
    assert_eq!(
        serde_json::to_value(assess_focus_occurrence(Some(&a), &context)).unwrap()["status"],
        "ambiguous"
    );
    let a = analysis(
        "寝る",
        vec![token(0..6, "寝る", "ネル", "動詞", "unsupported")],
    );
    assert_eq!(
        serde_json::to_value(assess_focus_occurrence(Some(&a), &context)).unwrap()["status"],
        "unassessable"
    );
}
#[test]
fn boundary_components_never_prove_focus_and_invalid_analysis_is_not_sliced() {
    let (grammar, permissions) = input();
    let context =
        select_context(&grammar, &permissions, VocabularyEntryId::new(2).unwrap()).unwrap();
    let mut a = analysis("猫科", vec![token(0..6, "猫科", "ネコカ", "名詞", "*")]);
    a.units[0].components = vec![
        token(0..3, "猫", "ネコ", "名詞", "*"),
        token(3..6, "科", "カ", "名詞", "*"),
    ];
    let r = serde_json::to_value(assess_focus_occurrence(Some(&a), &context)).unwrap();
    assert_eq!(r["status"], "ambiguous");
    assert_eq!(r["count"], 0);
    assert_eq!(r["uncertainties"][0]["reason"], "component_only");
    assert_eq!(
        r["uncertainties"][0]["span"],
        serde_json::json!({"start":0,"end":3})
    );
    for invalid in 0..4 {
        let mut a = analysis("猫", vec![token(0..3, "猫", "ネコ", "名詞", "*")]);
        match invalid {
            0 => a.units[0].token.span = 1..3,
            1 => a.units[0].components.clear(),
            2 => a.units[0].token.part_of_speech.clear(),
            _ => a.units[0].token.span = 0..99,
        }
        assert_eq!(
            serde_json::to_value(assess_focus_occurrence(Some(&a), &context)).unwrap()["status"],
            "not_run"
        );
        assert_eq!(
            serde_json::to_value(assess_context_usage(Some(&a), &context)).unwrap()["status"],
            "not_run"
        );
    }
    assert_eq!(
        serde_json::to_value(assess_focus_occurrence(None, &context)).unwrap()["status"],
        "not_run"
    );
}
#[test]
fn boundary_context_membership_is_separate_from_permission_judgment() {
    let (grammar, mut permissions) = input();
    permissions
        .vocabulary
        .push(permissions.vocabulary[2].clone());
    let context =
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap();
    let a = analysis(
        "猫犬鳥は。",
        vec![
            token(0..3, "猫", "ネコ", "名詞", "*"),
            token(3..6, "犬", "イヌ", "名詞", "*"),
            token(6..9, "鳥", "トリ", "名詞", "*"),
            token(9..12, "は", "ハ", "助詞", "*"),
            token(12..15, "。", "。", "補助記号", "*"),
        ],
    );
    let report = serde_json::to_value(assess_context_usage(Some(&a), &context)).unwrap();
    let units = report["units"].as_array().unwrap();
    assert_eq!(units.len(), 3);
    assert_eq!(units[0]["status"], "selected_evidence");
    assert_eq!(units[0]["entries"], serde_json::json!([2]));
    assert_eq!(units[1]["status"], "unselected_permission_evidence");
    assert_eq!(units[1]["entries"], serde_json::json!([3, 5]));
    assert_eq!(units[2]["status"], "no_permission_entry");
    assert_eq!(units[2]["span"], serde_json::json!({"start":6,"end":9}));
    permissions.vocabulary[4].sense = "different".into();
    let context =
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap();
    let report = serde_json::to_value(assess_context_usage(Some(&a), &context)).unwrap();
    assert_eq!(report["units"][1]["status"], "unresolved");
    assert_eq!(report["units"][1]["reason"], "competing_identity");
}
