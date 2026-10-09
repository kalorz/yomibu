use yomibu_core::{
    capabilities::SelectionStep,
    domain::{
        inventory::{InventoryWord, LearnerInventory},
        story::{
            PracticeTargets, SelectionCandidates, SelectionError, SelectionInput,
            StoryAssessmentInputs, StoryError, StoryRequest,
        },
    },
    pipeline::selection::select_target_first,
};

fn inventory() -> LearnerInventory {
    LearnerInventory::from_manual(
        serde_json::from_str(include_str!("../../../tests/fixtures/story/inventory.json")).unwrap(),
    )
    .unwrap()
}
fn request() -> StoryRequest {
    StoryRequest {
        version: 1,
        topic: None,
        targets: PracticeTargets {
            vocabulary: vec!["cat".into()],
            grammar: vec!["polite".into()],
        },
    }
}

struct Reverse;
impl SelectionStep for Reverse {
    fn apply<'a>(
        &self,
        _: SelectionInput<'a>,
        mut candidates: SelectionCandidates<'a>,
    ) -> Result<SelectionCandidates<'a>, StoryError> {
        candidates.entries.reverse();
        for entry in &mut candidates.entries {
            entry.reason = "external_reverse";
        }
        Ok(candidates)
    }
}

#[test]
fn an_external_step_composes_with_target_first_finalization_and_full_inventory_assessment() {
    let inventory = inventory();
    let request = request();
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 3,
    };
    let selection = select_target_first(input, &[&Reverse], "test-reverse-v1").unwrap();
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["cat", "walk", "dog"]
    );
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.reason)
            .collect::<Vec<_>>(),
        ["practice_target", "external_reverse", "external_reverse"]
    );
    assert_eq!(selection.selector_revision, "test-reverse-v1");
    assert_eq!(selection.grammar_targets, ["polite"]);
    assert_eq!(inventory.vocabulary.len(), 4);
    let assessment = StoryAssessmentInputs::new(&inventory, &request, &selection).unwrap();
    assert!(std::ptr::eq(assessment.inventory(), &inventory));
    assert_eq!(assessment.inventory().vocabulary.len(), 4);
    assert_eq!(assessment.selected_vocabulary_ids(), ["cat", "walk", "dog"]);
    let twice =
        select_target_first(input, &[&Reverse, &Reverse], "test-double-reverse-v1").unwrap();
    assert_eq!(
        twice
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["cat", "sleep", "dog"]
    );
}

#[test]
fn empty_composition_keeps_inventory_order_without_claiming_sampling() {
    let inventory = inventory();
    let request = request();
    let selection = select_target_first(
        SelectionInput {
            inventory: &inventory,
            request: &request,
            limit: 3,
        },
        &[],
        "test-inventory-order-v1",
    )
    .unwrap();
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| (entry.word.id.as_str(), entry.reason, entry.score))
            .collect::<Vec<_>>(),
        [
            ("cat", "practice_target", None),
            ("sleep", "inventory_entry", None),
            ("dog", "inventory_entry", None),
        ]
    );
}

struct MislabelTargets;
impl SelectionStep for MislabelTargets {
    fn apply<'a>(
        &self,
        _: SelectionInput<'a>,
        mut candidates: SelectionCandidates<'a>,
    ) -> Result<SelectionCandidates<'a>, StoryError> {
        candidates.entries.reverse();
        for entry in &mut candidates.entries {
            entry.reason = if entry.word.id == "cat" {
                "external_reason"
            } else {
                "practice_target"
            };
        }
        Ok(candidates)
    }
}

#[test]
fn finalization_assigns_target_reasons_only_from_the_request() {
    let inventory = inventory();
    for (targets, expected) in [
        (
            vec![],
            [
                ("walk", "inventory_entry"),
                ("dog", "inventory_entry"),
                ("cat", "external_reason"),
            ],
        ),
        (
            vec!["cat".into(), "sleep".into()],
            [
                ("cat", "practice_target"),
                ("sleep", "practice_target"),
                ("walk", "inventory_entry"),
            ],
        ),
    ] {
        let mut request = request();
        request.targets.vocabulary = targets;
        let selection = select_target_first(
            SelectionInput {
                inventory: &inventory,
                request: &request,
                limit: 3,
            },
            &[&MislabelTargets],
            "test-mislabel-targets-v1",
        )
        .unwrap();
        assert_eq!(
            selection
                .selected
                .iter()
                .map(|entry| (entry.word.id.as_str(), entry.reason))
                .collect::<Vec<_>>(),
            expected
        );
    }
}

enum Malformed {
    RemoveTarget,
    Duplicate,
    Foreign(&'static InventoryWord),
    Nonfinite,
    Empty,
}
impl SelectionStep for Malformed {
    fn apply<'a>(
        &self,
        _: SelectionInput<'a>,
        mut candidates: SelectionCandidates<'a>,
    ) -> Result<SelectionCandidates<'a>, StoryError> {
        match self {
            Self::RemoveTarget => candidates.entries.retain(|entry| entry.word.id != "cat"),
            Self::Duplicate => candidates.entries[0].word = candidates.entries[1].word,
            Self::Foreign(word) => candidates.entries[0].word = word,
            Self::Nonfinite => candidates.entries[0].score = Some(f64::NAN),
            Self::Empty => candidates.entries.clear(),
        }
        Ok(candidates)
    }
}

#[test]
fn every_step_is_checked_before_another_step_or_finalization_can_hide_invalid_candidates() {
    let inventory = inventory();
    let request = request();
    let input = SelectionInput {
        inventory: &inventory,
        request: &request,
        limit: 1,
    };
    static FOREIGN: std::sync::LazyLock<InventoryWord> =
        std::sync::LazyLock::new(|| self::inventory().vocabulary.remove(0));
    for (step, expected) in [
        (
            Malformed::RemoveTarget,
            SelectionError::MissingTarget { id: "cat".into() },
        ),
        (
            Malformed::Duplicate,
            SelectionError::DuplicateCandidate { id: "cat".into() },
        ),
        (
            Malformed::Foreign(&FOREIGN),
            SelectionError::ForeignCandidate { id: "sleep".into() },
        ),
        (
            Malformed::Nonfinite,
            SelectionError::NonfiniteScore { id: "sleep".into() },
        ),
        (Malformed::Empty, SelectionError::EmptyCandidates),
    ] {
        let error = select_target_first(input, &[&step, &MustNotRun], "test-invalid").unwrap_err();
        assert!(matches!(error, StoryError::Selection(actual) if actual == expected));
    }
}

struct MustNotRun;
impl SelectionStep for MustNotRun {
    fn apply<'a>(
        &self,
        _: SelectionInput<'a>,
        _: SelectionCandidates<'a>,
    ) -> Result<SelectionCandidates<'a>, StoryError> {
        panic!("invalid selection input reached a step")
    }
}
#[test]
fn input_validation_precedes_steps_and_empty_composition_still_finalizes_within_the_limit() {
    let mut inventory = inventory();
    let mut request = request();
    for limit in [0, 17, usize::MAX] {
        assert!(matches!(
            select_target_first(
                SelectionInput {
                    inventory: &inventory,
                    request: &request,
                    limit
                },
                &[&MustNotRun],
                "test"
            ),
            Err(StoryError::Invalid(
                "selection limit must include all targets and be at most 16"
            ))
        ));
    }
    request.targets.vocabulary.push("dog".into());
    assert!(
        select_target_first(
            SelectionInput {
                inventory: &inventory,
                request: &request,
                limit: 1
            },
            &[&MustNotRun],
            "test"
        )
        .is_err()
    );
    request.targets.vocabulary.push("cat".into());
    assert!(matches!(
        select_target_first(
            SelectionInput {
                inventory: &inventory,
                request: &request,
                limit: 3
            },
            &[&MustNotRun],
            "test"
        ),
        Err(StoryError::Invalid(
            "missing or duplicate vocabulary target"
        ))
    ));
    request.targets.vocabulary.pop();
    request.targets.vocabulary.push("outside".into());
    assert!(
        select_target_first(
            SelectionInput {
                inventory: &inventory,
                request: &request,
                limit: 3
            },
            &[&MustNotRun],
            "test"
        )
        .is_err()
    );
    request.targets.vocabulary.pop();
    let selection = select_target_first(
        SelectionInput {
            inventory: &inventory,
            request: &request,
            limit: 2,
        },
        &[],
        "inventory-order-v1",
    )
    .unwrap();
    assert_eq!(
        selection
            .selected
            .iter()
            .map(|entry| entry.word.id.as_str())
            .collect::<Vec<_>>(),
        ["cat", "dog"]
    );
    inventory.vocabulary[1].id = "sleep".into();
    assert!(matches!(
        select_target_first(
            SelectionInput {
                inventory: &inventory,
                request: &request,
                limit: 2
            },
            &[&MustNotRun],
            "test"
        ),
        Err(StoryError::Inventory(_))
    ));
}
