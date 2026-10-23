# Yomibu

Yomibu is being built to create Japanese reading material matched to what a learner
already knows. The project restarted on 2026-10-23 and is being rebuilt one behaviour
at a time; there are no runnable commands yet.

Guides: [ARCHITECTURE.md](ARCHITECTURE.md) for the rules, [AGENTS.md](AGENTS.md) for
how work is done, and [GLOSSARY.md](GLOSSARY.md) for domain terms.

## Local PostgreSQL

Install and start Docker Desktop, then from the repository root start PostgreSQL 18
with pgvector:

```sh
docker compose up -d --wait --wait-timeout 60
```

The [Compose setup](compose.yaml) uses the same pinned image as CI and listens on
`127.0.0.1:55432`. It creates database `yomibu` with user `yomibu` and password
`yomibu-dev`, for local development only. On the first start with an empty volume it
applies the [retained schema](docs/plan/schema.postgresql.sql). Later starts reuse the
data; a changed SQL file does not migrate an existing database. After a schema change,
recreate the database with `docker compose down --volumes` and the start command above.

```sh
docker compose exec postgres psql -U yomibu -d yomibu   # SQL console
docker compose down                                      # stop, keep data
```

## Developing

Build with `cargo build --locked` and run tests with `cargo test --locked --all`.
[CI](.github/workflows/ci.yml) defines the complete checks and runs them on Linux.

### Tests

Tests that need PostgreSQL are marked ignored and run separately. Create a disposable
test database once, with the `vector` extension and no application tables:

```sh
docker compose exec postgres createdb -U yomibu yomibu_test
docker compose exec postgres psql -U yomibu -d yomibu_test -v ON_ERROR_STOP=1 -c 'CREATE EXTENSION vector;'
```

Then run:

```sh
YOMIBU_TEST_DATABASE_URL='postgres://yomibu:yomibu-dev@127.0.0.1:55432/yomibu_test' \
  cargo test --locked --all -- --ignored
```

Each test applies the schema in an isolated PostgreSQL schema and removes it on
success. A missing database fails the run; nothing substitutes another backend.

### Architecture checks

The checks need Python 3, the [repository toolchain](rust-toolchain.toml) with
`rust-src`, and Node.js for ast-grep. Install the pinned
[cargo-bylaw](https://github.com/danielgerlag/cargo-bylaw) and ast-grep versions
with the commands in [CI](.github/workflows/ci.yml), then run from the repository root:

```sh
python3 -B tools/check_architecture.py
python3 -B tools/check_domain.py
ast-grep scan --rule tools/provider_placement.yml .
```

[ARCHITECTURE.md](ARCHITECTURE.md#verification) lists what each check enforces.
