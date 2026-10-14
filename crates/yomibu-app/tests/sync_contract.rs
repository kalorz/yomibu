use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
};
use yomibu_app::application::sync::{App, SyncError};
use yomibu_core::ports::infrastructure::{LearnerDataProvider, LearningStore, SourceSyncWriter};

#[derive(Default)]
struct State {
    published: Vec<u8>,
    steps: Vec<&'static str>,
}

struct TestSource {
    data: Vec<u8>,
    state: Arc<Mutex<State>>,
}

#[derive(Debug, thiserror::Error)]
#[error("empty snapshot")]
struct EmptyData;

impl LearnerDataProvider for TestSource {
    type Data = Vec<u8>;
    type Summary = usize;
    type Error = Infallible;
    type ValidationError = EmptyData;

    async fn fetch(&mut self) -> Result<Self::Data, Self::Error> {
        self.state.lock().unwrap().steps.push("fetch");
        Ok(std::mem::take(&mut self.data))
    }

    fn validate_and_summarize(
        &self,
        data: &Self::Data,
    ) -> Result<Self::Summary, Self::ValidationError> {
        self.state.lock().unwrap().steps.push("prepare");
        if data.is_empty() {
            Err(EmptyData)
        } else {
            Ok(data.len())
        }
    }
}

struct TestStore(Arc<Mutex<State>>);
struct TestWriter(Arc<Mutex<State>>);

impl LearningStore for TestStore {
    type WriteError = Infallible;
    type Writer = TestWriter;

    fn begin_sync(&self) -> Result<Self::Writer, Self::WriteError> {
        self.0.lock().unwrap().steps.push("reserve");
        Ok(TestWriter(Arc::clone(&self.0)))
    }
}

impl SourceSyncWriter for TestWriter {
    type Data = Vec<u8>;
    type Error = Infallible;

    fn replace(self, data: Self::Data) -> Result<(), Self::Error> {
        let mut state = self.0.lock().unwrap();
        state.steps.push("publish");
        state.published = data;
        Ok(())
    }
}

#[tokio::test]
async fn sync_prepares_a_distinct_summary_before_transferring_data_to_the_writer() {
    let state = Arc::new(Mutex::new(State::default()));
    let mut app = App::new(TestStore(Arc::clone(&state))).with_source(TestSource {
        data: vec![3, 1, 4],
        state: Arc::clone(&state),
    });
    let report = app.sync().await.unwrap();
    assert_eq!(report.summary, 3);
    {
        let mut state = state.lock().unwrap();
        assert_eq!(state.published, [3, 1, 4]);
        assert_eq!(state.steps, ["reserve", "fetch", "prepare", "publish"]);
        state.steps.clear();
    }

    assert!(matches!(
        app.sync().await,
        Err(SyncError::InvalidData(EmptyData))
    ));
    let state = state.lock().unwrap();
    assert_eq!(state.published, [3, 1, 4]);
    assert_eq!(state.steps, ["reserve", "fetch", "prepare"]);
}
