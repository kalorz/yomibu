use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu::{
    application::{Credentials, LocalApp, Operation},
    configuration::{Configuration, Invocation, components::GENERATION_KEY},
};
use yomibu_core::domain::{
    inventory::{InventoryWord, MappedContent, SourceOrigin},
    knowledge::LearnerProgress,
    learner::LearnerData,
};

fn normalized_data() -> LearnerData {
    LearnerData {
        records: vec![(
            MappedContent::Vocabulary(InventoryWord {
                id: "cat".into(),
                written_form: "猫".into(),
                readings: vec!["ねこ".into()],
                meanings: vec!["cat".into()],
                direct_object: None,
                origin: SourceOrigin::Manual,
            }),
            Some(LearnerProgress {
                learning_started_at: chrono::DateTime::from_timestamp(1, 0),
                ..Default::default()
            }),
        )],
        access_expires_at: None,
    }
}

#[tokio::test]
async fn story_accepts_normalized_data_without_source_or_writer_capabilities() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "id":"result", "model":"returned", "status":"completed",
        "output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"{\"candidates\":[{\"sentences\":[\"猫です。\",\"朝です。\",\"寝ます。\"]}]}"}]}]
    }))).expect(1).mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let config =
        Configuration::from_overrides(dir.path().into(), Invocation::default(), &Operation::Story)
            .unwrap();
    let mut credentials = Credentials::default();
    credentials.supply(GENERATION_KEY, "unused-key".into());
    let report = LocalApp::new(config, credentials)
        .with_endpoints(yomibu::application::ServiceEndpoints {
            openai: format!("{}/", server.uri()),
            ..Default::default()
        })
        .story(
            Some(normalized_data()),
            std::time::SystemTime::now().into(),
            7,
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(report.selection.vocabulary_ids, ["cat"]);
    assert_eq!(report.generated.passages().len(), 1);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
