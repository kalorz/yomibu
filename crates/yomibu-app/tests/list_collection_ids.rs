use std::cell::Cell;
use std::future::{Future, poll_fn};
use std::pin::{Pin, pin};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use uuid::Uuid;
use yomibu_app::application::list_collection_ids;
use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

const LEARNER: LearnerId = LearnerId(Uuid::from_u128(1));

struct SuppliedStorage {
    expected_deadline: Instant,
    ready: Cell<bool>,
    calls: Cell<usize>,
    result: Cell<Option<Result<Vec<CollectionId>, StorageError>>>,
}

impl LearnerCollectionQuery for SuppliedStorage {
    fn collection_ids_for_learner(
        &self,
        learner_id: LearnerId,
        deadline: Instant,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<CollectionId>, StorageError>> + '_>> {
        assert_eq!(learner_id, LEARNER);
        assert_eq!(deadline, self.expected_deadline);
        self.calls.set(self.calls.get() + 1);
        Box::pin(poll_fn(|_| {
            if self.ready.get() {
                Poll::Ready(self.result.take().expect("storage awaited once"))
            } else {
                Poll::Pending
            }
        }))
    }
}

fn poll<T>(future: Pin<&mut impl Future<Output = T>>) -> Poll<T> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

fn storage(result: Result<Vec<CollectionId>, StorageError>) -> SuppliedStorage {
    SuppliedStorage {
        expected_deadline: Instant::now() + Duration::from_secs(10),
        ready: Cell::new(true),
        calls: Cell::new(0),
        result: Cell::new(Some(result)),
    }
}

#[test]
fn forwards_learner_and_deadline_once_and_awaits_supplied_ids() {
    let ids = vec![
        CollectionId(Uuid::from_u128(9)),
        CollectionId(Uuid::from_u128(2)),
    ];
    let storage = storage(Ok(ids.clone()));
    storage.ready.set(false);
    let mut call = pin!(list_collection_ids(
        LEARNER,
        &storage,
        storage.expected_deadline,
    ));
    assert!(poll(call.as_mut()).is_pending());
    storage.ready.set(true);
    assert_eq!(poll(call.as_mut()), Poll::Ready(Ok(ids)));
    assert_eq!(storage.calls.get(), 1);
}

#[test]
fn preserves_supplied_empty_result() {
    let storage = storage(Ok(vec![]));
    assert_eq!(
        poll(pin!(list_collection_ids(
            LEARNER,
            &storage,
            storage.expected_deadline,
        ))),
        Poll::Ready(Ok(vec![]))
    );
    assert_eq!(storage.calls.get(), 1);
}

#[test]
fn preserves_storage_failure() {
    let storage = storage(Err(StorageError("storage unavailable".into())));
    assert_eq!(
        poll(pin!(list_collection_ids(
            LEARNER,
            &storage,
            storage.expected_deadline,
        ))),
        Poll::Ready(Err(StorageError("storage unavailable".into())))
    );
}

#[test]
fn expired_deadline_prevents_dispatch() {
    let storage = storage(Ok(vec![]));
    assert_eq!(
        poll(pin!(list_collection_ids(LEARNER, &storage, Instant::now()))),
        Poll::Ready(Err(StorageError(
            "collection lookup deadline exceeded".into()
        )))
    );
    assert_eq!(storage.calls.get(), 0);
}
