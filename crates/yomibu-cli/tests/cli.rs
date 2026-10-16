use std::ffi::OsStr;
use std::net::TcpListener;
use std::process::Output;
use std::time::Duration;

use tokio::process::Command;

const LEARNER: &str = "00000000-0000-0000-0000-000000000001";

async fn invoke(args: &[&str], database_url: Option<&OsStr>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    command.args(args).env_remove("YOMIBU_DATABASE_URL");
    for (name, _) in std::env::vars_os() {
        if name.to_str().is_some_and(|name| name.starts_with("PG")) {
            command.env_remove(name);
        }
    }
    // An explicit test password also prevents SQLx consulting the user's .pgpass.
    // A password supplied in the test URL takes precedence.
    command.env("PGPASSWORD", "");
    if let Some(url) = database_url {
        command.env("YOMIBU_DATABASE_URL", url);
    }
    tokio::time::timeout(Duration::from_secs(10), command.kill_on_drop(true).output())
        .await
        .expect("CLI exceeded its watchdog timeout")
        .expect("CLI could not start")
}

fn failure(output: &Output) -> &str {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    assert!(!stderr.is_empty());
    stderr
}

#[tokio::test]
async fn help_succeeds_without_configuration() {
    for (args, expected) in [
        (vec!["--help"], "collections"),
        (vec!["collections", "--help"], "--learner <UUID>"),
    ] {
        let output = invoke(&args, None).await;
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains(expected), "{stdout}");
    }
}

#[tokio::test]
async fn invalid_syntax_is_rejected_before_configuration_or_database_access() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!(
        "postgres://postgres@{}/unused",
        listener.local_addr().unwrap()
    );
    for args in [
        vec![],
        vec!["unknown"],
        vec!["collections"],
        vec!["collections", "--learner"],
        vec!["collections", "--learner", "not-a-uuid"],
        vec!["collections", "--learner", LEARNER, "--unknown"],
        vec!["collections", "list", "--learner", LEARNER],
        vec!["collections", "--learner", LEARNER, "--learner", LEARNER],
    ] {
        for config in [None, Some(url.as_str())] {
            let output = invoke(&args, config.map(OsStr::new)).await;
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            let stderr = failure(&output);
            assert!(stderr.contains("error:"), "{stderr}");
            assert!(!stderr.contains("YOMIBU_DATABASE_URL"));
        }
    }
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn configuration_failures_are_clear_and_do_not_echo_credentials() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let wrong_scheme = format!(
        "http://user:secret-marker@{}/db",
        listener.local_addr().unwrap()
    );
    for config in [
        None,
        Some(""),
        Some("postgres://user:secret-marker@localhost:invalid/db"),
        Some(wrong_scheme.as_str()),
    ] {
        let output = invoke(
            &["collections", "--learner", LEARNER],
            config.map(OsStr::new),
        )
        .await;
        let stderr = failure(&output);
        assert!(stderr.contains("YOMIBU_DATABASE_URL"), "{stderr}");
        assert!(!stderr.contains("secret-marker"));
        assert!(!stderr.contains("postgres://"));
    }
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[cfg(unix)]
#[tokio::test]
async fn non_unicode_configuration_is_reported_as_invalid() {
    use std::os::unix::ffi::OsStrExt;

    let output = invoke(
        &["collections", "--learner", LEARNER],
        Some(OsStr::from_bytes(&[0xff])),
    )
    .await;
    let stderr = failure(&output);
    assert!(
        stderr.contains("YOMIBU_DATABASE_URL must contain valid Unicode"),
        "{stderr}"
    );
}

#[tokio::test]
async fn unresponsive_database_startup_is_bounded_and_sanitized() {
    // A listening socket that never speaks PostgreSQL stalls the startup handshake.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "  PoStGrEsQl://user:secret-marker@{}/db \n",
        listener.local_addr().unwrap()
    );
    let output = invoke(
        &["collections", "--learner", LEARNER],
        Some(OsStr::new(&url)),
    )
    .await;
    let stderr = failure(&output);
    assert!(stderr.contains("database startup"), "{stderr}");
    assert!(!stderr.contains("secret-marker"));
    assert!(!stderr.contains("postgres://"));
}

#[tokio::test]
#[ignore = "requires YOMIBU_TEST_DATABASE_URL: disposable PostgreSQL 18+ database with vector installed"]
async fn collections_command_uses_postgres() {
    tokio::time::timeout(Duration::from_secs(30), check_postgres_wiring())
        .await
        .expect("PostgreSQL CLI wiring test exceeded its watchdog timeout");
}

async fn check_postgres_wiring() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use sqlx::{AssertSqlSafe, ConnectOptions};
    use uuid::Uuid;

    let url = std::env::var("YOMIBU_TEST_DATABASE_URL")
        .expect("set YOMIBU_TEST_DATABASE_URL to a disposable PostgreSQL 18+ database with vector installed");
    let options: PgConnectOptions = url
        .parse()
        .expect("invalid test database connection options");
    let options = options.options([("statement_timeout", "3s")]);
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options.clone())
        .await
        .unwrap_or_else(|_| panic!("could not connect to the PostgreSQL test database"));
    let suffix: Uuid = sqlx::query_scalar("SELECT uuidv7()")
        .fetch_one(&admin)
        .await
        .expect("test database must support uuidv7(); use PostgreSQL 18+");
    let schema = format!("yomibu_cli_{}", suffix.simple());
    sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let options = options.options([("search_path", format!("{schema},public"))]);
    let control = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../../docs/plan/schema.postgresql.sql"))
        .execute(&control)
        .await
        .expect(
            "could not apply retained schema; use PostgreSQL 18+ with vector installed in public",
        );
    sqlx::raw_sql(
        "INSERT INTO learners (id, name) VALUES ('00000000-0000-0000-0000-000000000001', 'CLI learner');
         INSERT INTO collections (id, provider, name) VALUES
            ('00000000-0000-0000-0000-000000000010', 'manual', 'First'),
            ('00000000-0000-0000-0000-000000000020', 'manual', 'Second');
         INSERT INTO learner_collections (learner_id, collection_id) VALUES
            ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010'),
            ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000020');"
    ).execute(&control).await.unwrap();

    // SQLx's URL serializer omits session options; forward them explicitly.
    let mut url = options.to_url_lossy();
    url.query_pairs_mut()
        .append_pair("options", options.get_options().unwrap());
    let database_url = OsStr::new(url.as_str());
    let output = invoke(&["collections", "--learner", LEARNER], Some(database_url)).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.ends_with('\n'));
    let mut lines: Vec<_> = stdout.lines().collect();
    lines.sort_unstable();
    assert_eq!(
        lines,
        [
            "00000000-0000-0000-0000-000000000010",
            "00000000-0000-0000-0000-000000000020"
        ]
    );

    let output = invoke(
        &[
            "collections",
            "--learner",
            "00000000-0000-0000-0000-000000000099",
        ],
        Some(database_url),
    )
    .await;
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    sqlx::query("ALTER TABLE learner_collections RENAME TO hidden_associations")
        .execute(&control)
        .await
        .unwrap();
    let output = invoke(&["collections", "--learner", LEARNER], Some(database_url)).await;
    let stderr = failure(&output);
    assert!(stderr.contains("collection lookup failed"));
    assert!(!stderr.contains(&schema));
    assert!(!stderr.contains("postgres://"));

    control.close().await;
    sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
