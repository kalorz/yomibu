use std::io::{self, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Arg, ArgAction, Command, value_parser};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use uuid::Uuid;
use yomibu_app::application::{CreateLearner, CreateLearnerError, list_collection_ids};
use yomibu_core::domain::{CollectionId, LearnerId};
use yomibu_ext::PostgresLearnerStorage;

const IO_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

fn cli() -> Command {
    Command::new("yomibu")
        .about("Japanese reading material tools")
        .subcommand_required(true)
        .disable_help_subcommand(true)
        .subcommand(
            Command::new("create-learner")
                .about("Create a learner and print its assigned UUID")
                .after_help("Requires YOMIBU_DATABASE_URL and a database with the retained schema. Surrounding name whitespace is trimmed; blank names are rejected. Names need not be unique. Prints the assigned UUID followed by a newline. With --json, prints {\"id\":\"<UUID>\"} followed by a newline. An interrupted write may have been saved; it is never automatically retried. If creation is confirmed but a later step fails, stderr includes its UUID.")
                .arg(
                    Arg::new("name")
                        .long("name")
                        .value_name("NAME")
                        .help("Name for the new learner")
                        .required(true),
                )
                .arg(
                    Arg::new("json")
                        .long("json")
                        .action(ArgAction::SetTrue)
                        .help("Print the assigned UUID as a JSON object"),
                ),
        )
        .subcommand(
            Command::new("collections")
                .about("List collection UUIDs associated with a learner")
                .after_help("Requires YOMIBU_DATABASE_URL and a database with the retained schema. By default, prints one UUID per line; no associations produce no output. With --json, prints a JSON array of UUID strings followed by a newline; no associations produce []. Ordering is unspecified in both formats.")
                .arg(
                    Arg::new("learner")
                        .long("learner")
                        .value_name("UUID")
                        .help("Learner whose associated collections to list")
                        .required(true)
                        .value_parser(value_parser!(Uuid)),
                )
                .arg(
                    Arg::new("json")
                        .long("json")
                        .action(ArgAction::SetTrue)
                        .help("Print collection UUIDs as a JSON array"),
                ),
        )
}

fn main() -> ExitCode {
    // clap handles help and rejects syntax before configuration or database access.
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("create-learner", create)) => {
            let name = create.get_one::<String>("name").expect("required name");
            report_created(
                run_create_learner(name),
                create.get_flag("json"),
                io::BufWriter::new(io::stdout().lock()),
                io::stderr().lock(),
            )
        }
        Some(("collections", collections)) => {
            let learner = *collections
                .get_one::<Uuid>("learner")
                .expect("required learner UUID");
            report_result(
                run_collections(LearnerId(learner)),
                collections.get_flag("json"),
                io::BufWriter::new(io::stdout().lock()),
                io::stderr().lock(),
            )
        }
        _ => unreachable!("clap requires a known subcommand"),
    }
}

fn report_result(
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

fn report_created(
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
        let ids: Vec<_> = ids.iter().map(|id| id.0.to_string()).collect();
        serde_json::to_writer(&mut stdout, &ids)?;
        writeln!(stdout)?;
    } else {
        for id in ids {
            writeln!(stdout, "{}", id.0)?;
        }
    }
    Ok(())
}

fn run_collections(learner: LearnerId) -> Result<Vec<CollectionId>, String> {
    let url = database_url()?;
    block_on_database(collections(learner, url))
}

fn run_create_learner(name: &str) -> Result<LearnerId, String> {
    let request = CreateLearner::new(name).map_err(|error| error.to_string())?;
    let url = database_url()?;
    block_on_database(create_learner(request, url))
}

fn creation_after_shutdown(
    result: Result<LearnerId, CreateLearnerError>,
    shutdown: Result<(), &'static str>,
) -> Result<LearnerId, String> {
    let id = result.map_err(|error| error.to_string())?;
    shutdown.map_err(|reason| CreateLearnerError::Created { id, reason }.to_string())?;
    Ok(id)
}

fn database_url() -> Result<String, String> {
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
    Ok(url)
}

fn block_on_database<T>(operation: impl Future<Output = Result<T, String>>) -> Result<T, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "could not start the database runtime")?;
    let result = runtime.block_on(operation);
    // Abandoned blocking work (for example DNS) must not hold process exit open.
    runtime.shutdown_timeout(SHUTDOWN_TIMEOUT);
    result
}

async fn connect(url: String) -> Result<sqlx::PgPool, String> {
    tokio::time::timeout(IO_TIMEOUT, async {
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
    .map_err(|_| "database startup timed out")?
    .map_err(Into::into)
}

async fn close_pool(pool: &sqlx::PgPool) -> Result<(), &'static str> {
    tokio::time::timeout(SHUTDOWN_TIMEOUT, pool.close())
        .await
        .map_err(|_| "database shutdown timed out")
}

async fn collections(learner: LearnerId, url: String) -> Result<Vec<CollectionId>, String> {
    let pool = connect(url).await?;
    let storage = PostgresLearnerStorage::new(pool.clone());
    let result = list_collection_ids(learner, &storage, Instant::now() + IO_TIMEOUT).await;
    let shutdown = close_pool(&pool).await;
    // Preserve a lookup failure even if cleanup also times out; cleanup failure
    // after a successful lookup must not be reported as successful completion.
    let ids = result.map_err(|error| error.to_string())?;
    shutdown?;
    Ok(ids)
}

async fn create_learner(request: CreateLearner, url: String) -> Result<LearnerId, String> {
    let pool = connect(url).await?;
    let storage = PostgresLearnerStorage::new(pool.clone());
    let result = request.execute(&storage, Instant::now() + IO_TIMEOUT).await;
    let shutdown = close_pool(&pool).await;
    creation_after_shutdown(result, shutdown)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn cleanup_failure_preserves_creation_and_prior_errors() {
        let id = LearnerId(Uuid::from_u128(7));
        let shutdown = Err("database shutdown timed out");
        assert_eq!(
            creation_after_shutdown(Ok(id), shutdown),
            Err(CreateLearnerError::Created {
                id,
                reason: "database shutdown timed out"
            }
            .to_string())
        );
        let error = CreateLearnerError::DeadlineExceeded;
        let expected = error.to_string();
        assert_eq!(creation_after_shutdown(Err(error), shutdown), Err(expected));
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
            report_result(Ok(ids), false, &mut stdout, &mut stderr),
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
            report_result(Ok(ids), true, &mut stdout, &mut stderr),
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
                report_result(Ok(vec![]), json, &mut stdout, &mut stderr),
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
                report_result(
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
                report_result(
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
            report_result(
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
