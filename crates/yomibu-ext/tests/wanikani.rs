use yomibu_core::{
    domain::{LearnerId, MaterialKind},
    ports::infrastructure::storage::*,
};
use yomibu_ext::wanikani::*;
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
    // Equivalent positive integer spellings normalize before identity checks.
    for id in ["001", "+1", "+001"] {
        let input = prepare(vec![subject(id, None)]).unwrap();
        assert_eq!(input.subjects()[0].external_id, "1");
        assert_eq!(input.collection(), &collection());
        assert!(prepare(vec![subject(id, None), subject("1", None)]).is_err());
    }
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
        assert!(prepare_group(learner, "a".into(), source("{}"), vec![], vec![record]).is_err());
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
