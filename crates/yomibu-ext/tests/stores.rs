use yomibu_core::ports::infrastructure::{LearningStore, SourceSyncWriter};
use yomibu_ext::file_store::{
    FileLearningStore,
    cache::{CacheError, WriteError},
};
use yomibu_ext::wanikani::data::WaniKaniSyncData;

fn fixture() -> WaniKaniSyncData {
    let envelope: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/mixed.json")).unwrap();
    serde_json::from_value(envelope["snapshot"].clone()).unwrap()
}

fn stores_a_complete_version(store: &FileLearningStore) {
    assert!(matches!(
        store.load_snapshot(),
        Err(CacheError::Missing { .. })
    ));
    let writer = store.begin_sync().unwrap();
    let _: () = writer.replace(fixture()).unwrap();
    let first = store.load_snapshot().unwrap();
    assert_eq!(first, fixture());

    let mut next = fixture();
    next.learner.username = "updated username".into();
    store.begin_sync().unwrap().replace(next).unwrap();
    assert_eq!(
        store.load_snapshot().unwrap().learner.username,
        "updated username"
    );
    assert_eq!(
        first,
        fixture(),
        "a reader keeps its original coherent version"
    );
}

#[test]
fn file_store_is_lazy_and_preserves_schema_one() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("data");
    let store = FileLearningStore::new(&directory);
    assert!(!directory.exists());
    stores_a_complete_version(&store);
    let reloaded = FileLearningStore::new(&directory).load_snapshot().unwrap();
    assert_eq!(reloaded.learner.username, "updated username");
    let envelope: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("wanikani.json")).unwrap()).unwrap();
    assert_eq!(envelope["schema_version"], 1);
    assert!(envelope.get("snapshot").is_some());
    std::fs::write(directory.join("wanikani.json"), b"corrupt").unwrap();
    assert!(matches!(
        store.load_snapshot(),
        Err(CacheError::Corrupt { .. })
    ));
}

fn rejects_invalid_replacements(store: &FileLearningStore) {
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    let mut invalid = fixture();
    invalid.sync_completed_at = invalid.sync_started_at - chrono::Duration::seconds(1);
    let error = store.begin_sync().unwrap().replace(invalid).unwrap_err();
    assert!(matches!(error, WriteError::InvalidSyncData(_)));
    assert_eq!(store.load_snapshot().unwrap(), fixture());

    let mut wrong_account = fixture();
    wrong_account.learner.id = "different-account".into();
    let error = store
        .begin_sync()
        .unwrap()
        .replace(wrong_account)
        .unwrap_err();
    assert!(matches!(error, WriteError::AccountMismatch));
    assert_eq!(
        error.to_string(),
        "This cache belongs to a different WaniKani account."
    );
    assert_eq!(store.load_snapshot().unwrap(), fixture());
    // A failed publication must release the writer reservation.
    store.begin_sync().unwrap().replace(fixture()).unwrap();
}

#[test]
fn file_store_preserves_the_previous_account_and_valid_version() {
    let directory = tempfile::tempdir().unwrap();
    rejects_invalid_replacements(&FileLearningStore::new(directory.path()));
}

fn writer_reservation_keeps_reads_available(store: &FileLearningStore, other: &FileLearningStore) {
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    let writer = store.begin_sync().unwrap();
    let error = other.begin_sync().unwrap_err();
    assert!(
        matches!(error, WriteError::Locked),
        "another handle must not reserve a writer"
    );
    assert_eq!(error.to_string(), "Another sync holds the lock.");
    assert_eq!(other.load_snapshot().unwrap(), fixture());
    drop(writer);
    other.begin_sync().unwrap().replace(fixture()).unwrap();
}

#[test]
fn file_writer_reservation_is_shared_and_released_on_drop() {
    let directory = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(directory.path());
    writer_reservation_keeps_reads_available(&store, &FileLearningStore::new(directory.path()));
}

#[test]
fn normalized_reads_keep_progress_and_remain_coherent_after_replacement() {
    use yomibu_core::domain::knowledge::inventory::MappedContent;
    let dir = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(dir.path());
    assert!(store.read_learner_data().unwrap().is_none());
    let data = fixture();
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    let first = store.read_learner_data().unwrap().unwrap();
    let (_, progress) = first.records.iter().find(|(content, _)| matches!(content, MappedContent::Vocabulary(word) if word.id == "wanikani:3")).unwrap();
    assert_eq!(
        progress.unwrap().learning_started_at,
        data.assignments[2].started_at
    );
    let mut next = fixture();
    next.subjects[2].characters = "updated".into();
    store.begin_sync().unwrap().replace(next).unwrap();
    let inventory = first.into_inventory(data.sync_completed_at).unwrap();
    assert_eq!(inventory.vocabulary[0].spelling, "これ");
    let updated = store
        .read_learner_data()
        .unwrap()
        .unwrap()
        .into_inventory(data.sync_completed_at)
        .unwrap();
    assert_eq!(updated.vocabulary[0].spelling, "updated");
}
