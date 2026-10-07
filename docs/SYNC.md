# Sync and offline status

Sync saves WaniKani observations for one account. Status reads them offline.
The saved data records progress; it does not prove mastery.

## CLI usage

Set `YOMIBU_WANIKANI_API_KEY` in the environment before sync:

```sh
cargo run --locked -- sync
cargo run --locked -- status
cargo run --locked -- --data-dir /path/to/data status
```

The default directory is `$HOME/.yomibu`. Status reads `wanikani.json` without
network access or writes. Use a separate directory for each account.
The [README demo](../README.md#try-it-without-an-account) needs no account.

Explicit sync always refreshes. Story auto-sync follows the
[story cache policy](STORY_GENERATION.md#cache). Sync locks the writer before fetching and validates data before replacing the cache.
Failures before replacement preserve the old cache. `DurabilityUncertain` means
replacement happened but directory synchronization failed; it does not mean rollback.

## Library usage

The caller supplies a Tokio runtime with I/O and timers for sync:

```rust,ignore
use yomibu::{App, adapters::{sources::wanikani::Client, stores::FileLearningStore}};

let mut app = App::new(FileLearningStore::new("/path/to/data"))
    .with_source(Client::new(token)?);
let report = app.sync().await?;
let summary = app.status()?;
```

`InMemoryLearningStore` provides volatile storage. `report.persistence` states
what the write guarantees. Status needs only `App::new(store).status()`, without a
source or runtime. Constructors do no I/O.

See [SPEC](../SPEC.md#wanikani-synchronization) for cache and sync contracts,
and [Rustdoc](../crates/yomibu/src/app.rs) for errors and cancellation. Direct file
callers must hold `SyncGuard` across retrieval and replacement. Reuse clients for
rate-limit state, and trust any custom endpoint that receives the token.
