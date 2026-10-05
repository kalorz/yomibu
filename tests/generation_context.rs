use yomibu::{
    evaluation::{EvaluationBindings, GrammarBinding, GrammarRule, VocabularyEntry},
    generation_context::{ContextError, VocabularyEntryId, select_context},
    grammar::GrammarDeclarations,
};

fn word(form: &str, reading: &str, sense: &str, direct_object: bool) -> VocabularyEntry {
    VocabularyEntry {
        written_form: form.into(),
        reading: reading.into(),
        sense: sense.into(),
        direct_object,
    }
}

fn permissions() -> (GrammarDeclarations, EvaluationBindings) {
    (
        GrammarDeclarations::from_descriptions(["は — topic marker", "ます — polite nonpast"])
            .unwrap(),
        EvaluationBindings {
            vocabulary: vec![
                word("寝る", "ネル", "sleep", false),
                word("猫", "ネコ", "cat", false),
                word("犬", "イヌ", "dog", false),
                word("歩く", "アルク", "walk", false),
            ],
            grammar: vec![
                GrammarBinding {
                    declaration_id: 1,
                    rule: GrammarRule::TopicWa,
                },
                GrammarBinding {
                    declaration_id: 2,
                    rule: GrammarRule::PoliteNonPast,
                },
            ],
        },
    )
}

#[test]
fn checked_focus_and_full_inventory_bounds() {
    assert!(VocabularyEntryId::new(0).is_err());
    let focus = VocabularyEntryId::new(1).unwrap();
    assert_eq!(focus.get(), 1);
    let (grammar, mut bindings) = permissions();
    bindings.vocabulary[3].sense = "x".repeat(1025);
    assert!(matches!(
        select_context(&grammar, &bindings, focus),
        Err(ContextError::Limit { .. })
    ));
    bindings.vocabulary[3].sense = " ".into();
    assert!(matches!(
        select_context(&grammar, &bindings, focus),
        Err(ContextError::Bindings(_))
    ));
    bindings.vocabulary[3].sense = "walk".into();
    assert!(matches!(
        select_context(&grammar, &bindings, VocabularyEntryId::new(5).unwrap()),
        Err(ContextError::Unavailable { .. })
    ));
}

#[test]
fn field_and_collection_limits_are_inclusive_for_library_callers() {
    let focus = VocabularyEntryId::new(1).unwrap();
    for field in ["written_form", "reading", "sense"] {
        let (grammar, mut bindings) = permissions();
        let limit = if field == "sense" { 1024 } else { 256 };
        let value = match field {
            "written_form" => &mut bindings.vocabulary[3].written_form,
            "reading" => &mut bindings.vocabulary[3].reading,
            _ => &mut bindings.vocabulary[3].sense,
        };
        *value = format!("{}x", "猫".repeat((limit - 1) / 3));
        while value.len() < limit {
            value.push('x');
        }
        assert!(select_context(&grammar, &bindings, focus).is_ok());
        match field {
            "written_form" => &mut bindings.vocabulary[3].written_form,
            "reading" => &mut bindings.vocabulary[3].reading,
            _ => &mut bindings.vocabulary[3].sense,
        }
        .push('x');
        assert!(matches!(
            select_context(&grammar, &bindings, focus),
            Err(ContextError::Limit { .. })
        ));
    }
    let (grammar, mut bindings) = permissions();
    bindings
        .vocabulary
        .resize(10_000, word("他", "ホカ", "other", false));
    assert!(select_context(&grammar, &bindings, focus).is_ok());
    bindings.vocabulary.push(word("余", "ヨ", "extra", false));
    assert!(matches!(
        select_context(&grammar, &bindings, focus),
        Err(ContextError::Limit { .. })
    ));
    let (_, mut bindings) = permissions();
    let mut descriptions = vec!["x".repeat(1024); 128];
    let grammar = GrammarDeclarations::from_descriptions(&descriptions).unwrap();
    bindings.grammar.resize(512, bindings.grammar[0].clone());
    assert!(select_context(&grammar, &bindings, focus).is_ok());
    bindings.grammar.push(bindings.grammar[0].clone());
    assert!(matches!(
        select_context(&grammar, &bindings, focus),
        Err(ContextError::Limit { .. })
    ));
    bindings.grammar.pop();
    descriptions[127].push('x');
    let grammar = GrammarDeclarations::from_descriptions(&descriptions).unwrap();
    assert!(matches!(
        select_context(&grammar, &bindings, focus),
        Err(ContextError::Limit { .. })
    ));
    descriptions[127].pop();
    descriptions.push("extra".into());
    let grammar = GrammarDeclarations::from_descriptions(&descriptions).unwrap();
    assert!(matches!(
        select_context(&grammar, &bindings, focus),
        Err(ContextError::Limit { .. })
    ));
    let (grammar, mut bindings) = permissions();
    bindings.grammar[0].declaration_id = 3;
    assert!(matches!(
        select_context(&grammar, &bindings, focus),
        Err(ContextError::Bindings(_))
    ));
}

#[test]
fn pet_rest_selects_focus_first_and_explains_every_exclusion() {
    let (grammar, mut bindings) = permissions();
    bindings.vocabulary.push(bindings.vocabulary[0].clone());
    let context = select_context(&grammar, &bindings, VocabularyEntryId::new(5).unwrap()).unwrap();
    assert_eq!(context.situation().id, "pet-rest");
    assert_eq!(
        context.situation().description,
        "A pet rests. Describe the selected pet sleeping."
    );
    assert_eq!(
        context
            .selected()
            .iter()
            .map(|s| s.entry.get())
            .collect::<Vec<_>>(),
        [5, 2]
    );
    assert!(std::ptr::eq(
        context.selected()[0].vocabulary,
        &bindings.vocabulary[4]
    ));
    assert!(std::ptr::eq(context.permissions(), &bindings));
    assert!(std::ptr::eq(context.grammar(), &grammar));
    let selected = serde_json::to_value(context.selected()).unwrap();
    assert_eq!(selected[0]["reason"], "explicit_focus");
    assert_eq!(selected[0]["role"], "predicate");
    assert_eq!(
        selected[1]["reason"],
        "first_available_declared_alternative"
    );
    let exclusions = serde_json::to_value(context.excluded()).unwrap();
    assert_eq!(
        exclusions,
        serde_json::json!([
            {"reason":"duplicate_of_selected_tuple", "entries":[1]},
            {"reason":"unused_slot_alternative", "entries":[3]},
            {"reason":"outside_chosen_situation", "entries":[4]}
        ])
    );
    context.validate().unwrap();
    let participant_focus =
        select_context(&grammar, &bindings, VocabularyEntryId::new(3).unwrap()).unwrap();
    assert_eq!(
        participant_focus
            .selected()
            .iter()
            .map(|s| s.entry.get())
            .collect::<Vec<_>>(),
        [3, 1]
    );
}

fn reasons(error: ContextError) -> serde_json::Value {
    let ContextError::Unavailable { situations, .. } = error else {
        panic!("expected unavailable: {error:?}")
    };
    assert_eq!(situations.len(), 3);
    serde_json::to_value(situations).unwrap()
}

#[test]
fn all_situations_alternatives_and_missing_prerequisites_are_deterministic() {
    let (grammar, mut bindings) = permissions();
    let cat = VocabularyEntryId::new(2).unwrap();
    let context = select_context(&grammar, &bindings, cat).unwrap();
    assert_eq!(
        context
            .situations()
            .iter()
            .map(|s| s.feasible)
            .collect::<Vec<_>>(),
        [true, true, false]
    );
    bindings.vocabulary[0] = word("他", "ホカ", "other", false);
    let context = select_context(&grammar, &bindings, cat).unwrap();
    assert_eq!(context.situation().id, "pet-walk");
    assert_eq!(
        context
            .selected()
            .iter()
            .map(|s| s.entry.get())
            .collect::<Vec<_>>(),
        [2, 4]
    );
    assert_eq!(context.situations()[0].reasons.len(), 1);
    bindings.vocabulary.extend([
        word("読む", "ヨム", "read", true),
        word("先生", "センセイ", "teacher", false),
        word("学生", "ガクセイ", "student", false),
        word("本", "ホン", "book", false),
    ]);
    bindings.grammar.push(GrammarBinding {
        declaration_id: 1,
        rule: GrammarRule::ObjectWo,
    });
    for (focus, ids) in [
        (5, vec![5, 7, 8]),
        (6, vec![6, 5, 8]),
        (7, vec![7, 5, 8]),
        (8, vec![8, 5, 7]),
    ] {
        let context =
            select_context(&grammar, &bindings, VocabularyEntryId::new(focus).unwrap()).unwrap();
        assert_eq!(context.situation().id, "book-reading");
        assert_eq!(
            context
                .selected()
                .iter()
                .map(|s| s.entry.get())
                .collect::<Vec<_>>(),
            ids
        );
    }
    bindings.grammar.clear();
    let decisions = reasons(
        select_context(&grammar, &bindings, VocabularyEntryId::new(5).unwrap()).unwrap_err(),
    );
    assert_eq!(
        decisions[2]["reasons"],
        serde_json::json!([
            {"code":"missing_grammar","rule":"TopicWa"}, {"code":"missing_grammar","rule":"ObjectWo"}, {"code":"missing_grammar","rule":"PoliteNonPast"}
        ])
    );
    let decisions = reasons(
        select_context(&grammar, &bindings, VocabularyEntryId::new(1).unwrap()).unwrap_err(),
    );
    assert_eq!(decisions[0]["reasons"][0]["code"], "unsupported_focus");
    let decisions = reasons(
        select_context(
            &grammar,
            &EvaluationBindings::default(),
            VocabularyEntryId::new(1).unwrap(),
        )
        .unwrap_err(),
    );
    assert!(
        decisions
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["reasons"][0]["code"] == "focus_not_found")
    );
}

#[test]
fn competing_identities_block_associations_but_identical_duplicates_do_not() {
    let (grammar, mut bindings) = permissions();
    let focus = VocabularyEntryId::new(1).unwrap();
    bindings
        .vocabulary
        .push(word("猫", "ネコ", "different sense", false));
    let context = select_context(&grammar, &bindings, focus).unwrap();
    assert_eq!(context.selected()[1].entry.get(), 3);
    assert_eq!(
        serde_json::to_value(context.excluded()).unwrap()[0],
        serde_json::json!({"reason":"ambiguous_association","entries":[2,5]})
    );
    bindings.vocabulary[2].reading = "イヌー".into();
    let decisions = reasons(select_context(&grammar, &bindings, focus).unwrap_err());
    assert_eq!(
        decisions[0]["reasons"][0],
        serde_json::json!({"code":"ambiguous_support","role":"participant","entries":[2,5]})
    );
    for variant in [
        word("寝る", "ネルー", "sleep", false),
        word("寝る", "ネル", "lie down", false),
        word("寝る", "ネル", "sleep", true),
    ] {
        let (_, mut b) = permissions();
        b.vocabulary.push(variant);
        let decisions = reasons(select_context(&grammar, &b, focus).unwrap_err());
        assert_eq!(
            decisions[0]["reasons"][0],
            serde_json::json!({"code":"ambiguous_focus","entries":[1,5]})
        );
    }
    let (_, mut bindings) = permissions();
    bindings.vocabulary.push(bindings.vocabulary[1].clone());
    assert_eq!(
        select_context(&grammar, &bindings, focus)
            .unwrap()
            .selected()[1]
            .entry
            .get(),
        2
    );
    bindings.vocabulary[1] = word("猫", "ねこ", "cat", false);
    bindings.vocabulary.pop();
    let context = select_context(&grammar, &bindings, focus).unwrap();
    assert_eq!(
        serde_json::to_value(context.excluded()).unwrap()[0],
        serde_json::json!({"reason":"unsupported_tuple_variant","entries":[2]})
    );
}

#[test]
fn unrelated_inventory_changes_and_reordering_preserve_selected_tuples() {
    let (grammar, mut bindings) = permissions();
    let focus = VocabularyEntryId::new(1).unwrap();
    let baseline: Vec<_> = select_context(&grammar, &bindings, focus)
        .unwrap()
        .selected()
        .iter()
        .map(|s| s.vocabulary.clone())
        .collect();
    for n in 0..100 {
        bindings
            .vocabulary
            .push(word(&format!("別{n}"), "ベツ", "unrelated", false));
    }
    assert_eq!(
        select_context(&grammar, &bindings, focus)
            .unwrap()
            .selected()
            .iter()
            .map(|s| s.vocabulary)
            .collect::<Vec<_>>(),
        baseline.iter().collect::<Vec<_>>()
    );
    bindings.vocabulary.rotate_left(1);
    let context = select_context(
        &grammar,
        &bindings,
        VocabularyEntryId::new(bindings.vocabulary.len()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        context
            .selected()
            .iter()
            .map(|s| s.vocabulary)
            .collect::<Vec<_>>(),
        baseline.iter().collect::<Vec<_>>()
    );
}
