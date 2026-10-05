use serde_json::{Value, json};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::{
    adapters::openai::{Client, ProviderError, prepare_focused_request},
    evaluation::EvaluationBindings,
    generation::GenerationError,
    generation_context::{VocabularyEntryId, select_context},
    grammar::GrammarDeclarations,
};

fn input() -> (GrammarDeclarations, EvaluationBindings) {
    let value: Value =
        serde_json::from_str(include_str!("fixtures/focused/pet-rest.json")).unwrap();
    (
        GrammarDeclarations::from_descriptions(
            value["grammar"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap()),
        )
        .unwrap(),
        serde_json::from_value(value["bindings"].clone()).unwrap(),
    )
}
fn response(payload: &str) -> Value {
    json!({"id":"resp_synthetic","model":"reported","status":"completed","output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":payload}]}]})
}

#[tokio::test]
async fn sends_the_prepared_bytes_once_and_records_local_context_provenance() {
    let (grammar, permissions) = input();
    let request = prepare_focused_request(
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap(),
    )
    .unwrap();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response(
            r#"{"candidates":["犬は寝ます。","猫は寝ます。"]}"#,
        )))
        .expect(1)
        .mount(&server)
        .await;
    let client =
        Client::with_base_url("synthetic-secret", &format!("{}/v1/", server.uri())).unwrap();
    let generated = client.generate_focused_candidates(&request).await.unwrap();
    assert_eq!(generated.texts(), &["犬は寝ます。", "猫は寝ます。"]);
    let received = server.received_requests().await.unwrap();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].body, request.body_utf8().as_bytes());
    assert_eq!(
        received[0].headers["authorization"],
        "Bearer synthetic-secret"
    );
    assert_eq!(generated.provenance().request_sha256, request.sha256());
    assert_eq!(generated.provenance().request_bytes, request.bytes());
    assert_eq!(
        generated.provenance().prompt_revision,
        "g2-focused-sentence-v2"
    );
    let provenance = serde_json::to_value(generated.provenance()).unwrap();
    assert_eq!(
        provenance["focused_context"],
        json!({"selector_revision":"g2-situations-v1","situation":"pet-rest","selected_entries":[1,2]})
    );
}

#[tokio::test]
async fn focused_adapter_preserves_whole_response_rejection_and_single_attempt() {
    let (grammar, permissions) = input();
    let request = prepare_focused_request(
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap(),
    )
    .unwrap();
    let server = MockServer::start().await;
    let destination = MockServer::start().await;
    let client =
        Client::with_base_url("synthetic-secret", &format!("{}/v1/", server.uri())).unwrap();
    let good = response(r#"{"candidates":["猫は寝ます。","猫は寝ます。"]}"#);
    let mut bodies: Vec<Vec<u8>> = [
        r#"{"candidates":["猫は寝ます。"]}"#,
        r#"{"candidates":["猫",null]}"#,
        r#"{"candidates":["猫","猫"],"extra":true}"#,
        r#"{"candidates":["猫","猫"],"candidates":["猫","猫"]}"#,
        "{",
        "```{}",
        r#"{"candidates":["猫","犬","鳥"]}"#,
    ]
    .into_iter()
    .map(|s| response(s).to_string().into_bytes())
    .collect();
    for (key, value) in [
        ("status", json!("incomplete")),
        ("output", json!([{"type":"function_call"}])),
        (
            "output",
            json!([{"type":"message","role":"assistant","status":"completed","content":[{"type":"refusal","refusal":"synthetic-secret"}]}]),
        ),
    ] {
        let mut body = good.clone();
        body[key] = value;
        bodies.push(body.to_string().into_bytes());
    }
    bodies.extend([vec![0xff], vec![b' '; 65537]]);
    for body in bodies {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .expect(1)
            .mount(&server)
            .await;
        let error = client
            .generate_focused_candidates(&request)
            .await
            .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("synthetic-secret"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
    for status in [302, 307, 401, 429, 500] {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(status).insert_header("location", destination.uri()),
            )
            .expect(1)
            .mount(&server)
            .await;
        assert!(matches!(
            client.generate_focused_candidates(&request).await,
            Err(GenerationError::Provider(ProviderError::Http { .. }))
        ));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
        assert!(destination.received_requests().await.unwrap().is_empty());
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    drop(listener);
    // A refused connection is an attempt, not evidence that a server received data.
    let client = Client::with_base_url("synthetic-secret", &base).unwrap();
    assert!(matches!(
        client.generate_focused_candidates(&request).await,
        Err(GenerationError::Provider(ProviderError::Transport))
    ));
}

#[tokio::test]
async fn real_sudachi_assessment_uses_full_permissions_and_keeps_fail_spans_and_partial_results() {
    use yomibu::{
        adapters::sudachi::SudachiAnalyzer,
        analysis::Sentence,
        evaluation::{CheckKind, CheckOutcome, CheckState, evaluate},
        generation::{
            CandidateAssessment, ContextUnitStatus, FocusCompleteness, FocusStatus,
            assess_context_usage, assess_focus_occurrence,
        },
    };
    let analyzer = SudachiAnalyzer::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/current/system_core.dic"),
    )
    .expect("explicit pinned dictionary prerequisite; run documented setup");
    let (grammar, permissions) = input();
    let prepared = prepare_focused_request(
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap(),
    )
    .unwrap();
    let server = MockServer::start().await;
    let client =
        Client::with_base_url("synthetic-secret", &format!("{}/v1/", server.uri())).unwrap();
    let overlong = "猫".repeat(101);
    for pair in [
        ["猫は寝ます。", "犬は寝ます。"],
        ["鳥は寝ます。", "犬は歩きます。"],
        [" \n", "猫は寝ます。"],
        [overlong.as_str(), "猫は寝ます。"],
        ["寝ます。寝ます。", "猫は寝ます。"],
        ["qzxvは寝ます。", "猫は寝ます。"],
        ["猫は美しい。寝ます。", "qzxvは歩きます。"],
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(response(&json!({"candidates":pair}).to_string())),
            )
            .expect(1)
            .mount(&server)
            .await;
        let generated = client.generate_focused_candidates(&prepared).await.unwrap();
        let assessments = generated.assess(&analyzer);
        assert_eq!(generated.texts(), &pair);
        for (text, assessment) in pair.iter().zip(&assessments) {
            if let Ok(sentence) = Sentence::new(text) {
                let direct = analyzer.analyze(sentence).unwrap();
                let expected = evaluate(&direct, &grammar, &permissions).unwrap();
                let CandidateAssessment::Completed {
                    analysis,
                    evaluation,
                } = assessment
                else {
                    panic!("expected real analysis")
                };
                assert_eq!(analysis, &direct);
                assert_eq!(**evaluation, expected);
                let usage = assess_context_usage(Some(analysis), prepared.context());
                let focus = assess_focus_occurrence(Some(analysis), prepared.context());
                match *text {
                    "猫は寝ます。" => {
                        assert_eq!(usage.units[0].status, ContextUnitStatus::SelectedEvidence);
                        assert_eq!(focus.status, FocusStatus::Observed);
                        assert_eq!(focus.completeness, FocusCompleteness::Complete);
                        assert_eq!(focus.occurrences[0].span, 6..9);
                    }
                    "犬は寝ます。" => {
                        assert_eq!(
                            usage.units[0].status,
                            ContextUnitStatus::UnselectedPermissionEvidence
                        );
                        assert_eq!(
                            evaluation.check(CheckKind::Vocabulary).state,
                            CheckState::Completed(CheckOutcome::Pass)
                        );
                    }
                    "鳥は寝ます。" => {
                        assert_eq!(usage.units[0].status, ContextUnitStatus::NoPermissionEntry);
                        assert_eq!(
                            evaluation.check(CheckKind::Vocabulary).state,
                            CheckState::Completed(CheckOutcome::Fail)
                        );
                        assert_eq!(
                            evaluation.check(CheckKind::Vocabulary).findings[0].span,
                            0..3
                        );
                    }
                    "犬は歩きます。" => assert_eq!(focus.status, FocusStatus::Absent),
                    "寝ます。寝ます。" => {
                        assert_eq!(focus.status, FocusStatus::Observed);
                        assert_eq!(focus.count, 2);
                    }
                    "qzxvは寝ます。" => {
                        assert_eq!(focus.status, FocusStatus::Observed);
                        assert_eq!(focus.completeness, FocusCompleteness::Partial);
                        assert_eq!(focus.count, 1);
                        assert_eq!(focus.occurrences[0].span, 7..10);
                        assert_eq!(usage.units[0].status, ContextUnitStatus::Unresolved);
                    }
                    "猫は美しい。寝ます。" => {
                        assert_eq!(focus.status, FocusStatus::Observed);
                        assert_eq!(focus.completeness, FocusCompleteness::Partial);
                        assert_eq!(focus.count, 1);
                        assert_eq!(focus.occurrences[0].span, 18..21);
                    }
                    "qzxvは歩きます。" => {
                        assert_eq!(focus.status, FocusStatus::Unassessable);
                        assert_eq!(focus.completeness, FocusCompleteness::Partial);
                        assert_eq!(focus.count, 0);
                    }
                    _ => unreachable!(),
                }
            } else {
                assert!(matches!(
                    assessment,
                    CandidateAssessment::ExecutionError { analysis: None, .. }
                ));
            }
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[test]
fn real_book_reading_retains_object_safeguard_and_observes_only_the_lexical_stem() {
    use yomibu::{
        adapters::sudachi::SudachiAnalyzer,
        analysis::Sentence,
        evaluation::{CheckKind, CheckOutcome, CheckState, evaluate},
        generation::{FocusStatus, OccurrenceEvidence, assess_focus_occurrence},
    };
    let analyzer = SudachiAnalyzer::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/a1/current/system_core.dic"),
    )
    .expect("explicit pinned dictionary prerequisite; run documented setup");
    let grammar = GrammarDeclarations::from_descriptions(["topic", "object", "polite"]).unwrap();
    let permissions:EvaluationBindings=serde_json::from_value(json!({"vocabulary":[{"written_form":"読む","reading":"ヨム","sense":"read","direct_object":true},{"written_form":"学生","reading":"ガクセイ","sense":"student","direct_object":false},{"written_form":"本","reading":"ホン","sense":"book","direct_object":false}],"grammar":[{"declaration_id":1,"rule":"TopicWa"},{"declaration_id":2,"rule":"ObjectWo"},{"declaration_id":3,"rule":"PoliteNonPast"}]})).unwrap();
    let context =
        select_context(&grammar, &permissions, VocabularyEntryId::new(1).unwrap()).unwrap();
    let analysis = analyzer
        .analyze(Sentence::new("学生は本を読みます。").unwrap())
        .unwrap();
    let evaluation = evaluate(&analysis, &grammar, &permissions).unwrap();
    assert_eq!(
        evaluation.check(CheckKind::Particles).state,
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
    assert_eq!(
        evaluation.check(CheckKind::Scope).state,
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
    let focus = assess_focus_occurrence(Some(&analysis), &context);
    assert_eq!(focus.status, FocusStatus::Observed);
    assert_eq!(focus.count, 1);
    assert_eq!(
        focus.occurrences[0].evidence,
        OccurrenceEvidence::RegularStem
    );
    assert_eq!(
        &analysis.sentence.text()[focus.occurrences[0].span.clone()],
        "読み"
    );
    let mut restricted = permissions;
    restricted.vocabulary.pop();
    restricted.grammar.clear();
    let evaluation = evaluate(&analysis, &grammar, &restricted).unwrap();
    assert_eq!(
        evaluation.outcome(),
        CheckState::Completed(CheckOutcome::Fail)
    );
    assert_eq!(
        evaluation.check(CheckKind::Vocabulary).findings[0].span,
        9..12
    );
    assert_eq!(
        evaluation.check(CheckKind::Scope).state,
        CheckState::Completed(CheckOutcome::Inconclusive)
    );
}
