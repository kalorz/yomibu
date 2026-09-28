//! WaniKani v2 retrieval. Transport shapes and credentials stay inside this adapter.
mod dto;

use crate::domain::{Snapshot, Subject, UnavailableSubject, ValidationError};
use chrono::{DateTime, Utc};
use reqwest::{
    Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::time::{Duration, SystemTime};
use thiserror::Error;

const MAX_PAGE_BYTES: usize = 16 * 1024 * 1024;

/// Sanitized retrieval failures; transport URLs, response bodies, and credentials
/// are deliberately omitted from both messages and underlying error chains.
#[derive(Debug, Error)]
pub enum Error {
    #[error("WaniKani rate-limit reset exceeds 120 seconds; retry sync later.")]
    RateLimitWait,
    #[error("Conflicting duplicate {collection} identity {id} in WaniKani response.")]
    ConflictingDuplicate { collection: &'static str, id: u64 },
    #[error("Unsafe or repeated pagination URL for WaniKani {endpoint}.")]
    Pagination { endpoint: &'static str },
    #[error("Invalid HTTP client configuration or WANIKANI_API_TOKEN.")]
    Configuration,
    #[error("Authentication failed; check WANIKANI_API_TOKEN and its read permissions.")]
    Authentication,
    #[error("HTTP {status} from WaniKani {endpoint}.")]
    Http { endpoint: &'static str, status: u16 },
    #[error("Transport failure or deadline exceeded for WaniKani {endpoint}.")]
    Transport { endpoint: &'static str },
    #[error("Invalid response from WaniKani {endpoint}.")]
    InvalidResponse { endpoint: &'static str },
    #[error("WaniKani {endpoint} response exceeds 16 MiB.")]
    ResponseTooLarge { endpoint: &'static str },
    #[error("Invalid WaniKani snapshot: {0}")]
    InvalidSnapshot(#[from] ValidationError),
}

/// Reusable authenticated client with sequential requests and bounded retries.
/// Owns its authorization header and pending rate-limit deadline. It deliberately
/// does not implement `Debug` or serialization, keeping credentials private.
pub struct Client {
    http: reqwest::Client,
    base_url: Url,
    authorization: HeaderValue,
    next_request_at: Option<tokio::time::Instant>,
}

impl Client {
    /// Configure the official WaniKani HTTPS origin without making a request.
    /// The token is supplied by the caller, never read from the environment.
    pub fn new(token: &str) -> Result<Self, Error> {
        Self::with_base_url(token, "https://api.wanikani.com/v2/")
    }

    /// Use a trusted API base URL ending in `/`. Plain HTTP is permitted only
    /// for loopback IP addresses, so callers can exercise the real adapter locally.
    /// The supplied origin receives the token; callers must trust its operator.
    /// The CLI always uses the default WaniKani HTTPS origin.
    pub fn with_base_url(token: &str, base_url: &str) -> Result<Self, Error> {
        Self::with_timeout(token, base_url, Duration::from_secs(30))
    }

    fn with_timeout(token: &str, base_url: &str, request_timeout: Duration) -> Result<Self, Error> {
        if token.trim().is_empty() {
            return Err(Error::Configuration);
        }
        let mut authorization =
            HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| Error::Configuration)?;
        authorization.set_sensitive(true);
        let base_url = Url::parse(base_url).map_err(|_| Error::Configuration)?;
        let loopback = base_url
            .host_str()
            .and_then(|host| {
                host.trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .ok()
            })
            .is_some_and(|ip| ip.is_loopback());
        if (base_url.scheme() != "https" && !(base_url.scheme() == "http" && loopback))
            || base_url.host_str().is_none()
            || !base_url.path().ends_with('/')
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
        {
            return Err(Error::Configuration);
        }

        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(request_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Configuration)?;
        Ok(Self {
            http,
            base_url,
            authorization,
            next_request_at: None,
        })
    }

    /// Fetch and validate a complete refresh without reading or writing a cache.
    /// Requires a Tokio runtime with I/O and time enabled. Requests have 10-second
    /// connect and 30-second total deadlines; the entire refresh has no fixed
    /// deadline. Reuse this client to retain rate-limit state, including after
    /// cancellation. Observations span the returned synchronization interval.
    pub async fn fetch(&mut self) -> Result<Snapshot, Error> {
        let sync_started_at = now();
        let user: dto::User = self.get("user").await?;
        if user.object != "user" {
            return Err(Error::InvalidResponse { endpoint: "user" });
        }
        let assignments = self
            .collection::<dto::Resource<dto::AssignmentData>>("assignments", None)
            .await?
            .into_iter()
            .filter_map(|r| {
                if r.object != "assignment" {
                    return Some(Err(Error::InvalidResponse {
                        endpoint: "assignments",
                    }));
                }
                r.normalize().map(Ok)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let review_statistics = self
            .collection::<dto::Resource<dto::ReviewStatisticData>>("review_statistics", None)
            .await?
            .into_iter()
            .filter_map(|r| {
                if r.object != "review_statistic" {
                    return Some(Err(Error::InvalidResponse {
                        endpoint: "review_statistics",
                    }));
                }
                r.normalize().map(Ok)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let assignments = deduplicate(assignments, "assignments", |a| a.id)?;
        let review_statistics = deduplicate(review_statistics, "review statistics", |r| r.id)?;
        let ids: BTreeSet<_> = assignments
            .iter()
            .map(|a| a.subject_id)
            .chain(review_statistics.iter().map(|r| r.subject_id))
            .collect();
        let learner: crate::domain::Learner = user.into();
        let mut subjects = Vec::new();

        let ids: Vec<_> = ids.into_iter().collect();
        for batch in ids.chunks(100) {
            let ids = batch
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            for resource in self
                .collection::<dto::SubjectResource>("subjects", Some(&ids))
                .await?
            {
                let subject: Subject = resource.into();
                if batch.binary_search(&subject.id).is_err() {
                    return Err(Error::InvalidResponse {
                        endpoint: "subjects",
                    });
                }
                subject.validate(60)?;
                subjects.push(subject);
            }
        }
        let subjects = deduplicate(subjects, "subjects", |s| s.id)?;
        let (subjects, excluded): (Vec<_>, Vec<_>) = subjects
            .into_iter()
            .partition(|s| s.level <= learner.subscription.max_level_granted);
        let unavailable_subjects = excluded
            .into_iter()
            .map(|s| UnavailableSubject {
                id: s.id,
                kind: s.kind(),
            })
            .collect();
        let snapshot = Snapshot {
            sync_started_at,
            sync_completed_at: now(),
            learner,
            subjects,
            unavailable_subjects,
            assignments,
            review_statistics,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    async fn get<T: DeserializeOwned>(&mut self, endpoint: &'static str) -> Result<T, Error> {
        let url = self
            .base_url
            .join(endpoint)
            .map_err(|_| Error::Configuration)?;
        self.get_url(endpoint, url).await
    }

    async fn collection<T: DeserializeOwned>(
        &mut self,
        endpoint: &'static str,
        ids: Option<&str>,
    ) -> Result<Vec<T>, Error> {
        let mut url = self
            .base_url
            .join(endpoint)
            .map_err(|_| Error::Configuration)?;
        if let Some(ids) = ids {
            url.query_pairs_mut().append_pair("ids", ids);
        }
        let mut records = Vec::new();
        let mut visited = BTreeSet::new();
        loop {
            if url.origin() != self.base_url.origin()
                || url.path() != format!("{}{endpoint}", self.base_url.path())
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
                || !visited.insert(url.as_str().to_owned())
            {
                return Err(Error::Pagination { endpoint });
            }
            let page: dto::Collection<T> = self.get_url(endpoint, url).await?;
            if page.object != "collection" {
                return Err(Error::InvalidResponse { endpoint });
            }
            records.extend(page.data);
            match page.pages.next_url {
                None => return Ok(records),
                Some(next) => {
                    url = Url::parse(&next).map_err(|_| Error::Pagination { endpoint })?
                }
            }
        }
    }

    async fn get_url<T: DeserializeOwned>(
        &mut self,
        endpoint: &'static str,
        url: Url,
    ) -> Result<T, Error> {
        let mut attempt = 0;
        loop {
            if let Some(ready) = self.next_request_at {
                // Cancellation must not let a reused client bypass the remaining wait.
                tokio::time::sleep_until(ready).await;
                self.next_request_at = None;
            }
            match self.request(endpoint, url.clone()).await {
                Ok(bytes) => {
                    return serde_json::from_slice(&bytes)
                        .map_err(|_| Error::InvalidResponse { endpoint });
                }
                Err(error) => {
                    if attempt == 2 {
                        return Err(error);
                    }
                    match error {
                        Error::Http { status: 429, .. } => {}
                        Error::Http {
                            status: 500..=599, ..
                        }
                        | Error::Transport { .. } => {
                            tokio::time::sleep(transient_delay(attempt)).await;
                        }
                        _ => return Err(error),
                    }
                }
            }
            attempt += 1;
        }
    }

    async fn request(&mut self, endpoint: &'static str, url: Url) -> Result<Vec<u8>, Error> {
        let mut response = self
            .http
            .get(url)
            .header(AUTHORIZATION, &self.authorization)
            .header("Wanikani-Revision", "20170710")
            .send()
            .await
            .map_err(|_| Error::Transport { endpoint })?;
        let status = response.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(Error::Authentication);
        }
        if status.as_u16() == 429
            || response
                .headers()
                .get("ratelimit-remaining")
                .is_some_and(|remaining| remaining == "0")
        {
            self.next_request_at =
                Some(tokio::time::Instant::now() + reset_delay(response.headers(), now())?);
        }
        if !status.is_success() {
            return Err(Error::Http {
                endpoint,
                status: status.as_u16(),
            });
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_PAGE_BYTES as u64)
        {
            return Err(Error::ResponseTooLarge { endpoint });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| Error::Transport { endpoint })?
        {
            if chunk.len() > MAX_PAGE_BYTES - bytes.len() {
                return Err(Error::ResponseTooLarge { endpoint });
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

fn transient_delay(attempt: u32) -> Duration {
    Duration::from_secs(u64::from(attempt) + 1)
}

fn reset_delay(headers: &HeaderMap, now: DateTime<Utc>) -> Result<Duration, Error> {
    let seconds = headers
        .get("ratelimit-reset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|reset| reset.saturating_sub(now.timestamp().max(0) as u64))
        .unwrap_or(60);
    if seconds > 120 {
        return Err(Error::RateLimitWait);
    }
    Ok(Duration::from_secs(seconds))
}

fn deduplicate<T: PartialEq>(
    records: Vec<T>,
    collection: &'static str,
    id: fn(&T) -> u64,
) -> Result<Vec<T>, Error> {
    let mut unique = BTreeMap::new();
    for record in records {
        let id = id(&record);
        match unique.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(record);
            }
            Entry::Occupied(entry) if entry.get() == &record => {}
            Entry::Occupied(_) => return Err(Error::ConflictingDuplicate { collection, id }),
        }
    }
    Ok(unique.into_values().collect())
}

fn now() -> DateTime<Utc> {
    SystemTime::now().into()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod resilience;
