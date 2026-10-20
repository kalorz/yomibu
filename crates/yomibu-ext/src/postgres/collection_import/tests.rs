use super::*;
use std::time::Duration;

#[test]
fn chunks_bound_rows_and_bytes_without_dropping_large_records() {
    for (sizes, lengths) in [
        (vec![1; CHUNK_ROWS + 1], vec![CHUNK_ROWS, 1]),
        (vec![CHUNK_BYTES / 2 + 1; 3], vec![1, 1, 1]),
        (vec![CHUNK_BYTES + 1, 1], vec![1, 1]),
    ] {
        let groups: Vec<_> = chunks(&sizes, |size| *size).collect();
        assert_eq!(groups.iter().map(|g| g.len()).collect::<Vec<_>>(), lengths);
        assert_eq!(groups.concat(), sizes);
    }
}

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
