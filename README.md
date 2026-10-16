# Yomibu

Yomibu is being rebuilt to create Japanese reading material. The previous
implementation has been removed; the workspace retains four minimal crates,
architecture enforcement and the [design plans](docs/plan/).

Build with `cargo build --locked`. Story generation, synchronization and credential
management are not currently available.

## List collections

`yomibu collections --learner <UUID>` lists the collection UUIDs associated with a
learner. Set `YOMIBU_DATABASE_URL` to a PostgreSQL connection URL. The database must
already have the [retained schema](docs/plan/schema.postgresql.sql), which requires
PostgreSQL 18+ and pgvector. The connection's role needs read access to
`learner_collections`, and its search path must include the schema containing that
table. The command does not install or migrate the schema.
Surrounding whitespace in the URL is ignored. SQLx's PostgreSQL defaults apply
to connection settings omitted from the URL, including `PG*` environment variables
and `.pgpass` credentials; `YOMIBU_DATABASE_URL` itself is always required.

```sh
export YOMIBU_DATABASE_URL='postgres://yomibu@localhost/yomibu'
./target/debug/yomibu collections --learner 00000000-0000-0000-0000-000000000001
```

Success prints one UUID per line, in unspecified order. No associations, including
an unknown learner, produce no output and succeed. Invalid arguments, missing or
invalid configuration, and storage failures print an error to stderr and exit
unsuccessfully. Connection credentials and raw provider errors are not printed.
`yomibu --help` and `yomibu collections --help` need no database configuration.

Startup and lookup each have a five-second limit; connection acquisition and
server statements are also limited to five seconds. Pool shutdown and runtime
shutdown each wait at most one second. These are cooperative I/O bounds, not
hard real-time limits; see the [storage contract](crates/yomibu-core/src/ports/infrastructure/mod.rs).
SQLx can retry refused connections within the startup bound. Output is written
only after lookup and pool shutdown succeed; a pool shutdown timeout fails the
command without printing IDs. Output write failures also exit unsuccessfully.

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
installed in `public`, and a test role permitted to create/drop schemas and
terminate its own connections. Run:

```sh
YOMIBU_TEST_DATABASE_URL='postgres://postgres@localhost/yomibu_test' \
  cargo test --locked -p yomibu-ext --test postgres -- --ignored
YOMIBU_TEST_DATABASE_URL='postgres://postgres@localhost/yomibu_test' \
  cargo test --locked -p yomibu-cli --test cli -- --ignored
```

Each test applies the complete retained schema in an isolated schema and removes
its schema on success. The adapter test covers database semantics; the CLI test
checks the real command's output and exit behavior. Failed runs can leave test
schemas behind; use only a disposable database. Missing prerequisites fail the
explicit test run rather than substituting another backend.

See the [application operation](crates/yomibu-app/src/application/mod.rs),
[storage contract](crates/yomibu-core/src/ports/infrastructure/mod.rs), and
[PostgreSQL adapter](crates/yomibu-ext/src/postgres.rs) for lookup guarantees,
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
