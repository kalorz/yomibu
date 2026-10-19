# Yomibu

Yomibu is being built to create Japanese reading material. The current CLI supports
learner creation and collection ID listing.

Build with `cargo build --locked`. Story generation, synchronization and credential
management are not currently available.

The [WaniKani epic](https://github.com/kalorz/yomibu/issues/97) records approved import
behavior; [#109](https://github.com/kalorz/yomibu/issues/109) scopes a storage task.
These are specifications, not available commands.

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

## Database configuration

Set `YOMIBU_DATABASE_URL` to a PostgreSQL connection URL. The database must
already have the [retained schema](docs/plan/schema.postgresql.sql), which requires
PostgreSQL 18+ and pgvector. The commands below do not install or migrate the schema.
Surrounding whitespace in the URL is ignored. SQLx's PostgreSQL defaults apply
to connection settings omitted from the URL, including `PG*` environment variables
and `.pgpass` credentials; `YOMIBU_DATABASE_URL` itself is always required.

For the local Compose database:

```sh
export YOMIBU_DATABASE_URL='postgres://yomibu:yomibu-dev@127.0.0.1:55432/yomibu'
```

For both commands, startup and the requested storage operation each have a five-second
limit; connection acquisition and server statements are also limited to five seconds.
Pool shutdown and runtime shutdown each wait at most one second. These are cooperative
I/O bounds, not hard real-time limits; see the
[storage contract](crates/yomibu-core/src/ports/infrastructure/mod.rs).
SQLx can retry refused connections within the startup bound.

## Create a learner

`yomibu create-learner --name <NAME>` creates one learner and prints its assigned
UUID followed by a newline. It uses the
[database configuration and limits](#database-configuration) above. The database
role needs `INSERT` on `learners` and `SELECT` on its `id` column for the returned UUID.

Create a learner and keep its UUID in a shell variable for the collection lookup:

```sh
learner_id=$(./target/debug/yomibu create-learner --name 'Alice') &&
  printf '%s\n' "$learner_id"
```

For JSON output instead, run `./target/debug/yomibu create-learner --name 'Alice' --json`.
It prints `{"id":"<assigned UUID>"}` followed by a newline.

Surrounding Unicode whitespace is trimmed; a blank name is rejected before
database configuration or connection. Names are not unique: running the command
again creates another learner with a different UUID. It creates no collections
or learner progress sources. `yomibu create-learner --help` needs no database
configuration.

The write is never automatically retried. PostgreSQL syntax/access and
integrity-constraint rejections report that no learner was created. Other failures
after dispatch, including timeouts and connection failures, conservatively report
uncertain completion instead of claiming rollback. Once creation is confirmed,
any subsequent deadline, cleanup, or output failure retains that fact and includes
the assigned UUID in stderr.
These failures exit unsuccessfully. No UUID is printed to stdout before creation
and pool shutdown succeed; an output failure can leave partial or complete output
bytes already written. Repeating creation after a failure may create a duplicate.
In both formats, diagnostics remain plain text on stderr and omit credentials
and raw provider messages.

## List collection IDs

`yomibu list-collection-ids --learner-id <UUID>` lists the learner's collection UUIDs.
It uses the [database configuration and limits](#database-configuration) above.
The connection's role needs read access to `learner_progress_sources`, and its search
path must include the schema containing that table.

After successful creation above, use the saved learner UUID:

```sh
./target/debug/yomibu list-collection-ids --learner-id "$learner_id"
```

On the freshly initialized database, this succeeds with no output: creating a learner
does not add learner progress sources.

Each collection is listed once, even when the learner has several progress sources
for it or those sources have no learner progress yet.
Success prints one UUID per line. Add `--json` to print a JSON array of UUID strings
followed by a newline. Ordering is unspecified in both formats. For an unknown
learner or one with no progress sources, the command succeeds with no output by
default or `[]` with `--json`. Invalid arguments, missing or invalid configuration,
and storage failures print an error to stderr and exit unsuccessfully. Connection credentials
and raw provider errors are not printed.
`yomibu --help` and `yomibu list-collection-ids --help` need no database configuration.

Output is written only after lookup and pool shutdown succeed; a pool shutdown timeout
fails the command without printing IDs. Output write failures also exit unsuccessfully.

## Developing

`crates/yomibu-cli` builds the `yomibu` binary used in the examples above.

The [architecture guide](ARCHITECTURE.md) defines boundaries, rules and coverage limits;
[AGENTS.md](AGENTS.md) gives the working instructions for agents.
The [glossary](GLOSSARY.md) explains domain terms.
See [crate ownership](ARCHITECTURE.md#boundaries) and
[design reference statuses](ARCHITECTURE.md#design-references) before using the
preserved plans.

### Tests

Run Rust tests with `cargo test --locked --all`. [CI](.github/workflows/ci.yml)
defines the full checks and runs them on Linux and macOS.

The PostgreSQL adapter contract and CLI wiring tests are explicitly ignored by
that default command; CI runs them separately against PostgreSQL with pgvector. For a local run,
supply a disposable PostgreSQL 18+ database with the `vector` extension
installed in `public`, no application tables in `public`, and a test role permitted
to create/drop schemas, terminate its own connections, and set `deadlock_timeout`
for the controlled deadlock test. With Compose running,
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
its schema on success. Adapter tests cover database semantics; the CLI test
checks the real command's output and exit behavior. Failed runs can leave test
schemas behind; use only a disposable database. Missing prerequisites fail the
explicit test run rather than substituting another backend.

The [shared collection import contracts](crates/yomibu-core/src/ports/infrastructure/collection_import.rs)
accept supplied collection/progress-source identities, opaque subject IDs and both
material kinds. [WaniKani preparation](crates/yomibu-ext/src/wanikani.rs) owns its
provider-specific validation and type mapping. The contracts cover finite atomic
groups and explicit successful-sync completion from
[#109](https://github.com/kalorz/yomibu/issues/109). This storage slice has no HTTP
reader, application import operation or CLI import command. Its PostgreSQL tests
include controlled overlap, cancellation, deadlock rollback without retry,
deferred commit rejection, and lost/late commit responses through a test-only
loopback protocol relay. Use a trusted disposable local database for these fault tests.

The ignored-test invocation also runs a bounded resource smoke test: 1,000 subjects
and progress records, generated in groups of 500. The larger measurement matrix is
opt-in. Run each combination in a separate process, setting the test database URL
as above:

```sh
YOMIBU_STORAGE_RESOURCE_SCENARIO=cold \
YOMIBU_STORAGE_RESOURCE_TOTAL=10000 \
YOMIBU_STORAGE_RESOURCE_POOL=2 \
  cargo test --locked -p yomibu-ext --test postgres_wanikani_resources -- --ignored --nocapture --test-threads=1
```

Scenarios are `cold`, `reuse` (no resupplied subjects), `replay` (stale progress
with later observations), `identical-replay` (equal subject/progress payloads with
later observations), `overlap-source`, and `overlap-accounts`. Measure each at
1,000 and 10,000 records per import with pool sizes 1 and 2. The overlap scenarios
have two active imports. Also run `near-limit` (one almost-8-MiB group) and
`byte-chunks` (eight approximately 900-KiB subject payloads); their totals are fixed.
The fixture generator retains one group per active import and a bounded size sample.

Each `RESOURCE` JSON record reports raw input bytes, maximum group and Bind-frame
bytes, SQL command count, Parse messages, protocol exchanges, elapsed time and a
payload-size sample. Setup/prepopulation counters are separate. Exchanges count
Simple Query and Sync boundaries, not TCP packets or measured network RTTs. For
process peak memory, first compile with `--no-run`, then run the test executable
printed by Cargo directly under the host's resource tool (Linux `/usr/bin/time -v`
or Python `os.wait4`). Run one scenario per process; measuring Cargo or a parallel
workspace suite does not isolate this test's RSS. RSS includes SQLx, fixtures,
setup and the local relay; elapsed measurements exclude prepopulation.

On Linux 6.18 x86_64, Rust 1.98.1 debug builds and the CI PostgreSQL 18 image,
the 2026-10-19 ownership-correction verification produced the following results
from 28 separate test processes. These synthetic
measurements verify bounds and scaling; they are not an import latency SLA or
measurements of live WaniKani payloads.

| Scenario | Records per import | Elapsed ms, pool 1 / 2 | Peak RSS KiB, pool 1 / 2 | SQL commands |
| --- | ---: | ---: | ---: | ---: |
| Cold | 10,000 | 2,647 / 2,600 | 11,952 / 12,496 | 445 |
| Reuse | 10,000 | 1,237 / 1,336 | 11,460 / 12,340 | 385 |
| Stale replay | 10,000 | 1,296 / 1,289 | 11,800 / 12,600 | 385 |
| Identical replay | 10,000 | 2,396 / 2,388 | 11,992 / 12,468 | 445 |
| Overlap, same source | 10,000 × 2 | 5,152 / 5,391 | 13,000 / 13,516 | 890 |
| Overlap, accounts | 10,000 × 2 | 5,272 / 4,661 | 13,400 / 13,660 | 890 |
| Near byte limit | 1 | 93 / 112 | 33,384 / 33,620 | 18 |
| Byte-based chunks | 8 | 405 / 398 | 20,504 / 19,708 | 32 |

At fixed group size, cold-run RSS increased from 11,644 to 11,952 KiB (pool 1)
and 12,124 to 12,496 KiB (pool 2) when the total rose from 1,000 to 10,000;
commands grew from 49 to 445. For the 10,000-record cold run, maximum raw group
size was 947,996 bytes and maximum Bind frame was 393,617 bytes. Its bounded
2,000-payload sample ranged from 368 to 35,187 UTF-8 bytes (p50 393, p95 2,542),
with synthetic multibyte text, arrays and long fields across all four subject types.
The near-limit group contained 8,384,512 raw bytes and its largest Bind frame was
8,384,098 bytes. Its larger RSS reflects the bounded large input/encoding/relay
buffers. Constructor rejection, adapter preflight rejection, grouped command
bounds and multi-chunk rollback are asserted independently of these timings.
HTTP/workflow resource verification belongs to the future import orchestration work.

See the [application operations](crates/yomibu-app/src/application/mod.rs),
[storage contract](crates/yomibu-core/src/ports/infrastructure/mod.rs), and
[PostgreSQL adapters](crates/yomibu-ext/src/postgres.rs) for read/write guarantees,
deadline handling, and pool configuration responsibilities.

### Architecture checks

Use Python 3 and the [repository Rust toolchain](rust-toolchain.toml), which includes
`rust-src` for semantic analysis, plus Node.js/npm to install ast-grep. Install the pinned
[cargo-bylaw](https://github.com/danielgerlag/cargo-bylaw) and ast-grep tools using the
version variables and `cargo install` / `npm install` commands in
[CI](.github/workflows/ci.yml). For a local Bylaw installation, omit `--root` to use
Cargo's default installation directory. For ast-grep, use a writable local `--prefix`
and add its `node_modules/.bin` to your shell's `PATH`.
Bylaw's separate executable keeps its rust-analyzer dependencies out of Yomibu's
dependency graph.

Run from the repository root. `check_domain.py` invokes `cargo bylaw`; the provider
scan invokes `ast-grep` on `PATH`. Install both tools before running the Python
enforcement tests, which invoke them as well:

```sh
python3 -B tools/check_architecture.py
python3 -B tools/check_domain.py
ast-grep scan --rule tools/provider_placement.yml .
python3 -B -m unittest discover -s tools -p 'test_*.py' -v
```

The crate checker uses offline Cargo metadata without building the application.
Architecture checks run separately from `cargo test`; CI runs them on Linux and macOS.
See [verification](ARCHITECTURE.md#verification) for the rules each check covers and
the remaining review obligations.

### API documentation

Build API documentation with `cargo doc --locked --no-deps`; the output is in
`target/doc/yomibu_app`.
