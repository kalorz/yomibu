use chrono::{DateTime, TimeZone, Utc};
use yomibu_core::ports::infrastructure::storage::{MAX_SOURCE_BYTES, SourceRecord};

fn time() -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000, 1).unwrap()
}

#[test]
fn source_record_validates_opaque_json_and_raw_bytes() {
    assert!(SourceRecord::new(time(), "{bad}").is_err());
    for raw in ["null", "[]", "123"] {
        assert!(SourceRecord::new(time(), raw).is_ok());
    }
    let payload = format!("{{\"x\":\"{}\"}}", "a".repeat(MAX_SOURCE_BYTES - 8));
    assert_eq!(payload.len(), MAX_SOURCE_BYTES);
    assert!(SourceRecord::new(time(), &payload).is_ok());
    assert!(SourceRecord::new(time(), &(payload + " ")).is_err());
    assert!(SourceRecord::new(time(), r#"{"x":"\u0000"}"#).is_ok()); // JSONB rejection belongs to adapter.
}
