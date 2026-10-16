use std::io::{self, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Arg, Command, value_parser};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use uuid::Uuid;
use yomibu_app::application::list_collection_ids;
use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_ext::PostgresLearnerCollectionQuery;

const IO_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

fn cli() -> Command {
    Command::new("yomibu")
        .about("Japanese reading material tools")
        .subcommand_required(true)
        .disable_help_subcommand(true)
        .subcommand(
            Command::new("collections")
                .about("List collection UUIDs associated with a learner")
                .after_help("Requires YOMIBU_DATABASE_URL and a database with the retained schema. Prints one UUID per line in unspecified order; no associations produce no output.")
                .arg(
                    Arg::new("learner")
                        .long("learner")
                        .value_name("UUID")
                        .help("Learner whose associated collections to list")
                        .required(true)
                        .value_parser(value_parser!(Uuid)),
                ),
        )
}

fn main() -> ExitCode {
    // clap handles help and rejects syntax before configuration or database access.
    let matches = cli().get_matches();
    let collections = matches
        .subcommand_matches("collections")
        .expect("required subcommand");
    let learner = *collections
        .get_one::<Uuid>("learner")
        .expect("required learner UUID");
    let result = run(LearnerId(learner));
    report_result(result, io::stdout().lock(), io::stderr().lock())
}

fn report_result(
    result: Result<Vec<CollectionId>, String>,
    stdout: impl Write,
    mut stderr: impl Write,
) -> ExitCode {
    let result = result.and_then(|ids| {
        let mut stdout = io::BufWriter::new(stdout);
        for id in ids {
            writeln!(stdout, "{}", id.0).map_err(|_| "could not write collection IDs to stdout")?;
        }
        stdout.flush().map_err(|_| "could not flush stdout")?;
        Ok(())
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(stderr, "error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(learner: LearnerId) -> Result<Vec<CollectionId>, String> {
    let url = std::env::var("YOMIBU_DATABASE_URL")
        .map_err(|error| match error {
            std::env::VarError::NotPresent => {
                "YOMIBU_DATABASE_URL must be set to a PostgreSQL connection URL"
            }
            std::env::VarError::NotUnicode(_) => "YOMIBU_DATABASE_URL must contain valid Unicode",
        })?
        .trim()
        .to_owned();
    if url.is_empty() {
        return Err("YOMIBU_DATABASE_URL must not be empty".into());
    }
    // SQLx parses URL components but does not validate the scheme.
    if !url.split_once("://").is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("postgres") || scheme.eq_ignore_ascii_case("postgresql")
    }) {
        return Err("YOMIBU_DATABASE_URL must use the postgres or postgresql URL scheme".into());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "could not start the database runtime")?;
    let result = runtime.block_on(collections(learner, url));
    // Abandoned blocking work (for example DNS) must not hold process exit open.
    runtime.shutdown_timeout(SHUTDOWN_TIMEOUT);
    result
}

async fn collections(learner: LearnerId, url: String) -> Result<Vec<CollectionId>, String> {
    let pool = tokio::time::timeout(IO_TIMEOUT, async {
        // The driver's option parsing may read credential files. Include that
        // blocking work in the startup bound, alongside connection establishment.
        let options = tokio::task::spawn_blocking(move || url.parse::<PgConnectOptions>())
            .await
            .map_err(|_| "database configuration task failed")?
            .map_err(|_| "YOMIBU_DATABASE_URL is not a valid PostgreSQL connection URL")?;
        PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(IO_TIMEOUT)
            .connect_with(
                options.options([("statement_timeout", IO_TIMEOUT.as_millis().to_string())]),
            )
            .await
            // Provider errors can contain connection credentials or server details.
            .map_err(|_| "database startup failed; check connection settings and availability")
    })
    .await
    .map_err(|_| "database startup timed out")??;

    let storage = PostgresLearnerCollectionQuery::new(pool.clone());
    let result = list_collection_ids(learner, &storage, Instant::now() + IO_TIMEOUT).await;
    let shutdown = tokio::time::timeout(SHUTDOWN_TIMEOUT, pool.close()).await;
    // Preserve a lookup failure even if cleanup also times out; cleanup failure
    // after a successful lookup must not be reported as successful completion.
    let ids = result.map_err(|error| error.to_string())?;
    shutdown.map_err(|_| "database shutdown timed out")?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_result_prints_one_uuid_per_line() {
        let ids = vec![
            CollectionId(Uuid::from_u128(9)),
            CollectionId(Uuid::from_u128(2)),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            report_result(Ok(ids), &mut stdout, &mut stderr),
            ExitCode::SUCCESS
        );
        assert!(stderr.is_empty());
        let stdout = String::from_utf8(stdout).unwrap();
        assert!(stdout.ends_with('\n'));
        let mut lines: Vec<_> = stdout.lines().collect();
        lines.sort_unstable();
        assert_eq!(
            lines,
            [
                "00000000-0000-0000-0000-000000000002",
                "00000000-0000-0000-0000-000000000009"
            ]
        );
    }

    #[test]
    fn empty_result_succeeds_without_output() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            report_result(Ok(vec![]), &mut stdout, &mut stderr),
            ExitCode::SUCCESS
        );
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }

    #[test]
    fn lookup_failure_prints_only_stderr_and_fails() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            report_result(
                Err("collection lookup failed (SQLSTATE 42P01)".into()),
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

    #[test]
    fn output_write_failure_is_unsuccessful() {
        let mut stderr = Vec::new();
        // A zero-capacity slice cannot accept the buffered UUID output.
        let stdout = &mut [][..];
        assert_eq!(
            report_result(
                Ok(vec![CollectionId(Uuid::from_u128(9))]),
                stdout,
                &mut stderr
            ),
            ExitCode::FAILURE
        );
        assert_eq!(stderr, b"error: could not flush stdout\n");
    }
}
