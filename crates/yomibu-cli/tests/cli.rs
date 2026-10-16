use std::ffi::OsStr;
use std::net::TcpListener;
use std::process::Output;
use std::time::Duration;

use tokio::process::Command;

const LEARNER: &str = "00000000-0000-0000-0000-000000000001";

async fn invoke(args: &[&str], database_url: Option<&OsStr>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yomibu"));
    // The CLI inherits no configuration or PG* defaults. An explicit test password
    // also prevents SQLx consulting the user's .pgpass; a password supplied in the
    // test URL takes precedence.
    command.args(args).env_clear().env("PGPASSWORD", "");
    if let Some(url) = database_url {
        command.env("YOMIBU_DATABASE_URL", url);
    }
    tokio::time::timeout(Duration::from_secs(10), command.kill_on_drop(true).output())
        .await
        .expect("CLI exceeded its watchdog timeout")
        .expect("CLI could not start")
}

/// A local address for checking that rejected invocations never connect.
fn idle_listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn assert_never_connected(listener: &TcpListener) {
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
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
        (vec!["--help"], vec!["collections", "create-learner"]),
        (
            vec!["create-learner", "--help"],
            vec!["--name <NAME>", "--json"],
        ),
        (
            vec!["collections", "--help"],
            vec!["--learner <UUID>", "--json"],
        ),
    ] {
        let output = invoke(&args, None).await;
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert!(stdout.contains("Usage:"));
        for text in expected {
            assert!(stdout.contains(text), "{stdout}");
        }
    }
}

#[tokio::test]
async fn blank_name_is_rejected_before_configuration_and_connection() {
    let listener = idle_listener();
    let url = format!(
        "postgres://user:secret-marker@{}/unused",
        listener.local_addr().unwrap()
    );
    for config in [None, Some(OsStr::new(&url))] {
        let output = invoke(&["create-learner", "--name", " "], config).await;
        let stderr = failure(&output);
        assert!(
            stderr.contains("learner name must not be blank"),
            "{stderr}"
        );
        assert!(!stderr.contains("YOMIBU_DATABASE_URL"));
        assert!(!stderr.contains("secret-marker"));
    }
    assert_never_connected(&listener);
}

#[tokio::test]
async fn invalid_syntax_is_rejected_before_configuration_or_database_access() {
    let listener = idle_listener();
    let url = format!(
        "postgres://postgres@{}/unused",
        listener.local_addr().unwrap()
    );
    for args in [
        vec![],
        vec!["unknown"],
        vec!["create-learner"],
        vec!["create-learner", "--name"],
        vec!["create-learner", "--name", "Alice", "--unknown"],
        vec!["create-learner", "--name", "Alice", "--name", "Bob"],
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
    assert_never_connected(&listener);
}

#[tokio::test]
async fn configuration_failures_are_clear_and_do_not_echo_credentials() {
    let listener = idle_listener();
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
    assert_never_connected(&listener);
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
async fn commands_use_postgres() {
    tokio::time::timeout(Duration::from_secs(30), check_postgres_wiring())
        .await
        .expect("PostgreSQL CLI wiring test exceeded its watchdog timeout");
}

async fn check_postgres_wiring() {
    use sqlx::postgres::PgConnectOptions;
    use sqlx::{AssertSqlSafe, ConnectOptions, Connection};
    use uuid::Uuid;

    let url = std::env::var("YOMIBU_TEST_DATABASE_URL")
        .expect("set YOMIBU_TEST_DATABASE_URL to a disposable PostgreSQL 18+ database with vector installed");
    let options: PgConnectOptions = url
        .parse()
        .expect("invalid test database connection options");
    let mut admin = options
        .connect()
        .await
        .expect("could not connect to the PostgreSQL test database");
    let suffix: Uuid = sqlx::query_scalar("SELECT uuidv7()")
        .fetch_one(&mut admin)
        .await
        .expect("test database must support uuidv7(); use PostgreSQL 18+");
    let schema = format!("yomibu_cli_{}", suffix.simple());
    sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&mut admin)
        .await
        .unwrap();
    let options = options.options([("search_path", format!("{schema},public"))]);
    let mut control = options
        .connect()
        .await
        .expect("could not connect to the PostgreSQL test database");
    sqlx::raw_sql(include_str!("../../../docs/plan/schema.postgresql.sql"))
        .execute(&mut control)
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
    ).execute(&mut control).await.unwrap();

    // SQLx's URL serializer omits session options; forward the search path explicitly.
    let mut url = options.to_url_lossy();
    url.query_pairs_mut()
        .append_pair("options", options.get_options().unwrap());
    let database_url = OsStr::new(url.as_str());
    let output = invoke(
        &["create-learner", "--name", "New CLI learner"],
        Some(database_url),
    )
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let id: Uuid = stdout.strip_suffix('\n').unwrap().parse().unwrap();
    let name: String = sqlx::query_scalar("SELECT name FROM learners WHERE id = $1")
        .bind(id)
        .fetch_one(&mut control)
        .await
        .unwrap();
    assert_eq!(name, "New CLI learner");

    let output = invoke(
        &["create-learner", "--name", "Alice", "--json"],
        Some(database_url),
    )
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM learners WHERE name = 'Alice'")
        .fetch_all(&mut control)
        .await
        .unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(result, serde_json::json!({"id": ids[0].to_string()}));

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
        &["collections", "--learner", LEARNER, "--json"],
        Some(database_url),
    )
    .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let mut json_ids: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    json_ids.sort_unstable();
    assert_eq!(json_ids, lines);

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
        .execute(&mut control)
        .await
        .unwrap();
    let output = invoke(&["collections", "--learner", LEARNER], Some(database_url)).await;
    let stderr = failure(&output);
    assert!(stderr.contains("collection lookup failed"));
    assert!(!stderr.contains(&schema));
    assert!(!stderr.contains("postgres://"));

    control.close().await.unwrap();
    sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&mut admin)
        .await
        .unwrap();
    admin.close().await.unwrap();
}
