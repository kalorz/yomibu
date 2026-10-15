use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use uuid::Uuid;
use yomibu_app::application::list_collection_ids;
use yomibu_core::domain::identity::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

struct TestStorage {
    received: Cell<Option<(LearnerId, Instant, *const AtomicBool)>>,
    result: Result<Vec<CollectionId>, StorageError>,
}

impl LearnerCollectionQuery for TestStorage {
    fn collection_ids_for_learner(
        &self,
        learner_id: LearnerId,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<CollectionId>, StorageError> {
        self.received.set(Some((learner_id, deadline, cancelled)));
        self.result.clone()
    }
}

fn future_deadline() -> Instant {
    Instant::now() + Duration::from_secs(60)
}

#[test]
fn forwards_inputs_and_preserves_supplied_results_through_trait_object() {
    let learner_id = LearnerId(Uuid::from_bytes([1; 16]));
    for supplied in [
        Ok(vec![
            CollectionId(Uuid::from_bytes([3; 16])),
            CollectionId(Uuid::from_bytes([2; 16])),
        ]),
        Ok(vec![]),
        Err(StorageError {
            message: "Collection storage is unavailable".into(),
        }),
    ] {
        let deadline = future_deadline();
        let cancelled = AtomicBool::new(false);
        let storage = TestStorage {
            received: Cell::new(None),
            result: supplied.clone(),
        };
        let selected_storage: &dyn LearnerCollectionQuery = &storage;

        let result = list_collection_ids(learner_id, selected_storage, deadline, &cancelled);

        assert_eq!(
            storage.received.get(),
            Some((learner_id, deadline, std::ptr::from_ref(&cancelled)))
        );
        assert_eq!(result, supplied);
    }
}

#[test]
fn rejects_duplicate_collection_ids() {
    let storage = TestStorage {
        received: Cell::new(None),
        result: Ok(vec![
            CollectionId(Uuid::from_bytes([2; 16])),
            CollectionId(Uuid::from_bytes([3; 16])),
            CollectionId(Uuid::from_bytes([2; 16])),
        ]),
    };

    assert_eq!(
        list_collection_ids(
            LearnerId(Uuid::from_bytes([1; 16])),
            &storage,
            future_deadline(),
            &AtomicBool::new(false),
        ),
        Err(StorageError {
            message: "Storage returned duplicate collection IDs".into(),
        })
    );
}

#[test]
fn rejects_cancelled_or_expired_calls_before_storage() {
    for (cancelled, deadline, message) in [
        (true, future_deadline(), "Collection query was cancelled"),
        (false, Instant::now(), "Collection query deadline expired"),
    ] {
        let storage = TestStorage {
            received: Cell::new(None),
            result: Ok(vec![]),
        };

        assert_eq!(
            list_collection_ids(
                LearnerId(Uuid::from_bytes([1; 16])),
                &storage,
                deadline,
                &AtomicBool::new(cancelled),
            ),
            Err(StorageError {
                message: message.into(),
            })
        );
        assert_eq!(storage.received.get(), None);
    }
}

#[test]
fn rejects_cancelled_success_but_preserves_storage_failure() {
    struct CancellingStorage(Result<Vec<CollectionId>, StorageError>);

    impl LearnerCollectionQuery for CancellingStorage {
        fn collection_ids_for_learner(
            &self,
            _: LearnerId,
            _: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Vec<CollectionId>, StorageError> {
            cancelled.store(true, Ordering::Relaxed);
            self.0.clone()
        }
    }

    let storage_error = StorageError {
        message: "Collection storage is unavailable".into(),
    };
    for (supplied, expected) in [
        (
            Ok(vec![]),
            Err(StorageError {
                message: "Collection query was cancelled".into(),
            }),
        ),
        (Err(storage_error.clone()), Err(storage_error)),
    ] {
        assert_eq!(
            list_collection_ids(
                LearnerId(Uuid::from_bytes([1; 16])),
                &CancellingStorage(supplied),
                future_deadline(),
                &AtomicBool::new(false),
            ),
            expected
        );
    }
}

#[test]
fn rejects_success_returned_after_deadline() {
    struct LateStorage(Cell<bool>);

    impl LearnerCollectionQuery for LateStorage {
        fn collection_ids_for_learner(
            &self,
            _: LearnerId,
            deadline: Instant,
            _: &AtomicBool,
        ) -> Result<Vec<CollectionId>, StorageError> {
            self.0.set(true);
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            Ok(vec![])
        }
    }

    let storage = LateStorage(Cell::new(false));
    assert_eq!(
        list_collection_ids(
            LearnerId(Uuid::from_bytes([1; 16])),
            &storage,
            Instant::now() + Duration::from_secs(1),
            &AtomicBool::new(false),
        ),
        Err(StorageError {
            message: "Collection query deadline expired".into(),
        })
    );
    assert!(storage.0.get());
}
