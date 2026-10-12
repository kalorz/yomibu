//! Synchronous SQLite persistence for explicitly scoped mapped observations.
//! Timestamps use chrono's canonical UTC text, including extended years.
#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Statement, Transaction, TransactionBehavior, params,
};
use std::{collections::BTreeSet, path::PathBuf, time::Duration};
use yomibu_core::domain::{
    inventory::{InventoryError, InventoryWord, MappedContent, SourceOrigin},
    knowledge::LearnerProgress,
};

const APPLICATION_ID: i64 = 0x594f4d49;
const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Manual,
    Wanikani,
}

impl SourceKind {
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
            _ => Err(SqliteKnowledgeError::Corrupt("unknown source kind")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeScope {
    pub learner_id: String,
    pub collection_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeMetadata {
    pub source_kind: SourceKind,
    pub provider_account_id: Option<String>,
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
    #[error("Knowledge scope binding disagrees with the stored identity.")]
    BindingMismatch,
    #[error("Invalid knowledge publication: {0}.")]
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
/// Writes request DELETE journaling, EXTRA synchronization, and fullfsync.
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
        let mut connection =
            self.open(OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)?;
        schema_state(&connection)?;
        connection.pragma_update(None, "journal_mode", "DELETE")?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if schema_state(&transaction)? {
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
            .provider_account_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
            return Err(SqliteKnowledgeError::InvalidInput("blank provider account"));
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
            if source_origin(metadata.source_kind, &record.external_id)
                .ok()
                .as_ref()
                != Some(origin)
            {
                return Err(SqliteKnowledgeError::InvalidInput(
                    "source provenance disagrees with external identity",
                ));
            }
        }
        let mut connection = self.open(OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_schema(&transaction)?;
        publish_scope(&transaction, scope, metadata, records)?;
        commit_transaction(transaction)
    }

    /// Read a coherent scope without writes, creation, migration, or access policy.
    /// A hot rollback journal may require explicit writable initialization first.
    pub fn read(&self, scope: &KnowledgeScope) -> Result<StoredKnowledge, SqliteKnowledgeError> {
        validate_scope(scope)?;
        let mut connection = self.open(OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let transaction = connection.transaction()?;
        require_schema(&transaction)?;
        read_scope(&transaction, scope).map_err(|error| match error {
            SqliteKnowledgeError::Database(
                rusqlite::Error::InvalidColumnType(..)
                | rusqlite::Error::FromSqlConversionFailure(..)
                | rusqlite::Error::IntegralValueOutOfRange(..),
            ) => SqliteKnowledgeError::Corrupt("invalid stored value type"),
            error => error,
        })
    }

    fn open(&self, flags: OpenFlags) -> Result<Connection, SqliteKnowledgeError> {
        if !flags.contains(OpenFlags::SQLITE_OPEN_CREATE) {
            match std::fs::metadata(&self.path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(SqliteKnowledgeError::MissingDatabase);
                }
                Err(error) => return Err(error.into()),
                Ok(_) => {}
            }
        }
        let connection = Connection::open_with_flags(&self.path, flags)?;
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

fn schema_state(connection: &Connection) -> Result<bool, SqliteKnowledgeError> {
    let application_id: i64 =
        connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if application_id == APPLICATION_ID && version == SCHEMA_VERSION {
        return Ok(false);
    }
    let empty: bool = connection.query_row(
        "SELECT NOT EXISTS(SELECT 1 FROM sqlite_schema)",
        [],
        |row| row.get(0),
    )?;
    if application_id == 0 && version == 0 && empty {
        Ok(true)
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
    if schema_state(connection)? {
        Err(SqliteKnowledgeError::UnsupportedSchema)
    } else {
        Ok(())
    }
}

fn validate_scope(scope: &KnowledgeScope) -> Result<(), SqliteKnowledgeError> {
    if [&scope.learner_id, &scope.collection_id, &scope.profile_id]
        .iter()
        .any(|id| id.trim().is_empty())
    {
        return Err(SqliteKnowledgeError::InvalidInput("blank scope identity"));
    }
    Ok(())
}

fn source_origin(
    kind: SourceKind,
    external_id: &str,
) -> Result<SourceOrigin, SqliteKnowledgeError> {
    match kind {
        SourceKind::Manual if !external_id.trim().is_empty() => Ok(SourceOrigin::Manual),
        SourceKind::Wanikani => {
            let subject_id: u64 = external_id
                .parse()
                .map_err(|_| SqliteKnowledgeError::Corrupt("invalid external subject identity"))?;
            if subject_id == 0 || subject_id.to_string() != external_id {
                return Err(SqliteKnowledgeError::Corrupt(
                    "noncanonical external subject identity",
                ));
            }
            Ok(SourceOrigin::Wanikani { subject_id })
        }
        _ => Err(SqliteKnowledgeError::Corrupt(
            "blank external subject identity",
        )),
    }
}

fn publish_scope(
    transaction: &Transaction<'_>,
    scope: &KnowledgeScope,
    metadata: &ScopeMetadata,
    records: &[MappedKnowledgeRecord],
) -> Result<(), SqliteKnowledgeError> {
    bind_scope(transaction, scope, metadata)?;
    let mut insert_subject = transaction.prepare(
        "INSERT INTO source_subjects (collection_id, external_id) VALUES (?1, ?2)
         ON CONFLICT (collection_id, external_id) DO NOTHING",
    )?;
    let mut find_subject = transaction.prepare(
        "SELECT subject_id FROM source_subjects WHERE collection_id = ?1 AND external_id = ?2",
    )?;
    let mut vocabulary = transaction.prepare(
        "INSERT INTO source_vocabulary (subject_id, collection_id, word_id, written_form, direct_object)
         VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (subject_id) DO UPDATE SET
         word_id = excluded.word_id, written_form = excluded.written_form, direct_object = excluded.direct_object")?;
    let mut delete_readings =
        transaction.prepare("DELETE FROM source_vocabulary_readings WHERE subject_id = ?1")?;
    let mut insert_readings = transaction.prepare("INSERT INTO source_vocabulary_readings (subject_id, position, reading) VALUES (?1, ?2, ?3)")?;
    let mut delete_meanings =
        transaction.prepare("DELETE FROM source_vocabulary_meanings WHERE subject_id = ?1")?;
    let mut insert_meanings = transaction.prepare("INSERT INTO source_vocabulary_meanings (subject_id, position, meaning) VALUES (?1, ?2, ?3)")?;
    let mut subject_ids = Vec::with_capacity(records.len());
    for record in records {
        insert_subject.execute(params![scope.collection_id, record.external_id])?;
        let subject_id: i64 = find_subject
            .query_row(params![scope.collection_id, record.external_id], |row| {
                row.get(0)
            })?;
        subject_ids.push(subject_id);
        if let MappedContent::Vocabulary(word) = &record.content {
            vocabulary.execute(params![
                subject_id,
                scope.collection_id,
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
    }
    transaction.execute(
        "DELETE FROM source_profile_subjects WHERE profile_id = ?1",
        [&scope.profile_id],
    )?;
    let mut associations = transaction.prepare(
        "INSERT INTO source_profile_subjects (profile_id, subject_id, collection_id, position, content_kind) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    let mut progress = transaction.prepare(
        "INSERT INTO source_learner_progress (profile_id, subject_id, is_marked_known, learning_started_at, first_passed_at, is_hidden)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
    for (position, (record, subject_id)) in records.iter().zip(subject_ids).enumerate() {
        let kind = match record.content {
            MappedContent::Vocabulary(_) => "vocabulary",
            MappedContent::NonVocabulary(_) => "non-vocabulary",
            MappedContent::Unavailable(_) => "unavailable",
        };
        associations.execute(params![
            scope.profile_id,
            subject_id,
            scope.collection_id,
            position as i64,
            kind
        ])?;
        if let Some(observation) = record.progress {
            progress.execute(params![
                scope.profile_id,
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
        "UPDATE source_learner_profiles SET access_expires_at = ?2 WHERE profile_id = ?1",
        params![
            scope.profile_id,
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
        "SELECT a.subject_id, a.collection_id, a.position, a.content_kind, s.collection_id, s.external_id,
                v.collection_id, v.word_id, v.written_form, v.direct_object,
                p.subject_id, p.is_marked_known, p.learning_started_at, p.first_passed_at, p.is_hidden
         FROM source_profile_subjects a LEFT JOIN source_subjects s ON s.subject_id = a.subject_id
         LEFT JOIN source_vocabulary v ON v.subject_id = a.subject_id
         LEFT JOIN source_learner_progress p ON p.profile_id = a.profile_id AND p.subject_id = a.subject_id
         WHERE a.profile_id = ?1 ORDER BY a.position")?;
    let mut readings = transaction.prepare(
        "SELECT position, reading FROM source_vocabulary_readings WHERE subject_id = ?1 ORDER BY position")?;
    let mut meanings = transaction.prepare(
        "SELECT position, meaning FROM source_vocabulary_meanings WHERE subject_id = ?1 ORDER BY position")?;
    let mut rows = statement.query([&scope.profile_id])?;
    let mut records = Vec::new();
    while let Some(row) = rows.next()? {
        let subject_id: i64 = row.get(0)?;
        let collection: String = row.get(1)?;
        let subject_collection: String = row
            .get::<_, Option<String>>(4)?
            .ok_or(SqliteKnowledgeError::Corrupt("missing subject"))?;
        if collection != scope.collection_id || subject_collection != collection {
            return Err(SqliteKnowledgeError::Corrupt(
                "cross-collection subject association",
            ));
        }
        if row.get::<_, i64>(2)? != records.len() as i64 {
            return Err(SqliteKnowledgeError::Corrupt("invalid subject order"));
        }
        let external_id: String = row.get(5)?;
        let origin = source_origin(metadata.source_kind, &external_id)?;
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
         ON a.profile_id = p.profile_id AND a.subject_id = p.subject_id
         WHERE p.profile_id = ?1 AND a.subject_id IS NULL)",
        [&scope.profile_id],
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
        "INSERT INTO learners (learner_id) VALUES (?1) ON CONFLICT DO NOTHING",
        [&scope.learner_id],
    )?;
    transaction.execute("INSERT INTO source_collections (collection_id, source_kind) VALUES (?1, ?2) ON CONFLICT DO NOTHING", params![scope.collection_id, metadata.source_kind.tag()])?;
    let kind: String = transaction.query_row(
        "SELECT source_kind FROM source_collections WHERE collection_id = ?1",
        [&scope.collection_id],
        |row| row.get(0),
    )?;
    if SourceKind::decode(&kind)? != metadata.source_kind {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    transaction.execute("INSERT INTO source_learner_profiles (profile_id, learner_id, collection_id, provider_account_id) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (profile_id) DO NOTHING",
        params![scope.profile_id, scope.learner_id, scope.collection_id, metadata.provider_account_id])?;
    let binding = transaction.query_row(
        "SELECT learner_id, collection_id, provider_account_id FROM source_learner_profiles WHERE profile_id = ?1",
        [&scope.profile_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?)),
    )?;
    if binding.0 != scope.learner_id
        || binding.1 != scope.collection_id
        || binding.2 != metadata.provider_account_id
    {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    Ok(())
}

fn read_metadata(
    transaction: &Transaction<'_>,
    scope: &KnowledgeScope,
) -> Result<ScopeMetadata, SqliteKnowledgeError> {
    let row = transaction.query_row(
        "SELECT p.learner_id, p.collection_id, p.provider_account_id, p.access_expires_at, c.source_kind, l.learner_id
         FROM source_learner_profiles p LEFT JOIN source_collections c ON c.collection_id = p.collection_id
         LEFT JOIN learners l ON l.learner_id = p.learner_id WHERE p.profile_id = ?1",
        [&scope.profile_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?)),
    ).optional()?.ok_or(SqliteKnowledgeError::MissingScope)?;
    if row.0 != scope.learner_id || row.1 != scope.collection_id {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    if row.5.is_none() {
        return Err(SqliteKnowledgeError::Corrupt("missing learner"));
    }
    let source_kind = row
        .4
        .ok_or(SqliteKnowledgeError::Corrupt("missing collection"))?;
    if row.2.as_ref().is_some_and(|id| id.trim().is_empty()) {
        return Err(SqliteKnowledgeError::Corrupt("blank provider account"));
    }
    Ok(ScopeMetadata {
        source_kind: SourceKind::decode(&source_kind)?,
        provider_account_id: row.2,
        access_expires_at: decode_date(row.3)?,
    })
}
