use super::{
    local::{ApplicationError, SetupIssue},
    source::access_expired,
};
use crate::application::input_file;
use crate::configuration::Configuration;
use crate::configuration::modules::ModuleId;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use std::path::Path;
use yomibu_components::wanikani_source as wanikani;
use yomibu_core::domain::{
    inventory::{LearnerInventory, ManualInventory},
    knowledge::LearnerKnowledgePolicy,
    source::WaniKaniSyncData,
    story::{PracticeTargets, StoryRequest, StoryTopic},
};

pub(super) fn read_manual(
    path: Option<&Path>,
) -> Result<Option<LearnerInventory>, ApplicationError> {
    path.map(|path| {
        Ok(LearnerInventory::from_manual(
            read_json::<ManualInventory>(path, "manual inventory", 4194304)?,
        )?)
    })
    .transpose()
}

pub(super) fn prepare_inventory(
    policy: &LearnerKnowledgePolicy,
    source: Option<&WaniKaniSyncData>,
    manual: Option<LearnerInventory>,
    now: DateTime<Utc>,
) -> Result<LearnerInventory, ApplicationError> {
    if source.is_some_and(|source| access_expired(source, now)) {
        return Err(ApplicationError::AccessExpired);
    }
    match (source, manual) {
        (Some(source), manual) => {
            let inventory =
                LearnerInventory::from_mapped(wanikani::map_content_and_progress(source)?, policy)?;
            match manual {
                Some(manual) => Ok(inventory.merge(manual)?),
                None => Ok(inventory),
            }
        }
        (None, Some(manual)) => Ok(manual),
        (None, None) => Err(ApplicationError::Setup {
            issues: vec![SetupIssue {
                module: ModuleId::Knowledge,
            }],
        }),
    }
}

pub(super) fn read_request(config: &Configuration) -> Result<StoryRequest, ApplicationError> {
    if let Some(path) = &config.story.request {
        let file: RequestFile = read_json(path, "story request", 65536)?;
        return Ok(StoryRequest {
            version: file.version,
            topic: match file.topic {
                RequestTopic::Omitted => saved_topic(config)?,
                RequestTopic::Supplied(topic) => topic,
            },
            targets: file.targets,
        });
    }
    Ok(StoryRequest {
        version: 1,
        topic: saved_topic(config)?,
        targets: PracticeTargets {
            vocabulary: config.story.vocabulary_targets.clone(),
            grammar: config.story.grammar_targets.clone(),
        },
    })
}

pub(super) fn read_json<T: DeserializeOwned>(
    path: &Path,
    kind: &'static str,
    limit: usize,
) -> Result<T, ApplicationError> {
    let bytes = input_file::read_bounded(path, kind, limit, &format!("{limit} bytes"))?;
    serde_json::from_slice(&bytes).map_err(|source| ApplicationError::InvalidJson { kind, source })
}

fn saved_topic(config: &Configuration) -> Result<Option<StoryTopic>, ApplicationError> {
    Ok(config
        .story
        .topic
        .as_ref()
        .map(|text| StoryTopic::new(text.clone()))
        .transpose()?)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestFile {
    version: u32,
    #[serde(default)]
    topic: RequestTopic,
    targets: PracticeTargets,
}
#[derive(Default)]
enum RequestTopic {
    #[default]
    Omitted,
    Supplied(Option<StoryTopic>),
}
impl<'de> serde::Deserialize<'de> for RequestTopic {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <Option<StoryTopic> as serde::Deserialize>::deserialize(deserializer).map(Self::Supplied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_file_topic_inherits_when_omitted_and_clears_when_null() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Configuration::from_overrides(
            dir.path().into(),
            Default::default(),
            &crate::application::Operation::Story,
        )
        .unwrap();
        let path = dir.path().join("request.json");
        config.story.request = Some(path.clone());
        config.story.topic = Some("saved topic".into());
        for (topic, expected) in [("", Some("saved topic")), ("\"topic\":null,", None)] {
            std::fs::write(
                &path,
                format!(r#"{{"version":1,{topic}"targets":{{"vocabulary":[],"grammar":[]}}}}"#),
            )
            .unwrap();
            let request = read_request(&config).unwrap();
            assert_eq!(request.topic.as_ref().map(StoryTopic::text), expected);
        }
    }

    #[test]
    fn source_preparation_preserves_mapping_and_inventory_error_categories() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../tests/fixtures/mixed.json")).unwrap();
        let mut source: WaniKaniSyncData =
            serde_json::from_value(fixture["snapshot"].clone()).unwrap();
        let now = source.sync_completed_at;
        source.subjects[2].characters = "x".repeat(257);
        let error = prepare_inventory(&LearnerKnowledgePolicy::default(), Some(&source), None, now)
            .unwrap_err();
        assert!(matches!(error, ApplicationError::Inventory(_)), "{error:?}");
        assert_eq!(
            error.to_string(),
            "Invalid learner inventory: invalid or duplicate vocabulary entry."
        );
        source.learner.id.clear();
        let error = prepare_inventory(&LearnerKnowledgePolicy::default(), Some(&source), None, now)
            .unwrap_err();
        assert!(
            matches!(error, ApplicationError::Validation(_)),
            "{error:?}"
        );
    }
}
