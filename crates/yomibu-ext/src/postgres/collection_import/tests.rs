use super::*;
use std::time::Duration;

#[test]
fn lookup_completion_rejects_expiry_and_returns_sorted_ids_within_budget() {
    assert_eq!(
        finish_lookup(vec!["2".into(), "10".into()], Instant::now()),
        Err(lookup_deadline()),
    );
    assert_eq!(
        finish_lookup(
            vec!["2".into(), "10".into()],
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap(),
        ["10", "2"],
    );
}
