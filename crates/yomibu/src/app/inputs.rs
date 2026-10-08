use super::{
    config::Configuration,
    local::{ApplicationError, SetupIssue},
    modules::ModuleId,
    source::access_expired,
};
use crate::{
    adapters::input_file,
    domain::WaniKaniSyncData,
    inventory::{LearnerInventory, ManualInventory},
    knowledge::LearnerKnowledgePolicy,
    story::{PracticeTargets, StoryRequest, StoryTopic},
};
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use std::path::Path;

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
    if let Some(path) = &config.request {
        return read_json(path, "story request", 65536);
    }
    Ok(StoryRequest {
        version: 1,
        topic: config
            .topic
            .as_ref()
            .map(|text| StoryTopic::new(text.clone()))
            .transpose()?,
        targets: PracticeTargets {
            vocabulary: Vec::new(),
            grammar: Vec::new(),
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
