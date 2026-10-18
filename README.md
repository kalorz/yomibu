# Yomibu

Yomibu is being rebuilt to create Japanese reading material. The previous
implementation has been removed; the workspace retains four minimal crates,
architecture enforcement and the [design plans](docs/plan/).

Build with `cargo build --locked`. Story generation, synchronization and credential
management are not currently available.

## Local PostgreSQL

On macOS, install and start Docker Desktop, which includes Docker Compose. From
the repository root, start PostgreSQL 18 with pgvector:

```sh
docker compose up -d --wait --wait-timeout 60
```

The [Compose setup](compose.yaml) uses the same pinned database image as CI,
supports Apple Silicon and Intel Macs, and listens on `127.0.0.1:55432`.
It creates database `yomibu` with user `yomibu` and password `yomibu-dev`.
These credentials are for local development.

On the first start with an empty volume, PostgreSQL applies the
[retained schema](docs/plan/schema.postgresql.sql), including the vector extension.
Later starts reuse the data; changes to the SQL file do not migrate an existing
database. The database starts without learners or collections.
Under the current [schema-change policy](ARCHITECTURE.md#schema-changes), recreate
the database after schema changes with `docker compose down --volumes`, then the
startup command above. This erases the local database.

Open a SQL console or stop the database with:

```sh
docker compose exec postgres psql -U yomibu -d yomibu
docker compose down
```

`docker compose down` preserves the named data volume.

## List collections

`yomibu collections --learner-id <UUID>` lists the learner's collection UUIDs.
Set `YOMIBU_DATABASE_URL` to a PostgreSQL connection URL. The database must
already have the [retained schema](docs/plan/schema.postgresql.sql), which requires
PostgreSQL 18+ and pgvector. The connection's role needs read access to
`learner_progress_sources`, and its search path must include the schema containing
that table. The command does not install or migrate the schema.
Surrounding whitespace in the URL is ignored. SQLx's PostgreSQL defaults apply
to connection settings omitted from the URL, including `PG*` environment variables
and `.pgpass` credentials; `YOMIBU_DATABASE_URL` itself is always required.

```sh
export YOMIBU_DATABASE_URL='postgres://yomibu:yomibu-dev@127.0.0.1:55432/yomibu'
./target/debug/yomibu collections --learner-id 00000000-0000-0000-0000-000000000001
```

Each collection is listed once, even when the learner has several progress sources
for it or those sources have no learner progress yet.
Success prints one UUID per line. Add `--json` to print a JSON array of UUID strings
followed by a newline. Ordering is unspecified in both formats. For an unknown
learner or one with no progress sources, the command succeeds with no output by
default or `[]` with `--json`. Invalid arguments, missing or invalid configuration,
and storage failures print an error to stderr and exit unsuccessfully. Connection credentials
and raw provider errors are not printed.
`yomibu --help` and `yomibu collections --help` need no database configuration.

Startup and lookup each have a five-second limit; connection acquisition and
server statements are also limited to five seconds. Pool shutdown and runtime
shutdown each wait at most one second. These are cooperative I/O bounds, not
hard real-time limits; see the [storage contract](crates/yomibu-core/src/ports/infrastructure/mod.rs).
SQLx can retry refused connections within the startup bound. Output is written
only after lookup and pool shutdown succeed; a pool shutdown timeout fails the
command without printing IDs. Output write failures also exit unsuccessfully.

## Create a learner

`yomibu create-learner --name <NAME>` creates one learner and prints its assigned
UUID followed by a newline. It uses the same `YOMIBU_DATABASE_URL` configuration
and retained schema described above. The database role needs `INSERT` on
`learners` and `SELECT` on its `id` column for the returned UUID.
Add `--json` to print `{"id":"<assigned UUID>"}` followed by a newline.

```sh
./target/debug/yomibu create-learner --name 'Alice'
./target/debug/yomibu create-learner --name 'Alice' --json
```

Surrounding Unicode whitespace is trimmed; a blank name is rejected before
database configuration or connection. Names are not unique: running the command
again creates another learner with a different UUID. It creates no collections
or learner progress sources. `yomibu create-learner --help` needs no database
configuration.

Startup and creation each have a five-second limit, with the same acquisition,
server statement, and shutdown bounds as collection lookup. The write is never
automatically retried. PostgreSQL syntax/access and integrity-constraint rejections
report that no learner was created. Other failures after dispatch, including
timeouts and connection failures, conservatively report uncertain completion
instead of claiming rollback. Once creation is confirmed, any subsequent deadline,
cleanup, or output failure retains that fact and includes the assigned UUID in stderr.
These failures exit unsuccessfully. No UUID is printed to stdout before creation
and pool shutdown succeed; an output failure can leave partial or complete output
bytes already written. Repeating creation after a failure may create a duplicate.
In both formats, diagnostics remain plain text on stderr and omit credentials
and raw provider messages.

## Developing

`crates/yomibu-app` is the application library; `crates/yomibu-cli` builds `yomibu`.
Shared contracts live in `yomibu-core`; implementations live in `yomibu-ext`.
The [architecture guide](ARCHITECTURE.md) defines enduring boundaries and practices;
[AGENTS.md](AGENTS.md) gives the working instructions for agents.
The [glossary](GLOSSARY.md) explains domain terms.
The [request example](docs/plan/story-generation-request.yml) explores a possible
structure; it neither specifies CLI input formats nor finalizes product behavior.
[Durable execution](docs/plan/durable-execution.md) and [replay](docs/plan/replay.md)
are exploratory proposals.

Run Rust tests with `cargo test --locked --all`. [CI](.github/workflows/ci.yml)
defines the full checks and runs them on Linux and macOS.

The PostgreSQL adapter contract and CLI wiring tests are explicitly ignored by
that default command; CI runs them separately against PostgreSQL with pgvector. For a local run,
supply a disposable PostgreSQL 18+ database with the `vector` extension
installed in `public`, no application tables in `public`, and a test role permitted
to create/drop schemas and terminate its own connections. With Compose running,
create a separate test database once:

```sh
docker compose exec postgres createdb -U yomibu yomibu_test
docker compose exec postgres psql -U yomibu -d yomibu_test -v ON_ERROR_STOP=1 -c 'CREATE EXTENSION vector;'
```

Then run:

```sh
YOMIBU_TEST_DATABASE_URL='postgres://yomibu:yomibu-dev@127.0.0.1:55432/yomibu_test' \
  cargo test --locked --all -- --ignored
```

Each test applies the complete retained schema in an isolated schema and removes
its schema on success. The adapter test covers database semantics; the CLI test
checks the real command's output and exit behavior. Failed runs can leave test
schemas behind; use only a disposable database. Missing prerequisites fail the
explicit test run rather than substituting another backend.

See the [application operations](crates/yomibu-app/src/application/mod.rs),
[storage contract](crates/yomibu-core/src/ports/infrastructure/mod.rs), and
[PostgreSQL adapters](crates/yomibu-ext/src/postgres.rs) for read/write guarantees,
deadline handling, and pool configuration responsibilities.

Check crate boundaries with `python3 -B tools/check_architecture.py` using Python 3
and the repository Rust toolchain. Install the pinned architecture analyzer with
`cargo install cargo-bylaw --version 0.1.0 --locked`, then run
`python3 -B tools/check_domain.py`. Verify the enforcement contracts with
`python3 -B -m unittest discover -s tools -p 'test_*.py' -v`.
Architecture checks run separately from `cargo test`.
See [verification](ARCHITECTURE.md#verification) for coverage and remaining
enforcement gaps. No legacy violations are exempted.

Build API documentation with `cargo doc --locked --no-deps`; the output is in
`target/doc/yomibu_app`.
