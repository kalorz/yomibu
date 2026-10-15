use std::cell::{Cell, RefCell};

use uuid::Uuid;
use yomibu_app::application::list_collection_ids;
use yomibu_core::domain::identity::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

struct TestStorage {
    received_learner_id: Cell<Option<LearnerId>>,
    result: RefCell<Option<Result<Vec<CollectionId>, StorageError>>>,
}

impl LearnerCollectionQuery for TestStorage {
    fn collection_ids_for_learner(
        &self,
        learner_id: LearnerId,
    ) -> Result<Vec<CollectionId>, StorageError> {
        self.received_learner_id.set(Some(learner_id));
        self.result.borrow_mut().take().expect("one storage call")
    }
}

#[test]
fn forwards_learner_id_and_returns_supplied_collection_ids() {
    let learner_id = LearnerId(Uuid::from_bytes([1; 16]));
    let collection_ids = vec![
        CollectionId(Uuid::from_bytes([3; 16])),
        CollectionId(Uuid::from_bytes([2; 16])),
    ];
    let storage = TestStorage {
        received_learner_id: Cell::new(None),
        result: RefCell::new(Some(Ok(collection_ids.clone()))),
    };

    let result = list_collection_ids(learner_id, &storage);

    assert_eq!(storage.received_learner_id.get(), Some(learner_id));
    assert_eq!(result, Ok(collection_ids));
}

#[test]
fn returns_successful_empty_storage_results() {
    let storage = TestStorage {
        received_learner_id: Cell::new(None),
        result: RefCell::new(Some(Ok(vec![]))),
    };

    assert_eq!(
        list_collection_ids(LearnerId(Uuid::from_bytes([1; 16])), &storage),
        Ok(vec![])
    );
}

#[test]
fn preserves_storage_failure() {
    let storage = TestStorage {
        received_learner_id: Cell::new(None),
        result: RefCell::new(Some(Err(StorageError {
            message: "Collection storage is unavailable".into(),
        }))),
    };

    assert_eq!(
        list_collection_ids(LearnerId(Uuid::from_bytes([1; 16])), &storage),
        Err(StorageError {
            message: "Collection storage is unavailable".into(),
        })
    );
}
