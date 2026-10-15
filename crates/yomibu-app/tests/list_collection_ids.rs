use std::cell::Cell;

use uuid::Uuid;
use yomibu_app::application::list_collection_ids;
use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_core::ports::infrastructure::{LearnerCollectionQuery, StorageError};

struct SuppliedStorage {
    expected_learner: LearnerId,
    result: Cell<Option<Result<Vec<CollectionId>, StorageError>>>,
}

impl LearnerCollectionQuery for SuppliedStorage {
    fn collection_ids_for_learner(
        &self,
        learner_id: LearnerId,
    ) -> Result<Vec<CollectionId>, StorageError> {
        assert_eq!(learner_id, self.expected_learner);
        self.result.take().expect("storage called once")
    }
}

#[test]
fn forwards_learner_id_and_returns_supplied_ids() {
    let learner_id = LearnerId(Uuid::from_u128(1));
    let ids = vec![
        CollectionId(Uuid::from_u128(9)),
        CollectionId(Uuid::from_u128(2)),
    ];
    let storage = SuppliedStorage {
        expected_learner: learner_id,
        result: Cell::new(Some(Ok(ids.clone()))),
    };

    assert_eq!(list_collection_ids(learner_id, &storage), Ok(ids));
}

#[test]
fn returns_empty_results_when_storage_finds_no_associations() {
    let learner_id = LearnerId(Uuid::from_u128(3));
    let storage = SuppliedStorage {
        expected_learner: learner_id,
        result: Cell::new(Some(Ok(vec![]))),
    };

    assert_eq!(list_collection_ids(learner_id, &storage), Ok(vec![]));
    assert!(storage.result.into_inner().is_none());
}

#[test]
fn preserves_storage_failure() {
    let learner_id = LearnerId(Uuid::from_u128(4));
    let storage = SuppliedStorage {
        expected_learner: learner_id,
        result: Cell::new(Some(Err(StorageError("storage unavailable".into())))),
    };

    assert_eq!(
        list_collection_ids(learner_id, &storage),
        Err(StorageError("storage unavailable".into())),
    );
}
