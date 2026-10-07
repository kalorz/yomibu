use serde_json::json;
use yomibu::{
    inventory::{LearnerInventory, ManualInventory},
    retrieval::{EmbeddingCache, EmbeddingModelIdentity, prepare_embedding_inputs},
    story::{StoryError, StoryRequest, fit_selection_and_build_request, select_vocabulary},
};
fn inventory() -> LearnerInventory {
    LearnerInventory::from_manual(serde_json::from_value::<ManualInventory>(json!({"version":1,
        "vocabulary":[
            {"id":"cat","written_form":"猫","readings":["ねこ"],"meanings":["cat"],"direct_object":null},
            {"id":"station","written_form":"駅","readings":["えき"],"meanings":["station"],"direct_object":null},
            {"id":"meet","written_form":"会う","readings":["あう"],"meanings":["meet"],"direct_object":null}],
        "grammar_declarations":[{"id":"past","description":"polite past"}],
        "grammar_bindings":[{"declaration_id":"past","rule":"PolitePast"}]})).unwrap()).unwrap()
}
fn request() -> StoryRequest {
    serde_json::from_value(json!({"version":1,"brief":"Meeting at a train station","targets":{"vocabulary":["cat"],"grammar":["past"]}})).unwrap()
}
fn model() -> EmbeddingModelIdentity {
    EmbeddingModelIdentity {
        provider: "test".into(),
        model: "fixture-vectors".into(),
        revision: "1".into(),
        dimensions: 2,
        encoding_revision: "1".into(),
    }
}
#[test]
fn semantic_ranking_keeps_explicit_targets_and_full_inventory_separate() {
    let inventory = inventory();
    let request = request();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let cache = EmbeddingCache::from_vectors(
        model(),
        &inputs,
        vec![vec![0., 1.], vec![1., 0.], vec![1., 0.], vec![1., 0.]],
    )
    .unwrap();
    let plan = select_vocabulary(&inventory, &request, &cache, &model(), 2).unwrap();
    assert_eq!(
        plan.selected
            .iter()
            .map(|s| s.word.id.as_str())
            .collect::<Vec<_>>(),
        ["cat", "meet"]
    );
    assert_eq!(plan.selected[0].reason, "practice_target");
    assert_eq!(inventory.vocabulary.len(), 3);
    assert_eq!(plan.grammar_targets, ["past"]);
}
#[test]
fn stale_model_text_and_invalid_vectors_fail_before_selection() {
    let inventory = inventory();
    let mut request = request();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    for vector in [
        vec![0., 0.],
        vec![f32::NAN, 1.],
        vec![1.],
        vec![f32::INFINITY, 1.],
    ] {
        assert!(EmbeddingCache::from_vectors(model(), &inputs, vec![vector; 4]).is_err());
    }
    let cache = EmbeddingCache::from_vectors(model(), &inputs, vec![vec![1., 0.]; 4]).unwrap();
    let mut changed = model();
    changed.revision = "2".into();
    assert!(select_vocabulary(&inventory, &request, &cache, &changed, 2).is_err());
    request.brief = "flowers".into();
    assert!(select_vocabulary(&inventory, &request, &cache, &model(), 2).is_err());
}
#[test]
fn targets_cannot_grant_inventory_entries_or_grammar_recognizers() {
    let inventory = inventory();
    let mut r = request();
    r.targets.vocabulary.push("missing".into());
    assert!(r.validate(&inventory).is_err());
    let mut r = request();
    r.targets.grammar.push("question-ka".into());
    assert!(r.validate(&inventory).is_err());
    let mut r = request();
    r.targets.vocabulary.push("cat".into());
    assert!(r.validate(&inventory).is_err());
    let mut r = request();
    r.targets.vocabulary.clear();
    r.targets.grammar.clear();
    assert!(r.validate(&inventory).is_ok());
}

#[test]
fn duplicate_embedding_inputs_do_not_hide_invalid_provider_vectors() {
    let inventory = inventory();
    let request = request();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    assert!(
        EmbeddingCache::from_vectors(
            model(),
            &[inputs[0].clone(), inputs[0].clone()],
            vec![vec![1., 0.], vec![f32::NAN, 0.]]
        )
        .is_err()
    );
}

#[tokio::test]
async fn manual_and_wanikani_sources_share_embedding_inputs_and_prepared_request() {
    use yomibu::{
        adapters::embeddings::LexicalEmbedder, knowledge::LearnerKnowledgePolicy, ports::Embedder,
        story::fit_selection_and_build_request,
    };
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/preparation.json")).unwrap();
    let source = serde_json::from_value(fixture["snapshot"].clone()).unwrap();
    let wk = LearnerInventory::from_wanikani(&source, &LearnerKnowledgePolicy::default()).unwrap();
    let mut words = serde_json::to_value(&wk.vocabulary).unwrap();
    for word in words.as_array_mut().unwrap() {
        word.as_object_mut().unwrap().remove("origin");
    }
    let manual = LearnerInventory::from_manual(
        serde_json::from_value(
            json!({"version":1,"vocabulary":words,"grammar_declarations":[],"grammar_bindings":[]}),
        )
        .unwrap(),
    )
    .unwrap();
    let request: StoryRequest = serde_json::from_value(
        json!({"version":1,"brief":"one thing","targets":{"vocabulary":[],"grammar":[]}}),
    )
    .unwrap();
    let encoder = LexicalEmbedder::new();
    let a = prepare_embedding_inputs(&wk, &request).unwrap();
    let b = prepare_embedding_inputs(&manual, &request).unwrap();
    assert_eq!(
        a.iter().map(|i| i.key()).collect::<Vec<_>>(),
        b.iter().map(|i| i.key()).collect::<Vec<_>>()
    );
    let cache = EmbeddingCache::from_vectors(
        encoder.model_identity().clone(),
        &a,
        encoder.embed(&a).await.unwrap(),
    )
    .unwrap();
    let mut requests = Vec::new();
    for inventory in [&wk, &manual] {
        let plan =
            select_vocabulary(inventory, &request, &cache, encoder.model_identity(), 4).unwrap();
        let (_, prepared_request) =
            fit_selection_and_build_request(inventory, &request, plan, Default::default()).unwrap();
        requests.push(prepared_request);
    }
    assert_eq!(requests[0].body_utf8(), requests[1].body_utf8());
    assert_eq!(requests[0].sha256(), requests[1].sha256());
    assert!(!requests[0].body_utf8().contains("synthetic-learner"));
    assert!(!requests[0].body_utf8().contains("sync_completed_at"));
}

#[test]
fn generation_options_keep_count_separate_and_validate_real_arithmetic_bounds() {
    use yomibu::story::StoryGenerationOptions;
    assert_eq!(StoryGenerationOptions::default().candidate_count, 2);
    assert!(
        serde_json::to_value(request())
            .unwrap()
            .get("candidate_count")
            .is_none()
    );
    for count in [1, 2, 8, 9, 100, usize::MAX / 512] {
        assert!(
            StoryGenerationOptions {
                candidate_count: count
            }
            .validate()
            .is_ok()
        );
    }
    for count in [0, usize::MAX / 512 + 1, usize::MAX] {
        assert!(
            StoryGenerationOptions {
                candidate_count: count
            }
            .validate()
            .is_err()
        );
    }
}

#[test]
fn request_bounds_count_brief_bytes_and_each_target_kind_independently() {
    let mut inventory = inventory();
    let word = inventory.vocabulary[0].clone();
    for index in 0..17 {
        let mut word = word.clone();
        word.id = format!("word-{index}");
        inventory.vocabulary.push(word);
        inventory
            .grammar_declarations
            .push(yomibu::inventory::InventoryGrammar {
                id: format!("grammar-{index}"),
                description: "familiar grammar".into(),
            });
    }
    let mut request = request();
    request.targets.vocabulary = (0..16).map(|index| format!("word-{index}")).collect();
    request.targets.grammar = (0..16).map(|index| format!("grammar-{index}")).collect();
    request.brief = format!("{}ab", "猫".repeat(682));
    assert_eq!(request.brief.len(), 2048);
    assert!(request.validate(&inventory).is_ok());
    assert!(request.validate_selection_limit(16).is_ok());
    for limit in [0, 15, 17] {
        assert!(request.validate_selection_limit(limit).is_err());
    }

    request.brief.push('c');
    assert!(matches!(
        request.validate(&inventory),
        Err(StoryError::Invalid("version, brief or target limits"))
    ));
    request.brief.pop();
    request.targets.vocabulary.push("word-16".into());
    assert!(request.validate(&inventory).is_err());
    request.targets.vocabulary.pop();
    request.targets.grammar.push("grammar-16".into());
    assert!(request.validate(&inventory).is_err());
}

#[test]
fn request_budget_drops_lowest_ranked_supports_and_keeps_targets_and_grammar() {
    let mut inventory = inventory();
    for word in &mut inventory.vocabulary {
        word.meanings = vec!["x".repeat(1024); 5];
    }
    inventory.grammar_declarations[0].description = "g".repeat(1024);
    let request = request();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let cache = EmbeddingCache::from_vectors(model(), &inputs, vec![vec![1., 0.]; 4]).unwrap();
    let selection = select_vocabulary(&inventory, &request, &cache, &model(), 3).unwrap();
    assert_eq!(selection.selected.len(), 3);

    let (selection, prepared_request) =
        fit_selection_and_build_request(&inventory, &request, selection, Default::default())
            .unwrap();

    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["cat", "meet"]
    );
    assert!(prepared_request.body_utf8().len() <= 16384);
    let body: serde_json::Value = serde_json::from_str(prepared_request.body_utf8()).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(body["input"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        data["targets"],
        serde_json::to_value(&request.targets).unwrap()
    );
    assert_eq!(
        data["grammar_declarations"],
        serde_json::to_value(&inventory.grammar_declarations).unwrap()
    );
    assert_eq!(
        data["grammar_bindings"],
        serde_json::to_value(&inventory.grammar_bindings).unwrap()
    );
    assert_eq!(inventory.vocabulary.len(), 3);
}

#[test]
fn request_budget_rejects_targets_that_cannot_fit() {
    let mut inventory = inventory();
    for word in &mut inventory.vocabulary {
        word.meanings = vec!["x".repeat(1024); 12];
    }
    let mut request = request();
    request.targets.vocabulary = vec!["cat".into(), "station".into()];
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let cache = EmbeddingCache::from_vectors(model(), &inputs, vec![vec![1., 0.]; 4]).unwrap();
    let selection = select_vocabulary(&inventory, &request, &cache, &model(), 2).unwrap();

    assert!(matches!(
        fit_selection_and_build_request(&inventory, &request, selection, Default::default()),
        Err(StoryError::RequiredMaterialTooLarge { bytes, limit: 16384 }) if bytes > 16384
    ));
}
