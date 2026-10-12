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
    inventory::{
        InventoryError, MappedContent, SourceOrigin, UnresolvedVocabulary, VocabularyEntry,
        VocabularyTranslation,
    },
    knowledge::LearnerProgress,
};

const APPLICATION_ID: i64 = 0x594f4d49;
const SCHEMA_VERSION: i64 = 2;

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
    pub learner_id: i64,
    pub source_collection_id: i64,
    pub source_learner_profile_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeMetadata {
    pub provider: Provider,
    pub learner_name: String,
    pub source_collection_name: String,
    pub owner_learner_id: Option<i64>,
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

    /// Create schema 2 in an empty database, or verify its application/version IDs.
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
        if metadata
            .external_account_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
            return Err(SqliteKnowledgeError::InvalidInput("blank external account"));
        }
        let mut external_ids = BTreeSet::new();
        let mut word_ids = BTreeSet::new();
        let mut translation_ids = BTreeSet::new();
        for record in records {
            if record.external_id.trim().is_empty() || !external_ids.insert(&record.external_id) {
                return Err(SqliteKnowledgeError::InvalidInput(
                    "invalid or duplicate external identity",
                ));
            }
            match &record.content {
                MappedContent::UnresolvedVocabulary(unresolved) => unresolved.word.validate()?,
                MappedContent::Vocabulary { entries, .. } => {
                    if entries.is_empty() {
                        return Err(SqliteKnowledgeError::InvalidInput(
                            "missing vocabulary entries",
                        ));
                    }
                    for entry in entries {
                        entry.validate()?;
                        let mut orders = BTreeSet::new();
                        for translation in &entry.translations {
                            if !translation_ids.insert(translation.id)
                                || !orders
                                    .insert((&translation.language_code, translation.display_order))
                            {
                                return Err(SqliteKnowledgeError::InvalidInput(
                                    "duplicate translation identity or order",
                                ));
                            }
                        }
                    }
                }
                MappedContent::NonVocabulary(_) | MappedContent::Unavailable(_) => {}
            }
            if !register_word_ids(&record.content, &mut word_ids) {
                return Err(SqliteKnowledgeError::InvalidInput(
                    "duplicate vocabulary identity",
                ));
            }
            if source_origin(metadata.provider, &record.external_id).as_ref()
                != Some(record.content.origin())
            {
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

fn register_word_ids(content: &MappedContent, ids: &mut BTreeSet<String>) -> bool {
    match content {
        MappedContent::Vocabulary { entries, .. } => {
            entries.iter().all(|entry| ids.insert(entry.inventory_id()))
        }
        MappedContent::UnresolvedVocabulary(unresolved) => ids.insert(unresolved.word.id.clone()),
        MappedContent::NonVocabulary(_) | MappedContent::Unavailable(_) => true,
    }
}

fn source_origin(provider: Provider, external_id: &str) -> Option<SourceOrigin> {
    match provider {
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
        "INSERT INTO source_subjects (source_collection_id, external_id, material_type, source_data) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (source_collection_id, external_id) DO UPDATE SET
         material_type = coalesce(source_subjects.material_type, excluded.material_type),
         source_data = coalesce(excluded.source_data, source_subjects.source_data)")?;
    let mut find_subject = transaction.prepare(
        "SELECT id, material_type FROM source_subjects WHERE source_collection_id = ?1 AND external_id = ?2")?;
    let mut find_entries =
        transaction.prepare("SELECT id FROM vocabulary_entries WHERE source_subject_id = ?1")?;
    let mut delete_entry = transaction.prepare("DELETE FROM vocabulary_entries WHERE id = ?1")?;
    let mut upsert_entry = transaction.prepare(
        "INSERT INTO vocabulary_entries (id, source_subject_id, spelling, reading, is_primary_reading_in_source, mapping_method, has_direct_object_evidence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (id) DO UPDATE SET
         spelling=excluded.spelling, reading=excluded.reading, is_primary_reading_in_source=excluded.is_primary_reading_in_source,
         mapping_method=excluded.mapping_method, has_direct_object_evidence=excluded.has_direct_object_evidence
         WHERE vocabulary_entries.source_subject_id=excluded.source_subject_id")?;
    let mut delete_parts = transaction
        .prepare("DELETE FROM vocabulary_entry_parts_of_speech WHERE vocabulary_entry_id = ?1")?;
    let mut insert_part =
        transaction.prepare("INSERT INTO vocabulary_entry_parts_of_speech VALUES (?1, ?2, ?3)")?;
    let mut delete_translations = transaction
        .prepare("DELETE FROM vocabulary_translations WHERE vocabulary_entry_id = ?1")?;
    let mut upsert_translation = transaction.prepare(
        "INSERT INTO vocabulary_translations (id, vocabulary_entry_id, language_code, text, display_order, is_primary_in_source, origin)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (id) DO UPDATE SET
         language_code=excluded.language_code, text=excluded.text, display_order=excluded.display_order,
         is_primary_in_source=excluded.is_primary_in_source, origin=excluded.origin
         WHERE vocabulary_translations.vocabulary_entry_id=excluded.vocabulary_entry_id")?;
    let mut insert_association = transaction.prepare(
        "INSERT INTO source_profile_subjects (source_learner_profile_id, source_subject_id, source_collection_id, position, content_kind) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    let mut insert_progress = transaction.prepare(
        "INSERT INTO source_learner_progress (source_learner_profile_id, source_subject_id, is_marked_known, learning_started_at, first_passed_at, is_hidden, source_collection_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?;
    for (position, record) in records.iter().enumerate() {
        let (kind, material_type, source_data) = match &record.content {
            MappedContent::Vocabulary { .. } => ("vocabulary", Some("vocabulary"), None),
            MappedContent::UnresolvedVocabulary(unresolved) => (
                "unresolved-vocabulary",
                Some("vocabulary"),
                Some(serde_json::to_string(unresolved).map_err(|_| {
                    SqliteKnowledgeError::InvalidInput("invalid subject observation")
                })?),
            ),
            MappedContent::NonVocabulary(_) => ("non-vocabulary", None, None),
            MappedContent::Unavailable(_) => ("unavailable", None, None),
        };
        insert_subject.execute(params![
            scope.source_collection_id,
            record.external_id,
            material_type,
            source_data
        ])?;
        let (subject_id, stored_type): (i64, Option<String>) = find_subject.query_row(
            params![scope.source_collection_id, record.external_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if material_type.is_some() && stored_type.as_deref() != material_type {
            return Err(SqliteKnowledgeError::BindingMismatch);
        }
        if let MappedContent::Vocabulary { entries, .. } = &record.content {
            let retained: BTreeSet<_> = entries.iter().map(|entry| entry.id).collect();
            let existing = find_entries
                .query_map([subject_id], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for id in existing {
                if !retained.contains(&id) {
                    delete_entry.execute([id])?;
                }
            }
            for entry in entries {
                if upsert_entry.execute(params![
                    entry.id,
                    subject_id,
                    entry.spelling,
                    entry.reading,
                    entry.is_primary_reading_in_source,
                    entry.mapping_method,
                    entry.has_direct_object_evidence
                ])? != 1
                {
                    return Err(SqliteKnowledgeError::BindingMismatch);
                }
                delete_parts.execute([entry.id])?;
                for (position, part) in entry.parts_of_speech.iter().enumerate() {
                    insert_part.execute(params![entry.id, position as i64, part])?;
                }
                // Replacing child rows permits swaps of unique display orders.
                delete_translations.execute([entry.id])?;
                for translation in &entry.translations {
                    if upsert_translation.execute(params![
                        translation.id,
                        entry.id,
                        translation.language_code,
                        translation.text,
                        translation.display_order,
                        translation.is_primary_in_source,
                        translation.origin
                    ])? != 1
                    {
                        return Err(SqliteKnowledgeError::BindingMismatch);
                    }
                }
            }
        }
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
                observation.is_hidden,
                scope.source_collection_id
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
                s.material_type, s.source_data, p.source_subject_id, p.is_marked_known, p.learning_started_at, p.first_passed_at, p.is_hidden, p.source_collection_id
         FROM source_profile_subjects a LEFT JOIN source_subjects s ON s.id = a.source_subject_id
         LEFT JOIN source_learner_progress p ON p.source_learner_profile_id = a.source_learner_profile_id AND p.source_subject_id = a.source_subject_id
         WHERE a.source_learner_profile_id = ?1 ORDER BY a.position")?;
    let mut entries = transaction.prepare(
        "SELECT id, spelling, reading, is_primary_reading_in_source, mapping_method, has_direct_object_evidence, material_type
         FROM vocabulary_entries WHERE source_subject_id = ?1 ORDER BY id")?;
    let mut parts = transaction.prepare(
        "SELECT position, part_of_speech FROM vocabulary_entry_parts_of_speech WHERE vocabulary_entry_id = ?1 ORDER BY position")?;
    let mut translations = transaction.prepare(
        "SELECT id, language_code, text, display_order, is_primary_in_source, origin
         FROM vocabulary_translations WHERE vocabulary_entry_id = ?1 ORDER BY language_code, display_order")?;
    let mut rows = statement.query([&scope.source_learner_profile_id])?;
    let mut records = Vec::new();
    let mut word_ids = BTreeSet::new();
    while let Some(row) = rows.next()? {
        let subject_id: i64 = row.get(0)?;
        let collection: i64 = row.get(1)?;
        let subject_collection: i64 = row
            .get::<_, Option<i64>>(4)?
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
        let material_type: Option<String> = row.get(6)?;
        let content = match row.get::<_, String>(3)?.as_str() {
            "vocabulary" => {
                if material_type.as_deref() != Some("vocabulary") {
                    return Err(SqliteKnowledgeError::Corrupt(
                        "invalid vocabulary subject type",
                    ));
                }
                let entries =
                    read_entries(&mut entries, &mut parts, &mut translations, subject_id)?;
                if entries.is_empty() {
                    return Err(SqliteKnowledgeError::Corrupt("missing vocabulary entries"));
                }
                MappedContent::Vocabulary { entries, origin }
            }
            "unresolved-vocabulary" => {
                if material_type.as_deref() != Some("vocabulary") {
                    return Err(SqliteKnowledgeError::Corrupt(
                        "invalid unresolved subject type",
                    ));
                }
                let source_data: String =
                    row.get::<_, Option<String>>(7)?
                        .ok_or(SqliteKnowledgeError::Corrupt(
                            "missing unresolved observation",
                        ))?;
                let mut value: serde_json::Value = serde_json::from_str(&source_data)
                    .map_err(|_| SqliteKnowledgeError::Corrupt("invalid subject observation"))?;
                if value["word"]["origin"]
                    != serde_json::to_value(&origin)
                        .map_err(|_| SqliteKnowledgeError::Corrupt("invalid source provenance"))?
                {
                    return Err(SqliteKnowledgeError::Corrupt("invalid source provenance"));
                }
                // Manual-file decoding excludes provenance; it has just been checked above.
                if let Some(word) = value["word"].as_object_mut() {
                    word.remove("origin");
                }
                let mut unresolved: UnresolvedVocabulary = serde_json::from_value(value)
                    .map_err(|_| SqliteKnowledgeError::Corrupt("invalid unresolved vocabulary"))?;
                unresolved.word.origin = origin;
                unresolved
                    .word
                    .validate()
                    .map_err(|_| SqliteKnowledgeError::Corrupt("invalid unresolved vocabulary"))?;
                MappedContent::UnresolvedVocabulary(unresolved)
            }
            "non-vocabulary" => MappedContent::NonVocabulary(origin),
            "unavailable" => MappedContent::Unavailable(origin),
            _ => return Err(SqliteKnowledgeError::Corrupt("unknown content kind")),
        };
        if !register_word_ids(&content, &mut word_ids) {
            return Err(SqliteKnowledgeError::Corrupt(
                "duplicate vocabulary identity",
            ));
        }
        let progress = if row.get::<_, Option<i64>>(8)?.is_some() {
            if row.get::<_, i64>(13)? != collection {
                return Err(SqliteKnowledgeError::Corrupt("cross-collection progress"));
            }
            Some(LearnerProgress {
                is_marked_known: decode_bool(row.get(9)?)?,
                learning_started_at: decode_date(row.get(10)?)?,
                first_passed_at: decode_date(row.get(11)?)?,
                is_hidden: decode_bool(row.get(12)?)?
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

fn read_entries(
    entries: &mut Statement<'_>,
    parts: &mut Statement<'_>,
    translations: &mut Statement<'_>,
    subject_id: i64,
) -> Result<Vec<VocabularyEntry>, SqliteKnowledgeError> {
    let mut rows = entries.query([subject_id])?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        if row.get::<_, String>(6)? != "vocabulary" {
            return Err(SqliteKnowledgeError::Corrupt(
                "invalid vocabulary discriminator",
            ));
        }
        let id = row.get(0)?;
        let mut translated = Vec::new();
        let mut translation_rows = translations.query([id])?;
        while let Some(translation) = translation_rows.next()? {
            translated.push(VocabularyTranslation {
                id: translation.get(0)?,
                language_code: translation.get(1)?,
                text: translation.get(2)?,
                display_order: translation.get(3)?,
                is_primary_in_source: decode_bool(translation.get(4)?)?,
                origin: translation.get(5)?,
            });
        }
        let entry = VocabularyEntry {
            id,
            spelling: row.get(1)?,
            reading: row.get(2)?,
            parts_of_speech: read_parts_of_speech(parts, id)?,
            is_primary_reading_in_source: decode_bool(row.get(3)?)?,
            mapping_method: row.get(4)?,
            has_direct_object_evidence: decode_bool(row.get(5)?)?,
            translations: translated,
        };
        entry
            .validate()
            .map_err(|_| SqliteKnowledgeError::Corrupt("invalid vocabulary entry"))?;
        result.push(entry);
    }
    Ok(result)
}

fn read_parts_of_speech(
    statement: &mut Statement<'_>,
    entry_id: i64,
) -> Result<Vec<String>, SqliteKnowledgeError> {
    let mut rows = statement.query([entry_id])?;
    let mut alternatives = Vec::new();
    while let Some(row) = rows.next()? {
        if row.get::<_, i64>(0)? != alternatives.len() as i64 {
            return Err(SqliteKnowledgeError::Corrupt(
                "invalid part-of-speech order",
            ));
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
        "INSERT INTO learners (id, name) VALUES (?1, ?2) ON CONFLICT (id) DO UPDATE SET name = excluded.name",
        params![scope.learner_id, metadata.learner_name],
    )?;
    transaction.execute(
        "INSERT INTO source_collections (id, provider, name, owner_learner_id) VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING",
        params![scope.source_collection_id, metadata.provider.tag(), metadata.source_collection_name, metadata.owner_learner_id],
    )?;
    let (provider, owner): (String, Option<i64>) = transaction.query_row(
        "SELECT provider, owner_learner_id FROM source_collections WHERE id = ?1",
        [scope.source_collection_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if Provider::decode(&provider)? != metadata.provider || owner != metadata.owner_learner_id {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    transaction.execute("INSERT INTO source_learner_profiles (id, learner_id, source_collection_id, external_account_id) VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING",
        params![scope.source_learner_profile_id, scope.learner_id, scope.source_collection_id, metadata.external_account_id])?;
    let (learner_id, source_collection_id, account_id) = transaction.query_row(
        "SELECT learner_id, source_collection_id, external_account_id FROM source_learner_profiles WHERE id = ?1",
        [scope.source_learner_profile_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<String>>(2)?)),
    ).optional()?.ok_or(SqliteKnowledgeError::BindingMismatch)?;
    if learner_id != scope.learner_id
        || source_collection_id != scope.source_collection_id
        || account_id != metadata.external_account_id
    {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    transaction.execute(
        "UPDATE source_collections SET name = ?2 WHERE id = ?1",
        params![scope.source_collection_id, metadata.source_collection_name],
    )?;
    Ok(())
}

fn read_metadata(
    transaction: &Transaction<'_>,
    scope: &KnowledgeScope,
) -> Result<ScopeMetadata, SqliteKnowledgeError> {
    let (learner_id, source_collection_id, account_id, deadline, provider, learner_name, collection_name, owner, owner_parent) = transaction.query_row(
        "SELECT p.learner_id, p.source_collection_id, p.external_account_id, p.access_expires_at, c.provider, l.name, c.name, c.owner_learner_id, o.id
         FROM source_learner_profiles p LEFT JOIN source_collections c ON c.id = p.source_collection_id
         LEFT JOIN learners l ON l.id = p.learner_id LEFT JOIN learners o ON o.id = c.owner_learner_id WHERE p.id = ?1",
        [scope.source_learner_profile_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?, row.get::<_, Option<i64>>(7)?, row.get::<_, Option<i64>>(8)?)),
    ).optional()?.ok_or(SqliteKnowledgeError::MissingScope)?;
    if learner_id != scope.learner_id || source_collection_id != scope.source_collection_id {
        return Err(SqliteKnowledgeError::BindingMismatch);
    }
    let learner_name = learner_name.ok_or(SqliteKnowledgeError::Corrupt("missing learner"))?;
    let provider = provider.ok_or(SqliteKnowledgeError::Corrupt("missing collection"))?;
    let source_collection_name =
        collection_name.ok_or(SqliteKnowledgeError::Corrupt("missing collection name"))?;
    if owner.is_some() && owner != owner_parent {
        return Err(SqliteKnowledgeError::Corrupt("missing collection owner"));
    }
    if account_id.as_ref().is_some_and(|id| id.trim().is_empty()) {
        return Err(SqliteKnowledgeError::Corrupt("blank external account"));
    }
    Ok(ScopeMetadata {
        provider: Provider::decode(&provider)?,
        learner_name,
        source_collection_name,
        owner_learner_id: owner,
        external_account_id: account_id,
        access_expires_at: decode_date(deadline)?,
    })
}
