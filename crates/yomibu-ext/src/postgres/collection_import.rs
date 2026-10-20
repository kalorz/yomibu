use super::{
    PostgresLearnerStorage, WriteConnection, bounded, expired, failure_reason,
    transaction::{deadline_error, transact, until},
};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, QueryBuilder, Transaction};
use std::{collections::HashMap, future::Future, pin::Pin, time::Instant};
use yomibu_core::{
    domain::{CollectionId, LearnerProgressSourceId, MaterialKind},
    ports::infrastructure::storage::{
        ImportWriteError, LearnerProgressImportWriter, PreparedProgress, PreparedSaveGroup,
        PreparedSubject, SaveReceipt, SourceRecord, StorageError, SubjectQuery,
        validate_subject_lookup,
    },
};

const CHUNK_ROWS: usize = 250;
const CHUNK_BYTES: usize = 1024 * 1024;
const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;
// Conservative per-row allowance for typed IDs, timestamps, lengths and envelope
// keys. Payloads stay borrowed until SQLx encodes the current bounded chunk.
const ROW_OVERHEAD: usize = 512;
// PostgreSQL's finite MIN_TIMESTAMP (4714-11-24 BC) as microseconds since the
// Unix epoch. Chrono's range ends before PostgreSQL's maximum.
const PG_MIN_UNIX_MICROS: i64 = 946_684_800_000_000 - 211_813_488_000_000_000;

// Stored freshness for account and progress records lives inside the JSONB
// envelope. A missing or malformed key fails the cast and the transaction.
macro_rules! newer {
    () => {
        "(excluded.source_data->>'source_updated_at')::timestamptz>COALESCE(saved.source_data->>'source_updated_at','')::timestamptz"
    };
}
const SOURCE_UPSERT: &str = concat!(
    "INSERT INTO learner_progress_sources AS saved
        (learner_id,collection_id,external_account_id,source_data)
     VALUES ($1,$2,$3,jsonb_build_object('source_updated_at',$4::timestamptz,'payload',$5::jsonb))
     ON CONFLICT(learner_id,collection_id,external_account_id)
     DO UPDATE SET source_data=excluded.source_data WHERE ",
    newer!()
);
// A source field takes the supplied value only when the supplied source is newer.
macro_rules! keep_newer {
    ($column:literal) => {
        concat!(
            $column,
            "=CASE WHEN ",
            newer!(),
            " THEN excluded.",
            $column,
            " ELSE saved.",
            $column,
            " END,"
        )
    };
}
// The observation time advances independently of source freshness.
const PROGRESS_UPSERT: &str = concat!(
    " ON CONFLICT(learner_progress_source_id,subject_id) DO UPDATE SET ",
    keep_newer!("progress_stage"),
    keep_newer!("is_hidden"),
    keep_newer!("learning_started_at"),
    keep_newer!("first_passed_at"),
    keep_newer!("source_data"),
    "recorded_at=GREATEST(saved.recorded_at,excluded.recorded_at) WHERE ",
    newer!(),
    " OR excluded.recorded_at>saved.recorded_at"
);

fn lookup_deadline() -> StorageError {
    StorageError("subject lookup deadline exceeded".into())
}
fn lookup_failure(error: Option<sqlx::Error>) -> StorageError {
    error.map_or_else(lookup_deadline, |error| {
        StorageError(format!("subject lookup failed ({})", failure_reason(error)))
    })
}

impl SubjectQuery for PostgresLearnerStorage {
    fn existing_subject_ids<'a>(
        &'a self,
        provider: &'a str,
        key: &'a str,
        ids: &'a [String],
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, StorageError>> + 'a>> {
        Box::pin(async move {
            expired(deadline, lookup_deadline)?;
            validate_subject_lookup(provider, key, ids)?;
            // The contract bounds raw bytes; this adapter also bounds array and
            // parameter encoding before SQLx allocates argument buffers.
            if provider.len() + key.len() + array_bytes(ids) > MAX_ENCODED_BYTES {
                return Err(StorageError(
                    "subject lookup exceeds encoded argument limit".into(),
                ));
            }
            if ids.is_empty() {
                return Ok(vec![]);
            }
            let connection = bounded(deadline, self.pool.acquire(), lookup_failure).await?;
            let mut lease = WriteConnection {
                connection,
                confirmed: false,
            };
            let query = sqlx::query_scalar::<_, String>(
                "SELECT s.external_id FROM subjects s JOIN collections c ON c.id=s.collection_id
                 WHERE c.provider=$1 AND c.key=$2 AND c.owner_learner_id IS NULL
                 AND s.external_id = ANY($3)",
            )
            .bind(provider)
            .bind(key)
            .bind(ids)
            .fetch_all(&mut *lease.connection);
            let rows = bounded(deadline, query, lookup_failure).await?;
            lease.confirmed = true;
            expired(deadline, lookup_deadline)?;
            finish_lookup(rows, deadline)
        })
    }
}

// Sorting is part of the read operation and can exhaust its remaining budget.
fn finish_lookup(mut rows: Vec<String>, deadline: Instant) -> Result<Vec<String>, StorageError> {
    rows.sort_unstable();
    expired(deadline, lookup_deadline)?;
    Ok(rows)
}

fn invalid(reason: &str) -> ImportWriteError {
    ImportWriteError::NotSaved(reason.into())
}
fn timestamp(value: DateTime<Utc>) -> Result<(), ImportWriteError> {
    // Leap-second and sub-microsecond values have no lossless microsecond wire form.
    let nanos = value.timestamp_subsec_nanos();
    if !nanos.is_multiple_of(1000)
        || nanos >= 1_000_000_000
        || value.timestamp_micros() < PG_MIN_UNIX_MICROS
    {
        return Err(invalid(
            "timestamp is not losslessly representable by PostgreSQL",
        ));
    }
    Ok(())
}
fn encoded_size(source: &SourceRecord, extra: usize) -> usize {
    source.payload().get().len() + extra + ROW_OVERHEAD
}
fn subject_size(subject: &PreparedSubject) -> usize {
    encoded_size(&subject.source, subject.external_id.len())
}
fn progress_size(progress: &PreparedProgress) -> usize {
    encoded_size(
        &progress.source,
        progress.progress_stage.as_ref().map_or(0, String::len),
    )
}
/// Encoded size of one text-array parameter holding the given IDs.
fn array_bytes<'a>(ids: impl IntoIterator<Item = &'a String>) -> usize {
    ids.into_iter().map(|id| id.len() + 4).sum::<usize>() + ROW_OVERHEAD
}
fn preflight(group: &PreparedSaveGroup) -> Result<(), ImportWriteError> {
    check_size(group.collection().input_bytes() + ROW_OVERHEAD)?;
    timestamp(group.account().updated_at())?;
    check_size(encoded_size(
        group.account(),
        group.learner_progress_source().external_account_id.len(),
    ))?;
    for subject in group.subjects() {
        timestamp(subject.source.updated_at())?;
        check_size(subject_size(subject))?;
    }
    for progress in group.progress() {
        for value in [
            Some(progress.source.updated_at()),
            Some(progress.observed_at),
            progress.learning_started_at,
            progress.first_passed_at,
        ]
        .into_iter()
        .flatten()
        {
            timestamp(value)?;
        }
        check_size(progress_size(progress))?;
    }
    // Resolution uses one text array containing the supplied/referenced IDs.
    // Bound its encoding too, conservatively counting IDs repeated across lists.
    check_size(array_bytes(
        group
            .subjects()
            .iter()
            .map(|s| &s.external_id)
            .chain(group.progress().iter().map(|p| &p.subject_external_id)),
    ))?;
    Ok(())
}

fn check_size(size: usize) -> Result<(), ImportWriteError> {
    if size > MAX_ENCODED_BYTES {
        return Err(invalid("record exceeds encoded argument limit"));
    }
    Ok(())
}

// Statement-sized chunks. A singleton may exceed the normal byte target, but
// preflight enforces the hard cap.
fn chunks<'a, T>(
    mut rows: &'a [T],
    size: impl Fn(&T) -> usize + 'a,
) -> impl Iterator<Item = &'a [T]> + 'a {
    std::iter::from_fn(move || {
        if rows.is_empty() {
            return None;
        }
        let mut bytes = 0;
        let mut count = 0;
        for row in rows.iter().take(CHUNK_ROWS) {
            let next = size(row);
            if count > 0 && bytes + next > CHUNK_BYTES {
                break;
            }
            count += 1;
            bytes += next;
        }
        let (chunk, tail) = rows.split_at(count);
        rows = tail;
        Some(chunk)
    })
}

/// Bounds encoded arguments and rejects lossy timestamps before acquiring a connection.
/// Other PostgreSQL representation failures roll back the whole group.
impl LearnerProgressImportWriter for PostgresLearnerStorage {
    fn save_group<'a>(
        &'a self,
        group: &'a PreparedSaveGroup,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<SaveReceipt, ImportWriteError>> + 'a>> {
        Box::pin(async move {
            expired(deadline, deadline_error)?;
            preflight(group)?;
            transact(&self.pool, deadline, async |tx| {
                persist(tx, group, deadline).await
            })
            .await
        })
    }
}

async fn persist(
    tx: &mut Transaction<'_, Postgres>,
    group: &PreparedSaveGroup,
    deadline: Instant,
) -> Result<SaveReceipt, ImportWriteError> {
    until(
        deadline,
        sqlx::query(
            "INSERT INTO collections(provider,key,name) VALUES ($1,$2,$3)
             ON CONFLICT(provider,key) WHERE owner_learner_id IS NULL DO NOTHING",
        )
        .bind(&group.collection().provider)
        .bind(&group.collection().key)
        .bind(&group.collection().name)
        .execute(&mut **tx),
    )
    .await?;
    // Separate READ COMMITTED statement sees a concurrent conflict winner.
    let collection: uuid::Uuid = until(
        deadline,
        sqlx::query_scalar(
            "SELECT id FROM collections WHERE provider=$1 AND key=$2 AND owner_learner_id IS NULL",
        )
        .bind(&group.collection().provider)
        .bind(&group.collection().key)
        .fetch_one(&mut **tx),
    )
    .await?;
    until(
        deadline,
        sqlx::query(SOURCE_UPSERT)
            .bind(group.learner_progress_source().learner_id.0)
            .bind(collection)
            .bind(&group.learner_progress_source().external_account_id)
            .bind(group.account().updated_at())
            .bind(group.account().payload().get())
            .execute(&mut **tx),
    )
    .await?;
    let source: uuid::Uuid = until(
        deadline,
        sqlx::query_scalar(
            "SELECT id FROM learner_progress_sources
             WHERE learner_id=$1 AND collection_id=$2 AND external_account_id=$3",
        )
        .bind(group.learner_progress_source().learner_id.0)
        .bind(collection)
        .bind(&group.learner_progress_source().external_account_id)
        .fetch_one(&mut **tx),
    )
    .await?;
    let receipt = SaveReceipt {
        collection_id: CollectionId(collection),
        learner_progress_source_id: LearnerProgressSourceId(source),
    };
    if group.subjects().is_empty() && group.progress().is_empty() {
        return Ok(receipt);
    }

    let mut subjects: Vec<_> = group.subjects().iter().collect();
    subjects.sort_unstable_by(|a, b| a.external_id.cmp(&b.external_id));
    for chunk in chunks(&subjects, |s| subject_size(s)) {
        let mut sql = QueryBuilder::<Postgres>::new(
            "INSERT INTO subjects AS saved(collection_id,external_id,kind,source_updated_at,source_data) ",
        );
        sql.push_values(chunk, |mut row, s| {
            row.push_bind(collection)
                .push_bind(&s.external_id)
                .push_bind(s.kind.map(|kind| match kind {
                    MaterialKind::Vocabulary => "vocabulary",
                    MaterialKind::Grammar => "grammar",
                }))
                .push_unseparated("::material_kind")
                .push_bind(s.source.updated_at())
                .push_bind(s.source.payload().get())
                .push_unseparated("::jsonb");
        });
        // Timestamped input initializes unavailable subject freshness. Once known,
        // the same atomic predicate permits only strictly newer source data.
        sql.push(
            " ON CONFLICT(collection_id,external_id) DO UPDATE
              SET kind=excluded.kind,source_updated_at=excluded.source_updated_at,
                  source_data=excluded.source_data
              WHERE saved.source_updated_at IS NULL
                  OR excluded.source_updated_at>saved.source_updated_at",
        );
        until(deadline, sql.build().execute(&mut **tx)).await?;
    }
    // Both buffers are bounded by the constructor's two record counts. No payloads
    // are fetched; concurrent deletion remains guarded by the composite FKs.
    let mut ids: Vec<_> = group
        .subjects()
        .iter()
        .map(|s| &s.external_id)
        .chain(group.progress().iter().map(|p| &p.subject_external_id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let resolved: Vec<(String, uuid::Uuid)> = until(
        deadline,
        sqlx::query_as(
            "SELECT external_id,id FROM subjects WHERE collection_id=$1 AND external_id=ANY($2)",
        )
        .bind(collection)
        .bind(&ids)
        .fetch_all(&mut **tx),
    )
    .await?;
    if resolved.len() != ids.len() {
        return Err(invalid(
            "progress references a missing subject in this collection",
        ));
    }
    let resolved: HashMap<_, _> = resolved.into_iter().collect();
    let mut progress: Vec<_> = group
        .progress()
        .iter()
        .map(|p| (resolved[&p.subject_external_id], p))
        .collect();
    progress.sort_unstable_by_key(|(id, _)| *id);
    for chunk in chunks(&progress, |(_, p)| progress_size(p)) {
        let mut sql = QueryBuilder::<Postgres>::new(
            "INSERT INTO learner_progress AS saved(learner_progress_source_id,subject_id,collection_id,progress_stage,is_hidden,learning_started_at,first_passed_at,recorded_at,source_data) ",
        );
        sql.push_values(chunk, |mut row, (id, p)| {
            row.push_bind(source)
                .push_bind(*id)
                .push_bind(collection)
                .push_bind(p.progress_stage.as_deref())
                .push_bind(p.is_hidden)
                .push_bind(p.learning_started_at)
                .push_bind(p.first_passed_at)
                .push_bind(p.observed_at);
            row.push("jsonb_build_object('source_updated_at',")
                .push_bind_unseparated(p.source.updated_at())
                .push_unseparated("::timestamptz,'payload',")
                .push_bind_unseparated(p.source.payload().get())
                .push_unseparated("::jsonb)");
        });
        sql.push(PROGRESS_UPSERT);
        until(deadline, sql.build().execute(&mut **tx)).await?;
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests;
