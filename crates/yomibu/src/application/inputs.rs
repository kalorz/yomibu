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
        (Some(source), Some(manual)) => {
            Ok(LearnerInventory::from_wanikani(source, policy)?.merge(manual)?)
        }
        (Some(source), None) => Ok(LearnerInventory::from_wanikani(source, policy)?),
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
