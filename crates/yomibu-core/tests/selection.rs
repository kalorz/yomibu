use yomibu_core::{
    domain::{
        inventory::LearnerInventory,
        story::{PracticeTargets, SelectionInput, StoryError, StoryRequest},
    },
    pipeline::selection::select_target_first,
};

#[test]
fn target_first_selection_rejects_limits_that_cannot_hold_targets_or_exceed_the_bound() {
    let inventory = LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap();
    let request = StoryRequest {
        version: 1,
        topic: None,
        targets: PracticeTargets {
            vocabulary: vec!["cat".into(), "dog".into()],
            grammar: vec![],
        },
    };
    for limit in [0, 1, 17, usize::MAX] {
        let result = select_target_first(
            SelectionInput {
                inventory: &inventory,
                request: &request,
                limit,
            },
            &[],
            "test",
        );
        assert!(
            matches!(
                result,
                Err(StoryError::Invalid(
                    "selection limit must include all targets and be at most 16"
                ))
            ),
            "limit {limit}: {result:?}"
        );
    }
}
