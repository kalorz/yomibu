use super::{is_statement_rejection, storage_error};
use sqlx::postgres::PgSeverity;
use yomibu_core::ports::infrastructure::storage::StorageError;

#[test]
fn only_statement_rejections_establish_that_no_learner_was_created() {
    for code in ["23505", "23503", "42601", "42501", "42P01"] {
        assert!(is_statement_rejection(PgSeverity::Error, code), "{code}");
        for severity in [PgSeverity::Fatal, PgSeverity::Panic] {
            assert!(
                !is_statement_rejection(severity, code),
                "{severity:?}: {code}"
            );
        }
    }
    for code in ["40003", "40001", "40P01", "57014", "08006", "XX000"] {
        assert!(!is_statement_rejection(PgSeverity::Error, code), "{code}");
    }
}

#[test]
fn provider_details_are_not_exposed() {
    assert_eq!(
        storage_error(sqlx::Error::Protocol("sensitive provider detail".into())),
        StorageError("collection lookup failed (protocol)".into())
    );
}
