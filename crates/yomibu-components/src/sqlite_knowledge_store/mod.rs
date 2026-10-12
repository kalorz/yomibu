//! Synchronous SQLite persistence for explicitly scoped mapped observations.
//! Timestamps use chrono's canonical UTC text, including extended years.
#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Statement, Transaction, TransactionBehavior, params,
};
use std::{
    collections::BTreeSet,
    fs::{OpenOptions, Permissions},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    time::Duration,
};
use yomibu_core::domain::{
    inventory::{InventoryError, InventoryWord, MappedContent, SourceOrigin},
    knowledge::LearnerProgress,
};

const APPLICATION_ID: i64 = 0x594f4d49;
const SCHEMA_VERSION: i64 = 1;

enum SchemaState {
    Empty,
    Supported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Manual,
    Wanikani,
}

impl Provider {
    fn tag(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Wanikani => "wanikani",
        }
    }

    fn decode(value: &str) -> Result<Self, SqliteKnowledgeError> {
        match value {
            "manual" => Ok(Self::Manual),
            "wanikani" => Ok(Self::Wanikani),
            _ => Err(SqliteKnowledgeError::Corrupt("unknown provider")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeScope {
    pub learner_id: String,
    pub source_collection_id: String,
    pub source_learner_profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeMetadata {
    pub provider: Provider,
    pub external_account_id: Option<String>,
    pub access_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct MappedKnowledgeRecord {
    pub external_id: String,
    pub content: MappedContent,
    pub progress: Option<LearnerProgress>,
}

#[derive(Debug)]
pub struct StoredKnowledge {
    pub scope: KnowledgeScope,
    pub metadata: ScopeMetadata,
    pub records: Vec<MappedKnowledgeRecord>,
}

#[derive(Debug, thiserror::Error)]
pub enum SqliteKnowledgeError {
    #[error("Knowledge database does not exist.")]
    MissingDatabase,
    #[error("Knowledge profile does not exist.")]
    MissingScope,
    #[error("Unsupported or uninitialized knowledge database.")]
    UnsupportedSchema,
    #[error("Knowledge database requires DELETE journaling; initialize it explicitly.")]
    UnsupportedJournalMode,
    #[error("Knowledge scope binding disagrees with the stored identity.")]
    BindingMismatch,
    #[error("Invalid knowledge input: {0}.")]
    InvalidInput(&'static str),
    #[error("Corrupt stored knowledge: {0}.")]
    Corrupt(&'static str),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error("Cannot access knowledge database: {0}")]
    Io(#[from] std::io::Error),
    #[error("Knowledge database operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("SQLite commit failed; inspect the database before retrying: {0}")]
    Commit(#[source] rusqlite::Error),
}

/// Operations block the caller; each SQLite lock wait is limited to five seconds.
/// Connections and transactions end before returning owned values.
/// Initialization sets DELETE journaling; ordinary operations require it.
/// Writes request EXTRA synchronization and fullfsync.
/// Durability depends on the filesystem and hardware honoring those requests.
#[derive(Debug, Clone)]
pub struct SqliteKnowledgeStore {
    path: PathBuf,
}

impl SqliteKnowledgeStore {
    /// Configure a database path without I/O.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Create schema 1 in an empty database, or verify its application/version IDs.
    pub fn initialize(&self) -> Result<(), SqliteKnowledgeError> {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.path)
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let mut connection = self.open(OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        if matches!(schema_state(&connection)?, SchemaState::Empty) {
            std::fs::set_permissions(&self.path, Permissions::from_mode(0o600))?;
        }
        connection.pragma_update(None, "journal_mode", "DELETE")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = schema_state(&transaction)?;
        require_delete_journal(&transaction)?;
        if matches!(state, SchemaState::Empty) {
            transaction.execute_batch(include_str!("001_initial.sql"))?;
            transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        commit_transaction(transaction)
    }

    /// Atomically publish mapped facts for one profile. Errors before commit roll back;
    /// commit errors require inspection before retry.
    pub fn publish(
        &self,
        scope: &KnowledgeScope,
        metadata: &ScopeMetadata,
        records: &[MappedKnowledgeRecord],
    ) -> Result<(), SqliteKnowledgeError> {
        validate_scope(scope)?;
        if metadata
            .external_account_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
            return Err(SqliteKnowledgeError::InvalidInput("blank external account"));
        }
        let mut external_ids = BTreeSet::new();
        let mut word_ids = BTreeSet::new();
        for record in records {
            if record.external_id.trim().is_empty() || !external_ids.insert(&record.external_id) {
                return Err(SqliteKnowledgeError::InvalidInput(
                    "invalid or duplicate external identity",
                ));
            }
            let origin = match &record.content {
                MappedContent::Vocabulary(word) => {
                    word.validate()?;
                    if !word_ids.insert(&word.id) {
                        return Err(SqliteKnowledgeError::InvalidInput(
                            "duplicate vocabulary identity",
                        ));
                    }
                    &word.origin
                }
                MappedContent::NonVocabulary(origin) | MappedContent::Unavailable(origin) => origin,
            };
            if source_origin(metadata.provider, &record.external_id).as_ref() != Some(origin) {
                return Err(SqliteKnowledgeError::InvalidInput(
                    "source provenance disagrees with external identity",
                ));
            }
        }
        let mut connection = self.open(OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_schema(&transaction)?;
        require_delete_journal(&transaction)?;
        publish_scope(&transaction, scope, metadata, records)?;
        commit_transaction(transaction)
    }

    /// Read a coherent scope without database writes, creation, migration, or access policy.
    /// SQLite may create WAL side files before rejecting externally changed journaling.
    /// A hot rollback journal may require explicit writable initialization first.
    pub fn read(&self, scope: &KnowledgeScope) -> Result<StoredKnowledge, SqliteKnowledgeError> {
        validate_scope(scope)?;
        let mut connection = self.open(OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let transaction = connection.transaction()?;
        require_schema(&transaction)?;
        require_delete_journal(&transaction)?;
        read_scope(&transaction, scope).map_err(|error| match error {
            SqliteKnowledgeError::Database(
                rusqlite::Error::InvalidColumnType(..)
                | rusqlite::Error::FromSqlConversionFailure(..)
                | rusqlite::Error::IntegralValueOutOfRange(..)
                | rusqlite::Error::Utf8Error(..),
            ) => SqliteKnowledgeError::Corrupt("invalid stored value type"),
            error => error,
        })
    }

    fn open(&self, flags: OpenFlags) -> Result<Connection, SqliteKnowledgeError> {
        match std::fs::metadata(&self.path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(SqliteKnowledgeError::MissingDatabase);
            }
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let connection = Connection::open_with_flags(std::path::absolute(&self.path)?, flags)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        if connection.pragma_query_value(None, "foreign_keys", |row| row.get::<_, i64>(0))? != 1 {
            return Err(SqliteKnowledgeError::Corrupt("foreign keys are disabled"));
        }
        if flags.contains(OpenFlags::SQLITE_OPEN_READ_WRITE) {
            connection.pragma_update(None, "synchronous", "EXTRA")?;
            connection.pragma_update(None, "fullfsync", true)?;
        }
        Ok(connection)
    }
}

fn schema_state(connection: &Connection) -> Result<SchemaState, SqliteKnowledgeError> {
    let application_id: i64 =
        connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if application_id == APPLICATION_ID && version == SCHEMA_VERSION {
        return Ok(SchemaState::Supported);
    }
    let empty: bool = connection.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM sqlite_schema)",
        [],
        |row| row.get(0),
    )?;
    if application_id == 0 && version == 0 && empty {
        Ok(SchemaState::Empty)
    } else {
        Err(SqliteKnowledgeError::UnsupportedSchema)
    }
}

fn commit_transaction(transaction: Transaction<'_>) -> Result<(), SqliteKnowledgeError> {
    if let Err(error) = transaction.execute_batch("COMMIT") {
        if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy) {
            transaction
                .rollback()
                .map_err(SqliteKnowledgeError::Commit)?;
            return Err(SqliteKnowledgeError::Database(error));
        }
        return Err(SqliteKnowledgeError::Commit(error));
    }
    Ok(())
}

fn require_schema(connection: &Connection) -> Result<(), SqliteKnowledgeError> {
    match schema_state(connection)? {
        SchemaState::Empty => Err(SqliteKnowledgeError::UnsupportedSchema),
        SchemaState::Supported => Ok(()),
    }
}

fn require_delete_journal(connection: &Connection) -> Result<(), SqliteKnowledgeError> {
    let mode: String = connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if mode != "delete" {
        return Err(SqliteKnowledgeError::UnsupportedJournalMode);
    }
    Ok(())
}

fn validate_scope(scope: &KnowledgeScope) -> Result<(), SqliteKnowledgeError> {
    if [
        &scope.learner_id,
        &scope.source_collection_id,
        &scope.source_learner_profile_id,
    ]
    .iter()
    .any(|id| id.trim().is_empty())
    {
        return Err(SqliteKnowledgeError::InvalidInput("blank scope identity"));
    }
    Ok(())
}

fn source_origin(kind: Provider, external_id: &str) -> Option<SourceOrigin> {
    match kind {
        Provider::Manual if !external_id.trim().is_empty() => Some(SourceOrigin::Manual),
        Provider::Wanikani => {
            let subject_id: u64 = external_id.parse().ok()?;
            (subject_id != 0 && subject_id.to_string() == external_id)
                .then_some(SourceOrigin::Wanikani { subject_id })
        }
        _ => None,
    }
}

fn publish_scope(
    transaction: &Transaction<'_>,
    scope: &KnowledgeScope,
    metadata: &ScopeMetadata,
    records: &[MappedKnowledgeRecord],
) -> Result<(), SqliteKnowledgeError> {
    bind_scope(transaction, scope, metadata)?;
    transaction.execute(
        "DELETE FROM source_profile_subjects WHERE source_learner_profile_id = ?1",
        [&scope.source_learner_profile_id],
    )?;
    let mut insert_subject = transaction.prepare(
        "INSERT INTO source_subjects (source_collection_id, external_id) VALUES (?1, ?2)
         ON CONFLICT (source_collection_id, external_id) DO NOTHING",
    )?;
    let mut find_subject = transaction.prepare(
        "SELECT id FROM source_subjects WHERE source_collection_id = ?1 AND external_id = ?2",
    )?;
    let mut upsert_vocabulary = transaction.prepare(
        "INSERT INTO source_vocabulary (source_subject_id, source_collection_id, word_id, spelling, has_direct_object_evidence)
         VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (source_subject_id) DO UPDATE SET
         word_id = excluded.word_id, spelling = excluded.spelling, has_direct_object_evidence = excluded.has_direct_object_evidence")?;
    let mut delete_readings = transaction
        .prepare("DELETE FROM source_vocabulary_readings WHERE source_subject_id = ?1")?;
    let mut insert_readings = transaction.prepare("INSERT INTO source_vocabulary_readings (source_subject_id, position, reading) VALUES (?1, ?2, ?3)")?;
    let mut delete_meanings = transaction
        .prepare("DELETE FROM source_vocabulary_meanings WHERE source_subject_id = ?1")?;
    let mut insert_meanings = transaction.prepare("INSERT INTO source_vocabulary_meanings (source_subject_id, position, meaning) VALUES (?1, ?2, ?3)")?;
    let mut insert_association = transaction.prepare(
        "INSERT INTO source_profile_subjects (source_learner_profile_id, source_subject_id, source_collection_id, position, content_kind) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    let mut insert_progress = transaction.prepare(
        "INSERT INTO source_learner_progress (source_learner_profile_id, source_subject_id, is_marked_known, learning_started_at, first_passed_at, is_hidden)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    for (position, record) in records.iter().enumerate() {
        insert_subject.execute(params![scope.source_collection_id, record.external_id])?;
        let subject_id: i64 = find_subject.query_row(
            params![scope.source_collection_id, record.external_id],
            |row| row.get(0),
        )?;
        if let MappedContent::Vocabulary(word) = &record.content {
            upsert_vocabulary.execute(params![
                subject_id,
                scope.source_collection_id,
                word.id,
                word.written_form,
                word.direct_object
            ])?;
            for (delete, insert, alternatives) in [
                (&mut delete_readings, &mut insert_readings, &word.readings),
                (&mut delete_meanings, &mut insert_meanings, &word.meanings),
            ] {
                delete.execute([subject_id])?;
                for (position, text) in alternatives.iter().enumerate() {
                    insert.execute(params![subject_id, position as i64, text])?;
                }
            }
        }
        let kind = match record.content {
            MappedContent::Vocabulary(_) => "vocabulary",
            MappedContent::NonVocabulary(_) => "non-vocabulary",
            MappedContent::Unavailable(_) => "unavailable",
        };
        insert_association.execute(params![
            scope.source_learner_profile_id,
            subject_id,
            scope.source_collection_id,
            position as i64,
            kind
        ])?;
        if let Some(observation) = record.progress {
            insert_progress.execute(params![
                scope.source_learner_profile_id,
                subject_id,
                observation.is_marked_known,
                observation
                    .learning_started_at
                    .map(|date| date.to_rfc3339()),
                observation.first_passed_at.map(|date| date.to_rfc3339()),
                observation.is_hidden
            ])?;
        }
    }
    transaction.execute(
        "UPDATE source_learner_profiles SET access_expires_at = ?2 WHERE id = ?1",
        params![
            scope.source_learner_profile_id,
            metadata.access_expires_at.map(|date| date.to_rfc3339())
        ],
    )?;
    Ok(())
}

fn read_scope(
    transaction: &Transaction<'_>,
    scope: &KnowledgeScope,
) -> Result<StoredKnowledge, SqliteKnowledgeError> {
    let metadata = read_metadata(transaction, scope)?;
    let mut statement = transaction.prepare(
        "SELECT a.source_subject_id, a.source_collection_id, a.position, a.content_kind, s.source_collection_id, s.external_id,
                v.source_collection_id, v.word_id, v.spelling, v.has_direct_object_evidence,
                p.source_subject_id, p.is_marked_known, p.learning_started_at, p.first_passed_at, p.is_hidden
         FROM source_profile_subjects a LEFT JOIN source_subjects s ON s.id = a.source_subject_id
         LEFT JOIN source_vocabulary v ON v.source_subject_id = a.source_subject_id
         LEFT JOIN source_learner_progress p ON p.source_learner_profile_id = a.source_learner_profile_id AND p.source_subject_id = a.source_subject_id
         WHERE a.source_learner_profile_id = ?1 ORDER BY a.position")?;
    let mut readings = transaction.prepare(
        "SELECT position, reading FROM source_vocabulary_readings WHERE source_subject_id = ?1 ORDER BY position")?;
    let mut meanings = transaction.prepare(
        "SELECT position, meaning FROM source_vocabulary_meanings WHERE source_subject_id = ?1 ORDER BY position")?;
    let mut rows = statement.query([&scope.source_learner_profile_id])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        let subject_id: i64 = row.get(0)?;
        let collection: String = row.get(1)?;
        let subject_collection: String = row
            .get::<_, Option<String>>(4)?
            .ok_or(SqliteKnowledgeError::Corrupt("missing subject"))?;
        if collection != scope.source_collection_id || subject_collection != collection {
            return Err(SqliteKnowledgeError::Corrupt(
                "cross-collection subject association",
            ));
        }
        if row.get::<_, i64>(2)? != records.len() as i64 {
            return Err(SqliteKnowledgeError::Corrupt("invalid subject order"));
        }
        let external_id: String = row.get(5)?;
        let origin = source_origin(metadata.provider, &external_id).ok_or(
            SqliteKnowledgeError::Corrupt("invalid external subject identity"),
        )?;
        let vocabulary_collection: Option<String> = row.get(6)?;
        if vocabulary_collection
            .as_ref()
            .is_some_and(|id| id != &collection)
        {
            return Err(SqliteKnowledgeError::Corrupt("cross-collection vocabulary"));
        }
        let content = match row.get::<_, String>(3)?.as_str() {
            "vocabulary" => {
                if vocabulary_collection.is_none() {
                    return Err(SqliteKnowledgeError::Corrupt("missing vocabulary"));
                }
                let word = InventoryWord {
                    id: row.get(7)?,
                    written_form: row.get(8)?,
                    direct_object: decode_bool(row.get(9)?)?,
                    origin,
                    readings: read_alternatives(&mut readings, subject_id)?,
                    meanings: read_alternatives(&mut meanings, subject_id)?,
                };
                word.validate()
                    .map_err(|_| SqliteKnowledgeError::Corrupt("invalid vocabulary"))?;
                MappedContent::Vocabulary(word)
            }
            "non-vocabulary" => MappedContent::NonVocabulary(origin),
            "unavailable" => MappedContent::Unavailable(origin),
            _ => return Err(SqliteKnowledgeError::Corrupt("unknown content kind")),
        };
        let progress = if row.get::<_, Option<i64>>(10)?.is_some() {
            Some(LearnerProgress {
                is_marked_known: decode_bool(row.get(11)?)?,
                learning_started_at: decode_date(row.get(12)?)?,
                first_passed_at: decode_date(row.get(13)?)?,
                is_hidden: decode_bool(row.get(14)?)?
                    .ok_or(SqliteKnowledgeError::Corrupt("missing hidden flag"))?,
            })
        } else {
            None
        };
        records.push(MappedKnowledgeRecord {
            external_id,
            content,
            progress,
        });
    }
    let orphan: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM source_learner_progress p LEFT JOIN source_profile_subjects a
         ON a.source_learner_profile_id = p.source_learner_profile_id AND a.source_subject_id = p.source_subject_id
         WHERE p.source_learner_profile_id = ?1 AND a.source_subject_id IS NULL)",
        [&scope.source_learner_profile_id],
        |row| row.get(0),
    )?;
    if orphan {
        return Err(SqliteKnowledgeError::Corrupt(
            "progress has no profile association",
        ));
    }
    Ok(StoredKnowledge {
        scope: scope.clone(),
        metadata,
        records,
    })
}

fn read_alternatives(
    statement: &mut Statement<'_>,
    subject_id: i64,
) -> Result<Vec<String>, SqliteKnowledgeError> {
    let mut rows = statement.query([subject_id])?;
    let mut alternatives = Vec::new();
    while let Some(row) = rows.next()? {
        if row.get::<_, i64>(0)? != alternatives.len() as i64 {
            return Err(SqliteKnowledgeError::Corrupt("invalid alternative order"));
        }
        alternatives.push(row.get(1)?);
    }
    Ok(alternatives)
}

fn decode_bool(value: Option<i64>) -> Result<Option<bool>, SqliteKnowledgeError> {
    match value {
        None => Ok(None),
        Some(0) => Ok(Some(false)),
        Some(1) => Ok(Some(true)),
        _ => Err(SqliteKnowledgeError::Corrupt("invalid boolean")),
    }
}

fn decode_date(value: Option<String>) -> Result<Option<DateTime<Utc>>, SqliteKnowledgeError> {
    value
        .map(|value| {
            let date = value
                .parse::<DateTime<Utc>>()
                .map_err(|_| SqliteKnowledgeError::Corrupt("invalid timestamp"))?;
            if date.to_rfc3339() != value {
                return Err(SqliteKnowledgeError::Corrupt("noncanonical UTC timestamp"));
            }
            Ok(date)
        })
        .transpose()
}

fn bind_scope(
    transaction: &Transaction<'_>,
    scope: &KnowledgeScope,
    metadata: &ScopeMetadata,
) -> Result<(), SqliteKnowledgeError> {
    transaction.execute(
        "INSERT INTO learners (id) VALUES (?1) ON CONFLICT DO NOTHING",
        [&scope.learner_id],
    )?;
    transaction.execute(
        "INSERT INTO source_collections (id, provider) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
        params![scope.source_collection_id, metadata.provider.tag()],
    )?;
    let kind: String = transaction.query_row(
        "SELECT provider FROM source_collections WHERE id = ?1",
        [&scope.source_collection_id],
        |row| row.get(0),
    )?;
    if Provider::decode(&kind)? != metadata.provider {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    transaction.execute("INSERT INTO source_learner_profiles (id, learner_id, source_collection_id, external_account_id) VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING",
        params![scope.source_learner_profile_id, scope.learner_id, scope.source_collection_id, metadata.external_account_id])?;
    let (learner_id, source_collection_id, account_id) = transaction.query_row(
        "SELECT learner_id, source_collection_id, external_account_id FROM source_learner_profiles WHERE id = ?1",
        [&scope.source_learner_profile_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?)),
    ).optional()?.ok_or(SqliteKnowledgeError::BindingMismatch)?;
    if learner_id != scope.learner_id
        || source_collection_id != scope.source_collection_id
        || account_id != metadata.external_account_id
    {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    Ok(())
}

fn read_metadata(
    transaction: &Transaction<'_>,
    scope: &KnowledgeScope,
) -> Result<ScopeMetadata, SqliteKnowledgeError> {
    let (learner_id, source_collection_id, account_id, deadline, provider, learner) = transaction.query_row(
        "SELECT p.learner_id, p.source_collection_id, p.external_account_id, p.access_expires_at, c.provider, l.id
         FROM source_learner_profiles p LEFT JOIN source_collections c ON c.id = p.source_collection_id
         LEFT JOIN learners l ON l.id = p.learner_id WHERE p.id = ?1",
        [&scope.source_learner_profile_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?)),
    ).optional()?.ok_or(SqliteKnowledgeError::MissingScope)?;
    if learner_id != scope.learner_id || source_collection_id != scope.source_collection_id {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    if learner.is_none() {
        return Err(SqliteKnowledgeError::Corrupt("missing learner"));
    }
    let provider = provider.ok_or(SqliteKnowledgeError::Corrupt("missing collection"))?;
    if account_id.as_ref().is_some_and(|id| id.trim().is_empty()) {
        return Err(SqliteKnowledgeError::Corrupt("blank external account"));
    }
    Ok(ScopeMetadata {
        provider: Provider::decode(&provider)?,
        external_account_id: account_id,
        access_expires_at: decode_date(deadline)?,
    })
}
