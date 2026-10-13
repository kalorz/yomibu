#[path = "../../../tests/support/story_inventory.rs"]
mod story_inventory;

use yomibu_core::{
    domain::generation::finalization::select_target_first,
    domain::generation::{PracticeTargets, SelectionInput, StoryError, StoryRequest},
};

#[test]
fn target_first_selection_rejects_limits_that_cannot_hold_targets_or_exceed_the_bound() {
    let inventory = story_inventory::inventory();
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
