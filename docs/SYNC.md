# Sync

Sync saves WaniKani observations for one account.
The saved data records progress; it does not prove mastery.

## CLI usage

Set `YOMIBU_WANIKANI_API_KEY` in the environment before sync:

```sh
cargo run --locked -- sync
cargo run --locked -- --data-dir /path/to/data sync
```

The default directory is `$HOME/.yomibu`. Use a separate directory for each account.

Sync always refreshes. It locks the writer before fetching and validates data
before replacing the cache.
Failures before replacement preserve the old cache. `DurabilityUncertain` means
replacement happened but directory synchronization failed; it does not mean rollback.

## Library usage

The caller supplies a Tokio runtime with I/O and timers for sync:

```rust,ignore
use yomibu::App;
use yomibu_components::{wanikani_source::Client, file_learning_store::FileLearningStore};

let mut app = App::new(FileLearningStore::new("/path/to/data"))
    .with_source(Client::new(token)?);
let report = app.sync().await?;
```

Constructors do no I/O.

See [SPEC](../SPEC.md#wanikani-synchronization) for cache and sync contracts,
and [Rustdoc](../crates/yomibu/src/application/mod.rs) for errors and cancellation. Direct file
callers must hold `SyncGuard` across retrieval and replacement. Reuse clients for
rate-limit state, and trust any custom endpoint that receives the token.
