# Current plan

Use [README](README.md) for commands,
[ARCHITECTURE](ARCHITECTURE.md#current-module-map) for code, and [SPEC](SPEC.md) for contracts.

## Current status

Auth, sync, learner inventories, and first-run passage generation are implemented.
Generation makes one requested provider attempt. It does not produce validated
exercises or assess naturalness, meaning in context, or comprehension.

## Remaining work

- Align vocabulary content with explicit spelling, reading, and sense; preserve
  unresolved source alternatives. Add SQLite import/write and common learner-read
  paths. See the
  [knowledge storage direction](SPEC.md#knowledge-storage-direction).
- Validated generation needs better linguistic evidence, quality review, candidate
  selection, and bounded repair. See [future design](ARCHITECTURE.md#future-composition).
- Quizzes, attempt history, adaptive targets, more sources, and other
  interfaces remain future work. Implement them only when requested.

## Earlier evidence

A1 ended with a historical no-go. See [analysis history](docs/history/README.md#analysis-investigation).
The [G2 live comparison](docs/history/generation/G2_COMPARISON.md) remains unrun.

Keep this page short and current. Put routine results in the PR or chat.
[History](docs/history/README.md) preserves existing research and delivery records;
new work does not need a history entry unless requested.
