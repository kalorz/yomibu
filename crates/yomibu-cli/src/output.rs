//! Render command results and diagnostics, retaining confirmed creation on output failure.

use std::io::{self, Write};
use std::process::ExitCode;

use yomibu_app::application::CreateLearnerError;
use yomibu_core::domain::{CollectionId, LearnerId};

pub(super) fn report_collections(
    result: Result<Vec<CollectionId>, String>,
    json: bool,
    mut stdout: impl Write,
    stderr: impl Write,
) -> ExitCode {
    let result = result.and_then(|ids| {
        write_collection_ids(&ids, json, &mut stdout)
            .map_err(|_| "could not write collection IDs to stdout")?;
        stdout.flush().map_err(|_| "could not flush stdout")?;
        Ok(())
    });
    report_status(result, stderr)
}

pub(super) fn report_created(
    result: Result<LearnerId, String>,
    json: bool,
    mut stdout: impl Write,
    stderr: impl Write,
) -> ExitCode {
    let result = result.and_then(|id| {
        let output = if json {
            writeln!(stdout, "{}", serde_json::json!({"id": id.0.to_string()}))
        } else {
            writeln!(stdout, "{}", id.0)
        };
        output.and_then(|()| stdout.flush()).map_err(|_| {
            CreateLearnerError::Created {
                id,
                reason: "could not write or flush learner UUID to stdout",
            }
            .to_string()
        })
    });
    report_status(result, stderr)
}

fn report_status(result: Result<(), String>, mut stderr: impl Write) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(stderr, "error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn write_collection_ids(
    ids: &[CollectionId],
    json: bool,
    mut stdout: impl Write,
) -> io::Result<()> {
    if json {
        let ids: Vec<_> = ids.iter().map(|id| id.0).collect();
        serde_json::to_writer(&mut stdout, &ids)?;
        writeln!(stdout)?;
    } else {
        for id in ids {
            writeln!(stdout, "{}", id.0)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn created_learner_prints_assigned_uuid_and_newline_in_both_formats() {
        let id = LearnerId(Uuid::from_u128(7));
        for (json, expected) in [
            (false, "00000000-0000-0000-0000-000000000007\n"),
            (true, "{\"id\":\"00000000-0000-0000-0000-000000000007\"}\n"),
        ] {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            assert_eq!(
                report_created(Ok(id), json, &mut stdout, &mut stderr),
                ExitCode::SUCCESS
            );
            assert_eq!(stdout, expected.as_bytes());
            assert!(stderr.is_empty());
        }
    }

    #[test]
    fn creation_errors_print_no_success_uuid() {
        let error = CreateLearnerError::Created {
            id: LearnerId(Uuid::from_u128(7)),
            reason: "creation deadline exceeded",
        }
        .to_string();
        for json in [false, true] {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            assert_eq!(
                report_created(Err(error.clone()), json, &mut stdout, &mut stderr),
                ExitCode::FAILURE
            );
            assert!(stdout.is_empty());
            assert_eq!(stderr, format!("error: {error}\n").as_bytes());
        }
    }

    #[test]
    fn output_failures_report_confirmed_creation_and_uuid() {
        let id = LearnerId(Uuid::from_u128(7));
        let expected = format!(
            "error: {}\n",
            CreateLearnerError::Created {
                id,
                reason: "could not write or flush learner UUID to stdout"
            }
        );
        for json in [false, true] {
            for stdout in [
                Box::new(&mut [][..]) as Box<dyn Write>,
                Box::new(io::BufWriter::new(&mut [][..])),
            ] {
                let mut stderr = Vec::new();
                assert_eq!(
                    report_created(Ok(id), json, stdout, &mut stderr),
                    ExitCode::FAILURE
                );
                assert_eq!(stderr, expected.as_bytes());
            }
        }
    }

    #[test]
    fn successful_result_prints_one_uuid_per_line() {
        let ids = vec![
            CollectionId(Uuid::from_u128(9)),
            CollectionId(Uuid::from_u128(2)),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            report_collections(Ok(ids), false, &mut stdout, &mut stderr),
            ExitCode::SUCCESS
        );
        assert!(stderr.is_empty());
        assert_eq!(
            stdout,
            b"00000000-0000-0000-0000-000000000009\n00000000-0000-0000-0000-000000000002\n"
        );
    }

    #[test]
    fn json_result_prints_an_array_of_uuid_strings() {
        let ids = vec![
            CollectionId(Uuid::from_u128(9)),
            CollectionId(Uuid::from_u128(2)),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            report_collections(Ok(ids), true, &mut stdout, &mut stderr),
            ExitCode::SUCCESS
        );
        assert!(stderr.is_empty());
        assert_eq!(
            stdout,
            b"[\"00000000-0000-0000-0000-000000000009\",\"00000000-0000-0000-0000-000000000002\"]\n"
        );
    }

    #[test]
    fn empty_result_succeeds_in_both_formats() {
        for (json, expected) in [(false, ""), (true, "[]\n")] {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            assert_eq!(
                report_collections(Ok(vec![]), json, &mut stdout, &mut stderr),
                ExitCode::SUCCESS
            );
            assert_eq!(stdout, expected.as_bytes());
            assert!(stderr.is_empty());
        }
    }

    #[test]
    fn lookup_failure_prints_only_stderr_and_fails() {
        for json in [false, true] {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            assert_eq!(
                report_collections(
                    Err("collection lookup failed (SQLSTATE 42P01)".into()),
                    json,
                    &mut stdout,
                    &mut stderr
                ),
                ExitCode::FAILURE
            );
            assert!(stdout.is_empty());
            assert_eq!(
                stderr,
                b"error: collection lookup failed (SQLSTATE 42P01)\n"
            );
        }
    }

    #[test]
    fn output_write_failure_is_unsuccessful() {
        for json in [false, true] {
            let mut stderr = Vec::new();
            // An empty slice rejects the first write.
            assert_eq!(
                report_collections(
                    Ok(vec![CollectionId(Uuid::from_u128(9))]),
                    json,
                    &mut [][..],
                    &mut stderr
                ),
                ExitCode::FAILURE
            );
            assert_eq!(stderr, b"error: could not write collection IDs to stdout\n");
        }
    }

    #[test]
    fn output_flush_failure_is_unsuccessful() {
        let mut stderr = Vec::new();
        // The buffer accepts one UUID; the empty slice behind it rejects the flush.
        let stdout = io::BufWriter::new(&mut [][..]);
        assert_eq!(
            report_collections(
                Ok(vec![CollectionId(Uuid::from_u128(9))]),
                false,
                stdout,
                &mut stderr
            ),
            ExitCode::FAILURE
        );
        assert_eq!(stderr, b"error: could not flush stdout\n");
    }
}
