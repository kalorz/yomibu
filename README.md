# Yomibu

Personalized Japanese reading practice, starting with a Rust CLI. Milestone 1a
implements offline inspection of a normalized WaniKani cache. Synchronization and
later practice features are not implemented yet.

```sh
cargo run -- status
cargo run -- --data-dir /path/to/data status
```

Status reads `wanikani.json` in the selected directory (default `$HOME/.yomibu`).
It needs no token or network and never writes files. Errors return a nonzero exit
status with recovery guidance. The cache contains source learner state, not a
fixed definition of “known” material.

To try the synthetic example without an account:

```sh
demo_dir=$(mktemp -d)
cp tests/fixtures/mixed.json "$demo_dir/wanikani.json"
cargo run -- status --data-dir "$demo_dir"
```

The library exposes `cache::load`, domain structs and validation, and
`Snapshot::summarize`. Argument/environment handling, text output, and exit codes
belong to the binary. The repository pins Rust 1.98.1 with rustfmt and Clippy.

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
- Domain work and local reads stay synchronous. A runtime and async functions
  would add no value to this milestone.
