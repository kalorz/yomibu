//! Preparation of supplied WaniKani records; no HTTP reading or import orchestration.
use std::fmt;
use yomibu_core::{
    domain::{LearnerId, MaterialKind},
    ports::infrastructure::collection_import::{
        PreparedProgress, PreparedSaveGroup, PreparedSubject, ProgressSourceIdentity,
        SharedCollection, SourceRecord,
    },
};

/// Sanitized failure preparing supplied integration data.
#[derive(Debug)]
pub struct InvalidImport(String);
impl fmt::Display for InvalidImport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for InvalidImport {}

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
        ProgressSourceIdentity {
            learner_id,
            external_account_id,
        },
        account,
        subjects,
        progress,
    )
    .map_err(|error| InvalidImport(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(raw: &str) -> SourceRecord {
        SourceRecord::new(
            chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            raw,
        )
        .unwrap()
    }
    fn subject(id: &str, kind: Option<MaterialKind>) -> PreparedSubject {
        PreparedSubject {
            external_id: id.into(),
            kind,
            source: source("{}"),
        }
    }
    fn prepare(subjects: Vec<PreparedSubject>) -> Result<PreparedSaveGroup, InvalidImport> {
        prepare_group(
            LearnerId(uuid::Uuid::nil()),
            "account".into(),
            source("{}"),
            subjects,
            vec![],
        )
    }
    #[test]
    fn type_translation_and_restrictions_belong_to_integration() {
        for object in ["vocabulary", "kana_vocabulary"] {
            assert_eq!(
                subject_kind(object).unwrap(),
                Some(MaterialKind::Vocabulary)
            );
        }
        for object in ["kanji", "radical"] {
            assert_eq!(subject_kind(object).unwrap(), None);
        }
        assert!(subject_kind("grammar").is_err());
        assert!(prepare(vec![subject("1", Some(MaterialKind::Grammar))]).is_err());
        for id in ["0", "-1", "lesson:alpha/7", "18446744073709551616"] {
            assert!(prepare(vec![subject(id, None)]).is_err());
        }
        let input = prepare(vec![subject("001", None)]).unwrap();
        assert_eq!(input.subjects()[0].external_id, "1");
        assert_eq!(input.collection(), &collection());
        assert!(prepare(vec![subject("001", None), subject("1", None)]).is_err());
    }
    #[test]
    fn account_progress_and_payload_shape_are_validated_here() {
        let learner = LearnerId(uuid::Uuid::nil());
        assert!(prepare_group(learner, String::new(), source("{}"), vec![], vec![]).is_err());
        assert!(prepare_group(learner, "a".into(), source("[]"), vec![], vec![]).is_err());
        assert!(
            prepare(vec![PreparedSubject {
                source: source("null"),
                ..subject("1", None)
            }])
            .is_err()
        );
        let progress = |id: &str, raw: &str| PreparedProgress {
            subject_external_id: id.into(),
            progress_stage: None,
            is_hidden: false,
            learning_started_at: None,
            first_passed_at: None,
            observed_at: source("{}").updated_at(),
            source: source(raw),
        };
        for record in [progress("0", "{}"), progress("1", "[]")] {
            assert!(
                prepare_group(learner, "a".into(), source("{}"), vec![], vec![record]).is_err()
            );
        }
        let input = prepare_group(
            learner,
            "a".into(),
            source("{}"),
            vec![],
            vec![progress("001", "{}")],
        )
        .unwrap();
        assert_eq!(input.progress()[0].subject_external_id, "1");
    }
}
