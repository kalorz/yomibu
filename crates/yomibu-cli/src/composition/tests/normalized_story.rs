#[path = "../../../../../tests/support/story_inventory.rs"]
mod story_inventory;

use crate::composition::{
    Credentials, LocalApp, Operation,
    configuration::{Configuration, Invocation, Patch, SelectionStep, components::GENERATION_KEY},
};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};
use yomibu_core::domain::{
    knowledge::LearnerProgress, knowledge::inventory::MappedContent,
    knowledge::learner::LearnerData,
};

fn normalized_data() -> LearnerData {
    LearnerData {
        records: story_inventory::inventory()
            .vocabulary
            .into_iter()
            .map(|word| {
                (
                    MappedContent::Vocabulary(word),
                    Some(LearnerProgress {
                        learning_started_at: chrono::DateTime::from_timestamp(1, 0),
                        ..Default::default()
                    }),
                )
            })
            .collect(),
        access_expires_at: None,
    }
}

#[tokio::test]
async fn normalized_data_uses_configured_selection_without_source_or_writer_capabilities() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "id":"result", "model":"returned", "status":"completed",
        "output":[{"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"{\"candidates\":[{\"sentences\":[\"猫です。\",\"朝です。\",\"寝ます。\"]}]}"}]}]
    }))).expect(2).mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    for (steps, expected_ids, expected_revisions) in [
        (
            None,
            ["walk", "dog"],
            vec!["lexical-topic-v1", "seeded-order-v1"],
        ),
        (
            Some(vec![SelectionStep::SeededOrder]),
            ["dog", "cat"],
            vec!["seeded-order-v1"],
        ),
    ] {
        let mut flags = Invocation::default();
        flags.pipeline.selection.steps = steps;
        flags.story.topic = Patch::Set("walk".into());
        flags.story.select = Some(2);
        let config =
            Configuration::from_overrides(dir.path().into(), flags, &Operation::Story).unwrap();
        let mut credentials = Credentials::default();
        credentials.supply(GENERATION_KEY, "unused-key".into());
        let report = LocalApp::new(config, credentials)
            .with_endpoints(crate::composition::ServiceEndpoints {
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
        assert_eq!(report.selection.vocabulary_ids, expected_ids);
        assert_eq!(
            report
                .selection
                .steps
                .iter()
                .map(|step| step.revision)
                .collect::<Vec<_>>(),
            expected_revisions
        );
        assert_eq!(report.selection.steps.last().unwrap().seed, Some(7));
        assert_eq!(report.generated.passages().len(), 1);
    }
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
