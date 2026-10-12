use super::{
    local::{ApplicationError, SetupIssue},
    progress::{ProgressEvent, RunProgress, Step},
};
use crate::configuration::Configuration;
use chrono::{DateTime, Utc};
use std::{path::PathBuf, sync::Arc};
use yomibu_components::wanikani_source as wanikani;
use yomibu_core::capabilities::{LearningStore, SourceSyncWriter};
use yomibu_core::domain::{
    inventory::LearnerInventory, knowledge::LearnerKnowledgePolicy, source::WaniKaniSyncData,
};

pub(super) enum SourceClient<'a> {
    Local {
        credentials: &'a super::Credentials,
        endpoint: &'a str,
    },
    Supplied(&'a mut wanikani::Client),
    Unavailable,
}

/// Resolve the configured source cache path without I/O.
pub fn cache_path(config: &Configuration) -> PathBuf {
    config
        .application
        .wanikani_cache
        .clone()
        .unwrap_or_else(|| config.application.data_dir.join("wanikani.json"))
}

pub(super) fn load_cache<Store: LearningStore>(
    store: &Store,
) -> Result<Option<Arc<WaniKaniSyncData>>, ApplicationError>
where
    ApplicationError: From<Store::ReadError>,
{
    match store.load() {
        Ok(data) => Ok(Some(data)),
        Err(error) if store.is_missing(&error) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn usable_cache(
    data: &WaniKaniSyncData,
    policy: &LearnerKnowledgePolicy,
    now: DateTime<Utc>,
) -> bool {
    !access_expired(data, now)
        && wanikani::map_content_and_progress(data)
            .ok()
            .and_then(|records| LearnerInventory::from_mapped(records, policy).ok())
            .is_some_and(|inventory| !inventory.vocabulary.is_empty())
}

fn can_reuse_cache(config: &Configuration, data: &WaniKaniSyncData, now: DateTime<Utc>) -> bool {
    usable_cache(data, &config.pipeline.knowledge_policy, now)
        && now
            .signed_duration_since(data.sync_completed_at)
            .to_std()
            .is_ok_and(|age| age < config.application.cache_max_age)
}

pub(super) async fn prepare_source<Store: LearningStore, F: FnMut(ProgressEvent)>(
    config: &Configuration,
    source_client: SourceClient<'_>,
    store: Option<&Store>,
    previous: Option<Arc<WaniKaniSyncData>>,
    now: DateTime<Utc>,
    progress: &mut RunProgress<F>,
) -> Result<Option<Arc<WaniKaniSyncData>>, ApplicationError>
where
    ApplicationError: From<Store::ReadError> + From<Store::WriteError>,
{
    let Some(store) = store else {
        progress.skip(Step::Sync, "Manual inventory selected");
        return Ok(None);
    };
    if previous
        .as_ref()
        .is_some_and(|data| can_reuse_cache(config, data, now))
    {
        progress.skip(Step::Sync, "Cache is fresh");
        return Ok(previous);
    }
    let key = match &source_client {
        SourceClient::Local { credentials, .. } if config.application.sync => {
            credentials.source()?
        }
        _ => None,
    };
    let available = config.application.sync
        && match &source_client {
            SourceClient::Local { .. } => key.is_some(),
            SourceClient::Supplied(_) => true,
            SourceClient::Unavailable => false,
        };
    if !available {
        if previous
            .as_ref()
            .is_some_and(|data| access_expired(data, now))
        {
            return Err(ApplicationError::AccessExpired);
        }
        let (warning, reason) = match source_client {
            SourceClient::Local { .. } => (
                "Using an old WaniKani cache; sync is disabled or no key was supplied.",
                "Sync disabled or no WaniKani key",
            ),
            _ => (
                "Using an old WaniKani cache; sync is disabled or no source client was supplied.",
                "Sync disabled or no WaniKani client supplied",
            ),
        };
        if previous.is_some() {
            progress.warn(Step::Sync, warning.into());
        }
        progress.skip(Step::Sync, reason);
        return Ok(previous);
    }
    let started = progress.start(Step::Sync);
    let mut local_client;
    let client = match source_client {
        SourceClient::Local { endpoint, .. } => {
            local_client = config.pipeline.components.source.client(
                key.ok_or_else(|| ApplicationError::Setup {
                    issues: vec![SetupIssue::Credential(wanikani::API_KEY)],
                })?,
                endpoint,
            )?;
            &mut local_client
        }
        SourceClient::Supplied(client) => client,
        SourceClient::Unavailable => {
            return Err(ApplicationError::Setup {
                issues: vec![SetupIssue::Inventory],
            });
        }
    };
    let guard = match store.begin_sync() {
        Ok(guard) => guard,
        Err(error)
            if store.is_locked(&error)
                && previous.as_ref().is_some_and(|data| {
                    usable_cache(data, &config.pipeline.knowledge_policy, now)
                }) =>
        {
            progress.warn(
                Step::Sync,
                "Another writer is refreshing WaniKani; using the previous usable cache.".into(),
            );
            progress.skip(Step::Sync, "Writer contention");
            progress.finish(Step::Sync, started);
            return Ok(previous);
        }
        Err(error) => return Err(error.into()),
    };
    let rechecked = load_cache(store)?;
    if rechecked
        .as_ref()
        .is_some_and(|data| can_reuse_cache(config, data, now))
    {
        progress.finish(Step::Sync, started);
        return Ok(rechecked);
    }
    let previous = rechecked.or(previous);
    match client.fetch().await {
        Ok(data) => {
            if access_expired(&data, now) {
                return Err(ApplicationError::AccessExpired);
            }
            let data = Arc::new(data);
            guard.replace(Arc::clone(&data))?;
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
                Step::Sync,
                format!("{error} Using the previous usable cache."),
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
