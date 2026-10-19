use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, PgPool};
use std::time::Duration;
use uuid::Uuid;
const SCHEMA: &str = include_str!("../../../../docs/plan/schema.postgresql.sql");

pub(crate) async fn connect(options: PgConnectOptions) -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .expect("could not connect to the PostgreSQL test database")
}

pub(crate) async fn with_postgres(check: impl AsyncFnOnce(&PgPool, &PgPool)) {
    tokio::time::timeout(Duration::from_secs(30), run_postgres_contract(check))
        .await
        .expect("PostgreSQL setup, contract checks or shutdown exceeded the watchdog timeout");
}

async fn run_postgres_contract(check: impl AsyncFnOnce(&PgPool, &PgPool)) {
    let url = std::env::var("YOMIBU_TEST_DATABASE_URL")
        .expect("set YOMIBU_TEST_DATABASE_URL to a disposable PostgreSQL 18+ database with vector installed");
    let options: PgConnectOptions = url
        .parse()
        .expect("invalid test database connection options");
    let options = options.options([("statement_timeout", "3s")]);
    let admin = connect(options.clone()).await;
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert!(
        version.parse::<u32>().unwrap() >= 180000,
        "retained schema requires PostgreSQL 18+"
    );
    let vector: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT FROM pg_extension WHERE extname = 'vector')")
            .fetch_one(&admin)
            .await
            .unwrap();
    assert!(
        vector,
        "install the vector extension in the disposable database before running this test"
    );

    // PostgreSQL 18 supplies a unique suffix; only hex digits enter the identifier.
    let suffix: Uuid = sqlx::query_scalar("SELECT uuidv7()")
        .fetch_one(&admin)
        .await
        .unwrap();
    let schema = format!("yomibu_query_{}", suffix.simple());
    sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let options = options
        .application_name(&schema)
        .options([("search_path", format!("{schema},public"))]);
    // Keep setup and inspection separate from the adapter pool so tests can hold
    // locks or interrupt its session through an independent database connection.
    let control_pool = connect(options.clone()).await;
    sqlx::raw_sql(SCHEMA).execute(&control_pool).await.unwrap();
    let pool = connect(options).await;
    check(&pool, &control_pool).await;
    pool.close().await;
    control_pool.close().await;
    // A closed client socket can precede backend exit after an interrupted
    // statement. Avoid racing DROP locks with that still-draining session.
    loop {
        let active: i64 =
            sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE application_name=$1")
                .bind(&schema)
                .fetch_one(&admin)
                .await
                .unwrap();
        if active == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    sqlx::query(AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
