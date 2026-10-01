use std::sync::Arc;
use yomibu::{
    adapters::stores::{FileLearningStore, InMemoryLearningStore},
    domain::WaniKaniSyncData,
    ports::{LearningStore, Persistence, SourceSyncWriter},
};

fn fixture() -> WaniKaniSyncData {
    let envelope: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/mixed.json")).unwrap();
    serde_json::from_value(envelope["snapshot"].clone()).unwrap()
}

fn stores_a_complete_version(store: &impl LearningStore, persistence: Persistence) {
    assert!(store.load().is_err());
    let writer = store.begin_sync().unwrap();
    assert_eq!(writer.replace(fixture()).unwrap(), persistence);
    let first = store.load().unwrap();
    assert_eq!(*first, fixture());

    let mut next = fixture();
    next.learner.username = "updated username".into();
    store.begin_sync().unwrap().replace(next).unwrap();
    assert_eq!(store.load().unwrap().learner.username, "updated username");
    assert_eq!(
        *first,
        fixture(),
        "a reader keeps its original coherent version"
    );
}

#[test]
fn in_memory_store_retains_data_for_its_handles_without_deep_copying_reads() {
    let store = InMemoryLearningStore::new();
    stores_a_complete_version(&store, Persistence::Volatile);
    let other_handle = store.clone();
    assert!(Arc::ptr_eq(
        &store.load().unwrap(),
        &other_handle.load().unwrap()
    ));
    drop(store);
    assert_eq!(
        other_handle.load().unwrap().learner.username,
        "updated username"
    );
    assert!(InMemoryLearningStore::new().load().is_err());
}

#[test]
fn file_store_is_lazy_and_preserves_schema_one() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("data");
    let store = FileLearningStore::new(&directory);
    assert!(!directory.exists());
    stores_a_complete_version(&store, Persistence::Durable);
    let reloaded = FileLearningStore::new(&directory).load().unwrap();
    assert_eq!(reloaded.learner.username, "updated username");
    let envelope: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("wanikani.json")).unwrap()).unwrap();
    assert_eq!(envelope["schema_version"], 1);
    assert!(envelope.get("snapshot").is_some());
}

fn rejects_invalid_replacements(store: &impl LearningStore) {
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    let mut invalid = fixture();
    invalid.sync_completed_at = invalid.sync_started_at - chrono::Duration::seconds(1);
    assert!(store.begin_sync().unwrap().replace(invalid).is_err());
    assert_eq!(*store.load().unwrap(), fixture());

    let mut wrong_account = fixture();
    wrong_account.learner.id = "different-account".into();
    assert!(store.begin_sync().unwrap().replace(wrong_account).is_err());
    assert_eq!(*store.load().unwrap(), fixture());
    // A failed publication must release the writer reservation.
    store.begin_sync().unwrap().replace(fixture()).unwrap();
}

#[test]
fn in_memory_store_preserves_the_previous_account_and_valid_version() {
    rejects_invalid_replacements(&InMemoryLearningStore::new());
}

#[test]
fn file_store_preserves_the_previous_account_and_valid_version() {
    let directory = tempfile::tempdir().unwrap();
    rejects_invalid_replacements(&FileLearningStore::new(directory.path()));
}

fn writer_reservation_keeps_reads_available(
    store: &impl LearningStore,
    other: &impl LearningStore,
) {
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    let writer = store.begin_sync().unwrap();
    assert!(
        other.begin_sync().is_err(),
        "another handle must not reserve a writer"
    );
    assert_eq!(*other.load().unwrap(), fixture());
    drop(writer);
    other.begin_sync().unwrap().replace(fixture()).unwrap();
}

#[test]
fn in_memory_writer_reservation_is_shared_and_released_on_drop() {
    let store = InMemoryLearningStore::new();
    writer_reservation_keeps_reads_available(&store, &store.clone());
}

#[test]
fn file_writer_reservation_is_shared_and_released_on_drop() {
    let directory = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(directory.path());
    writer_reservation_keeps_reads_available(&store, &FileLearningStore::new(directory.path()));
}
