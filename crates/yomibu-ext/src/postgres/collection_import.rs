use super::{PostgresLearnerStorage, WriteConnection, failure_reason};
use std::{future::Future, pin::Pin, time::Instant};
use yomibu_core::ports::infrastructure::{
    StorageError,
    collection_import::{SharedCollection, SubjectQuery, validate_subject_lookup},
};

fn lookup_expired(deadline: Instant) -> Result<(), StorageError> {
    if Instant::now() >= deadline {
        Err(lookup_deadline())
    } else {
        Ok(())
    }
}
fn lookup_deadline() -> StorageError {
    StorageError("subject lookup deadline exceeded".into())
}
fn lookup_error(error: sqlx::Error) -> StorageError {
    StorageError(format!("subject lookup failed ({})", failure_reason(error)))
}

impl SubjectQuery for PostgresLearnerStorage {
    fn existing_subject_ids<'a>(
        &'a self,
        collection: &'a SharedCollection,
        ids: &'a [String],
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, StorageError>> + 'a>> {
        Box::pin(async move {
            lookup_expired(deadline)?;
            validate_subject_lookup(collection, ids)?;
            // The contract bounds raw bytes; this adapter also bounds array and
            // parameter encoding before SQLx allocates argument buffers.
            let encoded = collection.provider.len()
                + collection.key.len()
                + ids.iter().map(|id| id.len() + 4).sum::<usize>()
                + ROW_OVERHEAD;
            check_size(encoded).map_err(|_| {
                StorageError("subject lookup exceeds encoded argument limit".into())
            })?;
            lookup_expired(deadline)?;
            if ids.is_empty() {
                return Ok(vec![]);
            }
            let connection = tokio::time::timeout_at(deadline.into(), self.pool.acquire())
                .await
                .map_err(|_| lookup_deadline())?
                .map_err(lookup_error)?;
            let mut lease = WriteConnection {
                connection,
                confirmed: false,
            };
            lookup_expired(deadline)?;
            let mut rows = tokio::time::timeout_at(deadline.into(), sqlx::query_scalar::<_,String>(
                "SELECT s.external_id FROM subjects s JOIN collections c ON c.id=s.collection_id
                 WHERE c.provider=$1 AND c.key=$2 AND c.owner_learner_id IS NULL
                 AND s.external_id = ANY($3)")
                 .bind(&collection.provider).bind(&collection.key).bind(ids).fetch_all(&mut *lease.connection)).await
                 .map_err(|_|lookup_deadline())?.map_err(lookup_error)?;
            lease.confirmed = true;
            lookup_expired(deadline)?;
            rows.sort_unstable();
            lookup_expired(deadline)?;
            Ok(rows)
        })
    }
}

use super::transaction::{expired, finish, limits, until};
use chrono::{DateTime, Utc};
use sqlx::{Acquire, Postgres, QueryBuilder, Transaction};
use std::collections::HashMap;
use yomibu_core::{
    domain::{CollectionId, LearnerProgressSourceId, MaterialKind},
    ports::infrastructure::collection_import::{
        CompletionReceipt, ImportGroupWriter, ImportWriteError, PreparedProgress,
        PreparedSaveGroup, PreparedSubject, SaveReceipt, SourceRecord,
    },
};

const CHUNK_ROWS: usize = 250;
const CHUNK_BYTES: usize = 1024 * 1024;
const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;
// Conservative per-row allowance for typed IDs, timestamps, lengths and envelope
// keys. Payloads stay borrowed until SQLx encodes the current bounded chunk.
const ROW_OVERHEAD: usize = 512;

fn invalid(reason: &str) -> ImportWriteError {
    ImportWriteError::NotSaved(reason.into())
}
fn timestamp(value: DateTime<Utc>) -> Result<(), ImportWriteError> {
    let epoch = DateTime::from_timestamp(946_684_800, 0).expect("PostgreSQL epoch");
    // PostgreSQL's finite MIN_TIMESTAMP and its microsecond wire representation.
    if !value.timestamp_subsec_nanos().is_multiple_of(1000)
        || value.timestamp_subsec_nanos() >= 1_000_000_000
        || value
            .signed_duration_since(epoch)
            .num_microseconds()
            .is_none_or(|v| v < -211_813_488_000_000_000)
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
fn preflight(group: &PreparedSaveGroup) -> Result<(), ImportWriteError> {
    check_size(
        group.collection().provider.len()
            + group.collection().key.len()
            + group.collection().name.len()
            + ROW_OVERHEAD,
    )?;
    timestamp(group.account().updated_at())?;
    check_size(encoded_size(
        group.account(),
        group.progress_source().external_account_id.len(),
    ))?;
    // Resolution uses one text array containing the supplied/referenced IDs.
    // Bound its encoding too, conservatively counting IDs repeated across lists.
    let mut resolution_bytes = ROW_OVERHEAD;
    for subject in group.subjects() {
        timestamp(subject.source.updated_at())?;
        check_size(subject_size(subject))?;
        resolution_bytes += subject.external_id.len() + 4;
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
        resolution_bytes += progress.subject_external_id.len() + 4;
    }
    check_size(resolution_bytes)?;
    Ok(())
}

fn check_size(size: usize) -> Result<(), ImportWriteError> {
    if size > MAX_ENCODED_BYTES {
        return Err(invalid("record exceeds encoded argument limit"));
    }
    Ok(())
}

// A singleton may exceed the normal target, but preflight enforces the hard cap.
fn chunk_len<T>(rows: &[T], size: impl Fn(&T) -> usize) -> usize {
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
    count
}

impl ImportGroupWriter for PostgresLearnerStorage {
    fn save_group<'a>(
        &'a self,
        group: &'a PreparedSaveGroup,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<SaveReceipt, ImportWriteError>> + 'a>> {
        Box::pin(async move {
            expired(deadline)?;
            preflight(group)?;
            let connection = until(deadline, self.pool.acquire()).await?;
            let mut lease = WriteConnection {
                connection,
                confirmed: false,
            };
            let mut tx = until(deadline, lease.connection.begin()).await?;
            let result = persist(&mut tx, group, deadline).await;
            let result = finish(tx, result, deadline).await;
            lease.confirmed = result.is_ok();
            result
        })
    }
    fn record_completion(
        &self,
        collection: CollectionId,
        source: LearnerProgressSourceId,
        completed_at: DateTime<Utc>,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<CompletionReceipt, ImportWriteError>> + '_>> {
        Box::pin(async move {
            expired(deadline)?;
            timestamp(completed_at)?;
            let connection = until(deadline, self.pool.acquire()).await?;
            let mut lease = WriteConnection {
                connection,
                confirmed: false,
            };
            let mut tx = until(deadline, lease.connection.begin()).await?;
            let result = complete(&mut tx, collection, source, completed_at, deadline).await;
            let result = finish(tx, result, deadline).await;
            lease.confirmed = result.is_ok();
            result
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
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").execute(&mut **tx),
    )
    .await?;
    limits(tx, deadline).await?;
    until(deadline,sqlx::query("INSERT INTO collections(provider,key,name) VALUES ($1,$2,$3) ON CONFLICT(provider,key) WHERE owner_learner_id IS NULL DO NOTHING").bind(&group.collection().provider).bind(&group.collection().key).bind(&group.collection().name).execute(&mut **tx)).await?;
    // Separate READ COMMITTED statement sees a concurrent conflict winner.
    limits(tx, deadline).await?;
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
    limits(tx, deadline).await?;
    until(deadline,sqlx::query("INSERT INTO learner_progress_sources AS saved(learner_id,collection_id,external_account_id,source_data)
        VALUES ($1,$2,$3,jsonb_build_object('source_updated_at',$4::timestamptz,'payload',$5::jsonb))
        ON CONFLICT(learner_id,collection_id,external_account_id) DO UPDATE SET source_data=excluded.source_data
        WHERE (excluded.source_data->>'source_updated_at')::timestamptz > COALESCE(saved.source_data->>'source_updated_at','')::timestamptz")
        .bind(group.progress_source().learner_id.0).bind(collection).bind(&group.progress_source().external_account_id).bind(group.account().updated_at()).bind(group.account().payload().get()).execute(&mut **tx)).await?;
    limits(tx, deadline).await?;
    let source:uuid::Uuid=until(deadline,sqlx::query_scalar("SELECT id FROM learner_progress_sources WHERE learner_id=$1 AND collection_id=$2 AND external_account_id=$3").bind(group.progress_source().learner_id.0).bind(collection).bind(&group.progress_source().external_account_id).fetch_one(&mut **tx)).await?;

    let mut subjects: Vec<_> = group.subjects().iter().collect();
    subjects.sort_unstable_by(|a, b| a.external_id.cmp(&b.external_id));
    let mut remaining = subjects.as_slice();
    while !remaining.is_empty() {
        let count = chunk_len(remaining, |s| subject_size(s));
        let (chunk, tail) = remaining.split_at(count);
        remaining = tail;
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
        sql.push(" ON CONFLICT(collection_id,external_id) DO UPDATE SET kind=excluded.kind,source_updated_at=excluded.source_updated_at,source_data=excluded.source_data WHERE saved.source_updated_at IS NULL OR excluded.source_updated_at>saved.source_updated_at RETURNING old.id IS NOT NULL AND old.source_updated_at IS NULL");
        limits(tx, deadline).await?;
        let invalid_old: Vec<bool> =
            until(deadline, sql.build_query_scalar().fetch_all(&mut **tx)).await?;
        if invalid_old.contains(&true) {
            return Err(invalid("stored subject freshness is missing"));
        }
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
    limits(tx, deadline).await?;
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
    let mut remaining = progress.as_slice();
    while !remaining.is_empty() {
        let count = chunk_len(remaining, |(_, p)| progress_size(p));
        let (chunk, tail) = remaining.split_at(count);
        remaining = tail;
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
        sql.push(" ON CONFLICT(learner_progress_source_id,subject_id) DO UPDATE SET ");
        const NEWER: &str = "(excluded.source_data->>'source_updated_at')::timestamptz>COALESCE(saved.source_data->>'source_updated_at','')::timestamptz";
        for (index, column) in [
            "progress_stage",
            "is_hidden",
            "learning_started_at",
            "first_passed_at",
            "source_data",
        ]
        .into_iter()
        .enumerate()
        {
            if index > 0 {
                sql.push(",");
            }
            sql.push(column)
                .push("=CASE WHEN ")
                .push(NEWER)
                .push(" THEN excluded.")
                .push(column)
                .push(" ELSE saved.")
                .push(column)
                .push(" END");
        }
        sql.push(", recorded_at=GREATEST(saved.recorded_at,excluded.recorded_at) WHERE ")
            .push(NEWER)
            .push(" OR excluded.recorded_at>saved.recorded_at");
        limits(tx, deadline).await?;
        until(deadline, sql.build().execute(&mut **tx)).await?;
    }
    Ok(SaveReceipt {
        collection_id: CollectionId(collection),
        learner_progress_source_id: LearnerProgressSourceId(source),
    })
}

async fn complete(
    tx: &mut Transaction<'_, Postgres>,
    collection: CollectionId,
    source: LearnerProgressSourceId,
    completed_at: DateTime<Utc>,
    deadline: Instant,
) -> Result<CompletionReceipt, ImportWriteError> {
    limits(tx, deadline).await?;
    let stored=until(deadline,sqlx::query_scalar("UPDATE learner_progress_sources AS s SET last_import_completed_at=GREATEST(s.last_import_completed_at,$3) FROM collections c WHERE s.id=$1 AND s.collection_id=$2 AND c.id=s.collection_id AND c.owner_learner_id IS NULL AND s.external_account_id IS NOT NULL RETURNING s.last_import_completed_at").bind(source.0).bind(collection.0).bind(completed_at).fetch_optional(&mut **tx)).await?
        .ok_or_else(|| invalid("external completion source does not exist in supplied shared collection"))?;
    Ok(CompletionReceipt {
        learner_progress_source_id: source,
        last_import_completed_at: stored,
    })
}
