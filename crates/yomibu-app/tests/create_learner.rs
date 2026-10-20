use std::cell::Cell;
use std::future::Future;
use std::pin::{Pin, pin};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use uuid::Uuid;
use yomibu_app::application::{CreateLearner, CreateLearnerError, prepare_learner_name};
use yomibu_core::domain::{LearnerId, LearnerName};
use yomibu_core::ports::infrastructure::storage::{LearnerWriteError, LearnerWriter};

const ID: LearnerId = LearnerId(Uuid::from_u128(7));

struct SuppliedWriter {
    deadline: Instant,
    calls: Cell<usize>,
    result: Cell<Option<Result<LearnerId, LearnerWriteError>>>,
}

impl LearnerWriter for SuppliedWriter {
    fn create_learner(
        &self,
        name: LearnerName,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<LearnerId, LearnerWriteError>> + '_>> {
        assert_eq!(name.as_str(), "Alice");
        assert_eq!(deadline, self.deadline);
        self.calls.set(self.calls.get() + 1);
        Box::pin(async { self.result.take().expect("write awaited once") })
    }
}

fn writer(result: Result<LearnerId, LearnerWriteError>) -> SuppliedWriter {
    SuppliedWriter {
        deadline: Instant::now() + Duration::from_secs(10),
        calls: Cell::new(0),
        result: Cell::new(Some(result)),
    }
}

fn ready<T>(future: impl Future<Output = T>) -> T {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("supplied writer should complete immediately"),
    }
}

#[test]
fn normalized_name_and_unchanged_deadline_reach_supplied_writer_once() {
    let name = prepare_learner_name("  Alice  ").unwrap();
    let writer = writer(Ok(ID));
    assert_eq!(
        ready(CreateLearner::new(&writer).execute(name, writer.deadline)),
        Ok(ID)
    );
    assert_eq!(writer.calls.get(), 1);
}

#[test]
fn invalid_name_and_expired_deadline_prevent_dispatch() {
    let writer = writer(Ok(ID));
    assert!(matches!(
        prepare_learner_name(" "),
        Err(CreateLearnerError::BlankName)
    ));
    assert!(matches!(
        ready(
            CreateLearner::new(&writer)
                .execute(prepare_learner_name("Alice").unwrap(), Instant::now())
        ),
        Err(CreateLearnerError::DeadlineExceeded)
    ));
    assert_eq!(writer.calls.get(), 0);
}

#[test]
fn write_failures_preserve_creation_knowledge_without_retrying() {
    for error in [
        LearnerWriteError::NotCreated("pool unavailable".into()),
        LearnerWriteError::CompletionUnknown("connection lost".into()),
    ] {
        let expected = error.to_string();
        let writer = writer(Err(error));
        let actual = ready(
            CreateLearner::new(&writer)
                .execute(prepare_learner_name("Alice").unwrap(), writer.deadline),
        )
        .unwrap_err();
        assert!(matches!(actual, CreateLearnerError::Storage(_)));
        assert_eq!(actual.to_string(), expected);
        assert_eq!(writer.calls.get(), 1);
    }
}
