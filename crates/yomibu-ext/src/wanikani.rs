//! Preparation of supplied WaniKani records; no HTTP reading or import orchestration.
use yomibu_core::{
    domain::{LearnerId, MaterialKind},
    ports::infrastructure::storage::{
        LearnerProgressSourceIdentity, PreparedProgress, PreparedSaveGroup, PreparedSubject,
        SharedCollection, SourceRecord,
    },
};

/// Sanitized failure preparing supplied integration data.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct InvalidImport(String);

/// Shared identity selected by this integration, never by the storage adapter.
pub fn collection() -> SharedCollection {
    SharedCollection {
        provider: "wanikani".into(),
        key: "subjects".into(),
        name: "WaniKani".into(),
    }
}

/// Translate a supplied provider object type; retain its original type in source data.
pub fn subject_kind(object: &str) -> Result<Option<MaterialKind>, InvalidImport> {
    match object {
        "vocabulary" | "kana_vocabulary" => Ok(Some(MaterialKind::Vocabulary)),
        "kanji" | "radical" => Ok(None),
        _ => Err(InvalidImport("unsupported WaniKani subject type".into())),
    }
}

fn subject_id(value: &str) -> Result<String, InvalidImport> {
    value
        .parse::<u64>()
        .ok()
        .filter(|id| *id > 0)
        .map(|id| id.to_string())
        .ok_or_else(|| InvalidImport("invalid WaniKani subject ID".into()))
}
fn object(source: &SourceRecord) -> Result<(), InvalidImport> {
    if source.payload().get().starts_with('{') {
        Ok(())
    } else {
        Err(InvalidImport(
            "WaniKani source payload must be an object".into(),
        ))
    }
}

/// Validate supplied translated records and prepare the shared storage request.
/// Account identity is the supplied user ID, never a credential. Canonicalize numeric
/// subject IDs before duplicate detection; do not infer IDs/types from opaque payloads.
/// Full HTTP response parsing and credential exclusion remain the future reader's job.
pub fn prepare_group(
    learner_id: LearnerId,
    external_account_id: String,
    account: SourceRecord,
    mut subjects: Vec<PreparedSubject>,
    mut progress: Vec<PreparedProgress>,
) -> Result<PreparedSaveGroup, InvalidImport> {
    if external_account_id.is_empty() {
        return Err(InvalidImport(
            "WaniKani account ID must not be empty".into(),
        ));
    }
    object(&account)?;
    for subject in &mut subjects {
        subject.external_id = subject_id(&subject.external_id)?;
        if subject.kind == Some(MaterialKind::Grammar) {
            return Err(InvalidImport(
                "WaniKani subjects cannot have grammar kind".into(),
            ));
        }
        object(&subject.source)?;
    }
    for record in &mut progress {
        record.subject_external_id = subject_id(&record.subject_external_id)?;
        object(&record.source)?;
    }
    PreparedSaveGroup::new(
        collection(),
        LearnerProgressSourceIdentity {
            learner_id,
            external_account_id,
        },
        account,
        subjects,
        progress,
    )
    .map_err(|error| InvalidImport(error.to_string()))
}
