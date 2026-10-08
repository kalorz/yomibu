use yomibu_core::{
    domain::{
        inventory::LearnerInventory,
        story::{PracticeTargets, StoryError, StoryRequest},
    },
    pipeline::selection::select_ranked,
};

#[test]
fn ranked_selection_rejects_limits_that_cannot_hold_targets_or_exceed_the_bound() {
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
        let result = select_ranked(
            &request,
            inventory
                .vocabulary
                .iter()
                .map(|word| (word, None))
                .collect(),
            limit,
            |_| "support",
            "test",
            None,
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
