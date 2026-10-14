use yomibu_app::application::sync::App;
use yomibu_core::ports::infrastructure::{LearnerDataProvider, LearningStore};
use yomibu_ext::{
    file_store::FileLearningStore,
    wanikani::{
        data::{ValidationError, WaniKaniSyncData},
        summary::{Summary, summarize},
    },
};

fn fixture() -> WaniKaniSyncData {
    let envelope: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/mixed.json")).unwrap();
    serde_json::from_value(envelope["snapshot"].clone()).unwrap()
}

struct FixtureSource;

impl LearnerDataProvider for FixtureSource {
    type Data = WaniKaniSyncData;
    type Summary = Summary;
    type Error = std::convert::Infallible;
    type ValidationError = ValidationError;

    async fn fetch(&mut self) -> Result<WaniKaniSyncData, Self::Error> {
        Ok(fixture())
    }

    fn validate_and_summarize(
        &self,
        data: &Self::Data,
    ) -> Result<Self::Summary, Self::ValidationError> {
        summarize(data)
    }
}

#[tokio::test]
async fn sync_publishes_to_file_storage() {
    let directory = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(directory.path());
    let mut app = App::new(store.clone()).with_source(FixtureSource);
    let report = app.sync().await.unwrap();
    drop(app);
    assert_eq!(report.summary, summarize(&fixture()).unwrap());
    assert_eq!(store.load_snapshot().unwrap(), fixture());
}

#[tokio::test]
async fn real_http_source_can_publish_to_file_storage() {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};
    use yomibu_ext::wanikani::Client;

    let server = MockServer::start().await;
    for (endpoint, body) in [
        (
            "user",
            include_str!("../../../tests/fixtures/wanikani/user.json"),
        ),
        (
            "assignments",
            include_str!("../../../tests/fixtures/wanikani/assignments.json"),
        ),
        (
            "review_statistics",
            include_str!("../../../tests/fixtures/wanikani/review_statistics.json"),
        ),
        (
            "subjects",
            include_str!("../../../tests/fixtures/wanikani/subjects.json"),
        ),
    ] {
        Mock::given(path(format!("/v2/{endpoint}")))
            .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
            .expect(1)
            .mount(&server)
            .await;
    }
    let directory = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(directory.path());
    let client =
        Client::with_base_url("synthetic-app-credential", &format!("{}/v2/", server.uri()))
            .unwrap();
    let mut app = App::new(store.clone()).with_source(client);
    let report = app.sync().await.unwrap();
    assert_eq!(store.load_snapshot().unwrap().subjects.len(), 4);
    assert_eq!(
        summarize(&store.load_snapshot().unwrap()).unwrap(),
        report.summary
    );
}

struct PendingSource;

impl LearnerDataProvider for PendingSource {
    type Data = WaniKaniSyncData;
    type Summary = Summary;
    type Error = std::convert::Infallible;
    type ValidationError = ValidationError;

    async fn fetch(&mut self) -> Result<WaniKaniSyncData, Self::Error> {
        std::future::pending().await
    }

    fn validate_and_summarize(
        &self,
        data: &Self::Data,
    ) -> Result<Self::Summary, Self::ValidationError> {
        summarize(data)
    }
}

fn cancellation_preserves_data_and_releases_writer(store: FileLearningStore) {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use yomibu_core::ports::infrastructure::SourceSyncWriter;

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
    assert_eq!(store.load_snapshot().unwrap(), fixture());
    drop(operation);
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    assert_eq!(
        summarize(&store.load_snapshot().unwrap()).unwrap(),
        summarize(&fixture()).unwrap()
    );
}

#[test]
fn cancelling_sync_releases_file_writer_without_replacing_data() {
    let directory = tempfile::tempdir().unwrap();
    cancellation_preserves_data_and_releases_writer(FileLearningStore::new(directory.path()));
}

struct OneResultSource(Option<Result<WaniKaniSyncData, std::io::Error>>);

impl LearnerDataProvider for OneResultSource {
    type Data = WaniKaniSyncData;
    type Summary = Summary;
    type Error = std::io::Error;
    type ValidationError = ValidationError;

    async fn fetch(&mut self) -> Result<WaniKaniSyncData, Self::Error> {
        self.0
            .take()
            .expect("source should be fetched exactly once")
    }

    fn validate_and_summarize(
        &self,
        data: &Self::Data,
    ) -> Result<Self::Summary, Self::ValidationError> {
        summarize(data)
    }
}

#[tokio::test]
async fn source_and_validation_failures_are_typed_and_preserve_data() {
    use yomibu_app::application::sync::SyncError;
    use yomibu_core::ports::infrastructure::SourceSyncWriter;
    let directory = tempfile::tempdir().unwrap();
    let store = FileLearningStore::new(directory.path());
    store.begin_sync().unwrap().replace(fixture()).unwrap();
    let source = OneResultSource(Some(Err(std::io::ErrorKind::TimedOut.into())));
    let mut app = App::new(store.clone()).with_source(source);
    assert!(
        matches!(app.sync().await, Err(SyncError::Source(error)) if error.kind() == std::io::ErrorKind::TimedOut)
    );
    assert_eq!(store.load_snapshot().unwrap(), fixture());
    assert!(store.begin_sync().is_ok());

    let mut invalid = fixture();
    invalid.sync_completed_at = invalid.sync_started_at - chrono::Duration::seconds(1);
    let mut app = App::new(store.clone()).with_source(OneResultSource(Some(Ok(invalid))));
    assert!(matches!(app.sync().await, Err(SyncError::InvalidData(_))));
    assert_eq!(store.load_snapshot().unwrap(), fixture());
    assert!(store.begin_sync().is_ok());
}

#[tokio::test]
async fn unusable_store_prevents_fetch_and_preserves_its_error() {
    use yomibu_app::application::sync::SyncError;
    use yomibu_ext::file_store::cache::CacheError;
    use yomibu_ext::file_store::cache::WriteError;
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
