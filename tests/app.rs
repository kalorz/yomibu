use yomibu::{
    App,
    adapters::stores::{FileLearningStore, InMemoryLearningStore},
    domain::WaniKaniSyncData,
    ports::{LearningSource, LearningStore, Persistence},
};

fn fixture() -> WaniKaniSyncData {
    let envelope: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/mixed.json")).unwrap();
    serde_json::from_value(envelope["snapshot"].clone()).unwrap()
}

struct FixtureSource;

impl LearningSource for FixtureSource {
    type Error = std::convert::Infallible;

    async fn fetch(&mut self) -> Result<WaniKaniSyncData, Self::Error> {
        Ok(fixture())
    }
}

async fn sync_and_status(store: impl LearningStore, persistence: Persistence) {
    let mut app = App::new(store).with_source(FixtureSource);
    assert!(app.status().is_err());
    let report = app.sync().await.unwrap();
    assert_eq!(report.persistence, persistence);
    assert_eq!(report.summary, fixture().summarize().unwrap());
    let summary = app.status().unwrap();
    drop(app);
    assert_eq!(
        summary, report.summary,
        "results outlive application and input data"
    );
}

#[tokio::test]
async fn same_use_case_runs_with_memory_and_file_storage() {
    sync_and_status(InMemoryLearningStore::new(), Persistence::Volatile).await;
    let directory = tempfile::tempdir().unwrap();
    sync_and_status(
        FileLearningStore::new(directory.path()),
        Persistence::Durable,
    )
    .await;
    // Offline inspection does not require a source, token, or async runtime.
    let summary = App::new(FileLearningStore::new(directory.path()))
        .status()
        .unwrap();
    assert_eq!(summary, fixture().summarize().unwrap());
}

#[tokio::test]
async fn real_http_source_can_publish_to_memory() {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};
    use yomibu::adapters::sources::wanikani::Client;

    let server = MockServer::start().await;
    for (endpoint, body) in [
        ("user", include_str!("fixtures/wanikani/user.json")),
        (
            "assignments",
            include_str!("fixtures/wanikani/assignments.json"),
        ),
        (
            "review_statistics",
            include_str!("fixtures/wanikani/review_statistics.json"),
        ),
        ("subjects", include_str!("fixtures/wanikani/subjects.json")),
    ] {
        Mock::given(path(format!("/v2/{endpoint}")))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .expect(1)
            .mount(&server)
            .await;
    }
    let store = InMemoryLearningStore::new();
    let client =
        Client::with_base_url("synthetic-app-credential", &format!("{}/v2/", server.uri()))
            .unwrap();
    let mut app = App::new(store.clone()).with_source(client);
    let report = app.sync().await.unwrap();
    assert_eq!(report.persistence, Persistence::Volatile);
    assert_eq!(store.load().unwrap().subjects.len(), 4);
    assert_eq!(app.status().unwrap(), report.summary);
}

struct PendingSource;

impl LearningSource for PendingSource {
    type Error = std::convert::Infallible;

    async fn fetch(&mut self) -> Result<WaniKaniSyncData, Self::Error> {
        std::future::pending().await
    }
}

fn cancellation_preserves_data_and_releases_writer<Store>(store: Store)
where
    Store: LearningStore + Clone + Send,
    Store::Writer: Send,
{
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use yomibu::ports::SourceSyncWriter;

    store.begin_sync().unwrap().replace(fixture()).unwrap();
    let mut app = App::new(store.clone()).with_source(PendingSource);
    fn require_send<T: Send>(value: T) -> T {
        value
    }
    let mut operation = Box::pin(require_send(app.sync()));
    assert!(matches!(
        operation
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    assert!(store.begin_sync().is_err());
    assert_eq!(*store.load().unwrap(), fixture());
    drop(operation);
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    assert_eq!(app.status().unwrap(), fixture().summarize().unwrap());
}

#[test]
fn cancelling_sync_releases_memory_and_file_writers_without_replacing_data() {
    cancellation_preserves_data_and_releases_writer(InMemoryLearningStore::new());
    let directory = tempfile::tempdir().unwrap();
    cancellation_preserves_data_and_releases_writer(FileLearningStore::new(directory.path()));
}

struct OneResultSource(Option<Result<WaniKaniSyncData, std::io::Error>>);

impl LearningSource for OneResultSource {
    type Error = std::io::Error;

    async fn fetch(&mut self) -> Result<WaniKaniSyncData, Self::Error> {
        self.0
            .take()
            .expect("source should be fetched exactly once")
    }
}

#[tokio::test]
async fn source_and_validation_failures_are_typed_and_preserve_data() {
    use yomibu::{app::SyncError, ports::SourceSyncWriter};
    let store = InMemoryLearningStore::new();
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    let source = OneResultSource(Some(Err(std::io::ErrorKind::TimedOut.into())));
    let mut app = App::new(store.clone()).with_source(source);
    assert!(
        matches!(app.sync().await, Err(SyncError::Source(error)) if error.kind() == std::io::ErrorKind::TimedOut)
    );
    assert_eq!(*store.load().unwrap(), fixture());
    assert!(store.begin_sync().is_ok());

    let mut invalid = fixture();
    invalid.sync_completed_at = invalid.sync_started_at - chrono::Duration::seconds(1);
    let mut app = App::new(store.clone()).with_source(OneResultSource(Some(Ok(invalid))));
    assert!(matches!(app.sync().await, Err(SyncError::InvalidData(_))));
    assert_eq!(*store.load().unwrap(), fixture());
    assert!(store.begin_sync().is_ok());
}

#[tokio::test]
async fn unusable_store_prevents_fetch_and_preserves_its_error() {
    use yomibu::{
        app::SyncError,
        cache::{CacheError, WriteError},
    };
    let directory = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(directory.path());
    let writer = store.begin_sync().unwrap();
    let mut app = App::new(store).with_source(OneResultSource(None));
    assert!(matches!(
        app.sync().await,
        Err(SyncError::Write(WriteError::Locked))
    ));
    drop(writer);
    std::fs::write(directory.path().join("wanikani.json"), b"broken cache").unwrap();
    assert!(matches!(
        app.sync().await,
        Err(SyncError::Write(WriteError::ExistingCache(
            CacheError::Corrupt { .. }
        )))
    ));
    assert_eq!(
        std::fs::read(directory.path().join("wanikani.json")).unwrap(),
        b"broken cache"
    );
}
