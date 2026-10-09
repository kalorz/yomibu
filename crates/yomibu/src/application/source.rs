use super::{
    local::{ApplicationError, SetupIssue},
    progress::{ProgressEvent, RunProgress, Step},
};
use crate::configuration::Configuration;
use crate::configuration::modules::{ModuleId, ModuleState};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use yomibu_components::{file_learning_store::cache, wanikani_source as wanikani};
use yomibu_core::capabilities::LearningStore;
use yomibu_core::domain::{
    inventory::LearnerInventory, knowledge::LearnerKnowledgePolicy, source::WaniKaniSyncData,
};

pub(super) fn cache_path(config: &Configuration) -> PathBuf {
    config
        .application
        .wanikani_cache
        .clone()
        .unwrap_or_else(|| config.application.data_dir.join("wanikani.json"))
}

pub(super) fn read_cache(
    config: &Configuration,
    use_default: bool,
) -> Result<Option<WaniKaniSyncData>, ApplicationError> {
    if !use_default && config.application.wanikani_cache.is_none() {
        return Ok(None);
    }
    match config
        .pipeline
        .components
        .learning_store
        .open(cache_path(config))
        .load_snapshot()
    {
        Ok(data) => Ok(Some(data)),
        Err(cache::CacheError::Missing { .. }) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn usable_cache(
    data: &WaniKaniSyncData,
    policy: &LearnerKnowledgePolicy,
    now: DateTime<Utc>,
) -> bool {
    !access_expired(data, now)
        && LearnerInventory::from_wanikani(data, policy)
            .is_ok_and(|inventory| !inventory.vocabulary.is_empty())
}

fn can_reuse_cache(config: &Configuration, data: &WaniKaniSyncData, now: DateTime<Utc>) -> bool {
    usable_cache(data, &config.pipeline.knowledge_policy, now)
        && now
            .signed_duration_since(data.sync_completed_at)
            .to_std()
            .is_ok_and(|age| age < config.application.cache_max_age)
}

pub(super) async fn prepare_source<F: FnMut(ProgressEvent)>(
    config: &Configuration,
    credentials: &super::Credentials,
    endpoint: &str,
    manual_selected: bool,
    previous: Option<WaniKaniSyncData>,
    now: DateTime<Utc>,
    progress: &mut RunProgress<F>,
) -> Result<Option<WaniKaniSyncData>, ApplicationError> {
    if manual_selected && config.application.wanikani_cache.is_none() {
        if config.enabled(ModuleId::Sync) {
            progress.state(
                ModuleId::Sync,
                ModuleState::Skipped {
                    reason: "Manual inventory selected".into(),
                },
            );
        }
        progress.skip(Step::Sync, "Manual inventory selected");
        return Ok(None);
    }
    if previous
        .as_ref()
        .is_some_and(|data| can_reuse_cache(config, data, now))
    {
        progress.state(
            ModuleId::Sync,
            if config.enabled(ModuleId::Sync) {
                ModuleState::Skipped {
                    reason: "Cache is fresh".into(),
                }
            } else {
                ModuleState::Disabled
            },
        );
        progress.skip(Step::Sync, "Cache is fresh");
        return Ok(previous);
    }
    let key = if config.enabled(ModuleId::Sync) {
        credentials.source()?
    } else {
        None
    };
    if !config.enabled(ModuleId::Sync) || key.is_none() {
        if previous
            .as_ref()
            .is_some_and(|data| access_expired(data, now))
        {
            return Err(ApplicationError::AccessExpired);
        }
        if previous.is_some() {
            progress.warn(
                ModuleId::Sync,
                "Using an old WaniKani cache; sync is disabled or no key was supplied.".into(),
            );
        }
        progress.skip(Step::Sync, "Sync disabled or no WaniKani key");
        return Ok(previous);
    }
    let started = progress.start(Step::Sync);
    let mut client = config.pipeline.components.source.client(
        key.ok_or_else(|| ApplicationError::Setup {
            issues: vec![SetupIssue {
                module: ModuleId::Sync,
            }],
        })?,
        endpoint,
    )?;
    let guard = match config
        .pipeline
        .components
        .learning_store
        .open(cache_path(config))
        .begin_sync()
    {
        Ok(guard) => guard,
        Err(cache::WriteError::Locked)
            if previous
                .as_ref()
                .is_some_and(|data| usable_cache(data, &config.pipeline.knowledge_policy, now)) =>
        {
            progress.warn(
                ModuleId::Sync,
                "Another writer is refreshing WaniKani; using the previous usable cache.".into(),
            );
            progress.state(
                ModuleId::Sync,
                ModuleState::Skipped {
                    reason: "Writer contention".into(),
                },
            );
            progress.skip(Step::Sync, "Writer contention");
            progress.finish(Step::Sync, started);
            return Ok(previous);
        }
        Err(error) => return Err(error.into()),
    };
    let rechecked = read_cache(config, true)?;
    if rechecked
        .as_ref()
        .is_some_and(|data| can_reuse_cache(config, data, now))
    {
        progress.state(
            ModuleId::Sync,
            ModuleState::Skipped {
                reason: "Another writer refreshed the cache".into(),
            },
        );
        progress.finish(Step::Sync, started);
        return Ok(rechecked);
    }
    let previous = rechecked.or(previous);
    match client.fetch().await {
        Ok(data) => {
            if access_expired(&data, now) {
                return Err(ApplicationError::AccessExpired);
            }
            guard.replace_snapshot(&data)?;
            progress.state(ModuleId::Sync, ModuleState::Available);
            progress.finish(Step::Sync, started);
            Ok(Some(data))
        }
        Err(error)
            if temporary_source_error(&error)
                && previous.as_ref().is_some_and(|data| {
                    usable_cache(data, &config.pipeline.knowledge_policy, now)
                }) =>
        {
            progress.warn(
                ModuleId::Sync,
                format!("{error} Using the previous usable cache."),
            );
            progress.state(
                ModuleId::Sync,
                ModuleState::Unavailable {
                    error: error.to_string(),
                },
            );
            progress.finish(Step::Sync, started);
            Ok(previous)
        }
        Err(error) => Err(error.into()),
    }
}

pub(super) fn access_expired(data: &WaniKaniSyncData, now: DateTime<Utc>) -> bool {
    data.learner.subscription.active
        && data
            .learner
            .subscription
            .period_ends_at
            .is_some_and(|expiry| now >= expiry)
}
fn temporary_source_error(error: &wanikani::Error) -> bool {
    matches!(
        error,
        wanikani::Error::Transport { .. }
            | wanikani::Error::RateLimitWait
            | wanikani::Error::Http {
                status: 429 | 500..=599,
                ..
            }
    )
}
