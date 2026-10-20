use chrono::{DateTime, Utc};
use sqlx::PgPool;
use yomibu_core::domain::LearnerId;
use yomibu_core::ports::infrastructure::storage::{
    PreparedProgress, PreparedSaveGroup, PreparedSubject, SourceRecord,
};

pub(crate) fn at(t: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000 + t, 0).unwrap()
}
pub(crate) fn source(t: i64, marker: &str) -> SourceRecord {
    SourceRecord::new(at(t), &serde_json::json!({"marker":marker,"source_updated_at":"provider-owned-field","nested":{"all":[1,"食べる",true]},"access":{"level":10,"expires_at":null}}).to_string()).unwrap()
}
pub(crate) fn subject(id: u64, t: i64) -> PreparedSubject {
    let object = ["vocabulary", "kana_vocabulary", "kanji", "radical"][(id as usize - 1) % 4];
    PreparedSubject { external_id: id.to_string(), kind: yomibu_ext::wanikani::subject_kind(object).unwrap(), source: SourceRecord::new(at(t), &serde_json::json!({"id":id,"object":object,"data":{"characters":"食べる","meanings":[{"meaning":"to eat","primary":true}],"readings":[{"reading":"たべる"}]},"version":t}).to_string()).unwrap() }
}
pub(crate) fn progress(id: u64, t: i64, observed: i64) -> PreparedProgress {
    PreparedProgress {
        subject_external_id: id.to_string(),
        progress_stage: Some("5".into()),
        is_hidden: false,
        learning_started_at: Some(at(1)),
        first_passed_at: Some(at(2)),
        observed_at: at(observed),
        source: source(t, "progress"),
    }
}
pub(crate) fn group(
    learner: u128,
    account: &str,
    t: i64,
    subjects: Vec<PreparedSubject>,
    progress: Vec<PreparedProgress>,
) -> PreparedSaveGroup {
    yomibu_ext::wanikani::prepare_group(
        LearnerId(uuid::Uuid::from_u128(learner)),
        account.into(),
        source(t, account),
        subjects,
        progress,
    )
    .unwrap()
}
pub(crate) async fn seed(pool: &PgPool) {
    sqlx::raw_sql("INSERT INTO learners(id,name) VALUES ('00000000-0000-0000-0000-000000000001','Alice'),('00000000-0000-0000-0000-000000000002','Bob');").execute(pool).await.unwrap();
}
