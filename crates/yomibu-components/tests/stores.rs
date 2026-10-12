use yomibu_components::file_learning_store::FileLearningStore;
use yomibu_core::{
    capabilities::{LearningStore, Persistence, SourceSyncWriter},
    domain::source::WaniKaniSyncData,
};

fn fixture() -> WaniKaniSyncData {
    let envelope: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/mixed.json")).unwrap();
    serde_json::from_value(envelope["snapshot"].clone()).unwrap()
}

fn stores_a_complete_version(store: &impl LearningStore, persistence: Persistence) {
    assert!(store.is_missing(&store.load().unwrap_err()));
    let writer = store.begin_sync().unwrap();
    assert_eq!(writer.replace(fixture().into()).unwrap(), persistence);
    let first = store.load().unwrap();
    assert_eq!(*first, fixture());

    let mut next = fixture();
    next.learner.username = "updated username".into();
    store.begin_sync().unwrap().replace(next.into()).unwrap();
    assert_eq!(store.load().unwrap().learner.username, "updated username");
    assert_eq!(
        *first,
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
    stores_a_complete_version(&store, Persistence::Durable);
    let reloaded = FileLearningStore::new(&directory).load().unwrap();
    assert_eq!(reloaded.learner.username, "updated username");
    let envelope: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("wanikani.json")).unwrap()).unwrap();
    assert_eq!(envelope["schema_version"], 1);
    assert!(envelope.get("snapshot").is_some());
    std::fs::write(directory.join("wanikani.json"), b"corrupt").unwrap();
    assert!(!store.is_missing(&store.load().unwrap_err()));
}

fn rejects_invalid_replacements(store: &impl LearningStore) {
    store
        .begin_sync()
        .unwrap()
        .replace(fixture().into())
        .unwrap();
    let mut invalid = fixture();
    invalid.sync_completed_at = invalid.sync_started_at - chrono::Duration::seconds(1);
    let error = store
        .begin_sync()
        .unwrap()
        .replace(invalid.into())
        .unwrap_err();
    assert!(!store.is_locked(&error));
    assert_eq!(*store.load().unwrap(), fixture());

    let mut wrong_account = fixture();
    wrong_account.learner.id = "different-account".into();
    let error = store
        .begin_sync()
        .unwrap()
        .replace(wrong_account.into())
        .unwrap_err();
    assert!(!store.is_locked(&error));
    assert_eq!(*store.load().unwrap(), fixture());
    // A failed publication must release the writer reservation.
    store
        .begin_sync()
        .unwrap()
        .replace(fixture().into())
        .unwrap();
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
    store
        .begin_sync()
        .unwrap()
        .replace(fixture().into())
        .unwrap();
    let writer = store.begin_sync().unwrap();
    assert!(
        other.is_locked(&other.begin_sync().err().unwrap()),
        "another handle must not reserve a writer"
    );
    assert_eq!(*other.load().unwrap(), fixture());
    drop(writer);
    other
        .begin_sync()
        .unwrap()
        .replace(fixture().into())
        .unwrap();
}

#[test]
fn file_writer_reservation_is_shared_and_released_on_drop() {
    let directory = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(directory.path());
    writer_reservation_keeps_reads_available(&store, &FileLearningStore::new(directory.path()));
}
