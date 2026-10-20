use chrono::{DateTime, TimeZone, Utc};
use yomibu_core::{
    domain::{LearnerId, MaterialKind},
    ports::infrastructure::storage::*,
};

fn collection() -> SharedCollection {
    SharedCollection {
        provider: "source".into(),
        key: "lessons".into(),
        name: "Lessons".into(),
    }
}

fn time() -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000, 1).unwrap()
}

fn source() -> SourceRecord {
    SourceRecord::new(time(), r#"{"id":1,"nested":{"all":[1,2]}}"#).unwrap()
}

fn progress(id: u64) -> PreparedProgress {
    PreparedProgress {
        subject_external_id: id.to_string(),
        progress_stage: Some("5".into()),
        is_hidden: false,
        learning_started_at: None,
        first_passed_at: None,
        observed_at: time(),
        source: source(),
    }
}

fn group(
    subjects: Vec<PreparedSubject>,
    progress: Vec<PreparedProgress>,
) -> Result<PreparedSaveGroup, InvalidSaveGroup> {
    PreparedSaveGroup::new(
        collection(),
        LearnerProgressSourceIdentity {
            learner_id: LearnerId(uuid::Uuid::from_u128(1)),
            external_account_id: "account-a".into(),
        },
        source(),
        subjects,
        progress,
    )
}

#[test]
fn group_accepts_account_only_existing_references_and_both_kinds() {
    assert!(group(vec![], vec![]).is_ok());
    assert!(group(vec![], vec![progress(99)]).is_ok());
    let grammar = PreparedSubject {
        external_id: "lesson:alpha/7".into(),
        kind: Some(MaterialKind::Grammar),
        source: source(),
    };
    let input = group(vec![grammar], vec![]).unwrap();
    assert_eq!(input.subjects()[0].external_id, "lesson:alpha/7"); // No normalization.
    assert_eq!(source().updated_at(), time()); // Nanoseconds are not a contract rejection.
}

#[test]
fn group_rejects_duplicate_identities() {
    assert!(group(vec![], vec![progress(1), progress(1)]).is_err());
    let subject = || PreparedSubject {
        external_id: "1".into(),
        kind: Some(MaterialKind::Vocabulary),
        source: source(),
    };
    assert!(group(vec![subject(), subject()], vec![]).is_err());
}

#[test]
fn source_record_validates_opaque_json_and_raw_bytes() {
    assert!(SourceRecord::new(time(), "{bad}").is_err());
    for raw in ["null", "[]", "123"] {
        assert!(SourceRecord::new(time(), raw).is_ok());
    }
    let payload = format!("{{\"x\":\"{}\"}}", "a".repeat(MAX_SOURCE_BYTES - 8));
    assert_eq!(payload.len(), MAX_SOURCE_BYTES);
    assert!(SourceRecord::new(time(), &payload).is_ok());
    assert!(SourceRecord::new(time(), &(payload + " ")).is_err());
    assert!(SourceRecord::new(time(), r#"{"x":"\u0000"}"#).is_ok()); // JSONB rejection belongs to adapter.
}

#[test]
fn group_limits_count_utf8_bytes_and_reject_oversized_input() {
    assert!(
        group(
            vec![],
            (1..=MAX_GROUP_RECORDS as u64).map(progress).collect()
        )
        .is_ok()
    );
    assert!(
        group(
            vec![],
            (1..=MAX_GROUP_RECORDS as u64 + 1).map(progress).collect()
        )
        .is_err()
    );
    let mut p = progress(1);
    let fixed = collection().input_bytes()
        + "account-a".len()
        + p.subject_external_id.len()
        + source().payload().get().len()
        + p.source.payload().get().len();
    p.progress_stage = Some("あ".repeat((MAX_REQUEST_INPUT_BYTES - fixed) / 3));
    let remaining = MAX_REQUEST_INPUT_BYTES - fixed - p.progress_stage.as_ref().unwrap().len();
    p.progress_stage
        .as_mut()
        .unwrap()
        .push_str(&"x".repeat(remaining));
    assert!(group(vec![], vec![p.clone()]).is_ok());
    p.progress_stage.as_mut().unwrap().push('x');
    assert!(group(vec![], vec![p]).is_err());
}

#[test]
fn group_limits_include_every_variable_identity_field() {
    for field in 0..6 {
        let mut collection = collection();
        let mut identity = LearnerProgressSourceIdentity {
            learner_id: LearnerId(uuid::Uuid::nil()),
            external_account_id: "a".into(),
        };
        let mut subject = PreparedSubject {
            external_id: "s".into(),
            kind: None,
            source: source(),
        };
        let mut progress = progress(1);
        let large = "x".repeat(MAX_REQUEST_INPUT_BYTES);
        match field {
            0 => collection.provider = large,
            1 => collection.key = large,
            2 => collection.name = large,
            3 => identity.external_account_id = large,
            4 => subject.external_id = large,
            _ => progress.subject_external_id = large,
        }
        let error = PreparedSaveGroup::new(
            collection,
            identity,
            source(),
            vec![subject],
            vec![progress],
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("byte limit"),
            "field {field}: {error}"
        );
    }
}
