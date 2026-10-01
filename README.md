# Yomibu

Yomibu is an unofficial WaniKani tool for personalized Japanese reading practice,
starting with a Rust CLI. Milestone 1 is complete through 1d: resilient full
synchronization, offline inspection of learner observations, and macOS/Linux CI.
The library now composes these use cases through `App` with file or in-memory
storage. Reading-practice features remain deferred.

```sh
WANIKANI_API_TOKEN=... cargo run -- sync
cargo run -- status
cargo run -- --data-dir /path/to/data status
```

Status reads `wanikani.json` in the selected directory (default `$HOME/.yomibu`).
It needs no token or network and never writes files. Errors return a nonzero exit
status with recovery guidance. The cache contains source learner state, not a
fixed definition of “known” material.

Sync reads the token only from the environment, fetches the complete account
state, and prints the saved summary. It holds an advisory writer lock and replaces
the cache only after validation. A different account requires another data
directory. Existing corrupt/unsupported caches are preserved for recovery.
Network, validation, and pre-replacement write failures leave the previous cache
usable. An error explicitly reporting uncertain durability means replacement
already occurred but synchronizing its directory failed.

To try the synthetic example without an account:

```sh
demo_dir=$(mktemp -d)
cp tests/fixtures/mixed.json "$demo_dir/wanikani.json"
cargo run -- status --data-dir "$demo_dir"
```

The recommended library entry point is `App`. Within a caller-owned Tokio runtime
with I/O and time enabled:

```rust,ignore
use yomibu::{App, adapters::{sources::wanikani::Client, stores::FileLearningStore}};

let mut app = App::new(FileLearningStore::new(data_dir))
    .with_source(Client::new(token)?);
let report = app.sync().await?;
let summary = app.status()?;
```

Replace the store with `InMemoryLearningStore::new()` for volatile storage; the
sync/status flow stays the same. `report.persistence` says whether the successful
write was volatile or durably acknowledged. Results own their data. Offline use
needs only `App::new(store).status()`, with no source or runtime. Store scope is
one WaniKani account, not an implicit current user. Constructors do not fetch or
write. Filesystem operations and domain calculations remain synchronous.

Lower-level access through `wanikani::Client`, `cache::SyncGuard`, `cache::load`,
and domain types remains available via adapter re-exports. Direct callers must
keep a sync guard alive around retrieval and replacement. Reuse the client to
retain rate-limit state. A custom base URL receives the supplied token and must
be trusted. Publicly constructed or directly deserialized sync data needs
validation; load, replace, and summarize validate automatically. Build API
documentation with `cargo doc --locked --no-deps` for typed error and cancellation
contracts; a persistence error does not always imply rollback.
Argument/environment handling, runtime startup, text output, and exit codes belong
to the binary. The repository pins Rust 1.98.1 with rustfmt and Clippy.

`domain::WaniKaniSyncData` replaces the earlier `domain::Snapshot` name;
`InvalidSnapshot` error variants are now `InvalidSyncData`. Rust callers must
update imports and matches. The schema-1 JSON key remains `snapshot`, so existing
cache files need no migration. This data covers one account's synchronization
interval, not the entire catalog or an instantaneous remote state.

The [design vocabulary](SPEC.md#design-vocabulary-and-composition) distinguishes
implemented types from future components such as `ExerciseGenerator` and
`LearnerKnowledgePolicy`. Generation, multi-source learners, SQL, and Cloud remain
future work; [ARCHITECTURE.md](ARCHITECTURE.md) records their intended composition.

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all
```

The [CI workflow](.github/workflows/ci.yml) runs those gates on `ubuntu-latest` and
`macos-latest` in parallel for pushes to `main` and pull requests. New commits
cancel older runs for the same branch or PR. It installs the repository toolchain,
requires the committed lockfile, and checks that it stays unchanged. Rust
dependency downloads and compiled dependencies are cached separately by platform
and compiler; only `main` saves caches, and PRs can restore them. Cache misses
still run every gate. No WaniKani secret is required.

See [SPEC.md](SPEC.md) for authoritative requirements,
[ARCHITECTURE.md](ARCHITECTURE.md) for responsibilities, file/repository structure,
and composition examples, [PLAN.md](PLAN.md) for milestones and TDD evidence, and
[AGENTS.md](AGENTS.md) for engineering rules.

## Rust notes for a Ruby developer

- `LexicalContent` is an enum with three data shapes. Pattern matching handles
  those shapes explicitly; kana-only vocabulary cannot hold a readings field.
- `Option` distinguishes absence from a value: missing lifecycle dates, unknown
  SRS systems, and no-review accuracy are different from zero-valued data.
- Summaries own one username string and their aggregates, so they outlive the
  input and application. Source collections are inspected by reference.
- `App<Store, Source>` selects dependencies through Rust generics. The compiler
  checks their contracts; no runtime dependency-injection container is needed.
  An `Arc` shares an immutable data version without copying its collections.
- `CacheError` and `ValidationError` are typed library boundaries. `?` propagates
  failures; `anyhow` is confined to executable orchestration and exit handling.
- Ordered maps make SRS output deterministic. Unlike Ruby's growable integers,
  Rust integers have fixed widths: review counts widen from `u64` to `u128`
  before aggregation, with conversion to floating point only for percentages.
- Async is confined to HTTP and retry waits. Validation, summary calculation,
  locking, and cache persistence stay synchronous.
- `SyncGuard` owns a file handle: leaving scope releases the lock even when `?`
  returns early. This replaces manual unlock/ensure bookkeeping.
- Private transport structs move strings and vectors into domain types. Serde's
  `Serialize` derives write the owned schema without storing API envelopes.
- `PartialEq` compares retained resource data for duplicate reconciliation; source
  timestamps participate, while ignored API preferences and mnemonics do not.
- `WriteError::BeforeReplacement` and `DurabilityUncertain` model different
  outcomes. A single generic exception would make safe recovery harder.
  `#[source]` exposes the underlying I/O cause through the standard error chain;
  including it in an error's display text alone does not do that.
- Dropping an async future cancels its work at an `await`. A saved rate-limit
  deadline stays in the client until the wait finishes, so a later fetch still
  observes the reset time.
- `tokio::join!` overlaps independent HTTP test scenarios on one runtime. Each
  keeps its own server/cache and real retry waits; a separate virtual-clock test
  checks the exact backoff deadlines without socket I/O.

The test suite (including two subprocess helpers) uses local mock/raw HTTP
servers, isolated directories, and child processes. It covers streamed limits,
deadlines, retry budgets, hostile pagination, later-page failures, storage faults,
writer contention, and process termination. Killed writers can leave private
staging files that later reads/writes ignore. Fault injection and process-kill
tests do not simulate power loss. The pre-migration 58-entry suite, formatting,
and Clippy passed on native macOS/arm64 and Debian Linux/arm64 in a container,
then on GitHub-hosted Ubuntu/x86_64 and macOS/arm64. Current migration verification
is recorded separately in `PLAN.md`; earlier runs do not validate later code.
No live account was used. Request deadlines and page limits
do not bound total refresh duration or collection size. See the 1d record in
[PLAN.md](PLAN.md) for full results and limits.

A test scheduling follow-up retained every HTTP/cache scenario and strengthened
the timer assertions. Native macOS test execution fell from about 17.8s to 8.1s,
excluding compilation. See the follow-up in [PLAN.md](PLAN.md) for validation and
hosted CI measurements; runner setup and compilation still contribute to CI time.
