use yomibu::adapters::input_file::{InputFileError, read_bounded};
#[test]
fn reads_exact_bytes_at_limit_and_rejects_excess_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.json");
    std::fs::write(&path, "猫").unwrap();
    assert_eq!(
        read_bounded(&path, "Story request", 3, "3 bytes").unwrap(),
        "猫".as_bytes()
    );
    assert!(matches!(
        read_bounded(&path, "Story request", 2, "2 bytes"),
        Err(InputFileError::TooLarge { .. })
    ));
    assert_eq!(
        read_bounded(&path, "Story request", usize::MAX, "maximum").unwrap(),
        "猫".as_bytes()
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "猫");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn absent_explicit_file_reports_the_open_cause_without_creating_directories() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        read_bounded(
            &dir.path().join("missing/input.json"),
            "Analysis",
            65536,
            "64 KiB (65536 bytes)"
        ),
        Err(InputFileError::Open { .. })
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
