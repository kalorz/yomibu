use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::{
    adapters::{embeddings::LexicalEmbedder, openai::Client, sudachi::SudachiAnalyzer},
    candidate::CandidateAssessment,
    evaluation::{CheckKind, CheckOutcome, CheckState},
    inventory::{LearnerInventory, ManualInventory},
    ports::Embedder,
    reports::story::StoryReport,
    retrieval::{EmbeddingCache, prepare_embedding_inputs},
    story::{StoryRequest, generate_story, plan_generation},
};
fn inventory() -> LearnerInventory {
    LearnerInventory::from_manual(serde_json::from_value::<ManualInventory>(json!({"version":1,"vocabulary":[
 {"id":"cat","written_form":"猫","readings":["ねこ"],"meanings":["cat"],"direct_object":null},
 {"id":"dog","written_form":"犬","readings":["いぬ"],"meanings":["dog"],"direct_object":null},
 {"id":"sleep","written_form":"寝る","readings":["ねる"],"meanings":["sleep"],"direct_object":false}],
 "grammar_declarations":[{"id":"topic","description":"topic は"},{"id":"polite","description":"polite nonpast"}],
 "grammar_bindings":[{"declaration_id":"topic","rule":"TopicWa"},{"declaration_id":"polite","rule":"PoliteNonPast"}]})).unwrap()).unwrap()
}
#[tokio::test]
async fn common_generation_sends_finalized_bytes_and_assesses_all_targets_and_full_inventory() {
    let inventory = inventory();
    let request:StoryRequest=serde_json::from_value(json!({"version":1,"brief":"A cat sleeping","targets":{"vocabulary":["cat","sleep"],"grammar":["polite"]}})).unwrap();
    let encoder = LexicalEmbedder::new();
    let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
    let cache = EmbeddingCache::from_vectors(
        encoder.model_identity().clone(),
        &inputs,
        encoder.embed(&inputs).await.unwrap(),
    )
    .unwrap();
    let plan = plan_generation(
        &inventory,
        &request,
        &cache,
        encoder.model_identity(),
        2,
        Default::default(),
    )
    .unwrap();
    let prepared_request = plan.prepared_request();
    assert!(!prepared_request.body_utf8().contains("いぬ"));
    assert!(prepared_request.body_utf8().contains("A cat sleeping"));
    let server = MockServer::start().await;
    let client = Client::with_base_url("synthetic", &format!("{}/v1/", server.uri())).unwrap();
    let analyzer = SudachiAnalyzer::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/a1/current/system_core.dic"),
    )
    .unwrap();
    for pair in [
        ["犬は寝ます。", "猫は寝ます。"],
        [" \n", "鳥は寝ます。"],
        ["qzxvは寝ます。", "猫は寝ます。"],
    ] {
        server.reset().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"test","model":"test","status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":json!({"candidates":pair}).to_string()}]}]}))).expect(1).mount(&server).await;
        let result = generate_story(&plan, &client, &analyzer).await.unwrap();
        let results = result.assessments();
        let report = serde_json::to_value(StoryReport::new(&request, &plan, &result)).unwrap();
        assert_eq!(
            report["candidates"][1]["assessment"]["evaluation"]["basis"],
            "full_learner_inventory"
        );
        assert_eq!(result.candidates().texts(), &pair);
        assert_eq!(
            server.received_requests().await.unwrap()[0].body,
            prepared_request.body_utf8().as_bytes()
        );
        if pair[0].trim().is_empty() {
            assert!(matches!(
                results[0].assessment,
                CandidateAssessment::ExecutionError { .. }
            ));
            assert_eq!(results[0].targets[0].state.status(), "not_run");
            assert_eq!(
                report["candidates"][0]["targets"][0]["uncertainties"],
                json!([])
            );
            let CandidateAssessment::Completed { evaluation, .. } = &results[1].assessment else {
                panic!()
            };
            assert_eq!(
                evaluation.check(CheckKind::Vocabulary).state,
                CheckState::Completed(CheckOutcome::Fail)
            );
            assert_eq!(
                evaluation.check(CheckKind::Vocabulary).coverage,
                "full learner inventory; contextual reading/sense unassessed"
            );
            assert_eq!(
                evaluation.check(CheckKind::Vocabulary).findings[0].span,
                0..3
            );
        } else if pair[0].starts_with("qzxv") {
            let uncertainty = &report["candidates"][0]["targets"][1]["uncertainties"][0];
            assert_eq!(uncertainty["scope"], "sentence_coverage");
            assert_eq!(uncertainty["span"], json!({"start":0,"end":4}));
            assert_eq!(uncertainty["inventory_entries"], json!([]));
            assert_eq!(
                uncertainty["reason"],
                json!({"code":"lexical","detail":"out_of_dictionary"})
            );
            assert_eq!(results[0].targets[1].state.status(), "observed");
            assert_eq!(
                serde_json::to_value(&results[0].targets[1]).unwrap()["completeness"],
                "partial"
            );
            assert_eq!(results[0].targets[0].state.status(), "unassessable");
        } else {
            let CandidateAssessment::Completed { evaluation, .. } = &results[0].assessment else {
                panic!()
            };
            assert_eq!(
                evaluation.check(CheckKind::Vocabulary).state,
                CheckState::Completed(CheckOutcome::Pass)
            );
            assert_eq!(results[0].targets[0].state.status(), "absent");
            assert_eq!(results[1].targets[0].state.status(), "observed");
            assert_eq!(results[1].targets[1].spans.len(), 1);
            assert_eq!(results[1].targets[1].spans[0], 6..9);
            assert_eq!(results[1].targets[2].state.status(), "observed");
            assert_eq!(results[0].plan_departures[0].span, 0..3);
        }
    }
}

#[tokio::test]
async fn target_observations_preserve_lexical_and_object_evidence_limits() {
    let analyzer = SudachiAnalyzer::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/a1/current/system_core.dic"),
    )
    .unwrap();
    let server = MockServer::start().await;
    let client = Client::with_base_url("synthetic", &format!("{}/v1/", server.uri())).unwrap();
    for case in [
        "alternatives",
        "competing",
        "unknown_object",
        "intransitive_object",
        "alternative_object",
        "competing_target_object",
        "explicit_object",
        "unsupported_morphology",
        "malformed_object",
        "competing_object",
        "compound_component",
    ] {
        let mut inventory = inventory();
        if case == "compound_component" {
            inventory.vocabulary[2].written_form = "東京".into();
            inventory.vocabulary[2].readings = vec!["とうきょう".into()];
            inventory.vocabulary[2].meanings = vec!["Tokyo".into()];
        }
        if case == "unsupported_morphology" {
            inventory.vocabulary[2].written_form = "高い".into();
            inventory.vocabulary[2].readings = vec!["たかい".into()];
            inventory.vocabulary[2].meanings = vec!["tall".into()];
        }
        if case == "alternatives" {
            inventory.vocabulary[2].meanings.push("lie down".into());
        }
        if case == "competing" {
            let mut other = inventory.vocabulary[2].clone();
            other.id = "another-sleep".into();
            other.meanings = vec!["lie down".into(), "go to bed".into()];
            inventory.vocabulary.push(other);
        }
        if case.ends_with("object") {
            inventory.vocabulary[2].written_form = "食べる".into();
            inventory.vocabulary[2].readings = vec!["たべる".into()];
            inventory.vocabulary[2].meanings = vec!["eat".into()];
            inventory.vocabulary[2].direct_object =
                (case == "explicit_object" || case == "competing_object").then_some(true);
            inventory
                .grammar_declarations
                .push(yomibu::inventory::InventoryGrammar {
                    id: "object".into(),
                    description: "object を".into(),
                });
            inventory
                .grammar_bindings
                .push(yomibu::inventory::InventoryGrammarBinding {
                    declaration_id: "object".into(),
                    rule: yomibu::grammar::GrammarRule::ObjectWo,
                });
        }
        if case == "intransitive_object" {
            inventory.vocabulary[2].written_form = "歩く".into();
            inventory.vocabulary[2].readings = vec!["あるく".into()];
            inventory.vocabulary[2].direct_object = Some(false);
        }
        if case == "alternative_object" {
            inventory.vocabulary[2].direct_object = Some(true);
            inventory.vocabulary[2]
                .meanings
                .push("another meaning".into());
        }
        if case == "competing_target_object" {
            inventory.vocabulary[2].direct_object = Some(true);
            let mut other = inventory.vocabulary[2].clone();
            other.id = "another-eat".into();
            other.direct_object = Some(false);
            inventory.vocabulary.push(other);
        }
        let request:StoryRequest=serde_json::from_value(json!({"version":1,"brief":"A cat resting","targets":{"vocabulary":["sleep"],"grammar":[]}})).unwrap();
        if case == "competing_object" {
            let mut alternative = inventory.vocabulary[2].clone();
            alternative.id = "another-use".into();
            alternative.meanings.push("another meaning".into());
            inventory.vocabulary.push(alternative);
            inventory
                .grammar_bindings
                .retain(|b| b.rule != yomibu::grammar::GrammarRule::ObjectWo);
        }
        let mut request = request;
        if case.ends_with("object") {
            request.targets.grammar.push("object".into());
        }
        let encoder = LexicalEmbedder::new();
        let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
        let cache = EmbeddingCache::from_vectors(
            encoder.model_identity().clone(),
            &inputs,
            encoder.embed(&inputs).await.unwrap(),
        )
        .unwrap();
        let plan = plan_generation(
            &inventory,
            &request,
            &cache,
            encoder.model_identity(),
            2,
            Default::default(),
        )
        .unwrap();
        let pair = if case == "compound_component" {
            ["東京都です。", "東京は東京都です。"]
        } else if case == "intransitive_object" {
            ["猫を歩きます。", "猫を歩きます。"]
        } else if case == "malformed_object" {
            ["猫を猫です。", "猫を猫です。"]
        } else if case == "unsupported_morphology" {
            ["猫は高い。", "猫は高い。"]
        } else if case.ends_with("object") {
            ["猫を食べます。", "猫を食べます。"]
        } else {
            ["猫は寝ます。", "qzxvは寝ます。"]
        };
        server.reset().await;
        Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"test","model":"test","status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":json!({"candidates":pair}).to_string()}]}]}))).mount(&server).await;
        let result = generate_story(&plan, &client, &analyzer).await.unwrap();
        let results = result.assessments();
        for (index, result) in results.iter().enumerate() {
            if case.ends_with("object") {
                let target = &result.targets[1];
                if case == "explicit_object" {
                    assert_eq!(target.state.status(), "observed", "{case}");
                    assert_eq!(target.state.completeness(), "complete", "{case}");
                    assert_eq!(target.spans.len(), 1);
                    assert_eq!(target.spans[0], 3..6);
                } else {
                    assert_eq!(target.state.status(), "unassessable", "{case}");
                    assert_eq!(target.state.completeness(), "partial", "{case}");
                    assert!(target.spans.is_empty(), "{case}");
                }
            }
            let CandidateAssessment::Completed { evaluation, .. } = &result.assessment else {
                panic!()
            };
            let kind = if case.ends_with("object") {
                CheckKind::Particles
            } else {
                CheckKind::Vocabulary
            };
            if case == "compound_component" {
                assert_eq!(
                    evaluation.check(CheckKind::Vocabulary).state,
                    CheckState::Completed(CheckOutcome::Fail)
                );
                assert_eq!(result.targets[0].state.completeness(), "partial");
                if index == 0 {
                    assert!(result.targets[0].spans.is_empty());
                } else {
                    assert_eq!(result.targets[0].spans.len(), 1);
                    assert_eq!(result.targets[0].spans[0], 0..6);
                }
                assert!(result.plan_departures.is_empty());
            } else if case != "unsupported_morphology" {
                assert_eq!(
                    evaluation.check(kind).state,
                    CheckState::Completed(CheckOutcome::Inconclusive),
                    "{case}"
                );
            }
            if !case.ends_with("object") {
                assert_eq!(result.targets[0].state.status(), "unassessable", "{case}");
            }
        }
    }
}

#[tokio::test]
async fn configured_count_controls_schema_transport_and_every_candidate_assessment() {
    let inventory = inventory();
    let encoder = LexicalEmbedder::new();
    let analyzer = SudachiAnalyzer::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/a1/current/system_core.dic"),
    )
    .unwrap();
    let server = MockServer::start().await;
    let client = Client::with_base_url("synthetic", &format!("{}/v1/", server.uri())).unwrap();
    for count in [1, 3, 8, 9] {
        let request: StoryRequest = serde_json::from_value(json!({"version":1,"brief":"A cat sleeping","targets":{"vocabulary":["cat","sleep"],"grammar":[]}})).unwrap();
        let inputs = prepare_embedding_inputs(&inventory, &request).unwrap();
        let cache = EmbeddingCache::from_vectors(
            encoder.model_identity().clone(),
            &inputs,
            encoder.embed(&inputs).await.unwrap(),
        )
        .unwrap();
        let plan = plan_generation(
            &inventory,
            &request,
            &cache,
            encoder.model_identity(),
            2,
            yomibu::story::StoryGenerationOptions {
                candidate_count: count,
            },
        )
        .unwrap();
        let prepared_request = plan.prepared_request();
        let body: serde_json::Value = serde_json::from_str(prepared_request.body_utf8()).unwrap();
        assert_eq!(
            body["text"]["format"]["schema"]["properties"]["candidates"]["minItems"],
            count
        );
        assert_eq!(
            body["text"]["format"]["schema"]["properties"]["candidates"]["maxItems"],
            count
        );
        assert_eq!(body["max_output_tokens"], count * 512);
        assert!(
            body["input"][0]["content"]
                .as_str()
                .unwrap()
                .contains(&format!("exactly {count}"))
        );
        for returned_count in [count, count - 1, count + 1] {
            let mut texts = vec!["猫は寝ます。"; returned_count];
            if count > 1 && returned_count == count {
                texts[1] = " \n";
            }
            server.reset().await;
            Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"test","model":"test","status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":json!({"candidates":texts}).to_string()}]}]}))).expect(1).mount(&server).await;
            let result = generate_story(&plan, &client, &analyzer).await;
            assert_eq!(
                server.received_requests().await.unwrap()[0].body,
                prepared_request.body_utf8().as_bytes()
            );
            if returned_count != count {
                assert!(
                    result.is_err(),
                    "requested {count}, returned {returned_count}"
                );
                continue;
            }
            let result = result.unwrap();
            assert_eq!(result.candidates().texts(), texts.as_slice());
            let assessments = result.assessments();
            assert_eq!(assessments.len(), count);
            for (i, assessment) in assessments.iter().enumerate() {
                assert_eq!(
                    matches!(
                        assessment.assessment,
                        CandidateAssessment::ExecutionError { .. }
                    ),
                    i == 1
                );
            }
        }
    }
}
