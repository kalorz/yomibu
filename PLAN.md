# Current plan

As of 2026-10-07. Use [README](README.md) for commands,
[ARCHITECTURE](ARCHITECTURE.md#current-module-map) for code, and [SPEC](SPEC.md) for contracts.

## Current status

Sync/status, learner inventories, dictionary management, offline analysis,
retrieval, story preview, and experimental sentence generation are implemented.
Generation makes one requested provider attempt. It does not produce validated
exercises or assess naturalness, meaning in context, or comprehension.

## Remaining work

- Compare local dense and hosted retrieval before choosing a default model.
  Only the nonsemantic lexical baseline has been measured. See [retrieval](docs/RETRIEVAL.md).
- Validated generation needs better linguistic evidence, quality review, candidate
  selection, and bounded repair. See [future design](ARCHITECTURE.md#future-composition).
- Quizzes, attempt history, adaptive targets, more sources, databases, and other
  interfaces remain future work. Implement them only when requested.

## Earlier evidence

A1 ended with a historical no-go; the analyzer remains active. The later safeguard
leaves object/predicate combinations unresolved. See [analysis history](docs/history/README.md#analysis-investigation).
The [G2 live comparison](docs/history/generation/G2_COMPARISON.md) remains unrun.

Keep this page short and current. Put routine results in the PR or chat.
[History](docs/history/README.md) preserves existing research and delivery records;
new work does not need a history entry unless requested.
