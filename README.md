# Yomibu

Yomibu is an unofficial WaniKani tool for personalized Japanese reading practice,
starting with a Rust CLI. Milestone 1c implements and exercises resilient full
synchronization and offline inspection of learner observations. Reading-practice
features remain deferred.

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

The library exposes `wanikani::Client`, `cache::SyncGuard`, `cache::load`, domain
structs/validation, and `Snapshot::summarize`. Keep a sync guard alive around
`client.fetch().await` and call `guard.replace(&snapshot)` after retrieval.
Argument/environment handling, runtime startup, text output, and exit codes belong
to the binary. The repository pins Rust 1.98.1 with rustfmt and Clippy.

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

See [SPEC.md](SPEC.md) for authoritative decisions, [PLAN.md](PLAN.md) for milestone
status and TDD evidence, and [AGENTS.md](AGENTS.md) for engineering rules.

## Rust notes for a Ruby developer

- `LexicalContent` is an enum with three data shapes. Pattern matching handles
  those shapes explicitly; kana-only vocabulary cannot hold a readings field.
- `Option` distinguishes absence from a value: missing lifecycle dates, unknown
  SRS systems, and no-review accuracy are different from zero-valued data.
- Summaries borrow the snapshot's username (`&str`) and inspect collections by
  reference. No cloned object graph or service-object hierarchy is needed.
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
- Dropping an async future cancels its work at an `await`. A saved rate-limit
  deadline stays in the client until the wait finishes, so a later fetch still
  observes the reset time.

The 56-entry suite (including two subprocess helpers) uses local mock/raw HTTP
servers, isolated directories, and child processes. It covers streamed limits,
deadlines, retry budgets, hostile pagination, later-page failures, storage faults,
writer contention, and process termination. Killed writers can leave private
staging files that later reads/writes ignore. Fault injection and process-kill
tests do not simulate power loss. Linux execution and CI remain for 1d; no live
account has been used for automated verification.
